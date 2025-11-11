/// Ollama embedding provider for code-specialized embeddings
///
/// Uses nomic-embed-code-GGUF:Q4_K_M for superior code understanding
/// Complements Qwen2.5-Coder analysis with specialized code embeddings
use async_trait::async_trait;
use codegraph_core::{CodeGraphError, CodeNode, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};
use tokio::time::timeout;
use tracing::{debug, info, warn};

use crate::providers::{
    BatchConfig, EmbeddingMetrics, EmbeddingProvider, MemoryUsage, ProviderCharacteristics,
};

/// Configuration for Ollama embedding provider
#[derive(Debug, Clone)]
pub struct OllamaEmbeddingConfig {
    pub model_name: String,
    pub base_url: String,
    pub timeout: Duration,
    pub batch_size: usize,
    pub max_retries: usize,
}

impl Default for OllamaEmbeddingConfig {
    fn default() -> Self {
        let batch_size = std::env::var("CODEGRAPH_EMBEDDING_BATCH_SIZE")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .map(|value| value.clamp(1, 4096))
            .unwrap_or(32);

        Self {
            model_name: std::env::var("CODEGRAPH_EMBEDDING_MODEL")
                .unwrap_or_else(|_| "nomic-embed-code".to_string()),
            base_url: std::env::var("CODEGRAPH_OLLAMA_URL")
                .unwrap_or_else(|_| "http://localhost:11434".to_string()),
            timeout: Duration::from_secs(60),
            batch_size,
            max_retries: 3,
        }
    }
}

/// Ollama API request for embeddings
#[derive(Debug, Serialize)]
struct OllamaEmbeddingRequest<'a> {
    model: &'a str,
    input: &'a [String],
    #[serde(skip_serializing_if = "Option::is_none")]
    truncate: Option<bool>,
}

/// Ollama API response for embeddings
#[derive(Debug, Deserialize)]
struct OllamaEmbeddingResponse {
    embeddings: Vec<Vec<f32>>,
}

/// Ollama embedding provider using nomic-embed-code
pub struct OllamaEmbeddingProvider {
    client: Client,
    config: OllamaEmbeddingConfig,
    characteristics: ProviderCharacteristics,
}

impl OllamaEmbeddingProvider {
    pub fn new(config: OllamaEmbeddingConfig) -> Self {
        let characteristics = ProviderCharacteristics {
            expected_throughput: 100.0,                  // Expected texts per second
            typical_latency: Duration::from_millis(200), // Per text latency
            max_batch_size: config.batch_size,
            supports_streaming: false,
            requires_network: false,           // Local Ollama model
            memory_usage: MemoryUsage::Medium, // ~500MB-1GB for embedding model
        };

        Self {
            client: Client::new(),
            config,
            characteristics,
        }
    }

    /// Check if nomic-embed-code model is available
    pub async fn check_availability(&self) -> Result<bool> {
        debug!(
            "Checking {} availability at {}",
            self.config.model_name, self.config.base_url
        );

        let response = timeout(
            Duration::from_secs(5),
            self.client
                .get(&format!("{}/api/tags", self.config.base_url))
                .send(),
        )
        .await
        .map_err(|_| CodeGraphError::Timeout("Ollama availability check timeout".to_string()))?
        .map_err(|e| CodeGraphError::Network(format!("Ollama availability check failed: {}", e)))?;

        if !response.status().is_success() {
            return Ok(false);
        }

        let models: serde_json::Value = response
            .json()
            .await
            .map_err(|_| CodeGraphError::Parse("Failed to parse models response".to_string()))?;

        let desired = self.config.model_name.to_lowercase();
        let has_model = models["models"]
            .as_array()
            .map(|models| {
                models.iter().any(|model| {
                    model["name"]
                        .as_str()
                        .map(|name| {
                            let lower = name.to_lowercase();
                            lower == desired
                                || lower.contains(&desired)
                                || lower.contains("nomic-embed")
                        })
                        .unwrap_or(false)
                })
            })
            .unwrap_or(false);

        info!("{} availability: {}", self.config.model_name, has_model);
        Ok(has_model)
    }

    fn format_node_text(node: &CodeNode) -> String {
        let mut header = format!(
            "{} {} {}",
            node.language
                .as_ref()
                .map_or("unknown".to_string(), |l| format!("{:?}", l)),
            node.node_type
                .as_ref()
                .map_or("unknown".to_string(), |t| format!("{:?}", t)),
            node.name.as_str()
        );

        if let Some(content) = &node.content {
            header.push(' ');
            header.push_str(content);
        }

        header
    }

    async fn call_embed_endpoint(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }

        let request = OllamaEmbeddingRequest {
            model: &self.config.model_name,
            input: texts,
            truncate: Some(true),
        };

        let request_start = Instant::now();

        let response = timeout(
            self.config.timeout,
            self.client
                .post(format!(
                    "{}/api/embed",
                    self.config.base_url.trim_end_matches('/')
                ))
                .json(&request)
                .send(),
        )
        .await
        .map_err(|_| {
            CodeGraphError::Timeout(format!(
                "Ollama embedding timeout after {:?}",
                self.config.timeout
            ))
        })?
        .map_err(|e| CodeGraphError::Network(format!("Ollama embedding request failed: {}", e)))?;

        if !response.status().is_success() {
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(CodeGraphError::External(format!(
                "Ollama embedding API error: {}",
                error_text
            )));
        }

        let response_data: OllamaEmbeddingResponse = response.json().await.map_err(|e| {
            CodeGraphError::Parse(format!("Failed to parse Ollama embedding response: {}", e))
        })?;

        if response_data.embeddings.len() != texts.len() {
            return Err(CodeGraphError::Vector(format!(
                "Ollama returned {} embeddings for {} inputs",
                response_data.embeddings.len(),
                texts.len()
            )));
        }

        debug!(
            "Ollama embed batch: {} texts in {}ms",
            texts.len(),
            request_start.elapsed().as_millis()
        );

        Ok(response_data.embeddings)
    }

    async fn generate_embeddings_for_texts(
        &self,
        texts: &[String],
        batch_size: usize,
    ) -> Result<Vec<Vec<f32>>> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }

        let mut all_embeddings = Vec::with_capacity(texts.len());

        for (batch_idx, batch) in texts.chunks(batch_size).enumerate() {
            debug!(
                "Sending Ollama embed batch {} ({} items)",
                batch_idx + 1,
                batch.len()
            );
            let batch_embeddings = self.call_embed_endpoint(batch).await?;
            all_embeddings.extend(batch_embeddings);
        }

        Ok(all_embeddings)
    }

    fn effective_batch_size(&self, requested: usize) -> usize {
        let provider_limit = self.config.batch_size.max(1);
        requested.max(1).min(provider_limit)
    }

    /// Generate embedding for single text
    pub async fn generate_single_embedding(&self, text: &str) -> Result<Vec<f32>> {
        let payload = vec![text.to_string()];
        let mut embeddings = self
            .generate_embeddings_for_texts(&payload, self.config.batch_size)
            .await?;
        embeddings
            .pop()
            .ok_or_else(|| CodeGraphError::Vector("Ollama returned no embedding".to_string()))
    }
}

#[async_trait]
impl EmbeddingProvider for OllamaEmbeddingProvider {
    /// Generate embedding for a single code node
    async fn generate_embedding(&self, node: &CodeNode) -> Result<Vec<f32>> {
        let formatted = Self::format_node_text(node);
        self.generate_single_embedding(&formatted).await
    }

    /// Generate embeddings for multiple code nodes with batch optimization
    async fn generate_embeddings(&self, nodes: &[CodeNode]) -> Result<Vec<Vec<f32>>> {
        if nodes.is_empty() {
            return Ok(Vec::new());
        }

        info!(
            "Generating {} embeddings with Ollama model {}",
            nodes.len(),
            self.config.model_name
        );
        let start_time = Instant::now();

        // Prepare texts from nodes
        let texts: Vec<String> = nodes.iter().map(Self::format_node_text).collect();
        let embeddings = self
            .generate_embeddings_for_texts(&texts, self.config.batch_size)
            .await?;

        let total_time = start_time.elapsed();
        let embeddings_per_second = nodes.len() as f64 / total_time.as_secs_f64().max(0.001);

        info!(
            "Ollama embeddings complete: {} embeddings in {:.2}s ({:.1} emb/s)",
            nodes.len(),
            total_time.as_secs_f64(),
            embeddings_per_second
        );

        Ok(embeddings)
    }

    /// Generate embeddings with batch configuration and metrics
    async fn generate_embeddings_with_config(
        &self,
        nodes: &[CodeNode],
        config: &BatchConfig,
    ) -> Result<(Vec<Vec<f32>>, EmbeddingMetrics)> {
        let start_time = Instant::now();
        let texts: Vec<String> = nodes.iter().map(Self::format_node_text).collect();
        let batch_size = self.effective_batch_size(config.batch_size);
        let embeddings = self
            .generate_embeddings_for_texts(&texts, batch_size)
            .await?;
        let duration = start_time.elapsed();

        let metrics = EmbeddingMetrics::new(
            format!("ollama-{}", self.config.model_name),
            nodes.len(),
            duration,
        );

        Ok((embeddings, metrics))
    }

    /// Get the embedding dimension for this provider
    pub fn embedding_dimension(&self) -> usize {
        infer_dimension_for_model(&self.config.model_name)
    }

    /// Get provider name for identification
    fn provider_name(&self) -> &str {
        &self.config.model_name
    }

    /// Check if provider is available (model loaded in Ollama)
    async fn is_available(&self) -> bool {
        self.check_availability().await.unwrap_or(false)
    }

    /// Get provider-specific performance characteristics
    fn performance_characteristics(&self) -> ProviderCharacteristics {
        self.characteristics.clone()
    }
}

/// Create Ollama embedding provider with default config
pub fn create_ollama_provider() -> OllamaEmbeddingProvider {
    OllamaEmbeddingProvider::new(OllamaEmbeddingConfig::default())
}

/// Create Ollama embedding provider with custom model
pub fn create_ollama_provider_with_model(model_name: String) -> OllamaEmbeddingProvider {
    let mut config = OllamaEmbeddingConfig::default();
    config.model_name = model_name;
    OllamaEmbeddingProvider::new(config)
}

fn infer_dimension_for_model(model: &str) -> usize {
    if let Ok(dim) = std::env::var("CODEGRAPH_EMBEDDING_DIMENSION")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
    {
        return dim;
    }

    let normalized = model.to_lowercase();
    if normalized.contains("all-mini") {
        384
    } else if normalized.contains("0.6b") {
        1024
    } else if normalized.contains("4b") {
        2048
    } else if normalized.contains("8b") {
        4096
    } else if normalized.contains("2048") {
        2048
    } else if normalized.contains("1024") {
        1024
    } else {
        768
    }
}
