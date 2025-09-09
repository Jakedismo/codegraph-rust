use async_trait::async_trait;
use codegraph_core::{CodeGraphError, CodeNode, NodeId, Result};
use std::collections::HashMap;
use std::time::{Duration, SystemTime};

/// Cache key types for different cache strategies
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum CacheKey {
    /// Node ID for direct node caching
    Node(NodeId),
    /// Embedding hash for embedding caching
    Embedding(String),
    /// Query hash for query result caching
    Query(String),
    /// Custom key for application-specific caching
    Custom(String),
}

/// Cache entry metadata
#[derive(Debug, Clone)]
pub struct CacheEntry<T> {
    pub value: T,
    pub created_at: SystemTime,
    pub last_accessed: SystemTime,
    pub access_count: u64,
    pub size_bytes: usize,
    pub ttl: Option<Duration>,
}

impl<T> CacheEntry<T> {
    pub fn new(value: T, size_bytes: usize, ttl: Option<Duration>) -> Self {
        let now = SystemTime::now();
        Self {
            value,
            created_at: now,
            last_accessed: now,
            access_count: 1,
            size_bytes,
            ttl,
        }
    }

    pub fn is_expired(&self) -> bool {
        if let Some(ttl) = self.ttl {
            self.created_at.elapsed().unwrap_or(Duration::ZERO) > ttl
        } else {
            false
        }
    }

    pub fn touch(&mut self) {
        self.last_accessed = SystemTime::now();
        self.access_count += 1;
    }
}

/// Core cache trait for AI operations
#[async_trait]
pub trait AiCache<K, V>: Send + Sync {
    /// Insert a value into the cache
    async fn insert(&mut self, key: K, value: V, ttl: Option<Duration>) -> Result<()>;
    
    /// Get a value from the cache
    async fn get(&mut self, key: &K) -> Result<Option<V>>;
    
    /// Remove a value from the cache
    async fn remove(&mut self, key: &K) -> Result<()>;
    
    /// Clear all cache entries
    async fn clear(&mut self) -> Result<()>;
    
    /// Get cache statistics
    async fn stats(&self) -> Result<CacheStats>;
    
    /// Check if cache contains key
    async fn contains_key(&self, key: &K) -> bool;
    
    /// Get cache size (number of entries)
    async fn size(&self) -> Result<usize>;
    
    /// Check if cache is empty
    async fn is_empty(&self) -> bool {
        self.size().await.unwrap_or(0) == 0
    }
}

/// Cache performance statistics
#[derive(Debug, Clone, Default)]
pub struct CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
    pub entries: usize,
    pub memory_usage: u64,
    pub hit_rate: f64,
}

impl CacheStats {
    pub fn hit_rate(&self) -> f64 {
        if self.hits + self.misses == 0 {
            0.0
        } else {
            self.hits as f64 / (self.hits + self.misses) as f64
        }
    }
    
    pub fn miss_rate(&self) -> f64 {
        1.0 - self.hit_rate()
    }
}

/// Cache configuration options
#[derive(Debug, Clone)]
pub struct CacheConfig {
    pub max_size: usize,
    pub max_memory_bytes: usize,
    pub default_ttl: Option<Duration>,
    pub cleanup_interval: Duration,
    pub enable_metrics: bool,
    pub enable_compression: bool,
    pub compression_threshold_bytes: usize,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            max_size: 10_000,
            max_memory_bytes: 512 * 1024 * 1024, // 512MB
            default_ttl: Some(Duration::from_hours(24)),
            cleanup_interval: Duration::from_minutes(5),
            enable_metrics: true,
            enable_compression: true,
            compression_threshold_bytes: 1024, // 1KB
        }
    }
}

/// Trait for cache size estimation
pub trait CacheSizeEstimator {
    fn estimate_size(&self) -> usize;
}

impl CacheSizeEstimator for Vec<f32> {
    fn estimate_size(&self) -> usize {
        self.len() * std::mem::size_of::<f32>()
    }
}

impl CacheSizeEstimator for CodeNode {
    fn estimate_size(&self) -> usize {
        std::mem::size_of::<Self>() +
            self.name.len() +
            self.content.as_ref().map_or(0, |c| c.len()) +
            self.embedding.as_ref().map_or(0, |e| e.estimate_size())
    }
}

impl CacheSizeEstimator for String {
    fn estimate_size(&self) -> usize {
        self.len()
    }
}

impl CacheSizeEstimator for Vec<NodeId> {
    fn estimate_size(&self) -> usize {
        self.len() * std::mem::size_of::<NodeId>()
    }
}

/// Duration extension trait for convenience
trait DurationExt {
    fn from_hours(hours: u64) -> Duration;
    fn from_minutes(minutes: u64) -> Duration;
}

impl DurationExt for Duration {
    fn from_hours(hours: u64) -> Duration {
        Duration::from_secs(hours * 3600)
    }
    
    fn from_minutes(minutes: u64) -> Duration {
        Duration::from_secs(minutes * 60)
    }
}