// ABOUTME: Text-based reranking for RAG pipelines with Jina and Ollama support
// ABOUTME: Uses cross-encoder models to rerank retrieved documents based on query relevance
use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

pub mod factory;
#[cfg(feature = "jina")]
pub mod jina;
#[cfg(feature = "ollama")]
pub mod ollama;

/// A document to be reranked with its metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RerankDocument {
    /// Unique identifier for the document
    pub id: String,
    /// Text content to be scored for relevance
    pub text: String,
    /// Optional metadata (file path, line numbers, etc.)
    pub metadata: Option<serde_json::Value>,
}

/// Result from reranking a single document
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RerankResult {
    /// Document ID
    pub id: String,
    /// Relevance score (0.0 to 1.0)
    pub score: f32,
    /// Original position in the input list
    pub index: usize,
    /// Optional metadata passed through from input
    pub metadata: Option<serde_json::Value>,
}

/// Trait for text-based reranking models
///
/// Unlike embedding-based similarity, rerankers use cross-encoder models
/// that jointly encode the query and document to produce a relevance score.
/// This is more accurate but slower than embedding similarity.
#[async_trait]
pub trait Reranker: Send + Sync {
    /// Rerank a list of documents based on query relevance
    ///
    /// # Arguments
    /// * `query` - The search query text
    /// * `documents` - List of documents to rerank
    /// * `top_n` - Maximum number of results to return
    ///
    /// # Returns
    /// Ranked list of documents sorted by relevance score (highest first)
    async fn rerank(
        &self,
        query: &str,
        documents: Vec<RerankDocument>,
        top_n: usize,
    ) -> Result<Vec<RerankResult>>;

    /// Get the name of the reranking model
    fn model_name(&self) -> &str;

    /// Get the provider name (jina, ollama, etc.)
    fn provider_name(&self) -> &str;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rerank_document_creation() {
        let doc = RerankDocument {
            id: "test-1".to_string(),
            text: "This is a test document".to_string(),
            metadata: Some(serde_json::json!({"file": "test.rs"})),
        };

        assert_eq!(doc.id, "test-1");
        assert_eq!(doc.text, "This is a test document");
        assert!(doc.metadata.is_some());
    }

    #[test]
    fn test_rerank_result_creation() {
        let result = RerankResult {
            id: "test-1".to_string(),
            score: 0.95,
            index: 0,
            metadata: Some(serde_json::json!({"file": "test.rs"})),
        };

        assert_eq!(result.id, "test-1");
        assert!((result.score - 0.95).abs() < 0.001);
        assert_eq!(result.index, 0);
    }
}
