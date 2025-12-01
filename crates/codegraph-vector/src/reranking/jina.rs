// ABOUTME: Jina AI reranking API implementation using jina-reranker-v3
// ABOUTME: Provides text-based cross-encoder reranking via Jina's HTTP API
use super::{RerankDocument, RerankResult, Reranker};
use anyhow::{Context, Result};
use async_trait::async_trait;
use codegraph_core::RerankConfig;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tracing::{info, warn};

/// Jina reranking request structure
#[derive(Debug, Serialize)]
struct JinaRerankRequest {
    model: String,
    query: String,
    documents: Vec<String>,
    top_n: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    return_documents: Option<bool>,
}

/// Jina reranking response structure
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct JinaRerankResponse {
    model: String,
    results: Vec<JinaRerankResult>,
    usage: JinaRerankUsage,
}

#[derive(Debug, Deserialize)]
struct JinaRerankResult {
    index: usize,
    relevance_score: f32,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct JinaRerankUsage {
    total_tokens: usize,
    #[serde(default)]
    prompt_tokens: Option<usize>,
}

/// Error response from Jina API
#[derive(Debug, Deserialize)]
struct JinaApiError {
    detail: Option<String>,
    message: Option<String>,
}

/// Jina reranker implementation using Jina AI API
pub struct JinaReranker {
    client: Client,
    api_key: String,
    api_base: String,
    model: String,
    max_retries: usize,
    timeout: Duration,
}

impl JinaReranker {
    pub fn new(config: &RerankConfig) -> Result<Self> {
        let jina_config = config
            .jina
            .as_ref()
            .context("Jina reranking configuration is required")?;

        // Get API key from environment variable
        let api_key = std::env::var(&jina_config.api_key_env).with_context(|| {
            format!(
                "Jina API key not found in environment variable: {}",
                jina_config.api_key_env
            )
        })?;

        Ok(Self {
            client: Client::new(),
            api_key,
            api_base: jina_config.api_base.clone(),
            model: jina_config.model.clone(),
            max_retries: jina_config.max_retries as usize,
            timeout: Duration::from_secs(jina_config.timeout_secs),
        })
    }
}

#[async_trait]
impl Reranker for JinaReranker {
    async fn rerank(
        &self,
        query: &str,
        documents: Vec<RerankDocument>,
        top_n: usize,
    ) -> Result<Vec<RerankResult>> {
        if documents.is_empty() {
            return Ok(Vec::new());
        }

        // Extract text content from documents
        let document_texts: Vec<String> = documents.iter().map(|d| d.text.clone()).collect();

        let request = JinaRerankRequest {
            model: self.model.clone(),
            query: query.to_string(),
            documents: document_texts,
            top_n,
            return_documents: Some(false),
        };

        let mut last_error = None;

        for attempt in 0..=self.max_retries {
            if attempt > 0 {
                tokio::time::sleep(Duration::from_millis(100 * (1 << attempt))).await;
            }

            let request_result = tokio::time::timeout(
                self.timeout,
                self.client
                    .post(&format!("{}/rerank", self.api_base))
                    .header("Authorization", format!("Bearer {}", self.api_key))
                    .header("Content-Type", "application/json")
                    .json(&request)
                    .send(),
            )
            .await;

            match request_result {
                Ok(Ok(response)) => {
                    if response.status().is_success() {
                        match response.json::<JinaRerankResponse>().await {
                            Ok(jina_response) => {
                                info!(
                                    "Jina rerank API call successful: {} results (model: {})",
                                    jina_response.results.len(),
                                    jina_response.model
                                );

                                // Convert Jina results to our format
                                let results: Vec<RerankResult> = jina_response
                                    .results
                                    .into_iter()
                                    .map(|jr| {
                                        let doc = &documents[jr.index];
                                        RerankResult {
                                            id: doc.id.clone(),
                                            score: jr.relevance_score,
                                            index: jr.index,
                                            metadata: doc.metadata.clone(),
                                        }
                                    })
                                    .collect();

                                return Ok(results);
                            }
                            Err(e) => {
                                last_error = Some(anyhow::anyhow!(
                                    "Failed to parse Jina rerank response: {}",
                                    e
                                ));
                            }
                        }
                    } else {
                        let status = response.status();
                        if let Ok(api_error) = response.json::<JinaApiError>().await {
                            let error_msg = api_error
                                .detail
                                .or(api_error.message)
                                .unwrap_or_else(|| "Unknown error".to_string());
                            last_error =
                                Some(anyhow::anyhow!("Jina rerank API error: {}", error_msg));
                        } else {
                            last_error =
                                Some(anyhow::anyhow!("Jina rerank API error: HTTP {}", status));
                        }
                    }
                }
                Ok(Err(e)) => {
                    last_error = Some(anyhow::anyhow!("Request failed: {}", e));
                }
                Err(_) => {
                    last_error = Some(anyhow::anyhow!("Jina rerank API request timed out"));
                }
            }

            if attempt < self.max_retries {
                warn!(
                    "Jina rerank API call failed (attempt {}/{}), retrying...",
                    attempt + 1,
                    self.max_retries + 1
                );
            }
        }

        Err(last_error
            .unwrap_or_else(|| anyhow::anyhow!("All Jina rerank API retry attempts failed")))
    }

    fn model_name(&self) -> &str {
        &self.model
    }

    fn provider_name(&self) -> &str {
        "jina"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use codegraph_core::{JinaRerankConfig, RerankProvider};

    #[test]
    fn test_jina_reranker_creation_without_api_key() {
        let config = RerankConfig {
            provider: RerankProvider::Jina,
            top_n: 10,
            jina: Some(JinaRerankConfig::default()),
            ollama: None,
        };

        // This should fail if JINA_API_KEY is not set
        let result = JinaReranker::new(&config);
        if std::env::var("JINA_API_KEY").is_err() {
            assert!(result.is_err());
        }
    }

    #[test]
    fn test_request_serialization() {
        let request = JinaRerankRequest {
            model: "jina-reranker-v3".to_string(),
            query: "test query".to_string(),
            documents: vec!["doc1".to_string(), "doc2".to_string()],
            top_n: 2,
            return_documents: Some(false),
        };

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("jina-reranker-v3"));
        assert!(json.contains("test query"));
        assert!(json.contains("doc1"));
    }
}
