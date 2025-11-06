use crate::llm_provider::*;
use async_trait::async_trait;
use codegraph_core::{CodeGraphError, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};
use tokio::time::timeout;
use tracing::{debug, info, warn};

/// Simple Qwen2.5-Coder client for CodeGraph MCP integration
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

#[derive(Debug, Serialize)]
struct SimpleRequest {
    model: String,
    prompt: String,
    stream: bool,
    options: SimpleOptions,
}

#[derive(Debug, Serialize)]
struct SimpleOptions {
    temperature: f32,
    num_predict: usize,
    num_ctx: usize,
}

#[derive(Debug, Deserialize)]
struct SimpleResponse {
    response: String,
    #[serde(default)]
    eval_count: Option<usize>,
    #[serde(default)]
    prompt_eval_count: Option<usize>,
}

#[derive(Debug, Clone)]
pub struct QwenResult {
    pub text: String,
    pub model_used: String,
    pub processing_time: Duration,
    pub context_tokens: usize,
    pub completion_tokens: usize,
    pub confidence_score: f32,
}

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
    pub async fn generate_analysis(
        &self,
        prompt: &str,
        system_prompt: Option<&str>,
    ) -> Result<QwenResult> {
        let start_time = Instant::now();

        // Build full prompt with system context if provided
        let full_prompt = if let Some(sys_prompt) = system_prompt {
            format!("{}\n\nUser: {}\n\nAssistant:", sys_prompt, prompt)
        } else {
            format!("You are Qwen2.5-Coder, a state-of-the-art AI specialized in comprehensive code analysis. Provide detailed, structured analysis.\n\nUser: {}\n\nAssistant:", prompt)
        };

        let request = SimpleRequest {
            model: self.config.model_name.clone(),
            prompt: full_prompt,
            stream: false,
            options: SimpleOptions {
                temperature: self.config.temperature,
                num_predict: self.config.max_tokens,
                num_ctx: self.config.context_window,
            },
        };

        debug!(
            "Sending request to Qwen2.5-Coder: {} context window",
            self.config.context_window
        );

        let response = timeout(
            self.config.timeout,
            self.client
                .post(&format!("{}/api/generate", self.config.base_url))
                .json(&request)
                .send(),
        )
        .await
        .map_err(|_| {
            CodeGraphError::Timeout(format!(
                "Qwen request timeout after {:?}",
                self.config.timeout
            ))
        })?
        .map_err(|e| CodeGraphError::Network(format!("Qwen request failed: {}", e)))?;

        if !response.status().is_success() {
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(CodeGraphError::External(format!(
                "Qwen API error: {}",
                error_text
            )));
        }

        let response_data: SimpleResponse = response
            .json()
            .await
            .map_err(|e| CodeGraphError::Parse(format!("Failed to parse Qwen response: {}", e)))?;

        let processing_time = start_time.elapsed();

        let confidence_score = self.calculate_confidence(&response_data.response);

        let result = QwenResult {
            text: response_data.response,
            model_used: self.config.model_name.clone(),
            processing_time,
            context_tokens: response_data.prompt_eval_count.unwrap_or(0),
            completion_tokens: response_data.eval_count.unwrap_or(0),
            confidence_score,
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
        debug!(
            "Checking Qwen2.5-Coder availability at {}",
            self.config.base_url
        );

        let response = timeout(
            Duration::from_secs(5),
            self.client
                .get(&format!("{}/api/tags", self.config.base_url))
                .send(),
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

    fn calculate_confidence(&self, response: &str) -> f32 {
        let mut confidence: f32 = 0.5;

        // Structured response indicates higher confidence
        if response.contains("1.") && response.contains("2.") {
            confidence += 0.2;
        }

        // Code examples indicate thorough analysis
        if response.contains("```") {
            confidence += 0.1;
        }

        // Detailed responses indicate comprehensive analysis
        if response.len() > 1000 {
            confidence += 0.1;
        }

        // Technical terminology indicates code understanding
        if response.contains("function")
            || response.contains("class")
            || response.contains("module")
        {
            confidence += 0.1;
        }

        confidence.min(0.95) // Cap at 95%
    }
}

// LLMProvider trait implementation for QwenClient
#[async_trait]
impl LLMProvider for QwenClient {
    async fn generate_chat(
        &self,
        messages: &[Message],
        config: &GenerationConfig,
    ) -> LLMResult<LLMResponse> {
        // Build prompt from messages
        let prompt = messages
            .iter()
            .map(|m| match m.role {
                MessageRole::System => format!("System: {}", m.content),
                MessageRole::User => format!("User: {}", m.content),
                MessageRole::Assistant => format!("Assistant: {}", m.content),
            })
            .collect::<Vec<_>>()
            .join("\n\n");

        let prompt = format!("{}\n\nAssistant:", prompt);

        let start_time = Instant::now();
        let request = SimpleRequest {
            model: self.config.model_name.clone(),
            prompt,
            stream: false,
            options: SimpleOptions {
                temperature: config.temperature,
                num_predict: config.max_tokens.unwrap_or(self.config.max_tokens),
                num_ctx: self.config.context_window,
            },
        };

        let response = timeout(
            self.config.timeout,
            self.client
                .post(&format!("{}/api/generate", self.config.base_url))
                .json(&request)
                .send(),
        )
        .await
        .map_err(|_| anyhow::anyhow!("Qwen request timeout"))?
        .map_err(|e| anyhow::anyhow!("Qwen request failed: {}", e))?;

        if !response.status().is_success() {
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(anyhow::anyhow!("Qwen API error: {}", error_text));
        }

        let response_data: SimpleResponse = response
            .json()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to parse Qwen response: {}", e))?;

        Ok(LLMResponse {
            content: response_data.response,
            total_tokens: Some(
                response_data.prompt_eval_count.unwrap_or(0)
                    + response_data.eval_count.unwrap_or(0),
            ),
            prompt_tokens: response_data.prompt_eval_count,
            completion_tokens: response_data.eval_count,
            finish_reason: Some("stop".to_string()),
            model: self.config.model_name.clone(),
        })
    }

    async fn is_available(&self) -> bool {
        self.check_availability().await.unwrap_or(false)
    }

    fn provider_name(&self) -> &str {
        "qwen"
    }

    fn model_name(&self) -> &str {
        &self.config.model_name
    }

    fn characteristics(&self) -> ProviderCharacteristics {
        ProviderCharacteristics {
            max_tokens: self.config.context_window,
            avg_latency_ms: 2000,
            rpm_limit: None,
            tpm_limit: None,
            supports_streaming: true,
            supports_functions: false,
        }
    }
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
}
