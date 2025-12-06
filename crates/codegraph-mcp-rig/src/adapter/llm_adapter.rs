// ABOUTME: Adapter for creating Rig providers from environment variables
// ABOUTME: Maps CODEGRAPH_LLM_PROVIDER to appropriate Rig provider clients

use anyhow::{anyhow, Result};
use std::env;

/// Supported LLM providers for Rig agents
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RigProvider {
    OpenAI,
    Anthropic,
    Ollama,
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
        if env::var("ANTHROPIC_API_KEY").is_ok() {
            return Ok(Self::Anthropic);
        }
        if env::var("OPENAI_API_KEY").is_ok() {
            return Ok(Self::OpenAI);
        }
        if env::var("OLLAMA_API_URL").is_ok() || env::var("OLLAMA_HOST").is_ok() {
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
            _ => Err(anyhow!("Unknown provider: {}. Supported: openai, anthropic, ollama", name)),
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
        Err(_) => "gpt-4o".to_string(),
    }
}

/// Get maximum turns for tool loop from environment
pub fn get_max_turns() -> usize {
    env::var("CODEGRAPH_AGENT_MAX_STEPS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(10)
}

/// Get context window size from environment (for tier detection)
pub fn get_context_window() -> usize {
    env::var("CODEGRAPH_LLM_CONTEXT_WINDOW")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(128_000) // Default to 128K
}

/// LLM adapter for creating Rig-compatible providers
pub struct RigLLMAdapter;

impl RigLLMAdapter {
    /// Create OpenAI client from environment
    #[cfg(feature = "openai")]
    pub fn openai_client() -> rig_core::providers::openai::Client {
        rig_core::providers::openai::Client::from_env()
    }

    /// Create Anthropic client from environment
    #[cfg(feature = "anthropic")]
    pub fn anthropic_client() -> rig_core::providers::anthropic::Client {
        rig_core::providers::anthropic::Client::from_env()
    }

    /// Create Ollama client from environment
    #[cfg(feature = "ollama")]
    pub fn ollama_client() -> rig_core::providers::ollama::Client {
        let base_url = env::var("OLLAMA_API_URL")
            .or_else(|_| env::var("OLLAMA_HOST"))
            .unwrap_or_else(|_| "http://localhost:11434".to_string());

        rig_core::providers::ollama::Client::new(&base_url)
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
        assert_eq!(RigProvider::from_name("openai").unwrap(), RigProvider::OpenAI);
        assert_eq!(RigProvider::from_name("ANTHROPIC").unwrap(), RigProvider::Anthropic);
        assert_eq!(RigProvider::from_name("Ollama").unwrap(), RigProvider::Ollama);
        assert!(RigProvider::from_name("unknown").is_err());
    }

    #[test]
    fn test_default_max_turns() {
        // Without env var, should return 10
        std::env::remove_var("CODEGRAPH_AGENT_MAX_STEPS");
        assert_eq!(get_max_turns(), 10);
    }

    #[test]
    fn test_default_context_window() {
        std::env::remove_var("CODEGRAPH_LLM_CONTEXT_WINDOW");
        assert_eq!(get_context_window(), 128_000);
    }
}
