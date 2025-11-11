use crate::{AiCache, CacheConfig, CacheEntry, CacheSizeEstimator, CacheStats};
use async_trait::async_trait;
use codegraph_core::{CodeNode, NodeId, Result};
use dashmap::DashMap;
use sha2::{Digest, Sha256};
use std::collections::VecDeque;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock as AsyncRwLock;

/// High-performance embedding cache with LRU eviction and compression
#[derive(Clone)]
pub struct EmbeddingCache {
    /// Thread-safe cache storage
    cache: Arc<DashMap<String, CacheEntry<Vec<f32>>>>,
    /// LRU tracking for eviction
    lru_queue: Arc<AsyncRwLock<VecDeque<String>>>,
    /// Cache configuration
    config: CacheConfig,
    /// Performance metrics
    stats: Arc<AsyncRwLock<CacheStats>>,
    /// Current memory usage in bytes
    memory_usage: Arc<parking_lot::Mutex<usize>>,
}

impl EmbeddingCache {
    pub fn new(config: CacheConfig) -> Self {
        Self {
            cache: Arc::new(DashMap::new()),
            lru_queue: Arc::new(AsyncRwLock::new(VecDeque::new())),
            config,
            stats: Arc::new(AsyncRwLock::new(CacheStats::default())),
            memory_usage: Arc::new(parking_lot::Mutex::new(0)),
        }
    }

    /// Create cache key from embedding vector
    pub fn create_embedding_key(embedding: &[f32]) -> String {
        let mut hasher = Sha256::new();
        for &value in embedding {
            hasher.update(value.to_le_bytes());
        }
        format!("emb_{:x}", hasher.finalize())
    }

    /// Create cache key from code node content
    pub fn create_node_content_key(node: &CodeNode) -> String {
        let mut hasher = Sha256::new();
        hasher.update(node.name.as_bytes());
        if let Some(content) = &node.content {
            hasher.update(content.as_bytes());
        }
        if let Some(lang) = &node.language {
            hasher.update(format!("{:?}", lang).as_bytes());
        }
        if let Some(node_type) = &node.node_type {
            hasher.update(format!("{:?}", node_type).as_bytes());
        }
        format!("node_{:x}", hasher.finalize())
    }

    /// Store embedding with node ID as key
    pub async fn store_node_embedding(
        &mut self,
        node_id: NodeId,
        embedding: Vec<f32>,
    ) -> Result<()> {
        let key = node_id.to_string();
        self.insert(key, embedding, self.config.default_ttl).await
    }

    /// Retrieve embedding by node ID
    pub async fn get_node_embedding(&mut self, node_id: NodeId) -> Result<Option<Vec<f32>>> {
        let key = node_id.to_string();
        self.get(&key).await
    }

    /// Store embedding by content hash
    pub async fn store_content_embedding(
        &mut self,
        node: &CodeNode,
        embedding: Vec<f32>,
    ) -> Result<()> {
        let key = Self::create_node_content_key(node);
        self.insert(key, embedding, self.config.default_ttl).await
    }

    /// Retrieve embedding by content hash
    pub async fn get_content_embedding(&mut self, node: &CodeNode) -> Result<Option<Vec<f32>>> {
        let key = Self::create_node_content_key(node);
        self.get(&key).await
    }

    /// Check if we need to evict entries due to memory pressure
    async fn check_memory_pressure(&self) -> bool {
        let current_memory = *self.memory_usage.lock();
        current_memory > self.config.max_memory_bytes
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

    /// Update LRU position for accessed key
    async fn update_lru(&self, key: &str) {
        let mut lru_queue = self.lru_queue.write().await;

        // Remove key if it exists (inefficient but simple for MVP)
        lru_queue.retain(|k| k != key);
        // Add to back as most recently used
        lru_queue.push_back(key.to_string());

        // Limit queue size to prevent memory issues
        while lru_queue.len() > self.config.max_size {
            lru_queue.pop_front();
        }
    }

    /// Compress embedding if enabled and above threshold
    fn maybe_compress(&self, embedding: Vec<f32>) -> Vec<f32> {
        if self.config.enable_compression
            && embedding.estimate_size() > self.config.compression_threshold_bytes
        {
            // Simple quantization compression (reduce precision)
            embedding
                .into_iter()
                .map(|f| (f * 127.0).round() / 127.0)
                .collect()
        } else {
            embedding
        }
    }

    /// Clean up expired entries
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
impl AiCache<String, Vec<f32>> for EmbeddingCache {
    async fn insert(&mut self, key: String, value: Vec<f32>, ttl: Option<Duration>) -> Result<()> {
        // Compress if configured
        let compressed_value = self.maybe_compress(value);
        let size_bytes = compressed_value.estimate_size() + key.len();

        // Create cache entry
        let entry = CacheEntry::new(compressed_value, size_bytes, ttl);

        // Check if we need to evict first
        {
            let mut memory_usage = self.memory_usage.lock();
            *memory_usage += size_bytes;
        }

        if self.check_memory_pressure().await {
            self.evict_lru().await?;
        }

        // Insert entry
        self.cache.insert(key.clone(), entry);

        // Update LRU
        self.update_lru(&key).await;

        // Update metrics
        if self.config.enable_metrics {
            let mut stats = self.stats.write().await;
            stats.entries = self.cache.len();
            stats.memory_usage = *self.memory_usage.lock() as u64;
        }

        Ok(())
    }

    async fn get(&mut self, key: &String) -> Result<Option<Vec<f32>>> {
        if let Some(mut entry) = self.cache.get_mut(key) {
            // Check if expired
            if entry.is_expired() {
                drop(entry);
                self.cache.remove(key);

                // Update metrics
                if self.config.enable_metrics {
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
            if self.config.enable_metrics {
                let mut stats = self.stats.write().await;
                stats.hits += 1;
            }

            Ok(Some(result))
        } else {
            // Update metrics
            if self.config.enable_metrics {
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
            if self.config.enable_metrics {
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

        if self.config.enable_metrics {
            let mut stats = self.stats.write().await;
            stats.entries = 0;
            stats.memory_usage = 0;
        }

        Ok(())
    }

    async fn stats(&self) -> Result<CacheStats> {
        if self.config.enable_metrics {
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

impl Default for EmbeddingCache {
    fn default() -> Self {
        Self::new(CacheConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_relative_eq;

    #[tokio::test]
    async fn test_embedding_cache_basic_operations() {
        let mut cache = EmbeddingCache::default();
        let embedding = vec![0.1, 0.2, 0.3, 0.4];
        let key = "test_key".to_string();

        // Test insert and get
        cache
            .insert(key.clone(), embedding.clone(), None)
            .await
            .unwrap();
        let result = cache.get(&key).await.unwrap();
        assert_eq!(result, Some(embedding));

        // Test remove
        let removed = cache.remove(&key).await.unwrap();
        assert!(removed.is_some());
        let result = cache.get(&key).await.unwrap();
        assert_eq!(result, None);
    }

    #[tokio::test]
    async fn test_node_embedding_storage() {
        let mut cache = EmbeddingCache::default();
        let node_id = NodeId::new_v4();
        let embedding = vec![0.5, 0.6, 0.7, 0.8];

        cache
            .store_node_embedding(node_id, embedding.clone())
            .await
            .unwrap();
        let result = cache.get_node_embedding(node_id).await.unwrap();
        assert_eq!(result, Some(embedding));
    }

    #[tokio::test]
    async fn test_content_based_caching() {
        let mut cache = EmbeddingCache::default();
        let node = CodeNode::new(
            "test_function".to_string(),
            Some(codegraph_core::NodeType::Function),
            Some(codegraph_core::Language::Rust),
            codegraph_core::Location {
                file_path: "test.rs".to_string(),
                line: 1,
                column: 1,
                end_line: None,
                end_column: None,
            },
        )
        .with_content("fn test() {}".to_string());

        let embedding = vec![0.9, 0.8, 0.7, 0.6];

        cache
            .store_content_embedding(&node, embedding.clone())
            .await
            .unwrap();
        let result = cache.get_content_embedding(&node).await.unwrap();
        assert_eq!(result, Some(embedding));
    }

    #[tokio::test]
    async fn test_memory_pressure_eviction() {
        let config = CacheConfig {
            max_memory_bytes: 1024, // Very small limit
            max_size: 100,
            ..Default::default()
        };
        let mut cache = EmbeddingCache::new(config);

        // Fill cache beyond memory limit
        for i in 0..20 {
            let key = format!("key_{}", i);
            let embedding = vec![i as f32; 128]; // Large embedding
            cache.insert(key, embedding, None).await.unwrap();
        }

        // Check that eviction occurred
        let stats = cache.stats().await;
        assert!(stats.evictions > 0);
    }

    #[tokio::test]
    async fn test_ttl_expiration() {
        let mut cache = EmbeddingCache::default();
        let key = "ttl_test".to_string();
        let embedding = vec![1.0, 2.0, 3.0];
        let ttl = Duration::from_millis(50);

        cache
            .insert(key.clone(), embedding, Some(ttl))
            .await
            .unwrap();

        // Should be available immediately
        assert!(cache.get(&key).await.unwrap().is_some());

        // Wait for expiration
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Should be expired now
        assert!(cache.get(&key).await.unwrap().is_none());
    }
}
