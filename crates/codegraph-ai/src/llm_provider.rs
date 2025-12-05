use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::env;
use std::fmt;

/// Result type for LLM operations
pub type LLMResult<T> = anyhow::Result<T>;

/// Performance characteristics of an LLM provider
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderCharacteristics {
    /// Maximum tokens that can be processed in a single request
    pub max_tokens: usize,
    /// Typical latency in milliseconds (for estimation)
    pub avg_latency_ms: u64,
    /// Requests per minute limit (for rate limiting)
    pub rpm_limit: Option<u64>,
    /// Tokens per minute limit (for rate limiting)
    pub tpm_limit: Option<u64>,
    /// Whether the provider supports streaming responses
    pub supports_streaming: bool,
    /// Whether the provider supports function calling
    pub supports_functions: bool,
}

/// JSON schema for structured output
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonSchema {
    /// Schema name
    pub name: String,
    /// JSON schema object (following JSON Schema specification)
    pub schema: serde_json::Value,
    /// Whether to strictly enforce the schema
    pub strict: bool,
}

/// Response format for structured outputs
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ResponseFormat {
    /// Standard text response
    Text,
    /// JSON object response
    JsonObject,
    /// Structured JSON with schema enforcement
    JsonSchema { json_schema: JsonSchema },
}

/// Configuration for generation parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerationConfig {
    /// Temperature for sampling (0.0 to 2.0) - Not supported by reasoning models
    pub temperature: f32,
    /// Maximum tokens to generate (legacy parameter for Chat Completions API)
    pub max_tokens: Option<usize>,
    /// Maximum output tokens (for Responses API and reasoning models)
    pub max_completion_token: Option<usize>,
    /// Reasoning effort for reasoning models: "minimal", "medium", "high"
    pub reasoning_effort: Option<String>,
    /// Top-p nucleus sampling parameter - Not supported by reasoning models
    pub top_p: Option<f32>,
    /// Frequency penalty (-2.0 to 2.0) - Not supported by reasoning models
    pub frequency_penalty: Option<f32>,
    /// Presence penalty (-2.0 to 2.0) - Not supported by reasoning models
    pub presence_penalty: Option<f32>,
    /// Stop sequences
    pub stop: Option<Vec<String>>,
    /// Response format for structured outputs (OpenAI, Ollama with JSON schema support)
    pub response_format: Option<ResponseFormat>,
    /// Whether to allow parallel tool calls (default: None = provider default, typically true)
    /// Set to false for structured outputs or o-series models
    pub parallel_tool_calls: Option<bool>,
}

impl Default for GenerationConfig {
    fn default() -> Self {
        Self {
            temperature: 0.1,
            max_tokens: Some(4096),
            max_completion_token: None, // Will use max_tokens if not set
            reasoning_effort: default_reasoning_effort(),
            top_p: None,
            frequency_penalty: None,
            presence_penalty: None,
            stop: None,
            response_format: None,
            parallel_tool_calls: None, // Use provider default
        }
    }
}

fn default_reasoning_effort() -> Option<String> {
    env::var("CODEGRAPH_REASONING_EFFORT")
        .ok()
        .filter(|v| matches!(v.as_str(), "minimal" | "medium" | "high"))
        .or_else(|| Some("medium".to_string()))
}

/// A message in the conversation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: MessageRole,
    pub content: String,
}

/// Role of a message in the conversation
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MessageRole {
    System,
    User,
    Assistant,
}

impl fmt::Display for MessageRole {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MessageRole::System => write!(f, "system"),
            MessageRole::User => write!(f, "user"),
            MessageRole::Assistant => write!(f, "assistant"),
        }
    }
}

// ============================================================================
// Native Tool Calling Types
// ============================================================================

/// Definition of a tool that can be called by the LLM
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    /// Tool type - always "function" for now
    #[serde(rename = "type")]
    pub tool_type: String,
    /// Function definition
    pub function: FunctionDefinition,
}

impl ToolDefinition {
    /// Create a new function tool definition
    pub fn function(
        name: impl Into<String>,
        description: impl Into<String>,
        parameters: serde_json::Value,
    ) -> Self {
        Self {
            tool_type: "function".to_string(),
            function: FunctionDefinition {
                name: name.into(),
                description: description.into(),
                parameters,
            },
        }
    }
}

/// Function definition for tool calling
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionDefinition {
    /// Name of the function
    pub name: String,
    /// Description of what the function does
    pub description: String,
    /// JSON Schema for the function parameters
    pub parameters: serde_json::Value,
}

/// A tool call made by the LLM
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    /// Unique identifier for this tool call
    pub id: String,
    /// Type of tool call - always "function" for now
    #[serde(rename = "type")]
    pub call_type: String,
    /// The function being called
    pub function: FunctionCall,
}

/// A function call within a tool call
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionCall {
    /// Name of the function to call
    pub name: String,
    /// JSON-encoded arguments for the function
    pub arguments: String,
}

impl ToolCall {
    /// Parse the arguments as a specific type
    pub fn parse_arguments<T: for<'de> Deserialize<'de>>(&self) -> Result<T, serde_json::Error> {
        serde_json::from_str(&self.function.arguments)
    }
}

/// Result of a tool execution to be sent back to the LLM
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    /// ID of the tool call this result corresponds to
    pub tool_call_id: String,
    /// Result content (typically JSON)
    pub content: String,
    /// Whether the tool execution was successful
    #[serde(default)]
    pub is_error: bool,
}

/// Response from the LLM
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LLMResponse {
    /// Generated text content
    pub content: String,
    /// Alias for content when agents expect an explicit answer field
    pub answer: String,
    /// Total tokens used in the request
    pub total_tokens: Option<usize>,
    /// Tokens used in the prompt
    pub prompt_tokens: Option<usize>,
    /// Tokens generated in the completion
    pub completion_tokens: Option<usize>,
    /// Finish reason (e.g., "stop", "length", "tool_calls")
    pub finish_reason: Option<String>,
    /// Model used for generation
    pub model: String,
    /// Tool calls requested by the LLM (for native tool calling)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
}

impl LLMResponse {
    /// Check if the LLM wants to make tool calls
    pub fn has_tool_calls(&self) -> bool {
        self.tool_calls.as_ref().map_or(false, |tc| !tc.is_empty())
    }

    /// Check if this is a final response (no more tool calls needed)
    pub fn is_final(&self) -> bool {
        !self.has_tool_calls() && self.finish_reason.as_deref() != Some("tool_calls")
    }
}

/// Metrics for LLM operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LLMMetrics {
    /// Total time taken for the request in milliseconds
    pub duration_ms: u64,
    /// Total tokens used
    pub total_tokens: usize,
    /// Tokens per second
    pub tokens_per_second: f64,
    /// Whether the request was cached
    pub cached: bool,
}

/// Main trait for LLM providers
#[async_trait]
pub trait LLMProvider: Send + Sync {
    /// Generate a completion for a single prompt
    async fn generate(&self, prompt: &str) -> LLMResult<LLMResponse> {
        let messages = vec![Message {
            role: MessageRole::User,
            content: prompt.to_string(),
        }];
        self.generate_chat(&messages, &GenerationConfig::default())
            .await
    }

    /// Generate a completion with custom configuration
    async fn generate_with_config(
        &self,
        prompt: &str,
        config: &GenerationConfig,
    ) -> LLMResult<LLMResponse> {
        let messages = vec![Message {
            role: MessageRole::User,
            content: prompt.to_string(),
        }];
        self.generate_chat(&messages, config).await
    }

    /// Generate a chat completion with message history
    async fn generate_chat(
        &self,
        messages: &[Message],
        config: &GenerationConfig,
    ) -> LLMResult<LLMResponse>;

    /// Generate a chat completion with native tool calling support
    ///
    /// This method enables the LLM to make structured tool calls that are
    /// returned in the response. The LLM will set `finish_reason: "tool_calls"`
    /// when it wants to call tools, and the caller should execute those tools
    /// and continue the conversation.
    ///
    /// Default implementation falls back to `generate_chat` (no tool support).
    /// Providers that support native tool calling should override this.
    async fn generate_chat_with_tools(
        &self,
        messages: &[Message],
        tools: Option<&[ToolDefinition]>,
        config: &GenerationConfig,
    ) -> LLMResult<LLMResponse> {
        // Default implementation: ignore tools, fall back to regular chat
        // Providers should override this to enable native tool calling
        if tools.is_some() {
            tracing::warn!(
                provider = self.provider_name(),
                "Provider does not support native tool calling, falling back to regular chat"
            );
        }
        self.generate_chat(messages, config).await
    }

    /// Check if this provider supports native tool calling
    fn supports_tool_calling(&self) -> bool {
        self.characteristics().supports_functions
    }

    /// Check if the provider is available and ready
    async fn is_available(&self) -> bool;

    /// Get the name of this provider
    fn provider_name(&self) -> &str;

    /// Get the model identifier
    fn model_name(&self) -> &str;

    /// Get performance characteristics
    fn characteristics(&self) -> ProviderCharacteristics;

    /// Get the maximum context window size
    fn context_window(&self) -> usize {
        self.characteristics().max_tokens
    }
}

/// Code intelligence capabilities (specialized for code analysis)
#[async_trait]
pub trait CodeIntelligenceProvider: LLMProvider {
    /// Analyze semantic context of code
    async fn analyze_semantic_context(&self, query: &str, context: &str) -> LLMResult<String>;

    /// Detect patterns in code samples
    async fn detect_patterns(&self, code_samples: &[String]) -> LLMResult<String>;

    /// Analyze impact of changes
    async fn analyze_impact(&self, target_code: &str, dependencies: &str) -> LLMResult<String>;
}
