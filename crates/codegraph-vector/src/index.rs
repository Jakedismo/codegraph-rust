#[cfg(feature = "persistent")]
use crate::storage::PersistentStorage;
use codegraph_core::{CodeGraphError, NodeId, Result};
use faiss::index::{Idx, IndexImpl};
use faiss::selector::IdSelector;
use faiss::{Index, MetricType};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
#[cfg(feature = "persistent")]
use std::sync::Arc;
use tracing::{debug, info};
#[cfg(not(feature = "gpu"))]
use tracing::warn;

/// Configuration for different FAISS index types optimized for various use cases
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexConfig {
    pub index_type: IndexType,
    #[serde(with = "crate::serde_utils::metric_type")]
    pub metric_type: MetricType,
    pub dimension: usize,
    pub training_size_threshold: usize,
    pub gpu_enabled: bool,
    pub compression_level: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum IndexType {
    /// Flat index - exact search, good for small datasets (&lt;10K vectors)
    Flat,
    /// IVF (Inverted File) index - good balance of speed/accuracy for medium datasets
    IVF {
        nlist: usize,  // Number of clusters (sqrt(N) is typical)
        nprobe: usize, // Number of clusters to search (1-nlist)
    },
    /// HNSW (Hierarchical Navigable Small World) - excellent for high-dimensional data
    HNSW {
        m: usize,               // Number of bi-directional links for each node (4-64)
        ef_construction: usize, // Size of dynamic candidate list (100-800)
        ef_search: usize,       // Search time accuracy/speed tradeoff (10-500)
    },
    /// LSH (Locality Sensitive Hashing) - very fast approximate search
    LSH {
        nbits: usize, // Number of hash bits (typically 1024-4096)
    },
    /// IVF+PQ hybrid (recommended for large datasets, memory-efficient)
    IVFPQ {
        nlist: usize,  // coarse centroids
        m: usize,      // sub-quantizers
        nbits: usize,  // bits per sub-quantizer
        nprobe: usize, // number of coarse lists to probe at search time
    },
    /// Product Quantization - memory efficient for large datasets
    PQ {
        m: usize,     // Number of sub-quantizers (multiple of dimension)
        nbits: usize, // Bits per sub-quantizer (1-16)
    },
    /// Hybrid approach combining multiple techniques for optimal performance
    Hybrid {
        coarse_quantizer: Box<IndexType>,
        fine_quantizer: Box<IndexType>,
    },
}

impl Default for IndexConfig {
    fn default() -> Self {
        Self {
            // Default to an IVFPQ tuned for 768-dim transformer embeddings
            index_type: IndexType::IVFPQ {
                nlist: 4096, // good for ~1M vectors
                m: 96,       // 768 / 96 = 8 dims per sub-vector
                nbits: 8,    // 8-bit codes => 1 byte per sub-vector
                nprobe: 64,  // search 64 lists
            },
            metric_type: MetricType::InnerProduct,
            dimension: 768, // Common transformer embedding dimension
            training_size_threshold: 10000,
            gpu_enabled: false,
            compression_level: 6,
        }
    }
}

impl IndexConfig {
    /// Create configuration optimized for sub-millisecond search
    pub fn fast_search(dimension: usize) -> Self {
        Self {
            index_type: IndexType::HNSW {
                m: 32,
                ef_construction: 400,
                ef_search: 32,
            },
            metric_type: MetricType::InnerProduct,
            dimension,
            training_size_threshold: 5000,
            gpu_enabled: true,
            compression_level: 3,
        }
    }

    /// Create configuration optimized for memory efficiency
    pub fn memory_efficient(dimension: usize) -> Self {
        Self {
            index_type: IndexType::IVFPQ {
                nlist: 4096,
                m: (dimension / 8).max(1),
                nbits: 8,
                nprobe: 32,
            },
            metric_type: MetricType::L2,
            dimension,
            training_size_threshold: 50000,
            gpu_enabled: false,
            compression_level: 9,
        }
    }

    /// Create configuration for balanced performance
    pub fn balanced(dimension: usize) -> Self {
        Self {
            index_type: IndexType::IVFPQ {
                nlist: 4096,
                m: (dimension / 8).max(1),
                nbits: 8,
                nprobe: 64,
            },
            metric_type: MetricType::InnerProduct,
            dimension,
            training_size_threshold: 20000,
            gpu_enabled: true,
            compression_level: 6,
        }
    }

    /// Generate FAISS index factory string for the configuration
    pub fn to_factory_string(&self) -> String {
        match &self.index_type {
            IndexType::Flat => "Flat".to_string(),
            IndexType::IVF { nlist, nprobe: _ } => {
                format!("IVF{},Flat", nlist)
            }
            IndexType::IVFPQ {
                nlist, m, nbits, ..
            } => {
                // Build a composite factory string: IVF coarse + PQ codes
                format!("IVF{},PQ{}x{}", nlist, m, nbits)
            }
            IndexType::HNSW { m, .. } => {
                format!("HNSW{}", m)
            }
            IndexType::LSH { nbits } => {
                format!("LSH{}", nbits)
            }
            IndexType::PQ { m, nbits } => {
                format!("PQ{}x{}", m, nbits)
            }
            IndexType::Hybrid {
                coarse_quantizer,
                fine_quantizer,
            } => {
                let coarse_config = IndexConfig {
                    index_type: *coarse_quantizer.clone(),
                    ..*self
                };
                let fine_config = IndexConfig {
                    index_type: *fine_quantizer.clone(),
                    ..*self
                };
                format!(
                    "{},{}",
                    coarse_config.to_factory_string(),
                    fine_config.to_factory_string()
                )
            }
        }
    }

    /// Configure search parameters after index creation
    pub fn configure_search_params(&self, index: &mut IndexImpl) -> Result<()> {
        // Not all IndexImpl variants expose tuning methods; keep as no-op for portability.
        let _ = index; // suppress unused var warning if features differ
        Ok(())
    }
}

/// High-performance FAISS index manager with multiple index type support
pub struct FaissIndexManager {
    config: IndexConfig,
    index: RwLock<Option<IndexImpl>>,
    // Stable ID mappings to support add_with_ids/remove/search by NodeId
    id_mapping: RwLock<HashMap<i64, NodeId>>, // faiss_id -> NodeId
    reverse_mapping: RwLock<HashMap<NodeId, i64>>, // NodeId -> faiss_id
    next_id: RwLock<i64>,
    #[cfg(feature = "persistent")]
    storage: Option<Arc<PersistentStorage>>,
    #[cfg(feature = "gpu")]
    gpu_resources: Option<faiss::gpu::StandardGpuResources>,
}

impl FaissIndexManager {
    pub fn new(config: IndexConfig) -> Self {
        Self {
            config,
            index: RwLock::new(None),
            id_mapping: RwLock::new(HashMap::new()),
            reverse_mapping: RwLock::new(HashMap::new()),
            next_id: RwLock::new(0),
            #[cfg(feature = "persistent")]
            storage: None,
            #[cfg(feature = "gpu")]
            gpu_resources: None,
        }
    }

    #[cfg(feature = "persistent")]
    pub fn with_persistence(mut self, storage_path: PathBuf) -> Result<Self> {
        self.storage = Some(Arc::new(PersistentStorage::new(storage_path)?));
        Ok(self)
    }

    #[cfg(not(feature = "persistent"))]
    pub fn with_persistence(self, _storage_path: PathBuf) -> Result<Self> {
        Ok(self)
    }

    /// Initialize GPU resources if GPU acceleration is enabled
    pub fn init_gpu(&mut self) -> Result<()> {
        if self.config.gpu_enabled {
            #[cfg(feature = "gpu")]
            {
                use faiss::gpu::StandardGpuResources;

                let gpu_resources = StandardGpuResources::new().map_err(|e| {
                    CodeGraphError::Vector(format!("Failed to initialize GPU resources: {}", e))
                })?;

                self.gpu_resources = Some(gpu_resources);
                info!("GPU acceleration enabled for FAISS index");
            }
            #[cfg(not(feature = "gpu"))]
            {
                warn!("GPU acceleration requested but not compiled with GPU support");
                return Err(CodeGraphError::Vector(
                    "GPU support not available".to_string(),
                ));
            }
        }
        Ok(())
    }

    /// Create or load the FAISS index
    pub fn create_index(&mut self, num_vectors: usize) -> Result<()> {
        // Load from persistence if available
        #[cfg(feature = "persistent")]
        if let Some(ref storage) = self.storage {
            if let Ok(index) = storage.load_index(&self.config) {
                // Also attempt to load ID mappings
                if let Ok((id_map, rev_map)) = storage.load_id_mapping() {
                    *self.id_mapping.write() = id_map;
                    *self.reverse_mapping.write() = rev_map;
                    // Update next_id to avoid collisions
                    if let Some(max_id) = self.id_mapping.read().keys().max() {
                        *self.next_id.write() = *max_id + 1;
                    }
                }
                *self.index.write() = Some(index);
                info!("Loaded existing FAISS index from disk");
                return Ok(());
            }
        }

        // Create new index
        let factory_string = self.config.to_factory_string();
        debug!(
            "Creating FAISS index with factory string: {}",
            factory_string
        );

        let mut index = if self.config.gpu_enabled {
            #[cfg(feature = "gpu")]
            {
                if let Some(ref gpu_resources) = self.gpu_resources {
                    let cpu_idx = faiss::index_factory(
                        self.config.dimension.try_into().unwrap(),
                        &factory_string,
                        self.config.metric_type,
                    )
                    .map_err(|e| CodeGraphError::Vector(e.to_string()))?;

                    // Move to GPU then back to CPU to ensure GPU-capable parameters are applied
                    let gpu_idx = cpu_idx
                        .to_gpu(gpu_resources, 0)
                        .map_err(|e| CodeGraphError::Vector(e.to_string()))?;
                    gpu_idx
                        .to_cpu()
                        .map_err(|e| CodeGraphError::Vector(e.to_string()))?
                } else {
                    return Err(CodeGraphError::Vector(
                        "GPU resources not initialized".to_string(),
                    ));
                }
            }
            #[cfg(not(feature = "gpu"))]
            {
                faiss::index_factory(
                    self.config.dimension.try_into().unwrap(),
                    &factory_string,
                    self.config.metric_type,
                )
                .map_err(|e| CodeGraphError::Vector(e.to_string()))?
            }
        } else {
            faiss::index_factory(
                self.config.dimension.try_into().unwrap(),
                &factory_string,
                self.config.metric_type,
            )
            .map_err(|e| CodeGraphError::Vector(e.to_string()))?
        };

        // Configure index-specific parameters
        self.config.configure_search_params(&mut index)?;

        *self.index.write() = Some(index);
        info!(
            "Created new FAISS index: {} for {} vectors",
            factory_string, num_vectors
        );
        Ok(())
    }

    /// Train the index if necessary (required for some index types)
    pub fn train_index(&mut self, training_vectors: &[f32]) -> Result<()> {
        let mut guard = self.index.write();
        let index = guard
            .as_mut()
            .ok_or_else(|| CodeGraphError::Vector("Index not created".to_string()))?;

        if !index.is_trained() {
            info!(
                "Training FAISS index with {} vectors",
                training_vectors.len() / self.config.dimension
            );
            index
                .train(training_vectors)
                .map_err(|e| CodeGraphError::Vector(format!("Index training failed: {}", e)))?;

            // Save trained index if persistence is enabled
            #[cfg(feature = "persistent")]
            if let Some(ref storage) = self.storage {
                storage.save_index(index, &self.config)?;
            }
        }
        Ok(())
    }

    /// Add vectors to the index with batch optimization
    pub fn add_vectors(&mut self, vectors: &[f32]) -> Result<Vec<i64>> {
        let mut guard = self.index.write();
        let index = guard
            .as_mut()
            .ok_or_else(|| CodeGraphError::Vector("Index not created".to_string()))?;

        let num_vectors = vectors.len() / self.config.dimension;
        let start_id = index.ntotal();

        debug!(
            "Adding {} vectors to FAISS index (starting from ID {})",
            num_vectors, start_id
        );

        index
            .add(vectors)
            .map_err(|e| CodeGraphError::Vector(format!("Failed to add vectors: {}", e)))?;

        let ids: Vec<i64> = (start_id..start_id + num_vectors as u64)
            .map(|x| x as i64)
            .collect();

        // Persist index updates if enabled
        #[cfg(feature = "persistent")]
        if let Some(ref storage) = self.storage {
            storage.save_index(index, &self.config)?;
        }

        info!("Successfully added {} vectors to index", num_vectors);
        Ok(ids)
    }

    /// Add vectors with explicit NodeIds. Supports large batches (10k+) efficiently.
    pub fn add_with_ids(&mut self, items: &[(NodeId, Vec<f32>)]) -> Result<Vec<i64>> {
        if items.is_empty() {
            return Ok(Vec::new());
        }

        // Flatten vectors and prepare IDs in chunks to avoid huge transient allocations
        let mut guard = self.index.write();
        let index = guard
            .as_mut()
            .ok_or_else(|| CodeGraphError::Vector("Index not created".to_string()))?;

        let dim = self.config.dimension;
        let mut assigned_ids: Vec<i64> = Vec::with_capacity(items.len());

        // Chunk size tuned for 10k+ per batch
        const CHUNK: usize = 10_000;
        for chunk in items.chunks(CHUNK) {
            // Prepare ids for this chunk
            let mut ids: Vec<i64> = Vec::with_capacity(chunk.len());
            {
                let mut rev = self.reverse_mapping.write();
                let mut fwd = self.id_mapping.write();
                let mut next = self.next_id.write();
                for (node_id, _) in chunk.iter() {
                    // If already present, reuse id; else assign new
                    let faiss_id = if let Some(id) = rev.get(node_id).copied() {
                        id
                    } else {
                        let id = *next;
                        *next += 1;
                        rev.insert(*node_id, id);
                        fwd.insert(id, *node_id);
                        id
                    };
                    ids.push(faiss_id);
                }
            }

            // Flat f32 matrix for this chunk
            let mut flat: Vec<f32> = Vec::with_capacity(chunk.len() * dim);
            for (_, v) in chunk.iter() {
                if v.len() != dim {
                    return Err(CodeGraphError::Vector(format!(
                        "Vector dimension {} does not match index dimension {}",
                        v.len(),
                        dim
                    )));
                }
                flat.extend_from_slice(v);
            }

            // Add with ids
            // Convert to FAISS Idx type
            let ids_idx: Vec<Idx> = ids.iter().map(|&i| Idx::from(i as i64)).collect();
            index
                .add_with_ids(&flat, &ids_idx)
                .map_err(|e| CodeGraphError::Vector(format!("Failed to add_with_ids: {}", e)))?;

            assigned_ids.extend_from_slice(&ids);
        }

        // Persist index and id mappings
        #[cfg(feature = "persistent")]
        if let Some(ref storage) = self.storage {
            storage.save_index(index, &self.config)?;
            let fwd = self.id_mapping.read();
            let rev = self.reverse_mapping.read();
            storage.save_id_mapping(&*fwd, &*rev)?;
        }

        info!(
            "Successfully added {} vectors with explicit IDs",
            items.len()
        );
        Ok(assigned_ids)
    }

    /// Perform optimized K-nearest neighbor search
    pub fn search(&self, query_vector: &[f32], k: usize) -> Result<(Vec<f32>, Vec<i64>)> {
        let mut guard = self.index.write();
        let index = guard
            .as_mut()
            .ok_or_else(|| CodeGraphError::Vector("Index not created".to_string()))?;

        if query_vector.len() != self.config.dimension {
            return Err(CodeGraphError::Vector(format!(
                "Query vector dimension {} doesn't match index dimension {}",
                query_vector.len(),
                self.config.dimension
            )));
        }

        let result = index
            .search(query_vector, k)
            .map_err(|e| CodeGraphError::Vector(format!("Search failed: {}", e)))?;

        let labels: Vec<i64> = result
            .labels
            .into_iter()
            .map(|idx| idx.get().map(|u| u as i64).unwrap_or(-1))
            .collect();
        Ok((result.distances, labels))
    }

    /// Search returning NodeIds by using the maintained mapping
    pub fn search_knn(&self, query_vector: &[f32], k: usize) -> Result<Vec<(NodeId, f32)>> {
        let (distances, labels) = self.search(query_vector, k)?;
        let map = self.id_mapping.read();
        let mut out = Vec::with_capacity(k);
        for (d, l) in distances.into_iter().zip(labels.into_iter()) {
            if let Some(node_id) = map.get(&l).copied() {
                out.push((node_id, d));
            }
        }
        Ok(out)
    }

    /// Remove vectors by NodeId using FAISS ID selector. Returns number removed.
    pub fn remove_vectors(&mut self, node_ids: &[NodeId]) -> Result<usize> {
        if node_ids.is_empty() {
            return Ok(0);
        }

        let ids: Vec<i64> = {
            let rev = self.reverse_mapping.read();
            node_ids
                .iter()
                .filter_map(|nid| rev.get(nid).copied())
                .collect()
        };

        if ids.is_empty() {
            return Ok(0);
        }

        let mut guard = self.index.write();
        let index = guard
            .as_mut()
            .ok_or_else(|| CodeGraphError::Vector("Index not created".to_string()))?;

        // Build selector and remove
        let ids_idx: Vec<Idx> = ids.iter().map(|&i| Idx::from(i as i64)).collect();
        let selector = IdSelector::batch(&ids_idx)
            .map_err(|e| CodeGraphError::Vector(format!("Failed to build ID selector: {}", e)))?;
        let removed = index
            .remove_ids(&selector)
            .map_err(|e| CodeGraphError::Vector(format!("Failed to remove ids: {}", e)))?;

        // Update mappings
        if removed > 0 {
            // Collect to avoid aliasing of borrows
            let to_remove: Vec<(NodeId, i64)> = {
                let rev_read = self.reverse_mapping.read();
                node_ids
                    .iter()
                    .filter_map(|nid| rev_read.get(nid).map(|id| (*nid, *id)))
                    .collect()
            };

            let mut fwd = self.id_mapping.write();
            let mut rev = self.reverse_mapping.write();
            for (nid, fid) in to_remove {
                fwd.remove(&fid);
                rev.remove(&nid);
            }
        }

        // Persist changes
        #[cfg(feature = "persistent")]
        if let Some(ref storage) = self.storage {
            storage.save_index(index, &self.config)?;
            let fwd = self.id_mapping.read();
            let rev = self.reverse_mapping.read();
            storage.save_id_mapping(&*fwd, &*rev)?;
        }

        info!("Removed {} vectors from index", removed);
        Ok(removed)
    }

    /// Get index statistics for monitoring and optimization
    pub fn get_stats(&self) -> Result<IndexStats> {
        let guard = self.index.read();
        let index = guard
            .as_ref()
            .ok_or_else(|| CodeGraphError::Vector("Index not created".to_string()))?;

        Ok(IndexStats {
            num_vectors: index.ntotal() as usize,
            dimension: self.config.dimension,
            index_type: self.config.index_type.clone(),
            is_trained: index.is_trained(),
            memory_usage: self.estimate_memory_usage()?,
        })
    }

    fn estimate_memory_usage(&self) -> Result<usize> {
        let guard = self.index.read();
        let index = guard
            .as_ref()
            .ok_or_else(|| CodeGraphError::Vector("Index not created".to_string()))?;

        let base_size = index.ntotal() as usize * self.config.dimension * 4; // 4 bytes per f32

        let overhead = match &self.config.index_type {
            IndexType::Flat => 0,
            IndexType::IVF { nlist, .. } => nlist * self.config.dimension * 4,
            IndexType::IVFPQ {
                nlist, m, nbits, ..
            } => {
                let codebook_size = (1 << nbits) * m * 4;
                let codes_size = index.ntotal() as usize * m; // approximate
                let ivf_overhead = nlist * self.config.dimension * 4;
                ivf_overhead + codebook_size + codes_size
            }
            IndexType::HNSW { m, .. } => index.ntotal() as usize * m * 8, // 8 bytes per link
            IndexType::LSH { nbits } => nbits / 8,
            IndexType::PQ { m, nbits } => {
                let codebook_size = (1 << nbits) * m * 4;
                let codes_size = index.ntotal() as usize * m;
                codebook_size + codes_size
            }
            IndexType::Hybrid { .. } => base_size / 2, // Rough estimate
        };

        Ok(base_size + overhead)
    }
}

#[derive(Debug, Clone)]
pub struct IndexStats {
    pub num_vectors: usize,
    pub dimension: usize,
    pub index_type: IndexType,
    pub is_trained: bool,
    pub memory_usage: usize,
}
