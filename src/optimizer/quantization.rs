use crate::embedding::EmbeddingError;
use super::PrecisionMode;
use candle_core::{Tensor, Result as CandleResult};
use serde::{Serialize, Deserialize};

#[derive(Debug, Clone)]
pub struct Quantizer {
    config: QuantizationConfig,
    scale: Option<f32>,
    zero_point: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuantizationConfig {
    pub precision_mode: PrecisionMode,
    pub symmetric: bool,
    pub per_channel: bool,
    pub clamp_outliers: bool,
    pub outlier_threshold: f32,
}

#[derive(Debug, Clone)]
pub struct QuantizationParams {
    pub scale: f32,
    pub zero_point: i32,
    pub min_val: f32,
    pub max_val: f32,
}

impl Quantizer {
    pub fn new(config: QuantizationConfig) -> Self {
        Self {
            config,
            scale: None,
            zero_point: None,
        }
    }

    pub fn calibrate(&mut self, calibration_data: &Tensor) -> Result<QuantizationParams, EmbeddingError> {
        let params = match self.config.precision_mode {
            PrecisionMode::Quantized => {
                self.calibrate_int8(calibration_data)?
            }
            PrecisionMode::Half => {
                // No calibration needed for f16
                QuantizationParams {
                    scale: 1.0,
                    zero_point: 0,
                    min_val: f32::MIN,
                    max_val: f32::MAX,
                }
            }
            _ => {
                return Err(EmbeddingError::InferenceError(
                    "Quantization not supported for this precision mode".to_string()
                ));
            }
        };

        self.scale = Some(params.scale);
        self.zero_point = Some(params.zero_point);
        
        Ok(params)
    }

    pub fn quantize(&self, data: &Tensor) -> Result<Vec<f32>, EmbeddingError> {
        match self.config.precision_mode {
            PrecisionMode::Full => {
                // No quantization, return as-is
                data.to_vec1::<f32>().map_err(EmbeddingError::CandleError)
            }
            PrecisionMode::Half => {
                self.quantize_to_f16(data)
            }
            PrecisionMode::Quantized => {
                self.quantize_to_int8(data)
            }
            PrecisionMode::Dynamic => {
                self.dynamic_quantize(data)
            }
        }
    }

    pub fn dequantize(&self, quantized_data: &[f32]) -> Result<Vec<f32>, EmbeddingError> {
        match self.config.precision_mode {
            PrecisionMode::Full => {
                Ok(quantized_data.to_vec())
            }
            PrecisionMode::Half => {
                // Data is already in f32 representation, but was quantized from f16
                Ok(quantized_data.to_vec())
            }
            PrecisionMode::Quantized => {
                self.dequantize_from_int8(quantized_data)
            }
            PrecisionMode::Dynamic => {
                // Dynamic quantization is lossy and doesn't support exact dequantization
                Ok(quantized_data.to_vec())
            }
        }
    }

    fn calibrate_int8(&self, data: &Tensor) -> Result<QuantizationParams, EmbeddingError> {
        let data_vec = data.to_vec1::<f32>().map_err(EmbeddingError::CandleError)?;
        
        let (min_val, max_val) = if self.config.clamp_outliers {
            self.compute_clipped_range(&data_vec)
        } else {
            let min_val = data_vec.iter().fold(f32::INFINITY, |a, &b| a.min(b));
            let max_val = data_vec.iter().fold(f32::NEG_INFINITY, |a, &b| a.max(b));
            (min_val, max_val)
        };

        let (scale, zero_point) = if self.config.symmetric {
            // Symmetric quantization
            let max_abs = max_val.abs().max(min_val.abs());
            let scale = max_abs / 127.0; // Int8 range: -127 to 127
            (scale, 0)
        } else {
            // Asymmetric quantization
            let scale = (max_val - min_val) / 255.0; // Int8 range: -128 to 127 (256 levels)
            let zero_point = (-min_val / scale).round() as i32 - 128;
            (scale, zero_point)
        };

        Ok(QuantizationParams {
            scale,
            zero_point,
            min_val,
            max_val,
        })
    }

    fn quantize_to_f16(&self, data: &Tensor) -> Result<Vec<f32>, EmbeddingError> {
        let data_vec = data.to_vec1::<f32>().map_err(EmbeddingError::CandleError)?;
        
        // Simulate f16 precision by converting to f16 and back
        let quantized: Vec<f32> = data_vec
            .into_iter()
            .map(|x| {
                let f16_val = half::f16::from_f32(x);
                f16_val.to_f32()
            })
            .collect();

        Ok(quantized)
    }

    fn quantize_to_int8(&self, data: &Tensor) -> Result<Vec<f32>, EmbeddingError> {
        if self.scale.is_none() || self.zero_point.is_none() {
            return Err(EmbeddingError::InferenceError(
                "Quantizer not calibrated for int8 quantization".to_string()
            ));
        }

        let scale = self.scale.unwrap();
        let zero_point = self.zero_point.unwrap();
        
        let data_vec = data.to_vec1::<f32>().map_err(EmbeddingError::CandleError)?;
        
        let quantized: Vec<f32> = data_vec
            .into_iter()
            .map(|x| {
                let quantized_val = if self.config.symmetric {
                    (x / scale).round().clamp(-127.0, 127.0)
                } else {
                    ((x / scale) + zero_point as f32).round().clamp(-128.0, 127.0)
                };
                
                // Convert back to f32 for storage (in real implementation, would store as i8)
                if self.config.symmetric {
                    quantized_val * scale
                } else {
                    (quantized_val - zero_point as f32) * scale
                }
            })
            .collect();

        Ok(quantized)
    }

    fn dynamic_quantize(&self, data: &Tensor) -> Result<Vec<f32>, EmbeddingError> {
        let data_vec = data.to_vec1::<f32>().map_err(EmbeddingError::CandleError)?;
        
        // Analyze the data distribution to choose quantization strategy
        let (min_val, max_val) = self.compute_clipped_range(&data_vec);
        let range = max_val - min_val;
        
        // Choose quantization based on data characteristics
        if range < 2.0 {
            // Small range, use high precision
            self.quantize_to_f16(data)
        } else if range > 100.0 {
            // Large range, use int8 with outlier clipping
            let mut temp_config = self.config.clone();
            temp_config.clamp_outliers = true;
            let mut temp_quantizer = Quantizer::new(temp_config);
            temp_quantizer.calibrate(data)?;
            temp_quantizer.quantize_to_int8(data)
        } else {
            // Medium range, use symmetric int8
            let mut temp_config = self.config.clone();
            temp_config.symmetric = true;
            let mut temp_quantizer = Quantizer::new(temp_config);
            temp_quantizer.calibrate(data)?;
            temp_quantizer.quantize_to_int8(data)
        }
    }

    fn dequantize_from_int8(&self, quantized_data: &[f32]) -> Result<Vec<f32>, EmbeddingError> {
        if self.scale.is_none() || self.zero_point.is_none() {
            return Err(EmbeddingError::InferenceError(
                "Quantizer not calibrated for int8 dequantization".to_string()
            ));
        }

        // Since we stored quantized values as f32, they're already dequantized
        Ok(quantized_data.to_vec())
    }

    fn compute_clipped_range(&self, data: &[f32]) -> (f32, f32) {
        let mut sorted_data = data.to_vec();
        sorted_data.sort_by(|a, b| a.partial_cmp(b).unwrap());
        
        let outlier_fraction = self.config.outlier_threshold;
        let lower_idx = (data.len() as f32 * outlier_fraction / 2.0) as usize;
        let upper_idx = data.len() - 1 - lower_idx;
        
        let min_val = sorted_data[lower_idx];
        let max_val = sorted_data[upper_idx];
        
        (min_val, max_val)
    }

    pub fn estimate_compression_ratio(&self) -> f32 {
        match self.config.precision_mode {
            PrecisionMode::Full => 1.0,        // No compression
            PrecisionMode::Half => 0.5,        // f32 -> f16
            PrecisionMode::Quantized => 0.25,  // f32 -> i8
            PrecisionMode::Dynamic => 0.35,    // Variable compression
        }
    }

    pub fn get_config(&self) -> &QuantizationConfig {
        &self.config
    }
}

impl QuantizationConfig {
    pub fn from_precision_mode(mode: PrecisionMode) -> Self {
        match mode {
            PrecisionMode::Full => Self {
                precision_mode: mode,
                symmetric: false,
                per_channel: false,
                clamp_outliers: false,
                outlier_threshold: 0.01,
            },
            PrecisionMode::Half => Self {
                precision_mode: mode,
                symmetric: true,
                per_channel: false,
                clamp_outliers: false,
                outlier_threshold: 0.01,
            },
            PrecisionMode::Quantized => Self {
                precision_mode: mode,
                symmetric: true,
                per_channel: false,
                clamp_outliers: true,
                outlier_threshold: 0.02,
            },
            PrecisionMode::Dynamic => Self {
                precision_mode: mode,
                symmetric: false,
                per_channel: true,
                clamp_outliers: true,
                outlier_threshold: 0.01,
            },
        }
    }

    pub fn for_speed() -> Self {
        Self {
            precision_mode: PrecisionMode::Quantized,
            symmetric: true,
            per_channel: false,
            clamp_outliers: true,
            outlier_threshold: 0.05,
        }
    }

    pub fn for_accuracy() -> Self {
        Self {
            precision_mode: PrecisionMode::Half,
            symmetric: false,
            per_channel: true,
            clamp_outliers: false,
            outlier_threshold: 0.001,
        }
    }
}

#[derive(Debug)]
pub struct BatchQuantizer {
    quantizers: Vec<Quantizer>,
    batch_size: usize,
}

impl BatchQuantizer {
    pub fn new(configs: Vec<QuantizationConfig>, batch_size: usize) -> Self {
        let quantizers = configs.into_iter().map(Quantizer::new).collect();
        
        Self {
            quantizers,
            batch_size,
        }
    }

    pub fn quantize_batch(&mut self, data_batch: &[Tensor]) -> Result<Vec<Vec<f32>>, EmbeddingError> {
        if data_batch.len() != self.quantizers.len() {
            return Err(EmbeddingError::InferenceError(
                "Batch size mismatch between data and quantizers".to_string()
            ));
        }

        let mut results = Vec::with_capacity(data_batch.len());
        
        for (data, quantizer) in data_batch.iter().zip(self.quantizers.iter()) {
            let quantized = quantizer.quantize(data)?;
            results.push(quantized);
        }

        Ok(results)
    }

    pub fn calibrate_batch(&mut self, calibration_data: &[Tensor]) -> Result<Vec<QuantizationParams>, EmbeddingError> {
        let mut params = Vec::with_capacity(self.quantizers.len());
        
        for (data, quantizer) in calibration_data.iter().zip(self.quantizers.iter_mut()) {
            let param = quantizer.calibrate(data)?;
            params.push(param);
        }

        Ok(params)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use candle_core::Device;

    #[test]
    fn test_quantization_config_presets() {
        let speed_config = QuantizationConfig::for_speed();
        assert_eq!(speed_config.precision_mode, PrecisionMode::Quantized);
        assert!(speed_config.symmetric);

        let accuracy_config = QuantizationConfig::for_accuracy();
        assert_eq!(accuracy_config.precision_mode, PrecisionMode::Half);
        assert!(accuracy_config.per_channel);
    }

    #[test]
    fn test_quantizer_creation() {
        let config = QuantizationConfig::from_precision_mode(PrecisionMode::Quantized);
        let quantizer = Quantizer::new(config);
        
        assert_eq!(quantizer.get_config().precision_mode, PrecisionMode::Quantized);
        assert!(quantizer.scale.is_none()); // Not calibrated yet
    }

    #[tokio::test]
    async fn test_f16_quantization() {
        let device = Device::Cpu;
        let config = QuantizationConfig::from_precision_mode(PrecisionMode::Half);
        let quantizer = Quantizer::new(config);
        
        let test_data = Tensor::new(&[1.0f32, 2.5, 3.7, -1.2, 0.0], &device).unwrap();
        
        let quantized = quantizer.quantize(&test_data).unwrap();
        let dequantized = quantizer.dequantize(&quantized).unwrap();
        
        // Check that values are close (f16 has limited precision)
        for (original, recovered) in test_data.to_vec1::<f32>().unwrap().iter().zip(dequantized.iter()) {
            assert!((original - recovered).abs() < 0.01);
        }
    }

    #[tokio::test]
    async fn test_int8_quantization_pipeline() {
        let device = Device::Cpu;
        let config = QuantizationConfig::from_precision_mode(PrecisionMode::Quantized);
        let mut quantizer = Quantizer::new(config);
        
        let calibration_data = Tensor::new(&[-10.0f32, -5.0, 0.0, 5.0, 10.0, 15.0], &device).unwrap();
        
        // Calibrate
        let params = quantizer.calibrate(&calibration_data).unwrap();
        assert!(params.scale > 0.0);
        
        // Quantize
        let test_data = Tensor::new(&[2.5f32, -3.7, 8.1], &device).unwrap();
        let quantized = quantizer.quantize(&test_data).unwrap();
        
        assert_eq!(quantized.len(), 3);
        
        // Values should be within reasonable range after quantization
        for &val in &quantized {
            assert!(val.abs() <= 20.0); // Should be in reasonable range
        }
    }

    #[test]
    fn test_compression_ratio_estimation() {
        let configs = [
            QuantizationConfig::from_precision_mode(PrecisionMode::Full),
            QuantizationConfig::from_precision_mode(PrecisionMode::Half),
            QuantizationConfig::from_precision_mode(PrecisionMode::Quantized),
            QuantizationConfig::from_precision_mode(PrecisionMode::Dynamic),
        ];

        let expected_ratios = [1.0, 0.5, 0.25, 0.35];

        for (config, expected) in configs.iter().zip(expected_ratios.iter()) {
            let quantizer = Quantizer::new(config.clone());
            let ratio = quantizer.estimate_compression_ratio();
            assert_eq!(ratio, *expected);
        }
    }
}