use crate::EmbeddingGenerator;
#[cfg(feature = "faiss")]
use crate::SemanticSearch;
#[cfg(feature = "cache")]
use codegraph_cache::{CacheConfig, EmbeddingCache};
use codegraph_core::{CodeGraphError, CodeNode, NodeId, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, instrument};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetrievalResult {
    pub node_id: NodeId,
    pub node: Option<CodeNode>,
    pub relevance_score: f32,
    pub retrieval_method: RetrievalMethod,
    pub context_snippet: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum RetrievalMethod {
    SemanticSimilarity,
    KeywordMatching,
    HybridApproach,
    GraphTraversal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetrievalConfig {
    pub max_results: usize,
    pub relevance_threshold: f32,
    pub use_semantic_search: bool,
    pub use_keyword_matching: bool,
    pub boost_recent_nodes: bool,
    pub context_window_size: usize,
}

impl Default for RetrievalConfig {
    fn default() -> Self {
        Self {
            max_results: 10,
            relevance_threshold: 0.1,
            use_semantic_search: true,
            use_keyword_matching: true,
            boost_recent_nodes: false,
            context_window_size: 500,
        }
    }
}

pub struct ContextRetriever {
    #[cfg(feature = "faiss")]
    semantic_search: Option<Arc<SemanticSearch>>,
    embedding_generator: Arc<EmbeddingGenerator>,
    config: RetrievalConfig,
    node_cache: HashMap<NodeId, CodeNode>,
    #[cfg(feature = "cache")]
    embedding_cache: Arc<RwLock<EmbeddingCache>>,
}

impl ContextRetriever {
    pub fn new() -> Self {
        #[cfg(feature = "cache")]
        let embedding_cache = {
            let cache_config = CacheConfig {
                max_entries: 10_000,
                max_memory_bytes: 100 * 1024 * 1024, // 100MB
                default_ttl: std::time::Duration::from_secs(3600),
                enable_compression: true,
            };
            Arc::new(RwLock::new(EmbeddingCache::new(cache_config)))
        };

        Self {
            #[cfg(feature = "faiss")]
            semantic_search: None,
            embedding_generator: Arc::new(EmbeddingGenerator::default()),
            config: RetrievalConfig::default(),
            node_cache: HashMap::new(),
            #[cfg(feature = "cache")]
            embedding_cache,
        }
    }

    pub fn with_config(config: RetrievalConfig) -> Self {
        #[cfg(feature = "cache")]
        let embedding_cache = {
            let cache_config = CacheConfig {
                max_entries: 10_000,
                max_memory_bytes: 100 * 1024 * 1024, // 100MB
                default_ttl: std::time::Duration::from_secs(3600),
                enable_compression: true,
            };
            Arc::new(RwLock::new(EmbeddingCache::new(cache_config)))
        };

        Self {
            #[cfg(feature = "faiss")]
            semantic_search: None,
            embedding_generator: Arc::new(EmbeddingGenerator::default()),
            config,
            node_cache: HashMap::new(),
            #[cfg(feature = "cache")]
            embedding_cache,
        }
    }

    #[cfg(feature = "faiss")]
    pub fn set_semantic_search(&mut self, semantic_search: Arc<SemanticSearch>) {
        self.semantic_search = Some(semantic_search);
    }

    pub fn add_node_to_cache(&mut self, node: CodeNode) {
        self.node_cache.insert(node.id, node);
    }

    #[instrument(skip(self, query_embedding))]
    pub async fn retrieve_context(
        &self,
        query: &str,
        query_embedding: &[f32],
        keywords: &[String],
    ) -> Result<Vec<RetrievalResult>> {
        debug!("Retrieving context for query: {}", query);

        let mut all_results = Vec::new();

        // Semantic similarity search (requires `faiss` feature)
        #[cfg(feature = "faiss")]
        if self.config.use_semantic_search && self.semantic_search.is_some() {
            let semantic_results = self.semantic_similarity_search(query_embedding).await?;
            all_results.extend(semantic_results);
        }

        // Keyword-based search
        if self.config.use_keyword_matching {
            let keyword_results = self.keyword_matching_search(keywords).await?;
            all_results.extend(keyword_results);
        }

        // Hybrid approach combining multiple methods
        let hybrid_results = self.hybrid_search(query, query_embedding, keywords).await?;
        all_results.extend(hybrid_results);

        // Remove duplicates and sort by relevance
        let mut unique_results = self.deduplicate_results(all_results);
        self.rank_results(&mut unique_results, query).await?;

        // Apply relevance threshold and limit
        unique_results.retain(|r| r.relevance_score >= self.config.relevance_threshold);
        unique_results.truncate(self.config.max_results);

        Ok(unique_results)
    }

    #[cfg(feature = "faiss")]
    async fn semantic_similarity_search(
        &self,
        query_embedding: &[f32],
    ) -> Result<Vec<RetrievalResult>> {
        let semantic_search = self.semantic_search.as_ref().ok_or_else(|| {
            CodeGraphError::InvalidOperation("SemanticSearch not configured".to_string())
        })?;

        let search_results = semantic_search
            .search_by_embedding(query_embedding, self.config.max_results * 2)
            .await?;

        let mut results = Vec::new();
        for search_result in search_results {
            if let Some(node) = self.get_node(search_result.node_id).await? {
                let context_snippet = self.extract_context_snippet(&node);
                results.push(RetrievalResult {
                    node_id: search_result.node_id,
                    node: Some(node),
                    relevance_score: search_result.score,
                    retrieval_method: RetrievalMethod::SemanticSimilarity,
                    context_snippet,
                });
            }
        }

        Ok(results)
    }

    #[cfg(not(feature = "faiss"))]
    async fn semantic_similarity_search(
        &self,
        _query_embedding: &[f32],
    ) -> Result<Vec<RetrievalResult>> {
        Ok(Vec::new())
    }

    async fn keyword_matching_search(&self, keywords: &[String]) -> Result<Vec<RetrievalResult>> {
        let mut results = Vec::new();

        for (node_id, node) in &self.node_cache {
            let score = self.calculate_keyword_relevance(node, keywords);
            if score > 0.0 {
                let context_snippet = self.extract_context_snippet(node);
                results.push(RetrievalResult {
                    node_id: *node_id,
                    node: Some(node.clone()),
                    relevance_score: score,
                    retrieval_method: RetrievalMethod::KeywordMatching,
                    context_snippet,
                });
            }
        }

        // Sort by relevance score
        results.sort_by(|a, b| {
            b.relevance_score
                .partial_cmp(&a.relevance_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        Ok(results)
    }

    async fn hybrid_search(
        &self,
        _query: &str,
        _query_embedding: &[f32],
        keywords: &[String],
    ) -> Result<Vec<RetrievalResult>> {
        let mut results = Vec::new();

        // Get semantic similarity scores
        #[cfg(feature = "faiss")]
        let semantic_scores = if let Some(semantic_search) = &self.semantic_search {
            let search_results = semantic_search
                .search_by_embedding(_query_embedding, self.config.max_results * 3)
                .await?;
            search_results
                .into_iter()
                .map(|r| (r.node_id, r.score))
                .collect::<HashMap<_, _>>()
        } else {
            HashMap::new()
        };
        #[cfg(not(feature = "faiss"))]
        let semantic_scores: HashMap<NodeId, f32> = HashMap::new();

        // Combine semantic and keyword scores
        for (node_id, node) in &self.node_cache {
            let semantic_score = semantic_scores.get(node_id).cloned().unwrap_or(0.0);
            let keyword_score = self.calculate_keyword_relevance(node, keywords);

            // Weighted combination of scores
            let combined_score = (semantic_score * 0.7) + (keyword_score * 0.3);

            if combined_score > self.config.relevance_threshold {
                let context_snippet = self.extract_context_snippet(node);
                results.push(RetrievalResult {
                    node_id: *node_id,
                    node: Some(node.clone()),
                    relevance_score: combined_score,
                    retrieval_method: RetrievalMethod::HybridApproach,
                    context_snippet,
                });
            }
        }

        Ok(results)
    }

    pub async fn calculate_relevance_scores(
        &self,
        query: &str,
        contexts: &[String],
    ) -> Result<Vec<f32>> {
        let mut scores = Vec::new();

        // Generate query embedding for comparison
        let query_embedding = self.generate_query_embedding(query).await?;

        for context in contexts {
            // Generate embedding for each context
            let context_embedding = self.generate_text_embedding(context).await?;

            // Calculate cosine similarity
            let similarity = cosine_similarity(&query_embedding, &context_embedding);
            scores.push(similarity);
        }

        Ok(scores)
    }

    async fn generate_query_embedding(&self, query: &str) -> Result<Vec<f32>> {
        // Try cache first if available
        #[cfg(feature = "cache")]
        {
            use sha2::{Digest, Sha256};
            let mut hasher = Sha256::new();
            hasher.update(query.as_bytes());
            let key = format!("query_{:x}", hasher.finalize());

            // Check cache
            if let Ok(Some(cached_embedding)) = self.embedding_cache.write().await.get(&key).await {
                info!("🎯 Cache hit for query embedding");
                return Ok(cached_embedding);
            }

            // Generate using embedding generator
            let embedding = self.embedding_generator.generate_text_embedding(query).await?;

            // Cache the result
            let _ = self.embedding_cache.write().await.insert(key, embedding.clone(), std::time::Duration::from_secs(3600)).await;
            info!("💾 Cached query embedding");
            return Ok(embedding);
        }

        #[cfg(not(feature = "cache"))]
        {
            // Fallback: Use embedding generator or deterministic fallback
            self.embedding_generator.generate_text_embedding(query).await
        }
    }

    async fn generate_text_embedding(&self, text: &str) -> Result<Vec<f32>> {
        // Similar to generate_query_embedding but could have different preprocessing
        self.generate_query_embedding(text).await
    }

    fn calculate_keyword_relevance(&self, node: &CodeNode, keywords: &[String]) -> f32 {
        if keywords.is_empty() {
            return 0.0;
        }

        let node_text = format!(
            "{} {} {}",
            node.name.as_str(),
            node.content.as_deref().unwrap_or(""),
            node.node_type
                .as_ref()
                .map(|t| format!("{:?}", t))
                .unwrap_or_default()
        )
        .to_lowercase();

        let mut matches = 0;
        let mut total_weight = 0.0;

        for keyword in keywords {
            let keyword_lower = keyword.to_lowercase();

            // Exact match in name gets highest weight
            if node.name.as_str().to_lowercase().contains(&keyword_lower) {
                matches += 1;
                total_weight += 3.0;
            }
            // Match in content gets medium weight
            else if node
                .content
                .as_ref()
                .map_or(false, |c| c.to_lowercase().contains(&keyword_lower))
            {
                matches += 1;
                total_weight += 2.0;
            }
            // Match in node type gets low weight
            else if node_text.contains(&keyword_lower) {
                matches += 1;
                total_weight += 1.0;
            }
        }

        if matches == 0 {
            0.0
        } else {
            total_weight / keywords.len() as f32
        }
    }

    fn extract_context_snippet(&self, node: &CodeNode) -> String {
        let content = node.content.as_deref().unwrap_or("");
        if content.len() <= self.config.context_window_size {
            content.to_string()
        } else {
            format!("{}...", &content[..self.config.context_window_size])
        }
    }

    async fn get_node(&self, node_id: NodeId) -> Result<Option<CodeNode>> {
        Ok(self.node_cache.get(&node_id).cloned())
    }

    fn deduplicate_results(&self, mut results: Vec<RetrievalResult>) -> Vec<RetrievalResult> {
        results.sort_by(|a, b| a.node_id.cmp(&b.node_id));
        results.dedup_by(|a, b| a.node_id == b.node_id);
        results
    }

    async fn rank_results(&self, results: &mut [RetrievalResult], _query: &str) -> Result<()> {
        // Sort by relevance score (descending)
        results.sort_by(|a, b| {
            b.relevance_score
                .partial_cmp(&a.relevance_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // Apply additional ranking factors
        for result in results.iter_mut() {
            // Boost certain node types
            if let Some(ref node) = result.node {
                if let Some(node_type) = &node.node_type {
                    let type_boost = match node_type {
                        codegraph_core::NodeType::Function => 1.2,
                        codegraph_core::NodeType::Struct => 1.1,
                        codegraph_core::NodeType::Trait => 1.1,
                        _ => 1.0,
                    };
                    result.relevance_score *= type_boost;
                }
            }
        }

        // Re-sort after applying boosts
        results.sort_by(|a, b| {
            b.relevance_score
                .partial_cmp(&a.relevance_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        Ok(())
    }
}

impl Default for ContextRetriever {
    fn default() -> Self {
        Self::new()
    }
}

fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() {
        return 0.0;
    }

    let dot_product: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

    if norm_a == 0.0 || norm_b == 0.0 {
        0.0
    } else {
        dot_product / (norm_a * norm_b)
    }
}

fn simple_hash(text: &str) -> u32 {
    let mut hash = 5381u32;
    for byte in text.bytes() {
        hash = hash.wrapping_mul(33).wrapping_add(byte as u32);
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::*;
    use codegraph_core::{Language, NodeType};
    use uuid::Uuid;

    fn create_test_node(name: &str, content: &str, node_type: NodeType) -> CodeNode {
        let now = chrono::Utc::now();
        CodeNode {
            id: Uuid::new_v4(),
            name: name.into(),
            node_type: Some(node_type),
            language: Some(Language::Rust),
            content: Some(content.into()),
            embedding: None,
            location: Location {
                file_path: "test.rs".to_string(),
                line: 1,
                column: 1,
                end_line: None,
                end_column: None,
            },
            metadata: Metadata {
                attributes: std::collections::HashMap::new(),
                created_at: now,
                updated_at: now,
            },
            complexity: None,
        }
    }

    #[tokio::test]
    async fn test_keyword_relevance_calculation() {
        let retriever = ContextRetriever::new();
        let node = create_test_node(
            "read_file",
            "fn read_file(path: &str) -> Result<String>",
            NodeType::Function,
        );

        let keywords = vec!["read".to_string(), "file".to_string()];
        let score = retriever.calculate_keyword_relevance(&node, &keywords);

        assert!(score > 0.0);
        assert!(score <= 3.0); // Maximum possible score
    }

    #[tokio::test]
    async fn test_context_snippet_extraction() {
        let config = RetrievalConfig {
            context_window_size: 50,
            ..Default::default()
        };
        let retriever = ContextRetriever::with_config(config);

        let long_content = "This is a very long content that exceeds the context window size and should be truncated properly to fit within the specified limits for the context snippet.";
        let node = create_test_node("test_function", long_content, NodeType::Function);

        let snippet = retriever.extract_context_snippet(&node);

        assert!(snippet.len() <= 53); // 50 + "..." = 53
        assert!(snippet.ends_with("..."));
    }

    #[tokio::test]
    async fn test_relevance_score_calculation() {
        let retriever = ContextRetriever::new();

        let contexts = vec![
            "function for reading files".to_string(),
            "async network operation".to_string(),
            "database connection handler".to_string(),
        ];

        let query = "file reading function";
        let scores = retriever
            .calculate_relevance_scores(query, &contexts)
            .await
            .unwrap();

        assert_eq!(scores.len(), 3);
        // First context should have highest relevance due to keyword overlap
        assert!(scores[0] >= scores[1]);
        assert!(scores[0] >= scores[2]);
    }

    #[test]
    fn test_cosine_similarity() {
        let vec1 = vec![1.0, 0.0, 0.0];
        let vec2 = vec![1.0, 0.0, 0.0];
        let vec3 = vec![0.0, 1.0, 0.0];

        assert!((cosine_similarity(&vec1, &vec2) - 1.0).abs() < 1e-6);
        assert!((cosine_similarity(&vec1, &vec3) - 0.0).abs() < 1e-6);
    }
}
