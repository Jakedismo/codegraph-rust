use crate::llm_provider::*;
use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use reqwest::{Client, StatusCode};
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};

const OPENAI_API_BASE: &str = "https://api.openai.com/v1";
const DEFAULT_MODEL: &str = "gpt-4o";

/// Configuration for OpenAI provider
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAIConfig {
    /// API key for OpenAI
    pub api_key: String,
    /// Base URL for API (default: https://api.openai.com/v1)
    pub base_url: String,
    /// Model to use (e.g., "gpt-4o", "gpt-4-turbo")
    pub model: String,
    /// Maximum context window
    pub context_window: usize,
    /// Request timeout in seconds
    pub timeout_secs: u64,
    /// Maximum retries for failed requests
    pub max_retries: u32,
    /// Optional organization ID
    pub organization: Option<String>,
}

impl Default for OpenAIConfig {
    fn default() -> Self {
        Self {
            api_key: std::env::var("OPENAI_API_KEY").unwrap_or_default(),
            base_url: OPENAI_API_BASE.to_string(),
            model: DEFAULT_MODEL.to_string(),
            context_window: 128_000,
            timeout_secs: 120,
            max_retries: 3,
            organization: std::env::var("OPENAI_ORG_ID").ok(),
        }
    }
}

/// OpenAI LLM provider
pub struct OpenAIProvider {
    config: OpenAIConfig,
    client: Client,
}

impl OpenAIProvider {
    /// Create a new OpenAI provider
    pub fn new(config: OpenAIConfig) -> Result<Self> {
        if config.api_key.is_empty() {
            return Err(anyhow!(
                "OpenAI API key is required. Set OPENAI_API_KEY environment variable."
            ));
        }

        let client = Client::builder()
            .timeout(Duration::from_secs(config.timeout_secs))
            .build()
            .context("Failed to create HTTP client")?;

        Ok(Self { config, client })
    }

    /// Create from environment variables
    pub fn from_env() -> Result<Self> {
        Self::new(OpenAIConfig::default())
    }

    /// Send a request to OpenAI API with retry logic
    async fn send_request(
        &self,
        messages: &[Message],
        config: &GenerationConfig,
    ) -> Result<OpenAIResponse> {
        let mut last_error = None;

        for attempt in 0..=self.config.max_retries {
            if attempt > 0 {
                // Exponential backoff: 1s, 2s, 4s
                let delay = Duration::from_secs(2u64.pow(attempt - 1));
                tokio::time::sleep(delay).await;
            }

            match self.try_request(messages, config).await {
                Ok(response) => return Ok(response),
                Err(e) => {
                    last_error = Some(e);
                    if attempt < self.config.max_retries {
                        tracing::warn!(
                            "OpenAI request failed (attempt {}/{}), retrying...",
                            attempt + 1,
                            self.config.max_retries + 1
                        );
                    }
                }
            }
        }

        Err(last_error.unwrap_or_else(|| anyhow!("All retry attempts failed")))
    }

    /// Try a single request to OpenAI API
    async fn try_request(
        &self,
        messages: &[Message],
        config: &GenerationConfig,
    ) -> Result<OpenAIResponse> {
        let request = OpenAIRequest {
            model: self.config.model.clone(),
            messages: messages
                .iter()
                .map(|m| OpenAIMessage {
                    role: m.role.to_string(),
                    content: m.content.clone(),
                })
                .collect(),
            temperature: Some(config.temperature),
            max_tokens: config.max_tokens,
            top_p: config.top_p,
            frequency_penalty: config.frequency_penalty,
            presence_penalty: config.presence_penalty,
            stop: config.stop.clone(),
        };

        let mut request_builder = self
            .client
            .post(format!("{}/chat/completions", self.config.base_url))
            .header("Authorization", format!("Bearer {}", self.config.api_key))
            .header("Content-Type", "application/json")
            .json(&request);

        if let Some(org) = &self.config.organization {
            request_builder = request_builder.header("OpenAI-Organization", org);
        }

        let response = request_builder
            .send()
            .await
            .context("Failed to send request to OpenAI API")?;

        let status = response.status();

        if !status.is_success() {
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());

            return Err(anyhow!("OpenAI API error ({}): {}", status, error_text));
        }

        response
            .json::<OpenAIResponse>()
            .await
            .context("Failed to parse OpenAI API response")
    }
}

#[async_trait]
impl LLMProvider for OpenAIProvider {
    async fn generate_chat(
        &self,
        messages: &[Message],
        config: &GenerationConfig,
    ) -> LLMResult<LLMResponse> {
        let start = Instant::now();
        let response = self.send_request(messages, config).await?;

        let choice = response
            .choices
            .first()
            .ok_or_else(|| anyhow!("No choices in OpenAI response"))?;

        Ok(LLMResponse {
            content: choice.message.content.clone(),
            total_tokens: response.usage.as_ref().map(|u| u.total_tokens),
            prompt_tokens: response.usage.as_ref().map(|u| u.prompt_tokens),
            completion_tokens: response.usage.as_ref().map(|u| u.completion_tokens),
            finish_reason: choice.finish_reason.clone(),
            model: response.model,
        })
    }

    async fn is_available(&self) -> bool {
        // Try a simple request to check availability
        let messages = vec![Message {
            role: MessageRole::User,
            content: "test".to_string(),
        }];

        let mut config = GenerationConfig::default();
        config.max_tokens = Some(1);

        self.generate_chat(&messages, &config).await.is_ok()
    }

    fn provider_name(&self) -> &str {
        "openai"
    }

    fn model_name(&self) -> &str {
        &self.config.model
    }

    fn characteristics(&self) -> ProviderCharacteristics {
        // Characteristics vary by model
        let (max_tokens, rpm_limit, tpm_limit) = match self.config.model.as_str() {
            "gpt-4o" => (128_000, Some(500), Some(30_000)),
            "gpt-4o-mini" => (128_000, Some(500), Some(200_000)),
            "gpt-4-turbo" => (128_000, Some(500), Some(30_000)),
            "gpt-4" => (8_192, Some(500), Some(10_000)),
            "gpt-3.5-turbo" => (16_385, Some(500), Some(60_000)),
            _ => (self.config.context_window, Some(500), Some(30_000)),
        };

        ProviderCharacteristics {
            max_tokens,
            avg_latency_ms: 800,
            rpm_limit,
            tpm_limit,
            supports_streaming: true,
            supports_functions: true,
        }
    }
}

#[async_trait]
impl CodeIntelligenceProvider for OpenAIProvider {
    async fn analyze_semantic_context(&self, query: &str, context: &str) -> LLMResult<String> {
        let messages = vec![
            Message {
                role: MessageRole::System,
                content: "You are an expert code analyst. Analyze code context and answer queries with detailed, accurate information.".to_string(),
            },
            Message {
                role: MessageRole::User,
                content: format!(
                    "Context:\n{}\n\nQuery: {}\n\nProvide a detailed analysis:",
                    context, query
                ),
            },
        ];

        let response = self
            .generate_chat(&messages, &GenerationConfig::default())
            .await?;
        Ok(response.content)
    }

    async fn detect_patterns(&self, code_samples: &[String]) -> LLMResult<String> {
        let samples = code_samples.join("\n\n---\n\n");
        let messages = vec![
            Message {
                role: MessageRole::System,
                content: "You are an expert at identifying code patterns and best practices."
                    .to_string(),
            },
            Message {
                role: MessageRole::User,
                content: format!(
                    "Analyze these code samples and identify common patterns:\n\n{}",
                    samples
                ),
            },
        ];

        let response = self
            .generate_chat(&messages, &GenerationConfig::default())
            .await?;
        Ok(response.content)
    }

    async fn analyze_impact(&self, target_code: &str, dependencies: &str) -> LLMResult<String> {
        let messages = vec![
            Message {
                role: MessageRole::System,
                content: "You are an expert at analyzing code dependencies and change impact."
                    .to_string(),
            },
            Message {
                role: MessageRole::User,
                content: format!(
                    "Target Code:\n{}\n\nDependencies:\n{}\n\nAnalyze the impact of changes:",
                    target_code, dependencies
                ),
            },
        ];

        let response = self
            .generate_chat(&messages, &GenerationConfig::default())
            .await?;
        Ok(response.content)
    }
}

// OpenAI API request/response types

#[derive(Debug, Serialize)]
struct OpenAIRequest {
    model: String,
    messages: Vec<OpenAIMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    frequency_penalty: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    presence_penalty: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stop: Option<Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize)]
struct OpenAIMessage {
    role: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct OpenAIResponse {
    id: String,
    object: String,
    created: u64,
    model: String,
    choices: Vec<Choice>,
    usage: Option<Usage>,
}

#[derive(Debug, Deserialize)]
struct Choice {
    index: usize,
    message: OpenAIMessage,
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Usage {
    prompt_tokens: usize,
    completion_tokens: usize,
    total_tokens: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_from_env() {
        std::env::set_var("OPENAI_API_KEY", "test-key");
        let config = OpenAIConfig::default();
        assert_eq!(config.api_key, "test-key");
    }

    #[test]
    fn test_provider_creation_requires_api_key() {
        let config = OpenAIConfig {
            api_key: String::new(),
            ..Default::default()
        };
        assert!(OpenAIProvider::new(config).is_err());
    }
}
