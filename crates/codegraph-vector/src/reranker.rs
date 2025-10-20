use async_trait::async_trait;
use codegraph_core::{CodeNode, NodeId, Result};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{debug, info};

/// Result from a reranking operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RerankedResult {
    pub node_id: NodeId,
    pub node: Option<CodeNode>,
    pub relevance_score: f32,
    pub original_rank: usize,
    pub reranked_position: usize,
    pub context_snippet: String,
}

/// Configuration for the reranking pipeline
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RerankerConfig {
    /// Stage 1: Fast embedding-based filtering
    pub embedding_top_k: usize,
    pub embedding_threshold: f32,

    /// Stage 2: Cross-encoder reranking
    pub enable_cross_encoder: bool,
    pub cross_encoder_top_k: usize,
    pub cross_encoder_threshold: f32,

    /// Stage 3: LLM-based insights (optional)
    pub enable_llm_insights: bool,
    pub llm_top_k: usize,

    /// Performance optimization
    pub enable_batch_processing: bool,
    pub batch_size: usize,
    pub max_concurrent_requests: usize,
}

impl Default for RerankerConfig {
    fn default() -> Self {
        Self {
            // Stage 1: Fast filter - get top 100 from embeddings
            embedding_top_k: 100,
            embedding_threshold: 0.3,

            // Stage 2: Reranking - narrow to top 20
            enable_cross_encoder: true,
            cross_encoder_top_k: 20,
            cross_encoder_threshold: 0.5,

            // Stage 3: LLM - only process top 10 (optional)
            enable_llm_insights: false, // Disabled by default for speed
            llm_top_k: 10,

            // Performance
            enable_batch_processing: true,
            batch_size: 32,
            max_concurrent_requests: 4,
        }
    }
}

/// Trait for reranking models
#[async_trait]
pub trait ReRanker: Send + Sync {
    /// Rerank a list of candidates based on query relevance
    async fn rerank(
        &self,
        query: &str,
        candidates: Vec<(NodeId, String)>,
    ) -> Result<Vec<(NodeId, f32)>>;

    /// Get the model name
    fn model_name(&self) -> &str;

    /// Check if the reranker supports batching
    fn supports_batching(&self) -> bool {
        false
    }
}

/// Fast embedding-based reranker (Stage 1)
pub struct EmbeddingReRanker {
    embedding_generator: Arc<crate::EmbeddingGenerator>,
}

impl EmbeddingReRanker {
    pub fn new(embedding_generator: Arc<crate::EmbeddingGenerator>) -> Self {
        Self { embedding_generator }
    }

    /// Fast cosine similarity computation
    fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
        if a.len() != b.len() {
            return 0.0;
        }

        let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
        let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
        let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

        if norm_a == 0.0 || norm_b == 0.0 {
            return 0.0;
        }

        dot / (norm_a * norm_b)
    }
}

#[async_trait]
impl ReRanker for EmbeddingReRanker {
    async fn rerank(
        &self,
        query: &str,
        candidates: Vec<(NodeId, String)>,
    ) -> Result<Vec<(NodeId, f32)>> {
        debug!("🔍 Fast embedding-based reranking for {} candidates", candidates.len());

        // Generate query embedding
        let query_embedding = self.embedding_generator.generate_text_embedding(query).await?;

        // Batch generate candidate embeddings for GPU efficiency
        let candidate_texts: Vec<String> = candidates.iter().map(|(_, text)| text.clone()).collect();
        let candidate_embeddings = self.embedding_generator.embed_texts_batched(&candidate_texts).await?;

        // Compute similarities
        let mut scores: Vec<(NodeId, f32)> = candidates
            .iter()
            .zip(candidate_embeddings.iter())
            .map(|((node_id, _), embedding)| {
                let similarity = Self::cosine_similarity(&query_embedding, embedding);
                (*node_id, similarity)
            })
            .collect();

        // Sort by score descending
        scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        info!("✅ Embedding reranking complete: top score = {:.3}", scores.first().map(|s| s.1).unwrap_or(0.0));
        Ok(scores)
    }

    fn model_name(&self) -> &str {
        "embedding-based-reranker"
    }

    fn supports_batching(&self) -> bool {
        true
    }
}

/// Cross-encoder reranker for fine-grained scoring (Stage 2)
/// This would use models like bge-reranker-large, ms-marco-MiniLM, etc.
pub struct CrossEncoderReRanker {
    model_name: String,
    // In production, this would load an actual cross-encoder model
    // For now, we'll use a placeholder that simulates the behavior
}

impl CrossEncoderReRanker {
    pub fn new(model_name: String) -> Self {
        Self { model_name }
    }

    /// Simulate cross-encoder scoring
    /// In production, this would call the actual model
    async fn compute_cross_encoder_score(&self, query: &str, text: &str) -> f32 {
        // Placeholder: In real implementation, this would:
        // 1. Tokenize query + text pair
        // 2. Pass through cross-encoder model
        // 3. Get relevance score

        // For now, simulate with simple keyword matching + length penalty
        let query_lower = query.to_lowercase();
        let text_lower = text.to_lowercase();

        let mut score = 0.0f32;

        // Keyword matching
        for word in query_lower.split_whitespace() {
            if text_lower.contains(word) {
                score += 0.2;
            }
        }

        // Length penalty (prefer concise matches)
        let length_penalty = 1.0 / (1.0 + (text.len() as f32 / 1000.0));
        score *= length_penalty;

        // Normalize to 0-1
        score.min(1.0)
    }
}

#[async_trait]
impl ReRanker for CrossEncoderReRanker {
    async fn rerank(
        &self,
        query: &str,
        candidates: Vec<(NodeId, String)>,
    ) -> Result<Vec<(NodeId, f32)>> {
        debug!("🎯 Cross-encoder reranking for {} candidates", candidates.len());

        // Compute cross-encoder scores for all candidates
        let mut scores = Vec::new();
        for (node_id, text) in candidates {
            let score = self.compute_cross_encoder_score(query, &text).await;
            scores.push((node_id, score));
        }

        // Sort by score descending
        scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        info!("✅ Cross-encoder reranking complete: top score = {:.3}", scores.first().map(|s| s.1).unwrap_or(0.0));
        Ok(scores)
    }

    fn model_name(&self) -> &str {
        &self.model_name
    }

    fn supports_batching(&self) -> bool {
        true
    }
}

/// Multi-stage reranking pipeline
pub struct ReRankingPipeline {
    config: RerankerConfig,
    embedding_reranker: EmbeddingReRanker,
    cross_encoder_reranker: Option<CrossEncoderReRanker>,
}

impl ReRankingPipeline {
    pub fn new(
        config: RerankerConfig,
        embedding_generator: Arc<crate::EmbeddingGenerator>,
    ) -> Self {
        let embedding_reranker = EmbeddingReRanker::new(embedding_generator);

        let cross_encoder_reranker = if config.enable_cross_encoder {
            Some(CrossEncoderReRanker::new("bge-reranker-base".to_string()))
        } else {
            None
        };

        Self {
            config,
            embedding_reranker,
            cross_encoder_reranker,
        }
    }

    /// Run the full reranking pipeline
    pub async fn rerank_pipeline(
        &self,
        query: &str,
        initial_candidates: Vec<(NodeId, CodeNode)>,
    ) -> Result<Vec<RerankedResult>> {
        let total_start = std::time::Instant::now();

        info!("🚀 Starting reranking pipeline for {} candidates", initial_candidates.len());
        info!("   📊 Stage 1: Embedding-based filter (target: top {})", self.config.embedding_top_k);
        info!("   📊 Stage 2: Cross-encoder rerank (target: top {})", self.config.cross_encoder_top_k);
        info!("   📊 Stage 3: LLM insights (enabled: {}, target: top {})",
            self.config.enable_llm_insights, self.config.llm_top_k);

        // Stage 1: Fast embedding-based filtering
        let stage1_start = std::time::Instant::now();
        let candidates_with_text: Vec<(NodeId, String)> = initial_candidates
            .iter()
            .map(|(id, node)| {
                let text = format!(
                    "{} {} {}",
                    node.name,
                    node.content.as_deref().unwrap_or(""),
                    node.location.file_path
                );
                (*id, text)
            })
            .collect();

        let mut embedding_scores = self.embedding_reranker.rerank(query, candidates_with_text.clone()).await?;

        // Apply threshold and top-k
        embedding_scores.retain(|(_, score)| *score >= self.config.embedding_threshold);
        embedding_scores.truncate(self.config.embedding_top_k);

        let stage1_duration = stage1_start.elapsed();
        info!("✅ Stage 1 complete in {:.2}ms: {} candidates passed filter",
            stage1_duration.as_secs_f64() * 1000.0, embedding_scores.len());

        // Stage 2: Cross-encoder reranking (if enabled)
        let mut final_scores = embedding_scores.clone();

        if let Some(ref cross_encoder) = self.cross_encoder_reranker {
            let stage2_start = std::time::Instant::now();

            // Get candidates that passed stage 1
            let stage2_candidates: Vec<(NodeId, String)> = embedding_scores
                .iter()
                .filter_map(|(id, _)| {
                    candidates_with_text.iter()
                        .find(|(cid, _)| cid == id)
                        .map(|(id, text)| (*id, text.clone()))
                })
                .collect();

            let mut cross_encoder_scores = cross_encoder.rerank(query, stage2_candidates).await?;

            // Apply threshold and top-k
            cross_encoder_scores.retain(|(_, score)| *score >= self.config.cross_encoder_threshold);
            cross_encoder_scores.truncate(self.config.cross_encoder_top_k);

            let stage2_duration = stage2_start.elapsed();
            info!("✅ Stage 2 complete in {:.2}ms: {} candidates reranked",
                stage2_duration.as_secs_f64() * 1000.0, cross_encoder_scores.len());

            final_scores = cross_encoder_scores;
        }

        // Build final results
        let mut results: Vec<RerankedResult> = Vec::new();
        for (reranked_position, (node_id, score)) in final_scores.iter().enumerate() {
            if let Some((original_rank, (_, node))) = initial_candidates
                .iter()
                .enumerate()
                .find(|(_, (id, _))| id == node_id)
            {
                let context_snippet = node.content.as_deref()
                    .unwrap_or(&node.name)
                    .chars()
                    .take(200)
                    .collect::<String>();

                results.push(RerankedResult {
                    node_id: *node_id,
                    node: Some(node.clone()),
                    relevance_score: *score,
                    original_rank,
                    reranked_position,
                    context_snippet,
                });
            }
        }

        let total_duration = total_start.elapsed();
        info!("🎉 Reranking pipeline complete in {:.2}ms", total_duration.as_secs_f64() * 1000.0);
        info!("   📈 Reduction: {} -> {} candidates ({:.1}% of original)",
            initial_candidates.len(), results.len(),
            (results.len() as f64 / initial_candidates.len() as f64) * 100.0);

        Ok(results)
    }

    /// Get candidates ready for LLM processing (if enabled)
    pub fn get_llm_candidates(&self, reranked_results: &[RerankedResult]) -> Vec<RerankedResult> {
        if !self.config.enable_llm_insights {
            return Vec::new();
        }

        reranked_results
            .iter()
            .take(self.config.llm_top_k)
            .cloned()
            .collect()
    }
}

/// Metrics for reranking performance
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReRankingMetrics {
    pub total_candidates: usize,
    pub stage1_passed: usize,
    pub stage2_passed: usize,
    pub llm_processed: usize,
    pub stage1_duration_ms: f64,
    pub stage2_duration_ms: f64,
    pub total_duration_ms: f64,
    pub reduction_ratio: f64,
}
