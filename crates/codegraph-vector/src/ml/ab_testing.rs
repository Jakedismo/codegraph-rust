//! A/B Testing framework for model evaluation and comparison
//! 
//! This module provides comprehensive A/B testing capabilities for comparing
//! different models, configurations, and optimization strategies. It includes
//! statistical significance testing, experiment design, and performance analytics.

use crate::ml::inference::{InferenceEngine, InferenceResult, InferenceMetrics};
use crate::ml::training::{TrainedModel, ModelType};
use crate::ml::features::CodeFeatures;
use codegraph_core::{CodeGraphError, CodeNode, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;
use uuid::Uuid;

/// Configuration for A/B testing experiments
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ABTestConfig {
    /// Experiment name
    pub name: String,
    /// Experiment description
    pub description: String,
    /// Traffic allocation between variants
    pub traffic_allocation: TrafficAllocation,
    /// Experiment duration
    pub duration: Duration,
    /// Statistical significance configuration
    pub statistical_config: StatisticalConfig,
    /// Metrics to track
    pub metrics: Vec<ExperimentMetric>,
    /// Sample size configuration
    pub sample_size: SampleSizeConfig,
}

/// Traffic allocation strategy for A/B tests
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrafficAllocation {
    /// Variant allocations (must sum to 1.0)
    pub allocations: HashMap<String, f32>,
    /// Allocation strategy
    pub strategy: AllocationStrategy,
    /// Sticky sessions (same user gets same variant)
    pub sticky_sessions: bool,
}

/// Allocation strategies for traffic splitting
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AllocationStrategy {
    /// Random allocation
    Random,
    /// Hash-based allocation (deterministic)
    HashBased,
    /// Round-robin allocation
    RoundRobin,
    /// Weighted random allocation
    WeightedRandom,
}

/// Statistical significance configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatisticalConfig {
    /// Significance level (alpha)
    pub alpha: f64,
    /// Statistical power (1 - beta)
    pub power: f64,
    /// Minimum detectable effect size
    pub min_effect_size: f64,
    /// Test type
    pub test_type: StatisticalTest,
    /// Multiple testing correction
    pub correction: MultipleTestingCorrection,
}

impl Default for StatisticalConfig {
    fn default() -> Self {
        Self {
            alpha: 0.05,
            power: 0.8,
            min_effect_size: 0.1,
            test_type: StatisticalTest::TTest,
            correction: MultipleTestingCorrection::Bonferroni,
        }
    }
}

/// Types of statistical tests
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StatisticalTest {
    /// Student's t-test
    TTest,
    /// Mann-Whitney U test
    MannWhitney,
    /// Chi-square test
    ChiSquare,
    /// Kolmogorov-Smirnov test
    KolmogorovSmirnov,
    /// Bootstrap test
    Bootstrap,
}

/// Multiple testing correction methods
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MultipleTestingCorrection {
    /// No correction
    None,
    /// Bonferroni correction
    Bonferroni,
    /// Benjamini-Hochberg (FDR)
    BenjaminiHochberg,
    /// Holm-Bonferroni
    HolmBonferroni,
}

/// Metrics to track in experiments
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExperimentMetric {
    /// Model accuracy
    Accuracy,
    /// Inference latency
    Latency,
    /// Throughput (requests per second)
    Throughput,
    /// Memory usage
    MemoryUsage,
    /// Error rate
    ErrorRate,
    /// User satisfaction score
    UserSatisfaction,
    /// Task completion rate
    TaskCompletionRate,
    /// Custom metric
    Custom {
        name: String,
        description: String,
        aggregation: MetricAggregation,
    },
}

/// Metric aggregation methods
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MetricAggregation {
    Mean,
    Median,
    P95,
    P99,
    Sum,
    Count,
    Rate,
}

/// Sample size configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SampleSizeConfig {
    /// Minimum sample size per variant
    pub min_sample_size: usize,
    /// Maximum sample size per variant
    pub max_sample_size: usize,
    /// Early stopping criteria
    pub early_stopping: EarlyStoppingConfig,
    /// Sample size calculation method
    pub calculation_method: SampleSizeMethod,
}

/// Early stopping configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EarlyStoppingConfig {
    /// Enable early stopping
    pub enabled: bool,
    /// Check interval
    pub check_interval: Duration,
    /// Minimum samples before early stopping
    pub min_samples: usize,
    /// Futility boundary
    pub futility_boundary: f64,
    /// Efficacy boundary
    pub efficacy_boundary: f64,
}

/// Sample size calculation methods
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SampleSizeMethod {
    /// Fixed sample size
    Fixed,
    /// Power analysis
    PowerAnalysis,
    /// Sequential testing
    Sequential,
    /// Adaptive
    Adaptive,
}

/// A/B test experiment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Experiment {
    /// Experiment ID
    pub id: String,
    /// Experiment configuration
    pub config: ABTestConfig,
    /// Experiment status
    pub status: ExperimentStatus,
    /// Start time
    pub start_time: SystemTime,
    /// End time
    pub end_time: Option<SystemTime>,
    /// Variants being tested
    pub variants: HashMap<String, ExperimentVariant>,
    /// Results
    pub results: Option<ExperimentResults>,
}

/// Experiment status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExperimentStatus {
    /// Experiment is being designed
    Draft,
    /// Experiment is ready to start
    Ready,
    /// Experiment is currently running
    Running,
    /// Experiment completed successfully
    Completed,
    /// Experiment was stopped early
    Stopped,
    /// Experiment failed
    Failed,
}

/// Experiment variant (model configuration)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExperimentVariant {
    /// Variant name
    pub name: String,
    /// Variant description
    pub description: String,
    /// Model identifier
    pub model_id: String,
    /// Configuration parameters
    pub parameters: HashMap<String, serde_json::Value>,
    /// Traffic allocation
    pub allocation: f32,
}

/// Experiment results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExperimentResults {
    /// Overall experiment summary
    pub summary: ExperimentSummary,
    /// Per-variant results
    pub variant_results: HashMap<String, VariantResults>,
    /// Statistical comparisons
    pub comparisons: Vec<VariantComparison>,
    /// Recommendations
    pub recommendations: Vec<String>,
}

/// Experiment summary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExperimentSummary {
    /// Total samples collected
    pub total_samples: usize,
    /// Experiment duration
    pub duration: Duration,
    /// Overall winner (if any)
    pub winner: Option<String>,
    /// Confidence level
    pub confidence: f64,
    /// Effect size
    pub effect_size: f64,
    /// Statistical significance achieved
    pub significant: bool,
}

/// Results for a single variant
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VariantResults {
    /// Variant name
    pub variant_name: String,
    /// Sample size
    pub sample_size: usize,
    /// Metric measurements
    pub metrics: HashMap<String, MetricSummary>,
    /// Performance over time
    pub time_series: Vec<TimeSeriesPoint>,
}

/// Summary statistics for a metric
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricSummary {
    /// Metric name
    pub name: String,
    /// Mean value
    pub mean: f64,
    /// Standard deviation
    pub std_dev: f64,
    /// Median
    pub median: f64,
    /// 95th percentile
    pub p95: f64,
    /// 99th percentile
    pub p99: f64,
    /// Minimum value
    pub min: f64,
    /// Maximum value
    pub max: f64,
    /// Confidence interval
    pub confidence_interval: (f64, f64),
}

/// Time series data point
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeSeriesPoint {
    /// Timestamp
    pub timestamp: SystemTime,
    /// Metric values at this time
    pub values: HashMap<String, f64>,
    /// Sample count at this time
    pub sample_count: usize,
}

/// Statistical comparison between variants
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VariantComparison {
    /// Baseline variant
    pub baseline: String,
    /// Treatment variant
    pub treatment: String,
    /// Metric being compared
    pub metric: String,
    /// Test statistic
    pub test_statistic: f64,
    /// P-value
    pub p_value: f64,
    /// Effect size
    pub effect_size: f64,
    /// Confidence interval for effect
    pub effect_confidence_interval: (f64, f64),
    /// Statistical significance
    pub significant: bool,
    /// Test type used
    pub test_type: StatisticalTest,
}

/// Data point for A/B testing
#[derive(Debug, Clone)]
pub struct ExperimentDataPoint {
    /// Experiment ID
    pub experiment_id: String,
    /// Variant assigned
    pub variant: String,
    /// User/session identifier
    pub session_id: String,
    /// Timestamp
    pub timestamp: SystemTime,
    /// Code node being analyzed
    pub code_node: CodeNode,
    /// Inference result
    pub inference_result: InferenceResult,
    /// Additional metrics
    pub metrics: HashMap<String, f64>,
}

/// A/B testing framework
pub struct ABTestingFramework {
    /// Active experiments
    experiments: Arc<RwLock<HashMap<String, Experiment>>>,
    /// Inference engines for different variants
    inference_engines: Arc<RwLock<HashMap<String, Arc<InferenceEngine>>>>,
    /// Data storage for experiment results
    data_points: Arc<RwLock<Vec<ExperimentDataPoint>>>,
    /// Session assignments (for sticky sessions)
    session_assignments: Arc<RwLock<HashMap<String, HashMap<String, String>>>>,
}

impl ABTestingFramework {
    /// Create a new A/B testing framework
    pub fn new() -> Self {
        Self {
            experiments: Arc::new(RwLock::new(HashMap::new())),
            inference_engines: Arc::new(RwLock::new(HashMap::new())),
            data_points: Arc::new(RwLock::new(Vec::new())),
            session_assignments: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Create a new experiment
    pub async fn create_experiment(&self, config: ABTestConfig) -> Result<String> {
        let experiment_id = Uuid::new_v4().to_string();
        
        // Validate traffic allocation
        let total_allocation: f32 = config.traffic_allocation.allocations.values().sum();
        if (total_allocation - 1.0).abs() > 1e-6 {
            return Err(CodeGraphError::Validation(
                format!("Traffic allocation must sum to 1.0, got {}", total_allocation)
            ));
        }

        // Create variants
        let mut variants = HashMap::new();
        for (variant_name, allocation) in &config.traffic_allocation.allocations {
            let variant = ExperimentVariant {
                name: variant_name.clone(),
                description: format!("Variant {}", variant_name),
                model_id: format!("model_{}", variant_name),
                parameters: HashMap::new(),
                allocation: *allocation,
            };
            variants.insert(variant_name.clone(), variant);
        }

        let experiment = Experiment {
            id: experiment_id.clone(),
            config,
            status: ExperimentStatus::Draft,
            start_time: SystemTime::now(),
            end_time: None,
            variants,
            results: None,
        };

        let mut experiments = self.experiments.write().await;
        experiments.insert(experiment_id.clone(), experiment);

        Ok(experiment_id)
    }

    /// Add an inference engine for a variant
    pub async fn add_variant_engine(&self, variant_id: &str, engine: Arc<InferenceEngine>) {
        let mut engines = self.inference_engines.write().await;
        engines.insert(variant_id.to_string(), engine);
    }

    /// Start an experiment
    pub async fn start_experiment(&self, experiment_id: &str) -> Result<()> {
        let mut experiments = self.experiments.write().await;
        let experiment = experiments
            .get_mut(experiment_id)
            .ok_or_else(|| CodeGraphError::NotFound(format!("Experiment {} not found", experiment_id)))?;

        if !matches!(experiment.status, ExperimentStatus::Draft | ExperimentStatus::Ready) {
            return Err(CodeGraphError::InvalidOperation(
                "Experiment cannot be started in current status".to_string()
            ));
        }

        experiment.status = ExperimentStatus::Running;
        experiment.start_time = SystemTime::now();

        Ok(())
    }

    /// Stop an experiment
    pub async fn stop_experiment(&self, experiment_id: &str) -> Result<()> {
        let mut experiments = self.experiments.write().await;
        let experiment = experiments
            .get_mut(experiment_id)
            .ok_or_else(|| CodeGraphError::NotFound(format!("Experiment {} not found", experiment_id)))?;

        if !matches!(experiment.status, ExperimentStatus::Running) {
            return Err(CodeGraphError::InvalidOperation(
                "Only running experiments can be stopped".to_string()
            ));
        }

        experiment.status = ExperimentStatus::Stopped;
        experiment.end_time = Some(SystemTime::now());

        Ok(())
    }

    /// Assign a variant to a session
    pub async fn assign_variant(&self, experiment_id: &str, session_id: &str) -> Result<String> {
        let experiments = self.experiments.read().await;
        let experiment = experiments
            .get(experiment_id)
            .ok_or_else(|| CodeGraphError::NotFound(format!("Experiment {} not found", experiment_id)))?;

        if !matches!(experiment.status, ExperimentStatus::Running) {
            return Err(CodeGraphError::InvalidOperation(
                "Experiment is not running".to_string()
            ));
        }

        // Check for existing assignment (sticky sessions)
        if experiment.config.traffic_allocation.sticky_sessions {
            let assignments = self.session_assignments.read().await;
            if let Some(experiment_assignments) = assignments.get(experiment_id) {
                if let Some(assigned_variant) = experiment_assignments.get(session_id) {
                    return Ok(assigned_variant.clone());
                }
            }
        }

        // Assign new variant
        let variant = self.allocate_variant(&experiment.config.traffic_allocation, session_id)?;

        // Store assignment for sticky sessions
        if experiment.config.traffic_allocation.sticky_sessions {
            let mut assignments = self.session_assignments.write().await;
            let experiment_assignments = assignments.entry(experiment_id.to_string()).or_insert_with(HashMap::new);
            experiment_assignments.insert(session_id.to_string(), variant.clone());
        }

        Ok(variant)
    }

    /// Run inference with A/B testing
    pub async fn run_inference_with_test(
        &self,
        experiment_id: &str,
        session_id: &str,
        code_node: &CodeNode,
    ) -> Result<InferenceResult> {
        let variant = self.assign_variant(experiment_id, session_id).await?;
        
        let engines = self.inference_engines.read().await;
        let engine = engines
            .get(&variant)
            .ok_or_else(|| CodeGraphError::NotFound(format!("Engine for variant {} not found", variant)))?;

        let start_time = Instant::now();
        let result = engine.predict("default", code_node).await?;
        let inference_time = start_time.elapsed();

        // Record data point
        let mut metrics = HashMap::new();
        metrics.insert("latency_ms".to_string(), inference_time.as_millis() as f64);
        metrics.insert("confidence".to_string(), result.confidence as f64);

        let data_point = ExperimentDataPoint {
            experiment_id: experiment_id.to_string(),
            variant,
            session_id: session_id.to_string(),
            timestamp: SystemTime::now(),
            code_node: code_node.clone(),
            inference_result: result.clone(),
            metrics,
        };

        let mut data_points = self.data_points.write().await;
        data_points.push(data_point);

        Ok(result)
    }

    /// Analyze experiment results
    pub async fn analyze_experiment(&self, experiment_id: &str) -> Result<ExperimentResults> {
        let data_points = self.data_points.read().await;
        let experiment_data: Vec<_> = data_points
            .iter()
            .filter(|dp| dp.experiment_id == experiment_id)
            .collect();

        if experiment_data.is_empty() {
            return Err(CodeGraphError::NotFound(
                "No data points found for experiment".to_string()
            ));
        }

        // Group data by variant
        let mut variant_data: HashMap<String, Vec<&ExperimentDataPoint>> = HashMap::new();
        for data_point in &experiment_data {
            variant_data.entry(data_point.variant.clone()).or_default().push(data_point);
        }

        // Calculate variant results
        let mut variant_results = HashMap::new();
        for (variant_name, data) in &variant_data {
            let results = self.calculate_variant_results(variant_name, data).await?;
            variant_results.insert(variant_name.clone(), results);
        }

        // Perform statistical comparisons
        let comparisons = self.perform_statistical_comparisons(&variant_data).await?;

        // Determine winner
        let winner = self.determine_winner(&comparisons);

        // Calculate summary
        let summary = ExperimentSummary {
            total_samples: experiment_data.len(),
            duration: experiment_data.first().unwrap().timestamp
                .duration_since(UNIX_EPOCH).unwrap(),
            winner: winner.clone(),
            confidence: 0.95, // Placeholder
            effect_size: 0.1,  // Placeholder
            significant: comparisons.iter().any(|c| c.significant),
        };

        // Generate recommendations
        let recommendations = self.generate_recommendations(&summary, &comparisons);

        Ok(ExperimentResults {
            summary,
            variant_results,
            comparisons,
            recommendations,
        })
    }

    /// Get experiment status
    pub async fn get_experiment(&self, experiment_id: &str) -> Result<Experiment> {
        let experiments = self.experiments.read().await;
        experiments
            .get(experiment_id)
            .cloned()
            .ok_or_else(|| CodeGraphError::NotFound(format!("Experiment {} not found", experiment_id)))
    }

    /// List all experiments
    pub async fn list_experiments(&self) -> Vec<Experiment> {
        let experiments = self.experiments.read().await;
        experiments.values().cloned().collect()
    }

    // Private helper methods

    fn allocate_variant(&self, allocation: &TrafficAllocation, session_id: &str) -> Result<String> {
        match allocation.strategy {
            AllocationStrategy::Random => {
                use std::collections::hash_map::DefaultHasher;
                use std::hash::{Hash, Hasher};
                
                let mut hasher = DefaultHasher::new();
                SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos().hash(&mut hasher);
                let hash = hasher.finish();
                let random_value = (hash as f64) / (u64::MAX as f64);
                
                let mut cumulative = 0.0;
                for (variant, weight) in &allocation.allocations {
                    cumulative += weight;
                    if random_value <= cumulative as f64 {
                        return Ok(variant.clone());
                    }
                }
                
                // Fallback to first variant
                allocation.allocations.keys().next().cloned()
                    .ok_or_else(|| CodeGraphError::Configuration("No variants configured".to_string()))
            }
            AllocationStrategy::HashBased => {
                use std::collections::hash_map::DefaultHasher;
                use std::hash::{Hash, Hasher};
                
                let mut hasher = DefaultHasher::new();
                session_id.hash(&mut hasher);
                let hash = hasher.finish();
                let hash_value = (hash as f64) / (u64::MAX as f64);
                
                let mut cumulative = 0.0;
                for (variant, weight) in &allocation.allocations {
                    cumulative += weight;
                    if hash_value <= cumulative as f64 {
                        return Ok(variant.clone());
                    }
                }
                
                // Fallback to first variant
                allocation.allocations.keys().next().cloned()
                    .ok_or_else(|| CodeGraphError::Configuration("No variants configured".to_string()))
            }
            _ => {
                // For other strategies, use hash-based for now
                self.allocate_variant(&TrafficAllocation {
                    allocations: allocation.allocations.clone(),
                    strategy: AllocationStrategy::HashBased,
                    sticky_sessions: allocation.sticky_sessions,
                }, session_id)
            }
        }
    }

    async fn calculate_variant_results(&self, variant_name: &str, data: &[&ExperimentDataPoint]) -> Result<VariantResults> {
        let sample_size = data.len();
        
        // Calculate metrics
        let mut metrics = HashMap::new();
        
        // Latency metric
        let latencies: Vec<f64> = data.iter()
            .filter_map(|dp| dp.metrics.get("latency_ms"))
            .copied()
            .collect();
        
        if !latencies.is_empty() {
            let latency_summary = self.calculate_metric_summary("latency_ms", &latencies);
            metrics.insert("latency_ms".to_string(), latency_summary);
        }

        // Confidence metric
        let confidences: Vec<f64> = data.iter()
            .filter_map(|dp| dp.metrics.get("confidence"))
            .copied()
            .collect();
        
        if !confidences.is_empty() {
            let confidence_summary = self.calculate_metric_summary("confidence", &confidences);
            metrics.insert("confidence".to_string(), confidence_summary);
        }

        // Create time series (simplified)
        let time_series = vec![
            TimeSeriesPoint {
                timestamp: SystemTime::now(),
                values: HashMap::new(),
                sample_count: sample_size,
            }
        ];

        Ok(VariantResults {
            variant_name: variant_name.to_string(),
            sample_size,
            metrics,
            time_series,
        })
    }

    fn calculate_metric_summary(&self, name: &str, values: &[f64]) -> MetricSummary {
        let mut sorted_values = values.to_vec();
        sorted_values.sort_by(|a, b| a.partial_cmp(b).unwrap());
        
        let mean = values.iter().sum::<f64>() / values.len() as f64;
        let variance = values.iter()
            .map(|x| (x - mean).powi(2))
            .sum::<f64>() / values.len() as f64;
        let std_dev = variance.sqrt();
        
        let median = if sorted_values.len() % 2 == 0 {
            (sorted_values[sorted_values.len() / 2 - 1] + sorted_values[sorted_values.len() / 2]) / 2.0
        } else {
            sorted_values[sorted_values.len() / 2]
        };
        
        let p95_idx = ((sorted_values.len() as f64) * 0.95) as usize;
        let p99_idx = ((sorted_values.len() as f64) * 0.99) as usize;
        
        let p95 = sorted_values.get(p95_idx).copied().unwrap_or(sorted_values.last().copied().unwrap_or(0.0));
        let p99 = sorted_values.get(p99_idx).copied().unwrap_or(sorted_values.last().copied().unwrap_or(0.0));
        
        let min = sorted_values.first().copied().unwrap_or(0.0);
        let max = sorted_values.last().copied().unwrap_or(0.0);
        
        // Simple confidence interval (±1.96 * std_err for 95% CI)
        let std_err = std_dev / (values.len() as f64).sqrt();
        let margin = 1.96 * std_err;
        let confidence_interval = (mean - margin, mean + margin);

        MetricSummary {
            name: name.to_string(),
            mean,
            std_dev,
            median,
            p95,
            p99,
            min,
            max,
            confidence_interval,
        }
    }

    async fn perform_statistical_comparisons(&self, variant_data: &HashMap<String, Vec<&ExperimentDataPoint>>) -> Result<Vec<VariantComparison>> {
        let mut comparisons = Vec::new();
        let variants: Vec<_> = variant_data.keys().collect();
        
        // Compare all pairs of variants
        for i in 0..variants.len() {
            for j in (i + 1)..variants.len() {
                let baseline = variants[i];
                let treatment = variants[j];
                
                // Compare latency metric
                let baseline_latencies: Vec<f64> = variant_data[baseline].iter()
                    .filter_map(|dp| dp.metrics.get("latency_ms"))
                    .copied()
                    .collect();
                
                let treatment_latencies: Vec<f64> = variant_data[treatment].iter()
                    .filter_map(|dp| dp.metrics.get("latency_ms"))
                    .copied()
                    .collect();
                
                if !baseline_latencies.is_empty() && !treatment_latencies.is_empty() {
                    let comparison = self.perform_t_test(&baseline_latencies, &treatment_latencies, baseline, treatment, "latency_ms");
                    comparisons.push(comparison);
                }
            }
        }
        
        Ok(comparisons)
    }

    fn perform_t_test(&self, baseline: &[f64], treatment: &[f64], baseline_name: &str, treatment_name: &str, metric: &str) -> VariantComparison {
        // Simplified t-test implementation
        let baseline_mean = baseline.iter().sum::<f64>() / baseline.len() as f64;
        let treatment_mean = treatment.iter().sum::<f64>() / treatment.len() as f64;
        
        let baseline_var = baseline.iter()
            .map(|x| (x - baseline_mean).powi(2))
            .sum::<f64>() / (baseline.len() - 1) as f64;
        
        let treatment_var = treatment.iter()
            .map(|x| (x - treatment_mean).powi(2))
            .sum::<f64>() / (treatment.len() - 1) as f64;
        
        let pooled_var = ((baseline.len() - 1) as f64 * baseline_var + (treatment.len() - 1) as f64 * treatment_var) 
            / (baseline.len() + treatment.len() - 2) as f64;
        
        let std_err = (pooled_var * (1.0 / baseline.len() as f64 + 1.0 / treatment.len() as f64)).sqrt();
        let t_stat = (treatment_mean - baseline_mean) / std_err;
        
        // Simplified p-value calculation (placeholder)
        let p_value = if t_stat.abs() > 1.96 { 0.04 } else { 0.1 };
        
        let effect_size = (treatment_mean - baseline_mean) / (pooled_var.sqrt());
        let effect_ci = (effect_size - 0.1, effect_size + 0.1); // Placeholder
        
        VariantComparison {
            baseline: baseline_name.to_string(),
            treatment: treatment_name.to_string(),
            metric: metric.to_string(),
            test_statistic: t_stat,
            p_value,
            effect_size,
            effect_confidence_interval: effect_ci,
            significant: p_value < 0.05,
            test_type: StatisticalTest::TTest,
        }
    }

    fn determine_winner(&self, comparisons: &[VariantComparison]) -> Option<String> {
        // Simple winner determination based on significant improvements
        for comparison in comparisons {
            if comparison.significant && comparison.effect_size < 0.0 { // Lower latency is better
                return Some(comparison.treatment.clone());
            }
        }
        None
    }

    fn generate_recommendations(&self, _summary: &ExperimentSummary, comparisons: &[VariantComparison]) -> Vec<String> {
        let mut recommendations = Vec::new();
        
        for comparison in comparisons {
            if comparison.significant {
                if comparison.effect_size < 0.0 && comparison.metric == "latency_ms" {
                    recommendations.push(format!(
                        "Recommend deploying variant '{}' as it shows significantly lower latency ({:.2}ms improvement)",
                        comparison.treatment,
                        comparison.effect_size.abs()
                    ));
                }
            } else {
                recommendations.push(format!(
                    "No significant difference found between '{}' and '{}' for {}",
                    comparison.baseline, comparison.treatment, comparison.metric
                ));
            }
        }
        
        if recommendations.is_empty() {
            recommendations.push("No clear winner identified. Consider collecting more data.".to_string());
        }
        
        recommendations
    }
}

impl Default for ABTestingFramework {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[tokio::test]
    async fn test_experiment_creation() {
        let framework = ABTestingFramework::new();
        
        let mut allocations = HashMap::new();
        allocations.insert("control".to_string(), 0.5);
        allocations.insert("treatment".to_string(), 0.5);
        
        let config = ABTestConfig {
            name: "Test Experiment".to_string(),
            description: "A test experiment".to_string(),
            traffic_allocation: TrafficAllocation {
                allocations,
                strategy: AllocationStrategy::Random,
                sticky_sessions: true,
            },
            duration: Duration::from_secs(3600),
            statistical_config: StatisticalConfig::default(),
            metrics: vec![ExperimentMetric::Latency, ExperimentMetric::Accuracy],
            sample_size: SampleSizeConfig {
                min_sample_size: 100,
                max_sample_size: 1000,
                early_stopping: EarlyStoppingConfig {
                    enabled: false,
                    check_interval: Duration::from_secs(300),
                    min_samples: 50,
                    futility_boundary: 0.1,
                    efficacy_boundary: 0.9,
                },
                calculation_method: SampleSizeMethod::Fixed,
            },
        };
        
        let experiment_id = framework.create_experiment(config).await.unwrap();
        assert!(!experiment_id.is_empty());
        
        let experiment = framework.get_experiment(&experiment_id).await.unwrap();
        assert_eq!(experiment.id, experiment_id);
        assert_eq!(experiment.variants.len(), 2);
    }

    #[tokio::test]
    async fn test_variant_assignment() {
        let framework = ABTestingFramework::new();
        
        let mut allocations = HashMap::new();
        allocations.insert("A".to_string(), 0.5);
        allocations.insert("B".to_string(), 0.5);
        
        let config = ABTestConfig {
            name: "Assignment Test".to_string(),
            description: "Test variant assignment".to_string(),
            traffic_allocation: TrafficAllocation {
                allocations,
                strategy: AllocationStrategy::HashBased,
                sticky_sessions: true,
            },
            duration: Duration::from_secs(3600),
            statistical_config: StatisticalConfig::default(),
            metrics: vec![ExperimentMetric::Latency],
            sample_size: SampleSizeConfig {
                min_sample_size: 10,
                max_sample_size: 100,
                early_stopping: EarlyStoppingConfig {
                    enabled: false,
                    check_interval: Duration::from_secs(300),
                    min_samples: 5,
                    futility_boundary: 0.1,
                    efficacy_boundary: 0.9,
                },
                calculation_method: SampleSizeMethod::Fixed,
            },
        };
        
        let experiment_id = framework.create_experiment(config).await.unwrap();
        framework.start_experiment(&experiment_id).await.unwrap();
        
        let variant1 = framework.assign_variant(&experiment_id, "user1").await.unwrap();
        let variant2 = framework.assign_variant(&experiment_id, "user1").await.unwrap();
        
        // Sticky sessions should return the same variant
        assert_eq!(variant1, variant2);
        assert!(variant1 == "A" || variant1 == "B");
    }

    #[test]
    fn test_metric_summary_calculation() {
        let framework = ABTestingFramework::new();
        let values = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0];
        
        let summary = framework.calculate_metric_summary("test_metric", &values);
        
        assert_eq!(summary.mean, 5.5);
        assert_eq!(summary.median, 5.5);
        assert_eq!(summary.min, 1.0);
        assert_eq!(summary.max, 10.0);
        assert!(summary.std_dev > 0.0);
    }
}