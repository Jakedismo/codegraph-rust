use crate::llm_provider::*;
use crate::qwen_simple::{QwenClient, QwenConfig};
use anyhow::{anyhow, Context, Result};
use codegraph_core::config_manager::LLMConfig;
use std::sync::Arc;

#[cfg(feature = "anthropic")]
use crate::anthropic_provider::{AnthropicConfig, AnthropicProvider};

#[cfg(feature = "openai-llm")]
use crate::openai_llm_provider::{OpenAIConfig, OpenAIProvider};

#[cfg(feature = "openai-compatible")]
use crate::openai_compatible_provider::{OpenAICompatibleConfig, OpenAICompatibleProvider};

/// Factory for creating LLM providers based on configuration
pub struct LLMProviderFactory;

impl LLMProviderFactory {
    /// Create an LLM provider from configuration
    pub fn create_from_config(config: &LLMConfig) -> Result<Arc<dyn LLMProvider>> {
        if !config.enabled {
            return Err(anyhow!("LLM is not enabled in configuration"));
        }

        let provider_name = config.provider.to_lowercase();

        match provider_name.as_str() {
            "ollama" | "qwen" => Self::create_qwen_provider(config),
            "lmstudio" => Self::create_lmstudio_provider(config),
            #[cfg(feature = "anthropic")]
            "anthropic" => Self::create_anthropic_provider(config),
            #[cfg(feature = "openai-llm")]
            "openai" => Self::create_openai_provider(config),
            #[cfg(feature = "openai-compatible")]
            "openai-compatible" => Self::create_openai_compatible_provider(config),
            _ => Err(anyhow!(
                "Unsupported LLM provider: {}. Available providers: ollama, lmstudio{}{}{}",
                provider_name,
                if cfg!(feature = "anthropic") {
                    ", anthropic"
                } else {
                    ""
                },
                if cfg!(feature = "openai-llm") {
                    ", openai"
                } else {
                    ""
                },
                if cfg!(feature = "openai-compatible") {
                    ", openai-compatible"
                } else {
                    ""
                }
            )),
        }
    }

    /// Create a Qwen provider (Ollama-based)
    fn create_qwen_provider(config: &LLMConfig) -> Result<Arc<dyn LLMProvider>> {
        let qwen_config = QwenConfig {
            model_name: config
                .model
                .clone()
                .unwrap_or_else(|| "qwen2.5-coder:14b".to_string()),
            base_url: config.ollama_url.clone(),
            context_window: config.context_window,
            max_tokens: config.max_tokens,
            temperature: config.temperature,
            timeout: std::time::Duration::from_secs(config.timeout_secs),
        };

        Ok(Arc::new(QwenClient::new(qwen_config)))
    }

    /// Create a provider using LM Studio's OpenAI-compatible endpoint
    fn create_lmstudio_provider(config: &LLMConfig) -> Result<Arc<dyn LLMProvider>> {
        #[cfg(feature = "openai-compatible")]
        {
            let base_url = format!("{}/v1", config.lmstudio_url.trim_end_matches('/'));
            let compat_config = OpenAICompatibleConfig {
                base_url,
                model: config
                    .model
                    .clone()
                    .unwrap_or_else(|| "local-model".to_string()),
                context_window: config.context_window,
                timeout_secs: config.timeout_secs,
                max_retries: 3,
                api_key: None, // LM Studio doesn't require API key
                provider_name: "lmstudio".to_string(),
                use_responses_api: false,
            };

            Ok(Arc::new(OpenAICompatibleProvider::new(compat_config)?))
        }

        #[cfg(not(feature = "openai-compatible"))]
        {
            Err(anyhow!(
                "LM Studio provider requires 'openai-compatible' feature to be enabled. \
                 Please rebuild with --features openai-compatible or use 'ollama' provider instead."
            ))
        }
    }

    /// Create an Anthropic Claude provider
    #[cfg(feature = "anthropic")]
    fn create_anthropic_provider(config: &LLMConfig) -> Result<Arc<dyn LLMProvider>> {
        let api_key = config
            .anthropic_api_key
            .clone()
            .or_else(|| std::env::var("ANTHROPIC_API_KEY").ok())
            .ok_or_else(|| {
                anyhow!(
                    "Anthropic API key not found. Set 'anthropic_api_key' in config \
                     or ANTHROPIC_API_KEY environment variable"
                )
            })?;

        let anthropic_config = AnthropicConfig {
            api_key,
            model: config
                .model
                .clone()
                .unwrap_or_else(|| "claude-3-5-sonnet-20241022".to_string()),
            context_window: config.context_window,
            timeout_secs: config.timeout_secs,
            max_retries: 3,
        };

        Ok(Arc::new(AnthropicProvider::new(anthropic_config)?))
    }

    /// Create an OpenAI provider
    #[cfg(feature = "openai-llm")]
    fn create_openai_provider(config: &LLMConfig) -> Result<Arc<dyn LLMProvider>> {
        let api_key = config
            .openai_api_key
            .clone()
            .or_else(|| std::env::var("OPENAI_API_KEY").ok())
            .ok_or_else(|| {
                anyhow!(
                    "OpenAI API key not found. Set 'openai_api_key' in config \
                     or OPENAI_API_KEY environment variable"
                )
            })?;

        let openai_config = OpenAIConfig {
            api_key,
            base_url: "https://api.openai.com/v1".to_string(),
            model: config.model.clone().unwrap_or_else(|| "gpt-4o".to_string()),
            context_window: config.context_window,
            timeout_secs: config.timeout_secs,
            max_retries: 3,
            organization: std::env::var("OPENAI_ORG_ID").ok(),
        };

        Ok(Arc::new(OpenAIProvider::new(openai_config)?))
    }

    /// Create an OpenAI-compatible provider
    #[cfg(feature = "openai-compatible")]
    fn create_openai_compatible_provider(config: &LLMConfig) -> Result<Arc<dyn LLMProvider>> {
        let base_url = config.openai_compatible_url.clone().ok_or_else(|| {
            anyhow!("OpenAI-compatible base URL not found. Set 'openai_compatible_url' in config")
        })?;

        let compat_config = OpenAICompatibleConfig {
            base_url,
            model: config
                .model
                .clone()
                .ok_or_else(|| anyhow!("Model name is required for OpenAI-compatible provider"))?,
            context_window: config.context_window,
            timeout_secs: config.timeout_secs,
            max_retries: 3,
            api_key: config.openai_api_key.clone(),
            provider_name: "openai-compatible".to_string(),
            use_responses_api: false,
        };

        Ok(Arc::new(OpenAICompatibleProvider::new(compat_config)?))
    }

    /// Check if any LLM provider is available
    pub async fn check_availability(provider: &Arc<dyn LLMProvider>) -> bool {
        provider.is_available().await
    }

    /// Get a list of supported providers (based on enabled features)
    pub fn supported_providers() -> Vec<&'static str> {
        let mut providers = vec!["ollama", "qwen"];

        #[cfg(feature = "openai-compatible")]
        providers.push("lmstudio");

        #[cfg(feature = "openai-compatible")]
        providers.push("openai-compatible");

        #[cfg(feature = "anthropic")]
        providers.push("anthropic");

        #[cfg(feature = "openai-llm")]
        providers.push("openai");

        providers
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_supported_providers() {
        let providers = LLMProviderFactory::supported_providers();
        assert!(!providers.is_empty());
        assert!(providers.contains(&"ollama"));
    }

    #[test]
    fn test_qwen_provider_creation() {
        let config = LLMConfig {
            enabled: true,
            provider: "ollama".to_string(),
            model: Some("qwen2.5-coder:14b".to_string()),
            ollama_url: "http://localhost:11434".to_string(),
            context_window: 128_000,
            temperature: 0.1,
            max_tokens: 4096,
            timeout_secs: 120,
            ..Default::default()
        };

        let result = LLMProviderFactory::create_from_config(&config);
        assert!(result.is_ok());
    }

    #[test]
    fn test_disabled_llm() {
        let config = LLMConfig {
            enabled: false,
            ..Default::default()
        };

        let result = LLMProviderFactory::create_from_config(&config);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("LLM is not enabled"));
    }
}
