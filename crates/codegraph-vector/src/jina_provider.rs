#[cfg(feature = "jina")]
use crate::{
    prep::chunker::{build_chunk_plan, ChunkPlan, ChunkerConfig, SanitizeMode},
    providers::{
        BatchConfig, EmbeddingMetrics, EmbeddingProvider, MemoryUsage, ProviderCharacteristics,
    },
};
use async_trait::async_trait;
use codegraph_core::{CodeGraphError, CodeNode, Language, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{Mutex as TokioMutex, Semaphore};
use tokio::time::timeout;
use tracing::{debug, error, info, warn};

pub const MAX_NODE_TEXTS_HARD_LIMIT: usize = 96;
pub const MAX_REL_TEXTS_HARD_LIMIT: usize = 32;

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
    pub truncate: bool,
    pub enable_reranking: bool,
    pub reranking_model: String,
    pub reranking_top_n: usize,
    pub batch_size: usize,
    pub max_concurrent: usize,
    pub max_tokens_per_text: usize,
    pub max_texts_per_request: usize,
    pub request_delay_ms: u64,
    pub relationship_batch_size: usize,
    pub relationship_max_texts_per_request: usize,
}

impl Default for JinaConfig {
    fn default() -> Self {
        Self {
            api_key: std::env::var("JINA_API_KEY").unwrap_or_default(),
            // Support both CODEGRAPH_EMBEDDING_MODEL and JINA_EMBEDDINGS_MODEL
            model: std::env::var("CODEGRAPH_EMBEDDING_MODEL")
                .or_else(|_| std::env::var("JINA_EMBEDDINGS_MODEL"))
                .unwrap_or_else(|_| "jina-embeddings-v4".to_string()),
            api_base: std::env::var("JINA_API_BASE")
                .unwrap_or_else(|_| "https://api.jina.ai/v1".to_string()),
            max_retries: 3,
            timeout: Duration::from_secs(30),
            task: std::env::var("JINA_API_TASK").unwrap_or_else(|_| "code.passage".to_string()),
            late_chunking: std::env::var("JINA_LATE_CHUNKING")
                .map(|v| v != "false")
                .unwrap_or(true),
            // Truncate: false by default (matches working curl), configurable via JINA_TRUNCATE=true
            truncate: std::env::var("JINA_TRUNCATE")
                .map(|v| v != "false")
                .unwrap_or(true),
            enable_reranking: std::env::var("JINA_ENABLE_RERANKING")
                .map(|v| v == "true")
                .unwrap_or(true),
            reranking_model: std::env::var("JINA_RERANKING_MODEL")
                .unwrap_or_else(|_| "jina-reranker-v3".to_string()),
            reranking_top_n: std::env::var("JINA_RERANKING_TOP_N")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(10),
            batch_size: 100,
            max_concurrent: 10,
            max_tokens_per_text: std::env::var("JINA_MAX_TOKENS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(2048),
            max_texts_per_request: std::env::var("JINA_MAX_TEXTS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(16)
                .clamp(1, MAX_NODE_TEXTS_HARD_LIMIT),
            request_delay_ms: std::env::var("JINA_REQUEST_DELAY_MS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(0),
            relationship_batch_size: std::env::var("JINA_REL_BATCH_SIZE")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(32)
                .max(1)
                .min(MAX_REL_TEXTS_HARD_LIMIT),
            relationship_max_texts_per_request: std::env::var("JINA_REL_MAX_TEXTS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(32)
                .clamp(1, MAX_REL_TEXTS_HARD_LIMIT),
        }
    }
}

impl From<&codegraph_core::EmbeddingConfig> for JinaConfig {
    fn from(config: &codegraph_core::EmbeddingConfig) -> Self {
        // Get API key from config or env var
        let api_key = config
            .jina_api_key
            .clone()
            .or_else(|| std::env::var("JINA_API_KEY").ok())
            .unwrap_or_default();

        // Get model from config, fallback to env var, then to default
        let model = config
            .model
            .clone()
            .or_else(|| std::env::var("CODEGRAPH_EMBEDDING_MODEL").ok())
            .or_else(|| std::env::var("JINA_EMBEDDINGS_MODEL").ok())
            .unwrap_or_else(|| "jina-embeddings-v4".to_string());

        Self {
            api_key,
            model,
            api_base: config.jina_api_base.clone(),
            max_retries: 3,
            timeout: Duration::from_secs(30),
            task: config.jina_task.clone(),
            late_chunking: config.jina_late_chunking,
            truncate: std::env::var("JINA_TRUNCATE")
                .map(|v| v != "false")
                .unwrap_or(true),
            enable_reranking: config.jina_enable_reranking,
            reranking_model: config.jina_reranking_model.clone(),
            reranking_top_n: config.jina_reranking_top_n,
            batch_size: config.batch_size.max(1),
            max_concurrent: 10,
            max_tokens_per_text: std::env::var("JINA_MAX_TOKENS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(2048),
            max_texts_per_request: std::env::var("JINA_MAX_TEXTS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(16)
                .clamp(1, MAX_NODE_TEXTS_HARD_LIMIT),
            request_delay_ms: std::env::var("JINA_REQUEST_DELAY_MS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(0),
            relationship_batch_size: std::env::var("JINA_REL_BATCH_SIZE")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(32)
                .max(1)
                .min(MAX_REL_TEXTS_HARD_LIMIT),
            relationship_max_texts_per_request: std::env::var("JINA_REL_MAX_TEXTS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(32)
                .clamp(1, MAX_REL_TEXTS_HARD_LIMIT),
        }
    }
}

/// Jina API request structure for embeddings
#[derive(Debug, Serialize)]
struct EmbeddingRequest {
    model: String,
    task: String,
    truncate: bool,
    late_chunking: bool,
    input: Vec<String>,
}

/// Jina API response structure for embeddings
#[derive(Debug, Deserialize)]
struct EmbeddingResponse {
    model: String,
    object: Option<String>,
    usage: Usage,
    data: Vec<EmbeddingData>,
}

#[derive(Debug, Deserialize)]
struct EmbeddingData {
    #[serde(default)]
    object: Option<String>,
    index: usize,
    embedding: Vec<f32>,
}

#[derive(Debug, Deserialize)]
struct Usage {
    total_tokens: usize,
    #[serde(default)]
    prompt_tokens: Option<usize>,
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
    model: String,
    object: Vec<String>,
    usage: RerankUsage,
    results: Vec<RerankResult>,
}

#[derive(Debug, Deserialize)]
struct RerankUsage {
    total_tokens: usize,
}

#[derive(Debug, Deserialize)]
pub struct RerankResult {
    pub index: usize,
    pub relevance_score: f32,
}

/// Error response from Jina API
#[derive(Debug, Deserialize)]
struct ApiError {
    detail: Option<String>,
    message: Option<String>,
}

/// Jina embedding provider implementation
#[cfg(feature = "jina")]
#[derive(Clone)]
pub struct JinaEmbeddingProvider {
    config: JinaConfig,
    client: Client,
    tokenizer: Arc<tokenizers::Tokenizer>,
    rate_limiter: Arc<JinaRateLimiter>,
}

#[cfg(feature = "jina")]
#[derive(Clone)]
struct ChunkMeta {
    file_path: String,
    node_name: String,
    language: Option<Language>,
    chunk_idx: usize,
}

struct JinaRateLimiter {
    rpm_limit: usize,
    tpm_limit: usize,
    max_tokens_hint: usize,
    window: TokioMutex<RateWindow>,
}

struct RateWindow {
    requests: VecDeque<Instant>,
    token_events: VecDeque<(Instant, usize)>,
    total_tokens: usize,
}

impl RateWindow {
    fn new() -> Self {
        Self {
            requests: VecDeque::new(),
            token_events: VecDeque::new(),
            total_tokens: 0,
        }
    }

    fn evict(&mut self) {
        let cutoff = Instant::now() - Duration::from_secs(60);
        while let Some(ts) = self.requests.front() {
            if *ts < cutoff {
                self.requests.pop_front();
            } else {
                break;
            }
        }
        while let Some((ts, tokens)) = self.token_events.front() {
            if *ts < cutoff {
                self.total_tokens = self.total_tokens.saturating_sub(*tokens);
                self.token_events.pop_front();
            } else {
                break;
            }
        }
    }

    fn wait_duration(&self, rpm_limit: usize, tpm_limit: usize, expect_tokens: usize) -> Duration {
        let now = Instant::now();
        let mut wait = Duration::ZERO;

        if self.requests.len() >= rpm_limit {
            if let Some(ts) = self.requests.front() {
                let candidate = ts
                    .checked_add(Duration::from_secs(60))
                    .unwrap_or(*ts)
                    .saturating_duration_since(now);
                wait = wait.max(candidate);
            }
        }

        if self.total_tokens + expect_tokens > tpm_limit {
            if let Some((ts, _)) = self.token_events.front() {
                let candidate = ts
                    .checked_add(Duration::from_secs(60))
                    .unwrap_or(*ts)
                    .saturating_duration_since(now);
                wait = wait.max(candidate);
            }
        }

        wait
    }
}

impl JinaRateLimiter {
    fn new(rpm_limit: usize, tpm_limit: usize, max_tokens_hint: usize) -> Self {
        Self {
            rpm_limit,
            tpm_limit,
            max_tokens_hint: max_tokens_hint.max(1),
            window: TokioMutex::new(RateWindow::new()),
        }
    }

    async fn acquire(&self, expected_tokens: usize) {
        let expect = expected_tokens.min(self.max_tokens_hint).max(1);
        loop {
            let mut guard = self.window.lock().await;
            guard.evict();
            if guard.requests.len() < self.rpm_limit
                && guard.total_tokens + expect <= self.tpm_limit
            {
                drop(guard);
                break;
            }
            let wait = guard.wait_duration(self.rpm_limit, self.tpm_limit, expect);
            drop(guard);
            tokio::time::sleep(wait).await;
        }
    }

    async fn record_request(&self, tokens_used: usize) {
        let mut guard = self.window.lock().await;
        guard.evict();
        let now = Instant::now();
        guard.requests.push_back(now);
        guard.token_events.push_back((now, tokens_used));
        guard.total_tokens = guard.total_tokens.saturating_add(tokens_used);
    }
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

        // Load Qwen2.5-Coder tokenizer for accurate token counting
        // Using bundled tokenizer file to avoid network dependency
        let tokenizer_path = std::path::PathBuf::from(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tokenizers/qwen2.5-coder.json"
        ));

        let tokenizer = tokenizers::Tokenizer::from_file(&tokenizer_path).map_err(|e| {
            CodeGraphError::Configuration(format!(
                "Failed to load Qwen2.5-Coder tokenizer from {:?}: {}. This is required for token counting.",
                tokenizer_path, e
            ))
        })?;

        let rate_limiter = Arc::new(JinaRateLimiter::new(
            500,
            1_000_000,
            config
                .max_tokens_per_text
                .saturating_mul(config.max_texts_per_request),
        ));

        Ok(Self {
            config,
            client,
            tokenizer: Arc::new(tokenizer),
            rate_limiter,
        }
        .log_runtime_config())
    }

    fn log_runtime_config(self) -> Self {
        info!(
            target: "codegraph_vector::jina_provider",
            "Jina provider config: max_tokens_per_text={}, max_texts_per_request={}, rel_max_texts={}, task={}, late_chunking={}, truncate={}",
            self.config.max_tokens_per_text,
            self.config.max_texts_per_request,
            self.config.relationship_max_texts_per_request,
            self.config.task,
            self.config.late_chunking,
            self.config.truncate
        );
        self
    }

    /// Update the batch size for embedding generation
    pub fn set_batch_size(&mut self, batch_size: usize) {
        let clamped = batch_size
            .max(1)
            .min(self.config.max_texts_per_request)
            .min(MAX_NODE_TEXTS_HARD_LIMIT);
        if batch_size != clamped {
            info!(
                target: "codegraph_vector::jina_provider",
                "Clamped requested Jina batch size {} to {}", batch_size, clamped
            );
        }
        self.config.batch_size = clamped;
    }

    /// Update the maximum concurrent requests for parallel processing
    pub fn set_max_concurrent(&mut self, max_concurrent: usize) {
        self.config.max_concurrent = max_concurrent;
    }

    /// Relationship embeddings need more conservative batching to avoid rate limits
    pub fn relationship_chunk_size(&self) -> usize {
        self.config
            .relationship_batch_size
            .min(self.config.relationship_max_texts_per_request)
            .min(self.config.max_texts_per_request)
            .min(MAX_REL_TEXTS_HARD_LIMIT)
            .max(1)
    }

    /// Get the embedding dimension for the current model
    pub fn embedding_dimension(&self) -> usize {
        match self.config.model.as_str() {
            "jina-code-embeddings-1.5b" => 1536,
            "jina-code-embeddings-0.5b" => 896,
            "jina-embeddings-v4" => 2048,
            "jina-embeddings-v3" => 1024,
            "jina-embeddings-v2-base-code" => 768,
            _ => 2048, // Default to v4 dimensions
        }
    }

    pub fn batch_size(&self) -> usize {
        self.config.batch_size
    }

    /// Count tokens in text using Qwen2.5-Coder tokenizer
    fn count_tokens(&self, text: &str) -> Result<usize> {
        let encoding = self
            .tokenizer
            .encode(text, false)
            .map_err(|e| CodeGraphError::External(format!("Failed to tokenize text: {}", e)))?;
        Ok(encoding.len())
    }

    /// Prepare text from CodeNode for embedding
    /// Jina code embeddings expect actual code, not formatted metadata
    /// Chunks code if it exceeds the configured token budget
    fn chunker_config(&self) -> ChunkerConfig {
        ChunkerConfig::new(self.config.max_tokens_per_text)
            .max_texts_per_request(self.config.max_texts_per_request)
            .cache_capacity(4096)
            .sanitize_mode(SanitizeMode::AsciiFastPath)
    }

    fn build_plan_for_nodes(&self, nodes: &[CodeNode]) -> ChunkPlan {
        build_chunk_plan(nodes, Arc::clone(&self.tokenizer), self.chunker_config())
    }

    fn prepare_text(&self, node: &CodeNode) -> Vec<String> {
        let plan = self.build_plan_for_nodes(std::slice::from_ref(node));
        if plan.chunks.is_empty() {
            return vec![node
                .content
                .as_deref()
                .unwrap_or_else(|| node.name.as_ref())
                .to_string()];
        }

        let mut texts = Vec::with_capacity(plan.chunks.len());
        let mut token_counts = Vec::with_capacity(plan.chunks.len());
        for chunk in plan.chunks {
            token_counts.push(chunk.tokens);
            texts.push(chunk.text);
        }

        if texts.len() == 1 {
            let token_count = *token_counts.first().unwrap_or(&0);
            debug!(
                "Text has {} tokens (<= {} limit) for node {}; single chunk",
                token_count, self.config.max_tokens_per_text, node.name
            );
        } else {
            let total_tokens: usize = token_counts.iter().sum();
            info!(
                "Chunked {} tokens into {} chunks for node {} (limit {})",
                total_tokens,
                texts.len(),
                node.name,
                self.config.max_tokens_per_text
            );
        }

        texts
    }

    /// Call Jina embeddings API with retry logic
    async fn call_embeddings_api(&self, texts: Vec<String>) -> Result<EmbeddingResponse> {
        // Debug logging: show all texts being sent with lengths
        if !texts.is_empty() {
            let approx_tokens: usize = texts
                .iter()
                .map(|text| {
                    self.count_tokens(text)
                        .unwrap_or_else(|_| std::cmp::max(1, text.len().saturating_div(4)))
                })
                .sum();

            info!(
                target: "codegraph_vector::jina_provider",
                "Jina embeddings request: {} texts (≈ {} tokens)",
                texts.len(),
                approx_tokens
            );

            debug!("Jina API request: {} texts", texts.len());
            for (i, text) in texts.iter().enumerate() {
                let sample = text.chars().take(100).collect::<String>();
                debug!(
                    "  Text {}: {} chars, sample: {:?}",
                    i + 1,
                    text.len(),
                    sample
                );
            }
        }

        let request = EmbeddingRequest {
            model: self.config.model.clone(),
            task: self.config.task.clone(),
            truncate: self.config.truncate,
            late_chunking: self.config.late_chunking,
            input: texts.clone(),
        };

        // Debug: log the COMPLETE JSON being sent
        if let Ok(json_str) = serde_json::to_string_pretty(&request) {
            debug!("=== FULL Jina API Request JSON ===\n{}", json_str);
        }

        let mut last_error = None;
        let expected_tokens = texts
            .len()
            .saturating_mul(self.config.max_tokens_per_text)
            .max(1);

        for attempt in 0..=self.config.max_retries {
            if attempt > 0 {
                // Exponential backoff
                let delay = Duration::from_millis(100 * 2_u64.pow(attempt as u32));
                tokio::time::sleep(delay).await;
            }

            if self.config.request_delay_ms > 0 {
                tokio::time::sleep(Duration::from_millis(self.config.request_delay_ms)).await;
            }

            if self.config.request_delay_ms > 0 {
                tokio::time::sleep(Duration::from_millis(self.config.request_delay_ms)).await;
            }

            let api_url = format!("{}/embeddings", self.config.api_base);
            debug!("Posting to: {}", api_url);

            self.rate_limiter.acquire(expected_tokens).await;

            let request_result = timeout(
                self.config.timeout,
                self.client
                    .post(&api_url)
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
                                self.rate_limiter
                                    .record_request(embedding_response.usage.total_tokens)
                                    .await;
                                return Ok(embedding_response);
                            }
                            Err(e) => {
                                self.rate_limiter.record_request(expected_tokens).await;
                                last_error = Some(CodeGraphError::External(format!(
                                    "Failed to parse response: {}",
                                    e
                                )));
                            }
                        }
                    } else {
                        self.rate_limiter.record_request(expected_tokens).await;
                        // Get status before consuming response
                        let status = response.status();

                        // Get raw response body for better error diagnostics
                        match response.text().await {
                            Ok(body) => {
                                // Try to parse as JSON error
                                if let Ok(api_error) = serde_json::from_str::<ApiError>(&body) {
                                    let error_msg = api_error
                                        .detail
                                        .or(api_error.message)
                                        .unwrap_or_else(|| "Unknown error".to_string());
                                    error!("Jina API error (HTTP {}): {}", status, error_msg);
                                    last_error = Some(CodeGraphError::External(format!(
                                        "Jina API error: {}",
                                        error_msg
                                    )));
                                } else {
                                    // Log raw body if we can't parse it
                                    error!(
                                        "Jina API error (HTTP {}): Raw response: {}",
                                        status,
                                        body.chars().take(500).collect::<String>()
                                    );
                                    last_error = Some(CodeGraphError::External(format!(
                                        "Jina API error (HTTP {}): {}",
                                        status,
                                        body.chars().take(200).collect::<String>()
                                    )));
                                }
                            }
                            Err(e) => {
                                error!("Failed to read Jina API error response: {}", e);
                                last_error = Some(CodeGraphError::External(format!(
                                    "Jina API error: HTTP {} (failed to read body: {})",
                                    status, e
                                )));
                            }
                        }
                    }
                }
                Ok(Err(e)) => {
                    self.rate_limiter.record_request(expected_tokens).await;
                    last_error = Some(CodeGraphError::Network(format!("Request failed: {}", e)));
                }
                Err(_) => {
                    self.rate_limiter.record_request(expected_tokens).await;
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

    fn log_failed_batch(&self, metas: &[ChunkMeta], texts: &[String], err: &CodeGraphError) {
        for (meta, text) in metas.iter().zip(texts.iter()).take(5) {
            let sample: String = text.chars().take(120).collect();
            let token_count = self
                .count_tokens(text)
                .unwrap_or_else(|_| sample.len().max(1) / 4);
            error!(
                target: "codegraph_vector::jina_provider",
                "Jina chunk failure (RID pending): file={} chunk={} lang={:?} tokens={} err={}",
                meta.file_path,
                meta.chunk_idx,
                meta.language,
                token_count,
                err
            );
            debug!(
                target: "codegraph_vector::jina_provider",
                "Chunk sample ({} chars) for {}: {:?}",
                text.len(),
                meta.node_name,
                sample
            );
        }
    }

    /// Generate embedding for a single text with custom task type (e.g., "code.query")
    pub async fn generate_text_embedding_with_task(
        &self,
        text: &str,
        task: &str,
    ) -> Result<Vec<f32>> {
        let request = EmbeddingRequest {
            model: self.config.model.clone(),
            task: task.to_string(),
            truncate: self.config.truncate,
            late_chunking: self.config.late_chunking,
            input: vec![text.to_string()],
        };

        let api_url = format!("{}/embeddings", self.config.api_base);

        let response = timeout(
            self.config.timeout,
            self.client
                .post(&api_url)
                .header("Authorization", format!("Bearer {}", self.config.api_key))
                .header("Content-Type", "application/json")
                .json(&request)
                .send(),
        )
        .await
        .map_err(|_| CodeGraphError::External("Jina API timeout".to_string()))?
        .map_err(|e| CodeGraphError::External(format!("Jina API request failed: {}", e)))?;

        if !response.status().is_success() {
            return Err(CodeGraphError::External(format!(
                "Jina API returned status: {}",
                response.status()
            )));
        }

        let embedding_response = response.json::<EmbeddingResponse>().await.map_err(|e| {
            CodeGraphError::External(format!("Failed to parse Jina response: {}", e))
        })?;

        if embedding_response.data.is_empty() {
            return Err(CodeGraphError::External(
                "Jina returned no embeddings".to_string(),
            ));
        }

        Ok(embedding_response.data[0].embedding.clone())
    }

    /// Batch embed free-form relationship texts with conservative limits
    pub async fn embed_relationship_texts(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }

        let chunk_size = self.relationship_chunk_size();
        let mut embeddings = Vec::with_capacity(texts.len());

        for chunk in texts.chunks(chunk_size) {
            let response = self.call_embeddings_api(chunk.to_vec()).await?;
            let mut batch_embeddings: Vec<_> = response.data.into_iter().collect();
            batch_embeddings.sort_by_key(|item| item.index);
            embeddings.extend(batch_embeddings.into_iter().map(|item| item.embedding));
        }

        Ok(embeddings)
    }

    /// Call Jina reranking API
    pub async fn rerank(&self, query: &str, documents: Vec<String>) -> Result<Vec<RerankResult>> {
        if !self.config.enable_reranking {
            return Ok(Vec::new());
        }

        let request = RerankRequest {
            model: self.config.reranking_model.clone(),
            query: query.to_string(),
            top_n: self.config.reranking_top_n,
            documents,
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

    /// Process nodes in optimal batches for Jina API with parallel execution
    async fn process_in_batches(
        &self,
        nodes: &[CodeNode],
        config: &BatchConfig,
    ) -> Result<(Vec<Vec<f32>>, EmbeddingMetrics)> {
        let start_time = Instant::now();

        // Convert nodes to texts, handling chunking
        // Keep track of (node_index, chunk_texts) to aggregate later
        let node_chunks: Vec<(usize, Vec<String>)> = nodes
            .iter()
            .enumerate()
            .map(|(idx, node)| (idx, self.prepare_text(node)))
            .collect();

        // Flatten all chunks into a single list while tracking which node they belong to
        let mut all_texts = Vec::new();
        let mut chunk_meta: Vec<ChunkMeta> = Vec::new();
        let mut chunk_to_node: Vec<usize> = Vec::new();

        for (node_idx, chunks) in &node_chunks {
            for (chunk_idx, chunk) in chunks.iter().enumerate() {
                let node_ref = &nodes[*node_idx];
                all_texts.push(chunk.clone());
                chunk_meta.push(ChunkMeta {
                    file_path: node_ref.location.file_path.clone(),
                    node_name: node_ref.name.to_string(),
                    language: node_ref.language.clone(),
                    chunk_idx,
                });
                chunk_to_node.push(*node_idx);
            }
        }

        debug!(
            "Processing {} nodes with {} total chunks (avg {:.2} chunks/node)",
            nodes.len(),
            all_texts.len(),
            all_texts.len() as f64 / nodes.len() as f64
        );

        let texts = all_texts;

        // Process in chunks to respect API limits and batch configuration
        let chunk_size = config
            .batch_size
            .min(self.config.max_texts_per_request)
            .max(1);

        // Create semaphore to limit concurrent requests
        let semaphore = Arc::new(Semaphore::new(self.config.max_concurrent));

        // Create tasks for parallel processing
        let mut tasks = Vec::new();

        for (batch_idx, chunk) in texts.chunks(chunk_size).enumerate() {
            let chunk_vec = chunk.to_vec();
            let meta_slice = chunk_meta[batch_idx * chunk_size
                ..(batch_idx * chunk_size + chunk_vec.len()).min(chunk_meta.len())]
                .to_vec();
            let semaphore = Arc::clone(&semaphore);
            let provider = self.clone();
            let task = tokio::spawn(async move {
                // Acquire semaphore permit to limit concurrency
                let _permit = semaphore.acquire().await.unwrap();

                debug!(
                    "Processing batch {} of {} texts",
                    batch_idx,
                    chunk_vec.len()
                );

                let response = provider.call_embeddings_api(chunk_vec.clone()).await;
                let response = match response {
                    Ok(resp) => resp,
                    Err(err) => {
                        provider.log_failed_batch(&meta_slice, &chunk_vec, &err);
                        return Err(err);
                    }
                };

                // Sort embeddings by index to maintain order within batch
                let mut batch_embeddings: Vec<_> = response.data.into_iter().collect();
                batch_embeddings.sort_by_key(|item| item.index);

                let embeddings: Vec<Vec<f32>> = batch_embeddings
                    .into_iter()
                    .map(|item| item.embedding)
                    .collect();

                Ok::<(usize, Vec<Vec<f32>>, usize, usize), CodeGraphError>((
                    batch_idx,
                    embeddings,
                    chunk_vec.len(),
                    response.usage.total_tokens,
                ))
            });

            tasks.push(task);
        }

        // Collect all results
        let mut batch_results = Vec::with_capacity(tasks.len());
        for task in tasks {
            let result = task
                .await
                .map_err(|e| CodeGraphError::External(format!("Task join error: {}", e)))??;
            batch_results.push(result);
        }

        // Sort by batch index to maintain order
        batch_results.sort_by_key(|(idx, _, _, _)| *idx);

        // Flatten chunk embeddings while maintaining order
        let mut chunk_embeddings = Vec::with_capacity(texts.len());
        let mut total_texts_processed = 0usize;
        let mut total_tokens_used = 0usize;
        let request_count = batch_results.len();
        for (_, embeddings, count, tokens) in batch_results.into_iter() {
            chunk_embeddings.extend(embeddings);
            total_texts_processed += count;
            total_tokens_used += tokens;
        }

        if request_count > 0 {
            info!(
                "Jina aggregated chunk: {} texts across {} calls (≈ {} tokens)",
                total_texts_processed, request_count, total_tokens_used
            );
        }

        // Aggregate chunk embeddings back into node embeddings
        let dimension = self.embedding_dimension();
        let mut node_embeddings: Vec<Vec<f32>> = vec![vec![0.0f32; dimension]; nodes.len()];
        let mut node_chunk_counts = vec![0usize; nodes.len()];

        // Accumulate chunk embeddings for each node
        for (chunk_idx, chunk_embedding) in chunk_embeddings.into_iter().enumerate() {
            let node_idx = chunk_to_node[chunk_idx];
            for (i, &val) in chunk_embedding.iter().enumerate() {
                node_embeddings[node_idx][i] += val;
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

        let duration = start_time.elapsed();
        let metrics = EmbeddingMetrics::new("Jina".to_string(), nodes.len(), duration);

        info!(
            "Jina parallel embedding generation completed: {} texts in {:?} ({:.2} texts/s, {} concurrent)",
            metrics.texts_processed, metrics.duration, metrics.throughput, self.config.max_concurrent
        );

        Ok((node_embeddings, metrics))
    }
}

#[cfg(feature = "jina")]
#[async_trait]
impl EmbeddingProvider for JinaEmbeddingProvider {
    async fn generate_embedding(&self, node: &CodeNode) -> Result<Vec<f32>> {
        let text_chunks = self.prepare_text(node);

        if text_chunks.len() == 1 {
            // Single chunk, no need to aggregate
            let response = self.call_embeddings_api(text_chunks).await?;
            if let Some(embedding_data) = response.data.into_iter().next() {
                Ok(embedding_data.embedding)
            } else {
                Err(CodeGraphError::External(
                    "No embedding returned from Jina API".to_string(),
                ))
            }
        } else {
            // Multiple chunks, need to aggregate embeddings
            debug!(
                "Node chunked into {} pieces, aggregating embeddings",
                text_chunks.len()
            );
            let response = self.call_embeddings_api(text_chunks).await?;

            if response.data.is_empty() {
                return Err(CodeGraphError::External(
                    "No embeddings returned from Jina API for chunked node".to_string(),
                ));
            }

            // Average the chunk embeddings to get final embedding
            let dimension = self.embedding_dimension();
            let mut averaged = vec![0.0f32; dimension];

            for embedding_data in &response.data {
                for (i, &val) in embedding_data.embedding.iter().enumerate() {
                    averaged[i] += val;
                }
            }

            let num_chunks = response.data.len() as f32;
            for val in &mut averaged {
                *val /= num_chunks;
            }

            Ok(averaged)
        }
    }

    async fn generate_embeddings(&self, nodes: &[CodeNode]) -> Result<Vec<Vec<f32>>> {
        let config = BatchConfig {
            batch_size: self.config.batch_size,
            ..BatchConfig::default()
        };
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
            "jina-code-embeddings-1.5b" => 1536,
            "jina-code-embeddings-0.5b" => 896,
            "jina-embeddings-v4" => 2048,
            "jina-embeddings-v3" => 1024,
            _ => 2048,
        }
    }

    fn provider_name(&self) -> &str {
        "Jina"
    }

    async fn is_available(&self) -> bool {
        // Simple health check - try to embed a small text
        let test_request = EmbeddingRequest {
            model: self.config.model.clone(),
            task: self.config.task.clone(),
            truncate: self.config.truncate,
            late_chunking: self.config.late_chunking,
            input: vec!["test".to_string()],
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
            max_batch_size: self.config.batch_size,
            supports_streaming: false,
            requires_network: true,
            memory_usage: MemoryUsage::Low,
        }
    }
}

#[cfg(all(test, feature = "jina"))]
mod tests {
    use super::*;
    use codegraph_core::{CodeNode, Language, Location};

    fn make_node_with_content(content: String) -> CodeNode {
        let location = Location {
            file_path: "test.rs".to_string(),
            line: 1,
            column: 1,
            end_line: None,
            end_column: None,
        };

        CodeNode::new("test", None, Some(Language::Rust), location).with_content(content)
    }

    fn build_provider() -> JinaEmbeddingProvider {
        let mut config = JinaConfig::default();
        config.api_key = "test-key".to_string();

        JinaEmbeddingProvider::new(config).expect("provider init")
    }

    #[test]
    fn long_single_line_is_chunked_under_token_limit() {
        let provider = build_provider();

        let mut content = String::new();
        for i in 0..10_000 {
            content.push_str(&format!(
                "let_variable_{i}_value_{i}_calculation_{i} = value_{i} + {};",
                i + 1
            ));
        }

        assert!(!content.contains('\n'));

        let node = make_node_with_content(content);
        let chunks = provider.prepare_text(&node);

        assert!(
            chunks.len() > 1,
            "expected long line to be chunked into multiple segments"
        );

        for chunk in chunks {
            let tokens = provider.count_tokens(&chunk).expect("token count");
            assert!(
                tokens <= provider.config.max_tokens_per_text.clamp(1000, 7500),
                "chunk exceeds token limit: {} tokens",
                tokens
            );
        }
    }

    #[test]
    fn semchunk_chunking_respects_token_limits_even_for_unicode() {
        let provider = build_provider();

        let text = "😀🚀".repeat(5000); // intentionally long unicode-only string
        let chunks = provider.chunk_with_semchunk(&text, 32);

        assert!(
            chunks.len() > 1,
            "expected semchunk to split very long unicode string"
        );

        for chunk in chunks {
            let tokens = provider.count_tokens(&chunk).expect("token count");
            assert!(
                tokens <= 32,
                "chunk exceeded token limit ({} tokens)",
                tokens
            );
        }
    }

    #[test]
    fn default_task_and_chunking_flags() {
        let mut config = JinaConfig::default();
        config.api_key = "test-key".to_string();
        let provider = JinaEmbeddingProvider::new(config).expect("provider init");
        assert_eq!(provider.config.task, "code.passage");
        assert!(provider.config.late_chunking);
        assert!(provider.config.truncate);
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
