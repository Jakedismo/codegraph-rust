use std::collections::HashMap;
use std::time::{Duration, Instant};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use std::sync::Arc;

/// Performance monitoring and regression detection system
/// Tracks performance metrics and detects regressions automatically

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceMetric {
    pub name: String,
    pub value: f64,
    pub unit: String,
    pub timestamp: u64,
    pub context: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceBaseline {
    pub metric_name: String,
    pub baseline_value: f64,
    pub target_improvement: f64, // e.g., 0.5 for 50% improvement
    pub warning_threshold: f64,  // e.g., 0.03 for 3% regression
    pub failure_threshold: f64,  // e.g., 0.05 for 5% regression
    pub last_updated: u64,
}

#[derive(Debug, Clone)]
pub enum RegressionStatus {
    Improved(f64),      // percentage improvement
    NoChange,
    Warning(f64),       // percentage regression
    Failure(f64),       // significant regression
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceReport {
    pub timestamp: u64,
    pub metrics: Vec<PerformanceMetric>,
    pub regressions: Vec<RegressionDetection>,
    pub improvements: Vec<ImprovementDetection>,
    pub summary: PerformanceSummary,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegressionDetection {
    pub metric_name: String,
    pub current_value: f64,
    pub baseline_value: f64,
    pub regression_percentage: f64,
    pub severity: String, // "warning" or "failure"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImprovementDetection {
    pub metric_name: String,
    pub current_value: f64,
    pub baseline_value: f64,
    pub improvement_percentage: f64,
    pub target_achieved: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceSummary {
    pub total_metrics: usize,
    pub regressions_count: usize,
    pub improvements_count: usize,
    pub targets_achieved: usize,
    pub overall_status: String,
}

pub struct PerformanceMonitor {
    baselines: Arc<RwLock<HashMap<String, PerformanceBaseline>>>,
    metrics_history: Arc<RwLock<Vec<PerformanceMetric>>>,
    max_history_size: usize,
}

impl PerformanceMonitor {
    pub fn new(max_history_size: usize) -> Self {
        Self {
            baselines: Arc::new(RwLock::new(HashMap::new())),
            metrics_history: Arc::new(RwLock::new(Vec::new())),
            max_history_size,
        }
    }

    /// Record a performance metric
    pub async fn record_metric(&self, metric: PerformanceMetric) {
        let mut history = self.metrics_history.write().await;
        history.push(metric);

        // Trim history if it exceeds max size
        if history.len() > self.max_history_size {
            history.drain(0..history.len() - self.max_history_size);
        }
    }

    /// Set or update a performance baseline
    pub async fn set_baseline(&self, baseline: PerformanceBaseline) {
        let mut baselines = self.baselines.write().await;
        baselines.insert(baseline.metric_name.clone(), baseline);
    }

    /// Check for performance regressions
    pub async fn check_regression(&self, metric: &PerformanceMetric) -> RegressionStatus {
        let baselines = self.baselines.read().await;
        
        if let Some(baseline) = baselines.get(&metric.name) {
            let change_ratio = (metric.value - baseline.baseline_value) / baseline.baseline_value;
            
            // For metrics where lower is better (latency), positive change is bad
            // For metrics where higher is better (throughput), negative change is bad
            let is_latency_metric = metric.unit.contains("ms") || metric.unit.contains("μs") || metric.unit.contains("ns");
            
            let regression_threshold = if is_latency_metric {
                baseline.warning_threshold
            } else {
                -baseline.warning_threshold
            };
            
            let failure_threshold = if is_latency_metric {
                baseline.failure_threshold
            } else {
                -baseline.failure_threshold
            };

            if change_ratio > failure_threshold {
                RegressionStatus::Failure(change_ratio.abs() * 100.0)
            } else if change_ratio > regression_threshold {
                RegressionStatus::Warning(change_ratio.abs() * 100.0)
            } else if change_ratio < -baseline.target_improvement {
                RegressionStatus::Improved(change_ratio.abs() * 100.0)
            } else {
                RegressionStatus::NoChange
            }
        } else {
            RegressionStatus::NoChange
        }
    }

    /// Generate comprehensive performance report
    pub async fn generate_report(&self) -> PerformanceReport {
        let history = self.metrics_history.read().await;
        let baselines = self.baselines.read().await;

        let mut regressions = Vec::new();
        let mut improvements = Vec::new();
        let mut targets_achieved = 0;

        // Group metrics by name and get latest values
        let mut latest_metrics = HashMap::new();
        for metric in history.iter() {
            latest_metrics.insert(metric.name.clone(), metric.clone());
        }

        for (metric_name, metric) in &latest_metrics {
            if let Some(baseline) = baselines.get(metric_name) {
                let status = self.check_regression(metric).await;

                match status {
                    RegressionStatus::Failure(percentage) => {
                        regressions.push(RegressionDetection {
                            metric_name: metric.name.clone(),
                            current_value: metric.value,
                            baseline_value: baseline.baseline_value,
                            regression_percentage: percentage,
                            severity: "failure".to_string(),
                        });
                    }
                    RegressionStatus::Warning(percentage) => {
                        regressions.push(RegressionDetection {
                            metric_name: metric.name.clone(),
                            current_value: metric.value,
                            baseline_value: baseline.baseline_value,
                            regression_percentage: percentage,
                            severity: "warning".to_string(),
                        });
                    }
                    RegressionStatus::Improved(percentage) => {
                        let target_achieved = percentage >= (baseline.target_improvement * 100.0);
                        if target_achieved {
                            targets_achieved += 1;
                        }
                        
                        improvements.push(ImprovementDetection {
                            metric_name: metric.name.clone(),
                            current_value: metric.value,
                            baseline_value: baseline.baseline_value,
                            improvement_percentage: percentage,
                            target_achieved,
                        });
                    }
                    RegressionStatus::NoChange => {}
                }
            }
        }

        let overall_status = if regressions.iter().any(|r| r.severity == "failure") {
            "FAILURE".to_string()
        } else if !regressions.is_empty() {
            "WARNING".to_string()
        } else if targets_achieved > 0 {
            "IMPROVED".to_string()
        } else {
            "STABLE".to_string()
        };

        PerformanceReport {
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            metrics: latest_metrics.into_values().collect(),
            regressions,
            improvements,
            summary: PerformanceSummary {
                total_metrics: latest_metrics.len(),
                regressions_count: regressions.len(),
                improvements_count: improvements.len(),
                targets_achieved,
                overall_status,
            },
        }
    }

    /// Initialize default baselines for CodeGraph components
    pub async fn initialize_codegraph_baselines(&self) {
        let baselines = vec![
            // Vector search baselines - targeting sub-millisecond performance
            PerformanceBaseline {
                metric_name: "vector_search_latency_us".to_string(),
                baseline_value: 1000.0, // 1ms baseline
                target_improvement: 0.5, // 50% improvement = 500μs target
                warning_threshold: 0.03,
                failure_threshold: 0.05,
                last_updated: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs(),
            },
            
            // Graph query baselines
            PerformanceBaseline {
                metric_name: "graph_query_latency_ms".to_string(),
                baseline_value: 50.0, // 50ms baseline
                target_improvement: 0.5, // 50% improvement = 25ms target
                warning_threshold: 0.03,
                failure_threshold: 0.05,
                last_updated: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs(),
            },
            
            // Cache operation baselines
            PerformanceBaseline {
                metric_name: "cache_operation_latency_us".to_string(),
                baseline_value: 200.0, // 200μs baseline
                target_improvement: 0.5, // 50% improvement = 100μs target
                warning_threshold: 0.03,
                failure_threshold: 0.05,
                last_updated: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs(),
            },
            
            // Memory usage baselines
            PerformanceBaseline {
                metric_name: "memory_usage_mb".to_string(),
                baseline_value: 1024.0, // 1GB baseline
                target_improvement: 0.5, // 50% reduction = 512MB target
                warning_threshold: 0.05,
                failure_threshold: 0.10,
                last_updated: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs(),
            },
            
            // Parser throughput baselines
            PerformanceBaseline {
                metric_name: "parser_throughput_bytes_per_sec".to_string(),
                baseline_value: 1000000.0, // 1MB/s baseline
                target_improvement: 0.5, // 50% increase = 1.5MB/s target
                warning_threshold: -0.03, // negative because higher is better
                failure_threshold: -0.05,
                last_updated: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs(),
            },
        ];

        for baseline in baselines {
            self.set_baseline(baseline).await;
        }
    }

    /// Save performance report to file for CI/CD integration
    pub async fn save_report_to_file(&self, report: &PerformanceReport, file_path: &str) -> Result<(), Box<dyn std::error::Error>> {
        let json = serde_json::to_string_pretty(report)?;
        tokio::fs::write(file_path, json).await?;
        Ok(())
    }

    /// Load baselines from file
    pub async fn load_baselines_from_file(&self, file_path: &str) -> Result<(), Box<dyn std::error::Error>> {
        let content = tokio::fs::read_to_string(file_path).await?;
        let baselines: Vec<PerformanceBaseline> = serde_json::from_str(&content)?;
        
        for baseline in baselines {
            self.set_baseline(baseline).await;
        }
        
        Ok(())
    }
}

/// Performance measurement utilities
pub struct PerformanceTimer {
    start: Instant,
    metric_name: String,
    context: HashMap<String, String>,
    monitor: Arc<PerformanceMonitor>,
}

impl PerformanceTimer {
    pub fn new(metric_name: String, monitor: Arc<PerformanceMonitor>) -> Self {
        Self {
            start: Instant::now(),
            metric_name,
            context: HashMap::new(),
            monitor,
        }
    }

    pub fn with_context(mut self, key: String, value: String) -> Self {
        self.context.insert(key, value);
        self
    }

    pub async fn finish(self, unit: &str) {
        let duration = self.start.elapsed();
        let value = match unit {
            "ms" => duration.as_millis() as f64,
            "μs" | "us" => duration.as_micros() as f64,
            "ns" => duration.as_nanos() as f64,
            "s" => duration.as_secs_f64(),
            _ => duration.as_millis() as f64,
        };

        let metric = PerformanceMetric {
            name: self.metric_name,
            value,
            unit: unit.to_string(),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            context: self.context,
        };

        self.monitor.record_metric(metric).await;
    }
}

/// Convenience macros for performance measurement
#[macro_export]
macro_rules! measure_performance {
    ($monitor:expr, $metric_name:expr, $unit:expr, $block:block) => {{
        let timer = PerformanceTimer::new($metric_name.to_string(), $monitor.clone());
        let result = $block;
        timer.finish($unit).await;
        result
    }};
}

#[macro_export]
macro_rules! measure_async_performance {
    ($monitor:expr, $metric_name:expr, $unit:expr, $async_block:block) => {{
        let timer = PerformanceTimer::new($metric_name.to_string(), $monitor.clone());
        let result = async $async_block.await;
        timer.finish($unit).await;
        result
    }};
}

/// CI/CD integration utilities
pub struct PerformanceCIIntegration;

impl PerformanceCIIntegration {
    /// Check if performance targets are met for CI/CD pipeline
    pub async fn validate_performance_targets(report: &PerformanceReport) -> bool {
        // Fail CI if there are any failure-level regressions
        if report.regressions.iter().any(|r| r.severity == "failure") {
            eprintln!("❌ Performance CI FAILED: Critical regressions detected");
            for regression in &report.regressions {
                if regression.severity == "failure" {
                    eprintln!("  🔴 {}: {:.2}% regression", regression.metric_name, regression.regression_percentage);
                }
            }
            return false;
        }

        // Warn about non-critical regressions
        if !report.regressions.is_empty() {
            eprintln!("⚠️  Performance CI WARNING: Non-critical regressions detected");
            for regression in &report.regressions {
                if regression.severity == "warning" {
                    eprintln!("  🟡 {}: {:.2}% regression", regression.metric_name, regression.regression_percentage);
                }
            }
        }

        // Report improvements
        if !report.improvements.is_empty() {
            eprintln!("✅ Performance improvements detected:");
            for improvement in &report.improvements {
                let status = if improvement.target_achieved { "🎯 TARGET MET" } else { "📈 IMPROVED" };
                eprintln!("  {} {}: {:.2}% improvement", status, improvement.metric_name, improvement.improvement_percentage);
            }
        }

        true
    }

    /// Generate GitHub Actions compatible output
    pub fn generate_github_actions_output(report: &PerformanceReport) -> String {
        let mut output = String::new();
        
        output.push_str(&format!("::set-output name=performance_status::{}\n", report.summary.overall_status));
        output.push_str(&format!("::set-output name=targets_achieved::{}\n", report.summary.targets_achieved));
        output.push_str(&format!("::set-output name=regressions_count::{}\n", report.summary.regressions_count));
        
        // Add annotations for regressions
        for regression in &report.regressions {
            let level = if regression.severity == "failure" { "error" } else { "warning" };
            output.push_str(&format!(
                "::{} title=Performance Regression::{}% regression in {}\n",
                level, regression.regression_percentage, regression.metric_name
            ));
        }
        
        output
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_performance_monitor() {
        let monitor = PerformanceMonitor::new(1000);
        monitor.initialize_codegraph_baselines().await;

        // Test recording a metric
        let metric = PerformanceMetric {
            name: "vector_search_latency_us".to_string(),
            value: 500.0, // 50% improvement from 1000μs baseline
            unit: "μs".to_string(),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            context: HashMap::new(),
        };

        monitor.record_metric(metric.clone()).await;

        // Check for improvement
        let status = monitor.check_regression(&metric).await;
        match status {
            RegressionStatus::Improved(percentage) => {
                assert!(percentage >= 50.0, "Expected 50% improvement, got {}%", percentage);
            }
            _ => panic!("Expected improvement, got {:?}", status),
        }

        // Generate report
        let report = monitor.generate_report().await;
        assert_eq!(report.summary.improvements_count, 1);
        assert_eq!(report.summary.targets_achieved, 1);
    }

    #[tokio::test]
    async fn test_regression_detection() {
        let monitor = PerformanceMonitor::new(1000);
        monitor.initialize_codegraph_baselines().await;

        // Test a regression
        let metric = PerformanceMetric {
            name: "vector_search_latency_us".to_string(),
            value: 1100.0, // 10% regression from 1000μs baseline
            unit: "μs".to_string(),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            context: HashMap::new(),
        };

        let status = monitor.check_regression(&metric).await;
        match status {
            RegressionStatus::Failure(percentage) => {
                assert!(percentage >= 5.0, "Expected failure-level regression");
            }
            _ => panic!("Expected regression failure, got {:?}", status),
        }
    }
}