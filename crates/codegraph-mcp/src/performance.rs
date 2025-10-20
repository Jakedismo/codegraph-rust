use serde_json::{json, Value};
use std::collections::HashMap;
use std::time::{Duration, Instant};
use tracing::{info, warn};

/// Performance monitor for Qwen2.5-Coder MCP operations
#[derive(Debug, Clone)]
pub struct QwenPerformanceMonitor {
    operation_metrics: HashMap<String, Vec<OperationMetric>>,
}

#[derive(Debug, Clone)]
struct OperationMetric {
    operation_type: String,
    duration: Duration,
    context_tokens: usize,
    completion_tokens: usize,
    confidence_score: f32,
    timestamp: Instant,
}

impl QwenPerformanceMonitor {
    pub fn new() -> Self {
        Self {
            operation_metrics: HashMap::new(),
        }
    }

    /// Record a Qwen operation metric
    pub fn record_operation(
        &mut self,
        operation_type: &str,
        duration: Duration,
        context_tokens: usize,
        completion_tokens: usize,
        confidence_score: f32,
    ) {
        let metric = OperationMetric {
            operation_type: operation_type.to_string(),
            duration,
            context_tokens,
            completion_tokens,
            confidence_score,
            timestamp: Instant::now(),
        };

        self.operation_metrics
            .entry(operation_type.to_string())
            .or_insert_with(Vec::new)
            .push(metric);

        // Log performance info
        info!(
            "Qwen operation: {} completed in {}ms (context: {}, completion: {}, confidence: {:.2})",
            operation_type,
            duration.as_millis(),
            context_tokens,
            completion_tokens,
            confidence_score
        );

        // Warn if performance is below targets
        self.check_performance_targets(operation_type, duration, confidence_score);

        // Keep only recent metrics (last 100 operations per type)
        if let Some(metrics) = self.operation_metrics.get_mut(operation_type) {
            if metrics.len() > 100 {
                metrics.remove(0);
            }
        }
    }

    /// Get performance summary for all operations
    pub fn get_performance_summary(&self) -> Value {
        let mut summary = json!({});

        for (operation_type, metrics) in &self.operation_metrics {
            if metrics.is_empty() {
                continue;
            }

            let total_ops = metrics.len();
            let avg_duration = metrics
                .iter()
                .map(|m| m.duration.as_millis() as f64)
                .sum::<f64>()
                / total_ops as f64;

            let avg_context_tokens =
                metrics.iter().map(|m| m.context_tokens as f64).sum::<f64>() / total_ops as f64;

            let avg_completion_tokens = metrics
                .iter()
                .map(|m| m.completion_tokens as f64)
                .sum::<f64>()
                / total_ops as f64;

            let avg_confidence = metrics
                .iter()
                .map(|m| m.confidence_score as f64)
                .sum::<f64>()
                / total_ops as f64;

            // Calculate performance rating
            let performance_rating =
                self.calculate_performance_rating(operation_type, avg_duration);

            summary[operation_type] = json!({
                "total_operations": total_ops,
                "average_duration_ms": avg_duration,
                "average_context_tokens": avg_context_tokens,
                "average_completion_tokens": avg_completion_tokens,
                "average_confidence": avg_confidence,
                "performance_rating": performance_rating,
                "meets_targets": self.meets_performance_targets(operation_type, avg_duration, avg_confidence)
            });
        }

        json!({
            "qwen_performance_summary": summary,
            "total_operations": self.operation_metrics.values().map(|v| v.len()).sum::<usize>(),
            "monitoring_since": "application_start",
            "model_info": {
                "model": "qwen2.5-coder-14b-128k",
                "context_window": 128000,
                "parameters": "14B",
                "quantization": "Q4_K_M"
            }
        })
    }

    /// Check if performance meets targets and log warnings
    fn check_performance_targets(&self, operation_type: &str, duration: Duration, confidence: f32) {
        let targets = get_performance_targets();

        if let Some(target_duration) = targets.get(operation_type) {
            if duration.as_millis() > *target_duration as u128 {
                warn!(
                    "⚠️ Performance below target for {}: {}ms > {}ms",
                    operation_type,
                    duration.as_millis(),
                    target_duration
                );
            }
        }

        if confidence < 0.8 {
            warn!(
                "⚠️ Low confidence for {}: {:.2} < 0.8",
                operation_type, confidence
            );
        }
    }

    fn calculate_performance_rating(&self, operation_type: &str, avg_duration: f64) -> String {
        let targets = get_performance_targets();

        if let Some(target) = targets.get(operation_type) {
            let performance_ratio = *target as f64 / avg_duration;

            if performance_ratio >= 1.5 {
                "excellent".to_string()
            } else if performance_ratio >= 1.0 {
                "good".to_string()
            } else if performance_ratio >= 0.7 {
                "acceptable".to_string()
            } else {
                "needs_optimization".to_string()
            }
        } else {
            "unknown".to_string()
        }
    }

    fn meets_performance_targets(
        &self,
        operation_type: &str,
        avg_duration: f64,
        avg_confidence: f64,
    ) -> bool {
        let targets = get_performance_targets();

        if let Some(target_duration) = targets.get(operation_type) {
            avg_duration <= *target_duration as f64 && avg_confidence >= 0.8
        } else {
            avg_confidence >= 0.8
        }
    }
}

/// Performance targets for different Qwen operations
fn get_performance_targets() -> HashMap<String, u64> {
    let mut targets = HashMap::new();

    // Target response times in milliseconds
    targets.insert("enhanced_search".to_string(), 3000); // 3 seconds
    targets.insert("semantic_intelligence".to_string(), 5000); // 5 seconds
    targets.insert("pattern_analysis".to_string(), 4000); // 4 seconds
    targets.insert("impact_analysis".to_string(), 4000); // 4 seconds

    targets
}

/// Global performance monitor instance
static mut GLOBAL_MONITOR: Option<QwenPerformanceMonitor> = None;
static MONITOR_INIT: std::sync::Once = std::sync::Once::new();

/// Get or initialize the global performance monitor
pub fn get_performance_monitor() -> &'static mut QwenPerformanceMonitor {
    unsafe {
        MONITOR_INIT.call_once(|| {
            GLOBAL_MONITOR = Some(QwenPerformanceMonitor::new());
        });
        GLOBAL_MONITOR.as_mut().unwrap()
    }
}

/// Record a Qwen operation metric (convenience function)
pub fn record_qwen_operation(
    operation_type: &str,
    duration: Duration,
    context_tokens: usize,
    completion_tokens: usize,
    confidence_score: f32,
) {
    get_performance_monitor().record_operation(
        operation_type,
        duration,
        context_tokens,
        completion_tokens,
        confidence_score,
    );
}

/// Get current performance summary (convenience function)
pub fn get_performance_summary() -> Value {
    get_performance_monitor().get_performance_summary()
}
