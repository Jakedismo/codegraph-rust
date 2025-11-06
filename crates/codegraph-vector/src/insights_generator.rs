use crate::reranker::{ReRankingPipeline, RerankedResult, RerankerConfig};
use crate::EmbeddingGenerator;
use codegraph_core::{CodeNode, NodeId, Result};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{info, warn};

/// Mode for insights generation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum InsightsMode {
    /// Fast mode: Return context only, no LLM processing
    /// Best for agent-based workflows (Claude, GPT-4, etc.)
    ContextOnly,

    /// Balanced mode: Use reranking + lightweight LLM
    /// Good for local processing with speed requirements
    Balanced,

    /// Deep mode: Use full LLM processing
    /// Best for comprehensive analysis, slower
    Deep,
}

impl Default for InsightsMode {
    fn default() -> Self {
        Self::ContextOnly // Default to fastest mode
    }
}

/// Configuration for insights generation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InsightsConfig {
    pub mode: InsightsMode,
    pub reranker_config: RerankerConfig,
    pub max_context_length: usize,
    pub include_metadata: bool,
}

impl Default for InsightsConfig {
    fn default() -> Self {
        Self {
            mode: InsightsMode::ContextOnly,
            reranker_config: RerankerConfig::default(),
            max_context_length: 8000, // Tokens
            include_metadata: true,
        }
    }
}

/// Result from insights generation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InsightsResult {
    pub query: String,
    pub mode: InsightsMode,
    pub reranked_files: Vec<RerankedResult>,
    pub context: String,
    pub llm_insights: Option<String>,
    pub metrics: InsightsMetrics,
}

/// Performance metrics for insights generation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InsightsMetrics {
    pub total_candidates: usize,
    pub files_analyzed: usize,
    pub reranking_duration_ms: f64,
    pub llm_duration_ms: f64,
    pub total_duration_ms: f64,
    pub speedup_ratio: f64, // vs processing all files
}

/// High-performance insights generator with reranking pipeline
pub struct InsightsGenerator {
    config: InsightsConfig,
    reranking_pipeline: ReRankingPipeline,
}

impl InsightsGenerator {
    pub fn new(config: InsightsConfig, embedding_generator: Arc<EmbeddingGenerator>) -> Self {
        let reranking_pipeline =
            ReRankingPipeline::new(config.reranker_config.clone(), embedding_generator);

        Self {
            config,
            reranking_pipeline,
        }
    }

    /// Generate insights with automatic mode selection
    pub async fn generate_insights(
        &self,
        query: &str,
        candidates: Vec<(NodeId, CodeNode)>,
    ) -> Result<InsightsResult> {
        let total_start = std::time::Instant::now();

        info!(
            "🚀 Generating insights in {:?} mode for {} candidates",
            self.config.mode,
            candidates.len()
        );

        // Stage 1 & 2: Reranking pipeline (always runs)
        let reranking_start = std::time::Instant::now();
        let reranked_results = self
            .reranking_pipeline
            .rerank_pipeline(query, candidates.clone())
            .await?;
        let reranking_duration = reranking_start.elapsed().as_secs_f64() * 1000.0;

        info!(
            "✅ Reranking complete: {} -> {} files ({:.1}% reduction)",
            candidates.len(),
            reranked_results.len(),
            (1.0 - reranked_results.len() as f64 / candidates.len() as f64) * 100.0
        );

        // Build context from reranked results
        let context = self.build_context(&reranked_results);

        // Stage 3: Optional LLM processing
        let (llm_insights, llm_duration) = match self.config.mode {
            InsightsMode::ContextOnly => {
                info!(
                    "📋 Context-only mode: Skipping LLM processing (returning context for agent)"
                );
                (None, 0.0)
            }
            InsightsMode::Balanced => {
                self.generate_llm_insights_lightweight(query, &reranked_results)
                    .await?
            }
            InsightsMode::Deep => {
                self.generate_llm_insights_deep(query, &reranked_results)
                    .await?
            }
        };

        let total_duration = total_start.elapsed().as_secs_f64() * 1000.0;

        // Calculate speedup vs processing all files
        let estimated_full_llm_time = candidates.len() as f64 * 500.0; // Estimate 500ms per file
        let speedup_ratio = estimated_full_llm_time / total_duration;

        let metrics = InsightsMetrics {
            total_candidates: candidates.len(),
            files_analyzed: reranked_results.len(),
            reranking_duration_ms: reranking_duration,
            llm_duration_ms: llm_duration,
            total_duration_ms: total_duration,
            speedup_ratio,
        };

        info!(
            "🎉 Insights generation complete in {:.2}ms ({:.1}x faster than processing all files)",
            total_duration, speedup_ratio
        );

        Ok(InsightsResult {
            query: query.to_string(),
            mode: self.config.mode,
            reranked_files: reranked_results,
            context,
            llm_insights,
            metrics,
        })
    }

    /// Build formatted context from reranked results
    fn build_context(&self, results: &[RerankedResult]) -> String {
        let mut context = String::new();

        context.push_str(&format!(
            "# Retrieved Context ({} files)\n\n",
            results.len()
        ));

        for (idx, result) in results.iter().enumerate() {
            if let Some(ref node) = result.node {
                context.push_str(&format!(
                    "## File {} (Score: {:.3})\n",
                    idx + 1,
                    result.relevance_score
                ));

                if self.config.include_metadata {
                    context.push_str(&format!("**Path**: {}\n", node.location.file_path));
                    context.push_str(&format!("**Name**: {}\n", node.name));
                    if let Some(ref lang) = node.language {
                        context.push_str(&format!("**Language**: {:?}\n", lang));
                    }
                    if let Some(ref node_type) = node.node_type {
                        context.push_str(&format!("**Type**: {:?}\n", node_type));
                    }
                    context.push_str("\n");
                }

                context.push_str("**Content**:\n```\n");
                if let Some(ref content) = node.content {
                    // Truncate to max context length
                    let truncated = if content.len() > self.config.max_context_length {
                        format!(
                            "{}... [truncated]",
                            &content[..self.config.max_context_length]
                        )
                    } else {
                        content.to_string()
                    };
                    context.push_str(&truncated);
                } else {
                    context.push_str(&node.name);
                }
                context.push_str("\n```\n\n");
            }
        }

        context
    }

    /// Generate lightweight LLM insights (balanced mode)
    async fn generate_llm_insights_lightweight(
        &self,
        query: &str,
        results: &[RerankedResult],
    ) -> Result<(Option<String>, f64)> {
        let start = std::time::Instant::now();

        // Get top K files for LLM processing
        let llm_candidates = self.reranking_pipeline.get_llm_candidates(results);

        if llm_candidates.is_empty() {
            warn!("No candidates for LLM processing in balanced mode");
            return Ok((None, 0.0));
        }

        info!(
            "🤖 Running lightweight LLM on {} files",
            llm_candidates.len()
        );

        // In production, this would call the local LLM (Qwen2.5-Coder)
        // For now, return a placeholder
        let insights = format!(
            "Lightweight analysis of {} files for query: '{}'\n\
            This would contain quick insights from Qwen2.5-Coder.",
            llm_candidates.len(),
            query
        );

        let duration = start.elapsed().as_secs_f64() * 1000.0;
        Ok((Some(insights), duration))
    }

    /// Generate deep LLM insights (deep mode)
    async fn generate_llm_insights_deep(
        &self,
        query: &str,
        results: &[RerankedResult],
    ) -> Result<(Option<String>, f64)> {
        let start = std::time::Instant::now();

        info!("🔬 Running deep LLM analysis on {} files", results.len());

        // In production, this would call the local LLM with more context
        let insights = format!(
            "Deep analysis of {} files for query: '{}'\n\
            This would contain comprehensive insights from Qwen2.5-Coder.",
            results.len(),
            query
        );

        let duration = start.elapsed().as_secs_f64() * 1000.0;
        Ok((Some(insights), duration))
    }

    /// Create a preset for fast agent-based workflows
    pub fn for_agent_workflow(embedding_generator: Arc<EmbeddingGenerator>) -> Self {
        let config = InsightsConfig {
            mode: InsightsMode::ContextOnly,
            reranker_config: RerankerConfig {
                embedding_top_k: 50, // More aggressive filtering
                embedding_threshold: 0.4,
                enable_cross_encoder: true,
                cross_encoder_top_k: 15,
                cross_encoder_threshold: 0.6,
                enable_llm_insights: false, // No local LLM
                llm_top_k: 0,
                enable_batch_processing: true,
                batch_size: 64,
                max_concurrent_requests: 8,
            },
            max_context_length: 4000,
            include_metadata: true,
        };

        Self::new(config, embedding_generator)
    }

    /// Create a preset for local LLM processing
    pub fn for_local_llm(embedding_generator: Arc<EmbeddingGenerator>) -> Self {
        let config = InsightsConfig {
            mode: InsightsMode::Balanced,
            reranker_config: RerankerConfig {
                embedding_top_k: 100,
                embedding_threshold: 0.3,
                enable_cross_encoder: true,
                cross_encoder_top_k: 20,
                cross_encoder_threshold: 0.5,
                enable_llm_insights: true,
                llm_top_k: 10, // Only top 10 to LLM
                enable_batch_processing: true,
                batch_size: 32,
                max_concurrent_requests: 4,
            },
            max_context_length: 8000,
            include_metadata: true,
        };

        Self::new(config, embedding_generator)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_insights_modes() {
        // Test that different modes produce different results
        let embedding_gen = Arc::new(EmbeddingGenerator::default());

        let context_only = InsightsGenerator::for_agent_workflow(embedding_gen.clone());
        let local_llm = InsightsGenerator::for_local_llm(embedding_gen.clone());

        assert_eq!(context_only.config.mode, InsightsMode::ContextOnly);
        assert_eq!(local_llm.config.mode, InsightsMode::Balanced);
    }
}
