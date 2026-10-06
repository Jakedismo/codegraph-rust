// ABOUTME: Resolves the agent's LLM provider, model and endpoints and builds Rig clients.
// ABOUTME: Each setting comes from the environment, then the config file's [llm] keys, then a default.

use anyhow::{Result, anyhow};
use codegraph_core::config_manager::{ConfigManager, ExplicitLlmSettings};
use std::env;

/// Where settings are looked up: an environment reader and the `[llm]` keys the user
/// wrote in the config file. Environment values win, so existing setups are unchanged.
struct Sources<'a> {
    env: &'a dyn Fn(&str) -> Option<String>,
    file: &'a ExplicitLlmSettings,
}

fn process_env(name: &str) -> Option<String> {
    env::var(name).ok().filter(|v| !v.trim().is_empty())
}

impl Sources<'_> {
    fn first_env(&self, names: &[&str]) -> Option<String> {
        names.iter().find_map(|name| (self.env)(name))
    }

    fn provider(&self) -> Result<RigProvider> {
        if let Some(name) = self
            .first_env(&["CODEGRAPH_LLM_PROVIDER"])
            .or_else(|| self.file.provider.clone())
        {
            return self.provider_from_name(&name);
        }

        // Fall back to API key detection
        if self.first_env(&["XAI_API_KEY"]).is_some() {
            return Ok(RigProvider::XAI);
        }
        if self.first_env(&["ANTHROPIC_API_KEY"]).is_some() {
            return Ok(RigProvider::Anthropic);
        }
        if self.first_env(&["OPENAI_API_KEY"]).is_some() {
            return Ok(RigProvider::OpenAI);
        }
        if self
            .first_env(&["OLLAMA_API_URL", "OLLAMA_API_BASE_URL", "OLLAMA_HOST"])
            .is_some()
            || self.file.ollama_url.is_some()
        {
            return Ok(RigProvider::Ollama);
        }

        Err(anyhow!(
            "No LLM provider configured. Set CODEGRAPH_LLM_PROVIDER, add [llm] provider to the \
             config file, or provide API keys."
        ))
    }

    fn provider_from_name(&self, name: &str) -> Result<RigProvider> {
        match name.to_lowercase().as_str() {
            "openai" => Ok(RigProvider::OpenAI),
            "anthropic" => Ok(RigProvider::Anthropic),
            "ollama" => Ok(RigProvider::Ollama),
            "xai" => Ok(RigProvider::XAI),
            "lmstudio" => Ok(RigProvider::LMStudio),
            "openai-compatible" => Ok(RigProvider::OpenAICompatible {
                base_url: self.openai_compatible_url(),
            }),
            _ => Err(anyhow!(
                "Unknown provider: {}. Supported: openai, anthropic, ollama, xai, lmstudio, openai-compatible",
                name
            )),
        }
    }

    fn model(&self) -> String {
        self.first_env(&[
            "CODEGRAPH_LLM_MODEL",
            "CODEGRAPH_AGENT_MODEL",
            "CODEGRAPH_MODEL",
        ])
        .or_else(|| self.file.model.clone())
        .unwrap_or_else(|| default_model(self.provider().ok().as_ref()))
    }

    #[cfg(any(feature = "ollama", test))]
    fn ollama_url(&self) -> String {
        self.first_env(&["OLLAMA_API_BASE_URL", "OLLAMA_API_URL", "OLLAMA_HOST"])
            .or_else(|| self.file.ollama_url.clone())
            .unwrap_or_else(|| "http://localhost:11434".to_string())
    }

    #[cfg(any(feature = "openai", test))]
    fn lmstudio_url(&self) -> String {
        self.first_env(&["LMSTUDIO_URL", "CODEGRAPH_LMSTUDIO_URL"])
            .or_else(|| self.file.lmstudio_url.as_deref().map(with_v1_suffix))
            .unwrap_or_else(|| "http://localhost:1234/v1".to_string())
    }

    fn openai_compatible_url(&self) -> String {
        self.first_env(&["CODEGRAPH_OPENAI_COMPATIBLE_URL", "OPENAI_COMPATIBLE_URL"])
            .or_else(|| self.file.openai_compatible_url.clone())
            .unwrap_or_else(|| "http://localhost:1234/v1".to_string())
    }
}

/// The config file documents LM Studio's server root; its OpenAI-compatible API is under `/v1`.
#[cfg(any(feature = "openai", test))]
fn with_v1_suffix(url: &str) -> String {
    let trimmed = url.trim_end_matches('/');
    if trimmed.ends_with("/v1") {
        trimmed.to_string()
    } else {
        format!("{trimmed}/v1")
    }
}

fn default_model(provider: Option<&RigProvider>) -> String {
    match provider {
        Some(RigProvider::OpenAI) => "gpt-4o".to_string(),
        Some(RigProvider::Anthropic) => "claude-sonnet-4-20250514".to_string(),
        Some(RigProvider::Ollama) => "llama3.2".to_string(),
        Some(RigProvider::XAI) => "grok-3-latest".to_string(),
        Some(RigProvider::LMStudio) => "default".to_string(),
        Some(RigProvider::OpenAICompatible { .. }) => "default".to_string(),
        None => "gpt-4o".to_string(),
    }
}

/// Run `f` against the process environment and the config file's `[llm]` keys.
fn with_sources<T>(f: impl FnOnce(&Sources<'_>) -> T) -> T {
    let file = ConfigManager::explicit_llm_settings();
    f(&Sources {
        env: &process_env,
        file: &file,
    })
}

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
    /// Detect the provider.
    /// Priority: CODEGRAPH_LLM_PROVIDER > `[llm] provider` in the config file > API key presence
    pub fn from_env() -> Result<Self> {
        with_sources(|sources| sources.provider())
    }

    /// Parse provider from name string
    pub fn from_name(name: &str) -> Result<Self> {
        with_sources(|sources| sources.provider_from_name(name))
    }
}

/// Get the model name: CODEGRAPH_LLM_MODEL, CODEGRAPH_AGENT_MODEL, CODEGRAPH_MODEL,
/// then `[llm] model` in the config file, then the provider's default.
pub fn get_model_name() -> String {
    with_sources(|sources| sources.model())
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

/// Get context window size (for tier detection): CODEGRAPH_CONTEXT_WINDOW, then
/// CODEGRAPH_LLM_CONTEXT_WINDOW, then `[llm] context_window` in the config file, then 128K.
pub fn get_context_window() -> usize {
    ConfigManager::agent_context_window()
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
        let base_url = with_sources(|sources| sources.ollama_url());
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
        let base_url = with_sources(|sources| sources.lmstudio_url());

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

    fn sources_with<'a>(
        env: &'a dyn Fn(&str) -> Option<String>,
        file: &'a ExplicitLlmSettings,
    ) -> Sources<'a> {
        Sources { env, file }
    }

    #[test]
    fn test_config_file_fills_in_when_env_is_silent() {
        let file = ExplicitLlmSettings {
            provider: Some("anthropic".into()),
            model: Some("claude-sonnet-4".into()),
            lmstudio_url: Some("http://studio:1234/".into()),
            ..Default::default()
        };
        let no_env = |_: &str| None;
        let sources = sources_with(&no_env, &file);
        assert_eq!(sources.provider().unwrap(), RigProvider::Anthropic);
        assert_eq!(sources.model(), "claude-sonnet-4");
        assert_eq!(sources.lmstudio_url(), "http://studio:1234/v1");
        assert_eq!(sources.ollama_url(), "http://localhost:11434");
    }

    #[test]
    fn test_env_wins_over_config_file() {
        let file = ExplicitLlmSettings {
            provider: Some("anthropic".into()),
            model: Some("claude-sonnet-4".into()),
            ollama_url: Some("http://file:11434".into()),
            ..Default::default()
        };
        let env = |name: &str| match name {
            "CODEGRAPH_LLM_PROVIDER" => Some("ollama".to_string()),
            "CODEGRAPH_MODEL" => Some("from-codegraph-model".to_string()),
            "OLLAMA_HOST" => Some("http://env:11434".to_string()),
            _ => None,
        };
        let sources = sources_with(&env, &file);
        assert_eq!(sources.provider().unwrap(), RigProvider::Ollama);
        assert_eq!(sources.model(), "from-codegraph-model");
        assert_eq!(sources.ollama_url(), "http://env:11434");

        let specific = |name: &str| match name {
            "CODEGRAPH_LLM_MODEL" => Some("llm-model".to_string()),
            "CODEGRAPH_MODEL" => Some("generic".to_string()),
            _ => None,
        };
        assert_eq!(sources_with(&specific, &file).model(), "llm-model");
    }

    #[test]
    fn test_api_key_detection_survives_without_explicit_provider() {
        // An empty [llm] table must not turn the config default ("lmstudio") into a choice.
        let file = ExplicitLlmSettings::default();
        let env = |name: &str| (name == "ANTHROPIC_API_KEY").then(|| "key".to_string());
        let sources = sources_with(&env, &file);
        assert_eq!(sources.provider().unwrap(), RigProvider::Anthropic);
        assert_eq!(sources.model(), "claude-sonnet-4-20250514");

        let no_env = |_: &str| None;
        assert!(sources_with(&no_env, &file).provider().is_err());
    }

    #[test]
    fn test_openai_compatible_url_from_config_file() {
        let file = ExplicitLlmSettings {
            provider: Some("openai-compatible".into()),
            openai_compatible_url: Some("http://gateway/v1".into()),
            ..Default::default()
        };
        let no_env = |_: &str| None;
        assert_eq!(
            sources_with(&no_env, &file).provider().unwrap(),
            RigProvider::OpenAICompatible {
                base_url: "http://gateway/v1".into()
            }
        );
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
                // Keep a developer's own config file out of the test.
                (
                    "CODEGRAPH_CONFIG_PATH",
                    Some("/nonexistent/codegraph-test.toml"),
                ),
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
