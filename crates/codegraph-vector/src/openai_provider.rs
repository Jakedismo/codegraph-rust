#[cfg(feature = "openai")]
use crate::{
    prep::chunker::{
        aggregate_chunk_embeddings, build_chunk_plan, ChunkPlan, ChunkerConfig, SanitizeMode,
    },
    providers::{
        BatchConfig, EmbeddingMetrics, EmbeddingProvider, MemoryUsage, ProviderCharacteristics,
    },
};
use async_trait::async_trait;
use codegraph_core::{CodeGraphError, CodeNode, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::{
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};
use tokenizers::Tokenizer;
use tokio::time::timeout;
use tracing::{debug, info, warn};

/// Configuration for OpenAI embedding provider
#[derive(Debug, Clone)]
pub struct OpenAiConfig {
    pub api_key: String,
    pub model: String,
    pub api_base: String,
    pub max_retries: usize,
    pub timeout: Duration,
    pub max_tokens_per_request: usize,
}

impl Default for OpenAiConfig {
    fn default() -> Self {
        Self {
            api_key: std::env::var("OPENAI_API_KEY").unwrap_or_default(),
            model: "text-embedding-3-small".to_string(),
            api_base: "https://api.openai.com/v1".to_string(),
            max_retries: 3,
            timeout: Duration::from_secs(30),
            max_tokens_per_request: 8000,
        }
    }
}

/// OpenAI API request structure for embeddings
#[derive(Debug, Serialize)]
struct EmbeddingRequest {
    input: Vec<String>,
    model: String,
    encoding_format: String,
}

/// OpenAI API response structure for embeddings
#[derive(Debug, Deserialize)]
struct EmbeddingResponse {
    data: Vec<EmbeddingData>,
    model: String,
    usage: Usage,
}

#[derive(Debug, Deserialize)]
struct EmbeddingData {
    embedding: Vec<f32>,
    index: usize,
}

#[derive(Debug, Deserialize)]
struct Usage {
    prompt_tokens: usize,
    total_tokens: usize,
}

/// Error response from OpenAI API
#[derive(Debug, Deserialize)]
struct ApiError {
    error: ErrorDetails,
}

#[derive(Debug, Deserialize)]
struct ErrorDetails {
    message: String,
    #[serde(rename = "type")]
    error_type: String,
    code: Option<String>,
}

/// OpenAI embedding provider implementation
#[cfg(feature = "openai")]
pub struct OpenAiEmbeddingProvider {
    config: OpenAiConfig,
    client: Client,
    tokenizer: Arc<Tokenizer>,
}

#[cfg(feature = "openai")]
impl OpenAiEmbeddingProvider {
    pub fn new(config: OpenAiConfig) -> Result<Self> {
        if config.api_key.is_empty() {
            return Err(CodeGraphError::Configuration(
                "OpenAI API key is required. Set OPENAI_API_KEY environment variable.".to_string(),
            ));
        }

        let client = Client::builder()
            .timeout(config.timeout)
            .user_agent("CodeGraph/1.0")
            .build()
            .map_err(|e| CodeGraphError::Network(e.to_string()))?;

        let tokenizer_path = PathBuf::from(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tokenizers/qwen2.5-coder.json"
        ));
        let tokenizer = Tokenizer::from_file(&tokenizer_path).map_err(|e| {
            CodeGraphError::Configuration(format!(
                "Failed to load tokenizer from {:?}: {}",
                tokenizer_path, e
            ))
        })?;

        Ok(Self {
            config,
            client,
            tokenizer: Arc::new(tokenizer),
        })
    }

    fn chunker_config(&self) -> ChunkerConfig {
        ChunkerConfig::new(self.config.max_tokens_per_request)
            .sanitize_mode(SanitizeMode::Strict)
            .cache_capacity(2048)
    }

    fn build_plan_for_nodes(&self, nodes: &[CodeNode]) -> ChunkPlan {
        build_chunk_plan(nodes, Arc::clone(&self.tokenizer), self.chunker_config())
    }

    /// Call OpenAI embeddings API with retry logic
    async fn call_api(&self, texts: Vec<String>) -> Result<EmbeddingResponse> {
        let request = EmbeddingRequest {
            input: texts,
            model: self.config.model.clone(),
            encoding_format: "float".to_string(),
        };

        let mut last_error = None;

        for attempt in 0..=self.config.max_retries {
            if attempt > 0 {
                // Exponential backoff
                let delay = Duration::from_millis(100 * 2_u64.pow(attempt as u32));
                tokio::time::sleep(delay).await;
            }

            let request_result = timeout(
                self.config.timeout,
                self.client
                    .post(&format!("{}/embeddings", self.config.api_base))
                    .header("Authorization", format!("Bearer {}", self.config.api_key))
                    .header("Content-Type", "application/json")
                    .json(&request)
                    .send(),
            )
            .await;

            match request_result {
                Ok(Ok(response)) => {
                    if response.status().is_success() {
                        match response.json::<EmbeddingResponse>().await {
                            Ok(embedding_response) => {
                                info!(
                                    "OpenAI API call successful: {} embeddings, {} tokens",
                                    embedding_response.data.len(),
                                    embedding_response.usage.total_tokens
                                );
                                return Ok(embedding_response);
                            }
                            Err(e) => {
                                last_error = Some(CodeGraphError::External(format!(
                                    "Failed to parse response: {}",
                                    e
                                )));
                            }
                        }
                    } else {
                        // Get status before consuming response
                        let status = response.status();

                        // Try to parse error response
                        if let Ok(api_error) = response.json::<ApiError>().await {
                            last_error = Some(CodeGraphError::External(format!(
                                "OpenAI API error: {} ({})",
                                api_error.error.message, api_error.error.error_type
                            )));
                        } else {
                            last_error = Some(CodeGraphError::External(format!(
                                "OpenAI API error: HTTP {}",
                                status
                            )));
                        }
                    }
                }
                Ok(Err(e)) => {
                    last_error = Some(CodeGraphError::Network(format!("Request failed: {}", e)));
                }
                Err(_) => {
                    last_error = Some(CodeGraphError::Timeout(
                        "OpenAI API request timed out".to_string(),
                    ));
                }
            }

            if attempt < self.config.max_retries {
                warn!(
                    "OpenAI API call failed (attempt {}/{}), retrying...",
                    attempt + 1,
                    self.config.max_retries + 1
                );
            }
        }

        Err(last_error.unwrap_or(CodeGraphError::External(
            "All OpenAI API retry attempts failed".to_string(),
        )))
    }

    /// Process nodes in optimal batches for OpenAI API
    async fn process_in_batches(
        &self,
        nodes: &[CodeNode],
        config: &BatchConfig,
    ) -> Result<(Vec<Vec<f32>>, EmbeddingMetrics)> {
        let start_time = Instant::now();
        let plan = self.build_plan_for_nodes(nodes);
        debug!(
            "OpenAI chunk planner: {} nodes -> {} chunks (avg {:.2} chunks/node)",
            plan.stats.total_nodes,
            plan.stats.total_chunks,
            plan.stats.total_chunks as f64 / plan.stats.total_nodes.max(1) as f64
        );
        let chunk_to_node = plan.chunk_to_node();
        let chunk_texts: Vec<String> = plan.chunks.into_iter().map(|c| c.text).collect();
        let chunk_size = config.batch_size.max(1);
        let mut chunk_embeddings = Vec::with_capacity(chunk_texts.len());

        for chunk in chunk_texts.chunks(chunk_size) {
            debug!("Processing OpenAI chunk of {} texts", chunk.len());
            let response = self.call_api(chunk.to_vec()).await?;

            let mut batch_embeddings: Vec<_> = response.data.into_iter().collect();
            batch_embeddings.sort_by_key(|item| item.index);
            chunk_embeddings.extend(batch_embeddings.into_iter().map(|item| item.embedding));
        }

        let node_embeddings = aggregate_chunk_embeddings(
            nodes.len(),
            &chunk_to_node,
            chunk_embeddings,
            self.embedding_dimension(),
        );

        let duration = start_time.elapsed();
        let metrics = EmbeddingMetrics::new("OpenAI".to_string(), nodes.len(), duration);

        info!(
            "OpenAI embedding generation completed: {} texts in {:?} ({:.2} texts/s)",
            metrics.texts_processed, metrics.duration, metrics.throughput
        );

        Ok((node_embeddings, metrics))
    }
}

#[cfg(feature = "openai")]
#[async_trait]
impl EmbeddingProvider for OpenAiEmbeddingProvider {
    async fn generate_embedding(&self, node: &CodeNode) -> Result<Vec<f32>> {
        let config = BatchConfig::default();
        let (mut embeddings, _) = self
            .generate_embeddings_with_config(std::slice::from_ref(node), &config)
            .await?;
        embeddings.pop().ok_or_else(|| {
            CodeGraphError::External("No embedding returned from OpenAI API".to_string())
        })
    }

    async fn generate_embeddings(&self, nodes: &[CodeNode]) -> Result<Vec<Vec<f32>>> {
        let config = BatchConfig::default();
        let (embeddings, _) = self.generate_embeddings_with_config(nodes, &config).await?;
        Ok(embeddings)
    }

    async fn generate_embeddings_with_config(
        &self,
        nodes: &[CodeNode],
        config: &BatchConfig,
    ) -> Result<(Vec<Vec<f32>>, EmbeddingMetrics)> {
        if nodes.is_empty() {
            return Ok((
                Vec::new(),
                EmbeddingMetrics::new("OpenAI".to_string(), 0, Duration::ZERO),
            ));
        }

        self.process_in_batches(nodes, config).await
    }

    fn embedding_dimension(&self) -> usize {
        match self.config.model.as_str() {
            "text-embedding-3-small" => 1536,
            "text-embedding-3-large" => 3072,
            "text-embedding-ada-002" => 1536,
            _ => 1536, // Default assumption
        }
    }

    fn provider_name(&self) -> &str {
        "OpenAI"
    }

    async fn is_available(&self) -> bool {
        // Simple health check - try to get models list
        let health_check = timeout(
            Duration::from_secs(5),
            self.client
                .get(&format!("{}/models", self.config.api_base))
                .header("Authorization", format!("Bearer {}", self.config.api_key))
                .send(),
        )
        .await;

        match health_check {
            Ok(Ok(response)) => response.status().is_success(),
            _ => false,
        }
    }

    fn performance_characteristics(&self) -> ProviderCharacteristics {
        ProviderCharacteristics {
            expected_throughput: 50.0, // Conservative estimate for OpenAI API
            typical_latency: Duration::from_millis(500),
            max_batch_size: 100, // OpenAI supports up to 2048 inputs per request
            supports_streaming: false,
            requires_network: true,
            memory_usage: MemoryUsage::Low,
        }
    }
}

// Provide empty implementations when openai feature is disabled
#[cfg(not(feature = "openai"))]
pub struct OpenAiEmbeddingProvider;

#[cfg(not(feature = "openai"))]
impl OpenAiEmbeddingProvider {
    pub fn new(_config: OpenAiConfig) -> Result<Self> {
        Err(CodeGraphError::Configuration(
            "OpenAI feature not enabled. Enable with --features openai".to_string(),
        ))
    }
}
