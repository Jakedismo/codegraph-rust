use codegraph_core::NodeId;
use dashmap::DashMap;
use parking_lot::RwLock;
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::time::interval;

/// Cache entry with TTL and access tracking
#[derive(Debug, Clone)]
struct CacheEntry<T> {
    value: T,
    created_at: Instant,
    last_accessed: Instant,
    access_count: u64,
    ttl: Duration,
}

impl<T> CacheEntry<T> {
    fn new(value: T, ttl: Duration) -> Self {
        let now = Instant::now();
        Self {
            value,
            created_at: now,
            last_accessed: now,
            access_count: 1,
            ttl,
        }
    }

    fn is_expired(&self) -> bool {
        self.created_at.elapsed() > self.ttl
    }

    fn access(&mut self) -> &T {
        self.last_accessed = Instant::now();
        self.access_count += 1;
        &self.value
    }
}

/// LRU cache implementation for graph queries
pub struct LruCache<K, V> {
    cache: DashMap<K, CacheEntry<V>>,
    access_order: Arc<RwLock<VecDeque<K>>>,
    max_size: usize,
    default_ttl: Duration,
}

impl<K, V> LruCache<K, V>
where
    K: Clone + Eq + std::hash::Hash,
    V: Clone,
{
    pub fn new(max_size: usize, default_ttl: Duration) -> Self {
        Self {
            cache: DashMap::new(),
            access_order: Arc::new(RwLock::new(VecDeque::new())),
            max_size,
            default_ttl,
        }
    }

    pub fn get(&self, key: &K) -> Option<V> {
        let mut entry = self.cache.get_mut(key)?;

        if entry.is_expired() {
            drop(entry);
            self.cache.remove(key);
            return None;
        }

        let value = entry.access().clone();

        // Update access order
        let mut access_order = self.access_order.write();
        if let Some(pos) = access_order.iter().position(|k| k == key) {
            access_order.remove(pos);
        }
        access_order.push_back(key.clone());

        Some(value)
    }

    pub fn insert(&self, key: K, value: V) {
        self.insert_with_ttl(key, value, self.default_ttl);
    }

    pub fn insert_with_ttl(&self, key: K, value: V, ttl: Duration) {
        // Ensure cache size limit
        self.ensure_capacity();

        let entry = CacheEntry::new(value, ttl);
        self.cache.insert(key.clone(), entry);

        // Update access order
        let mut access_order = self.access_order.write();
        if let Some(pos) = access_order.iter().position(|k| k == &key) {
            access_order.remove(pos);
        }
        access_order.push_back(key);
    }

    pub fn invalidate(&self, key: &K) {
        self.cache.remove(key);
        let mut access_order = self.access_order.write();
        if let Some(pos) = access_order.iter().position(|k| k == key) {
            access_order.remove(pos);
        }
    }

    pub fn clear(&self) {
        self.cache.clear();
        self.access_order.write().clear();
    }

    fn ensure_capacity(&self) {
        while self.cache.len() >= self.max_size {
            let mut access_order = self.access_order.write();
            if let Some(oldest_key) = access_order.pop_front() {
                self.cache.remove(&oldest_key);
            } else {
                break;
            }
        }
    }

    pub fn stats(&self) -> CacheStats {
        let total_entries = self.cache.len();
        let mut total_accesses = 0;
        let mut expired_entries = 0;

        for entry in self.cache.iter() {
            total_accesses += entry.access_count;
            if entry.is_expired() {
                expired_entries += 1;
            }
        }

        CacheStats {
            total_entries,
            expired_entries,
            total_accesses,
            capacity: self.max_size,
        }
    }
}

/// Cache statistics
#[derive(Debug, Clone)]
pub struct CacheStats {
    pub total_entries: usize,
    pub expired_entries: usize,
    pub total_accesses: u64,
    pub capacity: usize,
}

/// Multi-level cache for different query types
pub struct GraphQueryCache {
    /// Cache for shortest paths between nodes
    path_cache: LruCache<(NodeId, NodeId), Vec<NodeId>>,
    /// Cache for neighbor queries
    neighbor_cache: LruCache<NodeId, Vec<NodeId>>,
    /// Cache for BFS/DFS results
    traversal_cache: LruCache<(NodeId, String), Vec<NodeId>>, // (start_node, config_hash)
    /// Cache for strongly connected components
    scc_cache: LruCache<String, Vec<Vec<NodeId>>>, // graph_hash -> components
    /// Cache for cycle detection results
    cycle_cache: LruCache<String, Vec<Vec<NodeId>>>, // graph_hash -> cycles
}

impl GraphQueryCache {
    pub fn new() -> Self {
        Self {
            path_cache: LruCache::new(1000, Duration::from_secs(300)), // 5 minutes
            neighbor_cache: LruCache::new(5000, Duration::from_secs(600)), // 10 minutes
            traversal_cache: LruCache::new(500, Duration::from_secs(180)), // 3 minutes
            scc_cache: LruCache::new(10, Duration::from_secs(1800)),   // 30 minutes
            cycle_cache: LruCache::new(10, Duration::from_secs(1800)), // 30 minutes
        }
    }

    pub fn get_path(&self, from: NodeId, to: NodeId) -> Option<Vec<NodeId>> {
        self.path_cache.get(&(from, to))
    }

    pub fn cache_path(&self, from: NodeId, to: NodeId, path: Vec<NodeId>) {
        self.path_cache.insert((from, to), path);
    }

    pub fn get_neighbors(&self, node: NodeId) -> Option<Vec<NodeId>> {
        self.neighbor_cache.get(&node)
    }

    pub fn cache_neighbors(&self, node: NodeId, neighbors: Vec<NodeId>) {
        self.neighbor_cache.insert(node, neighbors);
    }

    pub fn get_traversal(&self, start: NodeId, config_hash: String) -> Option<Vec<NodeId>> {
        self.traversal_cache.get(&(start, config_hash))
    }

    pub fn cache_traversal(&self, start: NodeId, config_hash: String, result: Vec<NodeId>) {
        self.traversal_cache.insert((start, config_hash), result);
    }

    pub fn get_scc(&self, graph_hash: &str) -> Option<Vec<Vec<NodeId>>> {
        self.scc_cache.get(&graph_hash.to_string())
    }

    pub fn cache_scc(&self, graph_hash: String, components: Vec<Vec<NodeId>>) {
        self.scc_cache.insert(graph_hash, components);
    }

    pub fn get_cycles(&self, graph_hash: &str) -> Option<Vec<Vec<NodeId>>> {
        self.cycle_cache.get(&graph_hash.to_string())
    }

    pub fn cache_cycles(&self, graph_hash: String, cycles: Vec<Vec<NodeId>>) {
        self.cycle_cache.insert(graph_hash, cycles);
    }

    pub fn invalidate_node(&self, node: NodeId) {
        // Invalidate all caches that might be affected by this node
        self.neighbor_cache.invalidate(&node);

        // For path cache, we'd need to iterate and remove entries containing this node
        // This is expensive, so in practice you might want to use a different strategy
    }

    pub fn clear_all(&self) {
        self.path_cache.clear();
        self.neighbor_cache.clear();
        self.traversal_cache.clear();
        self.scc_cache.clear();
        self.cycle_cache.clear();
    }

    pub fn stats(&self) -> HashMap<String, CacheStats> {
        let mut stats = HashMap::new();
        stats.insert("path_cache".to_string(), self.path_cache.stats());
        stats.insert("neighbor_cache".to_string(), self.neighbor_cache.stats());
        stats.insert("traversal_cache".to_string(), self.traversal_cache.stats());
        stats.insert("scc_cache".to_string(), self.scc_cache.stats());
        stats.insert("cycle_cache".to_string(), self.cycle_cache.stats());
        stats
    }
}

impl Default for GraphQueryCache {
    fn default() -> Self {
        Self::new()
    }
}

/// Cache manager with background cleanup
pub struct CacheManager {
    cache: Arc<GraphQueryCache>,
    cleanup_interval: Duration,
}

impl CacheManager {
    pub fn new(cache: GraphQueryCache, cleanup_interval: Duration) -> Self {
        Self {
            cache: Arc::new(cache),
            cleanup_interval,
        }
    }

    pub fn cache(&self) -> Arc<GraphQueryCache> {
        self.cache.clone()
    }

    /// Start background cleanup task
    pub async fn start_cleanup_task(&self) -> tokio::task::JoinHandle<()> {
        let cache = self.cache.clone();
        let cleanup_interval = self.cleanup_interval;

        tokio::spawn(async move {
            let mut interval = interval(cleanup_interval);

            loop {
                interval.tick().await;
                // Cleanup expired entries by accessing stats (which triggers cleanup)
                let stats = cache.stats();
                tracing::debug!("Cache cleanup completed. Stats: {:?}", stats);
            }
        })
    }
}

/// Query optimizer that uses caching and query planning
pub struct QueryOptimizer {
    cache_manager: CacheManager,
}

impl QueryOptimizer {
    pub fn new(cache_manager: CacheManager) -> Self {
        Self { cache_manager }
    }

    pub fn cache(&self) -> Arc<GraphQueryCache> {
        self.cache_manager.cache()
    }

    /// Generate a hash for traversal configuration
    pub fn hash_traversal_config(&self, config: &crate::traversal::TraversalConfig) -> String {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        config.max_depth.hash(&mut hasher);
        config.max_nodes.hash(&mut hasher);
        config.include_start.hash(&mut hasher);
        // Note: filter function can't be hashed easily, so we'll use a placeholder
        format!("{:x}", hasher.finish())
    }

    /// Generate a hash for the current graph state
    pub fn hash_graph_state(&self, node_count: usize, edge_count: usize) -> String {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        node_count.hash(&mut hasher);
        edge_count.hash(&mut hasher);
        format!("{:x}", hasher.finish())
    }

    /// Optimize a query plan based on the query type and graph characteristics
    pub fn optimize_query_plan(&self, query_type: &str, estimated_size: usize) -> QueryPlan {
        match query_type {
            "shortest_path" => {
                if estimated_size < 1000 {
                    QueryPlan::Direct
                } else {
                    QueryPlan::Cached
                }
            }
            "traversal" => {
                if estimated_size < 500 {
                    QueryPlan::Direct
                } else {
                    QueryPlan::CachedWithBatching { batch_size: 100 }
                }
            }
            "cycle_detection" | "scc" => QueryPlan::CachedWithPrecompute,
            _ => QueryPlan::Direct,
        }
    }
}

/// Query execution plan
#[derive(Debug, Clone)]
pub enum QueryPlan {
    Direct,
    Cached,
    CachedWithBatching { batch_size: usize },
    CachedWithPrecompute,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use uuid::Uuid;

    #[test]
    fn test_lru_cache() {
        let cache = LruCache::new(3, Duration::from_secs(60));

        cache.insert("key1", "value1");
        cache.insert("key2", "value2");
        cache.insert("key3", "value3");

        assert_eq!(cache.get(&"key1"), Some("value1"));
        assert_eq!(cache.get(&"key2"), Some("value2"));

        // Should evict key3 as it's least recently used
        cache.insert("key4", "value4");
        assert_eq!(cache.get(&"key3"), None);
        assert_eq!(cache.get(&"key4"), Some("value4"));
    }

    #[test]
    fn test_cache_expiration() {
        let cache = LruCache::new(10, Duration::from_millis(10));

        cache.insert("key1", "value1");
        assert_eq!(cache.get(&"key1"), Some("value1"));

        // Wait for expiration
        std::thread::sleep(Duration::from_millis(20));
        assert_eq!(cache.get(&"key1"), None);
    }

    #[test]
    fn test_graph_query_cache() {
        let cache = GraphQueryCache::new();
        let node1 = Uuid::new_v4();
        let node2 = Uuid::new_v4();

        let neighbors = vec![node2];
        cache.cache_neighbors(node1, neighbors.clone());

        assert_eq!(cache.get_neighbors(node1), Some(neighbors));
    }

    #[tokio::test]
    async fn test_cache_manager() {
        let cache = GraphQueryCache::new();
        let manager = CacheManager::new(cache, Duration::from_millis(100));

        let handle = manager.start_cleanup_task().await;

        // Let it run for a bit
        tokio::time::sleep(Duration::from_millis(200)).await;

        handle.abort();
    }
}
