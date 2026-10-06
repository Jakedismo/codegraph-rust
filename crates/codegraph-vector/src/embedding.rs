#[cfg(any(feature = "local-embeddings", feature = "openai", feature = "onnx"))]
use crate::embeddings::generator::TextEmbeddingEngine;
use crate::prep::chunker::{
    ChunkPlan, ChunkerConfig, SanitizeMode, aggregate_chunk_embeddings, build_chunk_plan,
    build_chunk_plan_with_sources,
};
#[cfg(feature = "ollama")]
use crate::providers::EmbeddingProvider;
use codegraph_core::{CodeGraphError, CodeNode, Result};
use std::{path::PathBuf, sync::Arc};
use tokenizers::Tokenizer;

pub struct EmbeddingGenerator {
    model_config: ModelConfig,
    #[cfg(any(feature = "local-embeddings", feature = "openai", feature = "onnx"))]
    pub(crate) advanced: Option<Arc<crate::embeddings::generator::AdvancedEmbeddingGenerator>>,
    #[cfg(feature = "ollama")]
    ollama_provider: Option<crate::ollama_embedding_provider::OllamaEmbeddingProvider>,
    #[cfg(feature = "jina")]
    jina_provider: Option<crate::jina_provider::JinaEmbeddingProvider>,
    #[cfg(feature = "lmstudio")]
    lmstudio_provider: Option<crate::lmstudio_embedding_provider::LmStudioEmbeddingProvider>,
    tokenizer: Arc<Tokenizer>,
    submitted_cache: Option<crate::submitted_cache::SubmittedCache>,
    chunk_cache_dir: Option<PathBuf>,
}

#[derive(Debug, Clone)]
pub struct ModelConfig {
    pub dimension: usize,
    pub max_tokens: usize,
    pub model_name: String,
}

impl Default for ModelConfig {
    fn default() -> Self {
        Self {
            dimension: 384,
            max_tokens: 512,
            model_name: "sentence-transformers/all-MiniLM-L6-v2".to_string(),
        }
    }
}

impl Default for EmbeddingGenerator {
    fn default() -> Self {
        Self::new(ModelConfig::default())
    }
}

impl EmbeddingGenerator {
    pub fn new(config: ModelConfig) -> Self {
        let tokenizer_path = PathBuf::from(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tokenizers/qwen2.5-coder.json"
        ));
        let tokenizer = Tokenizer::from_file(&tokenizer_path).unwrap_or_else(|e| {
            panic!(
                "Failed to load tokenizer from {:?}: {}. This tokenizer is required for chunking.",
                tokenizer_path, e
            )
        });

        Self {
            model_config: config,
            #[cfg(any(feature = "local-embeddings", feature = "openai", feature = "onnx"))]
            advanced: None,
            #[cfg(feature = "ollama")]
            ollama_provider: None,
            #[cfg(feature = "jina")]
            jina_provider: None,
            #[cfg(feature = "lmstudio")]
            lmstudio_provider: None,
            tokenizer: Arc::new(tokenizer),
            submitted_cache: None,
            chunk_cache_dir: None,
        }
    }

    #[cfg(any(feature = "local-embeddings", feature = "openai", feature = "onnx"))]
    pub fn set_advanced_engine(
        &mut self,
        engine: Arc<crate::embeddings::generator::AdvancedEmbeddingGenerator>,
    ) {
        self.model_config.dimension = engine.dimension();
        if let Some(model) = engine.model_name() {
            self.model_config.model_name = model.to_owned();
        }
        if let Some(tokenizer) = engine.tokenizer() {
            self.tokenizer = tokenizer;
        }
        self.model_config.max_tokens = engine.max_input_tokens().saturating_sub(2).max(1);
        self.advanced = Some(engine);
    }

    #[cfg(feature = "jina")]
    pub fn set_jina_batch_size(&mut self, batch_size: usize) {
        if let Some(ref mut provider) = self.jina_provider {
            provider.set_batch_size(batch_size);
        }
    }

    #[cfg(feature = "jina")]
    pub fn set_jina_max_concurrent(&mut self, max_concurrent: usize) {
        if let Some(ref mut provider) = self.jina_provider {
            provider.set_max_concurrent(max_concurrent);
        }
    }

    fn chunker_config(&self) -> ChunkerConfig {
        let config = self.base_chunker_config();
        #[cfg(feature = "openai")]
        if self.model_config.model_name.starts_with("text-embedding-")
            && let Ok(bpe) = tiktoken_rs::bpe_for_model(&self.model_config.model_name)
        {
            let mut config = config;
            config.token_counter = Some((
                format!("tiktoken-0.12:{}", self.model_config.model_name),
                Arc::new(move |text| bpe.encode_ordinary(text).len()),
            ));
            return config;
        }
        config
    }

    fn base_chunker_config(&self) -> ChunkerConfig {
        // Allow skipping chunking for speed with env flag
        let skip_chunking = std::env::var("CODEGRAPH_EMBEDDING_SKIP_CHUNKING")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false);

        let max_tokens_env = std::env::var("CODEGRAPH_CHUNK_MAX_TOKENS")
            .ok()
            .and_then(|v| v.parse::<usize>().ok());
        let max_tokens = max_tokens_env
            .unwrap_or(self.model_config.max_tokens)
            .clamp(1, self.model_config.max_tokens.max(1));
        let overlap_tokens = std::env::var("CODEGRAPH_CHUNK_OVERLAP_TOKENS")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(64);
        let smart_split = std::env::var("CODEGRAPH_CHUNK_SMART_SPLIT")
            .ok()
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(true);

        if skip_chunking {
            ChunkerConfig::new(max_tokens)
                .sanitize_mode(SanitizeMode::AsciiFastPath)
                .cache_capacity(2048)
                .overlap_tokens(0)
                .smart_split(false)
        } else {
            ChunkerConfig::new(max_tokens)
                .sanitize_mode(SanitizeMode::AsciiFastPath)
                .cache_capacity(2048)
                .overlap_tokens(overlap_tokens)
                .smart_split(smart_split)
        }
    }

    fn build_plan_for_nodes(&self, nodes: &[CodeNode]) -> ChunkPlan {
        build_chunk_plan(nodes, Arc::clone(&self.tokenizer), self.chunker_config())
    }

    /// Expose chunking so callers can persist chunk-level embeddings.
    pub fn chunk_nodes(&self, nodes: &[CodeNode]) -> ChunkPlan {
        self.build_plan_for_nodes(nodes)
    }

    pub fn chunk_nodes_with_sources(
        &self,
        nodes: &[CodeNode],
        file_sources: &std::collections::HashMap<String, String>,
    ) -> ChunkPlan {
        build_chunk_plan_with_sources(
            nodes,
            file_sources,
            Arc::clone(&self.tokenizer),
            self.chunker_config()
                .cache_dir(self.chunk_cache_dir.clone()),
        )
    }

    pub fn chunk_nodes_with_source_lookup(
        &self,
        nodes: &[CodeNode],
        source_lookup: impl Fn(&str) -> Option<Arc<str>> + Sync,
    ) -> ChunkPlan {
        crate::prep::chunker::build_chunk_plan_with_source_lookup(
            nodes,
            source_lookup,
            self.tokenizer.clone(),
            self.chunker_config()
                .cache_dir(self.chunk_cache_dir.clone()),
        )
    }

    pub fn has_provider(&self) -> bool {
        #[cfg(any(feature = "local-embeddings", feature = "openai", feature = "onnx"))]
        if self
            .advanced
            .as_ref()
            .is_some_and(|engine| engine.has_provider())
        {
            return true;
        }
        #[cfg(feature = "jina")]
        if self.jina_provider.is_some() {
            return true;
        }
        #[cfg(feature = "ollama")]
        if self.ollama_provider.is_some() {
            return true;
        }
        #[cfg(feature = "lmstudio")]
        if self.lmstudio_provider.is_some() {
            return true;
        }
        false
    }
    pub fn dimension(&self) -> usize {
        self.model_config.dimension
    }

    /// Construct an EmbeddingGenerator that optionally wraps the advanced engine based on env.
    /// If CODEGRAPH_EMBEDDING_PROVIDER=local, tries to initialize a local-first engine.
    pub async fn with_auto_from_env() -> Self {
        #[cfg(any(
            feature = "local-embeddings",
            feature = "openai",
            feature = "onnx",
            feature = "ollama",
            feature = "jina",
            feature = "lmstudio"
        ))]
        let mut base = Self::new(ModelConfig::default());
        #[cfg(not(any(
            feature = "local-embeddings",
            feature = "openai",
            feature = "onnx",
            feature = "ollama",
            feature = "jina",
            feature = "lmstudio"
        )))]
        let base = Self::new(ModelConfig::default());
        let provider = std::env::var("CODEGRAPH_EMBEDDING_PROVIDER")
            .unwrap_or_default()
            .to_lowercase();
        if provider == "local" {
            #[cfg(any(feature = "local-embeddings", feature = "openai", feature = "onnx"))]
            {
                use crate::embeddings::generator::{
                    AdvancedEmbeddingGenerator, EmbeddingEngineConfig, LocalDeviceTypeCompat,
                    LocalEmbeddingConfigCompat, LocalPoolingCompat,
                };
                let mut cfg = EmbeddingEngineConfig::default();
                if let Ok(val) = std::env::var("CODEGRAPH_EMBEDDINGS_BATCH_SIZE") {
                    if let Ok(parsed) = val.parse::<usize>() {
                        cfg.batch_size = parsed.clamp(1, 2048);
                    }
                }
                cfg.prefer_local_first = true;
                // Optional model override via env
                if let Ok(model_name) = std::env::var("CODEGRAPH_LOCAL_MODEL") {
                    cfg.local = Some(LocalEmbeddingConfigCompat {
                        model_name,
                        device: LocalDeviceTypeCompat::Cpu,
                        cache_dir: None,
                        max_sequence_length: 512,
                        pooling_strategy: LocalPoolingCompat::Mean,
                    });
                }
                if let Ok(engine) = AdvancedEmbeddingGenerator::new(cfg).await {
                    base.set_advanced_engine(Arc::new(engine));
                }
            }
        } else if provider == "onnx" {
            #[cfg(feature = "onnx")]
            {
                use crate::embeddings::generator::{
                    AdvancedEmbeddingGenerator, EmbeddingEngineConfig, OnnxConfigCompat,
                };
                let mut cfg = EmbeddingEngineConfig::default();
                let model_repo = std::env::var("CODEGRAPH_LOCAL_MODEL").unwrap_or_default();
                tracing::info!(
                    "🚀 Initializing ONNX embedding provider with model: {}",
                    model_repo
                );

                cfg.onnx = Some(OnnxConfigCompat {
                    model_repo: model_repo.clone(),
                    model_file: Some("model.onnx".into()),
                    max_sequence_length: 512,
                    pooling: "mean".into(),
                });

                match AdvancedEmbeddingGenerator::new(cfg).await {
                    Ok(engine) => {
                        tracing::info!("✅ ONNX embedding provider initialized successfully");
                        base.set_advanced_engine(Arc::new(engine));
                    }
                    Err(e) => {
                        tracing::error!("❌ ONNX embedding provider failed to initialize: {}", e);
                        tracing::error!("   Model path: {}", model_repo);
                        tracing::warn!(
                            "🔄 Attempting fallback to Ollama embeddings for AI semantic matching..."
                        );

                        // INTELLIGENT FALLBACK: Try Ollama if ONNX fails
                        #[cfg(feature = "ollama")]
                        {
                            let ollama_config =
                                crate::ollama_embedding_provider::OllamaEmbeddingConfig::default();
                            let ollama_provider =
                                crate::ollama_embedding_provider::OllamaEmbeddingProvider::new(
                                    ollama_config,
                                );

                            match ollama_provider.check_availability().await {
                                Ok(true) => {
                                    tracing::info!(
                                        "✅ Fallback successful: Ollama nomic-embed-code available for AI semantic matching"
                                    );
                                    base.model_config.dimension =
                                        ollama_provider.embedding_dimension();
                                    base.ollama_provider = Some(ollama_provider);
                                }
                                Ok(false) => {
                                    tracing::error!(
                                        "❌ Ollama fallback failed: nomic-embed-code model not found"
                                    );
                                    tracing::error!(
                                        "   Install with: ollama pull hf.co/nomic-ai/nomic-embed-code-GGUF:Q4_K_M"
                                    );
                                    tracing::error!(
                                        "   Falling back to random embeddings (no semantic AI matching)"
                                    );
                                }
                                Err(e) => {
                                    tracing::error!("❌ Ollama fallback failed: {}", e);
                                    tracing::error!(
                                        "   Falling back to random embeddings (no semantic AI matching)"
                                    );
                                }
                            }
                        }
                        #[cfg(not(feature = "ollama"))]
                        {
                            tracing::error!(
                                "   Ollama fallback not available (feature not enabled)"
                            );
                            tracing::error!(
                                "   Falling back to random embeddings (no semantic AI matching)"
                            );
                        }

                        tracing::warn!(
                            "⚠️ Without real embeddings, AI semantic matching will be 0% effective"
                        );
                    }
                }
            }
        } else if provider == "ollama" {
            #[cfg(feature = "ollama")]
            {
                // Create Ollama embedding provider
                let ollama_config =
                    crate::ollama_embedding_provider::OllamaEmbeddingConfig::default();
                let ollama_provider =
                    crate::ollama_embedding_provider::OllamaEmbeddingProvider::new(ollama_config);

                // Check if model is available
                match ollama_provider.check_availability().await {
                    Ok(true) => {
                        tracing::info!("✅ Ollama nomic-embed-code available for embeddings");
                        base.model_config.dimension = ollama_provider.embedding_dimension();
                        base.ollama_provider = Some(ollama_provider);
                    }
                    Ok(false) => {
                        tracing::warn!(
                            "⚠️ nomic-embed-code model not found. Install with: ollama pull hf.co/nomic-ai/nomic-embed-code-GGUF:Q4_K_M"
                        );
                    }
                    Err(e) => {
                        tracing::error!("❌ Failed to connect to Ollama for embeddings: {}", e);
                    }
                }
            }
        } else if provider == "jina" {
            #[cfg(feature = "jina")]
            {
                // Create Jina embedding provider
                let jina_config = crate::jina_provider::JinaConfig::default();
                match crate::jina_provider::JinaEmbeddingProvider::new(jina_config) {
                    Ok(jina_provider) => {
                        tracing::info!("✅ Jina code embeddings initialized successfully");
                        // Get dimension from the provider based on model
                        let dimension = jina_provider.embedding_dimension();
                        base.jina_provider = Some(jina_provider);
                        base.model_config.dimension = dimension;
                    }
                    Err(e) => {
                        tracing::error!("❌ Failed to initialize Jina embeddings: {}", e);
                        tracing::error!("   Make sure JINA_API_KEY environment variable is set");
                    }
                }
            }
        } else if provider == "lmstudio" {
            #[cfg(feature = "lmstudio")]
            {
                let lmstudio_config =
                    crate::lmstudio_embedding_provider::LmStudioEmbeddingConfig::default();
                match crate::lmstudio_embedding_provider::LmStudioEmbeddingProvider::new(
                    lmstudio_config,
                ) {
                    Ok(provider) => {
                        tracing::info!("🔍 Checking LM Studio availability...");
                        if provider.check_availability().await {
                            use crate::providers::EmbeddingProvider;
                            tracing::info!("✅ LM Studio embeddings initialized (from env)");
                            base.model_config.dimension = provider.embedding_dimension();
                            base.lmstudio_provider = Some(provider);
                        } else {
                            tracing::error!("❌ LM Studio not available at default URL");
                            tracing::error!(
                                "   Make sure LM Studio is running with an embedding model loaded"
                            );
                        }
                    }
                    Err(e) => {
                        tracing::error!("❌ Failed to initialize LM Studio embeddings: {}", e);
                    }
                }
            }
            #[cfg(not(feature = "lmstudio"))]
            {
                tracing::error!(
                    "❌ 'lmstudio' feature is NOT ENABLED - cannot use LM Studio provider!"
                );
            }
        }
        base
    }

    /// Construct an EmbeddingGenerator from a CodeGraphConfig
    /// This enables TOML configuration file support in addition to environment variables
    pub fn with_config(
        config: &codegraph_core::CodeGraphConfig,
    ) -> futures::future::BoxFuture<'_, Self> {
        Box::pin(async move {
            // Allow env override for batch size (applies across providers)
            let mut embedding_config = config.embedding.clone();
            if let Ok(val) = std::env::var("CODEGRAPH_EMBEDDINGS_BATCH_SIZE")
                && let Ok(parsed) = val.parse::<usize>()
            {
                embedding_config.batch_size = parsed.clamp(1, 2048);
            }
            let model_name = embedding_config
                .model
                .clone()
                .unwrap_or_else(|| "auto".into());
            let max_tokens = if model_name.starts_with("text-embedding-") {
                8191
            } else {
                512
            };
            #[allow(unused_mut)]
            let mut base = Self::new(ModelConfig {
                dimension: embedding_config.dimension,
                max_tokens,
                model_name,
            });

            let provider = embedding_config.provider.to_lowercase();
            tracing::info!(
                "🔍 EmbeddingGenerator::with_config called with provider='{}', model={:?}, dimension={}",
                provider,
                embedding_config.model,
                embedding_config.dimension
            );

            if provider == "ollama" {
                tracing::info!("🎯 Provider matches 'ollama', attempting to initialize...");
                #[cfg(feature = "ollama")]
                {
                    tracing::info!("✅ 'ollama' feature is ENABLED");
                    let ollama_config =
                        crate::ollama_embedding_provider::OllamaEmbeddingConfig::from(
                            &embedding_config,
                        );
                    tracing::info!(
                        "🔧 Created OllamaEmbeddingConfig: model='{}', url='{}'",
                        ollama_config.model_name,
                        ollama_config.base_url
                    );
                    let ollama_provider =
                        crate::ollama_embedding_provider::OllamaEmbeddingProvider::new(
                            ollama_config,
                        );

                    tracing::info!("🔍 Checking Ollama availability...");
                    match ollama_provider.check_availability().await {
                        Ok(true) => {
                            use crate::providers::EmbeddingProvider;
                            tracing::info!(
                                "✅ Ollama {} available for embeddings (from config)",
                                ollama_provider.provider_name()
                            );
                            base.model_config.dimension = ollama_provider.embedding_dimension();
                            base.ollama_provider = Some(ollama_provider);
                            tracing::info!("✅ ollama_provider successfully set!");
                        }
                        Ok(false) => {
                            use crate::providers::EmbeddingProvider;
                            tracing::error!(
                                "❌ Ollama model {} not found. Install with: ollama pull <model>",
                                ollama_provider.provider_name()
                            );
                        }
                        Err(e) => {
                            tracing::error!("❌ Failed to connect to Ollama for embeddings: {}", e);
                        }
                    }
                }
                #[cfg(not(feature = "ollama"))]
                {
                    tracing::error!(
                        "❌ 'ollama' feature is NOT ENABLED - cannot use Ollama provider!"
                    );
                }
            } else if provider == "jina" {
                #[cfg(feature = "jina")]
                {
                    let jina_config = crate::jina_provider::JinaConfig::from(&embedding_config);
                    match crate::jina_provider::JinaEmbeddingProvider::new(jina_config) {
                        Ok(provider) => {
                            tracing::info!("✅ Jina embeddings initialized (from config)");
                            base.model_config.dimension = provider.embedding_dimension();
                            base.jina_provider = Some(provider);
                        }
                        Err(e) => {
                            tracing::error!("❌ Failed to initialize Jina embeddings: {}", e);
                            tracing::error!(
                                "   Make sure jina_api_key is set in config or JINA_API_KEY env var"
                            );
                        }
                    }
                }
            } else if provider == "lmstudio" {
                #[cfg(feature = "lmstudio")]
                {
                    let lmstudio_config =
                        crate::lmstudio_embedding_provider::LmStudioEmbeddingConfig::from(
                            &embedding_config,
                        );
                    match crate::lmstudio_embedding_provider::LmStudioEmbeddingProvider::new(
                        lmstudio_config,
                    ) {
                        Ok(provider) => {
                            tracing::info!("🔍 Checking LM Studio availability...");
                            if provider.check_availability().await {
                                use crate::providers::EmbeddingProvider;
                                tracing::info!("✅ LM Studio embeddings initialized (from config)");
                                base.model_config.dimension = provider.embedding_dimension();
                                base.lmstudio_provider = Some(provider);
                            } else {
                                tracing::error!(
                                    "❌ LM Studio not available at {}",
                                    embedding_config.lmstudio_url
                                );
                                tracing::error!(
                                    "   Make sure LM Studio is running with an embedding model loaded"
                                );
                            }
                        }
                        Err(e) => {
                            tracing::error!("❌ Failed to initialize LM Studio embeddings: {}", e);
                        }
                    }
                }
                #[cfg(not(feature = "lmstudio"))]
                {
                    tracing::error!(
                        "❌ 'lmstudio' feature is NOT ENABLED - cannot use LM Studio provider!"
                    );
                }
            }
            #[cfg(any(feature = "openai", feature = "onnx", feature = "local-embeddings"))]
            if provider == "onnx" || provider == "openai" {
                use crate::embeddings::generator::{
                    AdvancedEmbeddingGenerator, EmbeddingEngineConfig, OnnxConfigCompat,
                    OpenAiConfigCompat,
                };
                let mut engine = EmbeddingEngineConfig {
                    batch_size: embedding_config.batch_size,
                    dimension_hint: Some(embedding_config.dimension),
                    ..Default::default()
                };
                if provider == "onnx" {
                    engine.onnx = Some(OnnxConfigCompat {
                        model_repo: base.model_config.model_name.clone(),
                        model_file: std::env::var("CODEGRAPH_ONNX_MODEL_FILE").ok(),
                        max_sequence_length: 512,
                        pooling: "mean".into(),
                    });
                } else {
                    engine.openai = Some(OpenAiConfigCompat {
                        api_key: embedding_config
                            .openai_api_key
                            .clone()
                            .or_else(|| std::env::var("OPENAI_API_KEY").ok())
                            .unwrap_or_default(),
                        model: base.model_config.model_name.clone(),
                        api_base: std::env::var("OPENAI_API_BASE")
                            .unwrap_or_else(|_| "https://api.openai.com/v1".into()),
                        max_retries: 3,
                        timeout: std::time::Duration::from_secs(30),
                        max_tokens_per_request: 8191,
                    });
                }
                match AdvancedEmbeddingGenerator::new(engine).await {
                    Ok(engine) => base.set_advanced_engine(Arc::new(engine)),
                    Err(error) => tracing::error!(
                        "Failed to initialize {provider} embedding backend: {error}"
                    ),
                }
            }
            // Other unsupported providers fail the indexing startup guard.

            base
        })
    }

    pub async fn generate_embedding(&self, node: &CodeNode) -> Result<Vec<f32>> {
        let mut embeddings = self.generate_embeddings(std::slice::from_ref(node)).await?;
        embeddings
            .pop()
            .ok_or_else(|| CodeGraphError::Vector("No embedding generated".to_string()))
    }

    pub async fn generate_embeddings(&self, nodes: &[CodeNode]) -> Result<Vec<Vec<f32>>> {
        if nodes.is_empty() {
            return Ok(Vec::new());
        }
        // Prefer Jina provider for batch processing (cloud-based embeddings)
        #[cfg(feature = "jina")]
        if let Some(jina) = &self.jina_provider {
            tracing::debug!(
                target: "codegraph_vector::embeddings",
                "Using Jina embeddings for batch: {} items",
                nodes.len()
            );
            use crate::providers::EmbeddingProvider;
            let embs = jina.generate_embeddings(nodes).await?;
            if embs.len() != nodes.len() {
                return Err(CodeGraphError::Vector(format!(
                    "Jina provider returned {} embeddings for {} inputs",
                    embs.len(),
                    nodes.len()
                )));
            }
            return Ok(embs);
        }

        // Prefer Ollama provider for batch processing (code-specialized embeddings)
        #[cfg(feature = "ollama")]
        if let Some(ollama) = &self.ollama_provider {
            use crate::providers::EmbeddingProvider;
            tracing::debug!(
                target: "codegraph_vector::embeddings",
                "Using Ollama {} for batch: {} items",
                ollama.provider_name(),
                nodes.len()
            );
            let embs = ollama.generate_embeddings(nodes).await?;
            if embs.len() != nodes.len() {
                return Err(CodeGraphError::Vector(format!(
                    "Ollama provider returned {} embeddings for {} inputs",
                    embs.len(),
                    nodes.len()
                )));
            }
            return Ok(embs);
        }

        // Prefer LM Studio provider for batch processing (local OpenAI-compatible)
        #[cfg(feature = "lmstudio")]
        if let Some(lmstudio) = &self.lmstudio_provider {
            use crate::providers::EmbeddingProvider;
            tracing::debug!(
                target: "codegraph_vector::embeddings",
                "Using LM Studio {} for batch: {} items",
                lmstudio.provider_name(),
                nodes.len()
            );
            let embs = lmstudio.generate_embeddings(nodes).await?;
            if embs.len() != nodes.len() {
                return Err(CodeGraphError::Vector(format!(
                    "LM Studio provider returned {} embeddings for {} inputs",
                    embs.len(),
                    nodes.len()
                )));
            }
            return Ok(embs);
        }

        #[cfg(any(feature = "local-embeddings", feature = "openai", feature = "onnx"))]
        if let Some(engine) = &self.advanced {
            let plan = self.build_plan_for_nodes(nodes);
            tracing::debug!(
                target: "codegraph_vector::embeddings",
                "Advanced engine chunk plan: {} nodes -> {} chunks",
                plan.stats.total_nodes,
                plan.stats.total_chunks
            );
            let chunk_to_node = plan.chunk_to_node();
            let chunk_texts: Vec<String> =
                plan.chunks.into_iter().map(|chunk| chunk.text).collect();
            tracing::debug!(
                target: "codegraph_vector::embeddings",
                "Using advanced embedding engine for batch: {} chunks",
                chunk_texts.len()
            );
            let chunk_embeddings = engine.embed_many(&chunk_texts).await?;
            if chunk_embeddings.len() != chunk_texts.len() {
                return Err(CodeGraphError::Vector(format!(
                    "provider returned {} embeddings for {} inputs",
                    chunk_embeddings.len(),
                    chunk_texts.len()
                )));
            }
            let aggregated = aggregate_chunk_embeddings(
                nodes.len(),
                &chunk_to_node,
                chunk_embeddings,
                self.dimension(),
            );
            return Ok(aggregated);
        }

        // Fallback: sequential deterministic embeddings with chunking
        let plan = self.build_plan_for_nodes(nodes);
        let chunk_to_node = plan.chunk_to_node();
        let mut chunk_embeddings = Vec::with_capacity(plan.chunks.len());
        for chunk in plan.chunks {
            chunk_embeddings.push(self.encode_text(&chunk.text).await?);
        }
        Ok(aggregate_chunk_embeddings(
            nodes.len(),
            &chunk_to_node,
            chunk_embeddings,
            self.dimension(),
        ))
    }

    /// Generate an embedding directly from free text. Useful for query embeddings.
    pub async fn generate_text_embedding(&self, text: &str) -> Result<Vec<f32>> {
        self.encode_text(text).await
    }

    /// Generate embeddings for multiple texts in batches for GPU optimization.
    /// This method processes texts in batches to maximize GPU utilization.
    pub fn configure_index_cache(
        &mut self,
        root: PathBuf,
        identity: &serde_json::Value,
        dimension: usize,
        provider: &str,
        rows: usize,
    ) -> Result<()> {
        if let Some(path) = std::env::var_os("CODEGRAPH_TOKENIZER_PATH") {
            self.tokenizer =
                Arc::new(Tokenizer::from_file(path).map_err(|e| {
                    CodeGraphError::Vector(format!("Invalid provider tokenizer: {e}"))
                })?);
        }
        let mut tokenizer = self.tokenizer.as_ref().clone();
        tokenizer
            .with_truncation(None)
            .map_err(|error| CodeGraphError::Vector(error.to_string()))?;
        tokenizer.with_padding(None);
        self.tokenizer = Arc::new(tokenizer);
        let namespace = codegraph_core::artifact_cache::fingerprint(&(
            "prepared-v2",
            identity,
            dimension,
            self.tokenizer
                .to_string(false)
                .map_err(|e| CodeGraphError::Vector(e.to_string()))?,
            std::env::var("CODEGRAPH_MODEL_REVISION").unwrap_or_default(),
        ))
        .map_err(|e| CodeGraphError::Vector(e.to_string()))?;
        self.submitted_cache = Some(crate::submitted_cache::SubmittedCache::new(
            namespace,
            dimension,
            Some(root.clone()),
            matches!(provider, "local" | "onnx" | "ollama" | "lmstudio"),
            rows,
        ));
        self.chunk_cache_dir = Some(root);
        Ok(())
    }

    pub fn inference_stats(&self) -> (u64, u64, u64) {
        use std::sync::atomic::Ordering;
        self.submitted_cache.as_ref().map_or((0, 0, 0), |cache| {
            (
                cache.hits.load(Ordering::Relaxed),
                cache.inferred.load(Ordering::Relaxed),
                cache.submitted_tokens.load(Ordering::Relaxed),
            )
        })
    }

    pub async fn embed_texts_batched(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        if let Some(cache) = &self.submitted_cache {
            cache
                .embed(
                    texts,
                    |text| {
                        #[cfg(feature = "openai")]
                        if self.model_config.model_name.starts_with("text-embedding-")
                            && let Ok(bpe) =
                                tiktoken_rs::bpe_for_model(&self.model_config.model_name)
                        {
                            return bpe.encode_ordinary(text).len();
                        }
                        self.tokenizer
                            .encode(text, false)
                            .map_or(text.len(), |tokens| tokens.len())
                    },
                    |batch| Box::pin(async move { self.embed_prepared_uncached(&batch).await }),
                )
                .await
        } else {
            self.embed_prepared_uncached(texts).await
        }
    }

    #[allow(unused_variables)]
    async fn embed_prepared_uncached(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        // Use advanced engine's batching capabilities when available
        #[cfg(any(feature = "local-embeddings", feature = "openai", feature = "onnx"))]
        if let Some(engine) = &self.advanced {
            return engine.embed_texts_batched(texts).await;
        }

        #[cfg(feature = "jina")]
        if let Some(provider) = &self.jina_provider {
            return provider.embed_relationship_texts(texts).await;
        }

        #[cfg(feature = "ollama")]
        if let Some(provider) = &self.ollama_provider {
            return provider
                .generate_embeddings_for_texts(texts, provider.max_batch_size())
                .await;
        }

        #[cfg(feature = "lmstudio")]
        if let Some(provider) = &self.lmstudio_provider {
            return provider.process_in_batches(texts.to_vec()).await;
        }

        Err(CodeGraphError::Vector(
            "No semantic embedding provider configured for prepared text indexing".into(),
        ))
    }

    async fn encode_text(&self, text: &str) -> Result<Vec<f32>> {
        // Prefer Jina provider when available (cloud code embeddings with code.query task)
        #[cfg(feature = "jina")]
        if let Some(jina) = &self.jina_provider {
            // Use code.query task type for search queries (asymmetric embeddings)
            return jina
                .generate_text_embedding_with_task(text, "code.query")
                .await;
        }

        // Prefer Ollama provider when available (code-specialized embeddings)
        #[cfg(feature = "ollama")]
        if let Some(ollama) = &self.ollama_provider {
            return ollama.generate_single_embedding(text).await;
        }

        // Prefer LM Studio provider when available (local OpenAI-compatible embeddings)
        #[cfg(feature = "lmstudio")]
        if let Some(lmstudio) = &self.lmstudio_provider {
            return lmstudio.generate_single_embedding(text).await;
        }

        // Prefer advanced engine when available
        #[cfg(any(feature = "local-embeddings", feature = "openai", feature = "onnx"))]
        if let Some(engine) = &self.advanced {
            return engine.embed(text).await;
        }

        // FALLBACK WARNING: Using random hash-based embeddings (no semantic meaning)
        static FALLBACK_WARNING_SHOWN: std::sync::atomic::AtomicBool =
            std::sync::atomic::AtomicBool::new(false);
        if !FALLBACK_WARNING_SHOWN.swap(true, std::sync::atomic::Ordering::Relaxed) {
            tracing::error!("🚨 CRITICAL: Falling back to random hash-based embeddings");
            tracing::error!("   This means AI semantic matching will be 0% effective");
            tracing::error!(
                "   Resolution rates will remain at baseline (~60%) instead of target (85-90%)"
            );
            tracing::error!("   Fix: Ensure ONNX or Ollama embedding providers are working");
        }

        tokio::task::spawn_blocking({
            let text = text.to_string();
            let dimension = self.model_config.dimension;
            move || {
                let mut embedding = vec![0.0f32; dimension];

                let hash = simple_hash(&text);
                let mut rng_state = hash;

                for i in 0..dimension {
                    rng_state = rng_state.wrapping_mul(1103515245).wrapping_add(12345);
                    embedding[i] = ((rng_state as f32 / u32::MAX as f32) - 0.5) * 2.0;
                }

                let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
                if norm > 0.0 {
                    for x in &mut embedding {
                        *x /= norm;
                    }
                }

                embedding
            }
        })
        .await
        .map_err(|e| CodeGraphError::Vector(e.to_string()))
    }
}

fn simple_hash(text: &str) -> u32 {
    let mut hash = 5381u32;
    for byte in text.bytes() {
        hash = hash.wrapping_mul(33).wrapping_add(byte as u32);
    }
    hash
}
