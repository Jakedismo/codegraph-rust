use async_trait::async_trait;
use serde::{Deserialize, Serialize};
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

/// Configuration for generation parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerationConfig {
    /// Temperature for sampling (0.0 to 2.0) - Not supported by reasoning models
    pub temperature: f32,
    /// Maximum tokens to generate (legacy parameter for Chat Completions API)
    pub max_tokens: Option<usize>,
    /// Maximum output tokens (for Responses API and reasoning models)
    pub max_output_tokens: Option<usize>,
    /// Reasoning effort for reasoning models: "minimal", "low", "medium", "high"
    pub reasoning_effort: Option<String>,
    /// Top-p nucleus sampling parameter - Not supported by reasoning models
    pub top_p: Option<f32>,
    /// Frequency penalty (-2.0 to 2.0) - Not supported by reasoning models
    pub frequency_penalty: Option<f32>,
    /// Presence penalty (-2.0 to 2.0) - Not supported by reasoning models
    pub presence_penalty: Option<f32>,
    /// Stop sequences
    pub stop: Option<Vec<String>>,
}

impl Default for GenerationConfig {
    fn default() -> Self {
        Self {
            temperature: 0.1,
            max_tokens: Some(4096),
            max_output_tokens: None, // Will use max_tokens if not set
            reasoning_effort: None,  // Only for reasoning models
            top_p: None,
            frequency_penalty: None,
            presence_penalty: None,
            stop: None,
        }
    }
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

/// Response from the LLM
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LLMResponse {
    /// Generated text content
    pub content: String,
    /// Total tokens used in the request
    pub total_tokens: Option<usize>,
    /// Tokens used in the prompt
    pub prompt_tokens: Option<usize>,
    /// Tokens generated in the completion
    pub completion_tokens: Option<usize>,
    /// Finish reason (e.g., "stop", "length", "function_call")
    pub finish_reason: Option<String>,
    /// Model used for generation
    pub model: String,
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
