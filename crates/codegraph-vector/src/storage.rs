use crate::index::IndexConfig;
use codegraph_core::{CodeGraphError, NodeId, Result};
use faiss::index::IndexImpl;
use faiss::{read_index, write_index, Index};
use flate2::{read::GzDecoder, write::GzEncoder, Compression};
use memmap2::{Mmap, MmapOptions};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

/// Metadata for persistent vector storage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageMetadata {
    pub version: u32,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
    pub index_config: IndexConfig,
    pub num_vectors: usize,
    pub compression_enabled: bool,
    pub checksum: String,
}

/// Memory-mapped persistent storage for FAISS indices and embeddings
pub struct PersistentStorage {
    base_path: PathBuf,
    metadata: Arc<RwLock<StorageMetadata>>,
    embeddings_mmap: Arc<RwLock<Option<Mmap>>>,
    id_mapping_mmap: Arc<RwLock<Option<Mmap>>>,
}

impl PersistentStorage {
    const VERSION: u32 = 1;
    const INDEX_FILE: &'static str = "index.faiss";
    const EMBEDDINGS_FILE: &'static str = "embeddings.bin";
    const ID_MAPPING_FILE: &'static str = "id_mapping.bin";
    const METADATA_FILE: &'static str = "metadata.json";

    pub fn new(base_path: PathBuf) -> Result<Self> {
        std::fs::create_dir_all(&base_path).map_err(CodeGraphError::Io)?;

        let metadata_path = base_path.join(Self::METADATA_FILE);
        let metadata = if metadata_path.exists() {
            Self::load_metadata(&metadata_path)?
        } else {
            StorageMetadata {
                version: Self::VERSION,
                created_at: chrono::Utc::now(),
                updated_at: chrono::Utc::now(),
                index_config: IndexConfig::default(),
                num_vectors: 0,
                compression_enabled: true,
                checksum: String::new(),
            }
        };

        Ok(Self {
            base_path,
            metadata: Arc::new(RwLock::new(metadata)),
            embeddings_mmap: Arc::new(RwLock::new(None)),
            id_mapping_mmap: Arc::new(RwLock::new(None)),
        })
    }

    /// Load storage metadata from disk
    fn load_metadata(path: &Path) -> Result<StorageMetadata> {
        let file = File::open(path).map_err(CodeGraphError::Io)?;

        let reader = BufReader::new(file);
        let metadata: StorageMetadata =
            serde_json::from_reader(reader).map_err(CodeGraphError::Serialization)?;

        if metadata.version > Self::VERSION {
            return Err(CodeGraphError::Version(format!(
                "Storage version {} is newer than supported version {}",
                metadata.version,
                Self::VERSION
            )));
        }

        info!("Loaded storage metadata: {} vectors", metadata.num_vectors);
        Ok(metadata)
    }

    /// Save storage metadata to disk
    fn save_metadata(&self) -> Result<()> {
        let metadata_path = self.base_path.join(Self::METADATA_FILE);
        let file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&metadata_path)
            .map_err(CodeGraphError::Io)?;

        let writer = BufWriter::new(file);
        let metadata = self.metadata.read();

        serde_json::to_writer_pretty(writer, &*metadata).map_err(CodeGraphError::Serialization)?;

        debug!("Saved storage metadata to {:?}", metadata_path);
        Ok(())
    }

    /// Save FAISS index to disk with optional compression
    pub fn save_index(&self, index: &IndexImpl, config: &IndexConfig) -> Result<()> {
        let index_path = self.base_path.join(Self::INDEX_FILE);

        // Update metadata
        {
            let mut metadata = self.metadata.write();
            metadata.updated_at = chrono::Utc::now();
            metadata.index_config = config.clone();
            metadata.num_vectors = index.ntotal() as usize;
            metadata.compression_enabled = config.compression_level > 0;
        }

        if config.compression_level > 0 {
            self.save_compressed_index(index, &index_path, config.compression_level)?;
        } else {
            write_index(index, &index_path.to_string_lossy())
                .map_err(|e| CodeGraphError::Vector(format!("Failed to save index: {}", e)))?;
        }

        self.save_metadata()?;
        info!("Saved FAISS index to {:?}", index_path);
        Ok(())
    }

    /// Save index with compression
    fn save_compressed_index(
        &self,
        index: &IndexImpl,
        path: &Path,
        compression_level: u32,
    ) -> Result<()> {
        // First save to temporary uncompressed file
        let temp_path = path.with_extension("tmp");
        write_index(index, &temp_path.to_string_lossy()).map_err(|e| {
            CodeGraphError::Vector(format!("Failed to save temporary index: {}", e))
        })?;

        // Read and compress
        let input_file = File::open(&temp_path).map_err(CodeGraphError::Io)?;

        let output_file = File::create(path).map_err(CodeGraphError::Io)?;

        let mut encoder = GzEncoder::new(output_file, Compression::new(compression_level));
        let mut input_reader = BufReader::new(input_file);

        std::io::copy(&mut input_reader, &mut encoder).map_err(CodeGraphError::Io)?;

        encoder.finish().map_err(CodeGraphError::Io)?;

        // Remove temporary file
        std::fs::remove_file(&temp_path).map_err(CodeGraphError::Io)?;

        debug!("Compressed index saved to {:?}", path);
        Ok(())
    }

    /// Load FAISS index from disk with automatic decompression
    pub fn load_index(&self, config: &IndexConfig) -> Result<IndexImpl> {
        let index_path = self.base_path.join(Self::INDEX_FILE);

        if !index_path.exists() {
            return Err(CodeGraphError::NotFound("Index file not found".to_string()));
        }

        let metadata = self.metadata.read();

        let index = if metadata.compression_enabled {
            self.load_compressed_index(&index_path)?
        } else {
            read_index(&index_path.to_string_lossy())
                .map_err(|e| CodeGraphError::Vector(format!("Failed to load index: {}", e)))?
        };

        info!(
            "Loaded FAISS index from {:?} ({} vectors)",
            index_path,
            index.ntotal()
        );
        Ok(index)
    }

    /// Load compressed index
    fn load_compressed_index(&self, path: &Path) -> Result<IndexImpl> {
        let input_file = File::open(path).map_err(CodeGraphError::Io)?;

        let temp_path = path.with_extension("tmp_decomp");
        let output_file = File::create(&temp_path).map_err(CodeGraphError::Io)?;

        let mut decoder = GzDecoder::new(input_file);
        let mut output_writer = BufWriter::new(output_file);

        std::io::copy(&mut decoder, &mut output_writer).map_err(CodeGraphError::Io)?;

        drop(output_writer); // Ensure file is closed

        let index = read_index(&temp_path.to_string_lossy()).map_err(|e| {
            CodeGraphError::Vector(format!("Failed to load decompressed index: {}", e))
        })?;

        // Clean up temporary file
        std::fs::remove_file(&temp_path).map_err(CodeGraphError::Io)?;

        debug!("Decompressed and loaded index from {:?}", path);
        Ok(index)
    }

    /// Save embeddings to memory-mapped file for efficient access
    pub fn save_embeddings(&self, embeddings: &HashMap<NodeId, Vec<f32>>) -> Result<()> {
        let embeddings_path = self.base_path.join(Self::EMBEDDINGS_FILE);

        let serialized = bincode::serialize(embeddings).map_err(|e| {
            CodeGraphError::Vector(format!("Failed to serialize embeddings: {}", e))
        })?;

        std::fs::write(&embeddings_path, &serialized).map_err(CodeGraphError::Io)?;

        // Create memory map
        let file = OpenOptions::new()
            .read(true)
            .open(&embeddings_path)
            .map_err(CodeGraphError::Io)?;

        let mmap = unsafe { MmapOptions::new().map(&file).map_err(CodeGraphError::Io)? };

        *self.embeddings_mmap.write() = Some(mmap);

        info!(
            "Saved {} embeddings to memory-mapped file",
            embeddings.len()
        );
        Ok(())
    }

    /// Load embeddings from memory-mapped file
    pub fn load_embeddings(&self) -> Result<HashMap<NodeId, Vec<f32>>> {
        let embeddings_path = self.base_path.join(Self::EMBEDDINGS_FILE);

        if !embeddings_path.exists() {
            return Ok(HashMap::new());
        }

        // Create memory map if not already created
        if self.embeddings_mmap.read().is_none() {
            let file = File::open(&embeddings_path).map_err(CodeGraphError::Io)?;

            let mmap = unsafe { MmapOptions::new().map(&file).map_err(CodeGraphError::Io)? };

            *self.embeddings_mmap.write() = Some(mmap);
        }

        let mmap_guard = self.embeddings_mmap.read();
        let mmap = mmap_guard.as_ref().unwrap();

        let embeddings: HashMap<NodeId, Vec<f32>> =
            bincode::deserialize(&mmap[..]).map_err(|e| {
                CodeGraphError::Vector(format!("Failed to deserialize embeddings: {}", e))
            })?;

        info!(
            "Loaded {} embeddings from memory-mapped file",
            embeddings.len()
        );
        Ok(embeddings)
    }

    /// Save ID mapping to memory-mapped file
    pub fn save_id_mapping(
        &self,
        id_mapping: &HashMap<i64, NodeId>,
        reverse_mapping: &HashMap<NodeId, i64>,
    ) -> Result<()> {
        let mapping_data = (id_mapping, reverse_mapping);
        let id_mapping_path = self.base_path.join(Self::ID_MAPPING_FILE);

        let serialized = bincode::serialize(&mapping_data).map_err(|e| {
            CodeGraphError::Vector(format!("Failed to serialize ID mapping: {}", e))
        })?;

        std::fs::write(&id_mapping_path, &serialized).map_err(CodeGraphError::Io)?;

        // Create memory map
        let file = OpenOptions::new()
            .read(true)
            .open(&id_mapping_path)
            .map_err(CodeGraphError::Io)?;

        let mmap = unsafe { MmapOptions::new().map(&file).map_err(CodeGraphError::Io)? };

        *self.id_mapping_mmap.write() = Some(mmap);

        info!(
            "Saved ID mappings to memory-mapped file ({} entries)",
            id_mapping.len()
        );
        Ok(())
    }

    /// Load ID mapping from memory-mapped file
    pub fn load_id_mapping(&self) -> Result<(HashMap<i64, NodeId>, HashMap<NodeId, i64>)> {
        let id_mapping_path = self.base_path.join(Self::ID_MAPPING_FILE);

        if !id_mapping_path.exists() {
            return Ok((HashMap::new(), HashMap::new()));
        }

        // Create memory map if not already created
        if self.id_mapping_mmap.read().is_none() {
            let file = File::open(&id_mapping_path).map_err(CodeGraphError::Io)?;

            let mmap = unsafe { MmapOptions::new().map(&file).map_err(CodeGraphError::Io)? };

            *self.id_mapping_mmap.write() = Some(mmap);
        }

        let mmap_guard = self.id_mapping_mmap.read();
        let mmap = mmap_guard.as_ref().unwrap();

        let (id_mapping, reverse_mapping): (HashMap<i64, NodeId>, HashMap<NodeId, i64>) =
            bincode::deserialize(&mmap[..]).map_err(|e| {
                CodeGraphError::Vector(format!("Failed to deserialize ID mapping: {}", e))
            })?;

        info!(
            "Loaded ID mappings from memory-mapped file ({} entries)",
            id_mapping.len()
        );
        Ok((id_mapping, reverse_mapping))
    }

    /// Get storage statistics
    pub fn get_stats(&self) -> StorageStats {
        let metadata = self.metadata.read();
        let base_path_size = self.calculate_directory_size(&self.base_path);

        StorageStats {
            total_size_bytes: base_path_size,
            num_vectors: metadata.num_vectors,
            compression_enabled: metadata.compression_enabled,
            last_updated: metadata.updated_at,
        }
    }

    /// Calculate total size of storage directory
    fn calculate_directory_size(&self, path: &Path) -> u64 {
        std::fs::read_dir(path)
            .map(|entries| {
                entries
                    .filter_map(|entry| entry.ok())
                    .filter_map(|entry| entry.metadata().ok())
                    .map(|metadata| metadata.len())
                    .sum()
            })
            .unwrap_or(0)
    }

    /// Perform storage cleanup and optimization
    pub fn optimize(&self) -> Result<()> {
        info!("Performing storage optimization...");

        // Update metadata timestamp
        {
            let mut metadata = self.metadata.write();
            metadata.updated_at = chrono::Utc::now();
        }

        self.save_metadata()?;

        // TODO: Add more optimization strategies:
        // - Defragmentation
        // - Compression level adjustment
        // - Index restructuring based on usage patterns

        info!("Storage optimization completed");
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct StorageStats {
    pub total_size_bytes: u64,
    pub num_vectors: usize,
    pub compression_enabled: bool,
    pub last_updated: chrono::DateTime<chrono::Utc>,
}
