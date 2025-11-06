#[cfg(feature = "jina")]
use crate::providers::{
    BatchConfig, EmbeddingMetrics, EmbeddingProvider, MemoryUsage, ProviderCharacteristics,
};
use async_trait::async_trait;
use codegraph_core::{CodeGraphError, CodeNode, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};
use tokio::time::timeout;
use tracing::{debug, error, info, warn};

/// Configuration for Jina embedding provider
#[derive(Debug, Clone)]
pub struct JinaConfig {
    pub api_key: String,
    pub model: String,
    pub api_base: String,
    pub max_retries: usize,
    pub timeout: Duration,
    pub task: String,
    pub late_chunking: bool,
    pub enable_reranking: bool,
    pub reranking_model: String,
    pub reranking_top_n: usize,
}

impl Default for JinaConfig {
    fn default() -> Self {
        Self {
            api_key: std::env::var("JINA_API_KEY").unwrap_or_default(),
            model: "jina-embeddings-v4".to_string(),
            api_base: "https://api.jina.ai/v1".to_string(),
            max_retries: 3,
            timeout: Duration::from_secs(30),
            task: "code.query".to_string(),
            late_chunking: true,
            enable_reranking: true,
            reranking_model: "jina-reranker-v3".to_string(),
            reranking_top_n: 10,
        }
    }
}

/// Jina API request structure for embeddings
#[derive(Debug, Serialize)]
struct EmbeddingRequest {
    input: Vec<TextInput>,
    model: String,
    task: String,
    late_chunking: bool,
}

/// Input can be text or image
#[derive(Debug, Serialize)]
struct TextInput {
    text: String,
}

/// Jina API response structure for embeddings
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
    total_tokens: usize,
}

/// Jina reranking request structure
#[derive(Debug, Serialize)]
struct RerankRequest {
    model: String,
    query: String,
    documents: Vec<String>,
    top_n: usize,
    return_documents: bool,
}

/// Jina reranking response structure
#[derive(Debug, Deserialize)]
struct RerankResponse {
    results: Vec<RerankResult>,
}

#[derive(Debug, Deserialize)]
struct RerankResult {
    index: usize,
    relevance_score: f32,
}

/// Error response from Jina API
#[derive(Debug, Deserialize)]
struct ApiError {
    detail: Option<String>,
    message: Option<String>,
}

/// Jina embedding provider implementation
#[cfg(feature = "jina")]
pub struct JinaEmbeddingProvider {
    config: JinaConfig,
    client: Client,
}

#[cfg(feature = "jina")]
impl JinaEmbeddingProvider {
    pub fn new(config: JinaConfig) -> Result<Self> {
        if config.api_key.is_empty() {
            return Err(CodeGraphError::Configuration(
                "Jina API key is required. Set JINA_API_KEY environment variable.".to_string(),
            ));
        }

        let client = Client::builder()
            .timeout(config.timeout)
            .user_agent("CodeGraph/1.0")
            .build()
            .map_err(|e| CodeGraphError::Network(e.to_string()))?;

        Ok(Self { config, client })
    }

    /// Prepare text from CodeNode for embedding
    fn prepare_text(&self, node: &CodeNode) -> String {
        let mut text = format!(
            "{} {} {}",
            node.language
                .as_ref()
                .map_or("unknown".to_string(), |l| format!("{:?}", l).to_lowercase()),
            node.node_type
                .as_ref()
                .map_or("unknown".to_string(), |t| format!("{:?}", t).to_lowercase()),
            node.name.as_str()
        );

        if let Some(content) = &node.content {
            text.push(' ');
            text.push_str(content);
        }

        // Truncate to prevent token limit issues
        if text.len() > 8000 * 4 {
            text.truncate(8000 * 4);
        }

        text
    }

    /// Call Jina embeddings API with retry logic
    async fn call_embeddings_api(&self, texts: Vec<String>) -> Result<EmbeddingResponse> {
        let input: Vec<TextInput> = texts.into_iter().map(|text| TextInput { text }).collect();

        let request = EmbeddingRequest {
            input,
            model: self.config.model.clone(),
            task: self.config.task.clone(),
            late_chunking: self.config.late_chunking,
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
                                    "Jina API call successful: {} embeddings, {} tokens",
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
                            let error_msg = api_error
                                .detail
                                .or(api_error.message)
                                .unwrap_or_else(|| "Unknown error".to_string());
                            last_error = Some(CodeGraphError::External(format!(
                                "Jina API error: {}",
                                error_msg
                            )));
                        } else {
                            last_error = Some(CodeGraphError::External(format!(
                                "Jina API error: HTTP {}",
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
                        "Jina API request timed out".to_string(),
                    ));
                }
            }

            if attempt < self.config.max_retries {
                warn!(
                    "Jina API call failed (attempt {}/{}), retrying...",
                    attempt + 1,
                    self.config.max_retries + 1
                );
            }
        }

        Err(last_error.unwrap_or(CodeGraphError::External(
            "All Jina API retry attempts failed".to_string(),
        )))
    }

    /// Call Jina reranking API
    pub async fn rerank(&self, query: &str, documents: Vec<String>) -> Result<Vec<RerankResult>> {
        if !self.config.enable_reranking {
            return Ok(Vec::new());
        }

        let request = RerankRequest {
            model: self.config.reranking_model.clone(),
            query: query.to_string(),
            documents,
            top_n: self.config.reranking_top_n,
            return_documents: false,
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
                    .post(&format!("{}/rerank", self.config.api_base))
                    .header("Authorization", format!("Bearer {}", self.config.api_key))
                    .header("Content-Type", "application/json")
                    .json(&request)
                    .send(),
            )
            .await;

            match request_result {
                Ok(Ok(response)) => {
                    if response.status().is_success() {
                        match response.json::<RerankResponse>().await {
                            Ok(rerank_response) => {
                                info!(
                                    "Jina rerank API call successful: {} results",
                                    rerank_response.results.len()
                                );
                                return Ok(rerank_response.results);
                            }
                            Err(e) => {
                                last_error = Some(CodeGraphError::External(format!(
                                    "Failed to parse rerank response: {}",
                                    e
                                )));
                            }
                        }
                    } else {
                        let status = response.status();
                        if let Ok(api_error) = response.json::<ApiError>().await {
                            let error_msg = api_error
                                .detail
                                .or(api_error.message)
                                .unwrap_or_else(|| "Unknown error".to_string());
                            last_error = Some(CodeGraphError::External(format!(
                                "Jina rerank API error: {}",
                                error_msg
                            )));
                        } else {
                            last_error = Some(CodeGraphError::External(format!(
                                "Jina rerank API error: HTTP {}",
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
                        "Jina rerank API request timed out".to_string(),
                    ));
                }
            }

            if attempt < self.config.max_retries {
                warn!(
                    "Jina rerank API call failed (attempt {}/{}), retrying...",
                    attempt + 1,
                    self.config.max_retries + 1
                );
            }
        }

        Err(last_error.unwrap_or(CodeGraphError::External(
            "All Jina rerank API retry attempts failed".to_string(),
        )))
    }

    /// Process nodes in optimal batches for Jina API
    async fn process_in_batches(
        &self,
        nodes: &[CodeNode],
        config: &BatchConfig,
    ) -> Result<(Vec<Vec<f32>>, EmbeddingMetrics)> {
        let start_time = Instant::now();
        let mut all_embeddings = Vec::with_capacity(nodes.len());

        // Convert nodes to texts
        let texts: Vec<String> = nodes.iter().map(|node| self.prepare_text(node)).collect();

        // Process in chunks to respect API limits and batch configuration
        let chunk_size = config.batch_size.min(100); // Jina supports batching

        for chunk in texts.chunks(chunk_size) {
            debug!("Processing batch of {} texts", chunk.len());

            let response = self.call_embeddings_api(chunk.to_vec()).await?;

            // Sort embeddings by index to maintain order
            let mut batch_embeddings: Vec<_> = response.data.into_iter().collect();
            batch_embeddings.sort_by_key(|item| item.index);

            for item in batch_embeddings {
                all_embeddings.push(item.embedding);
            }
        }

        let duration = start_time.elapsed();
        let metrics = EmbeddingMetrics::new("Jina".to_string(), nodes.len(), duration);

        info!(
            "Jina embedding generation completed: {} texts in {:?} ({:.2} texts/s)",
            metrics.texts_processed, metrics.duration, metrics.throughput
        );

        Ok((all_embeddings, metrics))
    }
}

#[cfg(feature = "jina")]
#[async_trait]
impl EmbeddingProvider for JinaEmbeddingProvider {
    async fn generate_embedding(&self, node: &CodeNode) -> Result<Vec<f32>> {
        let text = self.prepare_text(node);
        let response = self.call_embeddings_api(vec![text]).await?;

        if let Some(embedding_data) = response.data.into_iter().next() {
            Ok(embedding_data.embedding)
        } else {
            Err(CodeGraphError::External(
                "No embedding returned from Jina API".to_string(),
            ))
        }
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
                EmbeddingMetrics::new("Jina".to_string(), 0, Duration::ZERO),
            ));
        }

        self.process_in_batches(nodes, config).await
    }

    fn embedding_dimension(&self) -> usize {
        match self.config.model.as_str() {
            "jina-embeddings-v4" => 1024,
            "jina-embeddings-v3" => 1024,
            "jina-embeddings-v2-base-code" => 768,
            _ => 1024, // Default assumption
        }
    }

    fn provider_name(&self) -> &str {
        "Jina"
    }

    async fn is_available(&self) -> bool {
        // Simple health check - try to embed a small text
        let test_request = EmbeddingRequest {
            input: vec![TextInput {
                text: "test".to_string(),
            }],
            model: self.config.model.clone(),
            task: self.config.task.clone(),
            late_chunking: false,
        };

        let health_check = timeout(
            Duration::from_secs(5),
            self.client
                .post(&format!("{}/embeddings", self.config.api_base))
                .header("Authorization", format!("Bearer {}", self.config.api_key))
                .header("Content-Type", "application/json")
                .json(&test_request)
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
            expected_throughput: 40.0, // Conservative estimate for Jina API
            typical_latency: Duration::from_millis(600),
            max_batch_size: 100,
            supports_streaming: false,
            requires_network: true,
            memory_usage: MemoryUsage::Low,
        }
    }
}

// Provide empty implementations when jina feature is disabled
#[cfg(not(feature = "jina"))]
pub struct JinaEmbeddingProvider;

#[cfg(not(feature = "jina"))]
impl JinaEmbeddingProvider {
    pub fn new(_config: JinaConfig) -> Result<Self> {
        Err(CodeGraphError::Configuration(
            "Jina feature not enabled. Enable with --features jina".to_string(),
        ))
    }
}
