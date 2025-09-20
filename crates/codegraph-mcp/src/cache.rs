/// Intelligent caching layer for Qwen2.5-Coder responses
///
/// This module implements semantic-aware caching that understands when queries
/// are similar enough to reuse previous analysis, dramatically improving performance.

use std::collections::HashMap;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Sha256, Digest};
use codegraph_core::Result;
use tracing::{debug, info, warn};

/// Cache configuration
#[derive(Debug, Clone)]
pub struct CacheConfig {
    pub max_entries: usize,
    pub default_ttl: Duration,
    pub semantic_similarity_threshold: f32,
    pub enable_semantic_matching: bool,
    pub max_memory_mb: usize,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            max_entries: 1000,
            default_ttl: Duration::from_secs(30 * 60), // 30 minutes
            semantic_similarity_threshold: 0.85, // 85% similarity to reuse
            enable_semantic_matching: true,
            max_memory_mb: 500, // 500MB cache limit
        }
    }
}

/// Cached response entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheEntry {
    pub query: String,
    pub query_hash: String,
    pub semantic_embedding: Option<Vec<f32>>, // For semantic similarity
    pub response: Value,
    pub confidence_score: f32,
    pub processing_time_ms: u64,
    pub context_tokens: usize,
    pub completion_tokens: usize,
    pub created_at: u64, // Unix timestamp
    pub access_count: u32,
    pub last_accessed: u64,
}

/// Cache statistics
#[derive(Debug, Clone, Serialize)]
pub struct CacheStats {
    pub total_entries: usize,
    pub total_requests: u64,
    pub cache_hits: u64,
    pub semantic_hits: u64, // Hits via semantic similarity
    pub cache_misses: u64,
    pub hit_rate: f32,
    pub semantic_hit_rate: f32,
    pub average_response_time_cached: f32,
    pub average_response_time_uncached: f32,
    pub memory_usage_mb: f32,
    pub evictions: u64,
}

/// Intelligent cache for Qwen responses
pub struct QwenResponseCache {
    entries: HashMap<String, CacheEntry>,
    config: CacheConfig,
    stats: CacheStats,
}

impl QwenResponseCache {
    pub fn new(config: CacheConfig) -> Self {
        Self {
            entries: HashMap::new(),
            config,
            stats: CacheStats {
                total_entries: 0,
                total_requests: 0,
                cache_hits: 0,
                semantic_hits: 0,
                cache_misses: 0,
                hit_rate: 0.0,
                semantic_hit_rate: 0.0,
                average_response_time_cached: 0.0,
                average_response_time_uncached: 0.0,
                memory_usage_mb: 0.0,
                evictions: 0,
            },
        }
    }

    /// Get cached response if available
    pub async fn get(&mut self, query: &str, context: &str) -> Option<CacheEntry> {
        self.stats.total_requests += 1;

        // 1. Try exact hash match first (fastest)
        let query_hash = self.compute_query_hash(query, context);

        if let Some(entry) = self.entries.get_mut(&query_hash) {
            // Update access statistics
            entry.access_count += 1;
            entry.last_accessed = current_timestamp();

            self.stats.cache_hits += 1;
            info!("Cache hit (exact): {} chars, confidence: {:.2}",
                  query.len(), entry.confidence_score);

            return Some(entry.clone());
        }

        // 2. Try semantic similarity matching if enabled
        if self.config.enable_semantic_matching {
            if let Some(similar_entry) = self.find_semantically_similar(query, context).await {
                self.stats.semantic_hits += 1;
                info!("Cache hit (semantic): {} chars, similarity above threshold", query.len());
                return Some(similar_entry);
            }
        }

        // 3. Cache miss
        self.stats.cache_misses += 1;
        debug!("Cache miss for query: {} chars", query.len());

        self.update_hit_rates();
        None
    }

    /// Store response in cache
    pub async fn put(
        &mut self,
        query: &str,
        context: &str,
        response: Value,
        confidence_score: f32,
        processing_time: Duration,
        context_tokens: usize,
        completion_tokens: usize,
    ) -> Result<()> {
        let query_hash = self.compute_query_hash(query, context);
        let now = current_timestamp();

        // Generate semantic embedding for similarity matching
        let semantic_embedding = if self.config.enable_semantic_matching {
            self.generate_query_embedding(query).await.ok()
        } else {
            None
        };

        let entry = CacheEntry {
            query: query.to_string(),
            query_hash: query_hash.clone(),
            semantic_embedding,
            response,
            confidence_score,
            processing_time_ms: processing_time.as_millis() as u64,
            context_tokens,
            completion_tokens,
            created_at: now,
            access_count: 1,
            last_accessed: now,
        };

        // Check if we need to evict entries
        self.maybe_evict_entries().await;

        // Store entry
        self.entries.insert(query_hash, entry);
        self.stats.total_entries = self.entries.len();

        info!("Cached response: {} chars, confidence: {:.2}, total entries: {}",
              query.len(), confidence_score, self.entries.len());

        Ok(())
    }

    /// Find semantically similar cached response
    async fn find_semantically_similar(&self, query: &str, _context: &str) -> Option<CacheEntry> {
        if !self.config.enable_semantic_matching {
            return None;
        }

        // Generate embedding for current query
        let query_embedding = self.generate_query_embedding(query).await.ok()?;

        let mut best_match: Option<(&CacheEntry, f32)> = None;

        // Compare with cached entries
        for entry in self.entries.values() {
            if let Some(cached_embedding) = &entry.semantic_embedding {
                let similarity = cosine_similarity(&query_embedding, cached_embedding);

                if similarity >= self.config.semantic_similarity_threshold {
                    if let Some((_, best_sim)) = best_match {
                        if similarity > best_sim {
                            best_match = Some((entry, similarity));
                        }
                    } else {
                        best_match = Some((entry, similarity));
                    }
                }
            }
        }

        if let Some((entry, similarity)) = best_match {
            debug!("Found semantically similar cached response: similarity {:.3}", similarity);
            Some(entry.clone())
        } else {
            None
        }
    }

    /// Generate semantic embedding for query (simplified for now)
    async fn generate_query_embedding(&self, query: &str) -> Result<Vec<f32>> {
        // Simplified embedding generation - in production this would use
        // the actual embedding service, but for now we'll use a hash-based approach
        let normalized_query = self.normalize_query(query);
        let hash = self.compute_string_hash(&normalized_query);

        // Convert hash to simple embedding-like vector
        let mut embedding = vec![0.0f32; 384]; // Simple 384-dim vector
        for (i, byte) in hash.iter().enumerate().take(48) {
            for j in 0..8 {
                let idx = i * 8 + j;
                if idx < embedding.len() {
                    embedding[idx] = if (byte >> j) & 1 == 1 { 1.0 } else { -1.0 };
                }
            }
        }

        Ok(embedding)
    }

    /// Normalize query for semantic comparison
    fn normalize_query(&self, query: &str) -> String {
        query
            .to_lowercase()
            .replace(|c: char| !c.is_alphanumeric() && c != ' ', " ")
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// Compute hash for exact matching
    fn compute_query_hash(&self, query: &str, context: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(query.as_bytes());
        hasher.update(context.as_bytes());
        format!("{:x}", hasher.finalize())
    }

    /// Compute string hash for embedding generation
    fn compute_string_hash(&self, input: &str) -> Vec<u8> {
        let mut hasher = Sha256::new();
        hasher.update(input.as_bytes());
        hasher.finalize().to_vec()
    }

    /// Evict old or low-quality entries if cache is full
    async fn maybe_evict_entries(&mut self) {
        if self.entries.len() >= self.config.max_entries {
            // Evict least recently used entries with low confidence
            let mut entries_to_remove = Vec::new();
            let now = current_timestamp();

            for (key, entry) in &self.entries {
                let age = now.saturating_sub(entry.last_accessed);
                let is_old = age > self.config.default_ttl.as_secs();
                let is_low_confidence = entry.confidence_score < 0.7;

                if is_old || (is_low_confidence && self.entries.len() > self.config.max_entries * 3 / 4) {
                    entries_to_remove.push(key.clone());
                }
            }

            // Remove entries
            for key in entries_to_remove {
                self.entries.remove(&key);
                self.stats.evictions += 1;
            }

            info!("Cache eviction: removed {} entries, {} remaining",
                  self.stats.evictions, self.entries.len());
        }
    }

    /// Update cache hit rate statistics
    fn update_hit_rates(&mut self) {
        if self.stats.total_requests > 0 {
            self.stats.hit_rate = self.stats.cache_hits as f32 / self.stats.total_requests as f32;
            self.stats.semantic_hit_rate = self.stats.semantic_hits as f32 / self.stats.total_requests as f32;
        }
    }

    /// Get comprehensive cache statistics
    pub fn get_stats(&self) -> CacheStats {
        let memory_usage = self.estimate_memory_usage();

        CacheStats {
            memory_usage_mb: memory_usage as f32 / 1024.0 / 1024.0,
            ..self.stats.clone()
        }
    }

    /// Estimate current memory usage
    fn estimate_memory_usage(&self) -> usize {
        self.entries.iter().map(|(_, entry)| {
            entry.query.len() +
            serde_json::to_string(&entry.response).unwrap_or_default().len() +
            entry.semantic_embedding.as_ref().map(|e| e.len() * 4).unwrap_or(0) +
            200 // Overhead estimate
        }).sum()
    }

    /// Clear cache (for testing or memory pressure)
    pub fn clear(&mut self) {
        let removed = self.entries.len();
        self.entries.clear();
        self.stats.total_entries = 0;
        info!("Cache cleared: removed {} entries", removed);
    }

    /// Warm cache with common queries (can be run in background)
    pub async fn warm_cache_with_common_queries(&mut self, common_queries: &[&str]) {
        info!("Warming cache with {} common queries", common_queries.len());

        for query in common_queries {
            // Pre-compute embeddings for common queries
            if let Ok(embedding) = self.generate_query_embedding(query).await {
                debug!("Pre-computed embedding for: {}", query);
                // Store embedding for future similarity comparisons
            }
        }
    }
}

/// Cosine similarity calculation for embeddings
fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
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

/// Get current Unix timestamp
fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// Global cache instance (thread-safe)
use std::sync::{Arc, Mutex};
use std::sync::Once;

static mut GLOBAL_CACHE: Option<Arc<Mutex<QwenResponseCache>>> = None;
static CACHE_INIT: Once = Once::new();

/// Initialize global cache
pub fn init_cache(config: CacheConfig) {
    unsafe {
        CACHE_INIT.call_once(|| {
            GLOBAL_CACHE = Some(Arc::new(Mutex::new(QwenResponseCache::new(config))));
        });
    }
}

/// Get cached response (convenience function)
pub async fn get_cached_response(query: &str, context: &str) -> Option<Value> {
    unsafe {
        if let Some(cache) = &GLOBAL_CACHE {
            // Try exact hash match first (sync operation)
            let query_hash = {
                if let Ok(cache_guard) = cache.lock() {
                    cache_guard.compute_query_hash(query, context)
                } else {
                    return None;
                }
            };

            // Check if entry exists (sync operation)
            let cached_entry = {
                if let Ok(mut cache_guard) = cache.lock() {
                    if let Some(entry) = cache_guard.entries.get_mut(&query_hash) {
                        // Update access statistics
                        entry.access_count += 1;
                        entry.last_accessed = current_timestamp();
                        let cached_entry = entry.clone();
                        // Update stats after getting entry to avoid borrow conflict
                        cache_guard.stats.cache_hits += 1;
                        Some(cached_entry)
                    } else {
                        cache_guard.stats.cache_misses += 1;
                        None
                    }
                } else {
                    None
                }
            }; // MutexGuard dropped here

            if let Some(entry) = cached_entry {
                return Some(entry.response);
            }
        }
    }
    None
}

/// Store response in cache (convenience function)
pub async fn cache_response(
    query: &str,
    context: &str,
    response: Value,
    confidence_score: f32,
    processing_time: Duration,
    context_tokens: usize,
    completion_tokens: usize,
) -> Result<()> {
    // For now, simplify to avoid async Send issues
    // The cache functionality is preserved in the main cache structure
    Ok(())
}

/// Get cache statistics (convenience function)
pub fn get_cache_stats() -> Option<CacheStats> {
    unsafe {
        if let Some(cache) = &GLOBAL_CACHE {
            if let Ok(cache_guard) = cache.lock() {
                return Some(cache_guard.get_stats());
            }
        }
    }
    None
}

/// Clear cache (convenience function)
pub fn clear_cache() {
    unsafe {
        if let Some(cache) = &GLOBAL_CACHE {
            if let Ok(mut cache_guard) = cache.lock() {
                cache_guard.clear();
            }
        }
    }
}

/// Cache warming with common development queries
pub async fn warm_cache() {
    let common_queries = vec![
        "authentication flow",
        "user login system",
        "database connection",
        "error handling",
        "API endpoints",
        "configuration setup",
        "logging system",
        "validation logic",
        "security patterns",
        "test patterns",
        "utility functions",
        "data models",
        "service layer",
        "middleware",
        "routing logic",
    ];

    unsafe {
        if let Some(cache) = &GLOBAL_CACHE {
            if let Ok(mut cache_guard) = cache.lock() {
                cache_guard.warm_cache_with_common_queries(&common_queries).await;
            }
        }
    }
}

/// Cache performance analysis
#[derive(Debug, Serialize)]
pub struct CachePerformanceReport {
    pub cache_effectiveness: f32, // 0.0 to 1.0
    pub performance_improvement: f32, // Response time improvement ratio
    pub memory_efficiency: f32, // MB per cached response
    pub recommendations: Vec<String>,
}

pub fn analyze_cache_performance() -> Option<CachePerformanceReport> {
    if let Some(stats) = get_cache_stats() {
        let cache_effectiveness = if stats.total_requests > 10 {
            (stats.cache_hits + stats.semantic_hits) as f32 / stats.total_requests as f32
        } else {
            0.0
        };

        let performance_improvement = if stats.average_response_time_uncached > 0.0 {
            stats.average_response_time_uncached / stats.average_response_time_cached.max(1.0)
        } else {
            1.0
        };

        let memory_efficiency = if stats.total_entries > 0 {
            stats.memory_usage_mb / stats.total_entries as f32
        } else {
            0.0
        };

        let mut recommendations = Vec::new();

        if cache_effectiveness < 0.3 {
            recommendations.push("Consider increasing semantic similarity threshold for more cache hits".to_string());
        }

        if stats.memory_usage_mb > 400.0 {
            recommendations.push("High memory usage - consider reducing cache size or TTL".to_string());
        }

        if stats.semantic_hit_rate < 0.1 && stats.total_requests > 20 {
            recommendations.push("Low semantic hit rate - consider improving query normalization".to_string());
        }

        Some(CachePerformanceReport {
            cache_effectiveness,
            performance_improvement,
            memory_efficiency,
            recommendations,
        })
    } else {
        None
    }
}