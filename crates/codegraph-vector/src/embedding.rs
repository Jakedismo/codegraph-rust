use codegraph_core::{CodeGraphError, CodeNode, Result};
#[cfg(any(feature = "local-embeddings", feature = "openai", feature = "onnx", feature = "ollama"))]
use std::sync::Arc;
#[cfg(any(feature = "local-embeddings", feature = "openai", feature = "onnx"))]
use crate::embeddings::generator::TextEmbeddingEngine;

pub struct EmbeddingGenerator {
    model_config: ModelConfig,
    #[cfg(any(feature = "local-embeddings", feature = "openai", feature = "onnx"))]
    pub(crate) advanced: Option<Arc<crate::embeddings::generator::AdvancedEmbeddingGenerator>>,
    #[cfg(feature = "ollama")]
    ollama_provider: Option<crate::ollama_embedding_provider::OllamaEmbeddingProvider>,
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
        Self {
            model_config: config,
            #[cfg(any(feature = "local-embeddings", feature = "openai", feature = "onnx"))]
            advanced: None,
            #[cfg(feature = "ollama")]
            ollama_provider: None,
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

    pub fn dimension(&self) -> usize {
        self.model_config.dimension
    }

    /// Construct an EmbeddingGenerator that optionally wraps the advanced engine based on env.
    /// If CODEGRAPH_EMBEDDING_PROVIDER=local, tries to initialize a local-first engine.
    pub async fn with_auto_from_env() -> Self {
        #[cfg(any(feature = "local-embeddings", feature = "openai", feature = "onnx", feature = "ollama"))]
        let mut base = Self::new(ModelConfig::default());
        #[cfg(not(any(feature = "local-embeddings", feature = "openai", feature = "onnx", feature = "ollama")))]
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
                use crate::embeddings::generator::{AdvancedEmbeddingGenerator, EmbeddingEngineConfig, OnnxConfigCompat};
                let mut cfg = EmbeddingEngineConfig::default();
                let model_repo = std::env::var("CODEGRAPH_LOCAL_MODEL").unwrap_or_default();
                tracing::info!("🚀 Initializing ONNX embedding provider with model: {}", model_repo);

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
                            let ollama_config = crate::ollama_embedding_provider::OllamaEmbeddingConfig::default();
                            let ollama_provider = crate::ollama_embedding_provider::OllamaEmbeddingProvider::new(ollama_config);

                            match ollama_provider.check_availability().await {
                                Ok(true) => {
                                    tracing::info!("✅ Fallback successful: Ollama nomic-embed-code available for AI semantic matching");
                                    base.ollama_provider = Some(ollama_provider);
                                    base.model_config.dimension = 768;
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
                            tracing::error!("   Ollama fallback not available (feature not enabled)");
                            tracing::error!("   Falling back to random embeddings (no semantic AI matching)");
                        }

                        tracing::warn!("⚠️ Without real embeddings, AI semantic matching will be 0% effective");
                    }
                }
            }
        } else if provider == "ollama" {
            #[cfg(feature = "ollama")]
            {
                // Create Ollama embedding provider
                let ollama_config = crate::ollama_embedding_provider::OllamaEmbeddingConfig::default();
                let ollama_provider = crate::ollama_embedding_provider::OllamaEmbeddingProvider::new(ollama_config);

                // Check if model is available
                match ollama_provider.check_availability().await {
                    Ok(true) => {
                        tracing::info!("✅ Ollama nomic-embed-code available for embeddings");
                        base.ollama_provider = Some(ollama_provider);
                        // Update dimension to match nomic-embed-code
                        base.model_config.dimension = 768;
                    }
                    Ok(false) => {
                        tracing::warn!("⚠️ nomic-embed-code model not found. Install with: ollama pull hf.co/nomic-ai/nomic-embed-code-GGUF:Q4_K_M");
                    }
                    Err(e) => {
                        tracing::error!("❌ Failed to connect to Ollama for embeddings: {}", e);
                    }
                }
            }
        }
        base
    }

    pub async fn generate_embedding(&self, node: &CodeNode) -> Result<Vec<f32>> {
        let text = self.prepare_text(node);
        self.encode_text(&text).await
    }

    pub async fn generate_embeddings(&self, nodes: &[CodeNode]) -> Result<Vec<Vec<f32>>> {
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
            // Use provider's batched path when available
            let texts: Vec<String> = nodes.iter().map(|n| self.prepare_text(n)).collect();
            tracing::info!(
                target: "codegraph_vector::embeddings",
                "Using advanced embedding engine for batch: {} items",
                texts.len()
            );
            let embs = engine.embed_many(&texts).await?;
            if embs.len() != texts.len() {
                return Err(CodeGraphError::Vector(format!(
                    "provider returned {} embeddings for {} inputs",
                    embs.len(),
                    texts.len()
                )));
            }
            return Ok(embs);
        }

        // Fallback: sequential deterministic embeddings
        let mut embeddings = Vec::with_capacity(nodes.len());
        for node in nodes {
            let embedding = self.generate_embedding(node).await?;
            embeddings.push(embedding);
        }
        Ok(embeddings)
    }

    /// Generate an embedding directly from free text. Useful for query embeddings.
    pub async fn generate_text_embedding(&self, text: &str) -> Result<Vec<f32>> {
        self.encode_text(text).await
    }

    fn prepare_text(&self, node: &CodeNode) -> String {
        let mut text = format!(
            "{} {} {}",
            node.language
                .as_ref()
                .map_or("unknown".to_string(), language_to_string),
            node.node_type
                .as_ref()
                .map_or("unknown".to_string(), node_type_to_string),
            node.name.as_str()
        );

        if let Some(content) = &node.content {
            text.push(' ');
            text.push_str(content);
        }

        if text.len() > self.model_config.max_tokens * 4 {
            let mut new_len = self.model_config.max_tokens * 4;
            if new_len > text.len() { new_len = text.len(); }
            while new_len > 0 && !text.is_char_boundary(new_len) {
                new_len -= 1;
            }
            text.truncate(new_len);
        }

        text
    }

    async fn encode_text(&self, text: &str) -> Result<Vec<f32>> {
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
        static FALLBACK_WARNING_SHOWN: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
        if !FALLBACK_WARNING_SHOWN.swap(true, std::sync::atomic::Ordering::Relaxed) {
            tracing::error!("🚨 CRITICAL: Falling back to random hash-based embeddings");
            tracing::error!("   This means AI semantic matching will be 0% effective");
            tracing::error!("   Resolution rates will remain at baseline (~60%) instead of target (85-90%)");
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

fn language_to_string(lang: &codegraph_core::Language) -> String {
    match lang {
        codegraph_core::Language::Rust => "rust".to_string(),
        codegraph_core::Language::TypeScript => "typescript".to_string(),
        codegraph_core::Language::JavaScript => "javascript".to_string(),
        codegraph_core::Language::Python => "python".to_string(),
        codegraph_core::Language::Go => "go".to_string(),
        codegraph_core::Language::Java => "java".to_string(),
        codegraph_core::Language::Cpp => "cpp".to_string(),
        // Revolutionary universal language support
        codegraph_core::Language::Swift => "swift".to_string(),
        codegraph_core::Language::Kotlin => "kotlin".to_string(),
        codegraph_core::Language::CSharp => "csharp".to_string(),
        codegraph_core::Language::Ruby => "ruby".to_string(),
        codegraph_core::Language::Php => "php".to_string(),
        codegraph_core::Language::Dart => "dart".to_string(),
        codegraph_core::Language::Other(name) => name.clone(),
    }
}

fn node_type_to_string(node_type: &codegraph_core::NodeType) -> String {
    match node_type {
        codegraph_core::NodeType::Function => "function".to_string(),
        codegraph_core::NodeType::Struct => "struct".to_string(),
        codegraph_core::NodeType::Enum => "enum".to_string(),
        codegraph_core::NodeType::Trait => "trait".to_string(),
        codegraph_core::NodeType::Module => "module".to_string(),
        codegraph_core::NodeType::Variable => "variable".to_string(),
        codegraph_core::NodeType::Import => "import".to_string(),
        codegraph_core::NodeType::Class => "class".to_string(),
        codegraph_core::NodeType::Interface => "interface".to_string(),
        codegraph_core::NodeType::Type => "type".to_string(),
        codegraph_core::NodeType::Other(name) => name.clone(),
    }
}
