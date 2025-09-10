use crate::{CacheConfig, CacheEntry, CacheSizeEstimator};
use codegraph_core::{CodeGraphError, Result};
use parking_lot::Mutex;
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use sysinfo::System;
use tracing::{debug, info, warn};

/// Memory pressure levels
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryPressure {
    Low,     // < 70% of limit
    Medium,  // 70-85% of limit  
    High,    // 85-95% of limit
    Critical, // > 95% of limit
}

/// Memory optimization strategies
#[derive(Debug, Clone, PartialEq)]
pub enum OptimizationStrategy {
    /// Least Recently Used eviction
    Lru,
    /// Least Frequently Used eviction
    Lfu,
    /// Time-based eviction (oldest first)
    Fifo,
    /// Size-based eviction (largest first)
    SizeBased,
    /// Hybrid strategy combining multiple factors
    Hybrid {
        lru_weight: f32,
        lfu_weight: f32,
        size_weight: f32,
        age_weight: f32,
    },
}

impl Default for OptimizationStrategy {
    fn default() -> Self {
        Self::Hybrid {
            lru_weight: 0.4,
            lfu_weight: 0.3,
            size_weight: 0.2,
            age_weight: 0.1,
        }
    }
}

/// Memory manager for AI cache operations
pub struct MemoryManager {
    /// Maximum memory allowed in bytes
    max_memory_bytes: usize,
    /// Current memory usage in bytes
    current_usage: Arc<Mutex<usize>>,
    /// Optimization strategy
    strategy: OptimizationStrategy,
    /// System information for memory monitoring
    system: System,
    /// Memory pressure thresholds
    pressure_thresholds: MemoryPressureThresholds,
    /// Compression settings
    compression_config: CompressionConfig,
}

/// Memory pressure thresholds
#[derive(Debug, Clone)]
pub struct MemoryPressureThresholds {
    pub low_threshold: f32,    // 0.7
    pub medium_threshold: f32, // 0.85
    pub high_threshold: f32,   // 0.95
}

impl Default for MemoryPressureThresholds {
    fn default() -> Self {
        Self {
            low_threshold: 0.7,
            medium_threshold: 0.85,
            high_threshold: 0.95,
        }
    }
}

/// Compression configuration
#[derive(Debug, Clone)]
pub struct CompressionConfig {
    pub enabled: bool,
    pub threshold_bytes: usize,
    pub compression_ratio: f32, // Expected compression ratio
    pub algorithm: CompressionAlgorithm,
}

#[derive(Debug, Clone)]
pub enum CompressionAlgorithm {
    /// Simple quantization for floating-point vectors
    Quantization { bits: u8 },
    /// LZ4 compression for general data
    Lz4,
    /// Zstd compression for better ratios
    Zstd { level: i32 },
}

impl Default for CompressionConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            threshold_bytes: 1024, // 1KB
            compression_ratio: 0.6, // Expect 40% size reduction
            algorithm: CompressionAlgorithm::Quantization { bits: 8 },
        }
    }
}

/// Entry metadata for optimization decisions
#[derive(Debug, Clone)]
pub struct EntryMetadata {
    pub size_bytes: usize,
    pub access_count: u64,
    pub last_accessed: SystemTime,
    pub created_at: SystemTime,
    pub compression_ratio: Option<f32>,
}

impl MemoryManager {
    pub fn new(config: &CacheConfig) -> Self {
        Self {
            max_memory_bytes: config.max_memory_bytes,
            current_usage: Arc::new(Mutex::new(0)),
            strategy: OptimizationStrategy::default(),
            system: System::new_all(),
            pressure_thresholds: MemoryPressureThresholds::default(),
            compression_config: CompressionConfig::default(),
        }
    }

    /// Get current memory pressure level
    pub fn get_memory_pressure(&self) -> MemoryPressure {
        let current = *self.current_usage.lock();
        let ratio = current as f32 / self.max_memory_bytes as f32;

        if ratio < self.pressure_thresholds.low_threshold {
            MemoryPressure::Low
        } else if ratio < self.pressure_thresholds.medium_threshold {
            MemoryPressure::Medium
        } else if ratio < self.pressure_thresholds.high_threshold {
            MemoryPressure::High
        } else {
            MemoryPressure::Critical
        }
    }

    /// Add memory usage
    pub fn add_usage(&self, bytes: usize) {
        let mut usage = self.current_usage.lock();
        *usage += bytes;
    }

    /// Remove memory usage
    pub fn remove_usage(&self, bytes: usize) {
        let mut usage = self.current_usage.lock();
        *usage = usage.saturating_sub(bytes);
    }

    /// Get current memory usage in bytes
    pub fn get_current_usage(&self) -> usize {
        *self.current_usage.lock()
    }

    /// Get memory usage ratio (0.0 to 1.0)
    pub fn get_usage_ratio(&self) -> f32 {
        self.get_current_usage() as f32 / self.max_memory_bytes as f32
    }

    /// Calculate optimization score for an entry
    pub fn calculate_optimization_score(&self, metadata: &EntryMetadata) -> f32 {
        match &self.strategy {
            OptimizationStrategy::Lru => {
                // Higher score = more likely to be evicted
                let age = metadata.last_accessed.elapsed()
                    .unwrap_or(Duration::ZERO)
                    .as_secs_f32();
                age / 3600.0 // Normalize to hours
            }
            OptimizationStrategy::Lfu => {
                // Lower access count = higher score
                1.0 / (metadata.access_count as f32 + 1.0)
            }
            OptimizationStrategy::Fifo => {
                let age = metadata.created_at.elapsed()
                    .unwrap_or(Duration::ZERO)
                    .as_secs_f32();
                age / 3600.0
            }
            OptimizationStrategy::SizeBased => {
                // Larger items get higher scores
                metadata.size_bytes as f32 / (1024.0 * 1024.0) // Normalize to MB
            }
            OptimizationStrategy::Hybrid { lru_weight, lfu_weight, size_weight, age_weight } => {
                let lru_score = {
                    let age = metadata.last_accessed.elapsed()
                        .unwrap_or(Duration::ZERO)
                        .as_secs_f32();
                    age / 3600.0
                };
                
                let lfu_score = 1.0 / (metadata.access_count as f32 + 1.0);
                
                let size_score = metadata.size_bytes as f32 / (1024.0 * 1024.0);
                
                let age_score = {
                    let age = metadata.created_at.elapsed()
                        .unwrap_or(Duration::ZERO)
                        .as_secs_f32();
                    age / 3600.0
                };

                lru_score * lru_weight + 
                lfu_score * lfu_weight + 
                size_score * size_weight + 
                age_score * age_weight
            }
        }
    }

    /// Determine if an entry should be compressed
    pub fn should_compress(&self, size_bytes: usize) -> bool {
        self.compression_config.enabled && size_bytes >= self.compression_config.threshold_bytes
    }

    /// Estimate compression savings
    pub fn estimate_compression_savings(&self, original_size: usize) -> usize {
        if self.should_compress(original_size) {
            let saved = original_size as f32 * (1.0 - self.compression_config.compression_ratio);
            saved as usize
        } else {
            0
        }
    }

    /// Compress vector data using quantization
    pub fn compress_vector(&self, vector: &[f32]) -> Result<Vec<u8>> {
        match &self.compression_config.algorithm {
            CompressionAlgorithm::Quantization { bits } => {
                let scale = (1 << bits) - 1;
                let quantized: Vec<u8> = vector.iter()
                    .map(|&v| {
                        let normalized = (v + 1.0) / 2.0; // Assume values in [-1, 1]
                        let quantized = (normalized * scale as f32).round() as u8;
                        quantized.min(scale as u8)
                    })
                    .collect();
                Ok(quantized)
            }
            _ => {
                // For other algorithms, we'd use external compression libraries
                Err(CodeGraphError::Vector("Unsupported compression algorithm".to_string()))
            }
        }
    }

    /// Decompress vector data
    pub fn decompress_vector(&self, compressed: &[u8], original_len: usize) -> Result<Vec<f32>> {
        match &self.compression_config.algorithm {
            CompressionAlgorithm::Quantization { bits } => {
                let scale = (1 << bits) - 1;
                let decompressed: Vec<f32> = compressed.iter()
                    .map(|&q| {
                        let normalized = q as f32 / scale as f32;
                        normalized * 2.0 - 1.0 // Convert back to [-1, 1]
                    })
                    .collect();
                Ok(decompressed)
            }
            _ => {
                Err(CodeGraphError::Vector("Unsupported compression algorithm".to_string()))
            }
        }
    }

    /// Get system memory information
    pub fn get_system_memory_info(&mut self) -> SystemMemoryInfo {
        self.system.refresh_memory();
        
        SystemMemoryInfo {
            total_memory_kb: self.system.total_memory(),
            available_memory_kb: self.system.available_memory(),
            used_memory_kb: self.system.used_memory(),
            cache_usage_bytes: self.get_current_usage(),
            cache_limit_bytes: self.max_memory_bytes,
        }
    }

    /// Check if immediate eviction is needed
    pub fn needs_immediate_eviction(&self) -> bool {
        matches!(self.get_memory_pressure(), MemoryPressure::Critical)
    }

    /// Calculate target eviction size to reach safe memory level
    pub fn calculate_target_eviction_size(&self) -> usize {
        let current = self.get_current_usage();
        let target_ratio = self.pressure_thresholds.medium_threshold;
        let target_usage = (self.max_memory_bytes as f32 * target_ratio) as usize;
        
        if current > target_usage {
            current - target_usage
        } else {
            0
        }
    }

    /// Update compression configuration
    pub fn update_compression_config(&mut self, config: CompressionConfig) {
        self.compression_config = config;
    }

    /// Get memory optimization recommendations
    pub fn get_optimization_recommendations(&self) -> Vec<OptimizationRecommendation> {
        let mut recommendations = Vec::new();
        let pressure = self.get_memory_pressure();
        let usage_ratio = self.get_usage_ratio();

        match pressure {
            MemoryPressure::Low => {
                if usage_ratio > 0.5 {
                    recommendations.push(OptimizationRecommendation::EnableCompression);
                }
            }
            MemoryPressure::Medium => {
                recommendations.push(OptimizationRecommendation::AggressiveCompression);
                recommendations.push(OptimizationRecommendation::ReduceTtl);
            }
            MemoryPressure::High => {
                recommendations.push(OptimizationRecommendation::ImmediateEviction { 
                    target_mb: self.calculate_target_eviction_size() / (1024 * 1024) 
                });
                recommendations.push(OptimizationRecommendation::IncreaseMemoryLimit);
            }
            MemoryPressure::Critical => {
                recommendations.push(OptimizationRecommendation::EmergencyEviction);
                recommendations.push(OptimizationRecommendation::ReduceCacheSize);
            }
        }

        recommendations
    }
}

/// System memory information
#[derive(Debug, Clone)]
pub struct SystemMemoryInfo {
    pub total_memory_kb: u64,
    pub available_memory_kb: u64,
    pub used_memory_kb: u64,
    pub cache_usage_bytes: usize,
    pub cache_limit_bytes: usize,
}

/// Memory optimization recommendations
#[derive(Debug, Clone)]
pub enum OptimizationRecommendation {
    EnableCompression,
    AggressiveCompression,
    ReduceTtl,
    ImmediateEviction { target_mb: usize },
    IncreaseMemoryLimit,
    EmergencyEviction,
    ReduceCacheSize,
}

/// LRU (Least Recently Used) implementation
pub struct LruManager<K> {
    /// Maximum capacity
    capacity: usize,
    /// Access order queue (front = least recent, back = most recent)
    access_queue: VecDeque<K>,
    /// Key to access order mapping for O(1) updates
    key_positions: HashMap<K, usize>,
}

impl<K> LruManager<K>
where
    K: Clone + Eq + std::hash::Hash,
{
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity,
            access_queue: VecDeque::with_capacity(capacity),
            key_positions: HashMap::with_capacity(capacity),
        }
    }

    /// Mark key as accessed (move to most recent position)
    pub fn access(&mut self, key: K) {
        // Remove from current position
        if let Some(&pos) = self.key_positions.get(&key) {
            self.access_queue.remove(pos);
            // Update positions for all elements after the removed one
            for (k, p) in self.key_positions.iter_mut() {
                if *p > pos {
                    *p -= 1;
                }
            }
        }

        // Add to back (most recent)
        self.access_queue.push_back(key.clone());
        self.key_positions.insert(key, self.access_queue.len() - 1);

        // Maintain capacity
        while self.access_queue.len() > self.capacity {
            if let Some(oldest) = self.access_queue.pop_front() {
                self.key_positions.remove(&oldest);
                // Update positions
                for (_, p) in self.key_positions.iter_mut() {
                    *p -= 1;
                }
            }
        }
    }

    /// Get the least recently used key
    pub fn get_lru(&self) -> Option<&K> {
        self.access_queue.front()
    }

    /// Remove a key from tracking
    pub fn remove(&mut self, key: &K) {
        if let Some(&pos) = self.key_positions.get(key) {
            self.access_queue.remove(pos);
            self.key_positions.remove(key);
            
            // Update positions for all elements after the removed one
            for (_, p) in self.key_positions.iter_mut() {
                if *p > pos {
                    *p -= 1;
                }
            }
        }
    }

    /// Get current size
    pub fn len(&self) -> usize {
        self.access_queue.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.access_queue.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_pressure_calculation() {
        let config = CacheConfig {
            max_memory_bytes: 1000,
            ..Default::default()
        };
        let manager = MemoryManager::new(&config);

        // Low pressure
        manager.add_usage(600);
        assert_eq!(manager.get_memory_pressure(), MemoryPressure::Low);

        // Medium pressure
        manager.add_usage(200);
        assert_eq!(manager.get_memory_pressure(), MemoryPressure::Medium);

        // High pressure
        manager.add_usage(100);
        assert_eq!(manager.get_memory_pressure(), MemoryPressure::High);

        // Critical pressure
        manager.add_usage(150);
        assert_eq!(manager.get_memory_pressure(), MemoryPressure::Critical);
    }

    #[test]
    fn test_compression_threshold() {
        let config = CacheConfig::default();
        let manager = MemoryManager::new(&config);

        assert!(!manager.should_compress(512)); // Below threshold
        assert!(manager.should_compress(2048)); // Above threshold
    }

    #[test]
    fn test_vector_compression() {
        let config = CacheConfig::default();
        let manager = MemoryManager::new(&config);

        let vector = vec![0.5, -0.3, 0.8, -1.0, 1.0];
        let compressed = manager.compress_vector(&vector).unwrap();
        let decompressed = manager.decompress_vector(&compressed, vector.len()).unwrap();

        // Check that decompressed values are approximately equal (quantization loses precision)
        for (orig, decomp) in vector.iter().zip(decompressed.iter()) {
            assert!((orig - decomp).abs() < 0.1);
        }
    }

    #[test]
    fn test_lru_manager() {
        let mut lru = LruManager::new(3);

        lru.access("a");
        lru.access("b");
        lru.access("c");
        assert_eq!(lru.get_lru(), Some(&"a"));

        lru.access("d"); // Should evict "a"
        assert_eq!(lru.get_lru(), Some(&"b"));

        lru.access("b"); // Move "b" to most recent
        assert_eq!(lru.get_lru(), Some(&"c"));

        lru.remove(&"c");
        assert_eq!(lru.get_lru(), Some(&"d"));
        assert_eq!(lru.len(), 2);
    }

    #[test]
    fn test_optimization_score_calculation() {
        let config = CacheConfig::default();
        let manager = MemoryManager::new(&config);

        let metadata = EntryMetadata {
            size_bytes: 1024,
            access_count: 5,
            last_accessed: SystemTime::now() - Duration::from_secs(3600), // 1 hour ago
            created_at: SystemTime::now() - Duration::from_secs(7200), // 2 hours ago
            compression_ratio: Some(0.6),
        };

        let score = manager.calculate_optimization_score(&metadata);
        assert!(score > 0.0);
    }
}
