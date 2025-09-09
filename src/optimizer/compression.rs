use crate::embedding::EmbeddingError;
use candle_core::{Tensor, Device, Result as CandleResult, DType};
use candle_nn::ops;

#[derive(Debug, Clone)]
pub struct PCACompressor {
    components: Option<Tensor>,
    mean: Option<Tensor>,
    explained_variance: Option<Tensor>,
    n_components: usize,
    device: Device,
    is_fitted: bool,
}

#[derive(Debug, Clone)]
pub struct CompressionResult {
    pub explained_variance_ratio: Vec<f32>,
    pub cumulative_variance: f32,
    pub compression_ratio: f32,
    pub fitting_time_ms: u64,
}

impl PCACompressor {
    pub fn new(n_components: usize, device: Device) -> Self {
        Self {
            components: None,
            mean: None,
            explained_variance: None,
            n_components,
            device,
            is_fitted: false,
        }
    }

    pub fn fit(&mut self, data: &Tensor) -> Result<CompressionResult, EmbeddingError> {
        let start_time = std::time::Instant::now();
        
        let shape = data.shape();
        if shape.dims().len() != 2 {
            return Err(EmbeddingError::InferenceError(
                "PCA expects 2D input tensor (n_samples, n_features)".to_string()
            ));
        }

        let n_samples = shape.dims()[0];
        let n_features = shape.dims()[1];
        
        if self.n_components > n_features {
            return Err(EmbeddingError::InferenceError(
                format!("n_components ({}) cannot be larger than n_features ({})", 
                       self.n_components, n_features)
            ));
        }

        // Center the data
        let mean = data.mean_keepdim(0)?;
        let centered = data.sub(&mean)?;
        
        // Compute covariance matrix
        let covariance = self.compute_covariance(&centered)?;
        
        // Compute eigendecomposition
        let (eigenvalues, eigenvectors) = self.eigen_decomposition(&covariance)?;
        
        // Sort by eigenvalues (descending)
        let (sorted_eigenvalues, sorted_eigenvectors) = self.sort_eigen_pairs(eigenvalues, eigenvectors)?;
        
        // Take top n_components
        let components = sorted_eigenvectors.narrow(1, 0, self.n_components)?;
        let explained_variance = sorted_eigenvalues.narrow(0, 0, self.n_components)?;
        
        // Compute explained variance ratio
        let total_variance = sorted_eigenvalues.sum_all()?;
        let selected_variance = explained_variance.sum_all()?;
        let explained_variance_ratio = explained_variance.div(&total_variance.unsqueeze(0)?)?;
        
        self.components = Some(components);
        self.mean = Some(mean);
        self.explained_variance = Some(explained_variance);
        self.is_fitted = true;

        let fitting_time = start_time.elapsed().as_millis() as u64;
        
        Ok(CompressionResult {
            explained_variance_ratio: explained_variance_ratio.to_vec1::<f32>()
                .map_err(EmbeddingError::CandleError)?,
            cumulative_variance: (selected_variance.to_scalar::<f32>().unwrap() / 
                                total_variance.to_scalar::<f32>().unwrap()),
            compression_ratio: self.n_components as f32 / n_features as f32,
            fitting_time_ms: fitting_time,
        })
    }

    pub fn compress(&self, data: &Tensor) -> Result<Tensor, EmbeddingError> {
        if !self.is_fitted {
            return Err(EmbeddingError::InferenceError("PCA not fitted yet".to_string()));
        }

        let components = self.components.as_ref().unwrap();
        let mean = self.mean.as_ref().unwrap();
        
        // Center the data
        let centered = data.sub(mean)?;
        
        // Project onto principal components
        let compressed = centered.matmul(components)?;
        
        Ok(compressed)
    }

    pub fn decompress(&self, compressed_data: &Tensor) -> Result<Tensor, EmbeddingError> {
        if !self.is_fitted {
            return Err(EmbeddingError::InferenceError("PCA not fitted yet".to_string()));
        }

        let components = self.components.as_ref().unwrap();
        let mean = self.mean.as_ref().unwrap();
        
        // Reconstruct from compressed representation
        let reconstructed = compressed_data.matmul(&components.t()?)?;
        
        // Add back the mean
        let decompressed = reconstructed.add(mean)?;
        
        Ok(decompressed)
    }

    pub fn get_explained_variance_ratio(&self) -> Result<Vec<f32>, EmbeddingError> {
        if !self.is_fitted {
            return Err(EmbeddingError::InferenceError("PCA not fitted yet".to_string()));
        }

        let explained_variance = self.explained_variance.as_ref().unwrap();
        let total_variance = explained_variance.sum_all()?;
        let ratios = explained_variance.div(&total_variance.unsqueeze(0)?)?;
        
        Ok(ratios.to_vec1::<f32>().map_err(EmbeddingError::CandleError)?)
    }

    pub fn is_fitted(&self) -> bool {
        self.is_fitted
    }

    pub fn get_n_components(&self) -> usize {
        self.n_components
    }

    fn compute_covariance(&self, centered_data: &Tensor) -> CandleResult<Tensor> {
        let n_samples = centered_data.shape().dims()[0] as f32;
        let covariance = centered_data.t()?.matmul(centered_data)?.div(&Tensor::new(n_samples - 1.0, &self.device)?)?;
        Ok(covariance)
    }

    fn eigen_decomposition(&self, matrix: &Tensor) -> Result<(Tensor, Tensor), EmbeddingError> {
        // Simplified eigendecomposition - in a real implementation, you'd use LAPACK
        // For now, we'll use SVD as an approximation
        let (u, s, vt) = matrix.svd()?;
        
        // For symmetric matrices, eigenvalues are singular values squared
        let eigenvalues = s.pow(&Tensor::new(2.0f32, &self.device)?)?;
        let eigenvectors = vt.t()?; // Transpose to get column eigenvectors
        
        Ok((eigenvalues, eigenvectors))
    }

    fn sort_eigen_pairs(&self, eigenvalues: Tensor, eigenvectors: Tensor) -> Result<(Tensor, Tensor), EmbeddingError> {
        // Get sorting indices (descending order)
        let eigenvalues_vec = eigenvalues.to_vec1::<f32>().map_err(EmbeddingError::CandleError)?;
        let mut indices: Vec<usize> = (0..eigenvalues_vec.len()).collect();
        indices.sort_by(|&a, &b| eigenvalues_vec[b].partial_cmp(&eigenvalues_vec[a]).unwrap());

        // Sort eigenvalues
        let sorted_eigenvalues = Tensor::new(
            indices.iter().map(|&i| eigenvalues_vec[i]).collect::<Vec<_>>().as_slice(),
            &self.device
        ).map_err(EmbeddingError::CandleError)?;

        // Sort eigenvectors
        let eigenvectors_shape = eigenvectors.shape();
        let n_features = eigenvectors_shape.dims()[0];
        let mut sorted_eigenvectors_data = Vec::with_capacity(n_features * indices.len());
        
        let eigenvectors_vec = eigenvectors.to_vec2::<f32>().map_err(EmbeddingError::CandleError)?;
        
        for i in 0..n_features {
            for &col_idx in &indices {
                sorted_eigenvectors_data.push(eigenvectors_vec[i][col_idx]);
            }
        }

        let sorted_eigenvectors = Tensor::new(
            sorted_eigenvectors_data.as_slice(),
            &self.device
        ).map_err(EmbeddingError::CandleError)?
         .reshape((n_features, indices.len()))?;

        Ok((sorted_eigenvalues, sorted_eigenvectors))
    }
}

#[derive(Debug, Clone)]
pub struct AdaptiveCompressor {
    target_quality: f32,
    max_components: usize,
    min_components: usize,
    quality_threshold: f32,
    device: Device,
}

impl AdaptiveCompressor {
    pub fn new(target_quality: f32, max_components: usize, device: Device) -> Self {
        Self {
            target_quality,
            max_components,
            min_components: 16,
            quality_threshold: 0.02, // 2% quality loss threshold
            device,
        }
    }

    pub fn find_optimal_components(&self, data: &Tensor) -> Result<usize, EmbeddingError> {
        let mut best_components = self.min_components;
        let mut best_quality = 0.0f32;

        for n_components in (self.min_components..=self.max_components).step_by(16) {
            let mut compressor = PCACompressor::new(n_components, self.device.clone());
            let result = compressor.fit(data)?;
            
            if result.cumulative_variance >= self.target_quality {
                return Ok(n_components);
            }

            if result.cumulative_variance > best_quality {
                best_quality = result.cumulative_variance;
                best_components = n_components;
            }
        }

        Ok(best_components)
    }

    pub fn compress_with_quality_target(&self, data: &Tensor) -> Result<(Tensor, CompressionResult), EmbeddingError> {
        let optimal_components = self.find_optimal_components(data)?;
        let mut compressor = PCACompressor::new(optimal_components, self.device.clone());
        
        let result = compressor.fit(data)?;
        let compressed = compressor.compress(data)?;
        
        Ok((compressed, result))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pca_compressor_creation() {
        let device = Device::Cpu;
        let compressor = PCACompressor::new(64, device);
        
        assert_eq!(compressor.get_n_components(), 64);
        assert!(!compressor.is_fitted());
    }

    #[tokio::test]
    async fn test_pca_compression_pipeline() {
        let device = Device::Cpu;
        let mut compressor = PCACompressor::new(2, device.clone());
        
        // Create test data (4 samples, 3 features)
        let test_data = Tensor::new(
            &[
                1.0f32, 2.0, 3.0,
                4.0, 5.0, 6.0,
                7.0, 8.0, 9.0,
                10.0, 11.0, 12.0,
            ],
            &device
        ).unwrap().reshape((4, 3)).unwrap();

        // Fit the compressor
        let result = compressor.fit(&test_data);
        assert!(result.is_ok());
        assert!(compressor.is_fitted());

        // Compress data
        let compressed = compressor.compress(&test_data);
        assert!(compressed.is_ok());

        let compressed_tensor = compressed.unwrap();
        let compressed_shape = compressed_tensor.shape();
        assert_eq!(compressed_shape.dims(), &[4, 2]); // 4 samples, 2 components
    }

    #[test]
    fn test_adaptive_compressor() {
        let device = Device::Cpu;
        let adaptive = AdaptiveCompressor::new(0.9, 128, device);
        
        assert_eq!(adaptive.target_quality, 0.9);
        assert_eq!(adaptive.max_components, 128);
    }
}