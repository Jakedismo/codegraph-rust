use crate::embedding::EmbeddingError;
use candle_core::{Tensor, Result as CandleResult};
use serde::{Serialize, Deserialize};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct SparseEncoder {
    config: SparsityConfig,
    threshold_cache: HashMap<String, f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SparsityConfig {
    pub threshold: f32,
    pub target_sparsity: f32,
    pub method: SparsityMethod,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SparsityMethod {
    Magnitude,      // Zero out values below magnitude threshold
    Topk,          // Keep only top-k values
    Structured,    // Structured sparsity (blocks)
    Adaptive,      // Adaptive thresholding
}

#[derive(Debug, Clone)]
pub struct SparseRepresentation {
    pub indices: Vec<usize>,
    pub values: Vec<f32>,
    pub shape: Vec<usize>,
    pub sparsity_ratio: f32,
}

#[derive(Debug, Clone)]
pub struct SparsityAnalysis {
    pub original_size: usize,
    pub sparse_size: usize,
    pub sparsity_ratio: f32,
    pub compression_ratio: f32,
    pub zero_count: usize,
    pub non_zero_count: usize,
}

impl SparseEncoder {
    pub fn new(config: SparsityConfig) -> Self {
        Self {
            config,
            threshold_cache: HashMap::new(),
        }
    }

    pub fn encode(&self, data: &Tensor) -> Result<Tensor, EmbeddingError> {
        match self.config.method {
            SparsityMethod::Magnitude => self.magnitude_pruning(data),
            SparsityMethod::Topk => self.topk_pruning(data),
            SparsityMethod::Structured => self.structured_pruning(data),
            SparsityMethod::Adaptive => self.adaptive_pruning(data),
        }
    }

    pub fn encode_to_sparse(&self, data: &Tensor) -> Result<SparseRepresentation, EmbeddingError> {
        let sparse_tensor = self.encode(data)?;
        let data_vec = sparse_tensor.to_vec1::<f32>().map_err(EmbeddingError::CandleError)?;
        
        let mut indices = Vec::new();
        let mut values = Vec::new();
        let mut zero_count = 0;

        for (i, &value) in data_vec.iter().enumerate() {
            if value.abs() > f32::EPSILON {
                indices.push(i);
                values.push(value);
            } else {
                zero_count += 1;
            }
        }

        let sparsity_ratio = zero_count as f32 / data_vec.len() as f32;

        Ok(SparseRepresentation {
            indices,
            values,
            shape: sparse_tensor.shape().dims().to_vec(),
            sparsity_ratio,
        })
    }

    pub fn decode_from_sparse(&self, sparse_repr: &SparseRepresentation) -> Result<Tensor, EmbeddingError> {
        let total_size: usize = sparse_repr.shape.iter().product();
        let mut dense_data = vec![0.0f32; total_size];

        for (&index, &value) in sparse_repr.indices.iter().zip(sparse_repr.values.iter()) {
            if index < total_size {
                dense_data[index] = value;
            }
        }

        Tensor::new(dense_data.as_slice(), &candle_core::Device::Cpu)
            .map_err(EmbeddingError::CandleError)?
            .reshape(sparse_repr.shape.as_slice())
            .map_err(EmbeddingError::CandleError)
    }

    pub fn analyze_sparsity(&self, data: &Tensor) -> Result<SparsityAnalysis, EmbeddingError> {
        let data_vec = data.to_vec1::<f32>().map_err(EmbeddingError::CandleError)?;
        let original_size = data_vec.len() * std::mem::size_of::<f32>();
        
        let zero_count = data_vec.iter().filter(|&&x| x.abs() <= f32::EPSILON).count();
        let non_zero_count = data_vec.len() - zero_count;
        
        // Sparse representation size: indices (usize) + values (f32) + metadata
        let sparse_size = non_zero_count * (std::mem::size_of::<usize>() + std::mem::size_of::<f32>()) + 64; // metadata overhead
        
        let sparsity_ratio = zero_count as f32 / data_vec.len() as f32;
        let compression_ratio = original_size as f32 / sparse_size as f32;

        Ok(SparsityAnalysis {
            original_size,
            sparse_size,
            sparsity_ratio,
            compression_ratio,
            zero_count,
            non_zero_count,
        })
    }

    pub fn calibrate_threshold(&mut self, calibration_data: &[Tensor], key: &str) -> Result<f32, EmbeddingError> {
        let mut all_values = Vec::new();
        
        for tensor in calibration_data {
            let values = tensor.to_vec1::<f32>().map_err(EmbeddingError::CandleError)?;
            all_values.extend(values.into_iter().map(|x| x.abs()));
        }
        
        all_values.sort_by(|a, b| a.partial_cmp(b).unwrap());
        
        let threshold = match self.config.method {
            SparsityMethod::Magnitude => {
                // Use percentile-based threshold
                let percentile_idx = (all_values.len() as f32 * self.config.target_sparsity) as usize;
                all_values[percentile_idx.min(all_values.len() - 1)]
            }
            SparsityMethod::Topk => {
                // Find threshold that keeps target percentage of values
                let keep_count = (all_values.len() as f32 * (1.0 - self.config.target_sparsity)) as usize;
                let threshold_idx = all_values.len().saturating_sub(keep_count);
                all_values[threshold_idx]
            }
            _ => self.config.threshold,
        };

        self.threshold_cache.insert(key.to_string(), threshold);
        Ok(threshold)
    }

    fn magnitude_pruning(&self, data: &Tensor) -> Result<Tensor, EmbeddingError> {
        let data_vec = data.to_vec1::<f32>().map_err(EmbeddingError::CandleError)?;
        
        let pruned: Vec<f32> = data_vec
            .into_iter()
            .map(|x| if x.abs() < self.config.threshold { 0.0 } else { x })
            .collect();

        Tensor::new(pruned.as_slice(), &candle_core::Device::Cpu)
            .map_err(EmbeddingError::CandleError)?
            .reshape(data.shape().dims())
            .map_err(EmbeddingError::CandleError)
    }

    fn topk_pruning(&self, data: &Tensor) -> Result<Tensor, EmbeddingError> {
        let data_vec = data.to_vec1::<f32>().map_err(EmbeddingError::CandleError)?;
        let k = ((data_vec.len() as f32) * (1.0 - self.config.target_sparsity)) as usize;
        
        let mut indexed_values: Vec<(usize, f32)> = data_vec
            .iter()
            .enumerate()
            .map(|(i, &val)| (i, val.abs()))
            .collect();
        
        // Sort by absolute value, descending
        indexed_values.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        
        // Keep only top-k values
        let mut pruned = vec![0.0f32; data_vec.len()];
        for &(original_idx, _) in indexed_values.iter().take(k) {
            pruned[original_idx] = data_vec[original_idx];
        }

        Tensor::new(pruned.as_slice(), &candle_core::Device::Cpu)
            .map_err(EmbeddingError::CandleError)?
            .reshape(data.shape().dims())
            .map_err(EmbeddingError::CandleError)
    }

    fn structured_pruning(&self, data: &Tensor) -> Result<Tensor, EmbeddingError> {
        let data_vec = data.to_vec1::<f32>().map_err(EmbeddingError::CandleError)?;
        let block_size = 4; // Process in blocks of 4
        
        let mut pruned = data_vec.clone();
        
        for chunk_start in (0..data_vec.len()).step_by(block_size) {
            let chunk_end = (chunk_start + block_size).min(data_vec.len());
            let chunk = &data_vec[chunk_start..chunk_end];
            
            // Calculate block magnitude
            let block_magnitude: f32 = chunk.iter().map(|x| x * x).sum::<f32>().sqrt();
            
            // Prune entire block if magnitude is below threshold
            if block_magnitude < self.config.threshold {
                for i in chunk_start..chunk_end {
                    pruned[i] = 0.0;
                }
            }
        }

        Tensor::new(pruned.as_slice(), &candle_core::Device::Cpu)
            .map_err(EmbeddingError::CandleError)?
            .reshape(data.shape().dims())
            .map_err(EmbeddingError::CandleError)
    }

    fn adaptive_pruning(&self, data: &Tensor) -> Result<Tensor, EmbeddingError> {
        let data_vec = data.to_vec1::<f32>().map_err(EmbeddingError::CandleError)?;
        
        // Calculate adaptive threshold based on data statistics
        let mean_abs: f32 = data_vec.iter().map(|x| x.abs()).sum::<f32>() / data_vec.len() as f32;
        let variance: f32 = data_vec.iter()
            .map(|x| (x.abs() - mean_abs).powi(2))
            .sum::<f32>() / data_vec.len() as f32;
        let std_dev = variance.sqrt();
        
        let adaptive_threshold = mean_abs + (std_dev * self.config.threshold);
        
        let pruned: Vec<f32> = data_vec
            .into_iter()
            .map(|x| if x.abs() < adaptive_threshold { 0.0 } else { x })
            .collect();

        Tensor::new(pruned.as_slice(), &candle_core::Device::Cpu)
            .map_err(EmbeddingError::CandleError)?
            .reshape(data.shape().dims())
            .map_err(EmbeddingError::CandleError)
    }

    pub fn get_config(&self) -> &SparsityConfig {
        &self.config
    }

    pub fn get_cached_threshold(&self, key: &str) -> Option<f32> {
        self.threshold_cache.get(key).copied()
    }
}

#[derive(Debug, Clone)]
pub struct SparseOptimizer {
    encoders: Vec<SparseEncoder>,
    optimization_target: OptimizationTarget,
}

#[derive(Debug, Clone, PartialEq)]
pub enum OptimizationTarget {
    MinimizeMemory,
    MaximizeSpeed,
    BalanceQuality,
}

impl SparseOptimizer {
    pub fn new(optimization_target: OptimizationTarget) -> Self {
        let encoders = match optimization_target {
            OptimizationTarget::MinimizeMemory => {
                vec![
                    SparseEncoder::new(SparsityConfig {
                        threshold: 0.1,
                        target_sparsity: 0.8,
                        method: SparsityMethod::Magnitude,
                    }),
                    SparseEncoder::new(SparsityConfig {
                        threshold: 0.05,
                        target_sparsity: 0.9,
                        method: SparsityMethod::Structured,
                    }),
                ]
            }
            OptimizationTarget::MaximizeSpeed => {
                vec![
                    SparseEncoder::new(SparsityConfig {
                        threshold: 0.01,
                        target_sparsity: 0.5,
                        method: SparsityMethod::Topk,
                    }),
                ]
            }
            OptimizationTarget::BalanceQuality => {
                vec![
                    SparseEncoder::new(SparsityConfig {
                        threshold: 0.02,
                        target_sparsity: 0.6,
                        method: SparsityMethod::Adaptive,
                    }),
                ]
            }
        };

        Self {
            encoders,
            optimization_target,
        }
    }

    pub fn find_best_encoder(&self, data: &Tensor) -> Result<(usize, SparsityAnalysis), EmbeddingError> {
        let mut best_encoder_idx = 0;
        let mut best_analysis = None;
        let mut best_score = f32::NEG_INFINITY;

        for (idx, encoder) in self.encoders.iter().enumerate() {
            let sparse_data = encoder.encode(data)?;
            let analysis = encoder.analyze_sparsity(&sparse_data)?;
            
            let score = self.compute_optimization_score(&analysis);
            
            if score > best_score {
                best_score = score;
                best_encoder_idx = idx;
                best_analysis = Some(analysis);
            }
        }

        Ok((best_encoder_idx, best_analysis.unwrap()))
    }

    fn compute_optimization_score(&self, analysis: &SparsityAnalysis) -> f32 {
        match self.optimization_target {
            OptimizationTarget::MinimizeMemory => {
                // Prioritize compression ratio and sparsity
                analysis.compression_ratio * 0.7 + analysis.sparsity_ratio * 0.3
            }
            OptimizationTarget::MaximizeSpeed => {
                // Prioritize moderate sparsity for fast operations
                let optimal_sparsity = 0.5;
                let sparsity_penalty = (analysis.sparsity_ratio - optimal_sparsity).abs();
                analysis.compression_ratio - sparsity_penalty * 2.0
            }
            OptimizationTarget::BalanceQuality => {
                // Balanced approach - moderate compression with quality preservation
                let quality_score = 1.0 - (analysis.sparsity_ratio - 0.5).abs(); // Prefer ~50% sparsity
                analysis.compression_ratio * 0.4 + quality_score * 0.6
            }
        }
    }
}

impl Default for SparsityConfig {
    fn default() -> Self {
        Self {
            threshold: 0.01,
            target_sparsity: 0.5,
            method: SparsityMethod::Magnitude,
        }
    }
}

impl SparsityConfig {
    pub fn for_aggressive_pruning() -> Self {
        Self {
            threshold: 0.1,
            target_sparsity: 0.9,
            method: SparsityMethod::Structured,
        }
    }

    pub fn for_conservative_pruning() -> Self {
        Self {
            threshold: 0.001,
            target_sparsity: 0.2,
            method: SparsityMethod::Magnitude,
        }
    }

    pub fn for_topk_pruning(k_ratio: f32) -> Self {
        Self {
            threshold: 0.0, // Not used for topk
            target_sparsity: 1.0 - k_ratio,
            method: SparsityMethod::Topk,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use candle_core::Device;

    #[test]
    fn test_sparsity_config_presets() {
        let aggressive = SparsityConfig::for_aggressive_pruning();
        assert_eq!(aggressive.target_sparsity, 0.9);
        assert_eq!(aggressive.method, SparsityMethod::Structured);

        let conservative = SparsityConfig::for_conservative_pruning();
        assert_eq!(conservative.target_sparsity, 0.2);
        assert_eq!(conservative.method, SparsityMethod::Magnitude);

        let topk = SparsityConfig::for_topk_pruning(0.1);
        assert_eq!(topk.target_sparsity, 0.9);
        assert_eq!(topk.method, SparsityMethod::Topk);
    }

    #[tokio::test]
    async fn test_magnitude_pruning() {
        let device = Device::Cpu;
        let config = SparsityConfig {
            threshold: 0.5,
            target_sparsity: 0.5,
            method: SparsityMethod::Magnitude,
        };
        
        let encoder = SparseEncoder::new(config);
        let data = Tensor::new(&[0.1f32, 0.8, 0.3, 1.2, 0.05, 2.0], &device).unwrap();
        
        let sparse_data = encoder.encode(&data).unwrap();
        let result = sparse_data.to_vec1::<f32>().unwrap();
        
        // Values below threshold should be zero
        assert_eq!(result[0], 0.0); // 0.1 < 0.5
        assert_eq!(result[1], 0.8); // 0.8 >= 0.5
        assert_eq!(result[2], 0.0); // 0.3 < 0.5
        assert_eq!(result[3], 1.2); // 1.2 >= 0.5
        assert_eq!(result[4], 0.0); // 0.05 < 0.5
        assert_eq!(result[5], 2.0); // 2.0 >= 0.5
    }

    #[tokio::test]
    async fn test_sparse_representation() {
        let device = Device::Cpu;
        let config = SparsityConfig::default();
        let encoder = SparseEncoder::new(config);
        
        let data = Tensor::new(&[0.0f32, 1.5, 0.0, 2.3, 0.0, 0.8], &device).unwrap();
        let sparse_repr = encoder.encode_to_sparse(&data).unwrap();
        
        // Should have 3 non-zero values
        assert_eq!(sparse_repr.values.len(), 3);
        assert_eq!(sparse_repr.indices.len(), 3);
        
        // Check sparsity ratio
        assert_eq!(sparse_repr.sparsity_ratio, 0.5); // 3 zeros out of 6 values
        
        // Test reconstruction
        let reconstructed = encoder.decode_from_sparse(&sparse_repr).unwrap();
        let reconstructed_vec = reconstructed.to_vec1::<f32>().unwrap();
        
        assert_eq!(reconstructed_vec[1], 1.5);
        assert_eq!(reconstructed_vec[3], 2.3);
        assert_eq!(reconstructed_vec[5], 0.8);
    }

    #[test]
    fn test_sparse_optimizer() {
        let optimizer = SparseOptimizer::new(OptimizationTarget::MinimizeMemory);
        assert_eq!(optimizer.optimization_target, OptimizationTarget::MinimizeMemory);
        assert_eq!(optimizer.encoders.len(), 2);

        let speed_optimizer = SparseOptimizer::new(OptimizationTarget::MaximizeSpeed);
        assert_eq!(speed_optimizer.encoders.len(), 1);
    }

    #[tokio::test]
    async fn test_sparsity_analysis() {
        let device = Device::Cpu;
        let encoder = SparseEncoder::new(SparsityConfig::default());
        
        let data = Tensor::new(&[1.0f32, 0.0, 2.0, 0.0, 3.0, 0.0], &device).unwrap();
        let analysis = encoder.analyze_sparsity(&data).unwrap();
        
        assert_eq!(analysis.zero_count, 3);
        assert_eq!(analysis.non_zero_count, 3);
        assert_eq!(analysis.sparsity_ratio, 0.5);
        assert!(analysis.compression_ratio > 1.0);
    }
}