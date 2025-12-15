use crate::rag::RetrievalResult;
use codegraph_core::{CodeGraphError, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{debug, instrument};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RankingConfig {
    pub semantic_weight: f32,
    pub keyword_weight: f32,
    pub recency_weight: f32,
    pub popularity_weight: f32,
    pub type_boost_factors: HashMap<String, f32>,
    pub enable_diversity_scoring: bool,
    pub max_similar_results: usize,
}

impl Default for RankingConfig {
    fn default() -> Self {
        let mut type_boost_factors = HashMap::new();
        type_boost_factors.insert("function".to_string(), 1.2);
        type_boost_factors.insert("struct".to_string(), 1.1);
        type_boost_factors.insert("trait".to_string(), 1.1);
        type_boost_factors.insert("enum".to_string(), 1.0);
        type_boost_factors.insert("module".to_string(), 0.9);
        type_boost_factors.insert("variable".to_string(), 0.8);

        Self {
            semantic_weight: 0.6,
            keyword_weight: 0.3,
            recency_weight: 0.05,
            popularity_weight: 0.05,
            type_boost_factors,
            enable_diversity_scoring: true,
            max_similar_results: 3,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RankedResult {
    pub retrieval_result: RetrievalResult,
    pub final_score: f32,
    pub score_breakdown: ScoreBreakdown,
    pub rank: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoreBreakdown {
    pub semantic_score: f32,
    pub keyword_score: f32,
    pub recency_score: f32,
    pub popularity_score: f32,
    pub type_boost: f32,
    pub diversity_penalty: f32,
}

pub struct ResultRanker {
    config: RankingConfig,
    node_popularity: HashMap<String, f32>,
    query_cache: HashMap<String, Vec<f32>>,
}

impl ResultRanker {
    pub fn new() -> Self {
        Self {
            config: RankingConfig::default(),
            node_popularity: HashMap::new(),
            query_cache: HashMap::new(),
        }
    }

    pub fn with_config(config: RankingConfig) -> Self {
        Self {
            config,
            node_popularity: HashMap::new(),
            query_cache: HashMap::new(),
        }
    }

    #[instrument(skip(self, results))]
    pub async fn rank_results(
        &mut self,
        mut results: Vec<RetrievalResult>,
        query: &str,
        query_embedding: &[f32],
    ) -> Result<Vec<RankedResult>> {
        debug!("Ranking {} results for query: {}", results.len(), query);

        if results.is_empty() {
            return Ok(Vec::new());
        }

        // Calculate scores for each result
        let mut ranked_results = Vec::new();
        for result in results.drain(..) {
            let score_breakdown = self
                .calculate_score_breakdown(&result, query, query_embedding)
                .await?;
            let final_score = self.calculate_final_score(&score_breakdown);

            ranked_results.push(RankedResult {
                retrieval_result: result,
                final_score,
                score_breakdown,
                rank: 0, // Will be set after sorting
            });
        }

        // Apply diversity scoring if enabled
        if self.config.enable_diversity_scoring {
            self.apply_diversity_scoring(&mut ranked_results).await?;
        }

        // Sort by final score
        ranked_results.sort_by(|a, b| {
            b.final_score
                .partial_cmp(&a.final_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // Assign ranks
        for (index, result) in ranked_results.iter_mut().enumerate() {
            result.rank = index + 1;
        }

        Ok(ranked_results)
    }

    pub async fn rank_by_semantic_similarity(
        &mut self,
        results: &mut Vec<(String, f32)>,
        query: &str,
    ) -> Result<()> {
        if results.is_empty() {
            return Ok(());
        }

        // Generate query embedding if not cached
        let query_embedding = if let Some(cached) = self.query_cache.get(query) {
            cached.clone()
        } else {
            let embedding = self.generate_query_embedding(query).await?;
            self.query_cache
                .insert(query.to_string(), embedding.clone());
            embedding
        };

        // Calculate semantic similarity for each result
        for (content, score) in results.iter_mut() {
            let content_embedding = self.generate_text_embedding(content).await?;
            let semantic_similarity =
                cosine_similarity(&query_embedding, &content_embedding).max(0.0);

            // Combine original score with semantic similarity
            let query_words: Vec<String> = query
                .to_lowercase()
                .split_whitespace()
                .map(|w| w.trim_matches(|c: char| !c.is_alphanumeric()).to_string())
                .filter(|w| w.len() > 2)
                .collect();
            let content_words: Vec<String> = content
                .to_lowercase()
                .split_whitespace()
                .map(|w| w.trim_matches(|c: char| !c.is_alphanumeric()).to_string())
                .filter(|w| w.len() > 2)
                .collect();

            let overlap = query_words
                .iter()
                .filter(|k| content_words.iter().any(|c| c == *k))
                .count();
            let keyword_score = if query_words.is_empty() {
                0.0
            } else {
                overlap as f32 / query_words.len() as f32
            };

            *score = (*score * 0.3) + (semantic_similarity * 0.1) + (keyword_score * 0.6);
        }

        // Sort by combined score
        results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        Ok(())
    }

    async fn calculate_score_breakdown(
        &self,
        result: &RetrievalResult,
        query: &str,
        query_embedding: &[f32],
    ) -> Result<ScoreBreakdown> {
        let semantic_score = self
            .calculate_semantic_score(result, query_embedding)
            .await?;
        let keyword_score = self.calculate_keyword_score(result, query);
        let recency_score = self.calculate_recency_score(result);
        let popularity_score = self.calculate_popularity_score(result);
        let type_boost = self.calculate_type_boost(result);
        let diversity_penalty = 0.0; // Will be calculated later

        Ok(ScoreBreakdown {
            semantic_score,
            keyword_score,
            recency_score,
            popularity_score,
            type_boost,
            diversity_penalty,
        })
    }

    fn calculate_final_score(&self, breakdown: &ScoreBreakdown) -> f32 {
        let base_score = (breakdown.semantic_score * self.config.semantic_weight)
            + (breakdown.keyword_score * self.config.keyword_weight)
            + (breakdown.recency_score * self.config.recency_weight)
            + (breakdown.popularity_score * self.config.popularity_weight);

        // Apply type boost
        let boosted_score = base_score * breakdown.type_boost;

        // Apply diversity penalty
        boosted_score * (1.0 - breakdown.diversity_penalty)
    }

    async fn calculate_semantic_score(
        &self,
        result: &RetrievalResult,
        query_embedding: &[f32],
    ) -> Result<f32> {
        if let Some(ref node) = result.node {
            if let Some(ref embedding) = node.embedding {
                Ok(cosine_similarity(query_embedding, embedding))
            } else {
                // Generate embedding for the node content
                let content = format!(
                    "{} {}",
                    node.name.as_str(),
                    node.content.as_deref().unwrap_or("")
                );
                let node_embedding = self.generate_text_embedding(&content).await?;
                Ok(cosine_similarity(query_embedding, &node_embedding))
            }
        } else {
            Ok(result.relevance_score) // Fallback to original relevance score
        }
    }

    fn calculate_keyword_score(&self, result: &RetrievalResult, query: &str) -> f32 {
        if let Some(ref node) = result.node {
            let query_lower = query.to_lowercase();
            let query_keywords: Vec<&str> = query_lower
                .split_whitespace()
                .filter(|w| w.len() > 2)
                .collect();

            if query_keywords.is_empty() {
                return 0.0;
            }

            let node_text = format!(
                "{} {}",
                node.name.as_str().to_lowercase(),
                node.content.as_deref().unwrap_or("").to_lowercase()
            );

            let mut matches = 0;
            for keyword in &query_keywords {
                if node_text.contains(keyword) {
                    matches += 1;
                }
            }

            matches as f32 / query_keywords.len() as f32
        } else {
            0.0
        }
    }

    fn calculate_recency_score(&self, result: &RetrievalResult) -> f32 {
        if let Some(ref node) = result.node {
            let now = chrono::Utc::now();
            let age_days = now
                .signed_duration_since(node.metadata.updated_at)
                .num_days();

            // Score decreases with age, but levels off after 30 days
            let max_age = 30.0;
            let normalized_age = (age_days as f32).min(max_age) / max_age;
            1.0 - normalized_age
        } else {
            0.0
        }
    }

    fn calculate_popularity_score(&self, result: &RetrievalResult) -> f32 {
        if let Some(ref node) = result.node {
            self.node_popularity
                .get(node.name.as_str())
                .cloned()
                .unwrap_or(0.0)
        } else {
            0.0
        }
    }

    fn calculate_type_boost(&self, result: &RetrievalResult) -> f32 {
        if let Some(ref node) = result.node {
            if let Some(ref node_type) = node.node_type {
                let type_str = format!("{:?}", node_type).to_lowercase();
                self.config
                    .type_boost_factors
                    .get(&type_str)
                    .cloned()
                    .unwrap_or(1.0)
            } else {
                1.0
            }
        } else {
            1.0
        }
    }

    async fn apply_diversity_scoring(&mut self, results: &mut [RankedResult]) -> Result<()> {
        // Group results by similarity to avoid redundant results
        let mut groups: Vec<Vec<usize>> = Vec::new();

        for (i, result) in results.iter().enumerate() {
            let mut assigned = false;

            for group in &mut groups {
                let group_representative = &results[group[0]];
                if self
                    .are_similar_results(result, group_representative)
                    .await?
                {
                    group.push(i);
                    assigned = true;
                    break;
                }
            }

            if !assigned {
                groups.push(vec![i]);
            }
        }

        // Apply diversity penalty to results beyond the first in each group
        for group in groups {
            if group.len() > self.config.max_similar_results {
                for &result_idx in group.iter().skip(self.config.max_similar_results) {
                    if let Some(result) = results.get_mut(result_idx) {
                        result.score_breakdown.diversity_penalty = 0.3; // 30% penalty
                        result.final_score = self.calculate_final_score(&result.score_breakdown);
                    }
                }
            }
        }

        Ok(())
    }

    async fn are_similar_results(
        &self,
        result1: &RankedResult,
        result2: &RankedResult,
    ) -> Result<bool> {
        if let (Some(ref node1), Some(ref node2)) = (
            &result1.retrieval_result.node,
            &result2.retrieval_result.node,
        ) {
            // Check if they have the same name or very similar content
            if node1.name == node2.name {
                return Ok(true);
            }

            // Check content similarity
            if let (Some(ref content1), Some(ref content2)) = (&node1.content, &node2.content) {
                let similarity = self
                    .calculate_content_similarity(content1, content2)
                    .await?;
                Ok(similarity > 0.8) // 80% similarity threshold
            } else {
                Ok(false)
            }
        } else {
            Ok(false)
        }
    }

    async fn calculate_content_similarity(&self, content1: &str, content2: &str) -> Result<f32> {
        let embedding1 = self.generate_text_embedding(content1).await?;
        let embedding2 = self.generate_text_embedding(content2).await?;
        Ok(cosine_similarity(&embedding1, &embedding2))
    }

    async fn generate_query_embedding(&self, query: &str) -> Result<Vec<f32>> {
        tokio::task::spawn_blocking({
            let query = query.to_string();
            move || {
                let dimension = 384;
                let mut embedding = vec![0.0f32; dimension];

                let hash = simple_hash(&query);
                let mut rng_state = hash;

                for i in 0..dimension {
                    rng_state = rng_state.wrapping_mul(1103515245).wrapping_add(12345);
                    embedding[i] = ((rng_state as f32 / u32::MAX as f32) - 0.5) * 2.0;
                }

                // Normalize embedding
                let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
                if norm > 0.0 {
                    for x in &mut embedding {
                        *x /= norm;
                    }
                }

                embedding
            }
        })
        .await
        .map_err(|e| CodeGraphError::Vector(e.to_string()))
    }

    async fn generate_text_embedding(&self, text: &str) -> Result<Vec<f32>> {
        // Similar to generate_query_embedding but could have different preprocessing
        self.generate_query_embedding(text).await
    }

    pub fn update_popularity_scores(&mut self, node_access_counts: &HashMap<String, u32>) {
        // Convert access counts to normalized popularity scores
        let max_count = node_access_counts.values().copied().max().unwrap_or(1);

        for (node_name, count) in node_access_counts.iter() {
            let popularity = *count as f32 / max_count as f32;
            self.node_popularity.insert(node_name.clone(), popularity);
        }
    }
}

impl Default for ResultRanker {
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
    use crate::rag::RetrievalMethod;
    use codegraph_core::{CodeNode, Language, Location, Metadata, NodeType};
    use uuid::Uuid;

    fn create_test_retrieval_result(name: &str, content: &str, score: f32) -> RetrievalResult {
        let now = chrono::Utc::now();
        RetrievalResult {
            node_id: Uuid::new_v4(),
            node: Some(CodeNode {
                id: Uuid::new_v4(),
                name: name.into(),
                node_type: Some(NodeType::Function),
                language: Some(Language::Rust),
                span: None,
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
            }),
            relevance_score: score,
            retrieval_method: RetrievalMethod::SemanticSimilarity,
            context_snippet: content.to_string(),
        }
    }

    #[tokio::test]
    async fn test_ranking_by_relevance_score() {
        let mut ranker = ResultRanker::new();

        let results = vec![
            create_test_retrieval_result("low_score", "some content", 0.3),
            create_test_retrieval_result("high_score", "important content", 0.9),
            create_test_retrieval_result("medium_score", "regular content", 0.6),
        ];

        let query = "test query";
        let query_embedding = vec![0.5; 384]; // Mock embedding

        let ranked = ranker
            .rank_results(results, query, &query_embedding)
            .await
            .unwrap();

        assert_eq!(ranked.len(), 3);
        assert_eq!(ranked[0].rank, 1);
        assert_eq!(ranked[1].rank, 2);
        assert_eq!(ranked[2].rank, 3);

        // Check that results are sorted by final score
        assert!(ranked[0].final_score >= ranked[1].final_score);
        assert!(ranked[1].final_score >= ranked[2].final_score);
    }

    #[tokio::test]
    async fn test_semantic_similarity_ranking() {
        let mut ranker = ResultRanker::new();

        let mut results = vec![
            ("function for data processing".to_string(), 0.5),
            ("async file operations".to_string(), 0.5),
            ("network connection handler".to_string(), 0.5),
        ];

        let query = "async file handling";
        ranker
            .rank_by_semantic_similarity(&mut results, query)
            .await
            .unwrap();

        // Results should be re-ranked based on semantic similarity
        // The "async file operations" should rank higher due to keyword overlap
        assert!(results[0].1 > 0.5); // Score should be boosted
    }

    #[test]
    fn test_keyword_score_calculation() {
        let ranker = ResultRanker::new();
        let result = create_test_retrieval_result("read_file", "async function to read files", 0.8);

        let score = ranker.calculate_keyword_score(&result, "async file reading");
        assert!(score > 0.0);
        assert!(score <= 1.0);
    }

    #[test]
    fn test_type_boost_calculation() {
        let ranker = ResultRanker::new();
        let result = create_test_retrieval_result("test_func", "test content", 0.5);

        let boost = ranker.calculate_type_boost(&result);
        assert_eq!(boost, 1.2); // Function should get 1.2x boost
    }

    #[test]
    fn test_final_score_calculation() {
        let ranker = ResultRanker::new();
        let breakdown = ScoreBreakdown {
            semantic_score: 0.8,
            keyword_score: 0.6,
            recency_score: 0.5,
            popularity_score: 0.7,
            type_boost: 1.2,
            diversity_penalty: 0.1,
        };

        let final_score = ranker.calculate_final_score(&breakdown);
        assert!(final_score > 0.0);
        assert!(final_score < 2.0); // Should be reasonable range
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
