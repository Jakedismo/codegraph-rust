//! ML Pipeline orchestration for code analysis
//!
//! This module provides a unified pipeline that orchestrates feature extraction,
//! model training, inference optimization, and A/B testing for a complete
//! machine learning workflow in code analysis.

use crate::ml::ab_testing::{ABTestConfig, ABTestingFramework, ExperimentResults};
use crate::ml::features::{CodeFeatures, FeatureConfig, FeatureExtractor};
use crate::ml::inference::{InferenceConfig, InferenceEngine, InferenceResult};
use crate::ml::training::{ModelTrainer, TrainingConfig, TrainingResults, TrainingTarget};
use codegraph_core::{CodeGraphError, CodeNode, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use uuid::Uuid;

/// Configuration for the complete ML pipeline
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MLPipelineConfig {
    /// Feature extraction configuration
    pub feature_config: FeatureConfig,
    /// Training configuration
    pub training_config: TrainingConfig,
    /// Inference configuration
    pub inference_config: InferenceConfig,
    /// Pipeline settings
    pub pipeline_settings: PipelineSettings,
}

/// Pipeline-specific settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineSettings {
    /// Pipeline name
    pub name: String,
    /// Pipeline version
    pub version: String,
    /// Enable automatic retraining
    pub auto_retrain: bool,
    /// Retraining interval
    pub retrain_interval: Duration,
    /// Model performance threshold for retraining
    pub performance_threshold: f32,
    /// Enable continuous learning
    pub continuous_learning: bool,
    /// Data drift detection
    pub drift_detection: DriftDetectionConfig,
    /// Model monitoring
    pub monitoring: PipelineMonitoringConfig,
}

impl Default for PipelineSettings {
    fn default() -> Self {
        Self {
            name: "CodeAnalysis".to_string(),
            version: "1.0.0".to_string(),
            auto_retrain: true,
            retrain_interval: Duration::from_secs(24 * 3600), // 24 hours
            performance_threshold: 0.8,
            continuous_learning: false,
            drift_detection: DriftDetectionConfig::default(),
            monitoring: PipelineMonitoringConfig::default(),
        }
    }
}

/// Data drift detection configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriftDetectionConfig {
    /// Enable drift detection
    pub enabled: bool,
    /// Detection method
    pub method: DriftDetectionMethod,
    /// Threshold for drift detection
    pub threshold: f32,
    /// Window size for comparison
    pub window_size: usize,
    /// Check interval
    pub check_interval: Duration,
}

impl Default for DriftDetectionConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            method: DriftDetectionMethod::KLDivergence,
            threshold: 0.1,
            window_size: 1000,
            check_interval: Duration::from_secs(3600), // 1 hour
        }
    }
}

/// Data drift detection methods
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DriftDetectionMethod {
    /// Kolmogorov-Smirnov test
    KolmogorovSmirnov,
    /// KL divergence
    KLDivergence,
    /// Population Stability Index
    PSI,
    /// Jensen-Shannon divergence
    JensenShannon,
}

/// Pipeline monitoring configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineMonitoringConfig {
    /// Enable monitoring
    pub enabled: bool,
    /// Metrics to track
    pub metrics: Vec<PipelineMetric>,
    /// Alert thresholds
    pub alert_thresholds: HashMap<String, f32>,
    /// Monitoring interval
    pub monitoring_interval: Duration,
}

impl Default for PipelineMonitoringConfig {
    fn default() -> Self {
        let mut alert_thresholds = HashMap::new();
        alert_thresholds.insert("accuracy".to_string(), 0.7);
        alert_thresholds.insert("latency_p95".to_string(), 1000.0);
        alert_thresholds.insert("error_rate".to_string(), 0.05);

        Self {
            enabled: true,
            metrics: vec![
                PipelineMetric::ModelAccuracy,
                PipelineMetric::InferenceLatency,
                PipelineMetric::ThroughputRPS,
                PipelineMetric::ErrorRate,
                PipelineMetric::DataDrift,
            ],
            alert_thresholds,
            monitoring_interval: Duration::from_secs(300), // 5 minutes
        }
    }
}

/// Pipeline metrics to monitor
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PipelineMetric {
    /// Model accuracy over time
    ModelAccuracy,
    /// Inference latency percentiles
    InferenceLatency,
    /// Prediction throughput
    ThroughputRPS,
    /// Error rate
    ErrorRate,
    /// Data drift score
    DataDrift,
    /// Feature importance drift
    FeatureDrift,
    /// Model staleness
    ModelStaleness,
    /// Resource utilization
    ResourceUtilization,
}

/// Pipeline execution status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PipelineStatus {
    /// Pipeline is initializing
    Initializing,
    /// Pipeline is ready for inference
    Ready,
    /// Pipeline is training
    Training,
    /// Pipeline is evaluating
    Evaluating,
    /// Pipeline is updating
    Updating,
    /// Pipeline has failed
    Failed,
    /// Pipeline is paused
    Paused,
}

/// Pipeline execution context
#[derive(Debug, Clone)]
pub struct PipelineContext {
    /// Pipeline ID
    pub pipeline_id: String,
    /// Current status
    pub status: PipelineStatus,
    /// Configuration
    pub config: MLPipelineConfig,
    /// Training history
    pub training_history: Vec<TrainingResults>,
    /// Active experiments
    pub active_experiments: Vec<String>,
    /// Performance metrics
    pub performance_metrics: HashMap<String, f32>,
}

/// Complete ML pipeline for code analysis
pub struct MLPipeline {
    /// Pipeline context
    context: Arc<RwLock<PipelineContext>>,
    /// Feature extractor
    feature_extractor: Arc<FeatureExtractor>,
    /// Model trainer
    model_trainer: Arc<RwLock<ModelTrainer>>,
    /// Inference engine
    inference_engine: Arc<RwLock<InferenceEngine>>,
    /// A/B testing framework
    ab_testing: Arc<ABTestingFramework>,
    /// Trained models registry
    models: Arc<RwLock<HashMap<String, Arc<crate::ml::training::TrainedModel>>>>,
}

/// Pipeline builder for configuration
pub struct MLPipelineBuilder {
    config: MLPipelineConfig,
    embedding_generator: Option<Arc<crate::EmbeddingGenerator>>,
}

impl MLPipelineBuilder {
    /// Create a new pipeline builder
    pub fn new() -> Self {
        Self {
            config: MLPipelineConfig {
                feature_config: FeatureConfig::default(),
                training_config: TrainingConfig {
                    model_type: crate::ml::training::ModelType::QualityClassifier,
                    hyperparameters: crate::ml::training::TrainingHyperparameters::default(),
                    data_config: crate::ml::training::DataConfig::default(),
                    validation_config: crate::ml::training::ValidationConfig::default(),
                    output_config: crate::ml::training::OutputConfig {
                        model_path: "models/".to_string(),
                        save_checkpoints: true,
                        checkpoint_frequency: 10,
                        export_for_inference: true,
                    },
                },
                inference_config: InferenceConfig::default(),
                pipeline_settings: PipelineSettings::default(),
            },
            embedding_generator: None,
        }
    }

    /// Set feature configuration
    pub fn with_feature_config(mut self, config: FeatureConfig) -> Self {
        self.config.feature_config = config;
        self
    }

    /// Set training configuration
    pub fn with_training_config(mut self, config: TrainingConfig) -> Self {
        self.config.training_config = config;
        self
    }

    /// Set inference configuration
    pub fn with_inference_config(mut self, config: InferenceConfig) -> Self {
        self.config.inference_config = config;
        self
    }

    /// Set pipeline settings
    pub fn with_pipeline_settings(mut self, settings: PipelineSettings) -> Self {
        self.config.pipeline_settings = settings;
        self
    }

    /// Set embedding generator
    pub fn with_embedding_generator(mut self, generator: Arc<crate::EmbeddingGenerator>) -> Self {
        self.embedding_generator = Some(generator);
        self
    }

    /// Build the ML pipeline
    pub fn build(self) -> Result<MLPipeline> {
        let embedding_generator = self
            .embedding_generator
            .unwrap_or_else(|| Arc::new(crate::EmbeddingGenerator::default()));

        let feature_extractor = Arc::new(FeatureExtractor::new(
            self.config.feature_config.clone(),
            embedding_generator,
        ));

        let model_trainer = Arc::new(RwLock::new(ModelTrainer::new(
            self.config.training_config.clone(),
            feature_extractor.clone(),
        )));

        let inference_engine = Arc::new(RwLock::new(InferenceEngine::new(
            self.config.inference_config.clone(),
            feature_extractor.clone(),
        )));

        let ab_testing = Arc::new(ABTestingFramework::new());

        let context = Arc::new(RwLock::new(PipelineContext {
            pipeline_id: Uuid::new_v4().to_string(),
            status: PipelineStatus::Initializing,
            config: self.config,
            training_history: Vec::new(),
            active_experiments: Vec::new(),
            performance_metrics: HashMap::new(),
        }));

        Ok(MLPipeline {
            context,
            feature_extractor,
            model_trainer,
            inference_engine,
            ab_testing,
            models: Arc::new(RwLock::new(HashMap::new())),
        })
    }
}

impl MLPipeline {
    /// Create a new pipeline builder
    pub fn builder() -> MLPipelineBuilder {
        MLPipelineBuilder::new()
    }

    /// Initialize the pipeline
    pub async fn initialize(&self) -> Result<()> {
        let mut context = self.context.write().await;
        context.status = PipelineStatus::Ready;
        Ok(())
    }

    /// Train a new model with the pipeline
    pub async fn train_model(
        &self,
        dataset_name: &str,
        nodes: &[CodeNode],
        targets: Vec<TrainingTarget>,
        model_name: &str,
    ) -> Result<TrainingResults> {
        // Update status
        {
            let mut context = self.context.write().await;
            context.status = PipelineStatus::Training;
        }

        // Prepare training dataset
        let trainer = self.model_trainer.read().await;
        trainer
            .prepare_dataset(nodes, targets, dataset_name)
            .await?;

        // Train the model
        let results = trainer.train_model(dataset_name, model_name).await?;

        // Get the trained model and add to inference engine
        if let Some(trained_model) = trainer.get_model(model_name).await {
            let inference_engine = self.inference_engine.write().await;
            inference_engine
                .add_model(model_name, trained_model.clone())
                .await?;

            // Store in models registry
            let mut models = self.models.write().await;
            models.insert(model_name.to_string(), Arc::new(trained_model));
        }

        // Update training history
        {
            let mut context = self.context.write().await;
            context.training_history.push(results.clone());
            context.status = PipelineStatus::Ready;
        }

        Ok(results)
    }

    /// Perform inference on a code node
    pub async fn predict(&self, model_name: &str, code_node: &CodeNode) -> Result<InferenceResult> {
        let inference_engine = self.inference_engine.read().await;
        inference_engine.predict(model_name, code_node).await
    }

    /// Perform batch inference
    pub async fn predict_batch(
        &self,
        model_name: &str,
        code_nodes: &[CodeNode],
    ) -> Result<Vec<InferenceResult>> {
        let inference_engine = self.inference_engine.read().await;
        inference_engine.predict_batch(model_name, code_nodes).await
    }

    /// Start an A/B test experiment
    pub async fn start_ab_test(&self, config: ABTestConfig) -> Result<String> {
        let experiment_id = self.ab_testing.create_experiment(config).await?;

        // Add to active experiments
        {
            let mut context = self.context.write().await;
            context.active_experiments.push(experiment_id.clone());
        }

        self.ab_testing.start_experiment(&experiment_id).await?;
        Ok(experiment_id)
    }

    /// Run inference with A/B testing
    pub async fn predict_with_ab_test(
        &self,
        experiment_id: &str,
        session_id: &str,
        code_node: &CodeNode,
    ) -> Result<InferenceResult> {
        self.ab_testing
            .run_inference_with_test(experiment_id, session_id, code_node)
            .await
    }

    /// Analyze A/B test results
    pub async fn analyze_ab_test(&self, experiment_id: &str) -> Result<ExperimentResults> {
        self.ab_testing.analyze_experiment(experiment_id).await
    }

    /// Extract features from code nodes
    pub async fn extract_features(&self, code_node: &CodeNode) -> Result<CodeFeatures> {
        self.feature_extractor.extract_features(code_node).await
    }

    /// Extract features from multiple code nodes
    pub async fn extract_features_batch(
        &self,
        code_nodes: &[CodeNode],
    ) -> Result<Vec<CodeFeatures>> {
        self.feature_extractor
            .extract_features_batch(code_nodes)
            .await
    }

    /// Get pipeline status
    pub async fn get_status(&self) -> PipelineStatus {
        let context = self.context.read().await;
        context.status.clone()
    }

    /// Get pipeline context
    pub async fn get_context(&self) -> PipelineContext {
        let context = self.context.read().await;
        context.clone()
    }

    /// Get inference metrics
    pub async fn get_inference_metrics(&self) -> crate::ml::inference::InferenceMetrics {
        let inference_engine = self.inference_engine.read().await;
        inference_engine.get_metrics().await
    }

    /// Save pipeline configuration
    pub async fn save_config<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let context = self.context.read().await;
        let config_json = serde_json::to_string_pretty(&context.config)
            .map_err(|e| CodeGraphError::Vector(format!("Failed to serialize config: {}", e)))?;

        tokio::fs::write(path, config_json)
            .await
            .map_err(|e| CodeGraphError::Vector(format!("Failed to write config file: {}", e)))?;

        Ok(())
    }

    /// Load pipeline configuration
    pub async fn load_config<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let config_content = tokio::fs::read_to_string(path)
            .await
            .map_err(|e| CodeGraphError::Vector(format!("Failed to read config file: {}", e)))?;

        let config: MLPipelineConfig = serde_json::from_str(&config_content)
            .map_err(|e| CodeGraphError::Vector(format!("Failed to parse config: {}", e)))?;

        let mut context = self.context.write().await;
        context.config = config;

        Ok(())
    }

    /// Save trained model
    pub async fn save_model<P: AsRef<Path>>(&self, model_name: &str, path: P) -> Result<()> {
        let trainer = self.model_trainer.read().await;
        trainer.save_model(model_name, path.as_ref()).await
    }

    /// Load trained model
    pub async fn load_model<P: AsRef<Path>>(&self, model_name: &str, path: P) -> Result<()> {
        let trainer = self.model_trainer.read().await;
        trainer.load_model(model_name, path.as_ref()).await?;

        // Add to inference engine if available
        if let Some(trained_model) = trainer.get_model(model_name).await {
            let inference_engine = self.inference_engine.write().await;
            inference_engine
                .add_model(model_name, trained_model.clone())
                .await?;

            // Store in models registry
            let mut models = self.models.write().await;
            models.insert(model_name.to_string(), Arc::new(trained_model));
        }

        Ok(())
    }

    /// Monitor pipeline performance
    pub async fn monitor_performance(&self) -> Result<HashMap<String, f32>> {
        let mut metrics = HashMap::new();

        // Get inference metrics
        let inference_metrics = self.get_inference_metrics().await;
        metrics.insert(
            "avg_latency_us".to_string(),
            inference_metrics.avg_latency_us,
        );
        metrics.insert(
            "throughput_rps".to_string(),
            inference_metrics.throughput_rps,
        );
        metrics.insert("error_rate".to_string(), inference_metrics.error_rate);
        metrics.insert(
            "cache_hit_rate".to_string(),
            inference_metrics.cache_hit_rate,
        );

        // Update context metrics
        {
            let mut context = self.context.write().await;
            context.performance_metrics.extend(metrics.clone());
        }

        Ok(metrics)
    }

    /// Detect data drift
    pub async fn detect_data_drift(
        &self,
        recent_features: &[CodeFeatures],
        baseline_features: &[CodeFeatures],
    ) -> Result<f32> {
        // Simplified drift detection using feature distribution comparison
        if recent_features.is_empty() || baseline_features.is_empty() {
            return Ok(0.0);
        }

        // Compare feature distributions (simplified KL divergence approximation)
        let mut total_drift = 0.0;
        let mut feature_count = 0;

        // Compare syntactic features
        if let (Some(recent_syntactic), Some(baseline_syntactic)) = (
            recent_features.first().and_then(|f| f.syntactic.as_ref()),
            baseline_features.first().and_then(|f| f.syntactic.as_ref()),
        ) {
            let recent_avg_tokens = recent_features
                .iter()
                .filter_map(|f| f.syntactic.as_ref().map(|s| s.token_count as f32))
                .sum::<f32>()
                / recent_features.len() as f32;

            let baseline_avg_tokens = baseline_features
                .iter()
                .filter_map(|f| f.syntactic.as_ref().map(|s| s.token_count as f32))
                .sum::<f32>()
                / baseline_features.len() as f32;

            let drift =
                (recent_avg_tokens - baseline_avg_tokens).abs() / baseline_avg_tokens.max(1.0);
            total_drift += drift;
            feature_count += 1;
        }

        // Compare complexity features
        if !recent_features.is_empty() && !baseline_features.is_empty() {
            let recent_avg_complexity = recent_features
                .iter()
                .filter_map(|f| {
                    f.complexity
                        .as_ref()
                        .map(|c| c.cyclomatic_complexity as f32)
                })
                .sum::<f32>()
                / recent_features.len() as f32;

            let baseline_avg_complexity = baseline_features
                .iter()
                .filter_map(|f| {
                    f.complexity
                        .as_ref()
                        .map(|c| c.cyclomatic_complexity as f32)
                })
                .sum::<f32>()
                / baseline_features.len() as f32;

            let drift = (recent_avg_complexity - baseline_avg_complexity).abs()
                / baseline_avg_complexity.max(1.0);
            total_drift += drift;
            feature_count += 1;
        }

        Ok(if feature_count > 0 {
            total_drift / feature_count as f32
        } else {
            0.0
        })
    }

    /// Check if retraining is needed
    pub async fn should_retrain(&self) -> Result<bool> {
        let context = self.context.read().await;

        // Check performance threshold
        if let Some(accuracy) = context.performance_metrics.get("accuracy") {
            if *accuracy < context.config.pipeline_settings.performance_threshold {
                return Ok(true);
            }
        }

        // Check time since last training
        if let Some(last_training) = context.training_history.last() {
            let training_age =
                chrono::Utc::now().signed_duration_since(last_training.model_metadata.timestamp);
            if training_age.num_seconds() as u64
                > context.config.pipeline_settings.retrain_interval.as_secs()
            {
                return Ok(true);
            }
        }

        Ok(false)
    }

    /// Trigger automatic retraining
    pub async fn trigger_retrain(
        &self,
        dataset_name: &str,
        nodes: &[CodeNode],
        targets: Vec<TrainingTarget>,
    ) -> Result<TrainingResults> {
        let model_name = format!("retrained_model_{}", chrono::Utc::now().timestamp());
        self.train_model(dataset_name, nodes, targets, &model_name)
            .await
    }

    /// Pause the pipeline
    pub async fn pause(&self) -> Result<()> {
        let mut context = self.context.write().await;
        context.status = PipelineStatus::Paused;
        Ok(())
    }

    /// Resume the pipeline
    pub async fn resume(&self) -> Result<()> {
        let mut context = self.context.write().await;
        context.status = PipelineStatus::Ready;
        Ok(())
    }

    /// Shutdown the pipeline
    pub async fn shutdown(&self) -> Result<()> {
        // Stop all active experiments
        {
            let context = self.context.read().await;
            for experiment_id in &context.active_experiments {
                let _ = self.ab_testing.stop_experiment(experiment_id).await;
            }
        }

        // Clear cache
        let inference_engine = self.inference_engine.read().await;
        inference_engine.clear_cache().await;

        Ok(())
    }
}

impl Default for MLPipelineBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use codegraph_core::{Language, NodeType};

    #[tokio::test]
    async fn test_pipeline_creation() {
        let pipeline = MLPipeline::builder()
            .with_pipeline_settings(PipelineSettings {
                name: "Test Pipeline".to_string(),
                version: "1.0.0".to_string(),
                ..PipelineSettings::default()
            })
            .build()
            .unwrap();

        pipeline.initialize().await.unwrap();

        let status = pipeline.get_status().await;
        assert!(matches!(status, PipelineStatus::Ready));
    }

    #[tokio::test]
    async fn test_feature_extraction() {
        let pipeline = MLPipeline::builder().build().unwrap();
        pipeline.initialize().await.unwrap();

        let code_node = CodeNode {
            id: "test_node".to_string(),
            name: "test_function".to_string(),
            language: Some(Language::Rust),
            node_type: Some(NodeType::Function),
            content: Some("fn test() { println!(\"Hello\"); }".to_string()),
            children: None,
        };

        let features = pipeline.extract_features(&code_node).await.unwrap();
        assert_eq!(features.node_id, "test_node");
    }

    #[tokio::test]
    async fn test_pipeline_config_serialization() {
        let pipeline = MLPipeline::builder().build().unwrap();
        pipeline.initialize().await.unwrap();

        let temp_dir = tempfile::tempdir().unwrap();
        let config_path = temp_dir.path().join("config.json");

        // Save and load configuration
        pipeline.save_config(&config_path).await.unwrap();
        pipeline.load_config(&config_path).await.unwrap();

        // Verify config is still valid
        let context = pipeline.get_context().await;
        assert_eq!(context.config.pipeline_settings.name, "CodeAnalysis");
    }

    #[tokio::test]
    async fn test_performance_monitoring() {
        let pipeline = MLPipeline::builder().build().unwrap();
        pipeline.initialize().await.unwrap();

        let metrics = pipeline.monitor_performance().await.unwrap();

        // Should have basic inference metrics
        assert!(metrics.contains_key("avg_latency_us"));
        assert!(metrics.contains_key("throughput_rps"));
        assert!(metrics.contains_key("error_rate"));
    }
}
