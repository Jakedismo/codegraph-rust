use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};
use parking_lot::{RwLock, Mutex};
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;

/// Comprehensive performance monitoring system for validating 50% improvement targets
#[derive(Debug)]
pub struct PerformanceMonitor {
    metrics: Arc<RwLock<PerformanceMetrics>>,
    targets: PerformanceTargets,
    alerts: Arc<Mutex<Vec<PerformanceAlert>>>,
    event_broadcaster: broadcast::Sender<PerformanceEvent>,
    historical_data: Arc<RwLock<HistoricalMetrics>>,
}

/// Core performance metrics with 50% improvement targets
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceMetrics {
    // Latency Metrics (50% reduction target)
    pub node_query_latency_ms: MovingAverage,
    pub edge_traversal_latency_ms: MovingAverage, 
    pub vector_search_latency_ms: MovingAverage,
    pub rag_response_latency_ms: MovingAverage,
    pub cache_lookup_latency_ms: MovingAverage,
    
    // Memory Metrics (50% reduction target)
    pub graph_memory_mb: u64,
    pub cache_memory_mb: u64,
    pub embedding_memory_mb: u64,
    pub total_memory_mb: u64,
    pub memory_efficiency_ratio: f64,
    
    // Throughput Metrics (2x increase target)
    pub concurrent_queries_per_sec: MovingAverage,
    pub nodes_processed_per_sec: MovingAverage,
    pub embeddings_generated_per_sec: MovingAverage,
    pub cache_operations_per_sec: MovingAverage,
    
    // Efficiency Metrics
    pub cache_hit_rate: f64,
    pub compression_ratio: f64,
    pub cpu_utilization: f64,
    pub io_wait_percentage: f64,
    
    // System Health Metrics
    pub error_rate: f64,
    pub availability_percentage: f64,
    pub last_updated: SystemTime,
}

/// Performance targets defining 50% improvement goals
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceTargets {
    // Latency Targets (baseline → target)
    pub node_query_latency_ms: (f64, f64),        // 100ms → 50ms
    pub edge_traversal_latency_ms: (f64, f64),    // 50ms → 25ms  
    pub vector_search_latency_ms: (f64, f64),     // 200ms → 100ms
    pub rag_response_latency_ms: (f64, f64),      // 200ms → 100ms
    
    // Memory Targets (baseline → target)
    pub graph_memory_mb: (u64, u64),              // 512MB → 256MB
    pub cache_memory_mb: (u64, u64),              // 256MB → 128MB
    pub embedding_memory_mb: (u64, u64),          // 1024MB → 512MB
    
    // Throughput Targets (baseline → target)  
    pub concurrent_queries_per_sec: (f64, f64),   // 1000 → 2000
    pub nodes_processed_per_sec: (f64, f64),      // 10000 → 20000
    pub embeddings_generated_per_sec: (f64, f64), // 500 → 1000
}

impl Default for PerformanceTargets {
    fn default() -> Self {
        Self {
            node_query_latency_ms: (100.0, 50.0),
            edge_traversal_latency_ms: (50.0, 25.0),
            vector_search_latency_ms: (200.0, 100.0),
            rag_response_latency_ms: (200.0, 100.0),
            graph_memory_mb: (512, 256),
            cache_memory_mb: (256, 128),
            embedding_memory_mb: (1024, 512),
            concurrent_queries_per_sec: (1000.0, 2000.0),
            nodes_processed_per_sec: (10000.0, 20000.0),
            embeddings_generated_per_sec: (500.0, 1000.0),
        }
    }
}

/// Moving average implementation for smooth metric tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MovingAverage {
    window_size: usize,
    values: VecDeque<f64>,
    sum: f64,
}

impl MovingAverage {
    pub fn new(window_size: usize) -> Self {
        Self {
            window_size,
            values: VecDeque::with_capacity(window_size),
            sum: 0.0,
        }
    }

    pub fn add_value(&mut self, value: f64) {
        if self.values.len() >= self.window_size {
            if let Some(old_value) = self.values.pop_front() {
                self.sum -= old_value;
            }
        }
        
        self.values.push_back(value);
        self.sum += value;
    }

    pub fn average(&self) -> f64 {
        if self.values.is_empty() {
            0.0
        } else {
            self.sum / self.values.len() as f64
        }
    }

    pub fn count(&self) -> usize {
        self.values.len()
    }
}

impl Default for MovingAverage {
    fn default() -> Self {
        Self::new(100) // 100-sample moving average
    }
}

/// Performance alerts for target violations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceAlert {
    pub alert_type: AlertType,
    pub metric_name: String,
    pub current_value: f64,
    pub target_value: f64,
    pub severity: AlertSeverity,
    pub timestamp: SystemTime,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AlertType {
    LatencyExceeded,
    MemoryExceeded,
    ThroughputBelowTarget,
    ErrorRateHigh,
    EfficiencyLow,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AlertSeverity {
    Info,
    Warning,
    Critical,
}

/// Performance events for real-time monitoring
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PerformanceEvent {
    MetricUpdated {
        metric_name: String,
        value: f64,
        timestamp: SystemTime,
    },
    TargetAchieved {
        metric_name: String,
        improvement_percentage: f64,
    },
    AlertTriggered(PerformanceAlert),
    BenchmarkCompleted {
        test_name: String,
        results: BenchmarkResults,
    },
}

/// Historical metrics for trend analysis
#[derive(Debug, Default)]
struct HistoricalMetrics {
    snapshots: VecDeque<PerformanceSnapshot>,
    max_history: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PerformanceSnapshot {
    metrics: PerformanceMetrics,
    timestamp: SystemTime,
}

impl PerformanceMonitor {
    pub fn new(targets: PerformanceTargets) -> Self {
        let (tx, _rx) = broadcast::channel(1000);
        
        Self {
            metrics: Arc::new(RwLock::new(PerformanceMetrics::default())),
            targets,
            alerts: Arc::new(Mutex::new(Vec::new())),
            event_broadcaster: tx,
            historical_data: Arc::new(RwLock::new(HistoricalMetrics {
                snapshots: VecDeque::new(),
                max_history: 1000,
            })),
        }
    }

    /// Record node query performance
    pub fn record_node_query_latency(&self, latency: Duration) {
        let latency_ms = latency.as_secs_f64() * 1000.0;
        
        {
            let mut metrics = self.metrics.write();
            metrics.node_query_latency_ms.add_value(latency_ms);
            metrics.last_updated = SystemTime::now();
        }

        self.check_latency_target("node_query_latency_ms", latency_ms, self.targets.node_query_latency_ms.1);
        
        let _ = self.event_broadcaster.send(PerformanceEvent::MetricUpdated {
            metric_name: "node_query_latency_ms".to_string(),
            value: latency_ms,
            timestamp: SystemTime::now(),
        });
    }

    /// Record vector search performance
    pub fn record_vector_search_latency(&self, latency: Duration) {
        let latency_ms = latency.as_secs_f64() * 1000.0;
        
        {
            let mut metrics = self.metrics.write();
            metrics.vector_search_latency_ms.add_value(latency_ms);
        }

        self.check_latency_target("vector_search_latency_ms", latency_ms, self.targets.vector_search_latency_ms.1);
    }

    /// Record memory usage
    pub fn record_memory_usage(&self, component: &str, memory_mb: u64) {
        {
            let mut metrics = self.metrics.write();
            match component {
                "graph" => {
                    metrics.graph_memory_mb = memory_mb;
                    self.check_memory_target("graph_memory_mb", memory_mb, self.targets.graph_memory_mb.1);
                }
                "cache" => {
                    metrics.cache_memory_mb = memory_mb;
                    self.check_memory_target("cache_memory_mb", memory_mb, self.targets.cache_memory_mb.1);
                }
                "embedding" => {
                    metrics.embedding_memory_mb = memory_mb;
                    self.check_memory_target("embedding_memory_mb", memory_mb, self.targets.embedding_memory_mb.1);
                }
                _ => {}
            }
            
            // Update total memory
            metrics.total_memory_mb = metrics.graph_memory_mb + metrics.cache_memory_mb + metrics.embedding_memory_mb;
        }
    }

    /// Record throughput metrics
    pub fn record_throughput(&self, metric: &str, value: f64) {
        {
            let mut metrics = self.metrics.write();
            match metric {
                "concurrent_queries_per_sec" => {
                    metrics.concurrent_queries_per_sec.add_value(value);
                    self.check_throughput_target(metric, value, self.targets.concurrent_queries_per_sec.1);
                }
                "nodes_processed_per_sec" => {
                    metrics.nodes_processed_per_sec.add_value(value);
                    self.check_throughput_target(metric, value, self.targets.nodes_processed_per_sec.1);
                }
                "embeddings_generated_per_sec" => {
                    metrics.embeddings_generated_per_sec.add_value(value);
                    self.check_throughput_target(metric, value, self.targets.embeddings_generated_per_sec.1);
                }
                _ => {}
            }
        }
    }

    /// Get current performance snapshot
    pub fn get_current_metrics(&self) -> PerformanceMetrics {
        self.metrics.read().clone()
    }

    /// Calculate improvement percentages vs targets
    pub fn calculate_improvements(&self) -> HashMap<String, f64> {
        let metrics = self.metrics.read();
        let mut improvements = HashMap::new();

        // Latency improvements (lower is better)
        let node_latency = metrics.node_query_latency_ms.average();
        if node_latency > 0.0 {
            let improvement = ((self.targets.node_query_latency_ms.0 - node_latency) / self.targets.node_query_latency_ms.0) * 100.0;
            improvements.insert("node_query_latency".to_string(), improvement);
        }

        // Memory improvements (lower is better)
        let memory_improvement = ((self.targets.graph_memory_mb.0 as f64 - metrics.graph_memory_mb as f64) / self.targets.graph_memory_mb.0 as f64) * 100.0;
        improvements.insert("graph_memory".to_string(), memory_improvement);

        // Throughput improvements (higher is better)
        let throughput = metrics.concurrent_queries_per_sec.average();
        if throughput > 0.0 {
            let improvement = ((throughput - self.targets.concurrent_queries_per_sec.0) / self.targets.concurrent_queries_per_sec.0) * 100.0;
            improvements.insert("concurrent_throughput".to_string(), improvement);
        }

        improvements
    }

    /// Check if performance targets are met
    pub fn targets_achieved(&self) -> TargetAchievementReport {
        let metrics = self.metrics.read();
        let mut report = TargetAchievementReport::default();

        // Check latency targets
        report.node_query_latency_achieved = metrics.node_query_latency_ms.average() <= self.targets.node_query_latency_ms.1;
        report.vector_search_latency_achieved = metrics.vector_search_latency_ms.average() <= self.targets.vector_search_latency_ms.1;

        // Check memory targets  
        report.graph_memory_achieved = metrics.graph_memory_mb <= self.targets.graph_memory_mb.1;
        report.cache_memory_achieved = metrics.cache_memory_mb <= self.targets.cache_memory_mb.1;

        // Check throughput targets
        report.concurrent_throughput_achieved = metrics.concurrent_queries_per_sec.average() >= self.targets.concurrent_queries_per_sec.1;
        report.processing_throughput_achieved = metrics.nodes_processed_per_sec.average() >= self.targets.nodes_processed_per_sec.1;

        // Calculate overall achievement
        let total_targets = 6;
        let achieved_targets = [
            report.node_query_latency_achieved,
            report.vector_search_latency_achieved,
            report.graph_memory_achieved,
            report.cache_memory_achieved,
            report.concurrent_throughput_achieved,
            report.processing_throughput_achieved,
        ].iter().filter(|&&x| x).count();

        report.overall_achievement_percentage = (achieved_targets as f64 / total_targets as f64) * 100.0;

        report
    }

    /// Subscribe to performance events
    pub fn subscribe_to_events(&self) -> broadcast::Receiver<PerformanceEvent> {
        self.event_broadcaster.subscribe()
    }

    /// Get recent alerts
    pub fn get_recent_alerts(&self, limit: usize) -> Vec<PerformanceAlert> {
        let alerts = self.alerts.lock();
        alerts.iter().rev().take(limit).cloned().collect()
    }

    // Internal helper methods
    fn check_latency_target(&self, metric_name: &str, current_value: f64, target_value: f64) {
        if current_value > target_value * 1.1 { // 10% tolerance
            let alert = PerformanceAlert {
                alert_type: AlertType::LatencyExceeded,
                metric_name: metric_name.to_string(),
                current_value,
                target_value,
                severity: if current_value > target_value * 1.5 { AlertSeverity::Critical } else { AlertSeverity::Warning },
                timestamp: SystemTime::now(),
                description: format!("{} exceeded target: {:.2}ms > {:.2}ms", metric_name, current_value, target_value),
            };
            
            self.alerts.lock().push(alert.clone());
            let _ = self.event_broadcaster.send(PerformanceEvent::AlertTriggered(alert));
        }
    }

    fn check_memory_target(&self, metric_name: &str, current_value: u64, target_value: u64) {
        if current_value > target_value {
            let alert = PerformanceAlert {
                alert_type: AlertType::MemoryExceeded,
                metric_name: metric_name.to_string(),
                current_value: current_value as f64,
                target_value: target_value as f64,
                severity: if current_value > target_value * 2 { AlertSeverity::Critical } else { AlertSeverity::Warning },
                timestamp: SystemTime::now(),
                description: format!("{} exceeded target: {}MB > {}MB", metric_name, current_value, target_value),
            };
            
            self.alerts.lock().push(alert.clone());
            let _ = self.event_broadcaster.send(PerformanceEvent::AlertTriggered(alert));
        }
    }

    fn check_throughput_target(&self, metric_name: &str, current_value: f64, target_value: f64) {
        if current_value < target_value * 0.9 { // 10% tolerance
            let alert = PerformanceAlert {
                alert_type: AlertType::ThroughputBelowTarget,
                metric_name: metric_name.to_string(),
                current_value,
                target_value,
                severity: if current_value < target_value * 0.5 { AlertSeverity::Critical } else { AlertSeverity::Warning },
                timestamp: SystemTime::now(),
                description: format!("{} below target: {:.2} < {:.2}", metric_name, current_value, target_value),
            };
            
            self.alerts.lock().push(alert.clone());
            let _ = self.event_broadcaster.send(PerformanceEvent::AlertTriggered(alert));
        }
    }
}

/// Target achievement report
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct TargetAchievementReport {
    pub node_query_latency_achieved: bool,
    pub vector_search_latency_achieved: bool,
    pub graph_memory_achieved: bool,
    pub cache_memory_achieved: bool,
    pub concurrent_throughput_achieved: bool,
    pub processing_throughput_achieved: bool,
    pub overall_achievement_percentage: f64,
}

/// Benchmark results structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkResults {
    pub test_name: String,
    pub duration: Duration,
    pub operations_per_second: f64,
    pub average_latency_ms: f64,
    pub memory_usage_mb: u64,
    pub success_rate: f64,
    pub improvement_vs_baseline: f64,
}

impl Default for PerformanceMetrics {
    fn default() -> Self {
        Self {
            node_query_latency_ms: MovingAverage::default(),
            edge_traversal_latency_ms: MovingAverage::default(),
            vector_search_latency_ms: MovingAverage::default(),
            rag_response_latency_ms: MovingAverage::default(),
            cache_lookup_latency_ms: MovingAverage::default(),
            graph_memory_mb: 0,
            cache_memory_mb: 0,
            embedding_memory_mb: 0,
            total_memory_mb: 0,
            memory_efficiency_ratio: 0.0,
            concurrent_queries_per_sec: MovingAverage::default(),
            nodes_processed_per_sec: MovingAverage::default(),
            embeddings_generated_per_sec: MovingAverage::default(),
            cache_operations_per_sec: MovingAverage::default(),
            cache_hit_rate: 0.0,
            compression_ratio: 0.0,
            cpu_utilization: 0.0,
            io_wait_percentage: 0.0,
            error_rate: 0.0,
            availability_percentage: 100.0,
            last_updated: SystemTime::now(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_moving_average() {
        let mut avg = MovingAverage::new(3);
        
        avg.add_value(10.0);
        assert_eq!(avg.average(), 10.0);
        
        avg.add_value(20.0);
        assert_eq!(avg.average(), 15.0);
        
        avg.add_value(30.0);
        assert_eq!(avg.average(), 20.0);
        
        // Should evict oldest value (10.0)
        avg.add_value(40.0);
        assert_eq!(avg.average(), 30.0); // (20 + 30 + 40) / 3
    }

    #[test]
    fn test_performance_monitor() {
        let targets = PerformanceTargets::default();
        let monitor = PerformanceMonitor::new(targets);
        
        // Test latency recording
        monitor.record_node_query_latency(Duration::from_millis(75));
        monitor.record_node_query_latency(Duration::from_millis(45));
        
        let metrics = monitor.get_current_metrics();
        assert!(metrics.node_query_latency_ms.average() < 100.0);
        
        // Test target achievement
        let achievements = monitor.targets_achieved();
        assert!(achievements.node_query_latency_achieved);
    }

    #[test]
    fn test_improvement_calculation() {
        let targets = PerformanceTargets::default();
        let monitor = PerformanceMonitor::new(targets);
        
        // Record performance that meets 50% improvement target
        monitor.record_node_query_latency(Duration::from_millis(50)); // Target achieved
        monitor.record_memory_usage("graph", 256); // Target achieved
        
        let improvements = monitor.calculate_improvements();
        assert!(improvements.get("node_query_latency").unwrap_or(&0.0) >= &50.0);
        assert!(improvements.get("graph_memory").unwrap_or(&0.0) >= &50.0);
    }
}