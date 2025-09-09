use super::{ContentHash, InvalidationResult};
use crate::embedding::EmbeddingError;

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{SystemTime, Duration};
use tokio::sync::RwLock;

#[derive(Debug, Clone)]
pub struct CachedEmbedding {
    pub embeddings: Vec<f32>,
    pub timestamp: SystemTime,
    pub access_count: u64,
    pub last_accessed: SystemTime,
    pub file_path: Option<String>,
    pub version: u32,
}

#[derive(Debug)]
pub struct EmbeddingCache {
    cache: HashMap<ContentHash, CachedEmbedding>,
    access_order: Vec<ContentHash>,
    max_size_bytes: usize,
    current_size_bytes: usize,
    hit_count: u64,
    miss_count: u64,
    ttl: Duration,
}

impl EmbeddingCache {
    pub fn new(max_size_bytes: usize) -> Self {
        Self {
            cache: HashMap::new(),
            access_order: Vec::new(),
            max_size_bytes,
            current_size_bytes: 0,
            hit_count: 0,
            miss_count: 0,
            ttl: Duration::from_secs(24 * 60 * 60), // 24 hours
        }
    }

    pub fn with_ttl(max_size_bytes: usize, ttl: Duration) -> Self {
        Self {
            cache: HashMap::new(),
            access_order: Vec::new(),
            max_size_bytes,
            current_size_bytes: 0,
            hit_count: 0,
            miss_count: 0,
            ttl,
        }
    }

    pub fn get(&mut self, hash: &ContentHash) -> Option<&CachedEmbedding> {
        if let Some(entry) = self.cache.get_mut(hash) {
            // Check TTL
            if entry.timestamp.elapsed().unwrap_or(Duration::ZERO) > self.ttl {
                self.cache.remove(hash);
                self.remove_from_access_order(hash);
                self.miss_count += 1;
                return None;
            }

            // Update access tracking
            entry.access_count += 1;
            entry.last_accessed = SystemTime::now();
            self.update_access_order(hash);
            self.hit_count += 1;
            
            Some(entry)
        } else {
            self.miss_count += 1;
            None
        }
    }

    pub fn insert(&mut self, hash: ContentHash, embedding: Vec<f32>) {
        let entry_size = self.calculate_entry_size(&embedding);
        
        // Make room if necessary
        while self.current_size_bytes + entry_size > self.max_size_bytes && !self.cache.is_empty() {
            self.evict_lru();
        }

        let cached_embedding = CachedEmbedding {
            embeddings: embedding,
            timestamp: SystemTime::now(),
            access_count: 1,
            last_accessed: SystemTime::now(),
            file_path: None,
            version: 1,
        };

        // Remove old entry if exists
        if let Some(old) = self.cache.remove(&hash) {
            self.current_size_bytes -= self.calculate_entry_size(&old.embeddings);
            self.remove_from_access_order(&hash);
        }

        // Insert new entry
        self.cache.insert(hash.clone(), cached_embedding);
        self.access_order.push(hash);
        self.current_size_bytes += entry_size;
    }

    pub fn invalidate(&mut self, hash: &ContentHash) -> bool {
        if let Some(entry) = self.cache.remove(hash) {
            self.current_size_bytes -= self.calculate_entry_size(&entry.embeddings);
            self.remove_from_access_order(hash);
            true
        } else {
            false
        }
    }

    pub fn invalidate_batch(&mut self, hashes: &[ContentHash]) -> usize {
        let mut count = 0;
        for hash in hashes {
            if self.invalidate(hash) {
                count += 1;
            }
        }
        count
    }

    pub fn clear(&mut self) {
        self.cache.clear();
        self.access_order.clear();
        self.current_size_bytes = 0;
    }

    pub fn cleanup_expired(&mut self) -> usize {
        let now = SystemTime::now();
        let mut expired = Vec::new();
        
        for (hash, entry) in &self.cache {
            if now.duration_since(entry.timestamp).unwrap_or(Duration::ZERO) > self.ttl {
                expired.push(hash.clone());
            }
        }

        let count = expired.len();
        for hash in expired {
            self.invalidate(&hash);
        }
        
        count
    }

    pub fn hit_count(&self) -> u64 {
        self.hit_count
    }

    pub fn miss_count(&self) -> u64 {
        self.miss_count
    }

    pub fn total_requests(&self) -> u64 {
        self.hit_count + self.miss_count
    }

    pub fn hit_rate(&self) -> f64 {
        let total = self.total_requests();
        if total > 0 {
            self.hit_count as f64 / total as f64
        } else {
            0.0
        }
    }

    pub fn size_bytes(&self) -> usize {
        self.current_size_bytes
    }

    pub fn entry_count(&self) -> usize {
        self.cache.len()
    }

    fn calculate_entry_size(&self, embedding: &[f32]) -> usize {
        // Size of vector + metadata overhead
        embedding.len() * std::mem::size_of::<f32>() + 128 // metadata overhead
    }

    fn evict_lru(&mut self) {
        if let Some(hash) = self.access_order.first().cloned() {
            self.invalidate(&hash);
        }
    }

    fn update_access_order(&mut self, hash: &ContentHash) {
        self.remove_from_access_order(hash);
        self.access_order.push(hash.clone());
    }

    fn remove_from_access_order(&mut self, hash: &ContentHash) {
        self.access_order.retain(|h| h != hash);
    }
}

pub struct IncrementalEmbeddingCache {
    cache: Arc<RwLock<EmbeddingCache>>,
    dependency_tracker: Arc<RwLock<super::DependencyTracker>>,
    update_processor: super::UpdateProcessor,
}

impl IncrementalEmbeddingCache {
    pub fn new(max_size_bytes: usize) -> Self {
        Self {
            cache: Arc::new(RwLock::new(EmbeddingCache::new(max_size_bytes))),
            dependency_tracker: Arc::new(RwLock::new(super::DependencyTracker::new())),
            update_processor: super::UpdateProcessor::new(),
        }
    }

    pub async fn get(&self, hash: &ContentHash) -> Option<CachedEmbedding> {
        let mut cache = self.cache.write().await;
        cache.get(hash).cloned()
    }

    pub async fn insert(&self, hash: ContentHash, embedding: Vec<f32>) {
        let mut cache = self.cache.write().await;
        cache.insert(hash, embedding);
    }

    pub async fn process_update(&self, request: super::UpdateRequest) -> Result<InvalidationResult, EmbeddingError> {
        let dependency_graph = {
            let tracker = self.dependency_tracker.read().await;
            tracker.get_graph().clone()
        };

        let invalidation_result = self.update_processor.process_update(&request, &dependency_graph)?;
        
        // Apply invalidations
        let hashes_to_invalidate: Vec<ContentHash> = invalidation_result
            .invalidated_files
            .iter()
            .map(|path| self.compute_file_hash(path))
            .collect();

        {
            let mut cache = self.cache.write().await;
            cache.invalidate_batch(&hashes_to_invalidate);
        }

        // Update dependencies
        {
            let mut tracker = self.dependency_tracker.write().await;
            tracker.update_dependencies(&request, &invalidation_result).await?;
        }

        Ok(invalidation_result)
    }

    pub async fn batch_process_updates(&self, requests: Vec<super::UpdateRequest>) -> Result<Vec<InvalidationResult>, EmbeddingError> {
        let mut results = Vec::with_capacity(requests.len());
        
        for request in requests {
            let result = self.process_update(request).await?;
            results.push(result);
        }
        
        Ok(results)
    }

    pub async fn get_stats(&self) -> CacheStats {
        let cache = self.cache.read().await;
        CacheStats {
            hit_count: cache.hit_count(),
            miss_count: cache.miss_count(),
            hit_rate: cache.hit_rate(),
            entry_count: cache.entry_count(),
            size_bytes: cache.size_bytes(),
            total_requests: cache.total_requests(),
        }
    }

    pub async fn cleanup(&self) -> CleanupStats {
        let mut cache = self.cache.write().await;
        let expired_count = cache.cleanup_expired();
        
        CleanupStats {
            expired_entries: expired_count,
            freed_bytes: 0, // Would need to track this
        }
    }

    fn compute_file_hash(&self, path: &std::path::Path) -> ContentHash {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        
        let mut hasher = DefaultHasher::new();
        path.hash(&mut hasher);
        ContentHash(hasher.finish())
    }
}

#[derive(Debug, Clone)]
pub struct CacheStats {
    pub hit_count: u64,
    pub miss_count: u64,
    pub hit_rate: f64,
    pub entry_count: usize,
    pub size_bytes: usize,
    pub total_requests: u64,
}

#[derive(Debug, Clone)]
pub struct CleanupStats {
    pub expired_entries: usize,
    pub freed_bytes: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_basic_operations() {
        let mut cache = EmbeddingCache::new(1024 * 1024); // 1MB
        let hash = ContentHash(12345);
        let embedding = vec![0.1, 0.2, 0.3, 0.4, 0.5];

        // Test insert and get
        cache.insert(hash.clone(), embedding.clone());
        let retrieved = cache.get(&hash);
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().embeddings, embedding);

        // Test hit rate
        assert_eq!(cache.hit_count(), 1);
        assert_eq!(cache.miss_count(), 0);
    }

    #[test]
    fn test_cache_lru_eviction() {
        let mut cache = EmbeddingCache::new(512); // Small cache
        let embedding = vec![0.0; 32]; // 128 bytes + overhead

        // Fill cache beyond capacity
        for i in 0..10 {
            cache.insert(ContentHash(i), embedding.clone());
        }

        // Earlier entries should be evicted
        assert!(cache.get(&ContentHash(0)).is_none());
        assert!(cache.get(&ContentHash(9)).is_some());
    }

    #[tokio::test]
    async fn test_incremental_cache() {
        let cache = IncrementalEmbeddingCache::new(1024 * 1024);
        let hash = ContentHash(54321);
        let embedding = vec![1.0, 2.0, 3.0];

        cache.insert(hash.clone(), embedding.clone()).await;
        let retrieved = cache.get(&hash).await;
        
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().embeddings, embedding);
    }
}