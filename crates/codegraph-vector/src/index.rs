use crate::storage::PersistentStorage;
use codegraph_core::{CodeGraphError, Result};
use faiss::{Index, IndexImpl, MetricType};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use tracing::{debug, info, warn};

/// Configuration for different FAISS index types optimized for various use cases
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexConfig {
    pub index_type: IndexType,
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
        nlist: usize,      // Number of clusters (sqrt(N) is typical)
        nprobe: usize,     // Number of clusters to search (1-nlist)
    },
    /// HNSW (Hierarchical Navigable Small World) - excellent for high-dimensional data
    HNSW {
        m: usize,          // Number of bi-directional links for each node (4-64)
        ef_construction: usize, // Size of dynamic candidate list (100-800)
        ef_search: usize,  // Search time accuracy/speed tradeoff (10-500)
    },
    /// LSH (Locality Sensitive Hashing) - very fast approximate search
    LSH {
        nbits: usize,      // Number of hash bits (typically 1024-4096)
    },
    /// Product Quantization - memory efficient for large datasets
    PQ {
        m: usize,          // Number of sub-quantizers (multiple of dimension)
        nbits: usize,      // Bits per sub-quantizer (1-16)
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
            index_type: IndexType::HNSW {
                m: 16,
                ef_construction: 200,
                ef_search: 50,
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
            index_type: IndexType::PQ {
                m: dimension / 8,
                nbits: 8,
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
            index_type: IndexType::IVF {
                nlist: 4096,
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
            IndexType::IVF { nlist, nprobe } => {
                format!("IVF{},Flat", nlist)
            },
            IndexType::HNSW { m, ef_construction, .. } => {
                format!("HNSW{}", m)
            },
            IndexType::LSH { nbits } => {
                format!("LSH{}", nbits)
            },
            IndexType::PQ { m, nbits } => {
                format!("PQ{}x{}", m, nbits)
            },
            IndexType::Hybrid { coarse_quantizer, fine_quantizer } => {
                let coarse_config = IndexConfig {
                    index_type: *coarse_quantizer.clone(),
                    ..*self
                };
                let fine_config = IndexConfig {
                    index_type: *fine_quantizer.clone(),
                    ..*self
                };
                format!("{},{}", coarse_config.to_factory_string(), fine_config.to_factory_string())
            },
        }
    }

    /// Configure search parameters after index creation
    pub fn configure_search_params(&self, index: &mut IndexImpl) -> Result<()> {
        match &self.index_type {
            IndexType::IVF { nprobe, .. } => {
                index.set_nprobe(*nprobe).map_err(|e| CodeGraphError::Vector(e.to_string()))?;
            },
            IndexType::HNSW { ef_search, .. } => {
                index.set_hnsw_ef(*ef_search).map_err(|e| CodeGraphError::Vector(e.to_string()))?;
            },
            _ => {}, // Other index types don't require runtime parameter tuning
        }
        Ok(())
    }
}

/// High-performance FAISS index manager with multiple index type support
pub struct FaissIndexManager {
    config: IndexConfig,
    index: Option<IndexImpl>,
    storage: Option<Arc<PersistentStorage>>,
    gpu_resources: Option<faiss::gpu::GpuResources>,
}

impl FaissIndexManager {
    pub fn new(config: IndexConfig) -> Self {
        Self {
            config,
            index: None,
            storage: None,
            gpu_resources: None,
        }
    }

    pub fn with_persistence(mut self, storage_path: PathBuf) -> Result<Self> {
        self.storage = Some(Arc::new(PersistentStorage::new(storage_path)?));
        Ok(self)
    }

    /// Initialize GPU resources if GPU acceleration is enabled
    pub fn init_gpu(&mut self) -> Result<()> {
        if self.config.gpu_enabled {
            #[cfg(feature = "gpu")]
            {
                use faiss::gpu::{GpuResources, StandardGpuResources};
                
                let gpu_resources = StandardGpuResources::new()
                    .map_err(|e| CodeGraphError::Vector(format!("Failed to initialize GPU resources: {}", e)))?;
                
                self.gpu_resources = Some(gpu_resources);
                info!("GPU acceleration enabled for FAISS index");
            }
            #[cfg(not(feature = "gpu"))]
            {
                warn!("GPU acceleration requested but not compiled with GPU support");
                return Err(CodeGraphError::Vector("GPU support not available".to_string()));
            }
        }
        Ok(())
    }

    /// Create or load the FAISS index
    pub fn create_index(&mut self, num_vectors: usize) -> Result<()> {
        // Load from persistence if available
        if let Some(ref storage) = self.storage {
            if let Ok(index) = storage.load_index(&self.config) {
                self.index = Some(index);
                info!("Loaded existing FAISS index from disk");
                return Ok(());
            }
        }

        // Create new index
        let factory_string = self.config.to_factory_string();
        debug!("Creating FAISS index with factory string: {}", factory_string);

        let mut index = if self.config.gpu_enabled {
            #[cfg(feature = "gpu")]
            {
                if let Some(ref gpu_resources) = self.gpu_resources {
                    faiss::gpu::index_gpu_to_cpu(
                        &faiss::gpu::index_cpu_to_gpu(
                            gpu_resources,
                            0, // device 0
                            &faiss::index_factory(
                                self.config.dimension,
                                &factory_string,
                                self.config.metric_type,
                            ).map_err(|e| CodeGraphError::Vector(e.to_string()))?,
                        ).map_err(|e| CodeGraphError::Vector(e.to_string()))?,
                    ).map_err(|e| CodeGraphError::Vector(e.to_string()))?
                } else {
                    return Err(CodeGraphError::Vector("GPU resources not initialized".to_string()));
                }
            }
            #[cfg(not(feature = "gpu"))]
            {
                faiss::index_factory(
                    self.config.dimension,
                    &factory_string,
                    self.config.metric_type,
                ).map_err(|e| CodeGraphError::Vector(e.to_string()))?
            }
        } else {
            faiss::index_factory(
                self.config.dimension,
                &factory_string,
                self.config.metric_type,
            ).map_err(|e| CodeGraphError::Vector(e.to_string()))?
        };

        // Configure index-specific parameters
        self.config.configure_search_params(&mut index)?;

        self.index = Some(index);
        info!("Created new FAISS index: {} for {} vectors", factory_string, num_vectors);
        Ok(())
    }

    /// Train the index if necessary (required for some index types)
    pub fn train_index(&mut self, training_vectors: &[f32]) -> Result<()> {
        let index = self.index.as_mut()
            .ok_or_else(|| CodeGraphError::Vector("Index not created".to_string()))?;

        if !index.is_trained() {
            info!("Training FAISS index with {} vectors", training_vectors.len() / self.config.dimension);
            index.train(training_vectors)
                .map_err(|e| CodeGraphError::Vector(format!("Index training failed: {}", e)))?;
            
            // Save trained index if persistence is enabled
            if let Some(ref storage) = self.storage {
                storage.save_index(index, &self.config)?;
            }
        }
        Ok(())
    }

    /// Add vectors to the index with batch optimization
    pub fn add_vectors(&mut self, vectors: &[f32]) -> Result<Vec<i64>> {
        let index = self.index.as_mut()
            .ok_or_else(|| CodeGraphError::Vector("Index not created".to_string()))?;

        let num_vectors = vectors.len() / self.config.dimension;
        let start_id = index.ntotal();
        
        debug!("Adding {} vectors to FAISS index (starting from ID {})", num_vectors, start_id);

        index.add(vectors)
            .map_err(|e| CodeGraphError::Vector(format!("Failed to add vectors: {}", e)))?;

        let ids: Vec<i64> = (start_id..start_id + num_vectors as i64).collect();

        // Persist index updates if enabled
        if let Some(ref storage) = self.storage {
            storage.save_index(index, &self.config)?;
        }

        info!("Successfully added {} vectors to index", num_vectors);
        Ok(ids)
    }

    /// Perform optimized K-nearest neighbor search
    pub fn search(&self, query_vector: &[f32], k: usize) -> Result<(Vec<f32>, Vec<i64>)> {
        let index = self.index.as_ref()
            .ok_or_else(|| CodeGraphError::Vector("Index not created".to_string()))?;

        if query_vector.len() != self.config.dimension {
            return Err(CodeGraphError::Vector(
                format!("Query vector dimension {} doesn't match index dimension {}", 
                       query_vector.len(), self.config.dimension)
            ));
        }

        let result = index.search(query_vector, k)
            .map_err(|e| CodeGraphError::Vector(format!("Search failed: {}", e)))?;

        Ok((result.distances, result.labels))
    }

    /// Get index statistics for monitoring and optimization
    pub fn get_stats(&self) -> Result<IndexStats> {
        let index = self.index.as_ref()
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
        let index = self.index.as_ref()
            .ok_or_else(|| CodeGraphError::Vector("Index not created".to_string()))?;

        let base_size = index.ntotal() as usize * self.config.dimension * 4; // 4 bytes per f32
        
        let overhead = match &self.config.index_type {
            IndexType::Flat => 0,
            IndexType::IVF { nlist, .. } => nlist * self.config.dimension * 4,
            IndexType::HNSW { m, .. } => (index.ntotal() as usize * m * 8), // 8 bytes per link
            IndexType::LSH { nbits } => nbits / 8,
            IndexType::PQ { m, nbits } => {
                let codebook_size = (1 << nbits) * m * 4;
                let codes_size = index.ntotal() as usize * m;
                codebook_size + codes_size
            },
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