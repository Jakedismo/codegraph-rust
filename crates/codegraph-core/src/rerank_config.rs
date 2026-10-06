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
#[serde(default)]
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

    /// Supply the selected provider's defaults without replacing explicit settings.
    pub fn with_provider_defaults(mut self) -> Self {
        match self.provider {
            RerankProvider::Jina => {
                self.jina.get_or_insert_with(JinaRerankConfig::default);
            }
            RerankProvider::Ollama => {
                self.ollama.get_or_insert_with(OllamaRerankConfig::default);
            }
            RerankProvider::None => {}
        }
        self
    }

    /// Resolve reranker environment overrides for both config and factory callers.
    /// Explicit CODEGRAPH_RERANK_PROVIDER overrides the legacy Jina toggle; the
    /// master CODEGRAPH_ENABLE_RERANKING=false switch disables every provider.
    pub fn with_env_overrides(mut self) -> Self {
        let env_bool = |name| {
            std::env::var(name).ok().and_then(|value| {
                match value.trim().to_ascii_lowercase().as_str() {
                    "true" | "1" => Some(true),
                    "false" | "0" => Some(false),
                    _ => {
                        tracing::warn!("Invalid boolean for {name}; keeping configured value");
                        None
                    }
                }
            })
        };

        match env_bool("JINA_ENABLE_RERANKING") {
            Some(true) => self.provider = RerankProvider::Jina,
            Some(false) if self.provider == RerankProvider::Jina => {
                self.provider = RerankProvider::None;
            }
            _ => {}
        }
        let enabled = env_bool("CODEGRAPH_ENABLE_RERANKING");
        if enabled == Some(true) && self.provider == RerankProvider::None {
            self.provider = RerankProvider::Jina;
        }
        if let Ok(provider) = std::env::var("CODEGRAPH_RERANK_PROVIDER") {
            match provider.trim().to_ascii_lowercase().as_str() {
                "jina" => self.provider = RerankProvider::Jina,
                "ollama" => self.provider = RerankProvider::Ollama,
                "none" | "" => self.provider = RerankProvider::None,
                _ => tracing::warn!(
                    "Unknown reranking provider: {provider}; keeping configured value"
                ),
            }
        }
        if enabled == Some(false) {
            self.provider = RerankProvider::None;
        }
        self = self.with_provider_defaults();

        if let Ok(model) = std::env::var("JINA_RERANKING_MODEL") {
            self.jina
                .get_or_insert_with(JinaRerankConfig::default)
                .model = model;
        }
        if let Some(jina) = self.jina.as_mut() {
            if let Ok(base) = std::env::var("JINA_API_BASE") {
                jina.api_base = base;
            }
        }
        if let Some(ollama) = self.ollama.as_mut() {
            if let Ok(model) = std::env::var("CODEGRAPH_OLLAMA_RERANK_MODEL")
                .or_else(|_| std::env::var("OLLAMA_RERANK_MODEL"))
            {
                ollama.model = model;
            }
            if let Ok(base) =
                std::env::var("CODEGRAPH_OLLAMA_URL").or_else(|_| std::env::var("OLLAMA_URL"))
            {
                ollama.api_base = base;
            }
        }
        for name in ["JINA_RERANKING_TOP_N", "CODEGRAPH_RERANKING_CANDIDATES"] {
            if let Ok(value) = std::env::var(name) {
                if let Ok(n) = value.parse() {
                    self.top_n = n;
                }
            }
        }
        self
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

    fn isolated_env(name: &str, overrides: &[(&str, Option<&str>)]) -> bool {
        let mut variables = vec![
            ("CODEGRAPH_RERANK_PROVIDER", None),
            ("CODEGRAPH_ENABLE_RERANKING", None),
            ("JINA_ENABLE_RERANKING", None),
            ("JINA_RERANKING_MODEL", None),
            ("JINA_API_BASE", None),
            ("JINA_RERANKING_TOP_N", None),
            ("CODEGRAPH_RERANKING_CANDIDATES", None),
            ("CODEGRAPH_OLLAMA_RERANK_MODEL", None),
            ("OLLAMA_RERANK_MODEL", None),
            ("CODEGRAPH_OLLAMA_URL", None),
            ("OLLAMA_URL", None),
        ];
        variables.extend_from_slice(overrides);
        test_env::run(name, &variables)
    }

    #[test]
    fn minimal_toml_supplies_only_the_selected_provider() {
        let config: RerankConfig = toml::from_str("provider = 'jina'").unwrap();
        let config = config.with_provider_defaults();
        assert!(config.validate().is_ok());
        assert_eq!(config.jina.unwrap().model, "jina-reranker-v3");
        assert!(config.ollama.is_none());

        let config: RerankConfig = toml::from_str("provider = 'ollama'").unwrap();
        let config = config.with_provider_defaults();
        assert!(config.validate().is_ok());
        assert!(config.ollama.is_some());
        assert!(config.jina.is_none());
    }

    #[test]
    fn partial_jina_toml_defaults_missing_fields_and_preserves_explicit_values() {
        let config: RerankConfig = toml::from_str(
            "provider = 'jina'\ntop_n = 7\n[jina]\nmodel = 'custom-model'\n\
             api_base = 'http://fixture/v1'\ntimeout_secs = 9\nmax_retries = 0",
        )
        .unwrap();
        let config = config.with_provider_defaults();
        assert!(config.validate().is_ok());
        assert_eq!(config.top_n, 7);
        let jina = config.jina.unwrap();
        assert_eq!(jina.model, "custom-model");
        assert_eq!(jina.api_key_env, "JINA_API_KEY");
        assert_eq!(jina.api_base, "http://fixture/v1");
        assert_eq!(jina.timeout_secs, 9);
        assert_eq!(jina.max_retries, 0);
    }

    #[test]
    fn jina_environment_overrides_toml_without_replacing_credentials_or_limits() {
        if !isolated_env(
            concat!(
                module_path!(),
                "::jina_environment_overrides_toml_without_replacing_credentials_or_limits"
            ),
            &[
                ("JINA_RERANKING_MODEL", Some("env-model")),
                ("JINA_API_BASE", Some("http://env/v1")),
                ("JINA_RERANKING_TOP_N", Some("3")),
                ("CODEGRAPH_RERANKING_CANDIDATES", Some("5")),
            ],
        ) {
            return;
        }
        let mut config = RerankConfig::for_jina("toml-model");
        let jina = config.jina.as_mut().unwrap();
        jina.api_key_env = "CUSTOM_JINA_KEY".into();
        jina.timeout_secs = 8;
        jina.max_retries = 0;
        let config = config.with_env_overrides();
        assert_eq!(config.top_n, 5);
        let jina = config.jina.unwrap();
        assert_eq!(jina.model, "env-model");
        assert_eq!(jina.api_base, "http://env/v1");
        assert_eq!(jina.api_key_env, "CUSTOM_JINA_KEY");
        assert_eq!(jina.timeout_secs, 8);
        assert_eq!(jina.max_retries, 0);
    }

    #[test]
    fn ollama_environment_overrides_explicit_toml_settings() {
        if !isolated_env(
            concat!(
                module_path!(),
                "::ollama_environment_overrides_explicit_toml_settings"
            ),
            &[
                ("CODEGRAPH_OLLAMA_RERANK_MODEL", Some("canonical-model")),
                ("OLLAMA_RERANK_MODEL", Some("legacy-model")),
                ("CODEGRAPH_OLLAMA_URL", Some("http://canonical:11434")),
                ("OLLAMA_URL", Some("http://legacy:11434")),
            ],
        ) {
            return;
        }
        let mut config = RerankConfig::for_ollama("toml-model");
        config.ollama.as_mut().unwrap().api_base = "http://toml:11434".into();
        let ollama = config.with_env_overrides().ollama.unwrap();
        assert_eq!(ollama.model, "canonical-model");
        assert_eq!(ollama.api_base, "http://canonical:11434");
    }

    #[test]
    fn master_disable_wins_over_an_explicit_provider() {
        if !isolated_env(
            concat!(
                module_path!(),
                "::master_disable_wins_over_an_explicit_provider"
            ),
            &[
                ("CODEGRAPH_ENABLE_RERANKING", Some("false")),
                ("CODEGRAPH_RERANK_PROVIDER", Some("jina")),
                ("JINA_ENABLE_RERANKING", Some("true")),
            ],
        ) {
            return;
        }
        let config = RerankConfig::default().with_env_overrides();
        assert_eq!(config.provider, RerankProvider::None);
        assert!(config.jina.is_none());
    }

    #[test]
    fn explicit_none_wins_over_enable_flags() {
        if !isolated_env(
            concat!(module_path!(), "::explicit_none_wins_over_enable_flags"),
            &[
                ("CODEGRAPH_RERANK_PROVIDER", Some("none")),
                ("CODEGRAPH_ENABLE_RERANKING", Some("true")),
                ("JINA_ENABLE_RERANKING", Some("true")),
            ],
        ) {
            return;
        }
        let config = RerankConfig::default().with_env_overrides();
        assert_eq!(config.provider, RerankProvider::None);
        assert!(config.jina.is_none());
    }

    #[test]
    fn generic_enable_defaults_to_jina() {
        if !isolated_env(
            concat!(module_path!(), "::generic_enable_defaults_to_jina"),
            &[("CODEGRAPH_ENABLE_RERANKING", Some("true"))],
        ) {
            return;
        }
        let config = RerankConfig::default().with_env_overrides();
        assert_eq!(config.provider, RerankProvider::Jina);
        assert!(config.validate().is_ok());
    }

    #[test]
    fn legacy_jina_disable_does_not_disable_ollama() {
        if !isolated_env(
            concat!(
                module_path!(),
                "::legacy_jina_disable_does_not_disable_ollama"
            ),
            &[("JINA_ENABLE_RERANKING", Some("false"))],
        ) {
            return;
        }
        let config = RerankConfig::for_jina("custom").with_env_overrides();
        assert_eq!(config.provider, RerankProvider::None);
        let config = RerankConfig::for_ollama("custom").with_env_overrides();
        assert_eq!(config.provider, RerankProvider::Ollama);
        assert_eq!(config.ollama.unwrap().model, "custom");
    }

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

#[cfg(test)]
mod test_env {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/support/env.rs"
    ));
}
