use codegraph_core::NodeId;
use dashmap::DashMap;
use parking_lot::{RwLock, Mutex};
use std::collections::{HashMap, VecDeque};
use std::hash::{Hash, Hasher, DefaultHasher};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::time;

#[derive(Debug, Clone)]
pub struct CacheConfig {
    pub max_entries: usize,
    pub ttl: Duration,
    pub cleanup_interval: Duration,
    pub enable_stats: bool,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            max_entries: 10_000,
            ttl: Duration::from_secs(3600), // 1 hour
            cleanup_interval: Duration::from_secs(300), // 5 minutes
            enable_stats: true,
        }
    }
}

#[derive(Debug, Clone)]
pub struct CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
    pub entries: usize,
    pub hit_ratio: f64,
}

impl CacheStats {
    pub fn new() -> Self {
        Self {
            hits: 0,
            misses: 0,
            evictions: 0,
            entries: 0,
            hit_ratio: 0.0,
        }
    }

    pub fn update_hit_ratio(&mut self) {
        let total = self.hits + self.misses;
        self.hit_ratio = if total > 0 {
            self.hits as f64 / total as f64
        } else {
            0.0
        };
    }
}

#[derive(Debug, Clone)]
struct CacheEntry<T> {
    value: T,
    created_at: Instant,
    last_accessed: Instant,
    access_count: u64,
}

impl<T> CacheEntry<T> {
    fn new(value: T) -> Self {
        let now = Instant::now();
        Self {
            value,
            created_at: now,
            last_accessed: now,
            access_count: 1,
        }
    }

    fn access(&mut self) {
        self.last_accessed = Instant::now();
        self.access_count += 1;
    }

    fn is_expired(&self, ttl: Duration) -> bool {
        self.created_at.elapsed() > ttl
    }
}

pub struct LfuCache<K, V> 
where
    K: Hash + Eq + Clone,
    V: Clone,
{
    data: Arc<DashMap<K, CacheEntry<V>>>,
    frequency: Arc<DashMap<K, u64>>,
    access_order: Arc<Mutex<VecDeque<K>>>,
    config: CacheConfig,
    stats: Arc<RwLock<CacheStats>>,
    cleanup_handle: Option<tokio::task::JoinHandle<()>>,
}

impl<K, V> LfuCache<K, V>
where
    K: Hash + Eq + Clone + Send + Sync + 'static,
    V: Clone + Send + Sync + 'static,
{
    pub fn new(config: CacheConfig) -> Self {
        let cache = Self {
            data: Arc::new(DashMap::new()),
            frequency: Arc::new(DashMap::new()),
            access_order: Arc::new(Mutex::new(VecDeque::new())),
            config: config.clone(),
            stats: Arc::new(RwLock::new(CacheStats::new())),
            cleanup_handle: None,
        };

        cache
    }

    pub fn start_cleanup_task(&mut self) {
        let data = Arc::clone(&self.data);
        let frequency = Arc::clone(&self.frequency);
        let access_order = Arc::clone(&self.access_order);
        let stats = Arc::clone(&self.stats);
        let interval = self.config.cleanup_interval;
        let ttl = self.config.ttl;
        let max_entries = self.config.max_entries;

        let handle = tokio::spawn(async move {
            let mut cleanup_interval = time::interval(interval);
            
            loop {
                cleanup_interval.tick().await;
                
                // Remove expired entries
                let expired_keys: Vec<K> = data
                    .iter()
                    .filter_map(|entry| {
                        if entry.value().is_expired(ttl) {
                            Some(entry.key().clone())
                        } else {
                            None
                        }
                    })
                    .collect();

                for key in expired_keys {
                    data.remove(&key);
                    frequency.remove(&key);
                    if let Some(mut order) = access_order.try_lock() {
                        order.retain(|k| k != &key);
                    }
                }

                // Evict least frequently used entries if over capacity
                if data.len() > max_entries {
                    let evict_count = data.len() - max_entries;
                    let mut keys_to_evict = Vec::new();

                    // Find least frequently used keys
                    let mut freq_list: Vec<_> = frequency
                        .iter()
                        .map(|entry| (entry.key().clone(), *entry.value()))
                        .collect();
                    
                    freq_list.sort_by(|a, b| a.1.cmp(&b.1));
                    
                    for (key, _) in freq_list.into_iter().take(evict_count) {
                        keys_to_evict.push(key);
                    }

                    for key in keys_to_evict {
                        data.remove(&key);
                        frequency.remove(&key);
                        if let Some(mut order) = access_order.try_lock() {
                            order.retain(|k| k != &key);
                        }
                        
                        if let Some(mut stats) = stats.try_write() {
                            stats.evictions += 1;
                        }
                    }
                }

                // Update stats
                if let Some(mut stats) = stats.try_write() {
                    stats.entries = data.len();
                    stats.update_hit_ratio();
                }
            }
        });

        self.cleanup_handle = Some(handle);
    }

    pub fn get(&self, key: &K) -> Option<V> {
        if let Some(mut entry) = self.data.get_mut(key) {
            entry.access();
            
            // Update frequency
            self.frequency.entry(key.clone())
                .and_modify(|freq| *freq += 1)
                .or_insert(1);

            // Update access order
            if let Some(mut order) = self.access_order.try_lock() {
                order.retain(|k| k != key);
                order.push_back(key.clone());
            }

            if self.config.enable_stats {
                if let Some(mut stats) = self.stats.try_write() {
                    stats.hits += 1;
                    stats.update_hit_ratio();
                }
            }

            Some(entry.value.clone())
        } else {
            if self.config.enable_stats {
                if let Some(mut stats) = self.stats.try_write() {
                    stats.misses += 1;
                    stats.update_hit_ratio();
                }
            }
            None
        }
    }

    pub fn put(&self, key: K, value: V) {
        // Check if we need to evict before inserting
        if self.data.len() >= self.config.max_entries {
            self.evict_lfu();
        }

        let entry = CacheEntry::new(value);
        self.data.insert(key.clone(), entry);
        self.frequency.insert(key.clone(), 1);

        if let Some(mut order) = self.access_order.try_lock() {
            order.push_back(key);
        }

        if self.config.enable_stats {
            if let Some(mut stats) = self.stats.try_write() {
                stats.entries = self.data.len();
            }
        }
    }

    fn evict_lfu(&self) {
        // Find the least frequently used key
        if let Some(min_entry) = self.frequency
            .iter()
            .min_by(|a, b| a.value().cmp(b.value()))
        {
            let key_to_evict = min_entry.key().clone();
            
            self.data.remove(&key_to_evict);
            self.frequency.remove(&key_to_evict);
            
            if let Some(mut order) = self.access_order.try_lock() {
                order.retain(|k| k != &key_to_evict);
            }

            if self.config.enable_stats {
                if let Some(mut stats) = self.stats.try_write() {
                    stats.evictions += 1;
                }
            }
        }
    }

    pub fn remove(&self, key: &K) -> Option<V> {
        let value = self.data.remove(key).map(|(_, entry)| entry.value);
        self.frequency.remove(key);
        
        if let Some(mut order) = self.access_order.try_lock() {
            order.retain(|k| k != key);
        }

        value
    }

    pub fn clear(&self) {
        self.data.clear();
        self.frequency.clear();
        if let Some(mut order) = self.access_order.try_lock() {
            order.clear();
        }

        if self.config.enable_stats {
            if let Some(mut stats) = self.stats.try_write() {
                *stats = CacheStats::new();
            }
        }
    }

    pub fn len(&self) -> usize {
        self.data.len()
    }

    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    pub fn get_stats(&self) -> CacheStats {
        if let Some(stats) = self.stats.try_read() {
            stats.clone()
        } else {
            CacheStats::new()
        }
    }

    pub fn contains_key(&self, key: &K) -> bool {
        self.data.contains_key(key)
    }
}

impl<K, V> Drop for LfuCache<K, V>
where
    K: Hash + Eq + Clone,
    V: Clone,
{
    fn drop(&mut self) {
        if let Some(handle) = self.cleanup_handle.take() {
            handle.abort();
        }
    }
}

// Specialized caches for search results
pub type QueryResultCache = LfuCache<QueryHash, Vec<(NodeId, f32)>>;
pub type EmbeddingCache = LfuCache<NodeId, Vec<f32>>;
pub type ContextCache = LfuCache<ContextHash, f32>;

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct QueryHash {
    embedding_hash: u64,
    k: usize,
    config_hash: u64,
}

impl QueryHash {
    pub fn new(embedding: &[f32], k: usize, config: &str) -> Self {
        let mut hasher = DefaultHasher::new();
        
        // Hash embedding (sample every 10th element for performance)
        for (i, &val) in embedding.iter().enumerate() {
            if i % 10 == 0 {
                (val as u32).hash(&mut hasher);
            }
        }
        let embedding_hash = hasher.finish();

        let mut config_hasher = DefaultHasher::new();
        config.hash(&mut config_hasher);
        let config_hash = config_hasher.finish();

        Self {
            embedding_hash,
            k,
            config_hash,
        }
    }
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct ContextHash {
    nodes: Vec<NodeId>,
    context_type: String,
}

impl ContextHash {
    pub fn new(nodes: Vec<NodeId>, context_type: String) -> Self {
        Self { nodes, context_type }
    }
}

// High-performance cache manager for the KNN engine
pub struct SearchCacheManager {
    query_cache: QueryResultCache,
    embedding_cache: EmbeddingCache,
    context_cache: ContextCache,
}

impl SearchCacheManager {
    pub fn new(
        query_config: CacheConfig,
        embedding_config: CacheConfig,
        context_config: CacheConfig,
    ) -> Self {
        let mut query_cache = QueryResultCache::new(query_config);
        let mut embedding_cache = EmbeddingCache::new(embedding_config);
        let mut context_cache = ContextCache::new(context_config);

        query_cache.start_cleanup_task();
        embedding_cache.start_cleanup_task();
        context_cache.start_cleanup_task();

        Self {
            query_cache,
            embedding_cache,
            context_cache,
        }
    }

    pub fn get_query_results(&self, query_hash: &QueryHash) -> Option<Vec<(NodeId, f32)>> {
        self.query_cache.get(query_hash)
    }

    pub fn cache_query_results(&self, query_hash: QueryHash, results: Vec<(NodeId, f32)>) {
        self.query_cache.put(query_hash, results);
    }

    pub fn get_embedding(&self, node_id: &NodeId) -> Option<Vec<f32>> {
        self.embedding_cache.get(node_id)
    }

    pub fn cache_embedding(&self, node_id: NodeId, embedding: Vec<f32>) {
        self.embedding_cache.put(node_id, embedding);
    }

    pub fn get_context_score(&self, context_hash: &ContextHash) -> Option<f32> {
        self.context_cache.get(context_hash)
    }

    pub fn cache_context_score(&self, context_hash: ContextHash, score: f32) {
        self.context_cache.put(context_hash, score);
    }

    pub fn clear_all(&self) {
        self.query_cache.clear();
        self.embedding_cache.clear();
        self.context_cache.clear();
    }

    pub fn get_cache_stats(&self) -> HashMap<String, CacheStats> {
        let mut stats = HashMap::new();
        stats.insert("query_cache".to_string(), self.query_cache.get_stats());
        stats.insert("embedding_cache".to_string(), self.embedding_cache.get_stats());
        stats.insert("context_cache".to_string(), self.context_cache.get_stats());
        stats
    }
}
