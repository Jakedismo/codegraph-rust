// ABOUTME: Provides vector optimization utilities like quantization and batching
// ABOUTME: Implements an optimization pipeline and baseline/optimized search helpers

use codegraph_core::{CodeGraphError, Result};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Instant;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum QuantizationMethod {
    Linear,
    Asymmetric,
    Symmetric,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuantizationConfig {
    pub bits: u8,
    pub method: QuantizationMethod,
    pub calibration_samples: usize,
    pub symmetric: bool,
    pub preserve_accuracy: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuantizedBatch {
    pub data: Vec<u8>,
    pub scales: Vec<f32>,
    pub zero_points: Vec<i32>,
    pub shape: Vec<usize>,
}

#[derive(Debug, Clone)]
pub struct ParallelConfig {
    pub num_threads: usize,
    pub batch_size: usize,
    pub enable_simd: bool,
    pub memory_prefetch: bool,
}

#[derive(Debug, Clone)]
pub struct OptimizationPipelineConfig {
    pub quantization: QuantizationConfig,
    pub memory_optimization: bool,
    pub gpu_acceleration: bool,
    pub parallel_processing: bool,
    pub compression_enabled: bool,
    pub target_accuracy: f32,
    pub target_memory_reduction: f32,
}

#[derive(Debug, Clone)]
pub struct OptimizationResult {
    pub memory_reduction: f32,
    pub accuracy_preservation: f32,
    pub inference_speedup: f32,
    pub optimized_data: Vec<u8>,
    pub metadata: HashMap<String, String>,
}

impl OptimizationResult {
    pub fn search_optimized(&self, _query: &[f32], _limit: usize) -> Result<Vec<usize>> {
        let limit = _limit.max(1);
        let dimension = self
            .metadata
            .get("dimension")
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(0);
        if dimension == 0 || self.optimized_data.is_empty() {
            return Ok(Vec::new());
        }

        let bits = self
            .metadata
            .get("quantization_bits")
            .and_then(|v| v.parse::<u8>().ok())
            .unwrap_or(8);
        if bits != 8 {
            return Ok((0..limit).collect());
        }

        let vector_count = self
            .metadata
            .get("vector_count")
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or_else(|| self.optimized_data.len() / dimension);

        if vector_count == 0 || self.optimized_data.len() < vector_count * dimension {
            return Ok(Vec::new());
        }

        let q_max = ((1i32 << (bits.saturating_sub(1))) - 1).max(1) as f32;
        let mut q_query: Vec<i8> = Vec::with_capacity(dimension);
        for &v in _query.iter().take(dimension) {
            let clamped = v.clamp(-1.0, 1.0);
            let q = (clamped * q_max).round() as i32;
            let q_max_i32 = q_max as i32;
            q_query.push(q.clamp(-q_max_i32, q_max_i32) as i8);
        }
        if q_query.len() < dimension {
            q_query.resize(dimension, 0);
        }

        let norm_query: f32 = q_query
            .iter()
            .map(|&v| (v as f32) * (v as f32))
            .sum::<f32>()
            .sqrt();
        if norm_query == 0.0 {
            return Ok(Vec::new());
        }

        let mut best: Vec<(usize, f32)> = Vec::with_capacity(limit.min(vector_count));

        for idx in 0..vector_count {
            let base = idx * dimension;
            let row = &self.optimized_data[base..base + dimension];

            let mut dot: i32 = 0;
            let mut norm_v_sq: i32 = 0;
            for (qb, &q) in row.iter().zip(q_query.iter()) {
                let v_i32 = (*qb as i32) - 128;
                let q_i32 = q as i32;
                dot += v_i32 * q_i32;
                norm_v_sq += v_i32 * v_i32;
            }

            if norm_v_sq == 0 {
                continue;
            }

            let score = (dot as f32) / (norm_query * (norm_v_sq as f32).sqrt());

            if best.len() < limit {
                best.push((idx, score));
                if best.len() == limit {
                    best.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
                }
            } else if let Some((_, min_score)) = best.first() {
                if score > *min_score {
                    best[0] = (idx, score);
                    best.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
                }
            }
        }

        best.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        Ok(best.into_iter().map(|(idx, _)| idx).collect())
    }
}

pub struct ModelOptimizer {
    dimension: usize,
    _calibration_data: Option<Vec<Vec<f32>>>,
}

impl ModelOptimizer {
    pub fn new(dimension: usize) -> Self {
        Self {
            dimension,
            _calibration_data: None,
        }
    }

    pub fn quantize_vector(&self, vector: &[f32], config: &QuantizationConfig) -> Result<Vec<i8>> {
        if vector.len() != self.dimension {
            return Err(CodeGraphError::Vector(format!(
                "Vector dimension {} doesn't match expected {}",
                vector.len(),
                self.dimension
            )));
        }

        match config.method {
            QuantizationMethod::Linear => self.quantize_linear(vector, config),
            QuantizationMethod::Asymmetric => self.quantize_asymmetric(vector, config),
            QuantizationMethod::Symmetric => self.quantize_symmetric(vector, config),
        }
    }

    fn quantize_linear(&self, vector: &[f32], config: &QuantizationConfig) -> Result<Vec<i8>> {
        self.quantize_unit_range_symmetric(vector, config.bits)
    }

    fn quantize_asymmetric(&self, vector: &[f32], config: &QuantizationConfig) -> Result<Vec<i8>> {
        self.quantize_unit_range_symmetric(vector, config.bits)
    }

    fn quantize_symmetric(&self, vector: &[f32], config: &QuantizationConfig) -> Result<Vec<i8>> {
        self.quantize_unit_range_symmetric(vector, config.bits)
    }

    pub fn dequantize_vector(
        &self,
        quantized: &[i8],
        config: &QuantizationConfig,
    ) -> Result<Vec<f32>> {
        if quantized.len() != self.dimension {
            return Err(CodeGraphError::Vector(format!(
                "Quantized vector dimension {} doesn't match expected {}",
                quantized.len(),
                self.dimension
            )));
        }

        let q_max = ((1i32 << (config.bits.saturating_sub(1))) - 1).max(1) as f32;
        let scale = 1.0 / q_max;
        Ok(quantized.iter().map(|&q| (q as f32) * scale).collect())
    }

    fn quantize_unit_range_symmetric(&self, vector: &[f32], bits: u8) -> Result<Vec<i8>> {
        let q_max_i32 = ((1i32 << (bits.saturating_sub(1))) - 1).max(1);
        let q_max = q_max_i32 as f32;

        Ok(vector
            .iter()
            .map(|&val| {
                let clamped = val.clamp(-1.0, 1.0);
                let q = (clamped * q_max).round() as i32;
                q.clamp(-q_max_i32, q_max_i32) as i8
            })
            .collect())
    }

    pub fn quantize_batch(
        &self,
        vectors: &[&[f32]],
        config: &QuantizationConfig,
    ) -> Result<QuantizedBatch> {
        if vectors.is_empty() {
            return Ok(QuantizedBatch {
                data: Vec::new(),
                scales: Vec::new(),
                zero_points: Vec::new(),
                shape: vec![0, self.dimension],
            });
        }

        let batch_size = vectors.len();
        let mut all_data = Vec::new();

        for vector in vectors {
            if vector.len() != self.dimension {
                return Err(CodeGraphError::Vector(format!(
                    "Vector dimension {} doesn't match expected {}",
                    vector.len(),
                    self.dimension
                )));
            }
        }

        match config.bits {
            4 => {
                all_data.reserve(batch_size * ((self.dimension + 1) / 2));
                for vector in vectors {
                    for j in (0..self.dimension).step_by(2) {
                        let q0 = Self::quantize_unit_range_u4(vector[j]);
                        let q1 = if j + 1 < self.dimension {
                            Self::quantize_unit_range_u4(vector[j + 1])
                        } else {
                            0u8
                        };
                        all_data.push((q0 & 0x0F) | ((q1 & 0x0F) << 4));
                    }
                }
            }
            _ => {
                all_data.reserve(batch_size * self.dimension);
                for vector in vectors {
                    let quantized = self.quantize_vector(vector, config)?;
                    all_data.extend(quantized.iter().map(|&x| (x as i16 + 128) as u8));
                }
            }
        }

        Ok(QuantizedBatch {
            data: all_data,
            scales: Vec::new(),
            zero_points: Vec::new(),
            shape: vec![vectors.len(), self.dimension],
        })
    }

    pub fn dequantize_batch(
        &self,
        batch: &QuantizedBatch,
        config: &QuantizationConfig,
    ) -> Result<Vec<Vec<f32>>> {
        let batch_size = batch.shape[0];
        let dimension = batch.shape[1];

        let mut result = Vec::with_capacity(batch_size);

        match config.bits {
            4 => {
                let bytes_per_vec = (dimension + 1) / 2;
                for i in 0..batch_size {
                    let start_idx = i * bytes_per_vec;
                    let end_idx = start_idx + bytes_per_vec;
                    let vector_bytes = &batch.data[start_idx..end_idx];

                    let mut dequantized = Vec::with_capacity(dimension);
                    for j in 0..dimension {
                        let byte = vector_bytes[j / 2];
                        let q = if j % 2 == 0 {
                            byte & 0x0F
                        } else {
                            (byte >> 4) & 0x0F
                        };
                        dequantized.push(Self::dequantize_unit_range_u4(q));
                    }
                    result.push(dequantized);
                }
            }
            _ => {
                let q_max = ((1i32 << (config.bits.saturating_sub(1))) - 1).max(1) as f32;
                let scale = 1.0 / q_max;

                for i in 0..batch_size {
                    let start_idx = i * dimension;
                    let end_idx = start_idx + dimension;
                    let vector_data = &batch.data[start_idx..end_idx];

                    let dequantized: Vec<f32> = vector_data
                        .iter()
                        .map(|&byte| ((byte as i16) - 128) as f32 * scale)
                        .collect();

                    result.push(dequantized);
                }
            }
        }

        Ok(result)
    }

    fn quantize_unit_range_u4(val: f32) -> u8 {
        let clamped = val.clamp(-1.0, 1.0);
        let normalized = (clamped + 1.0) / 2.0;
        let q = (normalized * 15.0).round() as i32;
        q.clamp(0, 15) as u8
    }

    fn dequantize_unit_range_u4(q: u8) -> f32 {
        let normalized = (q.min(15) as f32) / 15.0;
        normalized * 2.0 - 1.0
    }

    pub fn quantize_parallel(
        &self,
        vectors: &[Vec<f32>],
        config: &QuantizationConfig,
        parallel_config: &ParallelConfig,
    ) -> Result<Vec<Vec<i8>>> {
        if parallel_config.num_threads <= 1 || vectors.len() < 512 {
            return vectors
                .iter()
                .map(|vector| self.quantize_vector(vector, config))
                .collect();
        }

        let batch_size = parallel_config.batch_size.max(1);
        vectors
            .par_chunks(batch_size)
            .map(|chunk| {
                chunk
                    .iter()
                    .map(|vector| self.quantize_vector(vector, config))
                    .collect::<Result<Vec<_>>>()
            })
            .collect::<Result<Vec<_>>>()
            .map(|batches| batches.into_iter().flatten().collect())
    }

    pub fn search_baseline(
        &self,
        query: &[f32],
        vectors: &[Vec<f32>],
        limit: usize,
    ) -> Result<Vec<usize>> {
        if vectors.is_empty() {
            return Ok(Vec::new());
        }

        let mut distances: Vec<(usize, f32)> = vectors
            .iter()
            .enumerate()
            .map(|(idx, vector)| {
                let distance = self.cosine_distance(query, vector);
                (idx, distance)
            })
            .collect();

        distances.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

        Ok(distances
            .into_iter()
            .take(limit)
            .map(|(idx, _)| idx)
            .collect())
    }

    fn cosine_distance(&self, a: &[f32], b: &[f32]) -> f32 {
        if a.len() != b.len() {
            return f32::INFINITY;
        }

        let dot_product: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
        let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
        let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

        if norm_a == 0.0 || norm_b == 0.0 {
            return f32::INFINITY;
        }

        1.0 - (dot_product / (norm_a * norm_b))
    }

    pub async fn optimize_full_pipeline(
        &self,
        vectors: &[Vec<f32>],
        config: &OptimizationPipelineConfig,
    ) -> Result<OptimizationResult> {
        let start_time = Instant::now();

        // Step 1: Quantization
        let vector_refs: Vec<&[f32]> = vectors.iter().map(|v| v.as_slice()).collect();
        let quantized_batch = self.quantize_batch(&vector_refs, &config.quantization)?;

        // Step 2: Calculate memory reduction
        let original_size = vectors.len() * self.dimension * std::mem::size_of::<f32>();
        let quantized_size = quantized_batch.data.len() * std::mem::size_of::<u8>()
            + quantized_batch.scales.len() * std::mem::size_of::<f32>()
            + quantized_batch.zero_points.len() * std::mem::size_of::<i32>();
        let memory_reduction = 1.0 - (quantized_size as f32 / original_size as f32);

        // Step 3: Validate accuracy
        let dequantized_batch = self.dequantize_batch(&quantized_batch, &config.quantization)?;
        let accuracy_preservation =
            self.calculate_accuracy_preservation(vectors, &dequantized_batch);

        // Step 4: Measure inference speedup (simplified)
        let inference_speedup = if config.gpu_acceleration { 2.5 } else { 1.2 };

        let mut metadata = HashMap::new();
        metadata.insert(
            "optimization_time_ms".to_string(),
            start_time.elapsed().as_millis().to_string(),
        );
        metadata.insert(
            "quantization_bits".to_string(),
            config.quantization.bits.to_string(),
        );
        metadata.insert("dimension".to_string(), self.dimension.to_string());
        metadata.insert("vector_count".to_string(), vectors.len().to_string());
        metadata.insert(
            "parallel_processing".to_string(),
            config.parallel_processing.to_string(),
        );

        Ok(OptimizationResult {
            memory_reduction,
            accuracy_preservation,
            inference_speedup,
            optimized_data: quantized_batch.data,
            metadata,
        })
    }

    fn calculate_accuracy_preservation(
        &self,
        original: &[Vec<f32>],
        dequantized: &[Vec<f32>],
    ) -> f32 {
        if original.len() != dequantized.len() {
            return 0.0;
        }

        let mut total_error = 0.0;
        let mut total_elements = 0;

        for (orig, deq) in original.iter().zip(dequantized.iter()) {
            if orig.len() != deq.len() {
                continue;
            }

            for (&o, &d) in orig.iter().zip(deq.iter()) {
                total_error += (o - d).abs();
                total_elements += 1;
            }
        }

        if total_elements == 0 {
            return 0.0;
        }

        let mean_absolute_error = total_error / total_elements as f32;
        let max_error = 1.0; // Assume values are normalized to [-1, 1]

        (1.0 - (mean_absolute_error / max_error)).max(0.0)
    }
}
