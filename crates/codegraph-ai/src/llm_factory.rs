// ABOUTME: Builds LLM provider instances from user configuration.
// ABOUTME: Centralizes provider selection and feature-gated availability rules.
use crate::llm_provider::*;
use crate::qwen_simple::{QwenClient, QwenConfig};
use anyhow::{anyhow, Result};
use codegraph_core::config_manager::LLMConfig;
use std::sync::Arc;

#[cfg(feature = "anthropic")]
use crate::anthropic_provider::{AnthropicConfig, AnthropicProvider};

#[cfg(feature = "openai-llm")]
use crate::openai_llm_provider::{OpenAIConfig, OpenAIProvider};

#[cfg(feature = "openai-compatible")]
use crate::openai_compatible_provider::{OpenAICompatibleConfig, OpenAICompatibleProvider};

#[cfg(any(feature = "openai-compatible", feature = "anthropic"))]
const MINIMAX_MODELS: [&str; 2] = ["MiniMax-M3", "MiniMax-M2.7"];

#[cfg(any(feature = "openai-compatible", feature = "anthropic"))]
fn minimax_model(config: &LLMConfig) -> String {
    config
        .model
        .clone()
        .unwrap_or_else(|| MINIMAX_MODELS[0].to_string())
}

#[cfg(any(feature = "openai-compatible", feature = "anthropic"))]
fn minimax_is_cn_region(config: &LLMConfig) -> bool {
    matches!(
        config.minimax_region.to_ascii_lowercase().as_str(),
        "cn_zh" | "cn" | "china"
    )
}

#[cfg(feature = "openai-compatible")]
fn minimax_openai_base_url(config: &LLMConfig) -> String {
    if minimax_is_cn_region(config) {
        config.minimax_cn_openai_base_url.clone()
    } else {
        config.minimax_openai_base_url.clone()
    }
}

#[cfg(feature = "anthropic")]
fn minimax_anthropic_base_url(config: &LLMConfig) -> String {
    if minimax_is_cn_region(config) {
        config.minimax_cn_anthropic_base_url.clone()
    } else {
        config.minimax_anthropic_base_url.clone()
    }
}

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
            "ollama" => Self::create_ollama_openai_provider(config),
            "qwen" => Self::create_qwen_provider(config),
            "lmstudio" => Self::create_lmstudio_provider(config),
            #[cfg(feature = "anthropic")]
            "anthropic" => Self::create_anthropic_provider(config),
            #[cfg(feature = "anthropic")]
            "minimax-anthropic" => Self::create_minimax_anthropic_provider(config),
            #[cfg(feature = "openai-llm")]
            "openai" => Self::create_openai_provider(config),
            #[cfg(feature = "openai-llm")]
            "xai" => Self::create_xai_provider(config),
            #[cfg(feature = "openai-compatible")]
            "openai-compatible" => Self::create_openai_compatible_provider(config),
            #[cfg(feature = "openai-compatible")]
            "minimax" | "minimax-openai" => Self::create_minimax_openai_provider(config),
            _ => Err(anyhow!(
                "Unsupported LLM provider: {}. Available providers: ollama, lmstudio{}{}{}",
                provider_name,
                if cfg!(feature = "anthropic") {
                    ", anthropic, minimax-anthropic"
                } else {
                    ""
                },
                if cfg!(feature = "openai-llm") {
                    ", openai, xai"
                } else {
                    ""
                },
                if cfg!(feature = "openai-compatible") {
                    ", openai-compatible, minimax"
                } else {
                    ""
                }
            )),
        }
    }

    /// Create an Ollama provider using the OpenAI-compatible endpoint
    fn create_ollama_openai_provider(config: &LLMConfig) -> Result<Arc<dyn LLMProvider>> {
        #[cfg(feature = "openai-compatible")]
        {
            let base_url = format!("{}/v1", config.ollama_url.trim_end_matches('/'));
            let model = config
                .model
                .clone()
                .unwrap_or_else(|| "qwen2.5-coder:14b".to_string());

            // Use Ollama defaults (use_responses_api: false) since Ollama doesn't support Responses API
            let mut compat_config = OpenAICompatibleConfig::ollama(model);
            compat_config.base_url = base_url;
            compat_config.context_window = config.context_window;
            compat_config.timeout_secs = config.timeout_secs;

            Ok(Arc::new(OpenAICompatibleProvider::new(compat_config)?))
        }

        #[cfg(not(feature = "openai-compatible"))]
        {
            let _ = config;
            Err(anyhow!(
                "Ollama provider now relies on the 'openai-compatible' feature. \
                 Rebuild with --features codegraph-ai/openai-compatible"
            ))
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
                use_responses_api: !config.use_completions_api, // Default to Responses API unless completions API requested
            };

            Ok(Arc::new(OpenAICompatibleProvider::new(compat_config)?))
        }

        #[cfg(not(feature = "openai-compatible"))]
        {
            let _ = config;
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
            base_url: config.anthropic_base_url.clone(),
            model: config.model.clone().unwrap_or_else(|| "claude".to_string()),
            context_window: config.context_window,
            timeout_secs: config.timeout_secs,
            max_retries: 3,
        };

        Ok(Arc::new(AnthropicProvider::new(anthropic_config)?))
    }

    /// Create a MiniMax provider using its OpenAI-compatible endpoint.
    #[cfg(feature = "openai-compatible")]
    fn create_minimax_openai_provider(config: &LLMConfig) -> Result<Arc<dyn LLMProvider>> {
        let api_key = config
            .minimax_api_key
            .clone()
            .or_else(|| std::env::var("MINIMAX_API_KEY").ok())
            .ok_or_else(|| {
                anyhow!(
                    "MiniMax API key not found. Set 'minimax_api_key' in config or MINIMAX_API_KEY environment variable"
                )
            })?;

        let compat_config = OpenAICompatibleConfig {
            base_url: minimax_openai_base_url(config),
            model: minimax_model(config),
            context_window: config.context_window,
            timeout_secs: config.timeout_secs,
            max_retries: 3,
            api_key: Some(api_key),
            provider_name: "minimax".to_string(),
            use_responses_api: false,
        };

        Ok(Arc::new(OpenAICompatibleProvider::new(compat_config)?))
    }

    /// Create a MiniMax provider using its Anthropic-compatible endpoint.
    #[cfg(feature = "anthropic")]
    fn create_minimax_anthropic_provider(config: &LLMConfig) -> Result<Arc<dyn LLMProvider>> {
        let api_key = config
            .minimax_api_key
            .clone()
            .or_else(|| std::env::var("MINIMAX_API_KEY").ok())
            .ok_or_else(|| {
                anyhow!(
                    "MiniMax API key not found. Set 'minimax_api_key' in config or MINIMAX_API_KEY environment variable"
                )
            })?;

        let anthropic_config = AnthropicConfig {
            api_key,
            base_url: minimax_anthropic_base_url(config),
            model: minimax_model(config),
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
            model: config
                .model
                .clone()
                .unwrap_or_else(|| "gpt-5.1-codex".to_string()),
            context_window: config.context_window,
            timeout_secs: config.timeout_secs,
            max_retries: 3,
            organization: std::env::var("OPENAI_ORG_ID").ok(),
            reasoning_effort: config.reasoning_effort.clone(),
        };

        Ok(Arc::new(OpenAIProvider::new(openai_config)?))
    }

    /// Create an xAI provider (uses OpenAI-compatible API)
    #[cfg(feature = "openai-llm")]
    fn create_xai_provider(config: &LLMConfig) -> Result<Arc<dyn LLMProvider>> {
        let api_key = config
            .xai_api_key
            .clone()
            .or_else(|| std::env::var("XAI_API_KEY").ok())
            .ok_or_else(|| {
                anyhow!(
                    "xAI API key not found. Set 'xai_api_key' in config \
                     or XAI_API_KEY environment variable"
                )
            })?;

        let xai_config = OpenAIConfig {
            api_key,
            base_url: config.xai_base_url.clone(),
            model: config
                .model
                .clone()
                .unwrap_or_else(|| "grok-4-fast".to_string()),
            // If using reasoning-tuned Grok models, default reasoning effort to high
            reasoning_effort: if config
                .model
                .as_deref()
                .map(|m| m.eq_ignore_ascii_case("grok-4-1-fast-reasoning"))
                .unwrap_or(false)
            {
                Some("high".to_string())
            } else {
                config.reasoning_effort.clone()
            },
            context_window: config.context_window,
            timeout_secs: config.timeout_secs,
            max_retries: 3,
            organization: None, // xAI doesn't use organization ID
        };

        Ok(Arc::new(OpenAIProvider::new(xai_config)?))
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
            use_responses_api: !config.use_completions_api, // Default to Responses API unless completions API requested
        };

        Ok(Arc::new(OpenAICompatibleProvider::new(compat_config)?))
    }

    /// Check if any LLM provider is available
    pub async fn check_availability(provider: &Arc<dyn LLMProvider>) -> bool {
        provider.is_available().await
    }

    /// Get a list of supported providers (based on enabled features)
    pub fn supported_providers() -> Vec<&'static str> {
        #[allow(unused_mut)]
        let mut providers = vec!["ollama", "qwen"];

        #[cfg(feature = "openai-compatible")]
        providers.push("lmstudio");

        #[cfg(feature = "openai-compatible")]
        providers.push("openai-compatible");

        #[cfg(feature = "openai-compatible")]
        providers.push("minimax");

        #[cfg(feature = "anthropic")]
        providers.push("anthropic");

        #[cfg(feature = "anthropic")]
        providers.push("minimax-anthropic");

        #[cfg(feature = "openai-llm")]
        providers.push("openai");

        #[cfg(feature = "openai-llm")]
        providers.push("xai");

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
            provider: "qwen".to_string(),
            model: Some("qwen2.5-coder:14b".to_string()),
            ollama_url: "http://localhost:11434".to_string(),
            context_window: 128000,
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
        let err = result.err().unwrap();
        assert!(err.to_string().contains("LLM is not enabled"));
    }

    #[cfg(any(feature = "openai-compatible", feature = "anthropic"))]
    #[test]
    fn test_minimax_model_defaults() {
        assert_eq!(MINIMAX_MODELS, ["MiniMax-M3", "MiniMax-M2.7"]);
        assert_eq!(minimax_model(&LLMConfig::default()), "MiniMax-M3");
    }

    #[cfg(feature = "openai-compatible")]
    #[test]
    fn test_minimax_openai_provider_uses_cn_endpoint() {
        let config = LLMConfig {
            enabled: true,
            provider: "minimax".to_string(),
            minimax_api_key: Some("test-key".to_string()),
            minimax_region: "cn_zh".to_string(),
            ..Default::default()
        };

        assert_eq!(
            minimax_openai_base_url(&config),
            "https://api.minimaxi.com/v1"
        );
        assert!(LLMProviderFactory::create_from_config(&config).is_ok());
    }

    #[cfg(feature = "anthropic")]
    #[test]
    fn test_minimax_anthropic_provider_uses_global_endpoint() {
        let config = LLMConfig {
            enabled: true,
            provider: "minimax-anthropic".to_string(),
            minimax_api_key: Some("test-key".to_string()),
            ..Default::default()
        };

        assert_eq!(
            minimax_anthropic_base_url(&config),
            "https://api.minimax.io/anthropic"
        );
        assert!(LLMProviderFactory::create_from_config(&config).is_ok());
    }
}
