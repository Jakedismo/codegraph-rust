use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use dashmap::DashMap;
use crate::{ReadAheadOptimizer, ReadAheadConfig, ReadAheadMetrics};
use codegraph_core::{CodeGraphError, CompactCacheKey, CacheType, Result};

/// Integration layer that demonstrates read-ahead optimization in action
pub struct ReadAheadIntegration {
    optimizer: Arc<ReadAheadOptimizer>,
    cache_storage: Arc<DashMap<CompactCacheKey, Vec<u8>>>,
    performance_monitor: Arc<RwLock<PerformanceMonitor>>,
}

#[derive(Debug, Default)]
struct PerformanceMonitor {
    total_requests: u64,
    cache_hits: u64,
    prefetch_hits: u64,
    total_bytes_served: u64,
    average_response_time_ms: f64,
}

impl ReadAheadIntegration {
    pub fn new() -> Self {
        let config = ReadAheadConfig {
            max_pattern_history: 5000,
            prediction_window_size: 30,
            sequential_threshold: 3,
            cache_warming_interval: Duration::from_secs(30),
            prefetch_depth: 15,
            pattern_decay_factor: 0.9,
            min_confidence_threshold: 0.6,
            adaptive_learning_rate: 0.15,
        };

        Self {
            optimizer: Arc::new(ReadAheadOptimizer::new(config)),
            cache_storage: Arc::new(DashMap::new()),
            performance_monitor: Arc::new(RwLock::new(PerformanceMonitor::default())),
        }
    }

    /// Main entry point for optimized data access
    pub async fn get_data(&self, key: CompactCacheKey) -> Result<Vec<u8>> {
        let start_time = std::time::Instant::now();

        // 1. Check primary cache first
        if let Some(data) = self.cache_storage.get(&key) {
            self.record_cache_hit(start_time.elapsed()).await;
            return Ok(data.clone());
        }

        // 2. Apply read-ahead optimization
        if let Some(optimized_data) = self.optimizer.optimize_read(key).await? {
            // Store in cache for future access
            self.cache_storage.insert(key, optimized_data.clone());
            
            // Trigger background prefetching for related data
            self.background_prefetch(key).await;
            
            self.record_optimization_hit(start_time.elapsed()).await;
            return Ok(optimized_data);
        }

        // 3. Fallback to regular data loading
        let data = self.load_data_from_storage(key).await?;
        self.cache_storage.insert(key, data.clone());
        
        self.record_regular_access(start_time.elapsed()).await;
        Ok(data)
    }

    /// Demonstrates sequential access pattern optimization
    pub async fn demonstrate_sequential_access(&self) -> Result<Vec<Vec<u8>>> {
        println!("🚀 Demonstrating Sequential Access Pattern Optimization");
        
        let mut results = Vec::new();
        let base_hash = 1000;

        // Simulate sequential access pattern
        for i in 0..20 {
            let key = CompactCacheKey {
                hash: base_hash + i,
                cache_type: CacheType::Node,
            };
            
            let data = self.get_data(key).await?;
            results.push(data);
            
            // Small delay to simulate real-world access patterns
            tokio::time::sleep(Duration::from_millis(10)).await;
        }

        self.print_performance_summary().await;
        Ok(results)
    }

    /// Demonstrates predictive loading based on access patterns
    pub async fn demonstrate_predictive_loading(&self) -> Result<()> {
        println!("🧠 Demonstrating Predictive Loading");

        // Simulate a common access pattern: Node -> Embedding -> Query
        let patterns = vec![
            (CacheType::Node, 2000),
            (CacheType::Embedding, 2001),
            (CacheType::Query, 2002),
            (CacheType::Node, 3000),
            (CacheType::Embedding, 3001),
            (CacheType::Query, 3002),
            (CacheType::Node, 4000),
            (CacheType::Embedding, 4001),
            (CacheType::Query, 4002),
        ];

        // First, train the pattern
        println!("Training access patterns...");
        for (cache_type, hash) in &patterns {
            let key = CompactCacheKey {
                hash: *hash,
                cache_type: *cache_type,
            };
            let _ = self.get_data(key).await?;
            tokio::time::sleep(Duration::from_millis(50)).await;
        }

        // Now test prediction accuracy
        println!("Testing prediction accuracy...");
        let test_key = CompactCacheKey {
            hash: 5000,
            cache_type: CacheType::Node,
        };
        
        let _ = self.get_data(test_key).await?;

        // The optimizer should predict the next accesses (Embedding and Query)
        let embedding_key = CompactCacheKey {
            hash: 5001,
            cache_type: CacheType::Embedding,
        };
        let query_key = CompactCacheKey {
            hash: 5002,
            cache_type: CacheType::Query,
        };

        // These should be cache hits due to prediction
        let start = std::time::Instant::now();
        let _ = self.get_data(embedding_key).await?;
        let _ = self.get_data(query_key).await?;
        
        println!("Predicted access time: {:?}", start.elapsed());
        self.print_optimization_metrics().await;
        
        Ok(())
    }

    /// Demonstrates cache warming for hot data
    pub async fn demonstrate_cache_warming(&self) -> Result<()> {
        println!("🔥 Demonstrating Cache Warming");

        // Start cache warming background process
        self.optimizer.start_cache_warming().await?;

        // Simulate hot data access patterns
        let hot_keys = vec![
            CompactCacheKey { hash: 100, cache_type: CacheType::Node },
            CompactCacheKey { hash: 101, cache_type: CacheType::Embedding },
            CompactCacheKey { hash: 102, cache_type: CacheType::Query },
        ];

        // Access hot keys multiple times to establish patterns
        for _ in 0..10 {
            for &key in &hot_keys {
                let _ = self.get_data(key).await?;
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        }

        // Wait for cache warming to kick in
        tokio::time::sleep(Duration::from_secs(2)).await;

        // Now access should be very fast due to warming
        let start = std::time::Instant::now();
        for &key in &hot_keys {
            let _ = self.get_data(key).await?;
        }
        println!("Warmed cache access time: {:?}", start.elapsed());

        Ok(())
    }

    /// Background prefetching based on access patterns
    async fn background_prefetch(&self, key: CompactCacheKey) {
        let optimizer = Arc::clone(&self.optimizer);
        let cache_storage = Arc::clone(&self.cache_storage);

        tokio::spawn(async move {
            // Simulate intelligent prefetching
            let predicted_keys = vec![
                CompactCacheKey { hash: key.hash + 1, cache_type: key.cache_type },
                CompactCacheKey { hash: key.hash + 2, cache_type: key.cache_type },
                CompactCacheKey { hash: key.hash + 10, cache_type: key.cache_type },
            ];

            for predicted_key in predicted_keys {
                if !cache_storage.contains_key(&predicted_key) {
                    // Simulate data loading
                    let data = format!("prefetched_data_{}", predicted_key.hash).into_bytes();
                    cache_storage.insert(predicted_key, data);
                }
                
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        });
    }

    /// Simulate loading data from persistent storage
    async fn load_data_from_storage(&self, key: CompactCacheKey) -> Result<Vec<u8>> {
        // Simulate I/O delay
        tokio::time::sleep(Duration::from_millis(100)).await;
        
        // Generate synthetic data based on cache type
        let data = match key.cache_type {
            CacheType::Node => format!("node_data_{}", key.hash),
            CacheType::Embedding => format!("embedding_vector_{}", key.hash),
            CacheType::Query => format!("query_result_{}", key.hash),
            CacheType::Metadata => format!("metadata_{}", key.hash),
            CacheType::Path => format!("path_info_{}", key.hash),
        };

        Ok(data.into_bytes())
    }

    async fn record_cache_hit(&self, duration: Duration) {
        let mut monitor = self.performance_monitor.write().await;
        monitor.total_requests += 1;
        monitor.cache_hits += 1;
        self.update_response_time(&mut monitor, duration);
    }

    async fn record_optimization_hit(&self, duration: Duration) {
        let mut monitor = self.performance_monitor.write().await;
        monitor.total_requests += 1;
        monitor.prefetch_hits += 1;
        self.update_response_time(&mut monitor, duration);
    }

    async fn record_regular_access(&self, duration: Duration) {
        let mut monitor = self.performance_monitor.write().await;
        monitor.total_requests += 1;
        self.update_response_time(&mut monitor, duration);
    }

    fn update_response_time(&self, monitor: &mut PerformanceMonitor, duration: Duration) {
        let duration_ms = duration.as_secs_f64() * 1000.0;
        monitor.average_response_time_ms = if monitor.total_requests == 1 {
            duration_ms
        } else {
            (monitor.average_response_time_ms + duration_ms) / 2.0
        };
    }

    async fn print_performance_summary(&self) {
        let monitor = self.performance_monitor.read().await;
        println!("\n📊 Performance Summary:");
        println!("  Total Requests: {}", monitor.total_requests);
        println!("  Cache Hits: {}", monitor.cache_hits);
        println!("  Prefetch Hits: {}", monitor.prefetch_hits);
        println!("  Hit Rate: {:.2}%", 
            (monitor.cache_hits + monitor.prefetch_hits) as f64 / monitor.total_requests as f64 * 100.0);
        println!("  Average Response Time: {:.2}ms", monitor.average_response_time_ms);
    }

    async fn print_optimization_metrics(&self) {
        let metrics = self.optimizer.get_metrics().await;
        println!("\n🎯 Optimization Metrics:");
        println!("  Total Predictions: {}", metrics.total_predictions);
        println!("  Successful Predictions: {}", metrics.successful_predictions);
        println!("  Prediction Accuracy: {:.2}%", metrics.prediction_accuracy);
        println!("  Cache Hits from Read-ahead: {}", metrics.cache_hits_from_readahead);
        println!("  Sequential Reads Detected: {}", metrics.sequential_reads_detected);
        println!("  Cache Warming Events: {}", metrics.cache_warming_events);
        println!("  Bytes Prefetched: {}", metrics.bytes_prefetched);
        println!("  I/O Reduction: {:.2}%", metrics.io_reduction_percentage);
        println!("  Avg Prediction Time: {:.2}ms", metrics.average_prediction_time_ms);
    }

    /// Comprehensive demonstration of all read-ahead features
    pub async fn run_comprehensive_demo(&self) -> Result<()> {
        println!("🚀 Starting Comprehensive Read-Ahead Optimization Demo\n");

        // 1. Sequential Access Pattern
        println!("=== 1. Sequential Access Pattern ===");
        let _ = self.demonstrate_sequential_access().await?;
        
        tokio::time::sleep(Duration::from_secs(1)).await;

        // 2. Predictive Loading
        println!("\n=== 2. Predictive Loading ===");
        self.demonstrate_predictive_loading().await?;
        
        tokio::time::sleep(Duration::from_secs(1)).await;

        // 3. Cache Warming
        println!("\n=== 3. Cache Warming ===");
        self.demonstrate_cache_warming().await?;

        // 4. Final performance summary
        println!("\n=== Final Performance Summary ===");
        self.print_performance_summary().await;
        self.print_optimization_metrics().await;

        println!("\n✅ Read-Ahead Optimization Demo Complete!");
        Ok(())
    }
}

impl Default for ReadAheadIntegration {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_readahead_integration() {
        let integration = ReadAheadIntegration::new();
        let key = CompactCacheKey { hash: 12345, cache_type: CacheType::Node };
        
        let result = integration.get_data(key).await;
        assert!(result.is_ok());
        
        let data = result.unwrap();
        assert!(!data.is_empty());
    }

    #[tokio::test]
    async fn test_sequential_access_demo() {
        let integration = ReadAheadIntegration::new();
        let result = integration.demonstrate_sequential_access().await;
        
        assert!(result.is_ok());
        let data_list = result.unwrap();
        assert_eq!(data_list.len(), 20);
    }

    #[tokio::test]
    async fn test_predictive_loading_demo() {
        let integration = ReadAheadIntegration::new();
        let result = integration.demonstrate_predictive_loading().await;
        
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_cache_warming_demo() {
        let integration = ReadAheadIntegration::new();
        let result = integration.demonstrate_cache_warming().await;
        
        assert!(result.is_ok());
    }
}