use anyhow::Result;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum EmbeddingProvider {
    OpenAI,
    Local,
    Cohere,
    HuggingFace,
    Jina,
    Custom(String),
}

impl Default for EmbeddingProvider {
    fn default() -> Self {
        Self::Local
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct OpenAIEmbeddingConfig {
    pub model: String,
    pub api_key_env: String,
    #[serde(default = "OpenAIEmbeddingConfig::default_api_base")]
    pub api_base: String,
    #[serde(default = "OpenAIEmbeddingConfig::default_max_retries")]
    pub max_retries: u32,
    #[serde(default = "OpenAIEmbeddingConfig::default_timeout_secs")]
    pub timeout_secs: u64,
}

impl OpenAIEmbeddingConfig {
    fn default_api_base() -> String {
        "https://api.openai.com/v1".to_string()
    }

    fn default_max_retries() -> u32 {
        3
    }

    fn default_timeout_secs() -> u64 {
        30
    }
}

impl Default for OpenAIEmbeddingConfig {
    fn default() -> Self {
        Self {
            model: "text-embedding-3-small".to_string(),
            api_key_env: "OPENAI_API_KEY".to_string(),
            api_base: Self::default_api_base(),
            max_retries: Self::default_max_retries(),
            timeout_secs: Self::default_timeout_secs(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct LocalEmbeddingConfig {
    pub model_path: String,
    pub model_type: String,
    #[serde(default = "LocalEmbeddingConfig::default_device")]
    pub device: String,
    #[serde(default = "LocalEmbeddingConfig::default_batch_size")]
    pub batch_size: usize,
    #[serde(default = "LocalEmbeddingConfig::default_max_sequence_length")]
    pub max_sequence_length: usize,
}

impl LocalEmbeddingConfig {
    fn default_device() -> String {
        "cpu".to_string()
    }

    fn default_batch_size() -> usize {
        32
    }

    fn default_max_sequence_length() -> usize {
        512
    }
}

impl Default for LocalEmbeddingConfig {
    fn default() -> Self {
        Self {
            model_path: "./models/all-MiniLM-L6-v2".to_string(),
            model_type: "sentence-transformers".to_string(),
            device: Self::default_device(),
            batch_size: Self::default_batch_size(),
            max_sequence_length: Self::default_max_sequence_length(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct JinaEmbeddingConfig {
    pub model: String,
    pub api_key_env: String,
    #[serde(default = "JinaEmbeddingConfig::default_api_base")]
    pub api_base: String,
    #[serde(default = "JinaEmbeddingConfig::default_max_retries")]
    pub max_retries: u32,
    #[serde(default = "JinaEmbeddingConfig::default_timeout_secs")]
    pub timeout_secs: u64,
    #[serde(default = "JinaEmbeddingConfig::default_task")]
    pub task: String,
    #[serde(default = "JinaEmbeddingConfig::default_late_chunking")]
    pub late_chunking: bool,
    #[serde(default = "JinaEmbeddingConfig::default_enable_reranking")]
    pub enable_reranking: bool,
    #[serde(default = "JinaEmbeddingConfig::default_reranking_model")]
    pub reranking_model: String,
    #[serde(default = "JinaEmbeddingConfig::default_reranking_top_n")]
    pub reranking_top_n: usize,
}

impl JinaEmbeddingConfig {
    fn default_api_base() -> String {
        "https://api.jina.ai/v1".to_string()
    }

    fn default_max_retries() -> u32 {
        3
    }

    fn default_timeout_secs() -> u64 {
        30
    }

    fn default_task() -> String {
        "code.query".to_string()
    }

    fn default_late_chunking() -> bool {
        true
    }

    fn default_enable_reranking() -> bool {
        true
    }

    fn default_reranking_model() -> String {
        "jina-reranker-v3".to_string()
    }

    fn default_reranking_top_n() -> usize {
        10
    }
}

impl Default for JinaEmbeddingConfig {
    fn default() -> Self {
        Self {
            model: "jina-embeddings-v4".to_string(),
            api_key_env: "JINA_API_KEY".to_string(),
            api_base: Self::default_api_base(),
            max_retries: Self::default_max_retries(),
            timeout_secs: Self::default_timeout_secs(),
            task: Self::default_task(),
            late_chunking: Self::default_late_chunking(),
            enable_reranking: Self::default_enable_reranking(),
            reranking_model: Self::default_reranking_model(),
            reranking_top_n: Self::default_reranking_top_n(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct EmbeddingModelConfig {
    #[serde(default)]
    pub provider: EmbeddingProvider,

    #[serde(default = "EmbeddingModelConfig::default_dimension")]
    pub dimension: usize,

    #[serde(default)]
    pub openai: Option<OpenAIEmbeddingConfig>,

    #[serde(default)]
    pub local: Option<LocalEmbeddingConfig>,

    #[serde(default)]
    pub jina: Option<JinaEmbeddingConfig>,

    #[serde(default)]
    pub custom_config: HashMap<String, serde_json::Value>,

    #[serde(default = "EmbeddingModelConfig::default_cache_enabled")]
    pub cache_enabled: bool,

    #[serde(default = "EmbeddingModelConfig::default_cache_ttl_secs")]
    pub cache_ttl_secs: u64,

    #[serde(default = "EmbeddingModelConfig::default_normalize_embeddings")]
    pub normalize_embeddings: bool,
}

impl EmbeddingModelConfig {
    fn default_dimension() -> usize {
        768
    }

    fn default_cache_enabled() -> bool {
        true
    }

    fn default_cache_ttl_secs() -> u64 {
        3600
    }

    fn default_normalize_embeddings() -> bool {
        true
    }

    pub fn validate(&self) -> Result<()> {
        anyhow::ensure!(
            self.dimension > 0 && self.dimension <= 8192,
            "Embedding dimension must be between 1 and 8192"
        );

        match &self.provider {
            EmbeddingProvider::OpenAI => {
                anyhow::ensure!(
                    self.openai.is_some(),
                    "OpenAI configuration required when using OpenAI provider"
                );
                if let Some(config) = &self.openai {
                    anyhow::ensure!(
                        !config.model.is_empty(),
                        "OpenAI model name cannot be empty"
                    );
                    anyhow::ensure!(
                        !config.api_key_env.is_empty(),
                        "OpenAI API key environment variable name cannot be empty"
                    );
                }
            }
            EmbeddingProvider::Local => {
                anyhow::ensure!(
                    self.local.is_some(),
                    "Local configuration required when using Local provider"
                );
                if let Some(config) = &self.local {
                    anyhow::ensure!(
                        !config.model_path.is_empty(),
                        "Local model path cannot be empty"
                    );
                    anyhow::ensure!(config.batch_size > 0, "Batch size must be greater than 0");
                }
            }
            EmbeddingProvider::Jina => {
                anyhow::ensure!(
                    self.jina.is_some(),
                    "Jina configuration required when using Jina provider"
                );
                if let Some(config) = &self.jina {
                    anyhow::ensure!(!config.model.is_empty(), "Jina model name cannot be empty");
                    anyhow::ensure!(
                        !config.api_key_env.is_empty(),
                        "Jina API key environment variable name cannot be empty"
                    );
                }
            }
            _ => {}
        }

        Ok(())
    }

    pub fn for_openai(model: &str, dimension: usize) -> Self {
        Self {
            provider: EmbeddingProvider::OpenAI,
            dimension,
            openai: Some(OpenAIEmbeddingConfig {
                model: model.to_string(),
                ..Default::default()
            }),
            local: None,
            jina: None,
            custom_config: HashMap::new(),
            cache_enabled: Self::default_cache_enabled(),
            cache_ttl_secs: Self::default_cache_ttl_secs(),
            normalize_embeddings: Self::default_normalize_embeddings(),
        }
    }

    pub fn for_local(model_path: &str, dimension: usize) -> Self {
        Self {
            provider: EmbeddingProvider::Local,
            dimension,
            openai: None,
            local: Some(LocalEmbeddingConfig {
                model_path: model_path.to_string(),
                ..Default::default()
            }),
            jina: None,
            custom_config: HashMap::new(),
            cache_enabled: Self::default_cache_enabled(),
            cache_ttl_secs: Self::default_cache_ttl_secs(),
            normalize_embeddings: Self::default_normalize_embeddings(),
        }
    }

    pub fn for_jina(model: &str, dimension: usize) -> Self {
        Self {
            provider: EmbeddingProvider::Jina,
            dimension,
            openai: None,
            local: None,
            jina: Some(JinaEmbeddingConfig {
                model: model.to_string(),
                ..Default::default()
            }),
            custom_config: HashMap::new(),
            cache_enabled: Self::default_cache_enabled(),
            cache_ttl_secs: Self::default_cache_ttl_secs(),
            normalize_embeddings: Self::default_normalize_embeddings(),
        }
    }
}

impl Default for EmbeddingModelConfig {
    fn default() -> Self {
        Self {
            provider: EmbeddingProvider::default(),
            dimension: Self::default_dimension(),
            openai: None,
            local: Some(LocalEmbeddingConfig::default()),
            jina: None,
            custom_config: HashMap::new(),
            cache_enabled: Self::default_cache_enabled(),
            cache_ttl_secs: Self::default_cache_ttl_secs(),
            normalize_embeddings: Self::default_normalize_embeddings(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct EmbeddingPreset {
    pub name: String,
    pub description: String,
    pub config: EmbeddingModelConfig,
}

impl EmbeddingPreset {
    pub fn openai_small() -> Self {
        Self {
            name: "openai-small".to_string(),
            description: "OpenAI text-embedding-3-small model (fast, cost-effective)".to_string(),
            config: EmbeddingModelConfig::for_openai("text-embedding-3-small", 1536),
        }
    }

    pub fn openai_large() -> Self {
        Self {
            name: "openai-large".to_string(),
            description: "OpenAI text-embedding-3-large model (high quality)".to_string(),
            config: EmbeddingModelConfig::for_openai("text-embedding-3-large", 3072),
        }
    }

    pub fn openai_ada() -> Self {
        Self {
            name: "openai-ada".to_string(),
            description: "OpenAI text-embedding-ada-002 model (legacy, stable)".to_string(),
            config: EmbeddingModelConfig::for_openai("text-embedding-ada-002", 1536),
        }
    }

    pub fn local_minilm() -> Self {
        Self {
            name: "local-minilm".to_string(),
            description: "Local all-MiniLM-L6-v2 model (fast, lightweight)".to_string(),
            config: EmbeddingModelConfig::for_local("./models/all-MiniLM-L6-v2", 384),
        }
    }

    pub fn local_mpnet() -> Self {
        Self {
            name: "local-mpnet".to_string(),
            description: "Local all-mpnet-base-v2 model (high quality)".to_string(),
            config: EmbeddingModelConfig::for_local("./models/all-mpnet-base-v2", 768),
        }
    }

    pub fn jina_v4() -> Self {
        Self {
            name: "jina-v4".to_string(),
            description: "Jina embeddings-v4 model (multimodal, code-optimized with reranking)"
                .to_string(),
            config: EmbeddingModelConfig::for_jina("jina-embeddings-v4", 1024),
        }
    }

    pub fn all_presets() -> Vec<Self> {
        vec![
            Self::openai_small(),
            Self::openai_large(),
            Self::openai_ada(),
            Self::local_minilm(),
            Self::local_mpnet(),
            Self::jina_v4(),
        ]
    }

    pub fn get_by_name(name: &str) -> Option<Self> {
        Self::all_presets().into_iter().find(|p| p.name == name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_embedding_config() {
        let config = EmbeddingModelConfig::default();
        assert!(matches!(config.provider, EmbeddingProvider::Local));
        assert_eq!(config.dimension, 768);
        assert!(config.local.is_some());
        assert!(config.cache_enabled);
    }

    #[test]
    fn test_openai_config_creation() {
        let config = EmbeddingModelConfig::for_openai("text-embedding-3-small", 1536);
        assert!(matches!(config.provider, EmbeddingProvider::OpenAI));
        assert_eq!(config.dimension, 1536);
        assert!(config.openai.is_some());
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_local_config_creation() {
        let config = EmbeddingModelConfig::for_local("/path/to/model", 384);
        assert!(matches!(config.provider, EmbeddingProvider::Local));
        assert_eq!(config.dimension, 384);
        assert!(config.local.is_some());
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_config_validation() {
        let mut config = EmbeddingModelConfig::default();
        config.dimension = 0;
        assert!(config.validate().is_err());

        config.dimension = 10000;
        assert!(config.validate().is_err());

        config.dimension = 768;
        config.provider = EmbeddingProvider::OpenAI;
        config.openai = None;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_presets() {
        let presets = EmbeddingPreset::all_presets();
        assert!(!presets.is_empty());

        let openai_small = EmbeddingPreset::get_by_name("openai-small");
        assert!(openai_small.is_some());

        let preset = openai_small.unwrap();
        assert_eq!(preset.name, "openai-small");
        assert!(matches!(preset.config.provider, EmbeddingProvider::OpenAI));
    }
}
