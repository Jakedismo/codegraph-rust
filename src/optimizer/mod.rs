pub mod compression;
pub mod quantization;
pub mod sparse;

pub use compression::{PCACompressor, CompressionResult};
pub use quantization::{Quantizer, QuantizationConfig};
pub use sparse::{SparseEncoder, SparsityConfig};

use crate::embedding::EmbeddingError;
use candle_core::{Tensor, Device, Result as CandleResult};
use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PrecisionMode {
    Full,       // f32
    Half,       // f16
    Quantized,  // i8 with scale/offset
    Dynamic,    // Adaptive based on content
}

#[derive(Debug, Clone)]
pub struct EmbeddingOptimizer {
    compression_ratio: f32,
    target_dimensions: usize,
    precision_mode: PrecisionMode,
    compressor: Option<PCACompressor>,
    quantizer: Option<Quantizer>,
    sparse_encoder: Option<SparseEncoder>,
    device: Device,
}

#[derive(Debug, Clone)]
pub struct OptimizationConfig {
    pub target_dimensions: usize,
    pub compression_ratio: f32,
    pub precision_mode: PrecisionMode,
    pub sparsity_threshold: f32,
    pub enable_compression: bool,
    pub enable_quantization: bool,
    pub enable_sparsification: bool,
}

#[derive(Debug, Clone)]
pub struct OptimizationResult {
    pub original_size: usize,
    pub optimized_size: usize,
    pub compression_ratio: f32,
    pub memory_reduction: f32,
    pub performance_metrics: PerformanceMetrics,
}

#[derive(Debug, Clone)]
pub struct PerformanceMetrics {
    pub encoding_time_ms: f64,
    pub decoding_time_ms: f64,
    pub quality_score: f32, // Similarity to original
    pub memory_usage_bytes: usize,
}

impl EmbeddingOptimizer {
    pub fn new(config: OptimizationConfig, device: Device) -> Self {
        let compressor = if config.enable_compression {
            Some(PCACompressor::new(config.target_dimensions, device.clone()))
        } else {
            None
        };

        let quantizer = if config.enable_quantization {
            Some(Quantizer::new(QuantizationConfig::from_precision_mode(config.precision_mode)))
        } else {
            None
        };

        let sparse_encoder = if config.enable_sparsification {
            Some(SparseEncoder::new(SparsityConfig {
                threshold: config.sparsity_threshold,
                target_sparsity: 0.5,
                method: sparse::SparsityMethod::Magnitude,
            }))
        } else {
            None
        };

        Self {
            compression_ratio: config.compression_ratio,
            target_dimensions: config.target_dimensions,
            precision_mode: config.precision_mode,
            compressor,
            quantizer,
            sparse_encoder,
            device,
        }
    }

    pub fn optimize_embeddings(&self, embeddings: Vec<f32>) -> Result<Vec<f32>, EmbeddingError> {
        let start_time = std::time::Instant::now();
        
        // Convert to tensor for processing
        let tensor = Tensor::new(embeddings.as_slice(), &self.device)
            .map_err(EmbeddingError::CandleError)?;

        let mut optimized_tensor = tensor;
        let original_size = embeddings.len() * std::mem::size_of::<f32>();

        // Apply compression
        if let Some(ref compressor) = self.compressor {
            optimized_tensor = compressor.compress(&optimized_tensor)?;
        }

        // Apply sparsification
        if let Some(ref sparse_encoder) = self.sparse_encoder {
            optimized_tensor = sparse_encoder.encode(&optimized_tensor)?;
        }

        // Apply quantization (final step)
        let final_result = if let Some(ref quantizer) = self.quantizer {
            quantizer.quantize(&optimized_tensor)?
        } else {
            optimized_tensor.to_vec1::<f32>().map_err(EmbeddingError::CandleError)?
        };

        Ok(final_result)
    }

    pub fn optimize_batch(&self, embeddings_batch: Vec<Vec<f32>>) -> Result<Vec<Vec<f32>>, EmbeddingError> {
        let mut results = Vec::with_capacity(embeddings_batch.len());
        
        for embeddings in embeddings_batch {
            let optimized = self.optimize_embeddings(embeddings)?;
            results.push(optimized);
        }
        
        Ok(results)
    }

    pub fn train_compressor(&mut self, training_data: &[Vec<f32>]) -> Result<CompressionResult, EmbeddingError> {
        if let Some(ref mut compressor) = self.compressor {
            // Flatten training data for PCA
            let mut flat_data = Vec::new();
            for embedding in training_data {
                flat_data.extend_from_slice(embedding);
            }

            let data_shape = (training_data.len(), training_data[0].len());
            let tensor = Tensor::new(flat_data.as_slice(), &self.device)
                .map_err(EmbeddingError::CandleError)?
                .reshape(data_shape)?;

            compressor.fit(&tensor)
        } else {
            Err(EmbeddingError::InferenceError("No compressor configured".to_string()))
        }
    }

    pub fn get_target_dimensions(&self) -> usize {
        self.target_dimensions
    }

    pub fn get_compression_ratio(&self) -> f32 {
        self.compression_ratio
    }

    pub fn get_precision_mode(&self) -> PrecisionMode {
        self.precision_mode
    }

    pub fn estimate_memory_usage(&self, embedding_count: usize, original_dim: usize) -> MemoryUsageEstimate {
        let bytes_per_element = match self.precision_mode {
            PrecisionMode::Full => 4,     // f32
            PrecisionMode::Half => 2,     // f16
            PrecisionMode::Quantized => 1, // i8
            PrecisionMode::Dynamic => 2,  // Average
        };

        let effective_dimensions = if self.compressor.is_some() {
            self.target_dimensions
        } else {
            original_dim
        };

        let sparsity_factor = if self.sparse_encoder.is_some() { 0.5 } else { 1.0 };
        
        let optimized_size = (embedding_count * effective_dimensions) as f32 * bytes_per_element as f32 * sparsity_factor;
        let original_size = (embedding_count * original_dim * 4) as f32; // f32 baseline

        MemoryUsageEstimate {
            original_bytes: original_size as usize,
            optimized_bytes: optimized_size as usize,
            reduction_ratio: original_size / optimized_size,
            memory_saved_bytes: (original_size - optimized_size) as usize,
        }
    }

    pub fn benchmark_performance(&self, test_embeddings: Vec<Vec<f32>>) -> Result<BenchmarkResult, EmbeddingError> {
        let mut total_encoding_time = std::time::Duration::ZERO;
        let mut total_quality_loss = 0.0f32;
        let mut processed_count = 0;

        for original_embedding in test_embeddings.iter() {
            let start = std::time::Instant::now();
            let optimized = self.optimize_embeddings(original_embedding.clone())?;
            total_encoding_time += start.elapsed();

            // Measure quality loss (cosine similarity to original)
            let quality = self.compute_similarity(&original_embedding, &optimized);
            total_quality_loss += 1.0 - quality;
            processed_count += 1;
        }

        let memory_estimate = self.estimate_memory_usage(
            test_embeddings.len(), 
            test_embeddings[0].len()
        );

        Ok(BenchmarkResult {
            avg_encoding_time_ms: total_encoding_time.as_millis() as f64 / processed_count as f64,
            avg_quality_loss: total_quality_loss / processed_count as f32,
            memory_reduction_ratio: memory_estimate.reduction_ratio,
            throughput_embeddings_per_sec: (processed_count as f64 * 1000.0) / total_encoding_time.as_millis() as f64,
        })
    }

    fn compute_similarity(&self, a: &[f32], b: &[f32]) -> f32 {
        if a.len() != b.len() {
            return 0.0;
        }

        let dot_product: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
        let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
        let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

        if norm_a == 0.0 || norm_b == 0.0 {
            return 0.0;
        }

        dot_product / (norm_a * norm_b)
    }
}

#[derive(Debug, Clone)]
pub struct MemoryUsageEstimate {
    pub original_bytes: usize,
    pub optimized_bytes: usize,
    pub reduction_ratio: f32,
    pub memory_saved_bytes: usize,
}

#[derive(Debug, Clone)]
pub struct BenchmarkResult {
    pub avg_encoding_time_ms: f64,
    pub avg_quality_loss: f32,
    pub memory_reduction_ratio: f32,
    pub throughput_embeddings_per_sec: f64,
}

impl Default for OptimizationConfig {
    fn default() -> Self {
        Self {
            target_dimensions: 256,
            compression_ratio: 0.33,
            precision_mode: PrecisionMode::Half,
            sparsity_threshold: 0.01,
            enable_compression: true,
            enable_quantization: true,
            enable_sparsification: false,
        }
    }
}

impl OptimizationConfig {
    pub fn for_memory_constrained() -> Self {
        Self {
            target_dimensions: 128,
            compression_ratio: 0.17,
            precision_mode: PrecisionMode::Quantized,
            sparsity_threshold: 0.05,
            enable_compression: true,
            enable_quantization: true,
            enable_sparsification: true,
        }
    }

    pub fn for_quality_focused() -> Self {
        Self {
            target_dimensions: 512,
            compression_ratio: 0.67,
            precision_mode: PrecisionMode::Full,
            sparsity_threshold: 0.001,
            enable_compression: false,
            enable_quantization: false,
            enable_sparsification: false,
        }
    }

    pub fn for_balanced() -> Self {
        Self {
            target_dimensions: 256,
            compression_ratio: 0.33,
            precision_mode: PrecisionMode::Half,
            sparsity_threshold: 0.01,
            enable_compression: true,
            enable_quantization: true,
            enable_sparsification: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_optimization_config_presets() {
        let memory_config = OptimizationConfig::for_memory_constrained();
        assert_eq!(memory_config.target_dimensions, 128);
        assert!(memory_config.enable_sparsification);

        let quality_config = OptimizationConfig::for_quality_focused();
        assert_eq!(quality_config.target_dimensions, 512);
        assert!(!quality_config.enable_compression);

        let balanced_config = OptimizationConfig::for_balanced();
        assert_eq!(balanced_config.target_dimensions, 256);
        assert!(balanced_config.enable_compression);
    }

    #[test]
    fn test_memory_usage_estimation() {
        let config = OptimizationConfig::for_memory_constrained();
        let optimizer = EmbeddingOptimizer::new(config, Device::Cpu);
        
        let estimate = optimizer.estimate_memory_usage(1000, 768);
        assert!(estimate.original_bytes > estimate.optimized_bytes);
        assert!(estimate.reduction_ratio > 1.0);
    }
}