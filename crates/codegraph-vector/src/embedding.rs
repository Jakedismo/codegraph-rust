#[cfg(any(feature = "local-embeddings", feature = "openai", feature = "onnx"))]
use crate::embeddings::generator::TextEmbeddingEngine;
use crate::prep::chunker::{
    aggregate_chunk_embeddings, build_chunk_plan, ChunkPlan, ChunkerConfig, SanitizeMode,
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
    tokenizer: Arc<Tokenizer>,
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
            tokenizer: Arc::new(tokenizer),
        }
    }

    pub fn default() -> Self {
        Self::new(ModelConfig::default())
    }

    #[cfg(any(feature = "local-embeddings", feature = "openai", feature = "onnx"))]
    pub fn set_advanced_engine(
        &mut self,
        engine: Arc<crate::embeddings::generator::AdvancedEmbeddingGenerator>,
    ) {
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
        ChunkerConfig::new(self.model_config.max_tokens)
            .sanitize_mode(SanitizeMode::AsciiFastPath)
            .cache_capacity(2048)
    }

    fn build_plan_for_nodes(&self, nodes: &[CodeNode]) -> ChunkPlan {
        build_chunk_plan(nodes, Arc::clone(&self.tokenizer), self.chunker_config())
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
            feature = "jina"
        ))]
        let mut base = Self::new(ModelConfig::default());
        #[cfg(not(any(
            feature = "local-embeddings",
            feature = "openai",
            feature = "onnx",
            feature = "ollama",
            feature = "jina"
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
                    base.advanced = Some(Arc::new(engine));
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
                        base.advanced = Some(Arc::new(engine));
                    }
                    Err(e) => {
                        tracing::error!("❌ ONNX embedding provider failed to initialize: {}", e);
                        tracing::error!("   Model path: {}", model_repo);
                        tracing::warn!("🔄 Attempting fallback to Ollama embeddings for AI semantic matching...");

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
                                    tracing::info!("✅ Fallback successful: Ollama nomic-embed-code available for AI semantic matching");
                                    base.model_config.dimension =
                                        ollama_provider.embedding_dimension();
                                    base.ollama_provider = Some(ollama_provider);
                                }
                                Ok(false) => {
                                    tracing::error!("❌ Ollama fallback failed: nomic-embed-code model not found");
                                    tracing::error!("   Install with: ollama pull hf.co/nomic-ai/nomic-embed-code-GGUF:Q4_K_M");
                                    tracing::error!("   Falling back to random embeddings (no semantic AI matching)");
                                }
                                Err(e) => {
                                    tracing::error!("❌ Ollama fallback failed: {}", e);
                                    tracing::error!("   Falling back to random embeddings (no semantic AI matching)");
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
                        tracing::warn!("⚠️ nomic-embed-code model not found. Install with: ollama pull hf.co/nomic-ai/nomic-embed-code-GGUF:Q4_K_M");
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
        }
        base
    }

    /// Construct an EmbeddingGenerator from a CodeGraphConfig
    /// This enables TOML configuration file support in addition to environment variables
    pub async fn with_config(config: &codegraph_core::CodeGraphConfig) -> Self {
        let embedding_config = &config.embedding;
        #[allow(unused_mut)]
        let mut base = Self::new(ModelConfig {
            dimension: embedding_config.dimension,
            max_tokens: 512, // Default, could be added to config if needed
            model_name: embedding_config
                .model
                .clone()
                .unwrap_or_else(|| "auto".to_string()),
        });

        let provider = embedding_config.provider.to_lowercase();

        if provider == "ollama" {
            #[cfg(feature = "ollama")]
            {
                let ollama_config =
                    crate::ollama_embedding_provider::OllamaEmbeddingConfig::from(embedding_config);
                let ollama_provider =
                    crate::ollama_embedding_provider::OllamaEmbeddingProvider::new(ollama_config);

                match ollama_provider.check_availability().await {
                    Ok(true) => {
                        tracing::info!(
                            "✅ Ollama {} available for embeddings (from config)",
                            embedding_config
                                .model
                                .as_ref()
                                .unwrap_or(&"nomic-embed-code".to_string())
                        );
                        base.model_config.dimension = ollama_provider.embedding_dimension();
                        base.ollama_provider = Some(ollama_provider);
                    }
                    Ok(false) => {
                        tracing::warn!(
                            "⚠️ Ollama model {} not found. Install with: ollama pull <model>",
                            embedding_config
                                .model
                                .as_ref()
                                .unwrap_or(&"nomic-embed-code".to_string())
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
                let jina_config = crate::jina_provider::JinaConfig::from(embedding_config);
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
        }
        // Add other providers (ONNX, local, etc.) as needed following the same pattern

        base
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
            tracing::info!(
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
            tracing::info!(
                target: "codegraph_vector::embeddings",
                "Using Ollama nomic-embed-code for batch: {} items",
                nodes.len()
            );
            use crate::providers::EmbeddingProvider;
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

        #[cfg(any(feature = "local-embeddings", feature = "openai", feature = "onnx"))]
        if let Some(engine) = &self.advanced {
            let plan = self.build_plan_for_nodes(nodes);
            tracing::info!(
                target: "codegraph_vector::embeddings",
                "Advanced engine chunk plan: {} nodes -> {} chunks",
                plan.stats.total_nodes,
                plan.stats.total_chunks
            );
            let chunk_to_node = plan.chunk_to_node();
            let chunk_texts: Vec<String> =
                plan.chunks.into_iter().map(|chunk| chunk.text).collect();
            tracing::info!(
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
    pub async fn embed_texts_batched(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        // Use advanced engine's batching capabilities when available
        #[cfg(any(feature = "local-embeddings", feature = "openai", feature = "onnx"))]
        if let Some(engine) = &self.advanced {
            return engine.embed_texts_batched(texts).await;
        }

        #[cfg(feature = "jina")]
        if let Some(provider) = &self.jina_provider {
            return provider.embed_relationship_texts(texts).await;
        }

        // Fallback: process texts sequentially
        let mut embeddings = Vec::with_capacity(texts.len());
        for text in texts {
            let embedding = self.encode_text(text).await?;
            embeddings.push(embedding);
        }
        Ok(embeddings)
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
