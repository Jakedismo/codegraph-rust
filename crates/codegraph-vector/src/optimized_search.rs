use crate::index::{FaissIndexManager, IndexConfig, IndexStats, IndexType};
use crate::storage::PersistentStorage;
use codegraph_core::{CodeGraphError, NodeId, Result};
use dashmap::DashMap;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::{debug, info, warn};

/// Configuration for optimized search operations
#[derive(Debug, Clone)]
pub struct SearchConfig {
    /// Target maximum search latency in microseconds
    pub target_latency_us: u64,
    /// Enable result caching for repeated queries
    pub cache_enabled: bool,
    /// Maximum cache size in number of entries
    pub cache_max_entries: usize,
    /// Cache TTL in seconds
    pub cache_ttl_seconds: u64,
    /// Enable search result prefetching
    pub prefetch_enabled: bool,
    /// Number of results to prefetch beyond requested k
    pub prefetch_multiplier: f32,
    /// Enable parallel search for multiple queries
    pub parallel_search: bool,
    /// Memory pool size for search operations
    pub memory_pool_size_mb: usize,
}

impl Default for SearchConfig {
    fn default() -> Self {
        Self {
            target_latency_us: 800, // Sub-millisecond target
            cache_enabled: true,
            cache_max_entries: 10000,
            cache_ttl_seconds: 300,
            prefetch_enabled: true,
            prefetch_multiplier: 1.5,
            parallel_search: true,
            memory_pool_size_mb: 256,
        }
    }
}

/// Cached search result with TTL
#[derive(Debug, Clone)]
struct CachedResult {
    results: Vec<(NodeId, f32)>,
    timestamp: Instant,
    access_count: u64,
}

/// High-performance search engine optimized for sub-millisecond latency
pub struct OptimizedSearchEngine {
    config: SearchConfig,
    index_manager: Arc<RwLock<FaissIndexManager>>,
    
    // Result caching
    cache: Arc<DashMap<Vec<u8>, CachedResult>>, // Hash of query -> results
    cache_hits: Arc<AtomicU64>,
    cache_misses: Arc<AtomicU64>,
    
    // Performance monitoring
    search_times: Arc<RwLock<Vec<Duration>>>,
    total_searches: Arc<AtomicU64>,
    sub_ms_searches: Arc<AtomicU64>,
    
    // Memory pools for reuse
    vector_pool: Arc<RwLock<Vec<Vec<f32>>>>,
    result_pool: Arc<RwLock<Vec<Vec<(NodeId, f32)>>>>,
}

impl OptimizedSearchEngine {
    pub fn new(config: SearchConfig, index_config: IndexConfig) -> Result<Self> {
        let index_manager = Arc::new(RwLock::new(FaissIndexManager::new(index_config)));
        
        let engine = Self {
            config,
            index_manager,
            cache: Arc::new(DashMap::new()),
            cache_hits: Arc::new(AtomicU64::new(0)),
            cache_misses: Arc::new(AtomicU64::new(0)),
            search_times: Arc::new(RwLock::new(Vec::new())),
            total_searches: Arc::new(AtomicU64::new(0)),
            sub_ms_searches: Arc::new(AtomicU64::new(0)),
            vector_pool: Arc::new(RwLock::new(Vec::new())),
            result_pool: Arc::new(RwLock::new(Vec::new())),
        };

        // Pre-allocate memory pools
        engine.initialize_memory_pools();

        Ok(engine)
    }

    /// Initialize memory pools for efficient allocation reuse
    fn initialize_memory_pools(&self) {
        let pool_size = self.config.memory_pool_size_mb * 1024 * 1024 / (4 * 768); // Assume 768-dim f32 vectors
        
        {
            let mut vector_pool = self.vector_pool.write();
            for _ in 0..pool_size {
                vector_pool.push(Vec::with_capacity(768));
            }
        }

        {
            let mut result_pool = self.result_pool.write();
            for _ in 0..100 { // Reasonable number of result buffers
                result_pool.push(Vec::with_capacity(100));
            }
        }

        info!("Initialized memory pools: {} vector buffers", pool_size);
    }

    /// Perform optimized k-nearest neighbor search
    pub async fn search_knn(
        &self,
        query_embedding: &[f32],
        k: usize,
    ) -> Result<Vec<(NodeId, f32)>> {
        let search_start = Instant::now();
        self.total_searches.fetch_add(1, Ordering::Relaxed);

        // Check cache first if enabled
        if self.config.cache_enabled {
            if let Some(cached) = self.check_cache(query_embedding, k).await? {
                let search_time = search_start.elapsed();
                self.record_search_time(search_time);
                return Ok(cached);
            }
        }

        // Determine optimal k for prefetching
        let search_k = if self.config.prefetch_enabled {
            (k as f32 * self.config.prefetch_multiplier) as usize
        } else {
            k
        };

        // Perform the actual search
        let raw_results = self.perform_search(query_embedding, search_k).await?;
        
        // Truncate to requested k
        let mut results = raw_results;
        results.truncate(k);

        // Cache the result if caching is enabled
        if self.config.cache_enabled {
            self.cache_result(query_embedding, k, &results).await;
        }

        let search_time = search_start.elapsed();
        self.record_search_time(search_time);

        // Check if we met the latency target
        if search_time.as_micros() as u64 <= self.config.target_latency_us {
            self.sub_ms_searches.fetch_add(1, Ordering::Relaxed);
        }

        Ok(results)
    }

    /// Perform batch search for multiple queries in parallel
    pub async fn batch_search_knn(
        &self,
        queries: &[&[f32]],
        k: usize,
    ) -> Result<Vec<Vec<(NodeId, f32)>>> {
        if !self.config.parallel_search || queries.len() == 1 {
            // Sequential processing for small batches or when parallel is disabled
            let mut results = Vec::with_capacity(queries.len());
            for query in queries {
                results.push(self.search_knn(query, k).await?);
            }
            return Ok(results);
        }

        // Parallel processing for larger batches
        let search_futures: Vec<_> = queries
            .iter()
            .map(|query| self.search_knn(query, k))
            .collect();

        let results = futures::future::try_join_all(search_futures).await?;
        Ok(results)
    }

    /// Check cache for existing results
    async fn check_cache(&self, query: &[f32], k: usize) -> Result<Option<Vec<(NodeId, f32)>>> {
        let cache_key = self.compute_cache_key(query, k);
        
        if let Some(mut cached_entry) = self.cache.get_mut(&cache_key) {
            let now = Instant::now();
            let ttl = Duration::from_secs(self.config.cache_ttl_seconds);
            
            if now.duration_since(cached_entry.timestamp) < ttl {
                cached_entry.access_count += 1;
                self.cache_hits.fetch_add(1, Ordering::Relaxed);
                debug!("Cache hit for query (access count: {})", cached_entry.access_count);
                return Ok(Some(cached_entry.results.clone()));
            } else {
                // Entry expired, remove it
                drop(cached_entry);
                self.cache.remove(&cache_key);
            }
        }

        self.cache_misses.fetch_add(1, Ordering::Relaxed);
        Ok(None)
    }

    /// Cache search result
    async fn cache_result(&self, query: &[f32], k: usize, results: &[(NodeId, f32)]) {
        let cache_key = self.compute_cache_key(query, k);
        
        // Check cache size and evict if necessary
        if self.cache.len() >= self.config.cache_max_entries {
            self.evict_cache_entries().await;
        }

        let cached_result = CachedResult {
            results: results.to_vec(),
            timestamp: Instant::now(),
            access_count: 0,
        };

        self.cache.insert(cache_key, cached_result);
        debug!("Cached search result for k={}", k);
    }

    /// Compute cache key for query
    fn compute_cache_key(&self, query: &[f32], k: usize) -> Vec<u8> {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        
        // Hash the query vector (with some precision loss to increase cache hits)
        for &val in query {
            let rounded = (val * 1000.0).round() as i32; // 3 decimal places
            rounded.hash(&mut hasher);
        }
        k.hash(&mut hasher);
        
        hasher.finish().to_le_bytes().to_vec()
    }

    /// Evict old cache entries using LRU-like strategy
    async fn evict_cache_entries(&self) {
        let evict_count = self.config.cache_max_entries / 4; // Evict 25%
        let now = Instant::now();
        let ttl = Duration::from_secs(self.config.cache_ttl_seconds);

        // First, remove expired entries
        let mut expired_keys = Vec::new();
        for entry in self.cache.iter() {
            if now.duration_since(entry.value().timestamp) > ttl {
                expired_keys.push(entry.key().clone());
            }
        }

        for key in expired_keys {
            self.cache.remove(&key);
        }

        // If we still need to evict more, remove least accessed entries
        if self.cache.len() > self.config.cache_max_entries {
            let mut access_counts: Vec<_> = self.cache
                .iter()
                .map(|entry| (entry.key().clone(), entry.value().access_count))
                .collect();
            
            access_counts.sort_by_key(|&(_, count)| count);
            
            for (key, _) in access_counts.into_iter().take(evict_count) {
                self.cache.remove(&key);
            }
        }

        debug!("Cache eviction completed, {} entries remaining", self.cache.len());
    }

    /// Perform the actual FAISS search with optimizations
    async fn perform_search(&self, query: &[f32], k: usize) -> Result<Vec<(NodeId, f32)>> {
        // Get a vector buffer from the pool
        let query_vec = {
            let mut pool = self.vector_pool.write();
            if let Some(mut vec) = pool.pop() {
                vec.clear();
                vec.extend_from_slice(query);
                vec
            } else {
                query.to_vec()
            }
        };

        // Perform the search
        let results: Vec<(NodeId, f32)> = {
            let index_manager = self.index_manager.read();
            index_manager.search_knn(&query_vec, k)?
        };

        // Return vector to pool
        {
            let mut pool = self.vector_pool.write();
            if pool.len() < self.config.memory_pool_size_mb * 1024 / 4 { // Approximate capacity check
                pool.push(query_vec);
            }
        }

        Ok(results)
    }

    /// Record search time for performance monitoring
    fn record_search_time(&self, duration: Duration) {
        let mut times = self.search_times.write();
        times.push(duration);
        
        // Keep only recent search times (sliding window)
        if times.len() > 1000 {
            times.drain(0..500); // Remove oldest 500 entries
        }
    }

    /// Warm up the search engine with sample queries
    pub async fn warmup(&self, sample_queries: &[&[f32]], k: usize) -> Result<()> {
        info!("Starting search engine warmup with {} queries", sample_queries.len());
        let warmup_start = Instant::now();

        // Disable caching during warmup to get true performance
        let original_cache_enabled = self.config.cache_enabled;
        // We can't modify config directly, so we'll just note this for logging

        for query in sample_queries {
            let _ = self.search_knn(query, k).await;
        }

        let warmup_duration = warmup_start.elapsed();
        info!(
            "Search engine warmup completed in {:?} ({:.2} queries/sec)",
            warmup_duration,
            sample_queries.len() as f64 / warmup_duration.as_secs_f64()
        );

        Ok(())
    }

    /// Get comprehensive performance statistics
    pub fn get_performance_stats(&self) -> SearchPerformanceStats {
        let search_times = self.search_times.read();
        let total_searches = self.total_searches.load(Ordering::Relaxed);
        let sub_ms_searches = self.sub_ms_searches.load(Ordering::Relaxed);
        let cache_hits = self.cache_hits.load(Ordering::Relaxed);
        let cache_misses = self.cache_misses.load(Ordering::Relaxed);

        let (avg_latency, p95_latency, p99_latency) = if !search_times.is_empty() {
            let mut times: Vec<Duration> = search_times.clone();
            times.sort();

            let avg = Duration::from_nanos(
                times.iter().map(|d| d.as_nanos()).sum::<u128>() / times.len() as u128
            );

            let p95_idx = (times.len() as f64 * 0.95) as usize;
            let p99_idx = (times.len() as f64 * 0.99) as usize;

            (
                avg,
                times.get(p95_idx).copied().unwrap_or(Duration::ZERO),
                times.get(p99_idx).copied().unwrap_or(Duration::ZERO),
            )
        } else {
            (Duration::ZERO, Duration::ZERO, Duration::ZERO)
        };

        SearchPerformanceStats {
            total_searches,
            sub_millisecond_searches: sub_ms_searches,
            sub_ms_rate: if total_searches > 0 {
                sub_ms_searches as f64 / total_searches as f64
            } else {
                0.0
            },
            average_latency_us: avg_latency.as_micros() as u64,
            p95_latency_us: p95_latency.as_micros() as u64,
            p99_latency_us: p99_latency.as_micros() as u64,
            cache_hit_rate: if cache_hits + cache_misses > 0 {
                cache_hits as f64 / (cache_hits + cache_misses) as f64
            } else {
                0.0
            },
            cache_entries: self.cache.len(),
        }
    }

    /// Auto-tune search parameters based on performance feedback
    pub async fn auto_tune(&mut self) -> Result<()> {
        let stats = self.get_performance_stats();
        
        info!("Auto-tuning search parameters...");
        info!(
            "Current performance: {:.1}% sub-ms, avg: {}μs, p95: {}μs, cache hit: {:.1}%",
            stats.sub_ms_rate * 100.0,
            stats.average_latency_us,
            stats.p95_latency_us,
            stats.cache_hit_rate * 100.0
        );

        // Tune based on performance metrics
        if stats.sub_ms_rate < 0.8 && stats.average_latency_us > self.config.target_latency_us {
            warn!("Search performance below target, consider:");
            warn!("- Using HNSW index for better query performance");
            warn!("- Increasing cache size");
            warn!("- Enabling GPU acceleration");
            warn!("- Using lower precision embeddings");
        }

        // TODO: Implement automatic parameter adjustments
        // - Adjust search parameters (ef_search for HNSW)
        // - Modify cache settings
        // - Tune memory pool sizes

        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct SearchPerformanceStats {
    pub total_searches: u64,
    pub sub_millisecond_searches: u64,
    pub sub_ms_rate: f64,
    pub average_latency_us: u64,
    pub p95_latency_us: u64,
    pub p99_latency_us: u64,
    pub cache_hit_rate: f64,
    pub cache_entries: usize,
}
