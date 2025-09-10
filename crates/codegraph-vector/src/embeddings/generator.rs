use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::num::NonZeroUsize;
use std::sync::Arc;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use codegraph_core::{CodeGraphError, CodeNode, Language, Location, NodeId, NodeType, Result};
use tokio::sync::Mutex;
use tracing::{debug, info, warn};

use crate::providers::{BatchConfig as ProviderBatchConfig, EmbeddingMetrics, EmbeddingProvider, FallbackStrategy, HybridEmbeddingPipeline};
use crate::simd_ops::SIMDVectorOps;

#[cfg(feature = "local-embeddings")]
use crate::local_provider::{DeviceType, LocalEmbeddingConfig, LocalEmbeddingProvider, PoolingStrategy};

#[cfg(feature = "openai")]
use crate::openai_provider::{OpenAiConfig, OpenAiEmbeddingProvider};

/// Configuration for the dual-mode embedding engine.
#[derive(Debug, Clone)]
pub struct EmbeddingEngineConfig {
    pub prefer_local_first: bool,
    pub batch_size: usize,
    pub max_concurrent_batches: usize,
    pub request_timeout: Duration,
    pub cache_ttl: Duration,
    pub cache_max_entries: usize,
    pub cache_memory_limit_bytes: usize,
    pub quality_similarity_threshold: f32,
    pub dimension_hint: Option<usize>,

    // Optional local provider config
    pub local: Option<LocalEmbeddingConfigCompat>,
    // Optional OpenAI provider config
    pub openai: Option<OpenAiConfigCompat>,
}

impl Default for EmbeddingEngineConfig {
    fn default() -> Self {
        Self {
            prefer_local_first: true,
            batch_size: 256,
            max_concurrent_batches: 4,
            request_timeout: Duration::from_secs(30),
            cache_ttl: Duration::from_secs(3600),
            cache_max_entries: 10_000,
            cache_memory_limit_bytes: 50 * 1024 * 1024, // 50MB
            quality_similarity_threshold: 0.80,
            dimension_hint: Some(768),
            local: None,
            openai: None,
        }
    }
}

/// Compatibility layer for LocalEmbeddingConfig to avoid direct dependency when feature is off.
#[derive(Debug, Clone)]
pub struct LocalEmbeddingConfigCompat {
    pub model_name: String,
    pub device: LocalDeviceTypeCompat,
    pub cache_dir: Option<String>,
    pub max_sequence_length: usize,
    pub pooling_strategy: LocalPoolingCompat,
}

#[derive(Debug, Clone)]
pub enum LocalDeviceTypeCompat { Cpu, Cuda(usize), Metal }

#[derive(Debug, Clone)]
pub enum LocalPoolingCompat { Cls, Mean, Max }

#[cfg(feature = "local-embeddings")]
impl From<&LocalEmbeddingConfigCompat> for LocalEmbeddingConfig {
    fn from(v: &LocalEmbeddingConfigCompat) -> Self {
        let device = match v.device {
            LocalDeviceTypeCompat::Cpu => DeviceType::Cpu,
            LocalDeviceTypeCompat::Cuda(id) => DeviceType::Cuda(id),
            LocalDeviceTypeCompat::Metal => DeviceType::Metal,
        };
        let pooling = match v.pooling_strategy {
            LocalPoolingCompat::Cls => PoolingStrategy::Cls,
            LocalPoolingCompat::Mean => PoolingStrategy::Mean,
            LocalPoolingCompat::Max => PoolingStrategy::Max,
        };
        LocalEmbeddingConfig {
            model_name: v.model_name.clone(),
            device,
            cache_dir: v.cache_dir.clone(),
            max_sequence_length: v.max_sequence_length,
            pooling_strategy: pooling,
        }
    }
}

#[derive(Debug, Clone)]
pub struct OpenAiConfigCompat {
    pub api_key: String,
    pub model: String,
    pub api_base: String,
    pub max_retries: usize,
    pub timeout: Duration,
    pub max_tokens_per_request: usize,
}

#[cfg(feature = "openai")]
impl From<&OpenAiConfigCompat> for OpenAiConfig {
    fn from(v: &OpenAiConfigCompat) -> Self {
        OpenAiConfig {
            api_key: v.api_key.clone(),
            model: v.model.clone(),
            api_base: v.api_base.clone(),
            max_retries: v.max_retries,
            timeout: v.timeout,
            max_tokens_per_request: v.max_tokens_per_request,
        }
    }
}

/// Internal cache entry for embeddings.
#[derive(Clone)]
struct CacheEntry {
    created_at: Instant,
    size_bytes: usize,
    embedding: Arc<Vec<f32>>,
}

/// LRU cache with TTL and memory cap for embeddings.
struct EmbeddingLruCache {
    ttl: Duration,
    max_entries: usize,
    max_bytes: usize,
    current_bytes: usize,
    inner: lru::LruCache<u64, CacheEntry>,
}

impl EmbeddingLruCache {
    fn new(ttl: Duration, max_entries: usize, max_bytes: usize) -> Self {
        let cap = NonZeroUsize::new(max_entries.max(1)).unwrap();
        Self {
            ttl,
            max_entries: max_entries.max(1),
            max_bytes,
            current_bytes: 0,
            inner: lru::LruCache::new(cap),
        }
    }

    fn get(&mut self, key: &u64) -> Option<Arc<Vec<f32>>> {
        if let Some(entry) = self.inner.get(key) {
            if entry.created_at.elapsed() <= self.ttl {
                return Some(entry.embedding.clone());
            }
            // expired
        }
        // Remove expired
        if let Some(old) = self.inner.pop(key) {
            self.current_bytes = self.current_bytes.saturating_sub(old.size_bytes);
        }
        None
    }

    fn insert(&mut self, key: u64, embedding: Vec<f32>) {
        let size = embedding.len() * std::mem::size_of::<f32>();
        let value = CacheEntry {
            created_at: Instant::now(),
            size_bytes: size,
            embedding: Arc::new(embedding),
        };

        self.current_bytes = self.current_bytes.saturating_add(size);
        self.inner.put(key, value);
        self.enforce_limits();
    }

    fn enforce_limits(&mut self) {
        while self.inner.len() > self.max_entries || self.current_bytes > self.max_bytes {
            if let Some((_k, v)) = self.inner.pop_lru() {
                self.current_bytes = self.current_bytes.saturating_sub(v.size_bytes);
            } else {
                break;
            }
        }
    }
}

fn hash_text(text: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    text.hash(&mut hasher);
    hasher.finish()
}

fn node_from_text(text: &str) -> CodeNode {
    let name = if text.len() <= 32 { text.to_string() } else { text[..32].to_string() };
    CodeNode::new(
        name,
        Some(NodeType::Other("text".into())),
        Some(Language::Other("text".into())),
        Location {
            file_path: "<inline>".into(),
            line: 0,
            column: 0,
            end_line: None,
            end_column: None,
        },
    )
    .with_content(text.to_string())
}

/// Dual-mode embedding engine with local Candle provider and OpenAI fallback.
pub struct AdvancedEmbeddingGenerator {
    config: EmbeddingEngineConfig,
    cache: Arc<Mutex<EmbeddingLruCache>>,
    pipeline: Option<HybridEmbeddingPipeline>,
    // In case both features are disabled, expose a deterministic fallback
    deterministic_dim: usize,
}

impl AdvancedEmbeddingGenerator {
    pub async fn new(config: EmbeddingEngineConfig) -> Result<Self> {
        let cache = Arc::new(Mutex::new(EmbeddingLruCache::new(
            config.cache_ttl,
            config.cache_max_entries,
            config.cache_memory_limit_bytes,
        )));

        // Build providers based on features and availability
        let mut primary: Option<Box<dyn EmbeddingProvider>> = None;
        let mut fallbacks: Vec<Box<dyn EmbeddingProvider>> = Vec::new();

        // Helper to push local provider
        #[cfg(feature = "local-embeddings")]
        async fn make_local(cfg: &EmbeddingEngineConfig) -> Result<Box<dyn EmbeddingProvider>> {
            let local_cfg = if let Some(ref c) = cfg.local {
                c
            } else {
                // Sensible defaults for local inference
                &LocalEmbeddingConfigCompat {
                    model_name: "sentence-transformers/all-MiniLM-L6-v2".to_string(),
                    device: LocalDeviceTypeCompat::Cpu,
                    cache_dir: None,
                    max_sequence_length: 512,
                    pooling_strategy: LocalPoolingCompat::Mean,
                }
            };
            let provider = LocalEmbeddingProvider::new(LocalEmbeddingConfig::from(local_cfg)).await?;
            Ok(Box::new(provider))
        }

        #[cfg(feature = "openai")]
        fn make_openai(cfg: &EmbeddingEngineConfig) -> Result<Box<dyn EmbeddingProvider>> {
            let oc = if let Some(ref c) = cfg.openai {
                c
            } else {
                &OpenAiConfigCompat {
                    api_key: std::env::var("OPENAI_API_KEY").unwrap_or_default(),
                    model: "text-embedding-3-small".into(),
                    api_base: "https://api.openai.com/v1".into(),
                    max_retries: 3,
                    timeout: Duration::from_secs(30),
                    max_tokens_per_request: 8000,
                }
            };
            let provider = OpenAiEmbeddingProvider::new(OpenAiConfig::from(oc))?;
            Ok(Box::new(provider))
        }

        // Wire providers in preferred order
        #[allow(unused_mut)]
        let mut dimension_hint = config.dimension_hint.unwrap_or(768);

        #[cfg(all(feature = "local-embeddings", feature = "openai"))]
        {
            if config.prefer_local_first {
                if let Ok(local) = make_local(&config).await {
                    dimension_hint = local.embedding_dimension();
                    primary = Some(local);
                    if let Ok(openai) = make_openai(&config) {
                        fallbacks.push(openai);
                    }
                } else if let Ok(openai) = make_openai(&config) {
                    dimension_hint = openai.embedding_dimension();
                    primary = Some(openai);
                }
            } else {
                if let Ok(openai) = make_openai(&config) {
                    dimension_hint = openai.embedding_dimension();
                    primary = Some(openai);
                    if let Ok(local) = make_local(&config).await {
                        fallbacks.push(local);
                    }
                } else if let Ok(local) = make_local(&config).await {
                    dimension_hint = local.embedding_dimension();
                    primary = Some(local);
                }
            }
        }

        #[cfg(all(feature = "local-embeddings", not(feature = "openai")))]
        {
            if let Ok(local) = make_local(&config).await {
                dimension_hint = local.embedding_dimension();
                primary = Some(local);
            }
        }

        #[cfg(all(feature = "openai", not(feature = "local-embeddings")))]
        {
            if let Ok(openai) = make_openai(&config) {
                dimension_hint = openai.embedding_dimension();
                primary = Some(openai);
            }
        }

        let pipeline = if let Some(primary) = primary {
            let mut pipe = HybridEmbeddingPipeline::new(primary, FallbackStrategy::Sequential);
            for fb in fallbacks {
                pipe = pipe.add_fallback(fb);
            }
            Some(pipe)
        } else {
            None
        };

        Ok(Self {
            config,
            cache,
            pipeline,
            deterministic_dim: dimension_hint,
        })
    }

    fn provider_batch_config(&self) -> ProviderBatchConfig {
        ProviderBatchConfig {
            batch_size: self.config.batch_size,
            max_concurrent: self.config.max_concurrent_batches,
            timeout: self.config.request_timeout,
            retry_attempts: 3,
        }
    }

    /// Generate embeddings for arbitrary texts with caching and batching.
    pub async fn embed_texts(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }

        // Check cache
        let mut outputs: Vec<Option<Arc<Vec<f32>>>> = Vec::with_capacity(texts.len());
        let mut misses: Vec<(usize, String)> = Vec::new();

        {
            let mut cache = self.cache.lock().await;
            for (i, t) in texts.iter().enumerate() {
                let key = hash_text(t);
                if let Some(hit) = cache.get(&key) {
                    outputs.push(Some(hit));
                } else {
                    outputs.push(None);
                    misses.push((i, t.clone()));
                }
            }
        }

        if !misses.is_empty() {
            let nodes: Vec<CodeNode> = misses.iter().map(|(_, t)| node_from_text(t)).collect();
            let new_embeddings = self.generate_embeddings_for_nodes(&nodes).await?;
            // Populate cache and outputs
            let mut cache = self.cache.lock().await;
            for ((idx, text), emb) in misses.into_iter().zip(new_embeddings.into_iter()) {
                let key = hash_text(&text);
                cache.insert(key, emb.clone());
                outputs[idx] = Some(Arc::new(emb));
            }
        }

        // Convert Arcs into Vec<f32>
        Ok(outputs
            .into_iter()
            .map(|o| o.expect("all entries resolved").as_ref().clone())
            .collect())
    }

    /// Generate embeddings for CodeNodes; uses provider pipeline or deterministic fallback.
    pub async fn generate_embeddings_for_nodes(&self, nodes: &[CodeNode]) -> Result<Vec<Vec<f32>>> {
        if nodes.is_empty() {
            return Ok(Vec::new());
        }

        if let Some(pipeline) = &self.pipeline {
            let cfg = self.provider_batch_config();
            let (emb, metrics) = pipeline.generate_embeddings_with_config(nodes, &cfg).await?;
            info!(
                "Embedding pipeline: {} texts in {:?} ({:.1} tps)",
                metrics.texts_processed, metrics.duration, metrics.throughput
            );
            Ok(emb)
        } else {
            // Deterministic fallback (no features enabled) to keep API usable
            warn!("No embedding providers available. Using deterministic fallback embeddings.");
            let dim = self.deterministic_dim.max(32);
            let mut out = Vec::with_capacity(nodes.len());
            for n in nodes {
                let text = format!(
                    "{} {} {} {}",
                    n.language
                        .as_ref()
                        .map(|l| format!("{:?}", l))
                        .unwrap_or_else(|| "unknown".into()),
                    n.node_type
                        .as_ref()
                        .map(|t| format!("{:?}", t))
                        .unwrap_or_else(|| "unknown".into()),
                    n.name,
                    n.content.as_deref().unwrap_or("")
                );
                out.push(self.deterministic_embed(&text, dim));
            }
            Ok(out)
        }
    }

    /// Wrapper that ensures 1000+ text batches are processed in chunks efficiently.
    pub async fn embed_texts_batched(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        if texts.len() <= self.config.batch_size {
            return self.embed_texts(texts).await;
        }
        let mut result = Vec::with_capacity(texts.len());
        for chunk in texts.chunks(self.config.batch_size) {
            let emb = self.embed_texts(chunk).await?;
            result.extend(emb);
        }
        Ok(result)
    }

    /// Quality validation: compute cosine similarity across pairs and return average.
    pub fn validate_similarity_pairs(&self, pairs: &[(Vec<f32>, Vec<f32>)]) -> Result<(f32, usize)> {
        if pairs.is_empty() {
            return Ok((0.0, 0));
        }
        let mut sum = 0.0f32;
        let mut count = 0usize;
        for (a, b) in pairs.iter() {
            let sim = SIMDVectorOps::adaptive_cosine_similarity(a, b)?;
            sum += sim;
            count += 1;
        }
        Ok((sum / count as f32, count))
    }

    /// Returns true if average similarity across the set exceeds configured threshold.
    pub fn passes_quality_threshold(&self, pairs: &[(Vec<f32>, Vec<f32>)]) -> Result<bool> {
        let (avg, n) = self.validate_similarity_pairs(pairs)?;
        if n == 0 { return Ok(false); }
        Ok(avg >= self.config.quality_similarity_threshold)
    }

    fn deterministic_embed(&self, text: &str, dim: usize) -> Vec<f32> {
        // Simple deterministic pseudo-embedding with L2 normalization
        let mut out = vec![0.0f32; dim];
        let mut h = 5381u32;
        for b in text.bytes() { h = h.wrapping_mul(33).wrapping_add(b as u32); }
        let mut state = h;
        for i in 0..dim {
            state = state.wrapping_mul(1103515245).wrapping_add(12345);
            out[i] = ((state as f32 / u32::MAX as f32) - 0.5) * 2.0;
        }
        let norm: f32 = out.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 0.0 { for v in &mut out { *v /= norm; } }
        out
    }
}

#[async_trait]
pub trait TextEmbeddingEngine {
    async fn embed(&self, text: &str) -> Result<Vec<f32>>;
    async fn embed_many(&self, texts: &[String]) -> Result<Vec<Vec<f32>>>;
}

#[async_trait]
impl TextEmbeddingEngine for AdvancedEmbeddingGenerator {
    async fn embed(&self, text: &str) -> Result<Vec<f32>> {
        let v = self.embed_texts(&[text.to_string()]).await?;
        Ok(v.into_iter().next().unwrap_or_default())
    }

    async fn embed_many(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        self.embed_texts_batched(texts).await
    }
}

