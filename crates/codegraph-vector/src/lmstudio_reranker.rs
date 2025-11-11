use crate::{CodeGraphError, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Clone, Debug)]
pub struct LmStudioRerankerConfig {
    pub base_url: String,
    pub api_key: String,
    pub model: String,
    pub timeout: Duration,
}

impl Default for LmStudioRerankerConfig {
    fn default() -> Self {
        Self {
            base_url: std::env::var("LMSTUDIO_URL")
                .unwrap_or_else(|_| "http://localhost:1234/v1".to_string()),
            api_key: std::env::var("LMSTUDIO_API_KEY").unwrap_or_else(|_| "lm-studio".to_string()),
            model: std::env::var("LMSTUDIO_RERANK_MODEL")
                .unwrap_or_else(|_| "text-embedding-3-small".to_string()),
            timeout: Duration::from_secs(60),
        }
    }
}

#[derive(Clone)]
pub struct LmStudioReranker {
    client: Client,
    config: LmStudioRerankerConfig,
}

#[derive(Serialize)]
struct EmbeddingRequest {
    model: String,
    input: Vec<String>,
}

#[derive(Deserialize)]
struct EmbeddingResponse {
    data: Vec<EmbeddingData>,
}

#[derive(Deserialize)]
struct EmbeddingData {
    embedding: Vec<f32>,
}

pub struct LmStudioRerankResult {
    pub index: usize,
    pub score: f32,
}

impl LmStudioReranker {
    pub fn new(config: LmStudioRerankerConfig) -> Self {
        let client = Client::builder()
            .timeout(config.timeout)
            .build()
            .expect("Failed to build LM Studio client");
        Self { client, config }
    }

    pub fn from_env() -> Option<Self> {
        if std::env::var("CODEGRAPH_RERANKING_PROVIDER")
            .map(|v| v.eq_ignore_ascii_case("lmstudio"))
            .unwrap_or(false)
        {
            Some(Self::new(LmStudioRerankerConfig::default()))
        } else {
            None
        }
    }

    pub async fn rerank(
        &self,
        query: &str,
        documents: &[String],
    ) -> Result<Vec<LmStudioRerankResult>> {
        if documents.is_empty() {
            return Ok(Vec::new());
        }

        let mut inputs = Vec::with_capacity(documents.len() + 1);
        inputs.push(query.replace('\n', " "));
        inputs.extend(documents.iter().map(|d| d.replace('\n', " ")));

        let request = EmbeddingRequest {
            model: self.config.model.clone(),
            input: inputs.clone(),
        };

        let url = format!("{}/embeddings", self.config.base_url.trim_end_matches('/'));
        let response = self
            .client
            .post(&url)
            .bearer_auth(&self.config.api_key)
            .json(&request)
            .send()
            .await
            .map_err(|e| {
                CodeGraphError::External(format!("LM Studio rerank request failed: {}", e))
            })?;

        if !response.status().is_success() {
            return Err(CodeGraphError::External(format!(
                "LM Studio rerank API error: {}",
                response.status()
            )));
        }

        let embedding_response: EmbeddingResponse = response.json().await.map_err(|e| {
            CodeGraphError::External(format!(
                "Failed to decode LM Studio embedding response: {}",
                e
            ))
        })?;

        if embedding_response.data.len() != inputs.len() {
            return Err(CodeGraphError::External(
                "LM Studio embedding response size mismatch".to_string(),
            ));
        }

        let query_embedding = embedding_response.data[0].embedding.clone();
        let doc_embeddings: Vec<Vec<f32>> = embedding_response.data[1..]
            .iter()
            .map(|d| d.embedding.clone())
            .collect();

        let mut results: Vec<LmStudioRerankResult> = doc_embeddings
            .into_iter()
            .enumerate()
            .map(|(idx, emb)| {
                let score = cosine_similarity(&query_embedding, &emb);
                LmStudioRerankResult { index: idx, score }
            })
            .collect();

        results.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        Ok(results)
    }
}

fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let mut dot = 0.0;
    let mut norm_a = 0.0;
    let mut norm_b = 0.0;
    for (x, y) in a.iter().zip(b.iter()) {
        dot += x * y;
        norm_a += x * x;
        norm_b += y * y;
    }
    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }
    dot / (norm_a.sqrt() * norm_b.sqrt())
}
