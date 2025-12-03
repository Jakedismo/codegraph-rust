// ABOUTME: Ollama chat-based reranking using models like Qwen3-Reranker
// ABOUTME: Batches all documents into single prompt for efficient scoring
use super::{RerankDocument, RerankResult, Reranker};
use anyhow::{Context, Result};
use async_trait::async_trait;
use codegraph_core::RerankConfig;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
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
    #[allow(dead_code)]
    model: String,
    message: ResponseMessage,
    #[allow(dead_code)]
    done: bool,
}

#[derive(Debug, Deserialize)]
struct ResponseMessage {
    #[allow(dead_code)]
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

    /// Build batched prompt with all documents
    fn build_batch_prompt(query: &str, documents: &[RerankDocument]) -> String {
        let mut prompt = format!(
            r#"You are an expert relevance grader. Score each document's relevance to the query.

Query: {}

Documents:
"#,
            query
        );

        for (i, doc) in documents.iter().enumerate() {
            // Truncate very long documents to avoid context overflow
            let text = if doc.text.len() > 2000 {
                format!("{}...", &doc.text[..2000])
            } else {
                doc.text.clone()
            };
            prompt.push_str(&format!("\n[DOC{}] {}\n", i, text));
        }

        prompt.push_str(
            r#"
Return ONLY a JSON object mapping document numbers to relevance scores (0.0 to 1.0).
Example format: {"0": 0.95, "1": 0.2, "2": 0.8}

Scores:"#,
        );

        prompt
    }

    /// Parse batched scores from model response
    fn parse_batch_scores(content: &str, doc_count: usize) -> HashMap<usize, f32> {
        let mut scores = HashMap::new();

        // Try to find JSON object in response
        let json_start = content.find('{');
        let json_end = content.rfind('}');

        if let (Some(start), Some(end)) = (json_start, json_end) {
            let json_str = &content[start..=end];
            if let Ok(parsed) = serde_json::from_str::<HashMap<String, serde_json::Value>>(json_str)
            {
                for (key, value) in parsed {
                    if let Ok(idx) = key.parse::<usize>() {
                        let score = match value {
                            serde_json::Value::Number(n) => n.as_f64().unwrap_or(0.5) as f32,
                            serde_json::Value::String(s) => s.parse().unwrap_or(0.5),
                            _ => 0.5,
                        };
                        scores.insert(idx, score.clamp(0.0, 1.0));
                    }
                }
            }
        }

        // Fallback: try to parse line-by-line scores like "0: 0.95"
        if scores.is_empty() {
            for line in content.lines() {
                let parts: Vec<&str> = line.split(':').collect();
                if parts.len() == 2 {
                    if let Ok(idx) = parts[0].trim().trim_matches(|c| c == '"' || c == '[' || c == ']' || c == 'D' || c == 'O' || c == 'C').parse::<usize>() {
                        if let Ok(score) = parts[1].trim().trim_matches(|c| c == ',' || c == '"').parse::<f32>() {
                            scores.insert(idx, score.clamp(0.0, 1.0));
                        }
                    }
                }
            }
        }

        // Fill missing scores with default
        for i in 0..doc_count {
            scores.entry(i).or_insert(0.5);
        }

        scores
    }

    /// Score all documents in a single batched request
    async fn score_documents_batch(
        &self,
        query: &str,
        documents: &[RerankDocument],
    ) -> Result<HashMap<usize, f32>> {
        let prompt = Self::build_batch_prompt(query, documents);

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
                                debug!("Ollama batch rerank response: {}", content);

                                let scores = Self::parse_batch_scores(content, documents.len());
                                return Ok(scores);
                            }
                            Err(e) => {
                                last_error =
                                    Some(anyhow::anyhow!("Failed to parse Ollama response: {}", e));
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
                    "Ollama batch rerank failed (attempt {}/{}), retrying...",
                    attempt + 1,
                    self.max_retries + 1
                );
            }
        }

        Err(last_error
            .unwrap_or_else(|| anyhow::anyhow!("All Ollama rerank retry attempts failed")))
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
            "Ollama batch reranking {} documents with model: {}",
            documents.len(),
            self.model
        );

        // Score all documents in single batch request
        let scores = self.score_documents_batch(query, &documents).await?;

        // Build results with scores
        let mut scored_docs: Vec<RerankResult> = documents
            .iter()
            .enumerate()
            .map(|(index, doc)| {
                let score = scores.get(&index).copied().unwrap_or(0.5);
                RerankResult {
                    id: doc.id.clone(),
                    score,
                    index,
                    metadata: doc.metadata.clone(),
                }
            })
            .collect();

        // Sort by score descending
        scored_docs.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // Take top N
        scored_docs.truncate(top_n);

        info!(
            "Ollama batch reranking complete: {} results (top score: {:.3})",
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
    fn test_build_batch_prompt() {
        let docs = vec![
            RerankDocument {
                id: "1".to_string(),
                text: "Rust error handling".to_string(),
                metadata: None,
            },
            RerankDocument {
                id: "2".to_string(),
                text: "Python exceptions".to_string(),
                metadata: None,
            },
        ];

        let prompt = OllamaReranker::build_batch_prompt("error handling", &docs);
        assert!(prompt.contains("[DOC0]"));
        assert!(prompt.contains("[DOC1]"));
        assert!(prompt.contains("Rust error handling"));
        assert!(prompt.contains("Python exceptions"));
    }

    #[test]
    fn test_parse_batch_scores_json() {
        let response = r#"{"0": 0.95, "1": 0.2, "2": 0.8}"#;
        let scores = OllamaReranker::parse_batch_scores(response, 3);

        assert!((scores[&0] - 0.95).abs() < 0.01);
        assert!((scores[&1] - 0.2).abs() < 0.01);
        assert!((scores[&2] - 0.8).abs() < 0.01);
    }

    #[test]
    fn test_parse_batch_scores_with_text() {
        let response = r#"Based on relevance analysis:
{"0": 0.9, "1": 0.1}
The first document is highly relevant."#;
        let scores = OllamaReranker::parse_batch_scores(response, 2);

        assert!((scores[&0] - 0.9).abs() < 0.01);
        assert!((scores[&1] - 0.1).abs() < 0.01);
    }

    #[test]
    fn test_parse_batch_scores_fills_missing() {
        let response = r#"{"0": 0.9}"#;
        let scores = OllamaReranker::parse_batch_scores(response, 3);

        assert!((scores[&0] - 0.9).abs() < 0.01);
        assert!((scores[&1] - 0.5).abs() < 0.01); // Default
        assert!((scores[&2] - 0.5).abs() < 0.01); // Default
    }

    #[test]
    fn test_chat_request_serialization() {
        let request = OllamaChatRequest {
            model: "dengcao/Qwen3-Reranker-8B:Q3_K_M".to_string(),
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
