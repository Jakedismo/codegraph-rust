// ABOUTME: Collects cache performance metrics and produces aggregate reports.
// ABOUTME: Provides throughput tracking and human-readable recommendations from collected stats.
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};
use tokio::sync::RwLock as AsyncRwLock;
use tracing::{debug, info, warn};

/// Comprehensive cache metrics for performance monitoring
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheMetrics {
    /// Total number of cache hits
    pub hits: u64,
    /// Total number of cache misses
    pub misses: u64,
    /// Total number of cache insertions
    pub insertions: u64,
    /// Total number of cache evictions
    pub evictions: u64,
    /// Total number of cache removals
    pub removals: u64,
    /// Total memory usage in bytes
    pub memory_usage: u64,
    /// Peak memory usage in bytes
    pub peak_memory_usage: u64,
    /// Average query response time in microseconds
    pub avg_response_time_us: u64,
    /// Hit rate as percentage (0-100)
    pub hit_rate: f64,
    /// Total size of cached items
    pub total_size: usize,
    /// Number of expired entries cleaned up
    pub expired_cleanup_count: u64,
    /// Last reset timestamp
    pub last_reset: SystemTime,
}

impl Default for CacheMetrics {
    fn default() -> Self {
        Self {
            hits: 0,
            misses: 0,
            insertions: 0,
            evictions: 0,
            removals: 0,
            memory_usage: 0,
            peak_memory_usage: 0,
            avg_response_time_us: 0,
            hit_rate: 0.0,
            total_size: 0,
            expired_cleanup_count: 0,
            last_reset: SystemTime::now(),
        }
    }
}

impl CacheMetrics {
    /// Calculate hit rate percentage
    pub fn calculate_hit_rate(&mut self) {
        let total_requests = self.hits + self.misses;
        if total_requests > 0 {
            self.hit_rate = (self.hits as f64 / total_requests as f64) * 100.0;
        } else {
            self.hit_rate = 0.0;
        }
    }

    /// Reset all metrics to zero
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// Get efficiency score based on hit rate and memory usage
    pub fn efficiency_score(&self) -> f64 {
        // Combine hit rate (70% weight) with memory efficiency (30% weight)
        let hit_rate_score = self.hit_rate / 100.0;
        let memory_efficiency = if self.peak_memory_usage > 0 {
            1.0 - (self.memory_usage as f64 / self.peak_memory_usage as f64).min(1.0)
        } else {
            1.0
        };

        (hit_rate_score * 0.7) + (memory_efficiency * 0.3)
    }
}

/// Real-time metrics collector with atomic operations
pub struct MetricsCollector {
    /// Atomic counters for thread-safe operations
    hits: AtomicU64,
    misses: AtomicU64,
    insertions: AtomicU64,
    evictions: AtomicU64,
    removals: AtomicU64,
    memory_usage: AtomicU64,
    peak_memory_usage: AtomicU64,
    total_size: AtomicUsize,
    expired_cleanup_count: AtomicU64,

    /// Response time tracking
    response_times: Arc<AsyncRwLock<Vec<u64>>>,

    /// Detailed operation metrics
    operation_metrics: Arc<AsyncRwLock<HashMap<String, OperationMetrics>>>,

    /// Metrics start time for rate calculations
    start_time: Instant,

    /// Configuration for metrics collection
    config: MetricsConfig,
}

/// Configuration for metrics collection behavior
#[derive(Debug, Clone)]
pub struct MetricsConfig {
    /// Maximum number of response times to keep for averaging
    pub max_response_time_samples: usize,
    /// Enable detailed operation metrics tracking
    pub enable_operation_metrics: bool,
    /// Interval for automatic metrics reporting
    pub reporting_interval: Duration,
    /// Enable memory pressure alerts
    pub enable_memory_alerts: bool,
    /// Memory threshold for alerts (bytes)
    pub memory_alert_threshold: u64,
}

impl Default for MetricsConfig {
    fn default() -> Self {
        Self {
            max_response_time_samples: 1000,
            enable_operation_metrics: true,
            reporting_interval: Duration::from_secs(60),
            enable_memory_alerts: true,
            memory_alert_threshold: 1_000_000_000, // 1GB
        }
    }
}

/// Detailed metrics for specific operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperationMetrics {
    pub count: u64,
    pub total_duration_us: u64,
    pub avg_duration_us: u64,
    pub min_duration_us: u64,
    pub max_duration_us: u64,
    pub error_count: u64,
}

impl Default for OperationMetrics {
    fn default() -> Self {
        Self {
            count: 0,
            total_duration_us: 0,
            avg_duration_us: 0,
            min_duration_us: u64::MAX,
            max_duration_us: 0,
            error_count: 0,
        }
    }
}

impl OperationMetrics {
    /// Add a new duration measurement
    pub fn add_duration(&mut self, duration_us: u64) {
        self.count += 1;
        self.total_duration_us += duration_us;
        self.avg_duration_us = self.total_duration_us / self.count;
        self.min_duration_us = self.min_duration_us.min(duration_us);
        self.max_duration_us = self.max_duration_us.max(duration_us);
    }

    /// Record an error for this operation
    pub fn record_error(&mut self) {
        self.error_count += 1;
    }
}

impl MetricsCollector {
    /// Create a new metrics collector with default configuration
    pub fn new() -> Self {
        Self::with_config(MetricsConfig::default())
    }

    /// Create a new metrics collector with custom configuration
    pub fn with_config(config: MetricsConfig) -> Self {
        Self {
            hits: AtomicU64::new(0),
            misses: AtomicU64::new(0),
            insertions: AtomicU64::new(0),
            evictions: AtomicU64::new(0),
            removals: AtomicU64::new(0),
            memory_usage: AtomicU64::new(0),
            peak_memory_usage: AtomicU64::new(0),
            total_size: AtomicUsize::new(0),
            expired_cleanup_count: AtomicU64::new(0),
            response_times: Arc::new(AsyncRwLock::new(Vec::new())),
            operation_metrics: Arc::new(AsyncRwLock::new(HashMap::new())),
            start_time: Instant::now(),
            config,
        }
    }

    /// Record a cache hit
    pub fn record_hit(&self) {
        self.hits.fetch_add(1, Ordering::Relaxed);
        debug!("Cache hit recorded");
    }

    /// Record a cache miss
    pub fn record_miss(&self) {
        self.misses.fetch_add(1, Ordering::Relaxed);
        debug!("Cache miss recorded");
    }

    /// Record a cache insertion
    pub fn record_insertion(&self, size_bytes: usize) {
        self.insertions.fetch_add(1, Ordering::Relaxed);
        self.total_size.fetch_add(size_bytes, Ordering::Relaxed);
        self.add_memory_usage(size_bytes as u64);
        debug!("Cache insertion recorded: {} bytes", size_bytes);
    }

    /// Record a cache eviction
    pub fn record_eviction(&self, size_bytes: usize) {
        self.evictions.fetch_add(1, Ordering::Relaxed);
        self.total_size.fetch_sub(size_bytes, Ordering::Relaxed);
        self.subtract_memory_usage(size_bytes as u64);
        debug!("Cache eviction recorded: {} bytes", size_bytes);
    }

    /// Record a cache removal
    pub fn record_removal(&self, size_bytes: usize) {
        self.removals.fetch_add(1, Ordering::Relaxed);
        self.total_size.fetch_sub(size_bytes, Ordering::Relaxed);
        self.subtract_memory_usage(size_bytes as u64);
        debug!("Cache removal recorded: {} bytes", size_bytes);
    }

    /// Record expired entry cleanup
    pub fn record_expired_cleanup(&self, count: u64) {
        self.expired_cleanup_count
            .fetch_add(count, Ordering::Relaxed);
        debug!("Expired cleanup recorded: {} entries", count);
    }

    /// Add memory usage and update peak
    pub fn add_memory_usage(&self, bytes: u64) {
        let new_usage = self.memory_usage.fetch_add(bytes, Ordering::Relaxed) + bytes;

        // Update peak memory usage
        let mut peak = self.peak_memory_usage.load(Ordering::Relaxed);
        while new_usage > peak {
            match self.peak_memory_usage.compare_exchange_weak(
                peak,
                new_usage,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(current) => peak = current,
            }
        }

        // Check for memory alerts
        if self.config.enable_memory_alerts && new_usage > self.config.memory_alert_threshold {
            warn!(
                "Memory usage alert: {} bytes exceeds threshold {}",
                new_usage, self.config.memory_alert_threshold
            );
        }
    }

    /// Subtract memory usage
    pub fn subtract_memory_usage(&self, bytes: u64) {
        self.memory_usage.fetch_sub(bytes, Ordering::Relaxed);
    }

    /// Record response time for averaging
    pub async fn record_response_time(&self, duration: Duration) {
        let duration_us = duration.as_micros() as u64;
        let mut times = self.response_times.write().await;

        times.push(duration_us);

        // Keep only the most recent samples
        if times.len() > self.config.max_response_time_samples {
            times.remove(0);
        }
    }

    /// Record operation metrics
    pub async fn record_operation(&self, operation: &str, duration: Duration, success: bool) {
        if !self.config.enable_operation_metrics {
            return;
        }

        let duration_us = duration.as_micros() as u64;
        let mut metrics = self.operation_metrics.write().await;

        let op_metrics = metrics.entry(operation.to_string()).or_default();
        op_metrics.add_duration(duration_us);

        if !success {
            op_metrics.record_error();
        }
    }

    /// Get current metrics snapshot
    pub async fn get_metrics(&self) -> CacheMetrics {
        let hits = self.hits.load(Ordering::Relaxed);
        let misses = self.misses.load(Ordering::Relaxed);
        let total_requests = hits + misses;

        let hit_rate = if total_requests > 0 {
            (hits as f64 / total_requests as f64) * 100.0
        } else {
            0.0
        };

        // Calculate average response time
        let response_times = self.response_times.read().await;
        let avg_response_time_us = if !response_times.is_empty() {
            response_times.iter().sum::<u64>() / response_times.len() as u64
        } else {
            0
        };

        CacheMetrics {
            hits,
            misses,
            insertions: self.insertions.load(Ordering::Relaxed),
            evictions: self.evictions.load(Ordering::Relaxed),
            removals: self.removals.load(Ordering::Relaxed),
            memory_usage: self.memory_usage.load(Ordering::Relaxed),
            peak_memory_usage: self.peak_memory_usage.load(Ordering::Relaxed),
            avg_response_time_us,
            hit_rate,
            total_size: self.total_size.load(Ordering::Relaxed),
            expired_cleanup_count: self.expired_cleanup_count.load(Ordering::Relaxed),
            last_reset: SystemTime::now(),
        }
    }

    /// Get operation metrics
    pub async fn get_operation_metrics(&self) -> HashMap<String, OperationMetrics> {
        self.operation_metrics.read().await.clone()
    }

    /// Reset all metrics
    pub async fn reset(&self) {
        self.hits.store(0, Ordering::Relaxed);
        self.misses.store(0, Ordering::Relaxed);
        self.insertions.store(0, Ordering::Relaxed);
        self.evictions.store(0, Ordering::Relaxed);
        self.removals.store(0, Ordering::Relaxed);
        self.memory_usage.store(0, Ordering::Relaxed);
        self.peak_memory_usage.store(0, Ordering::Relaxed);
        self.total_size.store(0, Ordering::Relaxed);
        self.expired_cleanup_count.store(0, Ordering::Relaxed);

        self.response_times.write().await.clear();
        self.operation_metrics.write().await.clear();

        info!("Cache metrics reset");
    }

    /// Get throughput metrics (operations per second)
    pub fn get_throughput_metrics(&self) -> ThroughputMetrics {
        let elapsed = self.start_time.elapsed();
        let elapsed_seconds = elapsed.as_secs_f64();

        if elapsed_seconds > 0.0 {
            ThroughputMetrics {
                hits_per_second: self.hits.load(Ordering::Relaxed) as f64 / elapsed_seconds,
                misses_per_second: self.misses.load(Ordering::Relaxed) as f64 / elapsed_seconds,
                insertions_per_second: self.insertions.load(Ordering::Relaxed) as f64
                    / elapsed_seconds,
                evictions_per_second: self.evictions.load(Ordering::Relaxed) as f64
                    / elapsed_seconds,
                total_ops_per_second: (self.hits.load(Ordering::Relaxed)
                    + self.misses.load(Ordering::Relaxed)
                    + self.insertions.load(Ordering::Relaxed))
                    as f64
                    / elapsed_seconds,
            }
        } else {
            ThroughputMetrics::default()
        }
    }

    /// Generate performance report
    pub async fn generate_report(&self) -> PerformanceReport {
        let metrics = self.get_metrics().await;
        let throughput = self.get_throughput_metrics();
        let operation_metrics = self.get_operation_metrics().await;
        let recommendations = self.generate_recommendations(&metrics).await;

        PerformanceReport {
            timestamp: SystemTime::now(),
            cache_metrics: metrics,
            throughput_metrics: throughput,
            operation_metrics,
            uptime: self.start_time.elapsed(),
            recommendations,
        }
    }

    /// Generate performance recommendations based on current metrics
    async fn generate_recommendations(&self, metrics: &CacheMetrics) -> Vec<String> {
        let mut recommendations = Vec::new();

        // Hit rate recommendations
        if metrics.hit_rate < 50.0 {
            recommendations.push(
                "Low hit rate detected. Consider increasing cache size or adjusting TTL values."
                    .to_string(),
            );
        } else if metrics.hit_rate > 95.0 {
            recommendations.push(
                "Very high hit rate. Consider reducing cache size to free up memory.".to_string(),
            );
        }

        // Memory usage recommendations
        let memory_usage_mb = metrics.memory_usage as f64 / (1024.0 * 1024.0);
        if memory_usage_mb > 1000.0 {
            recommendations.push(format!("High memory usage: {:.1} MB. Consider enabling compression or reducing cache size.", memory_usage_mb));
        }

        // Response time recommendations
        if metrics.avg_response_time_us > 10_000 {
            recommendations.push(
                "High average response time detected. Consider optimizing cache lookup operations."
                    .to_string(),
            );
        }

        // Eviction rate recommendations
        let total_requests = metrics.hits + metrics.misses;
        if total_requests > 0 {
            let eviction_rate = metrics.evictions as f64 / total_requests as f64;
            if eviction_rate > 0.1 {
                recommendations.push("High eviction rate detected. Consider increasing cache size or adjusting eviction policy.".to_string());
            }
        }

        if recommendations.is_empty() {
            recommendations.push("Cache performance is within expected bounds.".to_string());
        }

        recommendations
    }
}

/// Throughput metrics for performance analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThroughputMetrics {
    pub hits_per_second: f64,
    pub misses_per_second: f64,
    pub insertions_per_second: f64,
    pub evictions_per_second: f64,
    pub total_ops_per_second: f64,
}

impl Default for ThroughputMetrics {
    fn default() -> Self {
        Self {
            hits_per_second: 0.0,
            misses_per_second: 0.0,
            insertions_per_second: 0.0,
            evictions_per_second: 0.0,
            total_ops_per_second: 0.0,
        }
    }
}

/// Comprehensive performance report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceReport {
    pub timestamp: SystemTime,
    pub cache_metrics: CacheMetrics,
    pub throughput_metrics: ThroughputMetrics,
    pub operation_metrics: HashMap<String, OperationMetrics>,
    pub uptime: Duration,
    pub recommendations: Vec<String>,
}

impl PerformanceReport {
    /// Export report as JSON string
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// Save report to file
    pub async fn save_to_file(&self, path: &str) -> Result<(), Box<dyn std::error::Error>> {
        let json = self.to_json()?;
        tokio::fs::write(path, json).await?;
        Ok(())
    }
}

/// Metrics aggregator for combining metrics from multiple cache instances
pub struct MetricsAggregator {
    collectors: Vec<Arc<MetricsCollector>>,
}

impl MetricsAggregator {
    pub fn new() -> Self {
        Self {
            collectors: Vec::new(),
        }
    }

    /// Add a metrics collector to the aggregator
    pub fn add_collector(&mut self, collector: Arc<MetricsCollector>) {
        self.collectors.push(collector);
    }

    /// Get aggregated metrics from all collectors
    pub async fn get_aggregated_metrics(&self) -> CacheMetrics {
        let mut aggregated = CacheMetrics::default();

        for collector in &self.collectors {
            let metrics = collector.get_metrics().await;
            aggregated.hits += metrics.hits;
            aggregated.misses += metrics.misses;
            aggregated.insertions += metrics.insertions;
            aggregated.evictions += metrics.evictions;
            aggregated.removals += metrics.removals;
            aggregated.memory_usage += metrics.memory_usage;
            aggregated.peak_memory_usage =
                aggregated.peak_memory_usage.max(metrics.peak_memory_usage);
            aggregated.total_size += metrics.total_size;
            aggregated.expired_cleanup_count += metrics.expired_cleanup_count;
        }

        aggregated.calculate_hit_rate();
        aggregated
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::{sleep, Duration};

    #[tokio::test]
    async fn test_metrics_collection() {
        let collector = MetricsCollector::new();

        // Record some operations
        collector.record_hit();
        collector.record_miss();
        collector.record_insertion(1024);
        collector.record_eviction(512);

        let metrics = collector.get_metrics().await;

        assert_eq!(metrics.hits, 1);
        assert_eq!(metrics.misses, 1);
        assert_eq!(metrics.insertions, 1);
        assert_eq!(metrics.evictions, 1);
        assert_eq!(metrics.hit_rate, 50.0);
        assert_eq!(metrics.memory_usage, 512); // 1024 - 512
    }

    #[tokio::test]
    async fn test_response_time_tracking() {
        let collector = MetricsCollector::new();

        collector
            .record_response_time(Duration::from_millis(10))
            .await;
        collector
            .record_response_time(Duration::from_millis(20))
            .await;
        collector
            .record_response_time(Duration::from_millis(30))
            .await;

        let metrics = collector.get_metrics().await;
        assert_eq!(metrics.avg_response_time_us, 20_000); // Average of 10, 20, 30 ms
    }

    #[tokio::test]
    async fn test_operation_metrics() {
        let collector = MetricsCollector::new();

        collector
            .record_operation("get", Duration::from_millis(5), true)
            .await;
        collector
            .record_operation("get", Duration::from_millis(10), true)
            .await;
        collector
            .record_operation("get", Duration::from_millis(15), false)
            .await;

        let op_metrics = collector.get_operation_metrics().await;
        let get_metrics = op_metrics.get("get").unwrap();

        assert_eq!(get_metrics.count, 3);
        assert_eq!(get_metrics.error_count, 1);
        assert_eq!(get_metrics.avg_duration_us, 10_000); // Average of 5, 10, 15 ms
    }

    #[tokio::test]
    async fn test_throughput_metrics() {
        let collector = MetricsCollector::new();

        // Wait a bit to ensure elapsed time > 0
        sleep(Duration::from_millis(100)).await;

        collector.record_hit();
        collector.record_hit();
        collector.record_miss();

        let throughput = collector.get_throughput_metrics();
        assert!(throughput.total_ops_per_second > 0.0);
        assert!(throughput.hits_per_second > 0.0);
    }

    #[tokio::test]
    async fn test_metrics_reset() {
        let collector = MetricsCollector::new();

        collector.record_hit();
        collector.record_insertion(1024);

        let metrics_before = collector.get_metrics().await;
        assert!(metrics_before.hits > 0);
        assert!(metrics_before.memory_usage > 0);

        collector.reset().await;

        let metrics_after = collector.get_metrics().await;
        assert_eq!(metrics_after.hits, 0);
        assert_eq!(metrics_after.memory_usage, 0);
    }

    #[tokio::test]
    async fn test_performance_report() {
        let collector = MetricsCollector::new();

        sleep(Duration::from_millis(1)).await;

        collector.record_hit();
        collector.record_miss();
        collector.record_insertion(1024);

        let report = collector.generate_report().await;

        assert!(report.cache_metrics.hits > 0);
        assert!(!report.recommendations.is_empty());
        assert!(report.uptime.as_millis() > 0);
    }

    #[tokio::test]
    async fn test_metrics_aggregator() {
        let mut aggregator = MetricsAggregator::new();

        let collector1 = Arc::new(MetricsCollector::new());
        let collector2 = Arc::new(MetricsCollector::new());

        collector1.record_hit();
        collector1.record_insertion(1024);

        collector2.record_hit();
        collector2.record_hit();
        collector2.record_insertion(512);

        aggregator.add_collector(collector1);
        aggregator.add_collector(collector2);

        let aggregated = aggregator.get_aggregated_metrics().await;

        assert_eq!(aggregated.hits, 3);
        assert_eq!(aggregated.insertions, 2);
        assert_eq!(aggregated.memory_usage, 1536); // 1024 + 512
    }
}
