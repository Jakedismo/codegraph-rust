// ABOUTME: Implements an embedding provider that calls a local Ollama server.
// ABOUTME: Handles batching, request sizing, retries, and result parsing for embeddings.
/// Ollama embedding provider for code-specialized embeddings
///
/// Uses nomic-embed-code-GGUF:Q4_K_M for superior code understanding
/// Complements Qwen2.5-Coder analysis with specialized code embeddings
use async_trait::async_trait;
use codegraph_core::{CodeGraphError, CodeNode, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokenizers::Tokenizer;
use tokio::time::timeout;
use tracing::{debug, info, trace, warn};

use crate::input_policy::{InputPolicy, env_positive, known_model};
use crate::prep::chunker::{ChunkPlan, ChunkerConfig, SanitizeMode, build_chunk_plan};
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
    pub num_ctx: Option<usize>,
}

impl Default for OllamaEmbeddingConfig {
    fn default() -> Self {
        Self {
            model_name: "nomic-embed-code".to_string(),
            base_url: "http://localhost:11434".to_string(),
            timeout: Duration::from_secs(300),
            batch_size: 32,
            max_retries: 3,
            max_tokens_per_text: usize::MAX,
            num_ctx: None,
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
        let batch_size = config.batch_size.max(1);

        let max_tokens_per_text = std::env::var("CODEGRAPH_CHUNK_MAX_TOKENS")
            .or_else(|_| std::env::var("CODEGRAPH_MAX_CHUNK_TOKENS"))
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(usize::MAX);

        let num_ctx = std::env::var("CODEGRAPH_OLLAMA_NUM_CTX")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .filter(|v| *v > 0);

        Self {
            model_name,
            base_url: config.ollama_url.clone(),
            timeout: Duration::from_secs(
                std::env::var("CODEGRAPH_OLLAMA_TIMEOUT_SECS")
                    .ok()
                    .and_then(|v| v.parse::<u64>().ok())
                    .unwrap_or(300),
            ),
            batch_size,
            max_retries: 3,
            max_tokens_per_text,
            num_ctx,
        }
    }
}

/// Ollama API options for embedding requests
#[derive(Debug, Serialize)]
struct OllamaOptions {
    /// Context window size (tokens)
    num_ctx: usize,
}

/// Ollama API request for embeddings
#[derive(Debug, Serialize)]
struct OllamaEmbeddingRequest<'a> {
    model: &'a str,
    input: &'a [String],
    #[serde(skip_serializing_if = "Option::is_none")]
    truncate: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    options: Option<OllamaOptions>,
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
    input_policy: tokio::sync::OnceCell<InputPolicy>,
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

        Self {
            client: Client::new(),
            config,
            characteristics,
            input_policy: tokio::sync::OnceCell::new(),
        }
    }

    /// Resolve once per provider lifecycle; no model weights are downloaded.
    pub async fn input_policy(&self) -> Result<&InputPolicy> {
        self.input_policy
            .get_or_try_init(|| self.resolve_input_policy())
            .await
    }

    async fn resolve_input_policy(&self) -> Result<InputPolicy> {
        env_positive("CODEGRAPH_OLLAMA_NUM_CTX")?;
        let base = self.config.base_url.trim_end_matches('/');
        let profile = known_model(&self.config.model_name);
        let response = self
            .client
            .post(format!("{base}/api/show"))
            .json(&serde_json::json!({"model": self.config.model_name}))
            .timeout(Duration::from_secs(15))
            .send()
            .await
            .map_err(|e| CodeGraphError::Network(format!("Ollama model metadata failed: {e}")))?;
        let metadata: serde_json::Value = if response.status().is_success() {
            response
                .json()
                .await
                .map_err(|e| CodeGraphError::Parse(e.to_string()))?
        } else {
            return Err(CodeGraphError::Vector(format!(
                "Cannot resolve Ollama model {}: /api/show returned {}",
                self.config.model_name,
                response.status()
            )));
        };
        if let Some(capabilities) = metadata["capabilities"].as_array()
            && !capabilities.iter().any(|value| value == "embedding")
        {
            return Err(CodeGraphError::Vector(format!(
                "Ollama model {} does not advertise embedding capability",
                self.config.model_name
            )));
        }
        let info = &metadata["model_info"];
        let architecture = info["general.architecture"].as_str();
        let advertised = architecture
            .and_then(|arch| info[format!("{arch}.context_length")].as_u64())
            .or_else(|| {
                info.as_object().and_then(|object| {
                    object
                        .iter()
                        .filter(|(key, _)| key.ends_with(".context_length"))
                        .filter_map(|(_, value)| value.as_u64())
                        .min()
                })
            })
            .and_then(|value| usize::try_from(value).ok())
            .filter(|value| *value > 0);
        let override_max = env_positive("CODEGRAPH_MODEL_MAX_TOKENS")?;
        let model_max = [advertised, profile.map(|p| p.1), override_max]
            .into_iter().flatten().min().ok_or_else(|| CodeGraphError::Vector(
                "Ollama metadata has no context limit; set CODEGRAPH_MODEL_MAX_TOKENS explicitly".into()))?;
        let parameter_ctx = metadata["parameters"]
            .as_str()
            .and_then(|text| {
                text.lines().find_map(|line| {
                    let mut fields = line.split_whitespace();
                    (fields.next() == Some("num_ctx"))
                        .then(|| fields.next()?.parse::<usize>().ok())
                        .flatten()
                })
            })
            .filter(|value| *value > 0);
        let mut running_ctx = None;
        if self.config.num_ctx.is_none()
            && let Ok(response) = self
                .client
                .get(format!("{base}/api/ps"))
                .timeout(Duration::from_secs(5))
                .send()
                .await
            && response.status().is_success()
            && let Ok(running) = response.json::<serde_json::Value>().await
        {
            running_ctx = running["models"]
                .as_array()
                .and_then(|models| {
                    models
                        .iter()
                        .find(|model| {
                            model["name"].as_str().is_some_and(|name| {
                                model_names_match(name, &self.config.model_name)
                            })
                        })
                        .and_then(|model| model["context_length"].as_u64())
                })
                .and_then(|value| usize::try_from(value).ok())
                .filter(|value| *value > 0);
        }
        let serving = self.config.num_ctx.unwrap_or_else(|| {
            [parameter_ctx, running_ctx]
                .into_iter()
                .flatten()
                .min()
                .unwrap_or(model_max)
        });
        let context_tokens = model_max.min(serving);
        let repository = std::env::var("CODEGRAPH_TOKENIZER_REPO")
            .ok()
            .or_else(|| profile.map(|p| p.0.to_owned()));
        let path = std::env::var_os("CODEGRAPH_TOKENIZER_PATH");
        let revision =
            std::env::var("CODEGRAPH_TOKENIZER_REVISION").unwrap_or_else(|_| "main".into());
        let tokenizer = tokio::task::spawn_blocking(move || -> Result<Tokenizer> {
            if let Some(path) = path {
                return Tokenizer::from_file(path).map_err(|e| CodeGraphError::Vector(format!("Invalid provider tokenizer: {e}")));
            }
            let repository = repository.ok_or_else(|| CodeGraphError::Vector(
                "Unknown Ollama embedding tokenizer. Set CODEGRAPH_TOKENIZER_PATH or CODEGRAPH_TOKENIZER_REPO; generic token counting is unsafe.".into()))?;
            tracing::info!("Loading embedding tokenizer {repository}@{revision} from the tokenizer cache (downloads tokenizer.json if absent)");
            Tokenizer::from_pretrained(&repository, Some(tokenizers::FromPretrainedParameters {
                revision, ..Default::default()
            })).map_err(|e| CodeGraphError::Vector(format!("Cannot load tokenizer {repository}: {e}. Set CODEGRAPH_TOKENIZER_PATH for offline use.")))
        }).await.map_err(|e| CodeGraphError::Vector(e.to_string()))??;
        let special_tokens = [
            "tokenizer.ggml.add_bos_token",
            "tokenizer.ggml.add_eos_token",
        ]
        .into_iter()
        .map(|key| usize::from(info[key].as_bool().unwrap_or(true)))
        .sum();
        let document_prefix = std::env::var("CODEGRAPH_EMBEDDING_DOCUMENT_PREFIX")
            .unwrap_or_else(|_| profile.map_or("", |p| p.2).into());
        let query_prefix = std::env::var("CODEGRAPH_EMBEDDING_QUERY_PREFIX")
            .unwrap_or_else(|_| profile.map_or("", |p| p.3).into());
        let tags = self
            .client
            .get(format!("{base}/api/tags"))
            .timeout(Duration::from_secs(5))
            .send()
            .await
            .map_err(|e| CodeGraphError::Network(format!("Ollama model identity failed: {e}")))?
            .error_for_status()
            .map_err(|e| CodeGraphError::Network(e.to_string()))?
            .json::<serde_json::Value>()
            .await
            .map_err(|e| CodeGraphError::Parse(e.to_string()))?;
        let digest = tags["models"]
            .as_array()
            .and_then(|models| {
                models.iter().find(|model| {
                    model["name"]
                        .as_str()
                        .is_some_and(|name| model_names_match(name, &self.config.model_name))
                })
            })
            .and_then(|model| model["digest"].as_str());
        let identity = codegraph_core::artifact_cache::fingerprint(&(
            &self.config.model_name,
            digest,
            &metadata,
            std::env::var("CODEGRAPH_MODEL_REVISION").unwrap_or_default(),
            std::env::var("CODEGRAPH_TOKENIZER_REPO").unwrap_or_default(),
            std::env::var("CODEGRAPH_TOKENIZER_REVISION").unwrap_or_default(),
        ))
        .map_err(|e| CodeGraphError::Vector(e.to_string()))?;
        let policy = InputPolicy::new(
            tokenizer,
            context_tokens,
            document_prefix,
            query_prefix,
            special_tokens,
            &identity,
        )?;
        info!(
            "Embedding input policy: model={} model_limit={} serving_context={} tokenizer={} document_prefix={:?} query_prefix={:?}; truncation disabled",
            self.config.model_name,
            model_max,
            context_tokens,
            policy.identity,
            policy.document_prefix,
            policy.query_prefix
        );
        Ok(policy)
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
                        .map(|name| model_names_match(name, &desired))
                        .unwrap_or(false)
                })
            })
            .unwrap_or(false);

        info!("{} availability: {}", self.config.model_name, has_model);
        Ok(has_model)
    }

    async fn build_plan_for_nodes(&self, nodes: &[CodeNode]) -> Result<ChunkPlan> {
        let policy = self.input_policy().await?.clone();
        let max_tokens = env_positive("CODEGRAPH_CHUNK_MAX_TOKENS")?
            .or(env_positive("CODEGRAPH_MAX_CHUNK_TOKENS")?)
            .unwrap_or(policy.context_tokens)
            .min(policy.context_tokens)
            .min(self.config.max_tokens_per_text.max(1));
        let mut config = ChunkerConfig::new(max_tokens)
            .max_texts_per_request(self.config.batch_size)
            .sanitize_mode(SanitizeMode::AsciiFastPath)
            .overlap_tokens(
                std::env::var("CODEGRAPH_CHUNK_OVERLAP_TOKENS")
                    .ok()
                    .and_then(|value| value.parse().ok())
                    .unwrap_or(64),
            );
        config.skip_chunking = std::env::var("CODEGRAPH_EMBEDDING_SKIP_CHUNKING")
            .is_ok_and(|value| value == "1" || value.eq_ignore_ascii_case("true"));
        config.smart_split = std::env::var("CODEGRAPH_CHUNK_SMART_SPLIT").map_or(true, |value| {
            value == "1" || value.eq_ignore_ascii_case("true")
        });
        let tokenizer = policy.tokenizer.clone();
        config.token_counter = Some((
            policy.identity.clone(),
            Arc::new(move |text| policy.document_tokens(text)),
        ));
        build_chunk_plan(nodes, tokenizer, config)
    }

    async fn call_embed_endpoint(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }

        let policy = self.input_policy().await?;
        for text in texts {
            policy.validate(text)?;
        }
        let options = Some(OllamaOptions {
            num_ctx: policy.context_tokens,
        });
        let request = OllamaEmbeddingRequest {
            model: &self.config.model_name,
            input: texts,
            truncate: Some(false),
            options,
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

        if response_data
            .embeddings
            .iter()
            .flatten()
            .any(|value| !value.is_finite())
        {
            return Err(CodeGraphError::Vector(
                "Ollama returned non-finite embedding values".into(),
            ));
        }
        Ok(response_data.embeddings)
    }

    fn is_context_overflow_message(message: &str) -> bool {
        let msg = message.to_lowercase();
        msg.contains("input length exceeds the context length")
            || msg.contains("maximum context length")
            || msg.contains("context length")
    }

    async fn embed_resilient(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        let result = embed_resilient_with(texts, |slice| self.call_embed_endpoint(slice)).await;

        match result {
            Ok(v) => Ok(v),
            Err(e) if texts.len() == 1 && Self::is_context_overflow_message(&e.to_string()) => {
                let chars = texts[0].len();
                Err(CodeGraphError::External(format!(
                    "Ollama embedding request exceeded context length for single input (model={}, chars={}). Verify CODEGRAPH_TOKENIZER_PATH and reduce CODEGRAPH_CHUNK_MAX_TOKENS or configure CODEGRAPH_OLLAMA_NUM_CTX. Input was not truncated. Root error: {}",
                    self.config.model_name, chars, e
                )))
            }
            Err(e) => Err(e),
        }
    }

    pub async fn generate_embeddings_for_texts(
        &self,
        texts: &[String],
        batch_size: usize,
    ) -> Result<Vec<Vec<f32>>> {
        let policy = self.input_policy().await?;
        let prepared = texts
            .iter()
            .map(|text| policy.prepare(text, false))
            .collect::<Result<Vec<_>>>()?;
        self.generate_prepared_embeddings(&prepared, batch_size)
            .await
    }

    pub async fn generate_prepared_embeddings(
        &self,
        texts: &[String],
        batch_size: usize,
    ) -> Result<Vec<Vec<f32>>> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }

        let mut all_embeddings = Vec::with_capacity(texts.len());

        for (batch_idx, batch) in texts.chunks(batch_size.max(1)).enumerate() {
            trace!(
                "Sending Ollama embed batch {} ({} items)",
                batch_idx + 1,
                batch.len()
            );
            let batch_embeddings = self.embed_resilient(batch).await?;
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
        let payload = vec![self.input_policy().await?.prepare(text, true)?];
        let mut embeddings = self
            .generate_prepared_embeddings(&payload, self.config.batch_size)
            .await?;
        embeddings
            .pop()
            .ok_or_else(|| CodeGraphError::Vector("Ollama returned no embedding".to_string()))
    }
}

fn model_names_match(actual: &str, desired: &str) -> bool {
    fn normalize(name: &str) -> String {
        let name = name.to_ascii_lowercase();
        if name
            .rsplit('/')
            .next()
            .is_some_and(|part| part.contains(':'))
        {
            name
        } else {
            format!("{name}:latest")
        }
    }
    normalize(actual) == normalize(desired)
}

async fn embed_resilient_with<'t, F, Fut>(
    texts: &'t [String],
    mut embed_once: F,
) -> Result<Vec<Vec<f32>>>
where
    F: FnMut(&'t [String]) -> Fut,
    Fut: std::future::Future<Output = Result<Vec<Vec<f32>>>> + Send + 't,
{
    if texts.is_empty() {
        return Ok(Vec::new());
    }

    let mut out: Vec<Option<Vec<f32>>> = vec![None; texts.len()];
    let mut stack: Vec<(usize, usize)> = vec![(0, texts.len())];

    while let Some((start, end)) = stack.pop() {
        let slice = &texts[start..end];
        match embed_once(slice).await {
            Ok(embeddings) => {
                if embeddings.len() != slice.len() {
                    return Err(CodeGraphError::Vector(format!(
                        "Embedding provider returned {} embeddings for {} inputs",
                        embeddings.len(),
                        slice.len()
                    )));
                }
                for (offset, emb) in embeddings.into_iter().enumerate() {
                    out[start + offset] = Some(emb);
                }
            }
            Err(e)
                if slice.len() > 1
                    && OllamaEmbeddingProvider::is_context_overflow_message(&e.to_string()) =>
            {
                let mid = start + (slice.len() / 2);
                stack.push((mid, end));
                stack.push((start, mid));
            }
            Err(e) => return Err(e),
        }
    }

    Ok(out
        .into_iter()
        .map(|v| v.ok_or_else(|| CodeGraphError::Vector("Missing embedding result".to_string())))
        .collect::<Result<Vec<_>>>()?)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn document_query_prefixes_and_overflow_errors_reach_strict_requests() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let mut bodies = Vec::new();
            for request in 0..3 {
                let (mut stream, _) = listener.accept().await.unwrap();
                let mut bytes = Vec::new();
                let body_start;
                loop {
                    let mut buffer = [0; 4096];
                    let count = stream.read(&mut buffer).await.unwrap();
                    assert!(count > 0);
                    bytes.extend_from_slice(&buffer[..count]);
                    if let Some(end) = bytes.windows(4).position(|window| window == b"\r\n\r\n") {
                        let header = String::from_utf8_lossy(&bytes[..end]).to_ascii_lowercase();
                        let length: usize = header
                            .lines()
                            .find_map(|line| line.strip_prefix("content-length: "))
                            .unwrap()
                            .parse()
                            .unwrap();
                        if bytes.len() >= end + 4 + length {
                            body_start = end + 4;
                            break;
                        }
                    }
                }
                bodies.push(
                    serde_json::from_slice::<serde_json::Value>(&bytes[body_start..]).unwrap(),
                );
                let (status, body) = if request == 2 {
                    (
                        "400 Bad Request",
                        "{\"error\":\"input length exceeds the context length\"}",
                    )
                } else {
                    ("200 OK", "{\"embeddings\":[[1.0,2.0]]}")
                };
                let response = format!(
                    "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                stream.write_all(response.as_bytes()).await.unwrap();
            }
            bodies
        });
        let provider = OllamaEmbeddingProvider::new(OllamaEmbeddingConfig {
            base_url: format!("http://{address}"),
            ..Default::default()
        });
        let tokenizer = Tokenizer::from_file(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tokenizers/qwen2.5-coder.json"
        ))
        .unwrap();
        provider
            .input_policy
            .set(
                InputPolicy::new(
                    tokenizer,
                    512,
                    "search_document: ".into(),
                    "search_query: ".into(),
                    2,
                    "fixture",
                )
                .unwrap(),
            )
            .unwrap_or_else(|_| panic!("policy initialized twice"));
        assert_eq!(
            provider
                .generate_embeddings_for_texts(&["code".into()], 64)
                .await
                .unwrap(),
            vec![vec![1.0, 2.0]]
        );
        assert_eq!(
            provider.generate_single_embedding("query").await.unwrap(),
            vec![1.0, 2.0]
        );
        let error = provider
            .generate_single_embedding("server-limit")
            .await
            .unwrap_err();
        assert!(error.to_string().contains("Input was not truncated"));
        let bodies = tokio::time::timeout(Duration::from_secs(5), server)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(bodies[0]["input"][0], "search_document: code");
        assert_eq!(bodies[1]["input"][0], "search_query: query");
        for body in bodies {
            assert_eq!(body["truncate"], false);
            assert_eq!(body["options"]["num_ctx"], 512);
        }
    }
    #[test]
    fn availability_requires_the_selected_model_and_tag() {
        assert!(model_names_match(
            "nomic-embed-text:latest",
            "nomic-embed-text"
        ));
        assert!(!model_names_match(
            "nomic-embed-text:latest",
            "qwen3-embedding:0.6b"
        ));
        assert!(!model_names_match(
            "qwen3-embedding:4b",
            "qwen3-embedding:0.6b"
        ));
    }

    #[test]
    fn configured_batch_sizes_above_256_are_preserved() {
        let config = codegraph_core::EmbeddingConfig {
            batch_size: 4096,
            ..Default::default()
        };
        assert_eq!(OllamaEmbeddingConfig::from(&config).batch_size, 4096);
    }

    #[test]
    fn detects_context_overflow_messages() {
        assert!(OllamaEmbeddingProvider::is_context_overflow_message(
            "Ollama embedding API error: {\"error\":\"the input length exceeds the context length\"}"
        ));
        assert!(OllamaEmbeddingProvider::is_context_overflow_message(
            "maximum context length exceeded"
        ));
    }

    #[tokio::test]
    async fn embed_resilient_with_splits_on_overflow_and_preserves_order() -> Result<()> {
        let texts: Vec<String> = (0..10).map(|i| "x".repeat((i + 1) * 10)).collect();

        // Fail when total chars in the request exceed 120.
        let embed_once = |slice: &[String]| {
            let lengths: Vec<usize> = slice.iter().map(|s| s.len()).collect();
            async move {
                let total: usize = lengths.iter().sum();
                if total > 120 {
                    return Err(CodeGraphError::External(
                        "the input length exceeds the context length".to_string(),
                    ));
                }
                Ok(lengths
                    .into_iter()
                    .map(|len| vec![len as f32])
                    .collect::<Vec<_>>())
            }
        };

        let embeddings = embed_resilient_with(&texts, embed_once).await?;
        assert_eq!(embeddings.len(), texts.len());
        for (i, emb) in embeddings.iter().enumerate() {
            assert_eq!(emb.len(), 1);
            assert_eq!(emb[0], texts[i].len() as f32);
        }
        Ok(())
    }

    #[tokio::test]
    async fn embed_resilient_with_does_not_split_on_non_overflow_error() {
        let texts: Vec<String> = (0..4).map(|_| "x".to_string()).collect();
        let calls = std::sync::Arc::new(std::sync::Mutex::new(0usize));
        let calls_clone = calls.clone();

        let embed_once = move |_slice: &[String]| {
            let calls_inner = calls_clone.clone();
            async move {
                *calls_inner.lock().unwrap() += 1;
                Err(CodeGraphError::External("some other error".to_string()))
            }
        };

        let res = embed_resilient_with(&texts, embed_once).await;
        assert!(res.is_err());
        assert_eq!(*calls.lock().unwrap(), 1);
    }
}

#[async_trait]
impl EmbeddingProvider for OllamaEmbeddingProvider {
    /// Generate embedding for a single code node
    async fn generate_embedding(&self, node: &CodeNode) -> Result<Vec<f32>> {
        self.generate_embeddings(std::slice::from_ref(node))
            .await?
            .pop()
            .ok_or_else(|| CodeGraphError::Vector("No node embedding returned".into()))
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

        let plan = self.build_plan_for_nodes(nodes).await?;
        let chunk_to_node = plan.chunk_to_node();
        let all_texts: Vec<_> = plan.chunks.into_iter().map(|chunk| chunk.text).collect();

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
