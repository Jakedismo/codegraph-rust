// ABOUTME: LM Studio embedding provider implementation using OpenAI-compatible API
// ABOUTME: Provides local embedding generation without authentication requirements

use crate::{
    providers::{
        BatchConfig, EmbeddingMetrics, EmbeddingProvider, ProviderCharacteristics,
    },
};
use async_trait::async_trait;
use codegraph_core::{CodeGraphError, CodeNode, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};
use tokenizers::Tokenizer;
use tracing::{debug, info, warn};

/// Configuration for LM Studio embedding provider
#[derive(Debug, Clone)]
pub struct LmStudioEmbeddingConfig {
    /// Model name (e.g., "jinaai/jina-embeddings-v3", "nomic-ai/nomic-embed-text-v1.5")
    pub model: String,
    /// API base URL (default: "http://localhost:8000/v1")
    pub api_base: String,
    /// Request timeout duration
    pub timeout: Duration,
    /// Maximum number of retry attempts for failed requests
    pub max_retries: usize,
    /// Maximum tokens per text chunk
    pub max_tokens_per_request: usize,
}

impl Default for LmStudioEmbeddingConfig {
    fn default() -> Self {
        Self {
            model: "jinaai/jina-embeddings-v3".to_string(),
            api_base: "http://localhost:1234/v1".to_string(),
            timeout: Duration::from_secs(60),
            max_retries: 3,
            max_tokens_per_request: 8192,
        }
    }
}

impl LmStudioEmbeddingConfig {
    /// Create configuration from environment variables
    pub fn from_env() -> Self {
        Self {
            model: std::env::var("CODEGRAPH_LMSTUDIO_MODEL")
                .unwrap_or_else(|_| "jinaai/jina-embeddings-v3".to_string()),
            api_base: std::env::var("CODEGRAPH_LMSTUDIO_URL")
                .unwrap_or_else(|_| "http://localhost:8000/v1".to_string()),
            timeout: Duration::from_secs(
                std::env::var("CODEGRAPH_LMSTUDIO_TIMEOUT")
                    .ok()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(60),
            ),
            max_retries: std::env::var("CODEGRAPH_LMSTUDIO_MAX_RETRIES")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(3),
            max_tokens_per_request: std::env::var("CODEGRAPH_MAX_CHUNK_TOKENS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(8192),
        }
    }

    /// Merge with config file values (env vars take precedence)
    pub fn merge_with_config(
        mut self,
        model: Option<String>,
        api_base: Option<String>,
    ) -> Self {
        // Only use config values if env vars weren't set
        if std::env::var("CODEGRAPH_LMSTUDIO_MODEL").is_err() {
            if let Some(m) = model {
                self.model = m;
            }
        }
        if std::env::var("CODEGRAPH_LMSTUDIO_URL").is_err() {
            if let Some(url) = api_base {
                self.api_base = url;
            }
        }
        self
    }
}

/// LM Studio embedding provider
pub struct LmStudioEmbeddingProvider {
    config: LmStudioEmbeddingConfig,
    client: Client,
    tokenizer: Tokenizer,
    performance_chars: ProviderCharacteristics,
}

impl LmStudioEmbeddingProvider {
    /// Create new LM Studio embedding provider
    pub fn new(config: LmStudioEmbeddingConfig) -> Result<Self> {
        let client = Client::builder()
            .timeout(config.timeout)
            .build()
            .map_err(|e| CodeGraphError::Network(format!("Failed to create HTTP client: {}", e)))?;

        // Load tokenizer for token counting (same as OpenAI/Ollama)
        let tokenizer = Tokenizer::from_pretrained("Qwen/Qwen2.5-Coder-32B-Instruct", None)
            .expect("Failed to load Qwen2.5-Coder tokenizer for token counting");

        let performance_chars = ProviderCharacteristics {
            expected_throughput: 50.0,  // Local service, slower than Ollama (single model)
            typical_latency: Duration::from_millis(500),
            max_batch_size: 32,
            supports_streaming: false,
            requires_network: true,  // Local network
            memory_usage: crate::providers::MemoryUsage::High,  // Running full model
        };

        info!(
            "Initialized LM Studio embedding provider: model={}, api_base={}",
            config.model, config.api_base
        );

        Ok(Self {
            config,
            client,
            tokenizer,
            performance_chars,
        })
    }

    /// Check if LM Studio is available
    pub async fn check_availability(&self) -> bool {
        let url = format!("{}/models", self.config.api_base);
        match self.client.get(&url).send().await {
            Ok(response) => {
                if response.status().is_success() {
                    debug!("LM Studio is available at {}", self.config.api_base);
                    true
                } else {
                    warn!(
                        "LM Studio returned non-success status: {}",
                        response.status()
                    );
                    false
                }
            }
            Err(e) => {
                warn!("Failed to connect to LM Studio: {}", e);
                false
            }
        }
    }

    /// Prepare text by chunking if necessary
    fn prepare_text(&self, text: &str) -> Vec<String> {
        let encoding = self.tokenizer.encode(text, false).ok();
        let token_count = encoding.as_ref().map(|e| e.len()).unwrap_or(0);

        if token_count <= self.config.max_tokens_per_request {
            // Fast path: text fits in single chunk
            vec![text.to_string()]
        } else {
            // Semantic chunking for large text
            use semchunk_rs::Chunker;
            let tokenizer = self.tokenizer.clone();
            let chunker = Chunker::new(self.config.max_tokens_per_request, move |s: &str| {
                tokenizer
                    .encode(s, false)
                    .map(|enc| enc.len())
                    .unwrap_or_else(|_| (s.len() + 3) / 4) // Fallback to char approximation
            });
            chunker.chunk_text(text)
        }
    }

    /// Call LM Studio embeddings endpoint
    async fn call_embed_endpoint(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        let url = format!("{}/embeddings", self.config.api_base);

        let request = EmbeddingRequest {
            input: texts.to_vec(),
            model: self.config.model.clone(),
            encoding_format: "float".to_string(),
        };

        let mut last_error = None;

        for attempt in 0..self.config.max_retries {
            if attempt > 0 {
                let backoff = Duration::from_millis(100 * 2_u64.pow(attempt as u32));
                debug!("Retrying after {:?} (attempt {})", backoff, attempt + 1);
                tokio::time::sleep(backoff).await;
            }

            match self
                .client
                .post(&url)
                .json(&request)
                .send()
                .await
            {
                Ok(response) => {
                    if response.status().is_success() {
                        match response.json::<EmbeddingResponse>().await {
                            Ok(resp) => {
                                let embeddings: Vec<Vec<f32>> = resp
                                    .data
                                    .into_iter()
                                    .map(|item| item.embedding)
                                    .collect();
                                return Ok(embeddings);
                            }
                            Err(e) => {
                                last_error = Some(format!("Failed to parse response: {}", e));
                            }
                        }
                    } else {
                        let status = response.status();
                        let error_text = response.text().await.unwrap_or_default();
                        last_error = Some(format!(
                            "LM Studio API error ({}): {}",
                            status, error_text
                        ));
                    }
                }
                Err(e) => {
                    last_error = Some(format!("Request failed: {}", e));
                }
            }
        }

        Err(CodeGraphError::Network(
            last_error.unwrap_or_else(|| "LM Studio embedding request failed".to_string()),
        ))
    }

    /// Process texts in batches with chunking
    async fn process_in_batches(&self, texts: Vec<String>) -> Result<Vec<Vec<f32>>> {
        let mut all_embeddings = Vec::new();
        let batch_size = self.performance_chars.max_batch_size;

        for batch in texts.chunks(batch_size) {
            let embeddings = self.call_embed_endpoint(batch).await?;
            all_embeddings.extend(embeddings);
        }

        Ok(all_embeddings)
    }

    /// Infer embedding dimension from model name
    fn infer_dimension_for_model(model: &str) -> usize {
        let model_lower = model.to_lowercase();

        // Jina models
        if model_lower.contains("jina-embeddings-v4") {
            return 2048;
        }
        if model_lower.contains("jina-embeddings-v3") {
            return 1024;
        }
        if model_lower.contains("jina-code-embeddings-1.5b") {
            return 1536;
        }
        if model_lower.contains("jina-code-embeddings-0.5b") {
            return 896;
        }

        // Nomic models
        if model_lower.contains("nomic-embed-text-v1.5") || model_lower.contains("nomic-embed-text") {
            return 768;
        }

        // OpenAI models (if served via LM Studio)
        if model_lower.contains("text-embedding-3-large") {
            return 3072;
        }
        if model_lower.contains("text-embedding-3-small") || model_lower.contains("text-embedding-ada-002") {
            return 1536;
        }

        // BGE models
        if model_lower.contains("bge-large") {
            return 1024;
        }
        if model_lower.contains("bge-base") {
            return 768;
        }
        if model_lower.contains("bge-small") {
            return 384;
        }

        // E5 models
        if model_lower.contains("e5-large") {
            return 1024;
        }
        if model_lower.contains("e5-base") {
            return 768;
        }
        if model_lower.contains("e5-small") {
            return 384;
        }

        // Safe default
        warn!(
            "Unknown model '{}', using default dimension 1536",
            model
        );
        1536
    }
}

#[async_trait]
impl EmbeddingProvider for LmStudioEmbeddingProvider {
    async fn generate_embedding(&self, node: &CodeNode) -> Result<Vec<f32>> {
        let content = node.content.as_ref().ok_or_else(|| {
            CodeGraphError::Validation("CodeNode missing content for embedding".to_string())
        })?;

        let chunks = self.prepare_text(content);

        if chunks.len() == 1 {
            // Single chunk - direct embedding
            let embeddings = self.call_embed_endpoint(&chunks).await?;
            Ok(embeddings.into_iter().next().unwrap_or_default())
        } else {
            // Multiple chunks - average embeddings
            let embeddings = self.process_in_batches(chunks).await?;
            let dimension = embeddings.first().map(|e| e.len()).unwrap_or(0);

            if dimension == 0 {
                return Err(CodeGraphError::Vector(
                    "No embeddings generated".to_string(),
                ));
            }

            let mut averaged = vec![0.0; dimension];
            let count = embeddings.len() as f32;

            for embedding in &embeddings {
                for (i, &value) in embedding.iter().enumerate() {
                    averaged[i] += value / count;
                }
            }

            Ok(averaged)
        }
    }

    async fn generate_embeddings(&self, nodes: &[CodeNode]) -> Result<Vec<Vec<f32>>> {
        let start = Instant::now();
        let mut all_embeddings = Vec::with_capacity(nodes.len());

        for node in nodes {
            let embedding = self.generate_embedding(node).await?;
            all_embeddings.push(embedding);
        }

        let duration = start.elapsed();
        info!(
            "Generated {} embeddings in {:?} ({:.2} texts/sec)",
            nodes.len(),
            duration,
            nodes.len() as f64 / duration.as_secs_f64()
        );

        Ok(all_embeddings)
    }

    async fn generate_embeddings_with_config(
        &self,
        nodes: &[CodeNode],
        config: &BatchConfig,
    ) -> Result<(Vec<Vec<f32>>, EmbeddingMetrics)> {
        let start = Instant::now();

        let embeddings = if config.max_concurrent > 1 {
            // Concurrent processing (not typically needed for local service)
            self.generate_embeddings(nodes).await?
        } else {
            // Sequential processing
            self.generate_embeddings(nodes).await?
        };

        let duration = start.elapsed();
        let throughput = nodes.len() as f64 / duration.as_secs_f64();
        let average_latency = duration / nodes.len() as u32;

        let metrics = EmbeddingMetrics {
            texts_processed: nodes.len(),
            duration,
            throughput,
            average_latency,
            provider_name: self.provider_name().to_string(),
        };

        Ok((embeddings, metrics))
    }

    fn embedding_dimension(&self) -> usize {
        Self::infer_dimension_for_model(&self.config.model)
    }

    fn provider_name(&self) -> &str {
        "lmstudio"
    }

    async fn is_available(&self) -> bool {
        self.check_availability().await
    }

    fn performance_characteristics(&self) -> ProviderCharacteristics {
        self.performance_chars.clone()
    }
}

// API request/response types (OpenAI-compatible)

#[derive(Debug, Serialize)]
struct EmbeddingRequest {
    input: Vec<String>,
    model: String,
    encoding_format: String,
}

#[derive(Debug, Deserialize)]
struct EmbeddingResponse {
    data: Vec<EmbeddingData>,
}

#[derive(Debug, Deserialize)]
struct EmbeddingData {
    embedding: Vec<f32>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dimension_inference() {
        assert_eq!(
            LmStudioEmbeddingProvider::infer_dimension_for_model("jinaai/jina-embeddings-v3"),
            1024
        );
        assert_eq!(
            LmStudioEmbeddingProvider::infer_dimension_for_model("jinaai/jina-embeddings-v4"),
            2048
        );
        assert_eq!(
            LmStudioEmbeddingProvider::infer_dimension_for_model("nomic-ai/nomic-embed-text-v1.5"),
            768
        );
        assert_eq!(
            LmStudioEmbeddingProvider::infer_dimension_for_model("BAAI/bge-large-en-v1.5"),
            1024
        );
        assert_eq!(
            LmStudioEmbeddingProvider::infer_dimension_for_model("unknown-model"),
            1536  // Default
        );
    }

    #[test]
    fn test_config_from_env() {
        std::env::set_var("CODEGRAPH_LMSTUDIO_MODEL", "test-model");
        std::env::set_var("CODEGRAPH_LMSTUDIO_URL", "http://test:9000/v1");

        let config = LmStudioEmbeddingConfig::from_env();
        assert_eq!(config.model, "test-model");
        assert_eq!(config.api_base, "http://test:9000/v1");

        std::env::remove_var("CODEGRAPH_LMSTUDIO_MODEL");
        std::env::remove_var("CODEGRAPH_LMSTUDIO_URL");
    }
}
