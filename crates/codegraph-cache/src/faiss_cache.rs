use crate::{AiCache, CacheEntry, CacheKey, MetricsCollector};
use async_trait::async_trait;
use codegraph_core::{CodeGraphError, NodeId, Result};
use faiss::{Index, IndexFlat, IndexIVFFlat, MetricType};
use ndarray::{Array1, Array2};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};
use tokio::sync::RwLock as AsyncRwLock;
use tracing::{debug, error, info, warn};

/// Configuration for FAISS-based vector cache
#[derive(Debug, Clone)]
pub struct FaissConfig {
    /// Vector dimension
    pub dimension: u32,
    /// Number of clusters for IVF index (0 for flat index)
    pub num_clusters: u32,
    /// Distance metric for similarity search
    pub metric_type: MetricType,
    /// Maximum number of vectors to cache
    pub max_vectors: usize,
    /// Similarity threshold for cache hits
    pub similarity_threshold: f32,
    /// Number of probes for IVF search
    pub num_probes: usize,
    /// Enable GPU acceleration if available
    pub use_gpu: bool,
    /// Training vector count threshold (min vectors needed for IVF training)
    pub training_threshold: usize,
}

impl Default for FaissConfig {
    fn default() -> Self {
        Self {
            dimension: 768, // Common embedding dimension
            num_clusters: 100,
            metric_type: MetricType::L2,
            max_vectors: 100_000,
            similarity_threshold: 0.85,
            num_probes: 10,
            use_gpu: false,
            training_threshold: 1000,
        }
    }
}

/// FAISS vector cache entry with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FaissVectorEntry {
    /// The vector embedding
    pub vector: Vec<f32>,
    /// Associated metadata
    pub metadata: HashMap<String, String>,
    /// Creation timestamp
    pub created_at: SystemTime,
    /// Last access timestamp
    pub last_accessed: SystemTime,
    /// Access count for LRU
    pub access_count: u64,
    /// Entry size in bytes
    pub size_bytes: usize,
}

impl FaissVectorEntry {
    pub fn new(vector: Vec<f32>, metadata: HashMap<String, String>) -> Self {
        let size_bytes = vector.len() * std::mem::size_of::<f32>() + 
                        metadata.iter().map(|(k, v)| k.len() + v.len()).sum::<usize>() +
                        std::mem::size_of::<Self>();
        
        Self {
            vector,
            metadata,
            created_at: SystemTime::now(),
            last_accessed: SystemTime::now(),
            access_count: 0,
            size_bytes,
        }
    }

    pub fn mark_accessed(&mut self) {
        self.last_accessed = SystemTime::now();
        self.access_count += 1;
    }
}

/// FAISS-backed vector cache for similarity search
pub struct FaissCache {
    /// FAISS index for vector similarity search
    index: Arc<RwLock<Option<Box<dyn Index>>>>,
    /// Vector entries mapped by internal ID
    entries: Arc<AsyncRwLock<HashMap<i64, FaissVectorEntry>>>,
    /// Key to FAISS ID mapping
    key_to_id: Arc<AsyncRwLock<HashMap<String, i64>>>,
    /// FAISS ID to key mapping
    id_to_key: Arc<AsyncRwLock<HashMap<i64, String>>>,
    /// Next available FAISS ID
    next_id: Arc<AsyncRwLock<i64>>,
    /// Cache configuration
    config: FaissConfig,
    /// Metrics collector
    metrics: Arc<MetricsCollector>,
    /// Training vectors for IVF index
    training_vectors: Arc<AsyncRwLock<Vec<Vec<f32>>>>,
    /// Whether index is trained (for IVF)
    is_trained: Arc<AsyncRwLock<bool>>,
}

impl FaissCache {
    /// Create a new FAISS cache with default configuration
    pub fn new() -> Result<Self> {
        Self::with_config(FaissConfig::default())
    }

    /// Create a new FAISS cache with custom configuration
    pub fn with_config(config: FaissConfig) -> Result<Self> {
        let metrics = Arc::new(MetricsCollector::new());
        
        Ok(Self {
            index: Arc::new(RwLock::new(None)),
            entries: Arc::new(AsyncRwLock::new(HashMap::new())),
            key_to_id: Arc::new(AsyncRwLock::new(HashMap::new())),
            id_to_key: Arc::new(AsyncRwLock::new(HashMap::new())),
            next_id: Arc::new(AsyncRwLock::new(0)),
            config,
            metrics,
            training_vectors: Arc::new(AsyncRwLock::new(Vec::new())),
            is_trained: Arc::new(AsyncRwLock::new(false)),
        })
    }

    /// Initialize the FAISS index
    pub async fn initialize(&self) -> Result<()> {
        let mut index_guard = self.index.write();
        
        let index: Box<dyn Index> = if self.config.num_clusters == 0 {
            // Use flat index for exact search
            Box::new(IndexFlat::new(self.config.dimension, self.config.metric_type)?)
        } else {
            // Use IVF index for approximate search
            let quantizer = IndexFlat::new(self.config.dimension, self.config.metric_type)?;
            let mut ivf_index = IndexIVFFlat::new(
                Box::new(quantizer),
                self.config.dimension,
                self.config.num_clusters,
                self.config.metric_type,
            )?;
            ivf_index.set_nprobe(self.config.num_probes);
            Box::new(ivf_index)
        };

        *index_guard = Some(index);
        info!("FAISS cache initialized with dimension {}", self.config.dimension);
        Ok(())
    }

    /// Add vector to cache with similarity search
    pub async fn add_vector(&self, key: String, vector: Vec<f32>, metadata: HashMap<String, String>) -> Result<()> {
        let start_time = Instant::now();
        
        if vector.len() != self.config.dimension as usize {
            return Err(CodeGraphError::InvalidInput(format!(
                "Vector dimension {} does not match expected dimension {}",
                vector.len(),
                self.config.dimension
            )));
        }

        // Check if we already have this key
        if self.key_to_id.read().await.contains_key(&key) {
            self.metrics.record_hit();
            return Ok(());
        }

        // Check cache size limit
        if self.entries.read().await.len() >= self.config.max_vectors {
            self.evict_lru().await?;
        }

        let mut next_id_guard = self.next_id.write().await;
        let id = *next_id_guard;
        *next_id_guard += 1;
        drop(next_id_guard);

        // Create cache entry
        let entry = FaissVectorEntry::new(vector.clone(), metadata);
        let entry_size = entry.size_bytes;

        // Store entry
        self.entries.write().await.insert(id, entry);
        self.key_to_id.write().await.insert(key.clone(), id);
        self.id_to_key.write().await.insert(id, key);

        // Handle index training for IVF
        if self.config.num_clusters > 0 && !*self.is_trained.read().await {
            let mut training_vectors = self.training_vectors.write().await;
            training_vectors.push(vector.clone());
            
            if training_vectors.len() >= self.config.training_threshold {
                self.train_index(&training_vectors).await?;
                training_vectors.clear();
            }
        }

        // Add to FAISS index if trained or using flat index
        if *self.is_trained.read().await || self.config.num_clusters == 0 {
            self.add_to_index(id, &vector).await?;
        }

        self.metrics.record_insertion(entry_size);
        self.metrics.record_operation("add_vector", start_time.elapsed(), true).await;
        
        debug!("Added vector with key '{}' and ID {}", key, id);
        Ok(())
    }

    /// Search for similar vectors
    pub async fn search_similar(&self, query_vector: Vec<f32>, k: usize) -> Result<Vec<(String, f32)>> {
        let start_time = Instant::now();
        
        if query_vector.len() != self.config.dimension as usize {
            return Err(CodeGraphError::InvalidInput(format!(
                "Query vector dimension {} does not match expected dimension {}",
                query_vector.len(),
                self.config.dimension
            )));
        }

        let index_guard = self.index.read();
        let index = match index_guard.as_ref() {
            Some(idx) => idx,
            None => {
                self.metrics.record_miss();
                return Ok(Vec::new());
            }
        };

        // Only search if index is trained (for IVF) or using flat index
        if !*self.is_trained.read().await && self.config.num_clusters > 0 {
            self.metrics.record_miss();
            return Ok(Vec::new());
        }

        // Perform similarity search
        let query_array = Array2::from_shape_vec((1, query_vector.len()), query_vector)?;
        let (distances, indices) = index.search(&query_array, k)?;

        let mut results = Vec::new();
        let id_to_key = self.id_to_key.read().await;
        let mut entries = self.entries.write().await;

        for i in 0..k.min(indices.len()) {
            let faiss_id = indices[i];
            let distance = distances[i];
            
            // Convert distance to similarity score
            let similarity = match self.config.metric_type {
                MetricType::L2 => 1.0 / (1.0 + distance),
                MetricType::InnerProduct => distance,
                _ => distance,
            };

            if similarity >= self.config.similarity_threshold {
                if let Some(key) = id_to_key.get(&faiss_id) {
                    if let Some(entry) = entries.get_mut(&faiss_id) {
                        entry.mark_accessed();
                        results.push((key.clone(), similarity));
                        self.metrics.record_hit();
                    }
                }
            } else {
                self.metrics.record_miss();
            }
        }

        self.metrics.record_response_time(start_time.elapsed()).await;
        self.metrics.record_operation("search_similar", start_time.elapsed(), true).await;

        debug!("Found {} similar vectors for query", results.len());
        Ok(results)
    }

    /// Get vector by key
    pub async fn get_vector(&self, key: &str) -> Result<Option<FaissVectorEntry>> {
        let start_time = Instant::now();
        
        let key_to_id = self.key_to_id.read().await;
        if let Some(&id) = key_to_id.get(key) {
            let mut entries = self.entries.write().await;
            if let Some(entry) = entries.get_mut(&id) {
                entry.mark_accessed();
                self.metrics.record_hit();
                self.metrics.record_operation("get_vector", start_time.elapsed(), true).await;
                return Ok(Some(entry.clone()));
            }
        }

        self.metrics.record_miss();
        self.metrics.record_operation("get_vector", start_time.elapsed(), false).await;
        Ok(None)
    }

    /// Remove vector by key
    pub async fn remove_vector(&self, key: &str) -> Result<bool> {
        let start_time = Instant::now();
        
        let mut key_to_id = self.key_to_id.write().await;
        if let Some(id) = key_to_id.remove(key) {
            let mut id_to_key = self.id_to_key.write().await;
            let mut entries = self.entries.write().await;
            
            id_to_key.remove(&id);
            if let Some(entry) = entries.remove(&id) {
                self.metrics.record_removal(entry.size_bytes);
                self.metrics.record_operation("remove_vector", start_time.elapsed(), true).await;
                
                // Note: FAISS doesn't support individual vector removal efficiently
                // In a production system, you might want to rebuild the index periodically
                warn!("Vector removed from cache but not from FAISS index: {}", key);
                return Ok(true);
            }
        }

        self.metrics.record_operation("remove_vector", start_time.elapsed(), false).await;
        Ok(false)
    }

    /// Train the IVF index with collected vectors
    async fn train_index(&self, training_vectors: &[Vec<f32>]) -> Result<()> {
        if self.config.num_clusters == 0 {
            return Ok(()); // Flat index doesn't need training
        }

        let mut index_guard = self.index.write();
        if let Some(index) = index_guard.as_mut() {
            // Convert training vectors to ndarray
            let n_vectors = training_vectors.len();
            let dimension = self.config.dimension as usize;
            
            let mut flat_vectors = Vec::with_capacity(n_vectors * dimension);
            for vector in training_vectors {
                flat_vectors.extend_from_slice(vector);
            }
            
            let training_array = Array2::from_shape_vec((n_vectors, dimension), flat_vectors)?;
            
            // Train the index
            index.train(&training_array)?;
            info!("FAISS IVF index trained with {} vectors", n_vectors);
            
            *self.is_trained.write().await = true;
        }

        Ok(())
    }

    /// Add vector to FAISS index
    async fn add_to_index(&self, id: i64, vector: &[f32]) -> Result<()> {
        let mut index_guard = self.index.write();
        if let Some(index) = index_guard.as_mut() {
            let vector_array = Array2::from_shape_vec((1, vector.len()), vector.to_vec())?;
            index.add_with_ids(&vector_array, &[id])?;
        }
        Ok(())
    }

    /// Evict least recently used entry
    async fn evict_lru(&self) -> Result<()> {
        let entries = self.entries.read().await;
        
        // Find LRU entry
        let lru_entry = entries
            .iter()
            .min_by_key(|(_, entry)| (entry.last_accessed, entry.access_count))
            .map(|(&id, _)| id);

        drop(entries);

        if let Some(lru_id) = lru_entry {
            let id_to_key = self.id_to_key.read().await;
            if let Some(key) = id_to_key.get(&lru_id).cloned() {
                drop(id_to_key);
                self.remove_vector(&key).await?;
                debug!("Evicted LRU entry with key '{}'", key);
            }
        }

        Ok(())
    }

    /// Get cache statistics
    pub async fn get_stats(&self) -> FaissCacheStats {
        let entries = self.entries.read().await;
        let total_vectors = entries.len();
        let total_memory = entries.values().map(|e| e.size_bytes).sum();
        
        let index_guard = self.index.read();
        let index_size = if let Some(index) = index_guard.as_ref() {
            index.ntotal() as usize
        } else {
            0
        };

        FaissCacheStats {
            total_vectors,
            index_size,
            total_memory_bytes: total_memory,
            is_trained: *self.is_trained.read().await,
            dimension: self.config.dimension,
            similarity_threshold: self.config.similarity_threshold,
        }
    }

    /// Clear all vectors from cache
    pub async fn clear(&self) -> Result<()> {
        self.entries.write().await.clear();
        self.key_to_id.write().await.clear();
        self.id_to_key.write().await.clear();
        *self.next_id.write().await = 0;
        *self.is_trained.write().await = false;
        
        // Reinitialize index
        self.initialize().await?;
        
        info!("FAISS cache cleared");
        Ok(())
    }
}

/// FAISS cache statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FaissCacheStats {
    pub total_vectors: usize,
    pub index_size: usize,
    pub total_memory_bytes: usize,
    pub is_trained: bool,
    pub dimension: u32,
    pub similarity_threshold: f32,
}

#[async_trait]
impl AiCache<String, FaissVectorEntry> for FaissCache {
    async fn insert(&mut self, key: String, value: FaissVectorEntry, _ttl: Option<Duration>) -> Result<()> {
        // Convert FaissVectorEntry back to components for add_vector
        self.add_vector(key, value.vector, value.metadata).await
    }

    async fn get(&mut self, key: &String) -> Result<Option<FaissVectorEntry>> {
        self.get_vector(key).await
    }

    async fn remove(&mut self, key: &String) -> Result<()> {
        self.remove_vector(key).await?;
        Ok(())
    }

    async fn clear(&mut self) -> Result<()> {
        FaissCache::clear(self).await
    }

    async fn size(&self) -> Result<usize> {
        Ok(self.entries.read().await.len())
    }

    async fn stats(&self) -> Result<crate::CacheStats> {
        let faiss_stats = self.get_stats().await;
        let cache_metrics = self.metrics.get_metrics().await;
        
        Ok(crate::CacheStats {
            entries: faiss_stats.total_vectors,
            memory_usage: faiss_stats.total_memory_bytes as u64,
            hit_rate: cache_metrics.hit_rate,
            evictions: cache_metrics.evictions,
        })
    }
}

/// Builder for FAISS cache configuration
pub struct FaissCacheBuilder {
    config: FaissConfig,
}

impl FaissCacheBuilder {
    pub fn new() -> Self {
        Self {
            config: FaissConfig::default(),
        }
    }

    pub fn dimension(mut self, dimension: u32) -> Self {
        self.config.dimension = dimension;
        self
    }

    pub fn num_clusters(mut self, num_clusters: u32) -> Self {
        self.config.num_clusters = num_clusters;
        self
    }

    pub fn metric_type(mut self, metric_type: MetricType) -> Self {
        self.config.metric_type = metric_type;
        self
    }

    pub fn max_vectors(mut self, max_vectors: usize) -> Self {
        self.config.max_vectors = max_vectors;
        self
    }

    pub fn similarity_threshold(mut self, threshold: f32) -> Self {
        self.config.similarity_threshold = threshold;
        self
    }

    pub fn num_probes(mut self, num_probes: usize) -> Self {
        self.config.num_probes = num_probes;
        self
    }

    pub fn use_gpu(mut self, use_gpu: bool) -> Self {
        self.config.use_gpu = use_gpu;
        self
    }

    pub fn training_threshold(mut self, threshold: usize) -> Self {
        self.config.training_threshold = threshold;
        self
    }

    pub fn build(self) -> Result<FaissCache> {
        FaissCache::with_config(self.config)
    }
}

impl Default for FaissCacheBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_relative_eq;

    #[tokio::test]
    async fn test_faiss_cache_creation() {
        let cache = FaissCache::new().unwrap();
        cache.initialize().await.unwrap();
        
        let stats = cache.get_stats().await;
        assert_eq!(stats.total_vectors, 0);
        assert_eq!(stats.dimension, 768);
    }

    #[tokio::test]
    async fn test_vector_addition_and_retrieval() {
        let mut cache = FaissCache::new().unwrap();
        cache.initialize().await.unwrap();
        
        let vector = vec![1.0, 2.0, 3.0, 4.0];
        let config = FaissConfig {
            dimension: 4,
            num_clusters: 0, // Use flat index for testing
            ..Default::default()
        };
        
        let mut cache = FaissCache::with_config(config).unwrap();
        cache.initialize().await.unwrap();
        
        let metadata = HashMap::new();
        cache.add_vector("test_key".to_string(), vector.clone(), metadata).await.unwrap();
        
        let retrieved = cache.get_vector("test_key").await.unwrap();
        assert!(retrieved.is_some());
        
        let entry = retrieved.unwrap();
        assert_eq!(entry.vector, vector);
    }

    #[tokio::test]
    async fn test_similarity_search() {
        let config = FaissConfig {
            dimension: 4,
            num_clusters: 0, // Use flat index
            similarity_threshold: 0.5,
            ..Default::default()
        };
        
        let mut cache = FaissCache::with_config(config).unwrap();
        cache.initialize().await.unwrap();
        
        // Add some test vectors
        let metadata = HashMap::new();
        cache.add_vector("vec1".to_string(), vec![1.0, 0.0, 0.0, 0.0], metadata.clone()).await.unwrap();
        cache.add_vector("vec2".to_string(), vec![0.9, 0.1, 0.0, 0.0], metadata.clone()).await.unwrap();
        cache.add_vector("vec3".to_string(), vec![0.0, 1.0, 0.0, 0.0], metadata).await.unwrap();
        
        // Search for similar vectors
        let query = vec![1.0, 0.0, 0.0, 0.0];
        let results = cache.search_similar(query, 3).await.unwrap();
        
        assert!(!results.is_empty());
        // First result should be exact match
        assert_eq!(results[0].0, "vec1");
    }

    #[tokio::test]
    async fn test_cache_eviction() {
        let config = FaissConfig {
            dimension: 2,
            num_clusters: 0,
            max_vectors: 2,
            ..Default::default()
        };
        
        let mut cache = FaissCache::with_config(config).unwrap();
        cache.initialize().await.unwrap();
        
        let metadata = HashMap::new();
        
        // Add vectors up to limit
        cache.add_vector("vec1".to_string(), vec![1.0, 0.0], metadata.clone()).await.unwrap();
        cache.add_vector("vec2".to_string(), vec![0.0, 1.0], metadata.clone()).await.unwrap();
        
        // This should trigger eviction
        cache.add_vector("vec3".to_string(), vec![1.0, 1.0], metadata).await.unwrap();
        
        let stats = cache.get_stats().await;
        assert_eq!(stats.total_vectors, 2); // Should still be at limit
    }

    #[tokio::test]
    async fn test_cache_clear() {
        let mut cache = FaissCache::new().unwrap();
        cache.initialize().await.unwrap();
        
        let config = FaissConfig {
            dimension: 2,
            num_clusters: 0,
            ..Default::default()
        };
        
        let mut cache = FaissCache::with_config(config).unwrap();
        cache.initialize().await.unwrap();
        
        let metadata = HashMap::new();
        cache.add_vector("vec1".to_string(), vec![1.0, 0.0], metadata).await.unwrap();
        
        let stats_before = cache.get_stats().await;
        assert_eq!(stats_before.total_vectors, 1);
        
        cache.clear().await.unwrap();
        
        let stats_after = cache.get_stats().await;
        assert_eq!(stats_after.total_vectors, 0);
    }

    #[tokio::test]
    async fn test_builder_pattern() {
        let cache = FaissCacheBuilder::new()
            .dimension(128)
            .num_clusters(10)
            .max_vectors(1000)
            .similarity_threshold(0.9)
            .build()
            .unwrap();
        
        cache.initialize().await.unwrap();
        
        let stats = cache.get_stats().await;
        assert_eq!(stats.dimension, 128);
        assert_eq!(stats.similarity_threshold, 0.9);
    }
}