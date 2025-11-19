use crate::{AiCache, CacheConfig, CacheEntry, CacheSizeEstimator, CacheStats};
use async_trait::async_trait;
use codegraph_core::{NodeId, Result};
use dashmap::DashMap;
use sha2::{Digest, Sha256};
use std::collections::VecDeque;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock as AsyncRwLock;

/// Query result with metadata
#[derive(Debug, Clone)]
pub struct QueryResult {
    pub node_ids: Vec<NodeId>,
    pub scores: Vec<f32>,
    pub total_results: usize,
    pub query_time_ms: u64,
}

impl CacheSizeEstimator for QueryResult {
    fn estimate_size(&self) -> usize {
        std::mem::size_of::<Self>()
            + self.node_ids.len() * std::mem::size_of::<NodeId>()
            + self.scores.len() * std::mem::size_of::<f32>()
    }
}

/// Cached query entry with similarity threshold
#[derive(Debug, Clone)]
pub struct CachedQuery {
    pub query_embedding: Vec<f32>,
    pub result: QueryResult,
    pub similarity_threshold: f32,
}

impl CacheSizeEstimator for CachedQuery {
    fn estimate_size(&self) -> usize {
        std::mem::size_of::<Self>()
            + self.query_embedding.estimate_size()
            + self.result.estimate_size()
    }
}

/// Semantic query cache using cosine similarity for cache hit detection
#[derive(Clone)]
pub struct QueryCache {
    /// Thread-safe cache storage
    cache: Arc<DashMap<String, CacheEntry<CachedQuery>>>,
    /// LRU tracking for eviction
    lru_queue: Arc<AsyncRwLock<VecDeque<String>>>,
    /// Configuration
    config: QueryCacheConfig,
    /// Performance metrics
    stats: Arc<AsyncRwLock<CacheStats>>,
    /// Memory usage tracking
    memory_usage: Arc<parking_lot::Mutex<usize>>,
}

/// Query cache specific configuration
#[derive(Debug, Clone)]
pub struct QueryCacheConfig {
    pub base_config: CacheConfig,
    pub similarity_threshold: f32,
    pub max_query_dimension: usize,
    pub enable_fuzzy_matching: bool,
    pub fuzzy_tolerance: f32,
}

impl Default for QueryCacheConfig {
    fn default() -> Self {
        Self {
            base_config: CacheConfig::default(),
            similarity_threshold: 0.85, // High similarity required for cache hit
            max_query_dimension: 1024,
            enable_fuzzy_matching: true,
            fuzzy_tolerance: 0.1,
        }
    }
}

impl QueryCache {
    pub fn new(config: QueryCacheConfig) -> Self {
        Self {
            cache: Arc::new(DashMap::new()),
            lru_queue: Arc::new(AsyncRwLock::new(VecDeque::new())),
            config,
            stats: Arc::new(AsyncRwLock::new(CacheStats::default())),
            memory_usage: Arc::new(parking_lot::Mutex::new(0)),
        }
    }

    /// Create a cache key for the query
    pub fn create_query_key(query_embedding: &[f32]) -> String {
        let mut hasher = Sha256::new();
        for &value in query_embedding {
            hasher.update(value.to_le_bytes());
        }
        format!("query_{:x}", hasher.finalize())
    }

    /// Calculate cosine similarity between two embeddings
    pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
        if a.len() != b.len() {
            return 0.0;
        }

        let dot_product: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
        let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
        let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

        if norm_a == 0.0 || norm_b == 0.0 {
            return 0.0;
        }

        dot_product / (norm_a * norm_b)
    }

    /// Find semantically similar cached queries
    pub async fn find_similar_query(&self, query_embedding: &[f32]) -> Option<(String, f32)> {
        let mut best_match: Option<(String, f32)> = None;

        for entry in self.cache.iter() {
            let cached_query = &entry.value().value;
            let similarity =
                Self::cosine_similarity(query_embedding, &cached_query.query_embedding);

            if similarity >= self.config.similarity_threshold {
                if let Some((_, current_best)) = &best_match {
                    if similarity > *current_best {
                        best_match = Some((entry.key().clone(), similarity));
                    }
                } else {
                    best_match = Some((entry.key().clone(), similarity));
                }
            }
        }

        best_match
    }

    /// Store query result with semantic indexing
    pub async fn store_query_result(
        &mut self,
        query_embedding: Vec<f32>,
        result: QueryResult,
        ttl: Option<Duration>,
    ) -> Result<()> {
        let key = Self::create_query_key(&query_embedding);

        let cached_query = CachedQuery {
            query_embedding,
            result,
            similarity_threshold: self.config.similarity_threshold,
        };

        self.insert(key, cached_query, ttl).await
    }

    /// Retrieve query result by semantic similarity
    pub async fn get_similar_query_result(
        &mut self,
        query_embedding: &[f32],
    ) -> Result<Option<(QueryResult, f32)>> {
        if let Some((key, similarity)) = self.find_similar_query(query_embedding).await {
            if let Some(cached_query) = self.get(&key).await? {
                return Ok(Some((cached_query.result, similarity)));
            }
        }
        Ok(None)
    }

    /// Update LRU position for accessed key
    async fn update_lru(&self, key: &str) {
        let mut lru_queue = self.lru_queue.write().await;

        // Remove key if it exists
        lru_queue.retain(|k| k != key);
        // Add to back as most recently used
        lru_queue.push_back(key.to_string());

        // Limit queue size
        while lru_queue.len() > self.config.base_config.max_size {
            lru_queue.pop_front();
        }
    }

    /// Check memory pressure
    async fn check_memory_pressure(&self) -> bool {
        let current_memory = *self.memory_usage.lock();
        current_memory > self.config.base_config.max_memory_bytes
    }

    /// Evict least recently used entries
    async fn evict_lru(&mut self) -> Result<()> {
        let mut evicted = 0;
        let mut lru_queue = self.lru_queue.write().await;

        while self.check_memory_pressure().await && !lru_queue.is_empty() {
            if let Some(key) = lru_queue.pop_front() {
                if let Some((_, entry)) = self.cache.remove(&key) {
                    let mut memory_usage = self.memory_usage.lock();
                    *memory_usage = memory_usage.saturating_sub(entry.size_bytes);
                    evicted += 1;
                }
            }
        }

        if evicted > 0 {
            let mut stats = self.stats.write().await;
            stats.evictions += evicted;
        }

        Ok(())
    }

    /// Get cache hit rate specifically for semantic matches
    pub async fn semantic_hit_rate(&self) -> f32 {
        let stats = self.stats.read().await;
        if stats.hits + stats.misses == 0 {
            0.0
        } else {
            stats.hits as f32 / (stats.hits + stats.misses) as f32
        }
    }

    /// Get average similarity of cache hits
    pub async fn average_hit_similarity(&self) -> f32 {
        // This would require tracking similarity scores in stats
        // For now, return the configured threshold as a proxy
        self.config.similarity_threshold
    }

    /// Cleanup expired entries
    pub async fn cleanup_expired(&mut self) -> Result<usize> {
        let mut removed = 0;
        let mut keys_to_remove = Vec::new();

        // Collect expired keys
        for entry in self.cache.iter() {
            if entry.value().is_expired() {
                keys_to_remove.push(entry.key().clone());
            }
        }

        // Remove expired entries
        for key in keys_to_remove {
            if let Some((_, entry)) = self.cache.remove(&key) {
                let mut memory_usage = self.memory_usage.lock();
                *memory_usage = memory_usage.saturating_sub(entry.size_bytes);
                removed += 1;
            }
        }

        // Update LRU queue
        if removed > 0 {
            let mut lru_queue = self.lru_queue.write().await;
            lru_queue.retain(|key| self.cache.contains_key(key));
        }

        Ok(removed)
    }
}

#[async_trait]
impl AiCache<String, CachedQuery> for QueryCache {
    async fn insert(
        &mut self,
        key: String,
        value: CachedQuery,
        ttl: Option<Duration>,
    ) -> Result<()> {
        let size_bytes = value.estimate_size() + key.len();
        let entry = CacheEntry::new(value, size_bytes, ttl);

        // Update memory usage
        {
            let mut memory_usage = self.memory_usage.lock();
            *memory_usage += size_bytes;
        }

        // Check memory pressure and evict if needed
        if self.check_memory_pressure().await {
            self.evict_lru().await?;
        }

        // Insert entry
        self.cache.insert(key.clone(), entry);

        // Update LRU
        self.update_lru(&key).await;

        // Update metrics
        if self.config.base_config.enable_metrics {
            let mut stats = self.stats.write().await;
            stats.entries = self.cache.len();
            stats.memory_usage = *self.memory_usage.lock() as u64;
        }

        Ok(())
    }

    async fn get(&mut self, key: &String) -> Result<Option<CachedQuery>> {
        if let Some(mut entry) = self.cache.get_mut(key) {
            // Check if expired
            if entry.is_expired() {
                drop(entry);
                self.cache.remove(key);

                if self.config.base_config.enable_metrics {
                    let mut stats = self.stats.write().await;
                    stats.misses += 1;
                }
                return Ok(None);
            }

            // Update access info
            entry.touch();
            let result = entry.value.clone();
            drop(entry);

            // Update LRU
            self.update_lru(key).await;

            // Update metrics
            if self.config.base_config.enable_metrics {
                let mut stats = self.stats.write().await;
                stats.hits += 1;
            }

            Ok(Some(result))
        } else {
            if self.config.base_config.enable_metrics {
                let mut stats = self.stats.write().await;
                stats.misses += 1;
            }
            Ok(None)
        }
    }

    async fn remove(&mut self, key: &String) -> Result<()> {
        if let Some((_, entry)) = self.cache.remove(key) {
            // Update memory usage
            {
                let mut memory_usage = self.memory_usage.lock();
                *memory_usage = memory_usage.saturating_sub(entry.size_bytes);
            }

            // Update LRU queue
            let mut lru_queue = self.lru_queue.write().await;
            lru_queue.retain(|k| k != key);

            // Update metrics
            if self.config.base_config.enable_metrics {
                let mut stats = self.stats.write().await;
                stats.entries = self.cache.len();
                stats.memory_usage = *self.memory_usage.lock() as u64;
            }

            Ok(())
        } else {
            Ok(())
        }
    }

    async fn clear(&mut self) -> Result<()> {
        self.cache.clear();
        self.lru_queue.write().await.clear();
        *self.memory_usage.lock() = 0;

        if self.config.base_config.enable_metrics {
            let mut stats = self.stats.write().await;
            stats.entries = 0;
            stats.memory_usage = 0;
        }

        Ok(())
    }

    async fn stats(&self) -> Result<CacheStats> {
        if self.config.base_config.enable_metrics {
            let mut stats = self.stats.read().await.clone();
            stats.hit_rate = stats.hit_rate();
            stats.entries = self.cache.len();
            stats.memory_usage = *self.memory_usage.lock() as u64;
            Ok(stats)
        } else {
            Ok(CacheStats::default())
        }
    }

    async fn contains_key(&self, key: &String) -> bool {
        self.cache.contains_key(key)
    }

    async fn size(&self) -> Result<usize> {
        Ok(self.cache.len())
    }
}

impl Default for QueryCache {
    fn default() -> Self {
        Self::new(QueryCacheConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_relative_eq;

    #[tokio::test]
    async fn test_cosine_similarity() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        let c = vec![0.0, 1.0, 0.0];

        assert_relative_eq!(QueryCache::cosine_similarity(&a, &b), 1.0, epsilon = 1e-6);
        assert_relative_eq!(QueryCache::cosine_similarity(&a, &c), 0.0, epsilon = 1e-6);
    }

    #[tokio::test]
    async fn test_semantic_query_matching() {
        let mut cache = QueryCache::default();

        let query1 = vec![0.8, 0.6, 0.0];
        let query2 = vec![0.7, 0.7, 0.1]; // Similar to query1
        let query3 = vec![0.0, 0.0, 1.0]; // Very different

        let result = QueryResult {
            node_ids: vec![NodeId::new_v4()],
            scores: vec![0.95],
            total_results: 1,
            query_time_ms: 50,
        };

        // Store first query result
        cache
            .store_query_result(query1.clone(), result.clone(), None)
            .await
            .unwrap();

        // Should find similar query
        let similar_result = cache.get_similar_query_result(&query2).await.unwrap();
        assert!(similar_result.is_some());

        // Should not find dissimilar query
        let dissimilar_result = cache.get_similar_query_result(&query3).await.unwrap();
        assert!(dissimilar_result.is_none());
    }

    #[tokio::test]
    async fn test_query_cache_basic_operations() {
        let mut cache = QueryCache::default();
        let key = "test_query".to_string();

        let cached_query = CachedQuery {
            query_embedding: vec![0.5, 0.5, 0.5],
            result: QueryResult {
                node_ids: vec![NodeId::new_v4()],
                scores: vec![0.9],
                total_results: 1,
                query_time_ms: 25,
            },
            similarity_threshold: 0.8,
        };

        // Test insert and get
        cache
            .insert(key.clone(), cached_query.clone(), None)
            .await
            .unwrap();
        let result = cache.get(&key).await.unwrap();
        assert!(result.is_some());

        // Test remove
        cache.remove(&key).await.unwrap();
        assert!(cache.get(&key).await.unwrap().is_none());
        let result = cache.get(&key).await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_memory_eviction() {
        let config = QueryCacheConfig {
            base_config: CacheConfig {
                max_memory_bytes: 2048, // Small limit
                max_size: 100,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut cache = QueryCache::new(config);

        // Fill cache beyond memory limit
        for i in 0..20 {
            let query_embedding = vec![i as f32; 256]; // Large embedding
            let result = QueryResult {
                node_ids: vec![NodeId::new_v4(); 10], // Multiple results
                scores: vec![0.9; 10],
                total_results: 10,
                query_time_ms: 100,
            };
            cache
                .store_query_result(query_embedding, result, None)
                .await
                .unwrap();
        }

        // Check that eviction occurred
        let stats = cache.stats().await;
        assert!(stats.unwrap().evictions > 0);
    }
}
