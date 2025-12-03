/// Ollama embedding provider for code-specialized embeddings
///
/// Uses nomic-embed-code-GGUF:Q4_K_M for superior code understanding
/// Complements Qwen2.5-Coder analysis with specialized code embeddings
use async_trait::async_trait;
use codegraph_core::{CodeGraphError, CodeNode, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokenizers::Tokenizer;
use tokio::time::timeout;
use tracing::{debug, info, trace, warn};

use crate::prep::chunker::{build_chunk_plan, ChunkPlan, ChunkerConfig, SanitizeMode};
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
    pub max_tokens_per_text: usize,
}

impl Default for OllamaEmbeddingConfig {
    fn default() -> Self {
        Self {
            model_name: "nomic-embed-code".to_string(),
            base_url: "http://localhost:11434".to_string(),
            timeout: Duration::from_secs(60),
            batch_size: 32,
            max_retries: 3,
            max_tokens_per_text: 512,
        }
    }
}

impl From<&codegraph_core::EmbeddingConfig> for OllamaEmbeddingConfig {
    fn from(config: &codegraph_core::EmbeddingConfig) -> Self {
        // Use model from config, fallback to env var, then to default
        let model_name = config
            .model
            .clone()
            .or_else(|| std::env::var("CODEGRAPH_EMBEDDING_MODEL").ok())
            .unwrap_or_else(|| "nomic-embed-code".to_string());

        // Use batch_size from config (already has env var fallback in config loading)
        let batch_size = config.batch_size.clamp(1, 256);

        let max_tokens_per_text = std::env::var("CODEGRAPH_MAX_CHUNK_TOKENS")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(512);

        Self {
            model_name,
            base_url: config.ollama_url.clone(),
            timeout: Duration::from_secs(60),
            batch_size,
            max_retries: 3,
            max_tokens_per_text,
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
    tokenizer: Arc<Tokenizer>,
}

impl OllamaEmbeddingProvider {
    pub fn max_batch_size(&self) -> usize {
        self.config.batch_size
    }

    pub fn new(config: OllamaEmbeddingConfig) -> Self {
        let characteristics = ProviderCharacteristics {
            expected_throughput: 100.0,                  // Expected texts per second
            typical_latency: Duration::from_millis(200), // Per text latency
            max_batch_size: config.batch_size,
            supports_streaming: false,
            requires_network: false,           // Local Ollama model
            memory_usage: MemoryUsage::Medium, // ~500MB-1GB for embedding model
        };

        // Load Qwen2.5-Coder tokenizer for accurate token counting
        let tokenizer_path = PathBuf::from(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tokenizers/qwen2.5-coder.json"
        ));
        let tokenizer = Tokenizer::from_file(&tokenizer_path).unwrap_or_else(|e| {
            warn!(
                "Failed to load Qwen2.5-Coder tokenizer from {:?}: {}. Using fallback character approximation.",
                tokenizer_path, e
            );
            // Create a minimal fallback tokenizer (shouldn't happen in practice)
            panic!("Tokenizer required for Ollama chunking");
        });

        Self {
            client: Client::new(),
            config,
            characteristics,
            tokenizer: Arc::new(tokenizer),
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

    fn chunker_config(&self) -> ChunkerConfig {
        let overlap_tokens = std::env::var("CODEGRAPH_CHUNK_OVERLAP_TOKENS")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(64);
        let smart_split = std::env::var("CODEGRAPH_CHUNK_SMART_SPLIT")
            .ok()
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(true);

        ChunkerConfig::new(self.config.max_tokens_per_text)
            .max_texts_per_request(self.config.batch_size)
            .cache_capacity(2048)
            .sanitize_mode(SanitizeMode::AsciiFastPath)
            .overlap_tokens(overlap_tokens)
            .smart_split(smart_split)
    }

    fn build_plan_for_nodes(&self, nodes: &[CodeNode]) -> ChunkPlan {
        build_chunk_plan(nodes, Arc::clone(&self.tokenizer), self.chunker_config())
    }

    fn prepare_text(&self, node: &CodeNode) -> Vec<String> {
        let formatted = Self::format_node_text(node);

        // Use tokenizer to accurately check if chunking is needed
        let token_count = self
            .tokenizer
            .encode(formatted.as_str(), false)
            .map(|enc| enc.len())
            .unwrap_or_else(|_| (formatted.len() + 3) / 4); // Fallback to char approximation

        if token_count <= self.config.max_tokens_per_text {
            // Fast path: Node is under token limit - no chunking needed (99% of nodes!)
            return vec![formatted];
        }

        // Slow path: Node exceeds token limit - use semantic chunking
        debug!(
            "Node '{}' has {} tokens (limit: {}), chunking required",
            node.name, token_count, self.config.max_tokens_per_text
        );

        let plan = self.build_plan_for_nodes(std::slice::from_ref(node));
        if plan.chunks.is_empty() {
            return vec![formatted];
        }

        let texts: Vec<String> = plan.chunks.into_iter().map(|chunk| chunk.text).collect();

        debug!(
            "Chunked large node '{}' into {} chunks (was {} tokens)",
            node.name,
            texts.len(),
            token_count
        );

        texts
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

    pub async fn generate_embeddings_for_texts(
        &self,
        texts: &[String],
        batch_size: usize,
    ) -> Result<Vec<Vec<f32>>> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }

        let mut all_embeddings = Vec::with_capacity(texts.len());

        for (batch_idx, batch) in texts.chunks(batch_size).enumerate() {
            trace!(
                "Sending Ollama embed batch {} ({} items)",
                batch_idx + 1,
                batch.len()
            );
            let batch_embeddings = self.call_embed_endpoint(batch).await?;
            all_embeddings.extend(batch_embeddings);
        }

        Ok(all_embeddings)
    }

    #[allow(dead_code)]
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

    /// Generate embeddings for multiple code nodes with batch optimization and chunking
    async fn generate_embeddings(&self, nodes: &[CodeNode]) -> Result<Vec<Vec<f32>>> {
        if nodes.is_empty() {
            return Ok(Vec::new());
        }

        debug!(
            "Generating {} embeddings with Ollama model {}",
            nodes.len(),
            self.config.model_name
        );
        let start_time = Instant::now();

        // Prepare texts from nodes with semantic chunking
        let node_chunks: Vec<(usize, Vec<String>)> = nodes
            .iter()
            .enumerate()
            .map(|(idx, node)| (idx, self.prepare_text(node)))
            .collect();

        // Flatten all chunks and track which node they belong to
        let mut all_texts = Vec::new();
        let mut chunk_to_node: Vec<usize> = Vec::new();

        for (node_idx, chunks) in &node_chunks {
            for chunk in chunks {
                all_texts.push(chunk.clone());
                chunk_to_node.push(*node_idx);
            }
        }

        debug!(
            "Processing {} nodes with {} total chunks (avg {:.2} chunks/node)",
            nodes.len(),
            all_texts.len(),
            all_texts.len() as f64 / nodes.len() as f64
        );

        // Generate embeddings for all chunks
        let chunk_embeddings = self
            .generate_embeddings_for_texts(&all_texts, self.config.batch_size)
            .await?;

        // Aggregate chunk embeddings back into node embeddings
        let dimension = self.embedding_dimension();
        let mut node_embeddings: Vec<Vec<f32>> = vec![vec![0.0f32; dimension]; nodes.len()];
        let mut node_chunk_counts = vec![0usize; nodes.len()];

        // Accumulate chunk embeddings for each node
        for (chunk_idx, chunk_embedding) in chunk_embeddings.into_iter().enumerate() {
            let node_idx = chunk_to_node[chunk_idx];
            if chunk_embedding.len() != dimension {
                warn!(
                    "⚠️ Ollama embedding dimension mismatch: expected {}, got {}",
                    dimension,
                    chunk_embedding.len()
                );
            }
            for (slot, value) in node_embeddings[node_idx]
                .iter_mut()
                .zip(chunk_embedding.iter())
            {
                *slot += *value;
            }
            node_chunk_counts[node_idx] += 1;
        }

        // Average the accumulated embeddings
        for (node_idx, count) in node_chunk_counts.iter().enumerate() {
            if *count > 0 {
                let divisor = *count as f32;
                for val in &mut node_embeddings[node_idx] {
                    *val /= divisor;
                }
            }
        }

        let total_time = start_time.elapsed();
        let embeddings_per_second = nodes.len() as f64 / total_time.as_secs_f64().max(0.001);

        info!(
            "Ollama embeddings complete: {} nodes ({} chunks) in {:.2}s ({:.1} emb/s)",
            nodes.len(),
            all_texts.len(),
            total_time.as_secs_f64(),
            embeddings_per_second
        );

        Ok(node_embeddings)
    }

    /// Generate embeddings with batch configuration and metrics
    async fn generate_embeddings_with_config(
        &self,
        nodes: &[CodeNode],
        _config: &BatchConfig,
    ) -> Result<(Vec<Vec<f32>>, EmbeddingMetrics)> {
        let start_time = Instant::now();

        // Use chunking-aware generate_embeddings instead of direct text formatting
        let embeddings = self.generate_embeddings(nodes).await?;

        let duration = start_time.elapsed();
        let metrics = EmbeddingMetrics::new(
            format!("ollama-{}", self.config.model_name),
            nodes.len(),
            duration,
        );

        Ok((embeddings, metrics))
    }

    /// Get the embedding dimension for this provider
    fn embedding_dimension(&self) -> usize {
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
    if let Some(dim) = std::env::var("CODEGRAPH_EMBEDDING_DIMENSION")
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
