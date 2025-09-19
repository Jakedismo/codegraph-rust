use codegraph_core::{CodeGraphError, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};
use tokio::time::timeout;
use tracing::{debug, info, warn};

/// Simple Qwen2.5-Coder client for MCP integration
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
            model_name: std::env::var("CODEGRAPH_MODEL")
                .unwrap_or_else(|_| "hf.co/unsloth/Qwen2.5-Coder-14B-Instruct-128K-GGUF:Q4_K_M".to_string()),
            base_url: "http://localhost:11434".to_string(),
            context_window: 128000,
            max_tokens: 8192,
            temperature: 0.1,
            timeout: Duration::from_secs(90),
        }
    }
}

#[derive(Debug, Serialize)]
struct OllamaRequest {
    model: String,
    prompt: String,
    stream: bool,
    options: OllamaOptions,
}

#[derive(Debug, Serialize)]
struct OllamaOptions {
    temperature: f32,
    num_predict: usize,
    num_ctx: usize,
}

#[derive(Debug, Deserialize)]
struct OllamaResponse {
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

#[derive(Clone)]
pub struct QwenClient {
    client: Client,
    pub config: QwenConfig,
}

impl QwenClient {
    pub fn new(config: QwenConfig) -> Self {
        Self {
            client: Client::new(),
            config,
        }
    }

    /// Generate semantic analysis using Qwen2.5-Coder with optimized prompts
    pub async fn analyze_codebase(&self, query: &str, context: &str) -> Result<QwenResult> {
        let start_time = Instant::now();

        // Use optimized prompt structure for Qwen2.5-Coder
        let prompt = crate::prompts::build_semantic_analysis_prompt(query, context);

        let request = OllamaRequest {
            model: self.config.model_name.clone(),
            prompt,
            stream: false,
            options: OllamaOptions {
                temperature: self.config.temperature,
                num_predict: self.config.max_tokens,
                num_ctx: self.config.context_window,
            },
        };

        debug!("Sending analysis request to Qwen2.5-Coder: {} context window", self.config.context_window);

        let response = timeout(
            self.config.timeout,
            self.client
                .post(&format!("{}/api/generate", self.config.base_url))
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
            "Qwen analysis completed: {}ms, context: {} tokens, completion: {} tokens, confidence: {:.2}",
            processing_time.as_millis(),
            result.context_tokens,
            result.completion_tokens,
            result.confidence_score
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
                        .map(|name| {
                            name.contains("qwen") && name.contains("coder") ||
                            name.contains("qwen2.5-coder")
                        })
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
        if response.contains("function") || response.contains("class") || response.contains("module") {
            confidence += 0.1;
        }

        confidence.min(0.95) // Cap at 95%
    }
}