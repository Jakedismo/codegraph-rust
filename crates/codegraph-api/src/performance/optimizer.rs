use std::collections::VecDeque;
use std::sync::Arc;
use std::time::{Duration, Instant};

use async_graphql::ID;
use codegraph_core::{Result, CodeGraphError, NodeId};
use codegraph_graph::CodeGraph;
use dashmap::DashMap;
use parking_lot::RwLock;
use serde::{de::DeserializeOwned, Serialize};
use sha2::{Digest, Sha256};
use tokio::sync::RwLock as AsyncRwLock;
use tracing::{debug, warn};

/// Configuration for the API-level performance optimizer
#[derive(Debug, Clone)]
pub struct PerformanceOptimizerConfig {
    /// Maximum cached entries (LRU)
    pub cache_capacity: usize,
    /// Time-to-live for cache entries
    pub cache_ttl_secs: u64,
    /// Maximum allowed traversal depth to guard complexity
    pub max_traversal_depth: usize,
    /// Maximum allowed expanded nodes for traversal
    pub max_traversal_nodes: usize,
    /// Whether to store cached payloads compressed in memory
    pub compress_cache_entries: bool,
}

impl Default for PerformanceOptimizerConfig {
    fn default() -> Self {
        Self {
            cache_capacity: 10_000,
            cache_ttl_secs: 60,
            max_traversal_depth: 64,
            max_traversal_nodes: 50_000,
            compress_cache_entries: true,
        }
    }
}

#[derive(Clone)]
struct CacheEntry {
    bytes: Vec<u8>,
    content_type: &'static str,
    compressed: bool,
    created_at: Instant,
    ttl: Duration,
}

impl CacheEntry {
    fn is_expired(&self) -> bool {
        self.created_at.elapsed() > self.ttl
    }
}

/// Simple concurrent LRU cache specialized for API query/results
#[derive(Clone)]
struct LruBytesCache {
    store: Arc<DashMap<String, CacheEntry>>,
    order: Arc<AsyncRwLock<VecDeque<String>>>,
    capacity: usize,
    ttl: Duration,
    hits: Arc<RwLock<u64>>,
    misses: Arc<RwLock<u64>>,
    evictions: Arc<RwLock<u64>>,
}

impl LruBytesCache {
    fn new(capacity: usize, ttl: Duration) -> Self {
        Self {
            store: Arc::new(DashMap::new()),
            order: Arc::new(AsyncRwLock::new(VecDeque::new())),
            capacity,
            ttl,
            hits: Arc::new(RwLock::new(0)),
            misses: Arc::new(RwLock::new(0)),
            evictions: Arc::new(RwLock::new(0)),
        }
    }

    async fn get(&self, key: &str) -> Option<CacheEntry> {
        if let Some(mut entry) = self.store.get_mut(key) {
            if entry.is_expired() {
                // drop expired
                drop(entry);
                self.store.remove(key);
                *self.misses.write() += 1;
                return None;
            }
            // Move to MRU position
            {
                let mut order = self.order.write().await;
                if let Some(pos) = order.iter().position(|k| k == key) {
                    order.remove(pos);
                }
                order.push_back(key.to_string());
            }
            *self.hits.write() += 1;
            return Some(entry.clone());
        }
        *self.misses.write() += 1;
        None
    }

    async fn put(&self, key: String, entry: CacheEntry) {
        // Ensure capacity (evict LRU)
        while self.store.len() >= self.capacity {
            let mut order = self.order.write().await;
            if let Some(lru_key) = order.pop_front() {
                let _ = self.store.remove(&lru_key);
                *self.evictions.write() += 1;
            } else {
                break;
            }
        }
        // Insert and mark MRU
        self.store.insert(key.clone(), entry);
        let mut order = self.order.write().await;
        if let Some(pos) = order.iter().position(|k| k == &key) {
            order.remove(pos);
        }
        order.push_back(key);
    }

    fn clear(&self) {
        self.store.clear();
    }

    fn stats(&self) -> CacheStats {
        CacheStats {
            entries: self.store.len(),
            hits: *self.hits.read(),
            misses: *self.misses.read(),
            evictions: *self.evictions.read(),
            hit_rate: {
                let h = *self.hits.read();
                let m = *self.misses.read();
                if h + m == 0 { 0.0 } else { h as f64 / (h + m) as f64 }
            },
        }
    }
}

#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct CacheStats {
    pub entries: usize,
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
    pub hit_rate: f64,
}

/// API-level performance optimizer that coordinates:
/// - Query result caching with LRU eviction
/// - Query complexity analysis
/// - Pathfinding algorithm selection (A*)
/// - Optional in-memory response compression
#[derive(Clone)]
pub struct PerformanceOptimizer {
    config: PerformanceOptimizerConfig,
    cache: LruBytesCache,
}

impl std::fmt::Debug for PerformanceOptimizer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PerformanceOptimizer")
            .field("capacity", &self.cache.capacity)
            .field("ttl_secs", &self.config.cache_ttl_secs)
            .field("compress_cache_entries", &self.config.compress_cache_entries)
            .finish()
    }
}

impl PerformanceOptimizer {
    pub fn new(config: PerformanceOptimizerConfig) -> Self {
        let ttl = Duration::from_secs(config.cache_ttl_secs);
        Self { config: config.clone(), cache: LruBytesCache::new(config.cache_capacity, ttl) }
    }

    pub fn stats(&self) -> CacheStats { self.cache.stats() }

    pub fn max_traversal_depth(&self) -> usize { self.config.max_traversal_depth }

    pub fn max_traversal_nodes(&self) -> usize { self.config.max_traversal_nodes }

    /// Generate a stable cache key for a GraphQL traversal request
    pub fn key_for_traversal(&self, start: &ID, max_depth: Option<i32>, limit: Option<i32>, edge_types: Option<&[String]>) -> String {
        let mut hasher = Sha256::new();
        hasher.update(start.to_string().as_bytes());
        hasher.update(max_depth.unwrap_or(0).to_le_bytes());
        hasher.update(limit.unwrap_or(0).to_le_bytes());
        if let Some(et) = edge_types { for e in et { hasher.update(e.as_bytes()); } }
        format!("traverse:{:x}", hasher.finalize())
    }

    /// Generate a stable cache key for a shortest path request
    pub fn key_for_path(&self, from: &ID, to: &ID, max_depth: Option<i32>) -> String {
        let mut hasher = Sha256::new();
        hasher.update(from.to_string().as_bytes());
        hasher.update(to.to_string().as_bytes());
        hasher.update(max_depth.unwrap_or(0).to_le_bytes());
        format!("path:{:x}", hasher.finalize())
    }

    /// Check a traversal request against complexity guardrails
    pub fn guard_traversal_complexity(&self, max_depth: Option<i32>, limit: Option<i32>) -> std::result::Result<(), CodeGraphError> {
        let depth = max_depth.unwrap_or(self.config.max_traversal_depth as i32) as usize;
        if depth > self.config.max_traversal_depth {
            return Err(CodeGraphError::InvalidQuery(format!(
                "Traversal depth {} exceeds limit {}", depth, self.config.max_traversal_depth
            )));
        }
        let limit_u = limit.unwrap_or(0).max(0) as usize;
        if limit_u > self.config.max_traversal_nodes {
            return Err(CodeGraphError::InvalidQuery(format!(
                "Traversal node limit {} exceeds max {}", limit_u, self.config.max_traversal_nodes
            )));
        }
        Ok(())
    }

    /// Get cached JSON payload
    pub async fn get_cached_json<T: DeserializeOwned>(&self, key: &str) -> Option<T> {
        if let Some(entry) = self.cache.get(key).await {
            let bytes = if entry.compressed {
                // decompress
                match zstd::decode_all(&entry.bytes[..]) {
                    Ok(b) => b,
                    Err(e) => { warn!("cache decompression failed: {}", e); entry.bytes }
                }
            } else { entry.bytes };
            match serde_json::from_slice::<T>(&bytes) {
                Ok(v) => Some(v),
                Err(e) => { warn!("cached json decode failed: {}", e); None }
            }
        } else { None }
    }

    /// Put cached JSON payload
    pub async fn put_cached_json<T: Serialize>(&self, key: String, value: &T) {
        match serde_json::to_vec(value) {
            Ok(mut bytes) => {
                let mut compressed = false;
                if self.config.compress_cache_entries && bytes.len() > 1024 {
                    match zstd::encode_all(&bytes[..], 0) {
                        Ok(b) => { bytes = b; compressed = true; }
                        Err(e) => warn!("cache compression failed: {}", e),
                    }
                }
                self.cache
                    .put(key, CacheEntry {
                        bytes,
                        content_type: "application/json",
                        compressed,
                        created_at: Instant::now(),
                        ttl: self.cache.ttl,
                    })
                    .await;
            }
            Err(e) => warn!("cache serialization failed: {}", e),
        }
    }

    /// Optimized pathfinding choosing A* when appropriate, falling back to BFS cache-assisted path
    pub async fn find_path_nodes(
        &self,
        graph: &CodeGraph,
        from: NodeId,
        to: NodeId,
        _max_depth: Option<i32>,
    ) -> Result<Option<Vec<NodeId>>> {
        // Prefer A* with a lightweight admissible heuristic (0 heuristic becomes Dijkstra)
        // If weights are non-uniform, A* will help prune. If not, fallback to BFS via graph.shortest_path
        // Try A* first and fall back on failure for safety.
        let heuristic = |_a: NodeId, _b: NodeId| -> f64 { 0.0 };
        match graph.astar_shortest_path(from, to, heuristic).await {
            Ok(opt) => Ok(opt),
            Err(_) => graph.shortest_path(from, to).await,
        }
    }
}

