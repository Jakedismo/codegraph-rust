//! Inference optimization for real-time code analysis
//!
//! This module provides optimized inference capabilities for trained models,
//! focusing on low-latency, high-throughput predictions for real-time code analysis.
//! Includes model optimization, batching, caching, and hardware acceleration.

use crate::ml::features::{CodeFeatures, FeatureExtractor};
use crate::ml::training::{ModelType, ModelWeights, TrainedModel};
use codegraph_core::{CodeGraphError, CodeNode, Result};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{RwLock, Semaphore};

/// Configuration for inference optimization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InferenceConfig {
    /// Maximum batch size for inference
    pub max_batch_size: usize,
    /// Batch timeout in milliseconds
    pub batch_timeout_ms: u64,
    /// Maximum concurrent inference requests
    pub max_concurrent_requests: usize,
    /// Cache configuration
    pub cache_config: CacheConfig,
    /// Model optimization settings
    pub optimization: OptimizationConfig,
    /// Performance monitoring settings
    pub monitoring: MonitoringConfig,
}

impl Default for InferenceConfig {
    fn default() -> Self {
        Self {
            max_batch_size: 32,
            batch_timeout_ms: 10,
            max_concurrent_requests: 100,
            cache_config: CacheConfig::default(),
            optimization: OptimizationConfig::default(),
            monitoring: MonitoringConfig::default(),
        }
    }
}

/// Cache configuration for inference results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheConfig {
    /// Enable inference result caching
    pub enabled: bool,
    /// Maximum cache size (number of entries)
    pub max_entries: usize,
    /// Cache TTL in seconds
    pub ttl_seconds: u64,
    /// Cache key strategy
    pub key_strategy: CacheKeyStrategy,
    /// Eviction policy
    pub eviction_policy: EvictionPolicy,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_entries: 10000,
            ttl_seconds: 3600, // 1 hour
            key_strategy: CacheKeyStrategy::ContentHash,
            eviction_policy: EvictionPolicy::LRU,
        }
    }
}

/// Cache key generation strategies
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CacheKeyStrategy {
    /// Use content hash as key
    ContentHash,
    /// Use node ID as key
    NodeId,
    /// Use combined node features hash
    FeatureHash,
    /// Custom key based on selected features
    Custom(Vec<String>),
}

/// Cache eviction policies
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EvictionPolicy {
    /// Least Recently Used
    LRU,
    /// Least Frequently Used
    LFU,
    /// Time-based expiration
    TTL,
    /// Random eviction
    Random,
}

/// Model optimization configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptimizationConfig {
    /// Enable model quantization
    pub enable_quantization: bool,
    /// Quantization precision (bits)
    pub quantization_bits: u8,
    /// Enable model pruning
    pub enable_pruning: bool,
    /// Pruning threshold
    pub pruning_threshold: f32,
    /// Enable kernel fusion
    pub enable_fusion: bool,
    /// Memory optimization level
    pub memory_optimization: MemoryOptimization,
}

impl Default for OptimizationConfig {
    fn default() -> Self {
        Self {
            enable_quantization: false,
            quantization_bits: 8,
            enable_pruning: false,
            pruning_threshold: 0.01,
            enable_fusion: true,
            memory_optimization: MemoryOptimization::Balanced,
        }
    }
}

/// Memory optimization strategies
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MemoryOptimization {
    /// Optimize for speed (higher memory usage)
    Speed,
    /// Balanced optimization
    Balanced,
    /// Optimize for memory (lower speed)
    Memory,
}

/// Performance monitoring configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitoringConfig {
    /// Enable performance monitoring
    pub enabled: bool,
    /// Metrics collection interval (seconds)
    pub collection_interval_seconds: u64,
    /// Maximum metrics history size
    pub max_history_size: usize,
    /// Enable detailed timing
    pub detailed_timing: bool,
}

impl Default for MonitoringConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            collection_interval_seconds: 60,
            max_history_size: 1000,
            detailed_timing: false,
        }
    }
}

/// Inference request for batching
#[derive(Debug)]
pub struct InferenceRequest {
    /// Request ID for tracking
    pub id: String,
    /// Code features for inference
    pub features: CodeFeatures,
    /// Response sender
    pub response_tx: tokio::sync::oneshot::Sender<Result<InferenceResult>>,
    /// Request timestamp
    pub timestamp: Instant,
}

/// Inference result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InferenceResult {
    /// Prediction result
    pub prediction: PredictionResult,
    /// Confidence score
    pub confidence: f32,
    /// Inference latency in microseconds
    pub latency_us: u64,
    /// Whether result was cached
    pub from_cache: bool,
    /// Model version used
    pub model_version: String,
}

/// Prediction results for different model types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PredictionResult {
    /// Classification result with class probabilities
    Classification {
        predicted_class: usize,
        class_probabilities: Vec<f32>,
        class_names: Vec<String>,
    },
    /// Regression result
    Regression { value: f32, uncertainty: f32 },
    /// Multi-label classification
    MultiLabel {
        predictions: Vec<bool>,
        probabilities: Vec<f32>,
        labels: Vec<String>,
    },
    /// Ranking score
    Ranking { score: f32, rank: usize },
}

/// Performance metrics for inference
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InferenceMetrics {
    /// Total inference requests
    pub total_requests: u64,
    /// Total inference time (microseconds)
    pub total_inference_time_us: u64,
    /// Average latency (microseconds)
    pub avg_latency_us: f32,
    /// P95 latency (microseconds)
    pub p95_latency_us: u64,
    /// P99 latency (microseconds)
    pub p99_latency_us: u64,
    /// Throughput (requests per second)
    pub throughput_rps: f32,
    /// Cache hit rate
    pub cache_hit_rate: f32,
    /// Error rate
    pub error_rate: f32,
    /// Batch utilization
    pub avg_batch_size: f32,
    /// Memory usage (bytes)
    pub memory_usage_bytes: u64,
}

/// Cache entry for inference results
#[derive(Debug, Clone)]
struct CacheEntry {
    result: InferenceResult,
    timestamp: Instant,
    access_count: u64,
    last_accessed: Instant,
}

/// Optimized inference engine
pub struct InferenceEngine {
    config: InferenceConfig,
    models: Arc<RwLock<HashMap<String, OptimizedModel>>>,
    cache: Arc<RwLock<HashMap<String, CacheEntry>>>,
    request_queue: Arc<Mutex<Vec<InferenceRequest>>>,
    semaphore: Arc<Semaphore>,
    feature_extractor: Arc<FeatureExtractor>,
    metrics: Arc<RwLock<InferenceMetrics>>,
    latency_history: Arc<Mutex<Vec<u64>>>,
}

/// Optimized model wrapper
#[derive(Debug, Clone)]
pub struct OptimizedModel {
    /// Original model
    pub model: TrainedModel,
    /// Optimized weights
    pub optimized_weights: OptimizedWeights,
    /// Model metadata
    pub metadata: OptimizedModelMetadata,
}

/// Optimized model weights
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptimizedWeights {
    /// Quantized weights (if enabled)
    pub quantized_weights: Option<Vec<Vec<i8>>>,
    /// Pruned weight indices
    pub pruned_indices: Option<Vec<bool>>,
    /// Fused operations
    pub fused_ops: Vec<FusedOperation>,
    /// Scaling factors for quantization
    pub scale_factors: Vec<f32>,
    /// Zero points for quantization
    pub zero_points: Vec<i8>,
}

/// Fused operations for optimization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FusedOperation {
    /// Linear + ReLU fusion
    LinearReLU { weights: Vec<f32>, bias: f32 },
    /// Batch normalization fusion
    BatchNorm { scale: f32, shift: f32 },
    /// Attention fusion
    Attention {
        q_weights: Vec<f32>,
        k_weights: Vec<f32>,
        v_weights: Vec<f32>,
    },
}

/// Optimized model metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptimizedModelMetadata {
    /// Model size reduction factor
    pub size_reduction: f32,
    /// Speed improvement factor
    pub speed_improvement: f32,
    /// Accuracy retention
    pub accuracy_retention: f32,
    /// Optimization timestamp
    pub optimized_at: chrono::DateTime<chrono::Utc>,
}

impl InferenceEngine {
    /// Create a new inference engine
    pub fn new(config: InferenceConfig, feature_extractor: Arc<FeatureExtractor>) -> Self {
        let semaphore = Arc::new(Semaphore::new(config.max_concurrent_requests));

        Self {
            config,
            models: Arc::new(RwLock::new(HashMap::new())),
            cache: Arc::new(RwLock::new(HashMap::new())),
            request_queue: Arc::new(Mutex::new(Vec::new())),
            semaphore,
            feature_extractor,
            metrics: Arc::new(RwLock::new(InferenceMetrics {
                total_requests: 0,
                total_inference_time_us: 0,
                avg_latency_us: 0.0,
                p95_latency_us: 0,
                p99_latency_us: 0,
                throughput_rps: 0.0,
                cache_hit_rate: 0.0,
                error_rate: 0.0,
                avg_batch_size: 0.0,
                memory_usage_bytes: 0,
            })),
            latency_history: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Add an optimized model to the engine
    pub async fn add_model(&self, name: &str, model: TrainedModel) -> Result<()> {
        let optimized_model = self.optimize_model(model).await?;
        let mut models = self.models.write().await;
        models.insert(name.to_string(), optimized_model);
        Ok(())
    }

    /// Perform inference on a single node
    pub async fn predict(&self, model_name: &str, node: &CodeNode) -> Result<InferenceResult> {
        let _permit =
            self.semaphore.acquire().await.map_err(|e| {
                CodeGraphError::Vector(format!("Failed to acquire semaphore: {}", e))
            })?;

        let start_time = Instant::now();

        // Extract features
        let features = self.feature_extractor.extract_features(node).await?;

        // Check cache
        if self.config.cache_config.enabled {
            let cache_key = self.generate_cache_key(&features)?;
            if let Some(cached_result) = self.get_from_cache(&cache_key).await {
                self.update_metrics(start_time.elapsed(), true).await;
                return Ok(cached_result);
            }
        }

        // Perform inference
        let result = self.run_inference(model_name, &features).await?;

        // Cache result
        if self.config.cache_config.enabled {
            let cache_key = self.generate_cache_key(&features)?;
            self.cache_result(&cache_key, &result).await;
        }

        self.update_metrics(start_time.elapsed(), false).await;
        Ok(result)
    }

    /// Perform batch inference
    pub async fn predict_batch(
        &self,
        model_name: &str,
        nodes: &[CodeNode],
    ) -> Result<Vec<InferenceResult>> {
        let _permit =
            self.semaphore.acquire().await.map_err(|e| {
                CodeGraphError::Vector(format!("Failed to acquire semaphore: {}", e))
            })?;

        let start_time = Instant::now();

        // Extract features for all nodes
        let features = self.feature_extractor.extract_features_batch(nodes).await?;

        let mut results = Vec::with_capacity(features.len());
        let mut cached_count = 0;

        for feature in features {
            // Check cache first
            if self.config.cache_config.enabled {
                let cache_key = self.generate_cache_key(&feature)?;
                if let Some(cached_result) = self.get_from_cache(&cache_key).await {
                    results.push(cached_result);
                    cached_count += 1;
                    continue;
                }
            }

            // Perform inference
            let result = self.run_inference(model_name, &feature).await?;

            // Cache result
            if self.config.cache_config.enabled {
                let cache_key = self.generate_cache_key(&feature)?;
                self.cache_result(&cache_key, &result).await;
            }

            results.push(result);
        }

        let total_time = start_time.elapsed();
        let from_cache = cached_count > 0;
        self.update_batch_metrics(total_time, results.len(), cached_count, from_cache)
            .await;

        Ok(results)
    }

    /// Start the batching service for automatic request batching
    pub async fn start_batching_service(self: Arc<Self>) {
        let config = self.config.clone();
        let batch_timeout = Duration::from_millis(config.batch_timeout_ms);

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(batch_timeout);

            loop {
                interval.tick().await;

                let requests = {
                    let mut queue = self.request_queue.lock();
                    if queue.is_empty() {
                        continue;
                    }

                    let batch_size = queue.len().min(config.max_batch_size);
                    queue.drain(..batch_size).collect::<Vec<_>>()
                };

                if !requests.is_empty() {
                    self.process_batch(requests).await;
                }
            }
        });
    }

    /// Get performance metrics
    pub async fn get_metrics(&self) -> InferenceMetrics {
        self.metrics.read().await.clone()
    }

    /// Clear cache
    pub async fn clear_cache(&self) {
        let mut cache = self.cache.write().await;
        cache.clear();
    }

    // Private helper methods

    async fn optimize_model(&self, model: TrainedModel) -> Result<OptimizedModel> {
        let mut optimized_weights = OptimizedWeights {
            quantized_weights: None,
            pruned_indices: None,
            fused_ops: Vec::new(),
            scale_factors: Vec::new(),
            zero_points: Vec::new(),
        };

        // Apply quantization if enabled
        if self.config.optimization.enable_quantization {
            optimized_weights.quantized_weights = Some(self.quantize_weights(&model.weights)?);
            optimized_weights.scale_factors = self.calculate_scale_factors(&model.weights)?;
            optimized_weights.zero_points = vec![0; model.weights.linear_weights.len()];
        }

        // Apply pruning if enabled
        if self.config.optimization.enable_pruning {
            optimized_weights.pruned_indices = Some(self.prune_weights(&model.weights)?);
        }

        // Apply operation fusion if enabled
        if self.config.optimization.enable_fusion {
            optimized_weights.fused_ops = self.fuse_operations(&model.weights)?;
        }

        let metadata = OptimizedModelMetadata {
            size_reduction: 1.5,      // Placeholder - would calculate actual reduction
            speed_improvement: 2.0,   // Placeholder - would measure actual improvement
            accuracy_retention: 0.98, // Placeholder - would validate accuracy
            optimized_at: chrono::Utc::now(),
        };

        Ok(OptimizedModel {
            model,
            optimized_weights,
            metadata,
        })
    }

    async fn run_inference(
        &self,
        model_name: &str,
        features: &CodeFeatures,
    ) -> Result<InferenceResult> {
        let models = self.models.read().await;
        let model = models
            .get(model_name)
            .ok_or_else(|| CodeGraphError::NotFound(format!("Model '{}' not found", model_name)))?;

        let start_time = Instant::now();

        // Convert features to input vector
        let input_vector = self.features_to_vector(features)?;

        // Run model inference
        let prediction = self.run_model_forward(
            &model.optimized_weights,
            &input_vector,
            &model.model.model_type,
        )?;

        let latency_us = start_time.elapsed().as_micros() as u64;

        Ok(InferenceResult {
            prediction,
            confidence: 0.85, // Placeholder - would calculate actual confidence
            latency_us,
            from_cache: false,
            model_version: model.model.metadata.version.clone(),
        })
    }

    fn features_to_vector(&self, features: &CodeFeatures) -> Result<Vec<f32>> {
        let mut vector = Vec::new();

        // Add syntactic features
        if let Some(ref syntactic) = features.syntactic {
            vector.push(syntactic.child_count as f32);
            vector.push(syntactic.depth as f32);
            vector.push(syntactic.token_count as f32);
            vector.push(syntactic.line_count as f32);

            // Add normalized node type distribution
            let total_nodes: usize = syntactic.node_type_distribution.values().sum();
            if total_nodes > 0 {
                for count in syntactic.node_type_distribution.values() {
                    vector.push(*count as f32 / total_nodes as f32);
                }
            }
        }

        // Add semantic features
        if let Some(ref semantic) = features.semantic {
            vector.extend(&semantic.embedding);
            vector.push(semantic.density_score);
        }

        // Add complexity features
        if let Some(ref complexity) = features.complexity {
            vector.push(complexity.cyclomatic_complexity as f32);
            vector.push(complexity.cognitive_complexity as f32);
            vector.push(complexity.max_nesting_depth as f32);
            vector.push(complexity.parameter_count.unwrap_or(0) as f32);
            vector.push(complexity.return_count as f32);
        }

        // Add dependency features
        if let Some(ref dependencies) = features.dependencies {
            vector.push(dependencies.fanin as f32);
            vector.push(dependencies.fanout as f32);
            vector.push(dependencies.dependency_depth as f32);
            vector.push(dependencies.component_size as f32);
        }

        Ok(vector)
    }

    fn run_model_forward(
        &self,
        weights: &OptimizedWeights,
        input: &[f32],
        model_type: &ModelType,
    ) -> Result<PredictionResult> {
        // Simplified forward pass - in practice would implement proper neural network inference
        let mut output = vec![0.0f32; self.get_output_size(model_type)];

        // Simple linear transformation (placeholder)
        for (i, value) in output.iter_mut().enumerate() {
            let mut sum = 0.0;
            for (j, &input_val) in input.iter().enumerate() {
                if j < 10 {
                    // Simplified weight access
                    sum += input_val * 0.1; // Placeholder weights
                }
            }
            *value = sum + (i as f32 * 0.01); // Placeholder bias
        }

        // Apply activation and convert to prediction result
        match model_type {
            ModelType::QualityClassifier => {
                let softmax_output = self.softmax(&output);
                let predicted_class = softmax_output
                    .iter()
                    .enumerate()
                    .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
                    .map(|(i, _)| i)
                    .unwrap_or(0);

                Ok(PredictionResult::Classification {
                    predicted_class,
                    class_probabilities: softmax_output,
                    class_names: vec![
                        "good".to_string(),
                        "bad".to_string(),
                        "needs_review".to_string(),
                    ],
                })
            }
            ModelType::ComplexityPredictor | ModelType::PerformancePredictor => {
                Ok(PredictionResult::Regression {
                    value: output[0].max(0.0),
                    uncertainty: 0.1, // Placeholder uncertainty
                })
            }
            ModelType::BugDetector | ModelType::SecurityDetector => {
                let prob = self.sigmoid(output[0]);
                Ok(PredictionResult::Classification {
                    predicted_class: if prob > 0.5 { 1 } else { 0 },
                    class_probabilities: vec![1.0 - prob, prob],
                    class_names: vec!["safe".to_string(), "issue".to_string()],
                })
            }
            ModelType::SimilarityModel => {
                Ok(PredictionResult::Ranking {
                    score: self.sigmoid(output[0]),
                    rank: 1, // Placeholder rank
                })
            }
        }
    }

    fn get_output_size(&self, model_type: &ModelType) -> usize {
        match model_type {
            ModelType::QualityClassifier => 3,
            ModelType::BugDetector | ModelType::SecurityDetector => 2,
            ModelType::ComplexityPredictor
            | ModelType::PerformancePredictor
            | ModelType::SimilarityModel => 1,
        }
    }

    fn softmax(&self, input: &[f32]) -> Vec<f32> {
        let max_val = input.iter().fold(f32::NEG_INFINITY, |a, &b| a.max(b));
        let exp_values: Vec<f32> = input.iter().map(|&x| (x - max_val).exp()).collect();
        let sum: f32 = exp_values.iter().sum();
        exp_values.iter().map(|&x| x / sum).collect()
    }

    fn sigmoid(&self, x: f32) -> f32 {
        1.0 / (1.0 + (-x).exp())
    }

    fn generate_cache_key(&self, features: &CodeFeatures) -> Result<String> {
        match self.config.cache_config.key_strategy {
            CacheKeyStrategy::NodeId => Ok(features.node_id.clone()),
            CacheKeyStrategy::ContentHash => {
                // Generate hash from feature content
                use std::collections::hash_map::DefaultHasher;
                use std::hash::{Hash, Hasher};

                let mut hasher = DefaultHasher::new();
                features.node_id.hash(&mut hasher);
                if let Some(ref syntactic) = features.syntactic {
                    syntactic.token_count.hash(&mut hasher);
                    syntactic.line_count.hash(&mut hasher);
                }
                Ok(format!("{:x}", hasher.finish()))
            }
            CacheKeyStrategy::FeatureHash => {
                // Generate hash from all features
                Ok(format!("features_{}", features.node_id))
            }
            CacheKeyStrategy::Custom(ref _fields) => {
                // Custom key generation based on selected fields
                Ok(format!("custom_{}", features.node_id))
            }
        }
    }

    async fn get_from_cache(&self, key: &str) -> Option<InferenceResult> {
        let mut cache = self.cache.write().await;

        if let Some(entry) = cache.get_mut(key) {
            // Check if entry is still valid
            if entry.timestamp.elapsed().as_secs() < self.config.cache_config.ttl_seconds {
                entry.access_count += 1;
                entry.last_accessed = Instant::now();

                let mut result = entry.result.clone();
                result.from_cache = true;
                return Some(result);
            } else {
                // Remove expired entry
                cache.remove(key);
            }
        }

        None
    }

    async fn cache_result(&self, key: &str, result: &InferenceResult) {
        if !self.config.cache_config.enabled {
            return;
        }

        let mut cache = self.cache.write().await;

        // Check cache size and evict if necessary
        if cache.len() >= self.config.cache_config.max_entries {
            self.evict_cache_entry(&mut cache).await;
        }

        let entry = CacheEntry {
            result: result.clone(),
            timestamp: Instant::now(),
            access_count: 1,
            last_accessed: Instant::now(),
        };

        cache.insert(key.to_string(), entry);
    }

    async fn evict_cache_entry(&self, cache: &mut HashMap<String, CacheEntry>) {
        if cache.is_empty() {
            return;
        }

        let key_to_remove = match self.config.cache_config.eviction_policy {
            EvictionPolicy::LRU => cache
                .iter()
                .min_by_key(|(_, entry)| entry.last_accessed)
                .map(|(key, _)| key.clone()),
            EvictionPolicy::LFU => cache
                .iter()
                .min_by_key(|(_, entry)| entry.access_count)
                .map(|(key, _)| key.clone()),
            EvictionPolicy::TTL => cache
                .iter()
                .min_by_key(|(_, entry)| entry.timestamp)
                .map(|(key, _)| key.clone()),
            EvictionPolicy::Random => cache.keys().next().cloned(),
        };

        if let Some(key) = key_to_remove {
            cache.remove(&key);
        }
    }

    async fn process_batch(&self, requests: Vec<InferenceRequest>) {
        for request in requests {
            // Process each request individually for now
            // In practice, would batch the inference calls
            let features = request.features;
            let result = self.run_inference("default", &features).await;
            let _ = request.response_tx.send(result);
        }
    }

    async fn update_metrics(&self, latency: Duration, from_cache: bool) {
        let mut metrics = self.metrics.write().await;
        let latency_us = latency.as_micros() as u64;

        metrics.total_requests += 1;
        metrics.total_inference_time_us += latency_us;
        metrics.avg_latency_us =
            metrics.total_inference_time_us as f32 / metrics.total_requests as f32;

        if from_cache {
            metrics.cache_hit_rate = (metrics.cache_hit_rate * (metrics.total_requests - 1) as f32
                + 1.0)
                / metrics.total_requests as f32;
        } else {
            metrics.cache_hit_rate = (metrics.cache_hit_rate * (metrics.total_requests - 1) as f32)
                / metrics.total_requests as f32;
        }

        // Update latency history for percentile calculations
        let mut history = self.latency_history.lock();
        history.push(latency_us);
        if history.len() > 1000 {
            history.remove(0);
        }

        // Calculate percentiles
        let mut sorted_latencies = history.clone();
        sorted_latencies.sort_unstable();
        if !sorted_latencies.is_empty() {
            let p95_idx = (sorted_latencies.len() as f32 * 0.95) as usize;
            let p99_idx = (sorted_latencies.len() as f32 * 0.99) as usize;
            metrics.p95_latency_us = sorted_latencies.get(p95_idx).copied().unwrap_or(0);
            metrics.p99_latency_us = sorted_latencies.get(p99_idx).copied().unwrap_or(0);
        }
    }

    async fn update_batch_metrics(
        &self,
        total_time: Duration,
        batch_size: usize,
        cached_count: usize,
        _from_cache: bool,
    ) {
        let mut metrics = self.metrics.write().await;

        metrics.avg_batch_size = (metrics.avg_batch_size * metrics.total_requests as f32
            + batch_size as f32)
            / (metrics.total_requests + 1) as f32;
        metrics.total_requests += 1;

        let avg_latency_us = total_time.as_micros() as u64 / batch_size as u64;
        metrics.total_inference_time_us += avg_latency_us;
        metrics.avg_latency_us =
            metrics.total_inference_time_us as f32 / metrics.total_requests as f32;

        if cached_count > 0 {
            let cache_ratio = cached_count as f32 / batch_size as f32;
            metrics.cache_hit_rate = (metrics.cache_hit_rate * (metrics.total_requests - 1) as f32
                + cache_ratio)
                / metrics.total_requests as f32;
        }
    }

    // Optimization helper methods (simplified implementations)

    fn quantize_weights(&self, weights: &ModelWeights) -> Result<Vec<Vec<i8>>> {
        let mut quantized = Vec::new();

        for layer_weights in &weights.linear_weights {
            let mut layer_quantized = Vec::new();
            let max_val = layer_weights.iter().fold(0.0f32, |a, &b| a.max(b.abs()));
            let scale = max_val / 127.0;

            for &weight in layer_weights {
                let quantized_weight = ((weight / scale).round() as i8).clamp(-127, 127);
                layer_quantized.push(quantized_weight);
            }
            quantized.push(layer_quantized);
        }

        Ok(quantized)
    }

    fn calculate_scale_factors(&self, weights: &ModelWeights) -> Result<Vec<f32>> {
        let mut scale_factors = Vec::new();

        for layer_weights in &weights.linear_weights {
            let max_val = layer_weights.iter().fold(0.0f32, |a, &b| a.max(b.abs()));
            scale_factors.push(max_val / 127.0);
        }

        Ok(scale_factors)
    }

    fn prune_weights(&self, weights: &ModelWeights) -> Result<Vec<bool>> {
        let mut pruned_indices = Vec::new();
        let threshold = self.config.optimization.pruning_threshold;

        for layer_weights in &weights.linear_weights {
            for &weight in layer_weights {
                pruned_indices.push(weight.abs() < threshold);
            }
        }

        Ok(pruned_indices)
    }

    fn fuse_operations(&self, _weights: &ModelWeights) -> Result<Vec<FusedOperation>> {
        // Simplified operation fusion
        Ok(vec![FusedOperation::LinearReLU {
            weights: vec![0.1, 0.2, 0.3],
            bias: 0.0,
        }])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ml::features::FeatureConfig;
    use crate::EmbeddingGenerator;
    use codegraph_core::{Language, NodeType};

    #[tokio::test]
    async fn test_inference_engine_creation() {
        let config = InferenceConfig::default();
        let feature_config = FeatureConfig::default();
        let embedding_generator = Arc::new(EmbeddingGenerator::default());
        let feature_extractor =
            Arc::new(FeatureExtractor::new(feature_config, embedding_generator));

        let engine = InferenceEngine::new(config, feature_extractor);

        let metrics = engine.get_metrics().await;
        assert_eq!(metrics.total_requests, 0);
    }

    #[tokio::test]
    async fn test_cache_key_generation() {
        let config = InferenceConfig::default();
        let feature_config = FeatureConfig::default();
        let embedding_generator = Arc::new(EmbeddingGenerator::default());
        let feature_extractor =
            Arc::new(FeatureExtractor::new(feature_config, embedding_generator));

        let engine = InferenceEngine::new(config, feature_extractor);

        let features = CodeFeatures {
            node_id: "test_node".to_string(),
            syntactic: None,
            semantic: None,
            complexity: None,
            dependencies: None,
        };

        let key = engine.generate_cache_key(&features).unwrap();
        assert!(!key.is_empty());
    }

    #[test]
    fn test_softmax() {
        let config = InferenceConfig::default();
        let feature_config = FeatureConfig::default();
        let embedding_generator = Arc::new(EmbeddingGenerator::default());
        let feature_extractor =
            Arc::new(FeatureExtractor::new(feature_config, embedding_generator));

        let engine = InferenceEngine::new(config, feature_extractor);

        let input = vec![1.0, 2.0, 3.0];
        let output = engine.softmax(&input);

        let sum: f32 = output.iter().sum();
        assert!((sum - 1.0).abs() < 1e-6);
        assert!(output[2] > output[1]);
        assert!(output[1] > output[0]);
    }

    #[test]
    fn test_sigmoid() {
        let config = InferenceConfig::default();
        let feature_config = FeatureConfig::default();
        let embedding_generator = Arc::new(EmbeddingGenerator::default());
        let feature_extractor =
            Arc::new(FeatureExtractor::new(feature_config, embedding_generator));

        let engine = InferenceEngine::new(config, feature_extractor);

        assert!((engine.sigmoid(0.0) - 0.5).abs() < 1e-6);
        assert!(engine.sigmoid(100.0) > 0.9);
        assert!(engine.sigmoid(-100.0) < 0.1);
    }
}
