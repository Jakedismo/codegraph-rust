use crate::llm_provider::*;
use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Configuration for OpenAI-compatible providers (LM Studio, Ollama, etc.)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAICompatibleConfig {
    /// Base URL for the API (e.g., "http://localhost:1234/v1")
    pub base_url: String,
    /// Model to use
    pub model: String,
    /// Maximum context window
    pub context_window: usize,
    /// Request timeout in seconds
    pub timeout_secs: u64,
    /// Maximum retries for failed requests
    pub max_retries: u32,
    /// Optional API key (some providers require it, some don't)
    pub api_key: Option<String>,
    /// Provider name for display purposes
    pub provider_name: String,
    /// Whether to use Responses API (true) or Chat Completions API (false)
    pub use_responses_api: bool,
}

impl Default for OpenAICompatibleConfig {
    fn default() -> Self {
        Self {
            base_url: "http://localhost:1234/v1".to_string(),
            model: "local-model".to_string(),
            context_window: 256_000,
            timeout_secs: 120,
            max_retries: 3,
            api_key: None,
            provider_name: "openai-compatible".to_string(),
            use_responses_api: true, // Default to new Responses API
        }
    }
}

impl OpenAICompatibleConfig {
    /// Create config for LM Studio
    pub fn lm_studio(model: String) -> Self {
        Self {
            base_url: "http://localhost:1234/v1".to_string(),
            model,
            context_window: 256_000,
            provider_name: "lmstudio".to_string(),
            use_responses_api: true,
            ..Default::default()
        }
    }

    /// Create config for Ollama (OpenAI-compatible endpoint)
    pub fn ollama(model: String) -> Self {
        Self {
            base_url: "http://localhost:11434/v1".to_string(),
            model,
            context_window: 256_000,
            provider_name: "ollama".to_string(),
            use_responses_api: true,
            ..Default::default()
        }
    }

    /// Create config for custom endpoint
    pub fn custom(base_url: String, model: String, provider_name: String) -> Self {
        Self {
            base_url,
            model,
            provider_name,
            use_responses_api: true,
            ..Default::default()
        }
    }
}

/// OpenAI-compatible LLM provider
pub struct OpenAICompatibleProvider {
    config: OpenAICompatibleConfig,
    client: Client,
}

impl OpenAICompatibleProvider {
    /// Create a new OpenAI-compatible provider
    pub fn new(config: OpenAICompatibleConfig) -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(config.timeout_secs))
            .build()
            .context("Failed to create HTTP client")?;

        Ok(Self { config, client })
    }

    /// Create for LM Studio
    pub fn lm_studio(model: String) -> Result<Self> {
        Self::new(OpenAICompatibleConfig::lm_studio(model))
    }

    /// Create for Ollama
    pub fn ollama(model: String) -> Result<Self> {
        Self::new(OpenAICompatibleConfig::ollama(model))
    }

    /// Send a request with retry logic
    async fn send_request(
        &self,
        messages: &[Message],
        config: &GenerationConfig,
    ) -> Result<ResponseAPIResponse> {
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
                            "{} request failed (attempt {}/{}), retrying...",
                            self.config.provider_name,
                            attempt + 1,
                            self.config.max_retries + 1
                        );
                    }
                }
            }
        }

        Err(last_error.unwrap_or_else(|| anyhow!("All retry attempts failed")))
    }

    /// Try a single request using Responses API format
    async fn try_request(
        &self,
        messages: &[Message],
        config: &GenerationConfig,
    ) -> Result<ResponseAPIResponse> {
        if self.config.use_responses_api {
            self.try_responses_api_request(messages, config).await
        } else {
            self.try_chat_completions_request(messages, config).await
        }
    }

    /// Try request using Responses API
    async fn try_responses_api_request(
        &self,
        messages: &[Message],
        config: &GenerationConfig,
    ) -> Result<ResponseAPIResponse> {
        // Extract system instructions and user input
        let instructions = messages
            .iter()
            .find(|m| matches!(m.role, MessageRole::System))
            .map(|m| m.content.clone());

        let input = messages
            .iter()
            .filter(|m| !matches!(m.role, MessageRole::System))
            .map(|m| format!("{}: {}", m.role, m.content))
            .collect::<Vec<_>>()
            .join("\n\n");

        let request = ResponsesAPIRequest {
            model: self.config.model.clone(),
            input,
            instructions,
            max_completion_tokens: config.max_completion_token.or(config.max_tokens),
            reasoning_effort: config.reasoning_effort.clone(),
            top_p: config.top_p,
            stop: config.stop.clone(),
        };

        let mut request_builder = self
            .client
            .post(format!("{}/responses", self.config.base_url))
            .header("Content-Type", "application/json")
            .json(&request);

        // Add API key if provided
        if let Some(api_key) = &self.config.api_key {
            request_builder =
                request_builder.header("Authorization", format!("Bearer {}", api_key));
        }

        let response = request_builder.send().await.context(format!(
            "Failed to send request to {} Responses API at {}",
            self.config.provider_name, self.config.base_url
        ))?;

        let status = response.status();

        if !status.is_success() {
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());

            return Err(anyhow!(
                "{} API error ({}): {}",
                self.config.provider_name,
                status,
                error_text
            ));
        }

        response
            .json::<ResponseAPIResponse>()
            .await
            .context(format!(
                "Failed to parse {} Responses API response",
                self.config.provider_name
            ))
    }

    /// Try request using Chat Completions API (fallback for older systems)
    async fn try_chat_completions_request(
        &self,
        messages: &[Message],
        config: &GenerationConfig,
    ) -> Result<ResponseAPIResponse> {
        let request = ChatCompletionsRequest {
            model: self.config.model.clone(),
            messages: messages
                .iter()
                .map(|m| ChatMessage {
                    role: m.role.to_string(),
                    content: m.content.clone(),
                })
                .collect(),
            max_tokens: config.max_tokens,
            max_completion_tokens: config.max_completion_token.or(config.max_tokens),
            reasoning_effort: config.reasoning_effort.clone(),
            top_p: config.top_p,
            stop: config.stop.clone(),
        };

        let mut request_builder = self
            .client
            .post(format!("{}/chat/completions", self.config.base_url))
            .header("Content-Type", "application/json")
            .json(&request);

        // Add API key if provided
        if let Some(api_key) = &self.config.api_key {
            request_builder =
                request_builder.header("Authorization", format!("Bearer {}", api_key));
        }

        let response = request_builder.send().await.context(format!(
            "Failed to send request to {} Chat Completions API at {}",
            self.config.provider_name, self.config.base_url
        ))?;

        let status = response.status();

        if !status.is_success() {
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());

            return Err(anyhow!(
                "{} API error ({}): {}",
                self.config.provider_name,
                status,
                error_text
            ));
        }

        let chat_response: ChatCompletionsResponse = response.json().await.context(format!(
            "Failed to parse {} Chat Completions API response",
            self.config.provider_name
        ))?;

        // Convert Chat Completions response to Responses API format
        let choice = chat_response
            .choices
            .first()
            .ok_or_else(|| anyhow!("No choices in response"))?;

        Ok(ResponseAPIResponse {
            id: chat_response.id,
            response_type: "response".to_string(),
            status: choice.finish_reason.clone(),
            output_text: choice.message.content.clone(),
            usage: chat_response.usage.map(|u| Usage {
                prompt_tokens: u.prompt_tokens,
                output_tokens: u.completion_tokens,
                total_tokens: u.total_tokens,
                reasoning_tokens: None,
            }),
        })
    }
}

#[async_trait]
impl LLMProvider for OpenAICompatibleProvider {
    async fn generate_chat(
        &self,
        messages: &[Message],
        config: &GenerationConfig,
    ) -> LLMResult<LLMResponse> {
        let response = self.send_request(messages, config).await?;

        Ok(LLMResponse {
            content: response.output_text,
            total_tokens: response.usage.as_ref().map(|u| u.total_tokens),
            prompt_tokens: response.usage.as_ref().map(|u| u.prompt_tokens),
            completion_tokens: response.usage.as_ref().map(|u| u.output_tokens),
            finish_reason: response.status.clone(),
            model: self.config.model.clone(),
        })
    }

    async fn is_available(&self) -> bool {
        // Check if the endpoint is reachable
        let result = self
            .client
            .get(format!("{}/models", self.config.base_url))
            .send()
            .await;

        result.is_ok()
    }

    fn provider_name(&self) -> &str {
        &self.config.provider_name
    }

    fn model_name(&self) -> &str {
        &self.config.model
    }

    fn characteristics(&self) -> ProviderCharacteristics {
        ProviderCharacteristics {
            max_tokens: self.config.context_window,
            avg_latency_ms: 1500, // Local models are typically slower
            rpm_limit: None,      // No rate limits for local providers
            tpm_limit: None,
            supports_streaming: true,
            supports_functions: false, // Most local providers don't support function calling
        }
    }
}

#[async_trait]
impl CodeIntelligenceProvider for OpenAICompatibleProvider {
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

// API request/response types for Responses API

#[derive(Debug, Serialize)]
struct ResponsesAPIRequest {
    model: String,
    input: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    instructions: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_completion_tokens: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    reasoning_effort: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stop: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
struct ResponseAPIResponse {
    id: String,
    #[serde(rename = "type")]
    response_type: String,
    status: Option<String>,
    output_text: String,
    #[serde(default)]
    usage: Option<Usage>,
}

#[derive(Debug, Deserialize)]
struct Usage {
    prompt_tokens: usize,
    #[serde(alias = "completion_tokens")]
    output_tokens: usize,
    total_tokens: usize,
    #[serde(default)]
    reasoning_tokens: Option<usize>,
}

// API request/response types for Chat Completions API (fallback)

#[derive(Debug, Serialize)]
struct ChatCompletionsRequest {
    model: String,
    messages: Vec<ChatMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_completion_tokens: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    reasoning_effort: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stop: Option<Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize)]
struct ChatMessage {
    role: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct ChatCompletionsResponse {
    id: String,
    object: String,
    choices: Vec<ChatChoice>,
    usage: Option<ChatUsage>,
}

#[derive(Debug, Deserialize)]
struct ChatChoice {
    message: ChatMessage,
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ChatUsage {
    prompt_tokens: usize,
    completion_tokens: usize,
    total_tokens: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lm_studio_config() {
        let config = OpenAICompatibleConfig::lm_studio("test-model".to_string());
        assert_eq!(config.base_url, "http://localhost:1234/v1");
        assert_eq!(config.provider_name, "lmstudio");
        assert!(config.use_responses_api);
    }

    #[test]
    fn test_ollama_config() {
        let config = OpenAICompatibleConfig::ollama("llama3".to_string());
        assert_eq!(config.base_url, "http://localhost:11434/v1");
        assert_eq!(config.provider_name, "ollama");
        assert!(config.use_responses_api);
    }
}
