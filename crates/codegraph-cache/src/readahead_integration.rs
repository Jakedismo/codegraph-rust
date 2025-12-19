use crate::{CacheKey, ReadAheadConfig, ReadAheadOptimizer};
use codegraph_core::{Result};
use dashmap::DashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

/// Integration layer that demonstrates read-ahead optimization in action
pub struct ReadAheadIntegration {
    optimizer: Arc<ReadAheadOptimizer>,
    cache_storage: Arc<DashMap<CacheKey, Vec<u8>>>,
    performance_monitor: Arc<RwLock<PerformanceMonitor>>,
}

#[derive(Debug, Default)]
struct PerformanceMonitor {
    total_requests: u64,
    cache_hits: u64,
    prefetch_hits: u64,
}

impl ReadAheadIntegration {
    pub fn new() -> Self {
        let config = ReadAheadConfig {
            max_pattern_history: 5000,
            prediction_window_size: 30,
            sequential_threshold: 3,
            cache_warming_interval: Duration::from_secs(30),
            prefetch_depth: 10,
            pattern_decay_factor: 0.95,
            min_confidence_threshold: 0.7,
            adaptive_learning_rate: 0.1,
        };

        Self {
            optimizer: Arc::new(ReadAheadOptimizer::new(config)),
            cache_storage: Arc::new(DashMap::new()),
            performance_monitor: Arc::new(RwLock::new(PerformanceMonitor::default())),
        }
    }

    /// Get the read-ahead optimizer
    pub fn optimizer(&self) -> Arc<ReadAheadOptimizer> {
        Arc::clone(&self.optimizer)
    }

    /// Get data with read-ahead optimization
    async fn get_data(&self, key: CacheKey) -> Result<Vec<u8>> {
        // Check cache first
        if let Some(data) = self.cache_storage.get(&key) {
            self.record_cache_hit().await;
            return Ok(data.clone());
        }

        // Simulate data fetch
        let data = self.simulate_data_fetch(&key).await?;
        self.cache_storage.insert(key.clone(), data.clone());

        // Record access pattern / trigger optimizer
        let _ = self.optimizer.optimize_read(key.clone()).await?;

        Ok(data)
    }

    async fn simulate_data_fetch(&self, key: &CacheKey) -> Result<Vec<u8>> {
        // Simulate network/disk I/O delay
        tokio::time::sleep(Duration::from_millis(10)).await;

        // Generate dummy data based on key type
        let data = match key {
            CacheKey::Node(_) => format!("node_data_{:?}", key),
            CacheKey::Embedding(_) => format!("embedding_vector_{:?}", key),
            CacheKey::Query(_) => format!("query_result_{:?}", key),
            CacheKey::Custom(_) => format!("custom_data_{:?}", key),
        };

        Ok(data.into_bytes())
    }

    async fn record_cache_hit(&self) {
        let mut monitor = self.performance_monitor.write().await;
        monitor.cache_hits += 1;
        monitor.total_requests += 1;
    }

    /// Demonstrates sequential access pattern optimization
    pub async fn demonstrate_sequential_optimization(&self) -> Result<Vec<Vec<u8>>> {
        println!("🚀 Demonstrating Sequential Access Optimization");
        let mut results = Vec::new();
        let base_id = 1000u64;

        // Simulate sequential access pattern
        for i in 0..20 {
            let key = CacheKey::Custom(format!("seq-{}", base_id + i as u64));

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
            ("node_2000", "embedding_2001", "query_2002"),
            ("node_3000", "embedding_3001", "query_3002"),
            ("node_4000", "embedding_4001", "query_4002"),
        ];

        // First, train the pattern
        println!("Training access patterns...");
        for (node_id, emb_id, query_id) in &patterns {
            let keys = vec![
                CacheKey::Custom(node_id.to_string()),
                CacheKey::Embedding(emb_id.to_string()),
                CacheKey::Query(query_id.to_string()),
            ];

            for key in keys {
                let _ = self.get_data(key).await?;
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
        }

        println!("Pattern training completed!");
        self.print_performance_summary().await;
        Ok(())
    }

    /// Demonstrates cache warming capabilities
    pub async fn demonstrate_cache_warming(&self) -> Result<()> {
        println!("🔥 Demonstrating Cache Warming");

        // Start cache warming background process
        self.optimizer.start_cache_warming().await?;

        // Simulate hot data access patterns
        let hot_keys = vec![
            CacheKey::Custom("hot_100".to_string()),
            CacheKey::Embedding("hot_101".to_string()),
            CacheKey::Query("hot_102".to_string()),
        ];

        // Access hot keys multiple times to establish patterns
        for _ in 0..10 {
            for key in &hot_keys {
                let _ = self.get_data(key.clone()).await?;
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        }

        // Wait for cache warming to kick in
        tokio::time::sleep(Duration::from_secs(2)).await;

        // Now access should be very fast due to warming
        let start = std::time::Instant::now();
        for key in &hot_keys {
            let _ = self.get_data(key.clone()).await?;
        }
        println!("Warmed cache access time: {:?}", start.elapsed());

        Ok(())
    }

    async fn print_performance_summary(&self) {
        let monitor = self.performance_monitor.read().await;
        let hit_rate = if monitor.total_requests > 0 {
            monitor.cache_hits as f64 / monitor.total_requests as f64 * 100.0
        } else {
            0.0
        };

        println!("📊 Performance Summary:");
        println!("  Cache Hit Rate: {:.2}%", hit_rate);
        println!("  Total Requests: {}", monitor.total_requests);
        println!("  Cache Hits: {}", monitor.cache_hits);
        println!("  Prefetch Hits: {}", monitor.prefetch_hits);
    }

    /// Runs a comprehensive demonstration of all read-ahead features
    pub async fn run_comprehensive_demo(&self) -> Result<()> {
        println!("🚀 Starting Comprehensive Read-Ahead Optimization Demo");
        println!("{}", "=".repeat(60));

        // Sequential optimization demo
        let _ = self.demonstrate_sequential_optimization().await?;

        println!();

        // Predictive loading demo
        self.demonstrate_predictive_loading().await?;

        println!();

        // Cache warming demo
        self.demonstrate_cache_warming().await?;

        println!();
        println!("✅ All read-ahead optimization demos completed successfully!");

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
    use uuid::Uuid;

    #[tokio::test]
    async fn test_basic_integration() -> Result<()> {
        let integration = ReadAheadIntegration::new();
        let key = CacheKey::Node(Uuid::new_v4());

        let data = integration.get_data(key).await?;
        assert!(!data.is_empty());

        Ok(())
    }

    #[tokio::test]
    async fn test_sequential_demo() -> Result<()> {
        let integration = ReadAheadIntegration::new();
        let results = integration.demonstrate_sequential_optimization().await?;
        assert_eq!(results.len(), 20);

        Ok(())
    }
}
