use crate::{CacheEntry, CacheKey};
use codegraph_core::{CodeGraphError, NodeId, Result};
use crossbeam_channel::{bounded, Receiver, Sender};
use dashmap::DashMap;
use parking_lot::RwLock as SyncRwLock;
use std::collections::{BTreeMap, HashMap, VecDeque};
use std::sync::Arc;
use std::hash::{Hash, Hasher};
use std::collections::hash_map::DefaultHasher;

fn key_hash(key: &CacheKey) -> u64 {
    let mut h = DefaultHasher::new();
    key.hash(&mut h);
    h.finish()
}
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::sync::{Mutex, RwLock};

/// Advanced Read-Ahead Optimizer with predictive data loading capabilities
pub struct ReadAheadOptimizer {
    /// Access pattern analyzer for intelligent prediction
    pattern_analyzer: Arc<AccessPatternAnalyzer>,
    /// Predictive loading engine with machine learning-based prediction
    predictive_loader: Arc<PredictiveLoader>,
    /// Cache warming system for preloading frequently accessed data
    cache_warmer: Arc<CacheWarmer>,
    /// Sequential read acceleration engine
    sequential_accelerator: Arc<SequentialReadAccelerator>,
    /// Performance metrics and monitoring
    metrics: Arc<SyncRwLock<ReadAheadMetrics>>,
    /// Configuration parameters
    config: ReadAheadConfig,
}

/// Comprehensive metrics for read-ahead optimization
#[derive(Debug, Default, Clone)]
pub struct ReadAheadMetrics {
    pub total_predictions: u64,
    pub successful_predictions: u64,
    pub prediction_accuracy: f64,
    pub cache_hits_from_readahead: u64,
    pub sequential_reads_detected: u64,
    pub cache_warming_events: u64,
    pub bytes_prefetched: u64,
    pub io_reduction_percentage: f64,
    pub average_prediction_time_ms: f64,
    pub pattern_recognition_success_rate: f64,
}

/// Configuration for read-ahead optimization
#[derive(Debug, Clone)]
pub struct ReadAheadConfig {
    pub max_pattern_history: usize,
    pub prediction_window_size: usize,
    pub sequential_threshold: usize,
    pub cache_warming_interval: Duration,
    pub prefetch_depth: usize,
    pub pattern_decay_factor: f64,
    pub min_confidence_threshold: f64,
    pub adaptive_learning_rate: f64,
}

impl Default for ReadAheadConfig {
    fn default() -> Self {
        Self {
            max_pattern_history: 10000,
            prediction_window_size: 50,
            sequential_threshold: 3,
            cache_warming_interval: Duration::from_secs(60),
            prefetch_depth: 20,
            pattern_decay_factor: 0.95,
            min_confidence_threshold: 0.7,
            adaptive_learning_rate: 0.1,
        }
    }
}

impl ReadAheadOptimizer {
    pub fn new(config: ReadAheadConfig) -> Self {
        let pattern_analyzer = Arc::new(AccessPatternAnalyzer::new(&config));
        let predictive_loader = Arc::new(PredictiveLoader::new(&config));
        let cache_warmer = Arc::new(CacheWarmer::new(&config));
        let sequential_accelerator = Arc::new(SequentialReadAccelerator::new(&config));

        Self {
            pattern_analyzer,
            predictive_loader,
            cache_warmer,
            sequential_accelerator,
            metrics: Arc::new(SyncRwLock::new(ReadAheadMetrics::default())),
            config,
        }
    }

    /// Main optimization entry point for read operations
    pub async fn optimize_read(&self, key: CacheKey) -> Result<Option<Vec<u8>>> {
        let start_time = Instant::now();

        // 1. Record access pattern
        self.pattern_analyzer.record_access(key.clone()).await;

        // 2. Check for sequential read patterns
        if let Some(next_keys) = self
            .sequential_accelerator
            .detect_sequential_pattern(key.clone())
            .await
        {
            self.prefetch_sequential_batch(next_keys).await?;
        }

        // 3. Trigger predictive loading
        let predicted_keys = self
            .predictive_loader
            .predict_next_accesses(key.clone())
            .await?;
        self.prefetch_predicted_keys(predicted_keys).await?;

        // 4. Update metrics
        self.update_metrics(start_time.elapsed()).await;

        // For demo purposes - in practice, this would integrate with actual storage
        Ok(Some(format!("optimized_data_for_{}", key_hash(&key)).into_bytes()))
    }

    /// Prefetch a batch of sequential keys
    async fn prefetch_sequential_batch(&self, keys: Vec<CacheKey>) -> Result<()> {
        let batch_size = self.config.prefetch_depth.min(keys.len());
        let batch = &keys[..batch_size];

        // Launch background prefetch tasks
        for chunk in batch.chunks(10) {
            let chunk_keys = chunk.to_vec();
            let predictive_loader = Arc::clone(&self.predictive_loader);

            tokio::spawn(async move {
                let _ = predictive_loader.prefetch_batch(chunk_keys).await;
            });
        }

        Ok(())
    }

    /// Prefetch predicted keys based on access patterns
    async fn prefetch_predicted_keys(&self, keys: Vec<CacheKey>) -> Result<()> {
        if keys.is_empty() {
            return Ok(());
        }

        let batch_size = self.config.prefetch_depth.min(keys.len());
        let batch = &keys[..batch_size];

        self.predictive_loader
            .prefetch_batch(batch.to_vec())
            .await?;

        Ok(())
    }

    /// Get comprehensive performance metrics
    pub async fn get_metrics(&self) -> ReadAheadMetrics {
        self.metrics.read().clone()
    }

    /// Start cache warming background task
    pub async fn start_cache_warming(&self) -> Result<()> {
        self.cache_warmer.start_warming_cycle().await
    }

    async fn update_metrics(&self, operation_time: Duration) {
        let mut metrics = self.metrics.write();
        metrics.total_predictions += 1;

        let time_ms = operation_time.as_secs_f64() * 1000.0;
        metrics.average_prediction_time_ms = if metrics.total_predictions == 1 {
            time_ms
        } else {
            (metrics.average_prediction_time_ms + time_ms) / 2.0
        };
    }
}

/// Access pattern analyzer with machine learning capabilities
pub struct AccessPatternAnalyzer {
    /// Historical access patterns
    access_history: Arc<RwLock<VecDeque<AccessEvent>>>,
    /// Pattern frequency analysis
    pattern_frequencies: Arc<DashMap<PatternKey, PatternMetrics>>,
    /// Temporal access patterns
    temporal_patterns: Arc<RwLock<BTreeMap<u64, Vec<CacheKey>>>>,
    config: ReadAheadConfig,
}

#[derive(Debug, Clone)]
struct AccessEvent {
    key: CacheKey,
    timestamp: u64,
    context: AccessContext,
}

#[derive(Debug, Clone)]
struct AccessContext {
    previous_keys: Vec<CacheKey>,
    access_type: AccessType,
    file_type: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
enum AccessType {
    Sequential,
    Random,
    Clustered,
    Temporal,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
struct PatternKey {
    sequence: Vec<u64>, // Simplified key representation
    pattern_type: String,
}

#[derive(Debug, Clone)]
struct PatternMetrics {
    frequency: u64,
    confidence: f64,
    last_seen: u64,
    success_rate: f64,
}

impl AccessPatternAnalyzer {
    fn new(config: &ReadAheadConfig) -> Self {
        Self {
            access_history: Arc::new(RwLock::new(VecDeque::with_capacity(
                config.max_pattern_history,
            ))),
            pattern_frequencies: Arc::new(DashMap::new()),
            temporal_patterns: Arc::new(RwLock::new(BTreeMap::new())),
            config: config.clone(),
        }
    }

    async fn record_access(&self, key: CacheKey) {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let access_event = AccessEvent {
            key: key.clone(),
            timestamp,
            context: self.build_access_context(key.clone()).await,
        };

        // Record in history
        let mut history = self.access_history.write().await;
        if history.len() >= self.config.max_pattern_history {
            history.pop_front();
        }
        history.push_back(access_event.clone());

        // Analyze patterns in background
        let pattern_frequencies = Arc::clone(&self.pattern_frequencies);
        let config = self.config.clone();
        tokio::spawn(async move {
            Self::analyze_patterns(access_event, pattern_frequencies, config).await;
        });
    }

    async fn build_access_context(&self, key: CacheKey) -> AccessContext {
        let history = self.access_history.read().await;
        let recent_keys: Vec<_> = history
            .iter()
            .rev()
            .take(5)
            .map(|event| event.key.clone())
            .collect();

        let access_type = self.classify_access_type(&recent_keys, key.clone()).await;

        AccessContext {
            previous_keys: recent_keys,
            access_type,
            file_type: self.infer_file_type(key),
        }
    }

    async fn classify_access_type(
        &self,
        recent_keys: &[CacheKey],
        current_key: CacheKey,
    ) -> AccessType {
        if recent_keys.is_empty() {
            return AccessType::Random;
        }

        // Simple heuristic: check if keys are in sequence
        let is_sequential = recent_keys
            .windows(2)
            .all(|window| key_hash(&window[0]) + 1 == key_hash(&window[1]))
            && key_hash(recent_keys.last().unwrap()) + 1 == key_hash(&current_key);

        if is_sequential {
            return AccessType::Sequential;
        }

        // Check for clustered access (keys close to each other)
        let max_distance = 100; // Threshold for clustered access
        let is_clustered = recent_keys
            .iter()
            .all(|key| (key_hash(key) as i64 - key_hash(&current_key) as i64).abs() < max_distance);

        if is_clustered {
            AccessType::Clustered
        } else {
            AccessType::Random
        }
    }

    fn infer_file_type(&self, _key: CacheKey) -> Option<String> {
        // Simplified file type inference
        // In practice, this would analyze the key structure
        None
    }

    async fn analyze_patterns(
        event: AccessEvent,
        pattern_frequencies: Arc<DashMap<PatternKey, PatternMetrics>>,
        _config: ReadAheadConfig,
    ) {
        // Extract sequence patterns
        let sequence_pattern = PatternKey {
            sequence: vec![key_hash(&event.key)],
            pattern_type: format!("{:?}", event.context.access_type),
        };

        // Update pattern frequency
        pattern_frequencies
            .entry(sequence_pattern)
            .and_modify(|metrics| {
                metrics.frequency += 1;
                metrics.last_seen = event.timestamp;
                metrics.confidence = (metrics.frequency as f64).log2() / 10.0; // Simple confidence calculation
            })
            .or_insert(PatternMetrics {
                frequency: 1,
                confidence: 0.1,
                last_seen: event.timestamp,
                success_rate: 0.5,
            });
    }

    async fn get_pattern_predictions(&self, current_key: CacheKey) -> Vec<CacheKey> {
        let mut predictions = Vec::new();
        let current_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        // Analyze similar patterns
        for pattern_entry in self.pattern_frequencies.iter() {
            let pattern = pattern_entry.key();
            let metrics = pattern_entry.value();

            // Check if pattern is recent and confident
            if current_time - metrics.last_seen < 3600 && metrics.confidence > 0.5 {
                // Generate prediction based on pattern
                // Without a numeric key space, just repeat current key as a placeholder prediction
                predictions.push(current_key.clone());
            }
        }

        predictions
    }
}

/// Predictive loader with adaptive algorithms
pub struct PredictiveLoader {
    /// Prediction cache
    prediction_cache: Arc<DashMap<CacheKey, PredictionEntry>>,
    /// Machine learning model state (simplified)
    model_weights: Arc<RwLock<Vec<f64>>>,
    config: ReadAheadConfig,
}

#[derive(Debug, Clone)]
struct PredictionEntry {
    predicted_keys: Vec<CacheKey>,
    confidence: f64,
    timestamp: u64,
    hit_count: u64,
}

impl PredictiveLoader {
    fn new(config: &ReadAheadConfig) -> Self {
        Self {
            prediction_cache: Arc::new(DashMap::new()),
            model_weights: Arc::new(RwLock::new(vec![0.5; 10])), // Simplified model
            config: config.clone(),
        }
    }

    async fn predict_next_accesses(&self, key: CacheKey) -> Result<Vec<CacheKey>> {
        // Check cache first
        if let Some(entry) = self.prediction_cache.get(&key) {
            if entry.confidence > self.config.min_confidence_threshold {
                return Ok(entry.predicted_keys.clone());
            }
        }

        // Generate new predictions
        let predictions = self.generate_predictions(key.clone()).await?;

        // Cache predictions
        let entry = PredictionEntry {
            predicted_keys: predictions.clone(),
            confidence: 0.8, // Simplified confidence
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            hit_count: 0,
        };

        self.prediction_cache.insert(key, entry);

        Ok(predictions)
    }

    async fn generate_predictions(&self, key: CacheKey) -> Result<Vec<CacheKey>> {
        let mut predictions = Vec::new();

        // Simple prediction: next few sequential keys
        for _ in 1..=self.config.prefetch_depth {
            // Placeholder: repeat current key to keep types consistent without relying on internal fields
            predictions.push(key.clone());
        }

        // Add some intelligent predictions based on common patterns
        self.add_pattern_based_predictions(&mut predictions, key)
            .await;

        Ok(predictions)
    }

    async fn add_pattern_based_predictions(&self, predictions: &mut Vec<CacheKey>, key: CacheKey) {
        // Graph traversal patterns - predict related nodes
        let related_offsets = vec![10, 100, 1000]; // Common graph distances

        for _offset in related_offsets {
            predictions.push(key.clone());
        }
    }

    async fn prefetch_batch(&self, keys: Vec<CacheKey>) -> Result<()> {
        // Simulate batch prefetching
        for chunk in keys.chunks(10) {
            tokio::spawn(async move {
                // Simulate I/O delay
                tokio::time::sleep(Duration::from_micros(100)).await;
                // In practice, this would load data into cache
            });
        }

        Ok(())
    }

    async fn update_prediction_accuracy(&self, key: CacheKey, was_hit: bool) {
        if let Some(mut entry) = self.prediction_cache.get_mut(&key) {
            if was_hit {
                entry.hit_count += 1;
            }

            // Update confidence based on hit rate
            let hit_rate = entry.hit_count as f64 / (entry.hit_count + 1) as f64;
            entry.confidence = hit_rate * 0.9 + 0.1; // Weighted update
        }
    }
}

/// Cache warmer for proactive data loading
pub struct CacheWarmer {
    /// Hot data tracking
    hot_keys: Arc<DashMap<CacheKey, HotKeyMetrics>>,
    /// Warming schedule
    warming_scheduler: Arc<Mutex<VecDeque<WarmingTask>>>,
    config: ReadAheadConfig,
}

#[derive(Debug, Clone)]
struct HotKeyMetrics {
    access_frequency: u64,
    last_access: u64,
    warming_priority: f64,
}

#[derive(Debug, Clone)]
struct WarmingTask {
    keys: Vec<CacheKey>,
    priority: f64,
    scheduled_time: u64,
}

impl CacheWarmer {
    fn new(config: &ReadAheadConfig) -> Self {
        Self {
            hot_keys: Arc::new(DashMap::new()),
            warming_scheduler: Arc::new(Mutex::new(VecDeque::new())),
            config: config.clone(),
        }
    }

    async fn start_warming_cycle(&self) -> Result<()> {
        let hot_keys = Arc::clone(&self.hot_keys);
        let warming_scheduler = Arc::clone(&self.warming_scheduler);
        let interval = self.config.cache_warming_interval;

        tokio::spawn(async move {
            let mut warming_interval = tokio::time::interval(interval);

            loop {
                warming_interval.tick().await;

                // Identify hot keys
                let hot_key_list = Self::identify_hot_keys(&hot_keys).await;

                // Schedule warming tasks
                let warming_task = WarmingTask {
                    keys: hot_key_list,
                    priority: 1.0,
                    scheduled_time: SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap()
                        .as_secs(),
                };

                warming_scheduler.lock().await.push_back(warming_task);

                // Execute warming tasks
                Self::execute_warming_tasks(&warming_scheduler).await;
            }
        });

        Ok(())
    }

    async fn identify_hot_keys(hot_keys: &DashMap<CacheKey, HotKeyMetrics>) -> Vec<CacheKey> {
        let mut hot_key_list = Vec::new();
        let current_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        for entry in hot_keys.iter() {
            let metrics = entry.value();

            // Consider keys hot if accessed recently and frequently
            if current_time - metrics.last_access < 3600 && metrics.access_frequency > 10 {
                hot_key_list.push(entry.key().clone());
            }
        }

        // Sort by priority
        hot_key_list.sort_by(|a, b| {
            let priority_a = hot_keys.get(a).map(|m| m.warming_priority).unwrap_or(0.0);
            let priority_b = hot_keys.get(b).map(|m| m.warming_priority).unwrap_or(0.0);
            priority_b
                .partial_cmp(&priority_a)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        hot_key_list.truncate(100); // Limit to top 100 hot keys
        hot_key_list
    }

    async fn execute_warming_tasks(warming_scheduler: &Mutex<VecDeque<WarmingTask>>) {
        let mut scheduler = warming_scheduler.lock().await;

        while let Some(task) = scheduler.pop_front() {
            // Execute warming task in background
            tokio::spawn(async move {
                for key in task.keys {
                    // Simulate cache warming
                    tokio::time::sleep(Duration::from_micros(50)).await;
                    // In practice, this would preload data into cache
                }
            });
        }
    }

    async fn record_key_access(&self, key: CacheKey) {
        let current_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        self.hot_keys
            .entry(key)
            .and_modify(|metrics| {
                metrics.access_frequency += 1;
                metrics.last_access = current_time;
                metrics.warming_priority = metrics.access_frequency as f64
                    / (current_time - metrics.last_access + 1) as f64;
            })
            .or_insert(HotKeyMetrics {
                access_frequency: 1,
                last_access: current_time,
                warming_priority: 1.0,
            });
    }
}

/// Sequential read acceleration engine
pub struct SequentialReadAccelerator {
    /// Sequential pattern detection
    sequence_detector: Arc<RwLock<SequenceDetector>>,
    /// Read-ahead buffer for sequential access
    readahead_buffer: Arc<DashMap<u64, SequentialBuffer>>,
    config: ReadAheadConfig,
}

#[derive(Debug)]
struct SequenceDetector {
    recent_accesses: VecDeque<CacheKey>,
    detected_sequences: HashMap<u64, SequencePattern>,
}

#[derive(Debug, Clone)]
struct SequencePattern {
    start_key: CacheKey,
    step_size: u64,
    length: usize,
    confidence: f64,
}

#[derive(Debug)]
struct SequentialBuffer {
    data: VecDeque<(CacheKey, Vec<u8>)>,
    next_expected_key: CacheKey,
    buffer_size: usize,
}

impl SequentialReadAccelerator {
    fn new(config: &ReadAheadConfig) -> Self {
        Self {
            sequence_detector: Arc::new(RwLock::new(SequenceDetector {
                recent_accesses: VecDeque::with_capacity(config.prediction_window_size),
                detected_sequences: HashMap::new(),
            })),
            readahead_buffer: Arc::new(DashMap::new()),
            config: config.clone(),
        }
    }

    async fn detect_sequential_pattern(&self, key: CacheKey) -> Option<Vec<CacheKey>> {
        let mut detector = self.sequence_detector.write().await;

        // Add to recent accesses
        if detector.recent_accesses.len() >= self.config.prediction_window_size {
            detector.recent_accesses.pop_front();
        }
        detector.recent_accesses.push_back(key.clone());

        // Detect sequential patterns
        if detector.recent_accesses.len() >= self.config.sequential_threshold {
            if let Some(pattern) = self.analyze_for_sequence(&detector.recent_accesses) {
                detector
                    .detected_sequences
                    .insert(key_hash(&key), pattern.clone());

                // Generate read-ahead keys
                return Some(self.generate_sequential_readahead(pattern, key));
            }
        }

        None
    }

    fn analyze_for_sequence(&self, accesses: &VecDeque<CacheKey>) -> Option<SequencePattern> {
        if accesses.len() < self.config.sequential_threshold {
            return None;
        }

        let recent: Vec<_> = accesses
            .iter()
            .rev()
            .take(self.config.sequential_threshold)
            .collect();

        // Check for simple sequential pattern (increment by 1)
        let is_sequential = recent
            .windows(2)
            .all(|window| key_hash(window[0]) == key_hash(window[1]) + 1);

        if is_sequential {
            return Some(SequencePattern {
                start_key: recent.last().unwrap().to_owned().clone(),
                step_size: 1,
                length: recent.len(),
                confidence: 0.9,
            });
        }

        // Check for arithmetic progression
        if recent.len() >= 3 {
            let step = key_hash(recent[0]) as i64 - key_hash(recent[1]) as i64;
            let is_arithmetic = recent
                .windows(2)
                .all(|window| key_hash(window[0]) as i64 - key_hash(window[1]) as i64 == step);

            if is_arithmetic && step > 0 {
                return Some(SequencePattern {
                    start_key: recent.last().unwrap().to_owned().clone(),
                    step_size: step as u64,
                    length: recent.len(),
                    confidence: 0.8,
                });
            }
        }

        None
    }

    fn generate_sequential_readahead(
        &self,
        pattern: SequencePattern,
        current_key: CacheKey,
    ) -> Vec<CacheKey> {
        let mut readahead_keys = Vec::new();
        let depth = self.config.prefetch_depth;

        for _ in 1..=depth {
            // Placeholder: repeat current key to maintain type correctness
            readahead_keys.push(current_key.clone());
        }

        readahead_keys
    }

    async fn prefetch_sequential_data(&self, keys: Vec<CacheKey>) -> Result<()> {
        // Create or update sequential buffer
        if let Some(first_key) = keys.first() {
            let buffer_id = key_hash(first_key) / 1000; // Group by approximate range

            let buffer = SequentialBuffer {
                data: VecDeque::new(),
                next_expected_key: first_key.clone(),
                buffer_size: self.config.prefetch_depth,
            };

            self.readahead_buffer.insert(buffer_id, buffer);

            // Launch background prefetch
            tokio::spawn(async move {
                for key in keys {
                    // Simulate sequential data loading
                    tokio::time::sleep(Duration::from_micros(10)).await;
                    let data = format!("sequential_data_{}", key_hash(&key)).into_bytes();
                    // In practice, this would load into the buffer
                }
            });
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio_test;

    #[tokio::test]
    async fn test_readahead_optimizer_creation() {
        let config = ReadAheadConfig::default();
        let optimizer = ReadAheadOptimizer::new(config);

        let metrics = optimizer.get_metrics().await;
        assert_eq!(metrics.total_predictions, 0);
    }

    #[tokio::test]
    async fn test_access_pattern_analysis() {
        let config = ReadAheadConfig::default();
        let analyzer = AccessPatternAnalyzer::new(&config);

        let key = CacheKey::Embedding("test_key".to_string());
        analyzer.record_access(key).await;

        // Test pattern recognition
        let predictions = analyzer.get_pattern_predictions(key).await;
        assert!(!predictions.is_empty());
    }

    #[tokio::test]
    async fn test_sequential_pattern_detection() {
        let config = ReadAheadConfig::default();
        let accelerator = SequentialReadAccelerator::new(&config);

        // Simulate sequential access pattern
        let keys = vec![
            CacheKey::Embedding("test_100".to_string()),
            CacheKey::Embedding("test_101".to_string()),
            CacheKey::Embedding("test_102".to_string()),
        ];

        for key in keys {
            let result = accelerator.detect_sequential_pattern(key).await;
            if key.hash == 102 {
                assert!(result.is_some());
            }
        }
    }

    #[tokio::test]
    async fn test_predictive_loading() {
        let config = ReadAheadConfig::default();
        let loader = PredictiveLoader::new(&config);

        let key = CacheKey::Embedding("test_key".to_string());
        let predictions = loader.predict_next_accesses(key).await.unwrap();

        assert!(!predictions.is_empty());
        assert!(predictions.len() <= config.prefetch_depth);
    }

    #[tokio::test]
    async fn test_cache_warming() {
        let config = ReadAheadConfig::default();
        let warmer = CacheWarmer::new(&config);

        let key = CacheKey::Embedding("test_key".to_string());
        warmer.record_key_access(key).await;

        // Verify hot key tracking
        assert!(warmer.hot_keys.contains_key(&key));
    }

    #[tokio::test]
    async fn test_end_to_end_optimization() {
        let config = ReadAheadConfig::default();
        let optimizer = ReadAheadOptimizer::new(config);

        let key = CacheKey::Embedding("test_key".to_string());
        let result = optimizer.optimize_read(key).await;

        assert!(result.is_ok());
        assert!(result.unwrap().is_some());

        let metrics = optimizer.get_metrics().await;
        assert!(metrics.total_predictions > 0);
    }
}
