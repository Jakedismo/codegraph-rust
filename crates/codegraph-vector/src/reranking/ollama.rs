// ABOUTME: Ollama chat-based reranking using models like Qwen3-Reranker
// ABOUTME: Uses chat completions with prompting to score document relevance
use super::{RerankDocument, RerankResult, Reranker};
use anyhow::{Context, Result};
use async_trait::async_trait;
use codegraph_core::RerankConfig;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tracing::{debug, info, warn};

/// Ollama chat request structure
#[derive(Debug, Serialize)]
struct OllamaChatRequest {
    model: String,
    messages: Vec<ChatMessage>,
    stream: bool,
    options: ChatOptions,
}

#[derive(Debug, Serialize)]
struct ChatMessage {
    role: String,
    content: String,
}

#[derive(Debug, Serialize)]
struct ChatOptions {
    temperature: f32,
}

/// Ollama chat response structure
#[derive(Debug, Deserialize)]
struct OllamaChatResponse {
    model: String,
    message: ResponseMessage,
    done: bool,
}

#[derive(Debug, Deserialize)]
struct ResponseMessage {
    role: String,
    content: String,
}

/// Ollama reranker implementation using chat completions
pub struct OllamaReranker {
    client: Client,
    api_base: String,
    model: String,
    max_retries: usize,
    timeout: Duration,
    temperature: f32,
}

impl OllamaReranker {
    pub fn new(config: &RerankConfig) -> Result<Self> {
        let ollama_config = config
            .ollama
            .as_ref()
            .context("Ollama reranking configuration is required")?;

        Ok(Self {
            client: Client::new(),
            api_base: ollama_config.api_base.clone(),
            model: ollama_config.model.clone(),
            max_retries: ollama_config.max_retries as usize,
            timeout: Duration::from_secs(ollama_config.timeout_secs),
            temperature: ollama_config.temperature,
        })
    }

    /// Score a single document using chat completion
    async fn score_document(&self, query: &str, document: &str) -> Result<f32> {
        let prompt = format!(
            r#"You are an expert relevance grader. Your task is to evaluate if the following document is relevant to the user's query.
Please respond with ONLY a number between 0 and 1, where:
- 0.0 means completely irrelevant
- 0.5 means somewhat relevant
- 1.0 means highly relevant

Query: {}

Document: {}

Relevance score:"#,
            query, document
        );

        let request = OllamaChatRequest {
            model: self.model.clone(),
            messages: vec![ChatMessage {
                role: "user".to_string(),
                content: prompt,
            }],
            stream: false,
            options: ChatOptions {
                temperature: self.temperature,
            },
        };

        let mut last_error = None;

        for attempt in 0..=self.max_retries {
            if attempt > 0 {
                tokio::time::sleep(Duration::from_millis(100 * (1 << attempt))).await;
            }

            let request_result = tokio::time::timeout(
                self.timeout,
                self.client
                    .post(&format!("{}/api/chat", self.api_base))
                    .header("Content-Type", "application/json")
                    .json(&request)
                    .send(),
            )
            .await;

            match request_result {
                Ok(Ok(response)) => {
                    if response.status().is_success() {
                        match response.json::<OllamaChatResponse>().await {
                            Ok(chat_response) => {
                                let content = chat_response.message.content.trim();

                                // Try to parse the score from the response
                                // Handle various formats: "0.95", "Score: 0.95", etc.
                                let score_str = content
                                    .split_whitespace()
                                    .find_map(|s| s.parse::<f32>().ok())
                                    .or_else(|| content.parse::<f32>().ok());

                                if let Some(score) = score_str {
                                    // Clamp score to 0.0-1.0 range
                                    let clamped_score = score.clamp(0.0, 1.0);
                                    debug!(
                                        "Ollama rerank score: {:.3} (raw: {})",
                                        clamped_score, content
                                    );
                                    return Ok(clamped_score);
                                } else {
                                    // Fallback: Try to detect Yes/No/Maybe
                                    let content_lower = content.to_lowercase();
                                    let score = if content_lower.contains("yes")
                                        || content_lower.contains("relevant")
                                    {
                                        1.0
                                    } else if content_lower.contains("no")
                                        || content_lower.contains("irrelevant")
                                    {
                                        0.0
                                    } else if content_lower.contains("maybe")
                                        || content_lower.contains("somewhat")
                                    {
                                        0.5
                                    } else {
                                        warn!(
                                            "Could not parse Ollama response as score: {}",
                                            content
                                        );
                                        0.5 // Default to neutral
                                    };

                                    return Ok(score);
                                }
                            }
                            Err(e) => {
                                last_error = Some(anyhow::anyhow!(
                                    "Failed to parse Ollama response: {}",
                                    e
                                ));
                            }
                        }
                    } else {
                        let status = response.status();
                        let error_text = response.text().await.unwrap_or_default();
                        last_error = Some(anyhow::anyhow!(
                            "Ollama API error: HTTP {} - {}",
                            status,
                            error_text
                        ));
                    }
                }
                Ok(Err(e)) => {
                    last_error = Some(anyhow::anyhow!("Request failed: {}", e));
                }
                Err(_) => {
                    last_error = Some(anyhow::anyhow!("Ollama API request timed out"));
                }
            }

            if attempt < self.max_retries {
                warn!(
                    "Ollama rerank call failed (attempt {}/{}), retrying...",
                    attempt + 1,
                    self.max_retries + 1
                );
            }
        }

        Err(last_error.unwrap_or_else(|| {
            anyhow::anyhow!("All Ollama rerank retry attempts failed")
        }))
    }
}

#[async_trait]
impl Reranker for OllamaReranker {
    async fn rerank(
        &self,
        query: &str,
        documents: Vec<RerankDocument>,
        top_n: usize,
    ) -> Result<Vec<RerankResult>> {
        if documents.is_empty() {
            return Ok(Vec::new());
        }

        info!(
            "Ollama reranking {} documents with model: {}",
            documents.len(),
            self.model
        );

        // Score each document
        let mut scored_docs = Vec::new();
        for (index, doc) in documents.iter().enumerate() {
            match self.score_document(query, &doc.text).await {
                Ok(score) => {
                    scored_docs.push(RerankResult {
                        id: doc.id.clone(),
                        score,
                        index,
                        metadata: doc.metadata.clone(),
                    });
                }
                Err(e) => {
                    warn!("Failed to score document {}: {}", doc.id, e);
                    // Include with low score rather than failing entirely
                    scored_docs.push(RerankResult {
                        id: doc.id.clone(),
                        score: 0.0,
                        index,
                        metadata: doc.metadata.clone(),
                    });
                }
            }
        }

        // Sort by score descending
        scored_docs.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));

        // Take top N
        scored_docs.truncate(top_n);

        info!(
            "Ollama reranking complete: {} results (top score: {:.3})",
            scored_docs.len(),
            scored_docs.first().map(|r| r.score).unwrap_or(0.0)
        );

        Ok(scored_docs)
    }

    fn model_name(&self) -> &str {
        &self.model
    }

    fn provider_name(&self) -> &str {
        "ollama"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use codegraph_core::{OllamaRerankConfig, RerankProvider};

    #[test]
    fn test_ollama_reranker_creation() {
        let config = RerankConfig {
            provider: RerankProvider::Ollama,
            top_n: 10,
            jina: None,
            ollama: Some(OllamaRerankConfig::default()),
        };

        let result = OllamaReranker::new(&config);
        assert!(result.is_ok());
    }

    #[test]
    fn test_chat_request_serialization() {
        let request = OllamaChatRequest {
            model: "dengcao/Qwen3-Reranker-4B:Q5_K_M".to_string(),
            messages: vec![ChatMessage {
                role: "user".to_string(),
                content: "test".to_string(),
            }],
            stream: false,
            options: ChatOptions { temperature: 0.0 },
        };

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("Qwen3-Reranker"));
        assert!(json.contains("\"stream\":false"));
    }
}
