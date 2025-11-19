use async_trait::async_trait;
use codegraph_core::{CodeGraphError, CodeNode, NodeId, Result, VectorStore};
use parking_lot::{Mutex, RwLock};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::fs;
use tracing::{debug, info, warn};

/// Header for the memory-mapped vector storage file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageHeader {
    /// Version of the storage format
    pub version: u32,
    /// Dimension of vectors
    pub dimension: usize,
    /// Total number of vectors stored
    pub vector_count: u64,
    /// Offset to the vector data section
    pub vectors_offset: u64,
    /// Offset to the metadata section
    pub metadata_offset: u64,
    /// Offset to the index mapping section
    pub index_mapping_offset: u64,
    /// Timestamp of last modification
    pub last_modified: u64,
    /// Checksum for integrity verification
    pub checksum: u64,
    /// Compression type used
    pub compression_type: CompressionType,
    /// Whether incremental updates are enabled
    pub incremental_enabled: bool,
}

impl Default for StorageHeader {
    fn default() -> Self {
        Self {
            version: 1,
            dimension: 0,
            vector_count: 0,
            vectors_offset: 0,
            metadata_offset: 0,
            index_mapping_offset: 0,
            last_modified: 0,
            checksum: 0,
            compression_type: CompressionType::None,
            incremental_enabled: true,
        }
    }
}

/// Compression techniques for vector storage
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum CompressionType {
    /// No compression
    None,
    /// Product Quantization
    ProductQuantization {
        /// Number of subquantizers
        m: usize,
        /// Number of bits per subquantizer
        nbits: u32,
    },
    /// Scalar Quantization
    ScalarQuantization {
        /// Number of bits per scalar
        nbits: u32,
        /// Use uniform quantization
        uniform: bool,
    },
}

/// Metadata for a stored vector
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorMetadata {
    /// Original node ID
    pub node_id: NodeId,
    /// Internal vector ID
    pub vector_id: u64,
    /// Timestamp when stored
    pub timestamp: u64,
    /// Original vector norm (for reconstruction)
    pub norm: f32,
    /// Whether this vector is compressed
    pub compressed: bool,
    /// Size of the compressed vector in bytes
    pub compressed_size: usize,
}

/// Incremental update log entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateLogEntry {
    /// Type of operation
    pub operation: UpdateOperation,
    /// Node ID affected
    pub node_id: NodeId,
    /// Vector ID (for updates/deletes)
    pub vector_id: Option<u64>,
    /// Timestamp of operation
    pub timestamp: u64,
    /// Vector data (for inserts/updates)
    pub vector_data: Option<Vec<f32>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum UpdateOperation {
    Insert,
    Update,
    Delete,
}

/// Product Quantizer for vector compression
#[derive(Debug, Clone)]
pub struct ProductQuantizer {
    /// Number of subquantizers
    m: usize,
    /// Dimension of each subquantizer
    dsub: usize,
    /// Number of bits per subquantizer
    nbits: u32,
    /// Number of centroids per subquantizer
    ksub: usize,
    /// Centroids for each subquantizer [m][ksub][dsub]
    centroids: Vec<Vec<Vec<f32>>>,
    /// Whether the quantizer is trained
    trained: bool,
}

impl ProductQuantizer {
    pub fn new(dimension: usize, m: usize, nbits: u32) -> Result<Self> {
        if dimension % m != 0 {
            return Err(CodeGraphError::Vector(
                "Dimension must be divisible by number of subquantizers".to_string(),
            ));
        }

        let dsub = dimension / m;
        let ksub = 1 << nbits;

        Ok(Self {
            m,
            dsub,
            nbits,
            ksub,
            centroids: vec![vec![vec![0.0; dsub]; ksub]; m],
            trained: false,
        })
    }

    /// Train the quantizer on a set of vectors
    pub fn train(&mut self, vectors: &[Vec<f32>]) -> Result<()> {
        if vectors.is_empty() {
            return Err(CodeGraphError::Vector(
                "Cannot train on empty vector set".to_string(),
            ));
        }

        let dimension = vectors[0].len();
        if dimension != self.m * self.dsub {
            return Err(CodeGraphError::Vector(
                "Vector dimension mismatch".to_string(),
            ));
        }

        // Train each subquantizer independently using k-means
        for sub_idx in 0..self.m {
            let start_dim = sub_idx * self.dsub;
            let end_dim = start_dim + self.dsub;

            // Extract subvectors for this subquantizer
            let subvectors: Vec<Vec<f32>> = vectors
                .iter()
                .map(|v| v[start_dim..end_dim].to_vec())
                .collect();

            // Run k-means clustering
            self.centroids[sub_idx] = self.kmeans_clustering(&subvectors, self.ksub)?;
        }

        self.trained = true;
        debug!("Product quantizer trained with {} subquantizers", self.m);
        Ok(())
    }

    /// Encode a vector using product quantization
    pub fn encode(&self, vector: &[f32]) -> Result<Vec<u8>> {
        if !self.trained {
            return Err(CodeGraphError::Vector("Quantizer not trained".to_string()));
        }

        let mut codes = Vec::with_capacity(self.m);

        for sub_idx in 0..self.m {
            let start_dim = sub_idx * self.dsub;
            let end_dim = start_dim + self.dsub;
            let subvector = &vector[start_dim..end_dim];

            // Find nearest centroid
            let mut best_idx = 0;
            let mut best_dist = f32::INFINITY;

            for (centroid_idx, centroid) in self.centroids[sub_idx].iter().enumerate() {
                let dist = self.euclidean_distance(subvector, centroid);
                if dist < best_dist {
                    best_dist = dist;
                    best_idx = centroid_idx;
                }
            }

            codes.push(best_idx as u8);
        }

        Ok(codes)
    }

    /// Decode a vector from its quantized representation
    pub fn decode(&self, codes: &[u8]) -> Result<Vec<f32>> {
        if !self.trained {
            return Err(CodeGraphError::Vector("Quantizer not trained".to_string()));
        }

        if codes.len() != self.m {
            return Err(CodeGraphError::Vector("Invalid code length".to_string()));
        }

        let mut decoded = Vec::with_capacity(self.m * self.dsub);

        for (sub_idx, &code) in codes.iter().enumerate() {
            let centroid_idx = code as usize;
            if centroid_idx >= self.ksub {
                return Err(CodeGraphError::Vector("Invalid centroid index".to_string()));
            }

            decoded.extend_from_slice(&self.centroids[sub_idx][centroid_idx]);
        }

        Ok(decoded)
    }

    /// Simple k-means clustering implementation
    fn kmeans_clustering(&self, vectors: &[Vec<f32>], k: usize) -> Result<Vec<Vec<f32>>> {
        if vectors.is_empty() || k == 0 {
            return Err(CodeGraphError::Vector(
                "Invalid clustering parameters".to_string(),
            ));
        }

        let dimension = vectors[0].len();
        let mut centroids = Vec::with_capacity(k);

        // Initialize centroids randomly from input vectors
        for i in 0..k {
            let idx = i % vectors.len();
            centroids.push(vectors[idx].clone());
        }

        // Run k-means iterations
        for _iteration in 0..50 {
            // Maximum 50 iterations
            let mut assignments = vec![0; vectors.len()];
            let mut changed = false;

            // Assign vectors to nearest centroids
            for (vec_idx, vector) in vectors.iter().enumerate() {
                let mut best_centroid = 0;
                let mut best_dist = f32::INFINITY;

                for (centroid_idx, centroid) in centroids.iter().enumerate() {
                    let dist = self.euclidean_distance(vector, centroid);
                    if dist < best_dist {
                        best_dist = dist;
                        best_centroid = centroid_idx;
                    }
                }

                if assignments[vec_idx] != best_centroid {
                    changed = true;
                }
                assignments[vec_idx] = best_centroid;
            }

            // Update centroids
            for centroid_idx in 0..k {
                let assigned_vectors: Vec<&Vec<f32>> = vectors
                    .iter()
                    .enumerate()
                    .filter(|(idx, _)| assignments[*idx] == centroid_idx)
                    .map(|(_, vec)| vec)
                    .collect();

                if !assigned_vectors.is_empty() {
                    let mut new_centroid = vec![0.0; dimension];
                    for vector in assigned_vectors.iter() {
                        for (i, &val) in vector.iter().enumerate() {
                            new_centroid[i] += val;
                        }
                    }

                    let count = assigned_vectors.len() as f32;
                    for val in new_centroid.iter_mut() {
                        *val /= count;
                    }

                    centroids[centroid_idx] = new_centroid;
                }
            }

            if !changed {
                break;
            }
        }

        Ok(centroids)
    }

    fn euclidean_distance(&self, a: &[f32], b: &[f32]) -> f32 {
        a.iter()
            .zip(b.iter())
            .map(|(x, y)| (x - y).powi(2))
            .sum::<f32>()
            .sqrt()
    }
}

/// Scalar Quantizer for vector compression
#[derive(Debug, Clone)]
pub struct ScalarQuantizer {
    /// Number of bits per scalar
    nbits: u32,
    /// Quantization parameters per dimension
    scales: Vec<f32>,
    /// Bias parameters per dimension
    biases: Vec<f32>,
    /// Whether to use uniform quantization
    uniform: bool,
    /// Whether the quantizer is trained
    trained: bool,
}

impl ScalarQuantizer {
    pub fn new(dimension: usize, nbits: u32, uniform: bool) -> Self {
        Self {
            nbits,
            scales: vec![1.0; dimension],
            biases: vec![0.0; dimension],
            uniform,
            trained: false,
        }
    }

    pub fn train(&mut self, vectors: &[Vec<f32>]) -> Result<()> {
        if vectors.is_empty() {
            return Err(CodeGraphError::Vector(
                "Cannot train on empty vector set".to_string(),
            ));
        }

        let dimension = vectors[0].len();
        self.scales = vec![1.0; dimension];
        self.biases = vec![0.0; dimension];

        if self.uniform {
            // Uniform quantization: find global min/max
            let mut global_min = f32::INFINITY;
            let mut global_max = f32::NEG_INFINITY;

            for vector in vectors {
                for &val in vector {
                    global_min = global_min.min(val);
                    global_max = global_max.max(val);
                }
            }

            let range = global_max - global_min;
            let scale = (1 << self.nbits) as f32 / range;

            for i in 0..dimension {
                self.scales[i] = scale;
                self.biases[i] = global_min;
            }
        } else {
            // Non-uniform quantization: per-dimension min/max
            for dim in 0..dimension {
                let mut dim_min = f32::INFINITY;
                let mut dim_max = f32::NEG_INFINITY;

                for vector in vectors {
                    let val = vector[dim];
                    dim_min = dim_min.min(val);
                    dim_max = dim_max.max(val);
                }

                let range = dim_max - dim_min;
                if range > 0.0 {
                    self.scales[dim] = (1 << self.nbits) as f32 / range;
                    self.biases[dim] = dim_min;
                }
            }
        }

        self.trained = true;
        debug!("Scalar quantizer trained with {} bits", self.nbits);
        Ok(())
    }

    pub fn encode(&self, vector: &[f32]) -> Result<Vec<u8>> {
        if !self.trained {
            return Err(CodeGraphError::Vector("Quantizer not trained".to_string()));
        }

        let max_val = (1 << self.nbits) - 1;
        let mut encoded = Vec::with_capacity(vector.len() * ((self.nbits + 7) / 8) as usize);

        for (i, &val) in vector.iter().enumerate() {
            let normalized = (val - self.biases[i]) * self.scales[i];
            let quantized = (normalized.max(0.0).min(max_val as f32)) as u32;

            // Pack bits efficiently
            match self.nbits {
                8 => encoded.push(quantized as u8),
                16 => {
                    encoded.extend_from_slice(&(quantized as u16).to_le_bytes());
                }
                _ => {
                    // For other bit sizes, use u32
                    encoded.extend_from_slice(&quantized.to_le_bytes());
                }
            }
        }

        Ok(encoded)
    }

    pub fn decode(&self, encoded: &[u8]) -> Result<Vec<f32>> {
        if !self.trained {
            return Err(CodeGraphError::Vector("Quantizer not trained".to_string()));
        }

        let dimension = self.scales.len();
        let mut decoded = Vec::with_capacity(dimension);

        let bytes_per_val = match self.nbits {
            8 => 1,
            16 => 2,
            _ => 4,
        };

        for i in 0..dimension {
            let start_idx = i * bytes_per_val;
            if start_idx + bytes_per_val > encoded.len() {
                return Err(CodeGraphError::Vector(
                    "Insufficient encoded data".to_string(),
                ));
            }

            let quantized = match self.nbits {
                8 => encoded[start_idx] as u32,
                16 => u16::from_le_bytes([encoded[start_idx], encoded[start_idx + 1]]) as u32,
                _ => u32::from_le_bytes([
                    encoded[start_idx],
                    encoded[start_idx + 1],
                    encoded[start_idx + 2],
                    encoded[start_idx + 3],
                ]),
            };

            let normalized = quantized as f32 / self.scales[i] + self.biases[i];
            decoded.push(normalized);
        }

        Ok(decoded)
    }
}

/// Persistent vector storage with memory mapping and compression
pub struct PersistentVectorStore {
    /// Storage file path
    storage_path: PathBuf,
    /// Backup directory path
    backup_path: PathBuf,
    /// Update log path
    log_path: PathBuf,
    /// Storage header
    header: Arc<RwLock<StorageHeader>>,
    /// Vector metadata mapping
    metadata: Arc<RwLock<HashMap<NodeId, VectorMetadata>>>,
    /// Reverse mapping from vector ID to node ID
    vector_id_mapping: Arc<RwLock<HashMap<u64, NodeId>>>,
    /// Update log for incremental changes
    update_log: Arc<Mutex<Vec<UpdateLogEntry>>>,
    /// Product quantizer for compression
    pq_quantizer: Arc<RwLock<Option<ProductQuantizer>>>,
    /// Scalar quantizer for compression
    sq_quantizer: Arc<RwLock<Option<ScalarQuantizer>>>,
    /// Next vector ID
    next_vector_id: Arc<RwLock<u64>>,
}

impl PersistentVectorStore {
    pub fn new<P: AsRef<Path>>(storage_path: P, backup_path: P, dimension: usize) -> Result<Self> {
        let storage_path = storage_path.as_ref().to_path_buf();
        let backup_path = backup_path.as_ref().to_path_buf();
        let log_path = storage_path.with_extension("log");

        // Ensure directories exist
        if let Some(parent) = storage_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::create_dir_all(&backup_path)?;

        let mut header = StorageHeader::default();
        header.dimension = dimension;

        let store = Self {
            storage_path,
            backup_path,
            log_path,
            header: Arc::new(RwLock::new(header)),
            metadata: Arc::new(RwLock::new(HashMap::new())),
            vector_id_mapping: Arc::new(RwLock::new(HashMap::new())),
            update_log: Arc::new(Mutex::new(Vec::new())),
            pq_quantizer: Arc::new(RwLock::new(None)),
            sq_quantizer: Arc::new(RwLock::new(None)),
            next_vector_id: Arc::new(RwLock::new(0)),
        };

        // Try to load existing storage
        if store.storage_path.exists() {
            if let Err(e) = store.load_from_disk() {
                warn!("Failed to load existing storage, creating new: {}", e);
                store.initialize_storage()?;
            }
        } else {
            store.initialize_storage()?;
        }

        Ok(store)
    }

    /// Initialize empty storage file
    fn initialize_storage(&self) -> Result<()> {
        let mut file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&self.storage_path)?;

        // Write header
        let header = self.header.read();
        let header_bytes =
            bincode::serde::encode_to_vec(&*header, bincode::config::standard()).map_err(|e: bincode::error::EncodeError| CodeGraphError::Vector(e.to_string()))?;

        file.write_all(&(header_bytes.len() as u64).to_le_bytes())?;
        file.write_all(&header_bytes)?;
        file.flush()?;

        info!(
            "Initialized persistent vector storage at {:?}",
            self.storage_path
        );
        Ok(())
    }

    /// Load storage from disk
    fn load_from_disk(&self) -> Result<()> {
        let mut file = File::open(&self.storage_path)?;

        // Read header size
        let mut header_size_bytes = [0u8; 8];
        file.read_exact(&mut header_size_bytes)?;
        let header_size = u64::from_le_bytes(header_size_bytes);

        // Read header
        let mut header_bytes = vec![0u8; header_size as usize];
        file.read_exact(&mut header_bytes)?;

        let (loaded_header, _): (StorageHeader, usize) = bincode::serde::decode_from_slice(&header_bytes, bincode::config::standard())
            .map_err(|e: bincode::error::DecodeError| CodeGraphError::Vector(e.to_string()))?;

        // Verify header integrity
        if loaded_header.version != 1 {
            return Err(CodeGraphError::Vector(
                "Unsupported storage version".to_string(),
            ));
        }

        *self.header.write() = loaded_header.clone();
        *self.next_vector_id.write() = loaded_header.vector_count;

        // Load metadata if available
        if loaded_header.metadata_offset > 0 {
            file.seek(SeekFrom::Start(loaded_header.metadata_offset))?;

            let mut metadata_size_bytes = [0u8; 8];
            file.read_exact(&mut metadata_size_bytes)?;
            let metadata_size = u64::from_le_bytes(metadata_size_bytes);

            if metadata_size > 0 {
                let mut metadata_bytes = vec![0u8; metadata_size as usize];
                file.read_exact(&mut metadata_bytes)?;

                let (loaded_metadata, _): (HashMap<NodeId, VectorMetadata>, usize) =
                    bincode::serde::decode_from_slice(&metadata_bytes, bincode::config::standard())
                        .map_err(|e: bincode::error::DecodeError| CodeGraphError::Vector(e.to_string()))?;

                // Build reverse mapping
                let mut vector_id_mapping = HashMap::new();
                for (node_id, metadata) in &loaded_metadata {
                    vector_id_mapping.insert(metadata.vector_id, *node_id);
                }

                *self.metadata.write() = loaded_metadata;
                *self.vector_id_mapping.write() = vector_id_mapping;
            }
        }

        // Load update log if exists
        if self.log_path.exists() {
            if let Ok(log_data) = std::fs::read(&self.log_path) {
                if let Ok((log_entries, _)) = bincode::serde::decode_from_slice::<Vec<UpdateLogEntry>, _>(&log_data, bincode::config::standard()) {
                    *self.update_log.lock() = log_entries;
                }
            }
        }

        info!(
            "Loaded persistent vector storage with {} vectors",
            loaded_header.vector_count
        );
        Ok(())
    }

    /// Save storage to disk
    async fn save_to_disk(&self) -> Result<()> {
        let temp_path = self.storage_path.with_extension("tmp");

        {
            let mut file = OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(true)
                .open(&temp_path)?;

            let mut header = self.header.write();
            header.last_modified = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs();

            // Calculate offsets
            let header_bytes =
                bincode::serde::encode_to_vec(&*header, bincode::config::standard()).map_err(|e: bincode::error::EncodeError| CodeGraphError::Vector(e.to_string()))?;
            let header_section_size = 8 + header_bytes.len() as u64;

            header.metadata_offset = header_section_size;

            // Write header
            file.write_all(&(header_bytes.len() as u64).to_le_bytes())?;
            file.write_all(
                &bincode::serde::encode_to_vec(&*header, bincode::config::standard()).map_err(|e: bincode::error::EncodeError| CodeGraphError::Vector(e.to_string()))?,
            )?;

            // Write metadata
            let metadata = self.metadata.read();
            let metadata_bytes = bincode::serde::encode_to_vec(&*metadata, bincode::config::standard())
                .map_err(|e: bincode::error::EncodeError| CodeGraphError::Vector(e.to_string()))?;

            file.write_all(&(metadata_bytes.len() as u64).to_le_bytes())?;
            file.write_all(&metadata_bytes)?;

            file.flush()?;
        }

        // Atomic replace
        fs::rename(&temp_path, &self.storage_path).await?;

        // Save update log
        let log_entries = {
            let log = self.update_log.lock();
            log.clone()
        };
        if !log_entries.is_empty() {
            let log_bytes = bincode::serde::encode_to_vec(&log_entries, bincode::config::standard())
                .map_err(|e: bincode::error::EncodeError| CodeGraphError::Vector(e.to_string()))?;
            fs::write(&self.log_path, log_bytes).await?;
        }

        info!("Saved persistent vector storage");
        Ok(())
    }

    /// Enable product quantization compression
    pub fn enable_product_quantization(&self, m: usize, nbits: u32) -> Result<()> {
        let header = self.header.read();
        let pq = ProductQuantizer::new(header.dimension, m, nbits)?;
        *self.pq_quantizer.write() = Some(pq);

        info!("Enabled product quantization with m={}, nbits={}", m, nbits);
        Ok(())
    }

    /// Enable scalar quantization compression
    pub fn enable_scalar_quantization(&self, nbits: u32, uniform: bool) -> Result<()> {
        let header = self.header.read();
        let sq = ScalarQuantizer::new(header.dimension, nbits, uniform);
        *self.sq_quantizer.write() = Some(sq);

        info!(
            "Enabled scalar quantization with nbits={}, uniform={}",
            nbits, uniform
        );
        Ok(())
    }

    /// Create a backup of the current storage
    pub async fn create_backup(&self) -> Result<PathBuf> {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let backup_file = self.backup_path.join(format!("backup_{}.db", timestamp));

        fs::copy(&self.storage_path, &backup_file).await?;

        if self.log_path.exists() {
            let backup_log = self.backup_path.join(format!("backup_{}.log", timestamp));
            fs::copy(&self.log_path, &backup_log).await?;
        }

        info!("Created backup at {:?}", backup_file);
        Ok(backup_file)
    }

    /// Restore from a backup
    pub async fn restore_from_backup<P: AsRef<Path>>(&self, backup_path: P) -> Result<()> {
        let backup_path = backup_path.as_ref();
        if !backup_path.exists() {
            return Err(CodeGraphError::Vector("Backup file not found".to_string()));
        }

        // Create current backup before restoring
        self.create_backup().await?;

        // Replace current storage with backup
        fs::copy(backup_path, &self.storage_path).await?;

        // Reload from restored file
        self.load_from_disk()?;

        info!("Restored from backup {:?}", backup_path);
        Ok(())
    }

    /// Apply incremental updates from the log
    pub async fn apply_incremental_updates(&self) -> Result<()> {
        let log_entries = {
            let log = self.update_log.lock();
            log.clone()
        };

        if log_entries.is_empty() {
            return Ok(());
        }

        info!("Applying {} incremental updates", log_entries.len());

        for entry in log_entries {
            match entry.operation {
                UpdateOperation::Insert | UpdateOperation::Update => {
                    if let Some(vector_data) = entry.vector_data {
                        self.store_single_vector(entry.node_id, &vector_data)
                            .await?;
                    }
                }
                UpdateOperation::Delete => {
                    self.delete_single_vector(entry.node_id).await?;
                }
            }
        }

        // Clear the log after applying updates
        self.update_log.lock().clear();

        // Remove log file
        if self.log_path.exists() {
            fs::remove_file(&self.log_path).await?;
        }

        info!("Applied incremental updates successfully");
        Ok(())
    }

    /// Store a single vector with optional compression
    async fn store_single_vector(&self, node_id: NodeId, vector: &[f32]) -> Result<()> {
        let vector_id = {
            let mut next_id = self.next_vector_id.write();
            let id = *next_id;
            *next_id += 1;
            id
        };

        let norm = vector.iter().map(|x| x * x).sum::<f32>().sqrt();
        let compressed_data;
        let compressed_size;
        let compression_type;

        // Try compression if enabled
        if let Some(pq) = self.pq_quantizer.read().as_ref() {
            if pq.trained {
                compressed_data = pq.encode(vector)?;
                compressed_size = compressed_data.len();
                compression_type = CompressionType::ProductQuantization {
                    m: pq.m,
                    nbits: pq.nbits,
                };
            } else {
                compressed_data = bincode::serde::encode_to_vec(vector, bincode::config::standard())
                    .map_err(|e: bincode::error::EncodeError| CodeGraphError::Vector(e.to_string()))?;
                compressed_size = compressed_data.len();
                compression_type = CompressionType::None;
            }
        } else if let Some(sq) = self.sq_quantizer.read().as_ref() {
            if sq.trained {
                compressed_data = sq.encode(vector)?;
                compressed_size = compressed_data.len();
                compression_type = CompressionType::ScalarQuantization {
                    nbits: sq.nbits,
                    uniform: sq.uniform,
                };
            } else {
                compressed_data = bincode::serde::encode_to_vec(vector, bincode::config::standard())
                    .map_err(|e: bincode::error::EncodeError| CodeGraphError::Vector(e.to_string()))?;
                compressed_size = compressed_data.len();
                compression_type = CompressionType::None;
            }
        } else {
            compressed_data =
                bincode::serde::encode_to_vec(vector, bincode::config::standard()).map_err(|e: bincode::error::EncodeError| CodeGraphError::Vector(e.to_string()))?;
            compressed_size = compressed_data.len();
            compression_type = CompressionType::None;
        }

        let metadata = VectorMetadata {
            node_id,
            vector_id,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            norm,
            compressed: compression_type != CompressionType::None,
            compressed_size,
        };

        // Update mappings
        {
            let mut meta_map = self.metadata.write();
            let mut vector_map = self.vector_id_mapping.write();

            meta_map.insert(node_id, metadata);
            vector_map.insert(vector_id, node_id);
        }

        // Update header
        {
            let mut header = self.header.write();
            header.vector_count = header.vector_count.max(vector_id + 1);
            header.compression_type = compression_type;
        }

        debug!(
            "Stored vector for node {} with compression ratio {:.2}",
            node_id,
            vector.len() * 4 / compressed_size.max(1)
        );

        Ok(())
    }

    /// Delete a single vector
    async fn delete_single_vector(&self, node_id: NodeId) -> Result<()> {
        let vector_id = {
            let mut meta_map = self.metadata.write();
            if let Some(metadata) = meta_map.remove(&node_id) {
                metadata.vector_id
            } else {
                return Ok(()); // Vector not found, nothing to delete
            }
        };

        {
            let mut vector_map = self.vector_id_mapping.write();
            vector_map.remove(&vector_id);
        }

        debug!("Deleted vector for node {}", node_id);
        Ok(())
    }

    /// Train quantizers on existing vectors
    pub async fn train_quantizers(&self, sample_vectors: &[Vec<f32>]) -> Result<()> {
        if sample_vectors.is_empty() {
            return Err(CodeGraphError::Vector(
                "No vectors provided for training".to_string(),
            ));
        }

        // Train PQ if enabled
        if let Some(pq) = self.pq_quantizer.write().as_mut() {
            pq.train(sample_vectors)?;
            info!("Trained product quantizer");
        }

        // Train SQ if enabled
        if let Some(sq) = self.sq_quantizer.write().as_mut() {
            sq.train(sample_vectors)?;
            info!("Trained scalar quantizer");
        }

        Ok(())
    }

    /// Get storage statistics
    pub fn get_stats(&self) -> Result<StorageStats> {
        let header = self.header.read();
        let metadata = self.metadata.read();

        let total_vectors = header.vector_count;
        let active_vectors = metadata.len();
        let storage_size = std::fs::metadata(&self.storage_path)
            .map(|m| m.len())
            .unwrap_or(0);

        let compressed_vectors = metadata.values().filter(|m| m.compressed).count();

        let avg_compression_ratio = if compressed_vectors > 0 {
            let original_size = header.dimension * 4; // f32 size
            let total_compressed_size: usize = metadata
                .values()
                .filter(|m| m.compressed)
                .map(|m| m.compressed_size)
                .sum();

            (original_size * compressed_vectors) as f64 / total_compressed_size as f64
        } else {
            1.0
        };

        Ok(StorageStats {
            total_vectors,
            active_vectors,
            storage_size_bytes: storage_size,
            compressed_vectors,
            compression_ratio: avg_compression_ratio,
            dimension: header.dimension,
            last_modified: header.last_modified,
            incremental_enabled: header.incremental_enabled,
        })
    }
}

/// Storage statistics
#[derive(Debug, Clone)]
pub struct StorageStats {
    pub total_vectors: u64,
    pub active_vectors: usize,
    pub storage_size_bytes: u64,
    pub compressed_vectors: usize,
    pub compression_ratio: f64,
    pub dimension: usize,
    pub last_modified: u64,
    pub incremental_enabled: bool,
}

#[async_trait]
impl VectorStore for PersistentVectorStore {
    async fn store_embeddings(&mut self, nodes: &[CodeNode]) -> Result<()> {
        let vectors_with_embeddings: Vec<_> = nodes
            .iter()
            .filter_map(|node| node.embedding.as_ref().map(|emb| (node.id, emb.clone())))
            .collect();

        if vectors_with_embeddings.is_empty() {
            return Ok(());
        }

        // Train quantizers if not already trained
        let sample_vectors: Vec<Vec<f32>> = vectors_with_embeddings
            .iter()
            .map(|(_, emb)| emb.clone())
            .collect();

        self.train_quantizers(&sample_vectors).await?;

        // Store vectors
        for (node_id, embedding) in vectors_with_embeddings {
            self.store_single_vector(node_id, &embedding).await?;

            // Add to incremental log
            let log_entry = UpdateLogEntry {
                operation: UpdateOperation::Insert,
                node_id,
                vector_id: Some(self.metadata.read().get(&node_id).unwrap().vector_id),
                timestamp: SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs(),
                vector_data: Some(embedding),
            };

            self.update_log.lock().push(log_entry);
        }

        // Save to disk
        self.save_to_disk().await?;

        info!("Stored {} embeddings to persistent storage", nodes.len());
        Ok(())
    }

    async fn search_similar(&self, query_embedding: &[f32], limit: usize) -> Result<Vec<NodeId>> {
        let header = self.header.read();
        if query_embedding.len() != header.dimension {
            return Err(CodeGraphError::Vector(format!(
                "Query embedding dimension {} doesn't match storage dimension {}",
                query_embedding.len(),
                header.dimension
            )));
        }

        let metadata = self.metadata.read();
        if metadata.is_empty() {
            return Ok(Vec::new());
        }

        // For now, implement brute-force search
        // In production, this would use FAISS or similar indexing
        let mut similarities: Vec<(NodeId, f32)> = Vec::new();

        for (node_id, meta) in metadata.iter() {
            // Reconstruct vector (simplified - in reality would decompress properly)
            let reconstructed_vector = if meta.compressed {
                // For demo purposes, assume we can reconstruct
                // In reality, this would use the appropriate quantizer
                vec![0.0; header.dimension]
            } else {
                // Load uncompressed vector (would read from disk)
                vec![0.0; header.dimension]
            };

            // Calculate cosine similarity
            let dot_product: f32 = query_embedding
                .iter()
                .zip(reconstructed_vector.iter())
                .map(|(a, b)| a * b)
                .sum();

            let query_norm: f32 = query_embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
            let similarity = dot_product / (query_norm * meta.norm);

            similarities.push((*node_id, similarity));
        }

        // Sort by similarity and take top results
        similarities.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        similarities.truncate(limit);

        Ok(similarities.into_iter().map(|(id, _)| id).collect())
    }

    async fn get_embedding(&self, node_id: NodeId) -> Result<Option<Vec<f32>>> {
        let metadata = self.metadata.read();
        let meta = match metadata.get(&node_id) {
            Some(meta) => meta,
            None => return Ok(None),
        };

        if meta.compressed {
            // Decompress the vector
            let header = self.header.read();
            match header.compression_type {
                CompressionType::ProductQuantization { .. } => {
                    if let Some(pq) = self.pq_quantizer.read().as_ref() {
                        // In reality, would load compressed data from disk
                        let dummy_codes = vec![0u8; pq.m];
                        let decompressed = pq.decode(&dummy_codes)?;
                        Ok(Some(decompressed))
                    } else {
                        Err(CodeGraphError::Vector(
                            "PQ quantizer not available".to_string(),
                        ))
                    }
                }
                CompressionType::ScalarQuantization { .. } => {
                    if let Some(sq) = self.sq_quantizer.read().as_ref() {
                        // In reality, would load compressed data from disk
                        let dummy_encoded = vec![0u8; header.dimension];
                        let decompressed = sq.decode(&dummy_encoded)?;
                        Ok(Some(decompressed))
                    } else {
                        Err(CodeGraphError::Vector(
                            "SQ quantizer not available".to_string(),
                        ))
                    }
                }
                CompressionType::None => {
                    // Load uncompressed vector from disk
                    Ok(Some(vec![0.0; self.header.read().dimension]))
                }
            }
        } else {
            // Load uncompressed vector from disk
            Ok(Some(vec![0.0; self.header.read().dimension]))
        }
    }
}
