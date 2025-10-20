/// Ollama embedding provider for code-specialized embeddings
///
/// Uses nomic-embed-code-GGUF:Q4_K_M for superior code understanding
/// Complements Qwen2.5-Coder analysis with specialized code embeddings
use async_trait::async_trait;
use codegraph_core::{CodeGraphError, CodeNode, Result};
use futures::future;
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
        Self {
            model_name: std::env::var("CODEGRAPH_EMBEDDING_MODEL")
                .unwrap_or_else(|_| "nomic-embed-code".to_string()),
            base_url: std::env::var("CODEGRAPH_OLLAMA_URL")
                .unwrap_or_else(|_| "http://localhost:11434".to_string()),
            timeout: Duration::from_secs(60),
            batch_size: 32, // Smaller batches for embedding model
            max_retries: 3,
        }
    }
}

/// Ollama API request for embeddings
#[derive(Debug, Serialize)]
struct OllamaEmbeddingRequest {
    model: String,
    prompt: String,
}

/// Ollama API response for embeddings
#[derive(Debug, Deserialize)]
struct OllamaEmbeddingResponse {
    embedding: Vec<f32>,
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
            "Checking nomic-embed-code availability at {}",
            self.config.base_url
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

        let has_model = models["models"]
            .as_array()
            .map(|models| {
                models.iter().any(|model| {
                    model["name"]
                        .as_str()
                        .map(|name| {
                            name.contains("nomic-embed")
                                || name.contains(&self.config.model_name)
                                || name == "nomic-embed-code"
                        })
                        .unwrap_or(false)
                })
            })
            .unwrap_or(false);

        info!("nomic-embed-code availability: {}", has_model);
        Ok(has_model)
    }

    /// Generate embedding for single text
    pub async fn generate_single_embedding(&self, text: &str) -> Result<Vec<f32>> {
        let start_time = Instant::now();

        let request = OllamaEmbeddingRequest {
            model: self.config.model_name.clone(),
            prompt: text.to_string(),
        };

        debug!("Generating embedding for {} chars", text.len());

        let response = timeout(
            self.config.timeout,
            self.client
                .post(&format!("{}/api/embeddings", self.config.base_url))
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

        let processing_time = start_time.elapsed();

        debug!(
            "Ollama embedding generated: {}ms, dimension: {}",
            processing_time.as_millis(),
            response_data.embedding.len()
        );

        Ok(response_data.embedding)
    }
}

#[async_trait]
impl EmbeddingProvider for OllamaEmbeddingProvider {
    /// Generate embedding for a single code node
    async fn generate_embedding(&self, node: &CodeNode) -> Result<Vec<f32>> {
        // Prepare text from code node (similar to other providers)
        let text = format!(
            "{} {} {}",
            node.language
                .as_ref()
                .map_or("unknown".to_string(), |l| format!("{:?}", l)),
            node.node_type
                .as_ref()
                .map_or("unknown".to_string(), |t| format!("{:?}", t)),
            node.name.as_str()
        );

        let full_text = if let Some(content) = &node.content {
            format!("{} {}", text, content)
        } else {
            text
        };

        self.generate_single_embedding(&full_text).await
    }

    /// Generate embeddings for multiple code nodes with batch optimization
    async fn generate_embeddings(&self, nodes: &[CodeNode]) -> Result<Vec<Vec<f32>>> {
        if nodes.is_empty() {
            return Ok(Vec::new());
        }

        info!(
            "Generating {} embeddings with nomic-embed-code",
            nodes.len()
        );
        let start_time = Instant::now();

        // Prepare texts from nodes
        let texts: Vec<String> = nodes
            .iter()
            .map(|node| {
                let text = format!(
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
                    format!("{} {}", text, content)
                } else {
                    text
                }
            })
            .collect();

        // Process in batches for optimal performance
        let mut all_embeddings = Vec::new();
        let batch_size = self.config.batch_size;

        for (batch_idx, batch) in texts.chunks(batch_size).enumerate() {
            debug!("Processing batch {} ({} texts)", batch_idx + 1, batch.len());

            // Process batch items in parallel but limited concurrency
            let batch_futures: Vec<_> = batch
                .iter()
                .map(|text| self.generate_single_embedding(text))
                .collect();

            // Wait for all embeddings in batch
            let batch_results = futures::future::try_join_all(batch_futures).await?;
            all_embeddings.extend(batch_results);

            // Small delay between batches to avoid overwhelming local model
            if batch_idx + 1 < texts.chunks(batch_size).len() {
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        }

        let total_time = start_time.elapsed();
        let embeddings_per_second = nodes.len() as f64 / total_time.as_secs_f64();

        info!(
            "Ollama embeddings complete: {} embeddings in {:.2}s ({:.1} emb/s)",
            nodes.len(),
            total_time.as_secs_f64(),
            embeddings_per_second
        );

        Ok(all_embeddings)
    }

    /// Generate embeddings with batch configuration and metrics
    async fn generate_embeddings_with_config(
        &self,
        nodes: &[CodeNode],
        _config: &BatchConfig,
    ) -> Result<(Vec<Vec<f32>>, EmbeddingMetrics)> {
        let start_time = Instant::now();
        let embeddings = self.generate_embeddings(nodes).await?;
        let duration = start_time.elapsed();

        let metrics =
            EmbeddingMetrics::new("ollama-nomic-embed-code".to_string(), nodes.len(), duration);

        Ok((embeddings, metrics))
    }

    /// Get the embedding dimension for this provider (nomic-embed-code = 768)
    fn embedding_dimension(&self) -> usize {
        768 // nomic-embed-code dimension
    }

    /// Get provider name for identification
    fn provider_name(&self) -> &str {
        "ollama-nomic-embed-code"
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
