use super::{EmbeddingBackend, EmbeddingError, EmbeddingOutput, EmbeddingProvider};
use crate::languages::{CodeInput, CodeLanguage, CodeProcessor};
use crate::cache::EmbeddingCache;
use crate::optimizer::EmbeddingOptimizer;

use candle_core::{Device, Tensor, Result as CandleResult};
use std::sync::Arc;
use tokio::sync::RwLock;

pub struct CodeEmbeddingModel {
    backend: Box<dyn EmbeddingBackend + Send + Sync>,
    processor: CodeProcessor,
    cache: Arc<RwLock<EmbeddingCache>>,
    optimizer: Option<EmbeddingOptimizer>,
    device: Device,
}

impl CodeEmbeddingModel {
    pub fn new(
        backend: Box<dyn EmbeddingBackend + Send + Sync>,
        processor: CodeProcessor,
        device: Device,
    ) -> Self {
        Self {
            backend,
            processor,
            cache: Arc::new(RwLock::new(EmbeddingCache::new(512 * 1024 * 1024))), // 512MB
            optimizer: None,
            device,
        }
    }

    pub fn with_optimizer(mut self, optimizer: EmbeddingOptimizer) -> Self {
        self.optimizer = Some(optimizer);
        self
    }

    pub async fn embed_code(&self, code: &str, language: CodeLanguage) -> Result<Vec<f32>, EmbeddingError> {
        // Check cache first
        let content_hash = self.compute_content_hash(code, language);
        
        {
            let cache = self.cache.read().await;
            if let Some(cached) = cache.get(&content_hash) {
                return Ok(cached.embeddings.clone());
            }
        }

        // Process code input
        let input = self.processor.process_code(code, language)?;
        
        // Generate embedding
        let output = self.backend.encode(&[input]).map_err(EmbeddingError::CandleError)?;
        let embeddings = self.tensor_to_vec(&output)?;
        
        // Apply optimization if configured
        let final_embeddings = if let Some(ref optimizer) = self.optimizer {
            optimizer.optimize_embeddings(embeddings)?
        } else {
            embeddings
        };

        // Cache result
        {
            let mut cache = self.cache.write().await;
            cache.insert(content_hash, final_embeddings.clone());
        }

        Ok(final_embeddings)
    }

    pub async fn embed_batch(&self, inputs: Vec<(String, CodeLanguage)>) -> Result<Vec<Vec<f32>>, EmbeddingError> {
        let mut results = Vec::with_capacity(inputs.len());
        let mut uncached_inputs = Vec::new();
        let mut uncached_indices = Vec::new();

        // Check cache for all inputs
        {
            let cache = self.cache.read().await;
            for (i, (code, lang)) in inputs.iter().enumerate() {
                let content_hash = self.compute_content_hash(code, *lang);
                if let Some(cached) = cache.get(&content_hash) {
                    results.push(Some(cached.embeddings.clone()));
                } else {
                    results.push(None);
                    uncached_inputs.push(self.processor.process_code(code, *lang)?);
                    uncached_indices.push(i);
                }
            }
        }

        // Process uncached inputs in batch
        if !uncached_inputs.is_empty() {
            let batch_output = self.backend.encode_batch(&uncached_inputs).map_err(EmbeddingError::CandleError)?;
            
            let mut cache = self.cache.write().await;
            for (idx, tensor) in batch_output.into_iter().enumerate() {
                let embeddings = self.tensor_to_vec(&tensor)?;
                let optimized = if let Some(ref optimizer) = self.optimizer {
                    optimizer.optimize_embeddings(embeddings)?
                } else {
                    embeddings
                };
                
                let result_idx = uncached_indices[idx];
                let (code, lang) = &inputs[result_idx];
                let content_hash = self.compute_content_hash(code, *lang);
                
                cache.insert(content_hash, optimized.clone());
                results[result_idx] = Some(optimized);
            }
        }

        Ok(results.into_iter().map(|r| r.unwrap()).collect())
    }

    pub async fn similarity(&self, code1: &str, lang1: CodeLanguage, code2: &str, lang2: CodeLanguage) -> Result<f32, EmbeddingError> {
        let emb1 = self.embed_code(code1, lang1).await?;
        let emb2 = self.embed_code(code2, lang2).await?;
        
        Ok(self.cosine_similarity(&emb1, &emb2))
    }

    pub fn get_embedding_dim(&self) -> usize {
        if let Some(ref optimizer) = self.optimizer {
            optimizer.get_target_dimensions()
        } else {
            self.backend.get_embedding_dim()
        }
    }

    pub fn supports_language(&self, lang: CodeLanguage) -> bool {
        self.backend.supports_language(lang)
    }

    pub async fn clear_cache(&self) {
        let mut cache = self.cache.write().await;
        cache.clear();
    }

    pub async fn cache_stats(&self) -> (usize, usize, f64) {
        let cache = self.cache.read().await;
        let hits = cache.hit_count();
        let total = cache.total_requests();
        let hit_rate = if total > 0 { hits as f64 / total as f64 } else { 0.0 };
        (hits, total, hit_rate)
    }

    fn compute_content_hash(&self, code: &str, language: CodeLanguage) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        
        let mut hasher = DefaultHasher::new();
        code.hash(&mut hasher);
        language.hash(&mut hasher);
        hasher.finish()
    }

    fn tensor_to_vec(&self, tensor: &Tensor) -> Result<Vec<f32>, EmbeddingError> {
        let shape = tensor.shape();
        if shape.dims().len() != 1 {
            return Err(EmbeddingError::InferenceError(
                "Expected 1D tensor for embeddings".to_string()
            ));
        }

        tensor.to_vec1::<f32>().map_err(|e| EmbeddingError::CandleError(e))
    }

    fn cosine_similarity(&self, a: &[f32], b: &[f32]) -> f32 {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::embedding::backend::MockBackend;
    use crate::languages::CodeProcessor;

    #[tokio::test]
    async fn test_embed_code() {
        let backend = Box::new(MockBackend::new());
        let processor = CodeProcessor::new();
        let device = Device::Cpu;
        
        let model = CodeEmbeddingModel::new(backend, processor, device);
        
        let code = "fn main() { println!(\"Hello, world!\"); }";
        let result = model.embed_code(code, CodeLanguage::Rust).await;
        
        assert!(result.is_ok());
        let embeddings = result.unwrap();
        assert!(!embeddings.is_empty());
    }

    #[tokio::test]
    async fn test_cache_functionality() {
        let backend = Box::new(MockBackend::new());
        let processor = CodeProcessor::new();
        let device = Device::Cpu;
        
        let model = CodeEmbeddingModel::new(backend, processor, device);
        
        let code = "fn test() { return 42; }";
        
        // First call should hit the backend
        let _result1 = model.embed_code(code, CodeLanguage::Rust).await.unwrap();
        
        // Second call should use cache
        let _result2 = model.embed_code(code, CodeLanguage::Rust).await.unwrap();
        
        let (hits, total, hit_rate) = model.cache_stats().await;
        assert!(hit_rate > 0.0);
    }

    #[tokio::test]
    async fn test_similarity_calculation() {
        let backend = Box::new(MockBackend::new());
        let processor = CodeProcessor::new();
        let device = Device::Cpu;
        
        let model = CodeEmbeddingModel::new(backend, processor, device);
        
        let code1 = "fn add(a: i32, b: i32) -> i32 { a + b }";
        let code2 = "fn subtract(a: i32, b: i32) -> i32 { a - b }";
        
        let similarity = model.similarity(code1, CodeLanguage::Rust, code2, CodeLanguage::Rust).await;
        
        assert!(similarity.is_ok());
        let score = similarity.unwrap();
        assert!(score >= 0.0 && score <= 1.0);
    }
}