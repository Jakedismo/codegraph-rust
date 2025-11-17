//! Model training infrastructure for code analysis
//!
//! This module provides comprehensive training infrastructure for domain-specific
//! machine learning models used in code analysis. It supports various model types
//! including classification, regression, and embedding models.

use crate::ml::features::{CodeFeatures, FeatureExtractor};
use codegraph_core::{CodeGraphError, CodeNode, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Training configuration for ML models
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainingConfig {
    /// Type of model to train
    pub model_type: ModelType,
    /// Training hyperparameters
    pub hyperparameters: TrainingHyperparameters,
    /// Training data configuration
    pub data_config: DataConfig,
    /// Validation configuration
    pub validation_config: ValidationConfig,
    /// Model output configuration
    pub output_config: OutputConfig,
}

/// Types of models that can be trained
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ModelType {
    /// Code quality classifier (good/bad/needs_review)
    QualityClassifier,
    /// Complexity predictor (cyclomatic/cognitive complexity)
    ComplexityPredictor,
    /// Bug detection classifier
    BugDetector,
    /// Code similarity model
    SimilarityModel,
    /// Performance predictor
    PerformancePredictor,
    /// Security vulnerability detector
    SecurityDetector,
}

/// Training hyperparameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainingHyperparameters {
    /// Learning rate
    pub learning_rate: f32,
    /// Batch size
    pub batch_size: usize,
    /// Number of epochs
    pub epochs: usize,
    /// L2 regularization strength
    pub l2_regularization: f32,
    /// Dropout rate
    pub dropout_rate: f32,
    /// Early stopping patience
    pub early_stopping_patience: usize,
    /// Validation frequency (epochs)
    pub validation_frequency: usize,
}

impl Default for TrainingHyperparameters {
    fn default() -> Self {
        Self {
            learning_rate: 0.001,
            batch_size: 32,
            epochs: 100,
            l2_regularization: 0.01,
            dropout_rate: 0.1,
            early_stopping_patience: 10,
            validation_frequency: 5,
        }
    }
}

/// Data configuration for training
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataConfig {
    /// Training data split ratio
    pub train_split: f32,
    /// Validation data split ratio
    pub validation_split: f32,
    /// Test data split ratio
    pub test_split: f32,
    /// Whether to shuffle data
    pub shuffle: bool,
    /// Random seed for reproducibility
    pub random_seed: u64,
    /// Data augmentation settings
    pub augmentation: DataAugmentation,
}

impl Default for DataConfig {
    fn default() -> Self {
        Self {
            train_split: 0.7,
            validation_split: 0.15,
            test_split: 0.15,
            shuffle: true,
            random_seed: 42,
            augmentation: DataAugmentation::default(),
        }
    }
}

/// Data augmentation configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataAugmentation {
    /// Whether to enable data augmentation
    pub enabled: bool,
    /// Noise injection rate for embeddings
    pub noise_rate: f32,
    /// Feature dropout rate
    pub feature_dropout_rate: f32,
    /// Synthetic sample generation rate
    pub synthetic_rate: f32,
}

impl Default for DataAugmentation {
    fn default() -> Self {
        Self {
            enabled: true,
            noise_rate: 0.05,
            feature_dropout_rate: 0.1,
            synthetic_rate: 0.2,
        }
    }
}

/// Validation configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationConfig {
    /// Cross-validation folds
    pub cv_folds: usize,
    /// Metrics to track during training
    pub metrics: Vec<TrainingMetric>,
    /// Whether to use stratified sampling
    pub stratified: bool,
}

impl Default for ValidationConfig {
    fn default() -> Self {
        Self {
            cv_folds: 5,
            metrics: vec![
                TrainingMetric::Accuracy,
                TrainingMetric::Precision,
                TrainingMetric::Recall,
                TrainingMetric::F1Score,
                TrainingMetric::Loss,
            ],
            stratified: true,
        }
    }
}

/// Training metrics to track
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TrainingMetric {
    Accuracy,
    Precision,
    Recall,
    F1Score,
    Loss,
    MeanSquaredError,
    MeanAbsoluteError,
    R2Score,
}

/// Model output configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputConfig {
    /// Model save path
    pub model_path: String,
    /// Whether to save checkpoints
    pub save_checkpoints: bool,
    /// Checkpoint frequency (epochs)
    pub checkpoint_frequency: usize,
    /// Whether to export model for inference
    pub export_for_inference: bool,
}

/// Training dataset
#[derive(Debug, Clone)]
pub struct TrainingDataset {
    /// Feature vectors
    pub features: Vec<CodeFeatures>,
    /// Target labels/values
    pub targets: Vec<TrainingTarget>,
    /// Dataset metadata
    pub metadata: DatasetMetadata,
}

/// Training targets for different model types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TrainingTarget {
    /// Classification target (class index)
    Classification(usize),
    /// Regression target (continuous value)
    Regression(f32),
    /// Multi-label classification
    MultiLabel(Vec<bool>),
    /// Ranking target
    Ranking(f32),
}

/// Dataset metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatasetMetadata {
    /// Total number of samples
    pub sample_count: usize,
    /// Number of features
    pub feature_count: usize,
    /// Class distribution (for classification)
    pub class_distribution: HashMap<String, usize>,
    /// Target statistics (for regression)
    pub target_statistics: Option<TargetStatistics>,
}

/// Target statistics for regression tasks
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TargetStatistics {
    pub mean: f32,
    pub std: f32,
    pub min: f32,
    pub max: f32,
    pub median: f32,
}

/// Training results and metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainingResults {
    /// Model type that was trained
    pub model_type: ModelType,
    /// Final training metrics
    pub training_metrics: HashMap<String, f32>,
    /// Final validation metrics
    pub validation_metrics: HashMap<String, f32>,
    /// Training history
    pub training_history: TrainingHistory,
    /// Model metadata
    pub model_metadata: ModelMetadata,
}

/// Training history tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainingHistory {
    /// Epoch-by-epoch training loss
    pub training_loss: Vec<f32>,
    /// Epoch-by-epoch validation loss
    pub validation_loss: Vec<f32>,
    /// Epoch-by-epoch metrics
    pub metrics_history: HashMap<String, Vec<f32>>,
    /// Total training time (seconds)
    pub training_time: f32,
}

/// Model metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelMetadata {
    /// Model version
    pub version: String,
    /// Training timestamp
    pub timestamp: chrono::DateTime<chrono::Utc>,
    /// Training configuration used
    pub config: TrainingConfig,
    /// Feature importance scores
    pub feature_importance: HashMap<String, f32>,
}

/// Domain-specific model trainer
pub struct ModelTrainer {
    config: TrainingConfig,
    feature_extractor: Arc<FeatureExtractor>,
    datasets: Arc<RwLock<HashMap<String, TrainingDataset>>>,
    trained_models: Arc<RwLock<HashMap<String, TrainedModel>>>,
}

/// Trained model container
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainedModel {
    /// Model type
    pub model_type: ModelType,
    /// Model weights (simplified representation)
    pub weights: ModelWeights,
    /// Model metadata
    pub metadata: ModelMetadata,
    /// Performance metrics
    pub performance: HashMap<String, f32>,
}

/// Model weights (simplified representation)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelWeights {
    /// Linear layer weights
    pub linear_weights: Vec<Vec<f32>>,
    /// Bias terms
    pub biases: Vec<f32>,
    /// Feature scaling parameters
    pub feature_scale: FeatureScaling,
}

/// Feature scaling parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureScaling {
    /// Feature means
    pub means: Vec<f32>,
    /// Feature standard deviations
    pub stds: Vec<f32>,
    /// Min values for min-max scaling
    pub mins: Vec<f32>,
    /// Max values for min-max scaling
    pub maxs: Vec<f32>,
}

impl ModelTrainer {
    /// Create a new model trainer
    pub fn new(config: TrainingConfig, feature_extractor: Arc<FeatureExtractor>) -> Self {
        Self {
            config,
            feature_extractor,
            datasets: Arc::new(RwLock::new(HashMap::new())),
            trained_models: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Prepare training dataset from code nodes
    pub async fn prepare_dataset(
        &self,
        nodes: &[CodeNode],
        targets: Vec<TrainingTarget>,
        dataset_name: &str,
    ) -> Result<()> {
        if nodes.len() != targets.len() {
            return Err(CodeGraphError::Training(
                "Number of nodes and targets must match".to_string(),
            ));
        }

        // Extract features from code nodes
        let features = self.feature_extractor.extract_features_batch(nodes).await?;

        // Calculate dataset metadata
        let metadata = self.calculate_dataset_metadata(&features, &targets);

        let dataset = TrainingDataset {
            features,
            targets,
            metadata,
        };

        // Store dataset
        let mut datasets = self.datasets.write().await;
        datasets.insert(dataset_name.to_string(), dataset);

        Ok(())
    }

    /// Train a domain-specific model
    pub async fn train_model(
        &self,
        dataset_name: &str,
        model_name: &str,
    ) -> Result<TrainingResults> {
        let datasets = self.datasets.read().await;
        let dataset = datasets.get(dataset_name).ok_or_else(|| {
            CodeGraphError::Training(format!("Dataset '{}' not found", dataset_name))
        })?;

        // Split dataset
        let (train_data, val_data, _test_data) = self.split_dataset(dataset)?;

        // Initialize model weights
        let mut model_weights = self.initialize_model_weights(&train_data)?;

        // Training loop
        let mut training_history = TrainingHistory {
            training_loss: Vec::new(),
            validation_loss: Vec::new(),
            metrics_history: HashMap::new(),
            training_time: 0.0,
        };

        let start_time = std::time::Instant::now();
        let mut best_val_loss = f32::INFINITY;
        let mut patience_counter = 0;

        for epoch in 0..self.config.hyperparameters.epochs {
            // Training step
            let train_loss = self.train_epoch(&train_data, &mut model_weights).await?;
            training_history.training_loss.push(train_loss);

            // Validation step
            if epoch % self.config.hyperparameters.validation_frequency == 0 {
                let val_loss = self.validate_epoch(&val_data, &model_weights).await?;
                training_history.validation_loss.push(val_loss);

                // Early stopping check
                if val_loss < best_val_loss {
                    best_val_loss = val_loss;
                    patience_counter = 0;
                } else {
                    patience_counter += 1;
                    if patience_counter >= self.config.hyperparameters.early_stopping_patience {
                        break;
                    }
                }
            }

            // Save checkpoint if configured
            if self.config.output_config.save_checkpoints
                && epoch % self.config.output_config.checkpoint_frequency == 0
            {
                self.save_checkpoint(&model_weights, epoch, model_name)
                    .await?;
            }
        }

        training_history.training_time = start_time.elapsed().as_secs_f32();

        // Calculate final metrics
        let training_metrics = self.calculate_metrics(&train_data, &model_weights).await?;
        let validation_metrics = self.calculate_metrics(&val_data, &model_weights).await?;

        // Create trained model
        let trained_model = TrainedModel {
            model_type: self.config.model_type.clone(),
            weights: model_weights,
            metadata: ModelMetadata {
                version: "1.0.0".to_string(),
                timestamp: chrono::Utc::now(),
                config: self.config.clone(),
                feature_importance: HashMap::new(), // TODO: Calculate feature importance
            },
            performance: validation_metrics.clone(),
        };

        // Store trained model
        let mut models = self.trained_models.write().await;
        models.insert(model_name.to_string(), trained_model);

        Ok(TrainingResults {
            model_type: self.config.model_type.clone(),
            training_metrics,
            validation_metrics,
            training_history,
            model_metadata: ModelMetadata {
                version: "1.0.0".to_string(),
                timestamp: chrono::Utc::now(),
                config: self.config.clone(),
                feature_importance: HashMap::new(),
            },
        })
    }

    /// Cross-validation training
    pub async fn cross_validate(&self, dataset_name: &str) -> Result<Vec<TrainingResults>> {
        let datasets = self.datasets.read().await;
        let dataset = datasets.get(dataset_name).ok_or_else(|| {
            CodeGraphError::Training(format!("Dataset '{}' not found", dataset_name))
        })?;

        let mut cv_results = Vec::new();
        let fold_size = dataset.features.len() / self.config.validation_config.cv_folds;

        for fold in 0..self.config.validation_config.cv_folds {
            // Create fold-specific train/validation split
            let val_start = fold * fold_size;
            let val_end = if fold == self.config.validation_config.cv_folds - 1 {
                dataset.features.len()
            } else {
                (fold + 1) * fold_size
            };

            let val_features: Vec<_> = dataset.features[val_start..val_end].to_vec();
            let val_targets: Vec<_> = dataset.targets[val_start..val_end].to_vec();

            let mut train_features = Vec::new();
            let mut train_targets = Vec::new();

            train_features.extend_from_slice(&dataset.features[..val_start]);
            train_features.extend_from_slice(&dataset.features[val_end..]);
            train_targets.extend_from_slice(&dataset.targets[..val_start]);
            train_targets.extend_from_slice(&dataset.targets[val_end..]);

            let train_data = TrainingDataset {
                features: train_features,
                targets: train_targets,
                metadata: dataset.metadata.clone(),
            };

            let val_data = TrainingDataset {
                features: val_features,
                targets: val_targets,
                metadata: dataset.metadata.clone(),
            };

            // Train on fold
            let mut model_weights = self.initialize_model_weights(&train_data)?;
            let _train_loss = self.train_epoch(&train_data, &mut model_weights).await?;
            let val_metrics = self.calculate_metrics(&val_data, &model_weights).await?;

            cv_results.push(TrainingResults {
                model_type: self.config.model_type.clone(),
                training_metrics: HashMap::new(),
                validation_metrics: val_metrics,
                training_history: TrainingHistory {
                    training_loss: Vec::new(),
                    validation_loss: Vec::new(),
                    metrics_history: HashMap::new(),
                    training_time: 0.0,
                },
                model_metadata: ModelMetadata {
                    version: format!("cv_fold_{}", fold),
                    timestamp: chrono::Utc::now(),
                    config: self.config.clone(),
                    feature_importance: HashMap::new(),
                },
            });
        }

        Ok(cv_results)
    }

    /// Get trained model by name
    pub async fn get_model(&self, model_name: &str) -> Option<TrainedModel> {
        let models = self.trained_models.read().await;
        models.get(model_name).cloned()
    }

    /// Save model to disk
    pub async fn save_model(&self, model_name: &str, path: &Path) -> Result<()> {
        let models = self.trained_models.read().await;
        let model = models
            .get(model_name)
            .ok_or_else(|| CodeGraphError::Training(format!("Model '{}' not found", model_name)))?;

        let serialized = serde_json::to_string_pretty(model)
            .map_err(|e| CodeGraphError::Training(format!("Failed to serialize model: {}", e)))?;

        tokio::fs::write(path, serialized)
            .await
            .map_err(|e| CodeGraphError::Training(format!("Failed to write model file: {}", e)))?;

        Ok(())
    }

    /// Load model from disk
    pub async fn load_model(&self, model_name: &str, path: &Path) -> Result<()> {
        let content = tokio::fs::read_to_string(path)
            .await
            .map_err(|e| CodeGraphError::Training(format!("Failed to read model file: {}", e)))?;

        let model: TrainedModel = serde_json::from_str(&content)
            .map_err(|e| CodeGraphError::Training(format!("Failed to deserialize model: {}", e)))?;

        let mut models = self.trained_models.write().await;
        models.insert(model_name.to_string(), model);

        Ok(())
    }

    // Private helper methods

    fn calculate_dataset_metadata(
        &self,
        features: &[CodeFeatures],
        targets: &[TrainingTarget],
    ) -> DatasetMetadata {
        let sample_count = features.len();
        let feature_count = self.estimate_feature_count(features);

        let mut class_distribution = HashMap::new();
        let mut target_values = Vec::new();

        for target in targets {
            match target {
                TrainingTarget::Classification(class) => {
                    let class_name = format!("class_{}", class);
                    *class_distribution.entry(class_name).or_insert(0) += 1;
                }
                TrainingTarget::Regression(value) => {
                    target_values.push(*value);
                }
                _ => {} // Handle other target types as needed
            }
        }

        let target_statistics = if !target_values.is_empty() {
            let mean = target_values.iter().sum::<f32>() / target_values.len() as f32;
            let variance = target_values
                .iter()
                .map(|x| (x - mean).powi(2))
                .sum::<f32>()
                / target_values.len() as f32;
            let std = variance.sqrt();
            let min = target_values.iter().fold(f32::INFINITY, |a, &b| a.min(b));
            let max = target_values
                .iter()
                .fold(f32::NEG_INFINITY, |a, &b| a.max(b));

            let mut sorted_values = target_values.clone();
            sorted_values.sort_by(|a, b| a.partial_cmp(b).unwrap());
            let median = sorted_values[sorted_values.len() / 2];

            Some(TargetStatistics {
                mean,
                std,
                min,
                max,
                median,
            })
        } else {
            None
        };

        DatasetMetadata {
            sample_count,
            feature_count,
            class_distribution,
            target_statistics,
        }
    }

    fn estimate_feature_count(&self, features: &[CodeFeatures]) -> usize {
        if features.is_empty() {
            return 0;
        }

        let mut count = 0;
        let sample = &features[0];

        if let Some(ref syntactic) = sample.syntactic {
            count += 5; // Basic syntactic features
            count += syntactic.node_type_distribution.len();
        }

        if let Some(ref semantic) = sample.semantic {
            count += semantic.embedding.len();
            count += semantic.pattern_similarities.len();
            count += 1; // density score
        }

        if sample.complexity.is_some() {
            count += 5; // Complexity features
        }

        if sample.dependencies.is_some() {
            count += 4; // Dependency features
        }

        count
    }

    fn split_dataset(
        &self,
        dataset: &TrainingDataset,
    ) -> Result<(TrainingDataset, TrainingDataset, TrainingDataset)> {
        let total_samples = dataset.features.len();
        let train_size = (total_samples as f32 * self.config.data_config.train_split) as usize;
        let val_size = (total_samples as f32 * self.config.data_config.validation_split) as usize;

        let train_features = dataset.features[..train_size].to_vec();
        let train_targets = dataset.targets[..train_size].to_vec();

        let val_features = dataset.features[train_size..train_size + val_size].to_vec();
        let val_targets = dataset.targets[train_size..train_size + val_size].to_vec();

        let test_features = dataset.features[train_size + val_size..].to_vec();
        let test_targets = dataset.targets[train_size + val_size..].to_vec();

        Ok((
            TrainingDataset {
                features: train_features,
                targets: train_targets,
                metadata: dataset.metadata.clone(),
            },
            TrainingDataset {
                features: val_features,
                targets: val_targets,
                metadata: dataset.metadata.clone(),
            },
            TrainingDataset {
                features: test_features,
                targets: test_targets,
                metadata: dataset.metadata.clone(),
            },
        ))
    }

    fn initialize_model_weights(&self, dataset: &TrainingDataset) -> Result<ModelWeights> {
        let feature_count = self.estimate_feature_count(&dataset.features);
        let output_size = match self.config.model_type {
            ModelType::QualityClassifier => 3,    // good/bad/needs_review
            ModelType::BugDetector => 2,          // bug/no_bug
            ModelType::SecurityDetector => 2,     // vulnerable/safe
            ModelType::ComplexityPredictor => 1,  // continuous value
            ModelType::PerformancePredictor => 1, // continuous value
            ModelType::SimilarityModel => 1,      // similarity score
        };

        // Simple linear model initialization
        let mut linear_weights = Vec::new();
        let mut rng_state = 12345u32;

        for _ in 0..output_size {
            let mut layer_weights = Vec::new();
            for _ in 0..feature_count {
                rng_state = rng_state.wrapping_mul(1103515245).wrapping_add(12345);
                let weight = ((rng_state as f32 / u32::MAX as f32) - 0.5) * 0.1;
                layer_weights.push(weight);
            }
            linear_weights.push(layer_weights);
        }

        let biases = vec![0.0; output_size];

        Ok(ModelWeights {
            linear_weights,
            biases,
            feature_scale: FeatureScaling {
                means: vec![0.0; feature_count],
                stds: vec![1.0; feature_count],
                mins: vec![0.0; feature_count],
                maxs: vec![1.0; feature_count],
            },
        })
    }

    async fn train_epoch(
        &self,
        dataset: &TrainingDataset,
        _weights: &mut ModelWeights,
    ) -> Result<f32> {
        // Simplified training step - in practice would implement backpropagation
        let mut total_loss = 0.0;
        let batch_count = (dataset.features.len() + self.config.hyperparameters.batch_size - 1)
            / self.config.hyperparameters.batch_size;

        for _batch in 0..batch_count {
            // Simulate training loss
            total_loss += 0.5; // Placeholder loss value
        }

        Ok(total_loss / batch_count as f32)
    }

    async fn validate_epoch(
        &self,
        dataset: &TrainingDataset,
        _weights: &ModelWeights,
    ) -> Result<f32> {
        // Simplified validation step
        let mut total_loss = 0.0;

        for _ in &dataset.features {
            // Simulate validation loss
            total_loss += 0.4; // Placeholder loss value
        }

        Ok(total_loss / dataset.features.len() as f32)
    }

    async fn calculate_metrics(
        &self,
        _dataset: &TrainingDataset,
        _weights: &ModelWeights,
    ) -> Result<HashMap<String, f32>> {
        let mut metrics = HashMap::new();

        // Placeholder metrics - in practice would calculate actual model performance
        metrics.insert("accuracy".to_string(), 0.85);
        metrics.insert("precision".to_string(), 0.82);
        metrics.insert("recall".to_string(), 0.88);
        metrics.insert("f1_score".to_string(), 0.85);
        metrics.insert("loss".to_string(), 0.3);

        Ok(metrics)
    }

    async fn save_checkpoint(
        &self,
        _weights: &ModelWeights,
        epoch: usize,
        model_name: &str,
    ) -> Result<()> {
        let checkpoint_path = format!(
            "{}_{}_epoch_{}.checkpoint",
            self.config.output_config.model_path, model_name, epoch
        );

        // In practice, would serialize and save model weights
        tokio::fs::write(&checkpoint_path, format!("Checkpoint for epoch {}", epoch))
            .await
            .map_err(|e| CodeGraphError::Training(format!("Failed to save checkpoint: {}", e)))?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ml::features::FeatureConfig;
    use crate::EmbeddingGenerator;
    use codegraph_core::{Language, Location, NodeType};

    fn sample_location() -> Location {
        Location {
            file_path: "test.rs".to_string(),
            line: 1,
            column: 1,
            end_line: None,
            end_column: None,
        }
    }

    fn sample_node(name: &str, content: &str) -> CodeNode {
        CodeNode::new(
            name,
            Some(NodeType::Function),
            Some(Language::Rust),
            sample_location(),
        )
        .with_content(content.to_string())
    }

    #[tokio::test]
    async fn test_dataset_preparation() {
        let config = TrainingConfig {
            model_type: ModelType::QualityClassifier,
            hyperparameters: TrainingHyperparameters::default(),
            data_config: DataConfig::default(),
            validation_config: ValidationConfig::default(),
            output_config: OutputConfig {
                model_path: "test_model".to_string(),
                save_checkpoints: false,
                checkpoint_frequency: 10,
                export_for_inference: true,
            },
        };

        let feature_config = FeatureConfig::default();
        let embedding_generator = Arc::new(EmbeddingGenerator::default());
        let feature_extractor =
            Arc::new(FeatureExtractor::new(feature_config, embedding_generator));
        let trainer = ModelTrainer::new(config, feature_extractor);

        let nodes = vec![sample_node(
            "test_function",
            "fn test() { println!(\"Hello\"); }",
        )];

        let targets = vec![TrainingTarget::Classification(1)]; // Good quality

        let result = trainer
            .prepare_dataset(&nodes, targets, "test_dataset")
            .await;
        assert!(result.is_ok());

        let datasets = trainer.datasets.read().await;
        assert!(datasets.contains_key("test_dataset"));
    }

    #[tokio::test]
    async fn test_model_training() {
        let config = TrainingConfig {
            model_type: ModelType::ComplexityPredictor,
            hyperparameters: TrainingHyperparameters {
                epochs: 5, // Short training for test
                ..TrainingHyperparameters::default()
            },
            data_config: DataConfig::default(),
            validation_config: ValidationConfig::default(),
            output_config: OutputConfig {
                model_path: "test_model".to_string(),
                save_checkpoints: false,
                checkpoint_frequency: 10,
                export_for_inference: true,
            },
        };

        let feature_config = FeatureConfig::default();
        let embedding_generator = Arc::new(EmbeddingGenerator::default());
        let feature_extractor =
            Arc::new(FeatureExtractor::new(feature_config, embedding_generator));
        let trainer = ModelTrainer::new(config, feature_extractor);

        let nodes = vec![
            sample_node("simple_function", "fn simple() { return 1; }"),
            sample_node(
                "complex_function",
                "fn complex() { if x > 0 { for i in 0..10 { if i % 2 == 0 { println!(\"{}\", i); } } } }",
            ),
        ];

        let targets = vec![
            TrainingTarget::Regression(1.0), // Low complexity
            TrainingTarget::Regression(5.0), // High complexity
        ];

        trainer
            .prepare_dataset(&nodes, targets, "complexity_dataset")
            .await
            .unwrap();
        let results = trainer
            .train_model("complexity_dataset", "complexity_model")
            .await;

        assert!(results.is_ok());
        let training_results = results.unwrap();
        assert_eq!(training_results.model_type, ModelType::ComplexityPredictor);
        assert!(!training_results.training_history.training_loss.is_empty());
    }

    #[test]
    fn test_dataset_metadata_calculation() {
        let config = TrainingConfig {
            model_type: ModelType::QualityClassifier,
            hyperparameters: TrainingHyperparameters::default(),
            data_config: DataConfig::default(),
            validation_config: ValidationConfig::default(),
            output_config: OutputConfig {
                model_path: "test".to_string(),
                save_checkpoints: false,
                checkpoint_frequency: 10,
                export_for_inference: true,
            },
        };

        let feature_config = FeatureConfig::default();
        let embedding_generator = Arc::new(EmbeddingGenerator::default());
        let feature_extractor =
            Arc::new(FeatureExtractor::new(feature_config, embedding_generator));
        let trainer = ModelTrainer::new(config, feature_extractor);

        let features = vec![]; // Empty for test
        let targets = vec![
            TrainingTarget::Classification(0),
            TrainingTarget::Classification(1),
            TrainingTarget::Classification(0),
        ];

        let metadata = trainer.calculate_dataset_metadata(&features, &targets);
        assert_eq!(metadata.sample_count, 0);
        assert_eq!(metadata.class_distribution.get("class_0"), Some(&2));
        assert_eq!(metadata.class_distribution.get("class_1"), Some(&1));
    }
}
