use async_trait::async_trait;
use codegraph_core::{CodeGraphError, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};
use tokio::time::timeout;
use tracing::{debug, error, info, warn};

/// Configuration for Qwen2.5-Coder-14B local model via Ollama
#[derive(Debug, Clone)]
pub struct QwenConfig {
    pub model_name: String,
    pub base_url: String,
    pub context_window: usize,
    pub max_tokens: usize,
    pub temperature: f32,
    pub timeout: Duration,
}

impl Default for QwenConfig {
    fn default() -> Self {
        Self {
            model_name: "qwen2.5-coder-14b-128k".to_string(),
            base_url: "http://localhost:11434".to_string(),
            context_window: 128000,
            max_tokens: 8192,
            temperature: 0.1,
            timeout: Duration::from_secs(90),
        }
    }
}

/// Ollama API request structure for chat completions
#[derive(Debug, Serialize)]
struct OllamaRequest {
    model: String,
    messages: Vec<OllamaMessage>,
    stream: bool,
    options: OllamaOptions,
}

#[derive(Debug, Serialize, Deserialize)]
struct OllamaMessage {
    role: String,
    content: String,
}

#[derive(Debug, Serialize)]
struct OllamaOptions {
    temperature: f32,
    top_p: f32,
    top_k: i32,
    num_predict: usize,
    num_ctx: usize,
}

/// Ollama API response structure
#[derive(Debug, Deserialize)]
struct OllamaResponse {
    message: OllamaMessage,
    done: bool,
    #[serde(default)]
    eval_count: Option<usize>,
    #[serde(default)]
    eval_duration: Option<u64>,
    #[serde(default)]
    prompt_eval_count: Option<usize>,
}

/// Qwen generation result with metadata
#[derive(Debug, Clone)]
pub struct QwenResult {
    pub text: String,
    pub model_used: String,
    pub processing_time: Duration,
    pub context_tokens: usize,
    pub completion_tokens: usize,
    pub confidence_score: f32,
}

/// Qwen client for CodeGraph intelligence tasks
pub struct QwenClient {
    client: Client,
    config: QwenConfig,
}

impl QwenClient {
    pub fn new(config: QwenConfig) -> Self {
        Self {
            client: Client::new(),
            config,
        }
    }

    /// Generate analysis using Qwen2.5-Coder with comprehensive context
    pub async fn generate_analysis(&self, prompt: &str, system_prompt: Option<&str>) -> Result<QwenResult> {
        let start_time = Instant::now();

        let mut messages = Vec::new();

        // Add system prompt for specialized code analysis
        if let Some(sys_prompt) = system_prompt {
            messages.push(OllamaMessage {
                role: "system".to_string(),
                content: sys_prompt.to_string(),
            });
        } else {
            // Default system prompt for CodeGraph intelligence
            messages.push(OllamaMessage {
                role: "system".to_string(),
                content: "You are Qwen2.5-Coder, a state-of-the-art AI specialized in comprehensive code analysis and understanding. You are integrated with CodeGraph's semantic analysis engine to provide intelligent codebase insights for powerful LLMs like Claude and GPT-4. Focus on providing detailed, structured analysis that enables superior code generation and understanding.".to_string(),
            });
        }

        // Add user prompt
        messages.push(OllamaMessage {
            role: "user".to_string(),
            content: prompt.to_string(),
        });

        let request = OllamaRequest {
            model: self.config.model_name.clone(),
            messages,
            stream: false,
            options: OllamaOptions {
                temperature: self.config.temperature,
                top_p: 0.9,
                top_k: 40,
                num_predict: self.config.max_tokens,
                num_ctx: self.config.context_window,
            },
        };

        debug!("Sending request to Qwen2.5-Coder: {} tokens context", self.config.context_window);

        let response = timeout(
            self.config.timeout,
            self.client
                .post(&format!("{}/api/chat", self.config.base_url))
                .json(&request)
                .send()
        )
        .await
        .map_err(|_| CodeGraphError::Timeout(format!("Qwen request timeout after {:?}", self.config.timeout)))?
        .map_err(|e| CodeGraphError::Network(format!("Qwen request failed: {}", e)))?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            return Err(CodeGraphError::External(format!("Qwen API error: {}", error_text)));
        }

        let response_data: OllamaResponse = response
            .json()
            .await
            .map_err(|e| CodeGraphError::Parse(format!("Failed to parse Qwen response: {}", e)))?;

        let processing_time = start_time.elapsed();

        let result = QwenResult {
            text: response_data.message.content,
            model_used: self.config.model_name.clone(),
            processing_time,
            context_tokens: response_data.prompt_eval_count.unwrap_or(0),
            completion_tokens: response_data.eval_count.unwrap_or(0),
            confidence_score: self.calculate_confidence(&response_data),
        };

        info!(
            "Qwen analysis completed: {}ms, context: {} tokens, completion: {} tokens",
            processing_time.as_millis(),
            result.context_tokens,
            result.completion_tokens
        );

        Ok(result)
    }

    /// Check if Qwen2.5-Coder model is available
    pub async fn check_availability(&self) -> Result<bool> {
        debug!("Checking Qwen2.5-Coder availability at {}", self.config.base_url);

        let response = timeout(
            Duration::from_secs(5),
            self.client.get(&format!("{}/api/tags", self.config.base_url)).send()
        )
        .await
        .map_err(|_| CodeGraphError::Timeout("Qwen availability check timeout".to_string()))?
        .map_err(|e| CodeGraphError::Network(format!("Qwen availability check failed: {}", e)))?;

        if !response.status().is_success() {
            return Ok(false);
        }

        let models: serde_json::Value = response
            .json()
            .await
            .map_err(|_| CodeGraphError::Parse("Failed to parse models response".to_string()))?;

        let has_qwen = models["models"]
            .as_array()
            .map(|models| {
                models.iter().any(|model| {
                    model["name"]
                        .as_str()
                        .map(|name| name.contains("qwen") && name.contains("coder"))
                        .unwrap_or(false)
                })
            })
            .unwrap_or(false);

        info!("Qwen2.5-Coder availability: {}", has_qwen);
        Ok(has_qwen)
    }

    /// Get model information and capabilities
    pub async fn get_model_info(&self) -> Result<QwenModelInfo> {
        let response = self.client
            .get(&format!("{}/api/show", self.config.base_url))
            .json(&serde_json::json!({
                "name": self.config.model_name
            }))
            .send()
            .await
            .map_err(|e| CodeGraphError::Network(format!("Failed to get model info: {}", e)))?;

        if !response.status().is_success() {
            return Err(CodeGraphError::External("Model not found".to_string()));
        }

        let model_data: serde_json::Value = response
            .json()
            .await
            .map_err(|e| CodeGraphError::Parse(format!("Failed to parse model info: {}", e)))?;

        Ok(QwenModelInfo {
            model_name: self.config.model_name.clone(),
            parameters: "14B".to_string(),
            quantization: "Q4_K_M".to_string(),
            context_window: self.config.context_window,
            specialization: "Code Analysis and Understanding".to_string(),
            memory_requirement: "~24GB VRAM".to_string(),
        })
    }

    fn calculate_confidence(&self, response: &OllamaResponse) -> f32 {
        // Calculate confidence based on response characteristics
        let content = &response.message.content;
        let mut confidence: f32 = 0.5;

        // Structured response indicates higher confidence
        if content.contains("1.") && content.contains("2.") {
            confidence += 0.2;
        }

        // Code examples indicate thorough analysis
        if content.contains("```") {
            confidence += 0.1;
        }

        // Detailed responses indicate comprehensive analysis
        if content.len() > 1000 {
            confidence += 0.1;
        }

        // Technical terminology indicates code understanding
        if content.contains("function") || content.contains("class") || content.contains("module") {
            confidence += 0.1;
        }

        confidence.min(0.95) // Cap at 95%
    }
}

#[derive(Debug, Clone)]
pub struct QwenModelInfo {
    pub model_name: String,
    pub parameters: String,
    pub quantization: String,
    pub context_window: usize,
    pub specialization: String,
    pub memory_requirement: String,
}

/// Trait for CodeGraph intelligence analysis
#[async_trait]
pub trait CodeIntelligenceProvider {
    async fn analyze_semantic_context(&self, query: &str, context: &str) -> Result<String>;
    async fn detect_patterns(&self, code_samples: &[String]) -> Result<String>;
    async fn analyze_impact(&self, target_code: &str, dependencies: &str) -> Result<String>;
    async fn build_generation_context(&self, request: &str, codebase_context: &str) -> Result<String>;
}

#[async_trait]
impl CodeIntelligenceProvider for QwenClient {
    async fn analyze_semantic_context(&self, query: &str, context: &str) -> Result<String> {
        let prompt = format!(
            "Analyze this codebase context for semantic understanding:\n\n\
            SEARCH QUERY: {}\n\n\
            CODEBASE CONTEXT:\n{}\n\n\
            Provide structured semantic analysis:\n\
            1. SEMANTIC_MATCHES: What code semantically matches the query and why\n\
            2. ARCHITECTURAL_CONTEXT: How this functionality fits in the system\n\
            3. USAGE_PATTERNS: How this code is typically used and integrated\n\
            4. GENERATION_GUIDANCE: How to generate similar high-quality code\n\n\
            Focus on actionable insights for code generation and understanding.",
            query, context
        );

        let result = self.generate_analysis(&prompt, Some(
            "You are providing semantic code analysis for powerful LLMs. Focus on understanding code purpose, patterns, and architectural context."
        )).await?;

        Ok(result.text)
    }

    async fn detect_patterns(&self, code_samples: &[String]) -> Result<String> {
        let prompt = format!(
            "Analyze these code samples for patterns and conventions:\n\n\
            CODE SAMPLES:\n{}\n\n\
            Provide structured pattern analysis:\n\
            1. IDENTIFIED_PATTERNS: What consistent patterns are used\n\
            2. QUALITY_ASSESSMENT: Quality and adherence to best practices\n\
            3. TEAM_CONVENTIONS: Team-specific conventions and standards\n\
            4. GENERATION_TEMPLATES: How to generate code following these patterns\n\n\
            Focus on actionable guidance for consistent code generation.",
            code_samples.join("\n---\n")
        );

        let result = self.generate_analysis(&prompt, Some(
            "You are analyzing code patterns for consistency and quality. Provide actionable guidance for code generation."
        )).await?;

        Ok(result.text)
    }

    async fn analyze_impact(&self, target_code: &str, dependencies: &str) -> Result<String> {
        let prompt = format!(
            "Analyze the impact of modifying this code:\n\n\
            TARGET CODE:\n{}\n\n\
            DEPENDENCIES:\n{}\n\n\
            Provide structured impact analysis:\n\
            1. RISK_ASSESSMENT: Risk level and reasoning\n\
            2. AFFECTED_COMPONENTS: What will be impacted\n\
            3. TESTING_REQUIREMENTS: Tests that need updating\n\
            4. SAFETY_RECOMMENDATIONS: How to make changes safely\n\n\
            Focus on safe implementation guidance.",
            target_code, dependencies
        );

        let result = self.generate_analysis(&prompt, Some(
            "You are providing critical impact analysis for code changes. Prioritize safety and thoroughness."
        )).await?;

        Ok(result.text)
    }

    async fn build_generation_context(&self, request: &str, codebase_context: &str) -> Result<String> {
        let prompt = format!(
            "Build comprehensive context for code generation:\n\n\
            GENERATION REQUEST: {}\n\n\
            CODEBASE CONTEXT:\n{}\n\n\
            Provide structured generation context:\n\
            1. CONTEXT_SUMMARY: Key context for this generation request\n\
            2. REQUIRED_PATTERNS: Patterns that must be followed\n\
            3. INTEGRATION_REQUIREMENTS: How code should integrate\n\
            4. QUALITY_STANDARDS: Standards that must be met\n\n\
            Focus on enabling perfect code generation.",
            request, codebase_context
        );

        let result = self.generate_analysis(&prompt, Some(
            "You are building context to enable powerful LLMs to generate perfect code. Be comprehensive and specific."
        )).await?;

        Ok(result.text)
    }
}