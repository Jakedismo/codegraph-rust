// ABOUTME: Adapter for creating Rig providers from environment variables
// ABOUTME: Maps CODEGRAPH_LLM_PROVIDER to appropriate Rig provider clients

use anyhow::{Result, anyhow};
use std::env;

/// Supported LLM providers for Rig agents
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RigProvider {
    OpenAI,
    Anthropic,
    Ollama,
    /// xAI (Grok) - native rig provider
    XAI,
    /// Generic OpenAI-compatible endpoint
    OpenAICompatible {
        base_url: String,
    },
    /// LM Studio - uses OpenAI-compatible API
    LMStudio,
}

impl RigProvider {
    /// Detect provider from environment variables
    /// Priority: CODEGRAPH_LLM_PROVIDER > API key presence
    pub fn from_env() -> Result<Self> {
        // Check explicit provider setting
        if let Ok(provider) = env::var("CODEGRAPH_LLM_PROVIDER") {
            return Self::from_name(&provider);
        }

        // Fall back to API key detection
        if env::var("XAI_API_KEY").is_ok() {
            return Ok(Self::XAI);
        }
        if env::var("ANTHROPIC_API_KEY").is_ok() {
            return Ok(Self::Anthropic);
        }
        if env::var("OPENAI_API_KEY").is_ok() {
            return Ok(Self::OpenAI);
        }
        if env::var("OLLAMA_API_URL").is_ok()
            || env::var("OLLAMA_API_BASE_URL").is_ok()
            || env::var("OLLAMA_HOST").is_ok()
        {
            return Ok(Self::Ollama);
        }

        Err(anyhow!(
            "No LLM provider configured. Set CODEGRAPH_LLM_PROVIDER or provide API keys."
        ))
    }

    /// Parse provider from name string
    pub fn from_name(name: &str) -> Result<Self> {
        match name.to_lowercase().as_str() {
            "openai" => Ok(Self::OpenAI),
            "anthropic" => Ok(Self::Anthropic),
            "ollama" => Ok(Self::Ollama),
            "xai" => Ok(Self::XAI),
            "lmstudio" => Ok(Self::LMStudio),
            "openai-compatible" => {
                let base_url = env::var("CODEGRAPH_OPENAI_COMPATIBLE_URL")
                    .or_else(|_| env::var("OPENAI_COMPATIBLE_URL"))
                    .unwrap_or_else(|_| "http://localhost:1234/v1".to_string());
                Ok(Self::OpenAICompatible { base_url })
            }
            _ => Err(anyhow!(
                "Unknown provider: {}. Supported: openai, anthropic, ollama, xai, lmstudio, openai-compatible",
                name
            )),
        }
    }
}

/// Get the model name from environment
pub fn get_model_name() -> String {
    env::var("CODEGRAPH_LLM_MODEL")
        .or_else(|_| env::var("CODEGRAPH_AGENT_MODEL"))
        .unwrap_or_else(|_| default_model_for_provider())
}

/// Get default model based on detected provider
fn default_model_for_provider() -> String {
    match RigProvider::from_env() {
        Ok(RigProvider::OpenAI) => "gpt-4o".to_string(),
        Ok(RigProvider::Anthropic) => "claude-sonnet-4-20250514".to_string(),
        Ok(RigProvider::Ollama) => "llama3.2".to_string(),
        Ok(RigProvider::XAI) => "grok-3-latest".to_string(),
        Ok(RigProvider::LMStudio) => "default".to_string(),
        Ok(RigProvider::OpenAICompatible { .. }) => "default".to_string(),
        Err(_) => "gpt-4o".to_string(),
    }
}

/// Get maximum turns for tool loop from environment
/// Default is 8 to prevent context overflow and runaway costs
pub fn get_max_turns() -> usize {
    env::var("CODEGRAPH_AGENT_MAX_STEPS")
        .ok()
        .and_then(|v| v.parse().ok())
        .map(|v: usize| std::cmp::min(v, 10)) // Hard cap at 10 even with env override
        .unwrap_or(8)
}

/// Get context window size from environment (for tier detection)
/// Checks CODEGRAPH_CONTEXT_WINDOW first, then CODEGRAPH_LLM_CONTEXT_WINDOW for compatibility
pub fn get_context_window() -> usize {
    env::var("CODEGRAPH_CONTEXT_WINDOW")
        .or_else(|_| env::var("CODEGRAPH_LLM_CONTEXT_WINDOW"))
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(128_000) // Default to 128K
}

/// LLM adapter for creating Rig-compatible providers
pub struct RigLLMAdapter;

impl RigLLMAdapter {
    /// Create OpenAI client from environment
    #[cfg(feature = "openai")]
    pub fn openai_client() -> Result<rig::providers::openai::OpenAI> {
        Ok(rig::providers::openai::OpenAI::from_env()?)
    }

    /// Create Anthropic client from environment
    #[cfg(feature = "anthropic")]
    pub fn anthropic_client() -> Result<rig::providers::anthropic::Anthropic> {
        Ok(rig::providers::anthropic::Anthropic::from_env()?)
    }

    /// Create Ollama client from environment
    #[cfg(feature = "ollama")]
    pub fn ollama_client() -> Result<rig::providers::ollama::Ollama> {
        let base_url = env::var("OLLAMA_API_BASE_URL")
            .or_else(|_| env::var("OLLAMA_API_URL"))
            .or_else(|_| env::var("OLLAMA_HOST"))
            .unwrap_or_else(|_| "http://localhost:11434".to_string());
        let mut config = rig::providers::ollama::OllamaConfig::new().with_base_url(base_url);
        if let Ok(api_key) = env::var("OLLAMA_API_KEY") {
            config = config.with_api_key(api_key);
        }
        Ok(config.client())
    }

    /// Create xAI client from environment (native rig xAI provider)
    #[cfg(feature = "xai")]
    pub fn xai_client() -> Result<rig::providers::openai::OpenAI> {
        Ok(rig::providers::xai::from_env()?)
    }

    /// Create LM Studio client (uses OpenAI-compatible API)
    /// Uses explicit client settings without changing the process environment.
    #[cfg(feature = "openai")]
    pub fn lmstudio_client() -> Result<rig::providers::openai::OpenAI> {
        let base_url = env::var("LMSTUDIO_URL")
            .or_else(|_| env::var("CODEGRAPH_LMSTUDIO_URL"))
            .unwrap_or_else(|_| "http://localhost:1234/v1".to_string());

        // LM Studio doesn't require API key but OpenAI client needs something
        let api_key = env::var("LMSTUDIO_API_KEY").unwrap_or_else(|_| "lm-studio".to_string());

        Ok(rig::providers::openai::OpenAIConfig::new(api_key)
            .with_base_url(base_url)
            .with_route(rig::providers::openai::wire::Route::Chat)
            .client())
    }

    /// Create OpenAI-compatible client with custom base URL
    /// Uses explicit client settings without changing the process environment.
    #[cfg(feature = "openai")]
    pub fn openai_compatible_client(base_url: &str) -> Result<rig::providers::openai::OpenAI> {
        let api_key = env::var("OPENAI_COMPATIBLE_API_KEY")
            .or_else(|_| env::var("OPENAI_API_KEY"))
            .unwrap_or_else(|_| "no-key".to_string());

        Ok(rig::providers::openai::OpenAIConfig::new(api_key)
            .with_base_url(base_url)
            .with_route(rig::providers::openai::wire::Route::Chat)
            .client())
    }

    /// Get the detected provider
    pub fn provider() -> Result<RigProvider> {
        RigProvider::from_env()
    }

    /// Get the configured model name
    pub fn model() -> String {
        get_model_name()
    }

    /// Get max turns for agent tool loop
    pub fn max_turns() -> usize {
        get_max_turns()
    }

    /// Get context window for tier detection
    pub fn context_window() -> usize {
        get_context_window()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_from_name() {
        assert_eq!(
            RigProvider::from_name("openai").unwrap(),
            RigProvider::OpenAI
        );
        assert_eq!(
            RigProvider::from_name("ANTHROPIC").unwrap(),
            RigProvider::Anthropic
        );
        assert_eq!(
            RigProvider::from_name("Ollama").unwrap(),
            RigProvider::Ollama
        );
        assert_eq!(RigProvider::from_name("xai").unwrap(), RigProvider::XAI);
        assert_eq!(
            RigProvider::from_name("lmstudio").unwrap(),
            RigProvider::LMStudio
        );
        // openai-compatible returns with default base_url
        assert!(matches!(
            RigProvider::from_name("openai-compatible").unwrap(),
            RigProvider::OpenAICompatible { .. }
        ));
        assert!(RigProvider::from_name("unknown").is_err());
    }

    #[test]
    fn test_default_max_turns() {
        // Without env var, should return 8 (conservative default)
        if !test_env::run(
            concat!(module_path!(), "::test_default_max_turns"),
            &[("CODEGRAPH_AGENT_MAX_STEPS", None)],
        ) {
            return;
        }
        assert_eq!(get_max_turns(), 8);
    }

    #[test]
    fn test_default_context_window() {
        if !test_env::run(
            concat!(module_path!(), "::test_default_context_window"),
            &[
                ("CODEGRAPH_CONTEXT_WINDOW", None),
                ("CODEGRAPH_LLM_CONTEXT_WINDOW", None),
            ],
        ) {
            return;
        }
        assert_eq!(get_context_window(), 128_000);
    }
}

#[cfg(test)]
mod test_env {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/support/env.rs"
    ));
}
