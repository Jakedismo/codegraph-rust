use crate::llm_provider::*;
use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};

const ANTHROPIC_API_BASE: &str = "https://api.anthropic.com/v1";
const DEFAULT_MODEL: &str = "claude-3-5-sonnet-20241022";
const API_VERSION: &str = "2023-06-01";
const STRUCTURED_OUTPUTS_BETA: &str = "structured-outputs-2025-11-13";

/// Configuration for Anthropic Claude provider
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnthropicConfig {
    /// API key for Anthropic
    pub api_key: String,
    /// Model to use (e.g., "claude-3-5-sonnet-20241022")
    pub model: String,
    /// Maximum context window
    pub context_window: usize,
    /// Request timeout in seconds
    pub timeout_secs: u64,
    /// Maximum retries for failed requests
    pub max_retries: u32,
}

impl Default for AnthropicConfig {
    fn default() -> Self {
        Self {
            api_key: std::env::var("ANTHROPIC_API_KEY").unwrap_or_default(),
            model: DEFAULT_MODEL.to_string(),
            context_window: 200_000,
            timeout_secs: 120,
            max_retries: 3,
        }
    }
}

/// Anthropic Claude LLM provider
pub struct AnthropicProvider {
    config: AnthropicConfig,
    client: Client,
}

impl AnthropicProvider {
    /// Create a new Anthropic provider
    pub fn new(config: AnthropicConfig) -> Result<Self> {
        if config.api_key.is_empty() {
            return Err(anyhow!(
                "Anthropic API key is required. Set ANTHROPIC_API_KEY environment variable."
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
        Self::new(AnthropicConfig::default())
    }

    /// Send a request to Anthropic API with retry logic
    async fn send_request(
        &self,
        messages: &[Message],
        config: &GenerationConfig,
    ) -> Result<AnthropicResponse> {
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
                            "Anthropic request failed (attempt {}/{}), retrying...",
                            attempt + 1,
                            self.config.max_retries + 1
                        );
                    }
                }
            }
        }

        Err(last_error.unwrap_or_else(|| anyhow!("All retry attempts failed")))
    }

    /// Try a single request to Anthropic API
    async fn try_request(
        &self,
        messages: &[Message],
        config: &GenerationConfig,
    ) -> Result<AnthropicResponse> {
        self.try_request_with_tools(messages, None, config).await
    }

    /// Try a single request to Anthropic API with optional tools
    async fn try_request_with_tools(
        &self,
        messages: &[Message],
        tools: Option<&[ToolDefinition]>,
        config: &GenerationConfig,
    ) -> Result<AnthropicResponse> {
        // Convert CodeGraph ToolDefinition to Anthropic format
        let anthropic_tools: Option<Vec<AnthropicTool>> = tools.map(|t| {
            t.iter()
                .map(|tool| AnthropicTool {
                    name: tool.function.name.clone(),
                    description: tool.function.description.clone(),
                    input_schema: tool.function.parameters.clone(),
                })
                .collect()
        });

        // Debug log tools being sent
        if let Some(ref tools) = anthropic_tools {
            tracing::info!(
                "🔧 Anthropic request with {} tools: {:?}",
                tools.len(),
                tools.iter().map(|t| &t.name).collect::<Vec<_>>()
            );
        } else {
            tracing::info!("🔧 Anthropic request with NO tools");
        }

        // Extract system message
        let system = messages
            .iter()
            .find(|m| matches!(m.role, MessageRole::System))
            .map(|m| m.content.clone());

        // Convert response_format to Anthropic's native output_format
        let output_format = match &config.response_format {
            Some(ResponseFormat::JsonSchema { json_schema }) => {
                Some(AnthropicOutputFormat {
                    format_type: "json_schema".to_string(),
                    schema: Some(json_schema.schema.clone()),
                })
            }
            Some(ResponseFormat::JsonObject) => Some(AnthropicOutputFormat {
                format_type: "json_schema".to_string(),
                schema: Some(serde_json::json!({
                    "type": "object",
                    "additionalProperties": true
                })),
            }),
            _ => None,
        };

        // Check if we need the structured outputs beta header
        let needs_beta = output_format.is_some();

        let request = AnthropicRequest {
            model: self.config.model.clone(),
            messages: messages
                .iter()
                .filter(|m| !matches!(m.role, MessageRole::System))
                .map(|m| AnthropicMessage {
                    role: match m.role {
                        MessageRole::User => "user".to_string(),
                        MessageRole::Assistant => "assistant".to_string(),
                        MessageRole::System => "user".to_string(),
                    },
                    content: m.content.clone(),
                })
                .collect(),
            system,
            max_tokens: config.max_tokens.unwrap_or(4096),
            temperature: Some(config.temperature),
            top_p: config.top_p,
            stop_sequences: config.stop.clone(),
            tools: anthropic_tools,
            output_format,
        };

        let mut request_builder = self
            .client
            .post(format!("{}/messages", ANTHROPIC_API_BASE))
            .header("x-api-key", &self.config.api_key)
            .header("anthropic-version", API_VERSION)
            .header("content-type", "application/json");

        // Add beta header for structured outputs
        if needs_beta {
            request_builder = request_builder.header("anthropic-beta", STRUCTURED_OUTPUTS_BETA);
        }

        let response = request_builder
            .json(&request)
            .send()
            .await
            .context("Failed to send request to Anthropic API")?;

        let status = response.status();

        if !status.is_success() {
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());

            return Err(anyhow!("Anthropic API error ({}): {}", status, error_text));
        }

        response
            .json::<AnthropicResponse>()
            .await
            .context("Failed to parse Anthropic API response")
    }
}

#[async_trait]
impl LLMProvider for AnthropicProvider {
    async fn generate_chat(
        &self,
        messages: &[Message],
        config: &GenerationConfig,
    ) -> LLMResult<LLMResponse> {
        let _start = Instant::now();
        let response = self.send_request(messages, config).await?;

        let content = response
            .content
            .iter()
            .filter_map(|c| {
                if c.content_type == "text" {
                    Some(c.text.as_deref().unwrap_or(""))
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
            .join("");

        let content_clone = content.clone();

        Ok(LLMResponse {
            content,
            answer: content_clone,
            total_tokens: Some(response.usage.input_tokens + response.usage.output_tokens),
            prompt_tokens: Some(response.usage.input_tokens),
            completion_tokens: Some(response.usage.output_tokens),
            finish_reason: Some(response.stop_reason),
            model: response.model,
            tool_calls: None,
        })
    }

    async fn generate_chat_with_tools(
        &self,
        messages: &[Message],
        tools: Option<&[ToolDefinition]>,
        config: &GenerationConfig,
    ) -> LLMResult<LLMResponse> {
        let _start = Instant::now();
        let response = self
            .try_request_with_tools(messages, tools, config)
            .await?;

        // Extract text content
        let content = response
            .content
            .iter()
            .filter_map(|c| {
                if c.content_type == "text" {
                    c.text.as_deref().map(String::from)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
            .join("");

        // Extract tool_use blocks as tool calls
        let tool_calls: Option<Vec<ToolCall>> = {
            let calls: Vec<ToolCall> = response
                .content
                .iter()
                .filter(|c| c.content_type == "tool_use")
                .filter_map(|c| {
                    let id = c.id.as_ref()?;
                    let name = c.name.as_ref()?;
                    let input = c.input.as_ref()?;
                    Some(ToolCall {
                        id: id.clone(),
                        call_type: "function".to_string(),
                        function: FunctionCall {
                            name: name.clone(),
                            arguments: serde_json::to_string(input).unwrap_or_default(),
                        },
                    })
                })
                .collect();

            if calls.is_empty() {
                None
            } else {
                Some(calls)
            }
        };

        // Determine finish_reason - "tool_use" in Anthropic means tool calls
        let finish_reason = if tool_calls.is_some() {
            Some("tool_calls".to_string())
        } else {
            Some(response.stop_reason.clone())
        };

        tracing::info!(
            "Anthropic generate_chat_with_tools: tool_calls={}, finish_reason={:?}",
            tool_calls.as_ref().map_or(0, |tc| tc.len()),
            finish_reason
        );

        Ok(LLMResponse {
            content: content.clone(),
            answer: content,
            total_tokens: Some(response.usage.input_tokens + response.usage.output_tokens),
            prompt_tokens: Some(response.usage.input_tokens),
            completion_tokens: Some(response.usage.output_tokens),
            finish_reason,
            model: response.model,
            tool_calls,
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
        "anthropic"
    }

    fn model_name(&self) -> &str {
        &self.config.model
    }

    fn characteristics(&self) -> ProviderCharacteristics {
        // Characteristics vary by model
        let (max_tokens, rpm_limit, tpm_limit) = match self.config.model.as_str() {
            m if m.contains("opus") => (200_000, Some(50), Some(40_000)),
            m if m.contains("sonnet") => (200_000, Some(50), Some(40_000)),
            m if m.contains("haiku") => (200_000, Some(50), Some(50_000)),
            _ => (self.config.context_window, Some(50), Some(40_000)),
        };

        ProviderCharacteristics {
            max_tokens,
            avg_latency_ms: 1000,
            rpm_limit,
            tpm_limit,
            supports_streaming: true,
            supports_functions: true, // Claude supports native tool calling
        }
    }
}

#[async_trait]
impl CodeIntelligenceProvider for AnthropicProvider {
    async fn analyze_semantic_context(&self, query: &str, context: &str) -> LLMResult<String> {
        let prompt = format!(
            "Analyze the following code context and answer the query.\n\nContext:\n{}\n\nQuery: {}\n\nProvide a detailed analysis:",
            context, query
        );

        let response = self.generate(&prompt).await?;
        Ok(response.content)
    }

    async fn detect_patterns(&self, code_samples: &[String]) -> LLMResult<String> {
        let samples = code_samples.join("\n\n---\n\n");
        let prompt = format!(
            "Analyze the following code samples and identify common patterns, idioms, and best practices:\n\n{}\n\nProvide a detailed analysis:",
            samples
        );

        let response = self.generate(&prompt).await?;
        Ok(response.content)
    }

    async fn analyze_impact(&self, target_code: &str, dependencies: &str) -> LLMResult<String> {
        let prompt = format!(
            "Analyze the impact of changes to the following code, considering its dependencies.\n\nTarget Code:\n{}\n\nDependencies:\n{}\n\nProvide a detailed impact analysis:",
            target_code, dependencies
        );

        let response = self.generate(&prompt).await?;
        Ok(response.content)
    }
}

// Anthropic API request/response types

#[derive(Debug, Serialize)]
struct AnthropicRequest {
    model: String,
    messages: Vec<AnthropicMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    system: Option<String>,
    max_tokens: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stop_sequences: Option<Vec<String>>,
    /// Tools for function calling (Anthropic native format)
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<AnthropicTool>>,
    /// Structured output format (beta feature)
    #[serde(skip_serializing_if = "Option::is_none")]
    output_format: Option<AnthropicOutputFormat>,
}

/// Anthropic structured output format (beta)
#[derive(Debug, Serialize)]
struct AnthropicOutputFormat {
    #[serde(rename = "type")]
    format_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    schema: Option<serde_json::Value>,
}

/// Anthropic tool definition format
#[derive(Debug, Serialize)]
struct AnthropicTool {
    name: String,
    description: String,
    input_schema: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize)]
struct AnthropicMessage {
    role: String,
    content: String,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct AnthropicResponse {
    id: String,
    #[serde(rename = "type")]
    response_type: String,
    role: String,
    content: Vec<ContentBlock>,
    model: String,
    stop_reason: String,
    usage: Usage,
}

#[derive(Debug, Deserialize)]
struct ContentBlock {
    #[serde(rename = "type")]
    content_type: String,
    /// Text content (for type: "text")
    #[serde(default)]
    text: Option<String>,
    /// Tool call ID (for type: "tool_use")
    #[serde(default)]
    id: Option<String>,
    /// Tool name (for type: "tool_use")
    #[serde(default)]
    name: Option<String>,
    /// Tool input/arguments (for type: "tool_use")
    #[serde(default)]
    input: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct Usage {
    input_tokens: usize,
    output_tokens: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_from_env() {
        std::env::set_var("ANTHROPIC_API_KEY", "test-key");
        let config = AnthropicConfig::default();
        assert_eq!(config.api_key, "test-key");
    }

    #[test]
    fn test_provider_creation_requires_api_key() {
        let config = AnthropicConfig {
            api_key: String::new(),
            ..Default::default()
        };
        assert!(AnthropicProvider::new(config).is_err());
    }
}
