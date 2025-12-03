// ABOUTME: Reranking provider configuration for CodeGraph
// ABOUTME: Supports Jina API-based reranking and Ollama chat-based reranking
use anyhow::Result;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum RerankProvider {
    /// Jina AI reranking API (jina-reranker-v3)
    Jina,
    /// Ollama chat-based reranking (e.g., Qwen3-Reranker)
    Ollama,
    /// No reranking (use HNSW scores directly)
    None,
}

impl Default for RerankProvider {
    fn default() -> Self {
        Self::None
    }
}

impl std::fmt::Display for RerankProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Jina => write!(f, "jina"),
            Self::Ollama => write!(f, "ollama"),
            Self::None => write!(f, "none"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct JinaRerankConfig {
    /// Jina reranking model
    pub model: String,
    /// Jina API key environment variable name
    pub api_key_env: String,
    /// Jina API base URL
    #[serde(default = "JinaRerankConfig::default_api_base")]
    pub api_base: String,
    /// Maximum number of retries for API requests
    #[serde(default = "JinaRerankConfig::default_max_retries")]
    pub max_retries: u32,
    /// Timeout for API requests (seconds)
    #[serde(default = "JinaRerankConfig::default_timeout_secs")]
    pub timeout_secs: u64,
}

impl JinaRerankConfig {
    fn default_api_base() -> String {
        "https://api.jina.ai/v1".to_string()
    }

    fn default_max_retries() -> u32 {
        3
    }

    fn default_timeout_secs() -> u64 {
        30
    }
}

impl Default for JinaRerankConfig {
    fn default() -> Self {
        Self {
            model: "jina-reranker-v3".to_string(),
            api_key_env: "JINA_API_KEY".to_string(),
            api_base: Self::default_api_base(),
            max_retries: Self::default_max_retries(),
            timeout_secs: Self::default_timeout_secs(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct OllamaRerankConfig {
    /// Ollama reranking model (e.g., "dengcao/Qwen3-Reranker-8B:Q3_K_M")
    /// Defaults to CODEGRAPH_OLLAMA_RERANK_MODEL or OLLAMA_RERANK_MODEL env var
    #[serde(default = "OllamaRerankConfig::default_model")]
    pub model: String,
    /// Ollama API base URL
    /// Defaults to CODEGRAPH_OLLAMA_URL or OLLAMA_URL env var
    #[serde(default = "OllamaRerankConfig::default_api_base")]
    pub api_base: String,
    /// Maximum number of retries for API requests
    #[serde(default = "OllamaRerankConfig::default_max_retries")]
    pub max_retries: u32,
    /// Timeout for API requests (seconds)
    #[serde(default = "OllamaRerankConfig::default_timeout_secs")]
    pub timeout_secs: u64,
    /// Temperature for chat completion (0.0 = deterministic)
    #[serde(default = "OllamaRerankConfig::default_temperature")]
    pub temperature: f32,
}

impl OllamaRerankConfig {
    fn default_model() -> String {
        std::env::var("CODEGRAPH_OLLAMA_RERANK_MODEL")
            .or_else(|_| std::env::var("OLLAMA_RERANK_MODEL"))
            .unwrap_or_else(|_| "dengcao/Qwen3-Reranker-8B:Q3_K_M".to_string())
    }

    fn default_api_base() -> String {
        std::env::var("CODEGRAPH_OLLAMA_URL")
            .or_else(|_| std::env::var("OLLAMA_URL"))
            .unwrap_or_else(|_| "http://localhost:11434".to_string())
    }

    fn default_max_retries() -> u32 {
        3
    }

    fn default_timeout_secs() -> u64 {
        30
    }

    fn default_temperature() -> f32 {
        0.0 // Deterministic for consistent reranking
    }
}

impl Default for OllamaRerankConfig {
    fn default() -> Self {
        Self {
            model: Self::default_model(),
            api_base: Self::default_api_base(),
            max_retries: Self::default_max_retries(),
            timeout_secs: Self::default_timeout_secs(),
            temperature: Self::default_temperature(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RerankConfig {
    /// Reranking provider
    #[serde(default)]
    pub provider: RerankProvider,

    /// Number of top results to return after reranking
    #[serde(default = "RerankConfig::default_top_n")]
    pub top_n: usize,

    /// Jina-specific configuration
    #[serde(default)]
    pub jina: Option<JinaRerankConfig>,

    /// Ollama-specific configuration
    #[serde(default)]
    pub ollama: Option<OllamaRerankConfig>,
}

impl RerankConfig {
    fn default_top_n() -> usize {
        10
    }

    pub fn validate(&self) -> Result<()> {
        match &self.provider {
            RerankProvider::Jina => {
                anyhow::ensure!(
                    self.jina.is_some(),
                    "Jina configuration required when using Jina reranking provider"
                );
                if let Some(config) = &self.jina {
                    anyhow::ensure!(
                        !config.model.is_empty(),
                        "Jina reranking model name cannot be empty"
                    );
                    anyhow::ensure!(
                        !config.api_key_env.is_empty(),
                        "Jina API key environment variable name cannot be empty"
                    );
                }
            }
            RerankProvider::Ollama => {
                anyhow::ensure!(
                    self.ollama.is_some(),
                    "Ollama configuration required when using Ollama reranking provider"
                );
                if let Some(config) = &self.ollama {
                    anyhow::ensure!(
                        !config.model.is_empty(),
                        "Ollama reranking model name cannot be empty"
                    );
                }
            }
            RerankProvider::None => {
                // No validation needed for None
            }
        }

        Ok(())
    }

    pub fn for_jina(model: &str) -> Self {
        Self {
            provider: RerankProvider::Jina,
            top_n: Self::default_top_n(),
            jina: Some(JinaRerankConfig {
                model: model.to_string(),
                ..Default::default()
            }),
            ollama: None,
        }
    }

    pub fn for_ollama(model: &str) -> Self {
        Self {
            provider: RerankProvider::Ollama,
            top_n: Self::default_top_n(),
            jina: None,
            ollama: Some(OllamaRerankConfig {
                model: model.to_string(),
                ..Default::default()
            }),
        }
    }
}

impl Default for RerankConfig {
    fn default() -> Self {
        Self {
            provider: RerankProvider::default(),
            top_n: Self::default_top_n(),
            jina: None,
            ollama: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_rerank_config() {
        let config = RerankConfig::default();
        assert_eq!(config.provider, RerankProvider::None);
        assert_eq!(config.top_n, 10);
        assert!(config.jina.is_none());
        assert!(config.ollama.is_none());
    }

    #[test]
    fn test_jina_config_creation() {
        let config = RerankConfig::for_jina("jina-reranker-v3");
        assert_eq!(config.provider, RerankProvider::Jina);
        assert!(config.jina.is_some());
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_ollama_config_creation() {
        let config = RerankConfig::for_ollama("dengcao/Qwen3-Reranker-8B:Q3_K_M");
        assert_eq!(config.provider, RerankProvider::Ollama);
        assert!(config.ollama.is_some());
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_config_validation() {
        let mut config = RerankConfig::default();
        config.provider = RerankProvider::Jina;
        config.jina = None;
        assert!(config.validate().is_err());

        config.provider = RerankProvider::Ollama;
        config.ollama = None;
        assert!(config.validate().is_err());
    }
}
