use crate::llm_provider::*;
use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};

const OPENAI_API_BASE: &str = "https://api.openai.com/v1";
const DEFAULT_MODEL: &str = "gpt-5.1-codex";

/// Configuration for OpenAI provider
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAIConfig {
    /// API key for OpenAI
    pub api_key: String,
    /// Base URL for API (default: https://api.openai.com/v1)
    pub base_url: String,
    /// Model to use (e.g., "gpt-5.1, gpt-5.1-codex, gpt-5.1-codex-mini)
    pub model: String,
    /// Maximum context window
    pub context_window: usize,
    /// Request timeout in seconds
    pub timeout_secs: u64,
    /// Maximum retries for failed requests
    pub max_retries: u32,
    /// Optional organization ID
    pub organization: Option<String>,
    /// Optional reasoning effort (for reasoning models)
    pub reasoning_effort: Option<String>,
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
            reasoning_effort: None,
        }
    }
}

/// OpenAI LLM provider using the Responses API
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

    /// Check if this is a reasoning model
    fn is_reasoning_model(&self) -> bool {
        let model = self.config.model.to_lowercase();
        model.starts_with("gpt-5") || model.starts_with('o')
    }

    /// Send a request to OpenAI Responses API with retry logic
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

    /// Try a single request to OpenAI Responses API
    async fn try_request(
        &self,
        messages: &[Message],
        config: &GenerationConfig,
    ) -> Result<OpenAIResponse> {
        self.try_request_with_tools(messages, None, config)
            .await
            .map(|(response, _)| response)
    }

    /// Try a single request to OpenAI Responses API with optional tool calling support
    async fn try_request_with_tools(
        &self,
        messages: &[Message],
        tools: Option<&[crate::llm_provider::ToolDefinition]>,
        config: &GenerationConfig,
    ) -> Result<(OpenAIResponse, Option<Vec<crate::llm_provider::ToolCall>>)> {
        let is_reasoning = self.is_reasoning_model();

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

        let model_lower = self.config.model.to_lowercase();

        // Default reasoning effort for GPT-5.1 family when not provided
        let reasoning_effort = if is_reasoning && model_lower.starts_with("gpt-5.1") {
            Some(
                config
                    .reasoning_effort
                    .clone()
                    .unwrap_or_else(|| "medium".to_string()),
            )
        } else {
            config.reasoning_effort.clone()
        };

        // Convert CodeGraph ToolDefinition to Responses API format
        let responses_tools = tools.map(|t| {
            t.iter()
                .map(|tool| ResponsesAPITool {
                    tool_type: "function".to_string(),
                    name: tool.function.name.clone(),
                    description: tool.function.description.clone(),
                    parameters: tool.function.parameters.clone(),
                })
                .collect()
        });

        // Build request based on model type
        // OpenAI Responses API uses text.format instead of response_format
        let text_config = config
            .response_format
            .clone()
            .map(|rf| TextConfig { format: rf.into() });
        let mut request = OpenAIRequest {
            model: self.config.model.clone(),
            input,
            instructions,
            max_completion_tokens: config.max_completion_token.or(config.max_tokens),
            reasoning: None,
            temperature: None,
            top_p: None,
            stop: config.stop.clone(),
            text: text_config,
            tools: responses_tools,
        };

        // Only add sampling parameters for non-reasoning models
        if !is_reasoning {
            request.temperature = Some(config.temperature);
            request.top_p = config.top_p;
        } else {
            // Add reasoning effort for reasoning models
            request.reasoning = reasoning_effort.as_ref().map(|effort| Reasoning {
                effort: effort.clone(),
            });
        }

        let mut request_builder = self
            .client
            .post(format!("{}/responses", self.config.base_url))
            .header("Authorization", format!("Bearer {}", self.config.api_key))
            .header("Content-Type", "application/json")
            .json(&request);

        if let Some(org) = &self.config.organization {
            request_builder = request_builder.header("OpenAI-Organization", org);
        }

        let response = request_builder
            .send()
            .await
            .context("Failed to send request to OpenAI Responses API")?;

        let status = response.status();

        if !status.is_success() {
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());

            return Err(anyhow!("OpenAI API error ({}): {}", status, error_text));
        }

        // Get raw response text for debugging
        let response_text = response
            .text()
            .await
            .context("Failed to read OpenAI Responses API response body")?;

        // Log the raw response for debugging
        tracing::debug!(
            model = %self.config.model,
            response = %response_text,
            "Raw OpenAI Responses API response"
        );

        // Parse the response
        let parsed: OpenAIResponse = serde_json::from_str::<OpenAIResponse>(&response_text)
            .context(format!(
                "Failed to parse OpenAI Responses API response. Raw response: {}",
                response_text
            ))?;

        // Extract tool calls from function_call output items
        let tool_calls: Option<Vec<crate::llm_provider::ToolCall>> = {
            let calls: Vec<crate::llm_provider::ToolCall> = parsed
                .output
                .iter()
                .filter(|item| item.output_type == "function_call")
                .filter_map(|item| {
                    let call_id = item.call_id.as_ref()?;
                    let name = item.name.as_ref()?;
                    let arguments = item.arguments.as_ref()?;
                    Some(crate::llm_provider::ToolCall {
                        id: call_id.clone(),
                        call_type: "function".to_string(),
                        function: crate::llm_provider::FunctionCall {
                            name: name.clone(),
                            arguments: arguments.clone(),
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

        Ok((parsed, tool_calls))
    }
}

#[async_trait]
impl LLMProvider for OpenAIProvider {
    async fn generate_chat(
        &self,
        messages: &[Message],
        config: &GenerationConfig,
    ) -> LLMResult<LLMResponse> {
        let _start = Instant::now();
        let response = self.send_request(messages, config).await?;

        // Extract text from output array
        // OpenAI GPT-5.1 returns: output[{type: "message", content: [{type: "output_text", text: "..."}]}]
        let content = response
            .output
            .iter()
            .filter(|item| item.output_type == "message")
            .flat_map(|item| &item.content)
            .filter(|c| c.content_type == "output_text")
            .map(|c| c.text.as_str())
            .collect::<Vec<_>>()
            .join("\n");

        // Fallback: if empty, serialize the output array for structured answers
        let content = if content.is_empty() {
            serde_json::to_string(&response.output).unwrap_or_default()
        } else {
            content
        };

        Ok(LLMResponse {
            answer: content.clone(),
            content,
            total_tokens: response.usage.as_ref().map(|u| u.total_tokens),
            prompt_tokens: response.usage.as_ref().map(|u| u.input_tokens),
            completion_tokens: response.usage.as_ref().map(|u| u.output_tokens),
            finish_reason: response.status.clone(),
            model: self.config.model.clone(),
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
        let (response, tool_calls) = self.try_request_with_tools(messages, tools, config).await?;

        // Extract text from output array
        let content = response
            .output
            .iter()
            .filter(|item| item.output_type == "message")
            .flat_map(|item| &item.content)
            .filter(|c| c.content_type == "output_text")
            .map(|c| c.text.as_str())
            .collect::<Vec<_>>()
            .join("\n");

        let content = if content.is_empty() {
            serde_json::to_string(&response.output).unwrap_or_default()
        } else {
            content
        };

        // Determine finish_reason - if we have tool calls, it should be "tool_calls"
        let finish_reason = if tool_calls.is_some() {
            Some("tool_calls".to_string())
        } else {
            response.status.clone()
        };

        tracing::info!(
            "OpenAI generate_chat_with_tools: tool_calls={}, finish_reason={:?}",
            tool_calls.as_ref().map_or(0, |tc| tc.len()),
            finish_reason
        );

        Ok(LLMResponse {
            answer: content.clone(),
            content,
            total_tokens: response.usage.as_ref().map(|u| u.total_tokens),
            prompt_tokens: response.usage.as_ref().map(|u| u.input_tokens),
            completion_tokens: response.usage.as_ref().map(|u| u.output_tokens),
            finish_reason,
            model: self.config.model.clone(),
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
        config.max_completion_token = Some(1);

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
        let (max_tokens, rpm_limit, tpm_limit, supports_functions) =
            match self.config.model.as_str() {
                m if m.starts_with("gpt-5") => (400_000, Some(50), Some(30_000), true),

                _ => (self.config.context_window, Some(500), Some(30_000), true),
            };

        ProviderCharacteristics {
            max_tokens,
            avg_latency_ms: 800,
            rpm_limit,
            tpm_limit,
            supports_streaming: true,
            supports_functions,
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

// OpenAI Responses API request/response types

#[derive(Debug, Serialize)]
struct Reasoning {
    effort: String,
}

#[derive(Debug, Serialize)]
struct OpenAIRequest {
    model: String,
    input: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    instructions: Option<String>,
    #[serde(rename = "max_output_tokens", skip_serializing_if = "Option::is_none")]
    max_completion_tokens: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    reasoning: Option<Reasoning>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stop: Option<Vec<String>>,
    /// OpenAI Responses API uses text.format instead of response_format
    #[serde(skip_serializing_if = "Option::is_none")]
    text: Option<TextConfig>,
    /// Tools for function calling (Responses API format)
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<ResponsesAPITool>>,
}

/// Tool definition for OpenAI Responses API
#[derive(Debug, Serialize)]
struct ResponsesAPITool {
    #[serde(rename = "type")]
    tool_type: String,
    name: String,
    description: String,
    parameters: serde_json::Value,
}

/// OpenAI Responses API text format - flattened structure
/// OpenAI expects: {"type": "json_schema", "name": "...", "schema": {...}, "strict": true}
/// NOT: {"type": "json_schema", "json_schema": {...}}
#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum OpenAITextFormat {
    Text,
    JsonObject,
    JsonSchema {
        name: String,
        schema: serde_json::Value,
        strict: bool,
    },
}

impl From<crate::llm_provider::ResponseFormat> for OpenAITextFormat {
    fn from(rf: crate::llm_provider::ResponseFormat) -> Self {
        use crate::llm_provider::ResponseFormat;
        match rf {
            ResponseFormat::Text => OpenAITextFormat::Text,
            ResponseFormat::JsonObject => OpenAITextFormat::JsonObject,
            ResponseFormat::JsonSchema { json_schema } => OpenAITextFormat::JsonSchema {
                name: json_schema.name,
                schema: json_schema.schema,
                strict: json_schema.strict,
            },
        }
    }
}

#[derive(Debug, Serialize)]
struct TextConfig {
    format: OpenAITextFormat,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct OpenAIResponse {
    id: String,
    object: String,
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    output: Vec<OutputItem>,
    #[serde(default)]
    usage: Option<OpenAIUsage>,
}

#[derive(Debug, Serialize, Deserialize)]
struct OutputItem {
    #[serde(rename = "type")]
    output_type: String,
    #[serde(default)]
    content: Vec<OutputContent>,
    /// Function call ID (for type: "function_call")
    #[serde(default)]
    call_id: Option<String>,
    /// Function name (for type: "function_call")
    #[serde(default)]
    name: Option<String>,
    /// Function arguments as JSON string (for type: "function_call")
    #[serde(default)]
    arguments: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct OutputContent {
    #[serde(rename = "type")]
    content_type: String,
    #[serde(default)]
    text: String,
}

#[derive(Debug, Deserialize)]
struct OpenAIUsage {
    input_tokens: usize,
    output_tokens: usize,
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

    #[test]
    fn test_reasoning_model_detection() {
        let models = vec!["gpt-5.1"];
        for model in models {
            let config = OpenAIConfig {
                api_key: "test".to_string(),
                model: model.to_string(),
                ..Default::default()
            };
            let provider = OpenAIProvider::new(config).unwrap();
            assert!(
                provider.is_reasoning_model(),
                "Model {} should be detected as reasoning model",
                model
            );
        }
    }

    #[test]
    fn request_serializes_plural_completion_field() {
        let request = OpenAIRequest {
            model: "gpt-test".to_string(),
            input: "hello".to_string(),
            instructions: None,
            max_completion_tokens: Some(42),
            reasoning: None,
            temperature: None,
            top_p: None,
            stop: None,
            text: None,
        };

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("\"max_output_tokens\""));
        assert!(!json.contains("\"max_completion_token\""));
    }

    #[test]
    fn text_format_flattens_json_schema() {
        use crate::llm_provider::{JsonSchema, ResponseFormat};

        // Create a ResponseFormat with JSON schema
        let response_format = ResponseFormat::JsonSchema {
            json_schema: JsonSchema {
                name: "test_schema".to_string(),
                schema: serde_json::json!({"type": "object"}),
                strict: true,
            },
        };

        // Convert to OpenAI format
        let text_config = TextConfig {
            format: response_format.into(),
        };

        let json = serde_json::to_string(&text_config).unwrap();

        // Should have flattened structure: {"format":{"type":"json_schema","name":"...","schema":...,"strict":...}}
        // NOT nested: {"format":{"type":"json_schema","json_schema":{...}}}
        assert!(json.contains("\"name\":\"test_schema\""));
        assert!(json.contains("\"strict\":true"));
        assert!(json.contains("\"type\":\"json_schema\""));
        assert!(
            !json.contains("\"json_schema\":{"),
            "Should not have nested json_schema object"
        );
    }
}
