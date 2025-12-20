// ABOUTME: Provides repository counting and embedding time estimation utilities.
// ABOUTME: Shared helpers for CLI planners and indexer symbol handling logic.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use codegraph_core::{CodeNode, EdgeRelationship, NodeId};
use codegraph_parser::{
    file_collect, file_collect::FileCollectionConfig, ParsingStatistics, TreeSitterParser,
};
use futures::stream::{self, StreamExt};
use serde::Serialize;
use tokio::sync::Semaphore;
use tracing::{debug, info, warn};

use crate::indexer::{filter_edges_for_tier, IndexerConfig};

#[derive(Debug, Clone, Serialize)]
pub struct RepositoryCounts {
    pub total_files: usize,
    pub parsed_files: usize,
    pub failed_files: usize,
    pub nodes: usize,
    pub edges: usize,
    pub symbols: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct ParsingSummary {
    pub total_lines: usize,
    pub duration_seconds: f64,
    pub files_per_second: f64,
    pub lines_per_second: f64,
}

impl From<&ParsingStatistics> for ParsingSummary {
    fn from(stats: &ParsingStatistics) -> Self {
        Self {
            total_lines: stats.total_lines,
            duration_seconds: stats.parsing_duration.as_secs_f64(),
            files_per_second: stats.files_per_second,
            lines_per_second: stats.lines_per_second,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct TimeEstimates {
    pub jina_batches: usize,
    pub jina_batch_size: usize,
    pub jina_batch_minutes: f64,
    pub jina_minutes: f64,
    pub local_minutes: Option<f64>,
    pub local_rate_per_minute: Option<f64>,
}

impl TimeEstimates {
    pub fn from_node_count(node_count: usize, cfg: &EmbeddingThroughputConfig) -> Self {
        let jina_batches = if node_count == 0 {
            0
        } else {
            (node_count + cfg.jina_batch_size - 1) / cfg.jina_batch_size
        };
        let jina_minutes = jina_batches as f64 * cfg.jina_batch_minutes;

        let local_minutes = cfg.local_embeddings_per_minute.and_then(|rate| {
            if rate > 0.0 {
                Some(node_count as f64 / rate)
            } else {
                None
            }
        });

        Self {
            jina_batches,
            jina_batch_size: cfg.jina_batch_size,
            jina_batch_minutes: cfg.jina_batch_minutes,
            jina_minutes,
            local_minutes,
            local_rate_per_minute: cfg.local_embeddings_per_minute,
        }
    }
}

#[derive(Debug, Clone)]
pub struct RepositoryEstimate {
    pub counts: RepositoryCounts,
    pub parsing: ParsingSummary,
    pub parsing_duration: Duration,
    pub timings: TimeEstimates,
}

#[derive(Debug, Clone)]
pub struct EmbeddingThroughputConfig {
    pub jina_batch_size: usize,
    pub jina_batch_minutes: f64,
    pub local_embeddings_per_minute: Option<f64>,
}

impl EmbeddingThroughputConfig {
    pub fn with_local_rate(mut self, rate: Option<f64>) -> Self {
        self.local_embeddings_per_minute = rate;
        self
    }
}

pub struct RepositoryEstimator {
    parser: TreeSitterParser,
    config: IndexerConfig,
}

impl RepositoryEstimator {
    pub fn new(config: IndexerConfig) -> Self {
        Self {
            parser: TreeSitterParser::new(),
            config,
        }
    }

    pub async fn analyze(
        &self,
        path: impl AsRef<Path>,
        throughput: &EmbeddingThroughputConfig,
    ) -> Result<RepositoryEstimate> {
        let path = path.as_ref();
        let file_config: FileCollectionConfig = (&self.config).into();
        let files = file_collect::collect_source_files_with_config(path, &file_config)?;
        let total_files = files.len() as u64;

        let (nodes, mut edges, stats) =
            parse_files_with_unified_extraction(&self.parser, files, total_files).await?;
        filter_edges_for_tier(self.config.indexing_tier, &mut edges);
        let symbol_map = build_symbol_index(&nodes);

        let counts = RepositoryCounts {
            total_files: stats.total_files,
            parsed_files: stats.parsed_files,
            failed_files: stats.failed_files,
            nodes: nodes.len(),
            edges: edges.len(),
            symbols: symbol_map.len(),
        };

        let timings = TimeEstimates::from_node_count(nodes.len(), throughput);
        let parsing_duration = stats.parsing_duration;

        Ok(RepositoryEstimate {
            counts,
            parsing: ParsingSummary::from(&stats),
            parsing_duration,
            timings,
        })
    }
}

pub fn build_symbol_index(nodes: &[CodeNode]) -> HashMap<String, NodeId> {
    let mut symbol_map = HashMap::with_capacity(nodes.len().saturating_mul(4));
    for node in nodes {
        extend_symbol_index(&mut symbol_map, node);
    }
    symbol_map
}

pub(crate) fn extend_symbol_index(target: &mut HashMap<String, NodeId>, node: &CodeNode) {
    let base_name = node.name.to_string();
    target.insert(base_name.clone(), node.id);

    if let Some(qname) = node.metadata.attributes.get("qualified_name") {
        target.insert(qname.clone(), node.id);
    }

    if let Some(node_type) = &node.node_type {
        target.insert(format!("{:?}::{}", node_type, base_name), node.id);
    }

    target.insert(
        format!("{}::{}", node.location.file_path, base_name),
        node.id,
    );

    if let Some(short_name) = base_name.split("::").last() {
        target.insert(short_name.to_string(), node.id);
    }

    if let Some(method_of) = node.metadata.attributes.get("method_of") {
        target.insert(format!("{}::{}", method_of, base_name), node.id);
    }

    if let Some(trait_impl) = node.metadata.attributes.get("implements_trait") {
        target.insert(format!("{}::{}", trait_impl, base_name), node.id);
    }
}

pub(crate) async fn parse_files_with_unified_extraction(
    parser: &TreeSitterParser,
    files: Vec<(PathBuf, u64)>,
    total_files: u64,
) -> Result<(Vec<CodeNode>, Vec<EdgeRelationship>, ParsingStatistics)> {
    let mut all_nodes = Vec::new();
    let mut all_edges = Vec::new();
    let total_lines = 0;
    let mut parsed_files = 0;
    let mut failed_files = 0;

    let semaphore = Arc::new(Semaphore::new(4));
    let start_time = std::time::Instant::now();

    let parser_ref = parser;
    let mut stream = stream::iter(files.into_iter().map(|(file_path, _)| {
        let semaphore = semaphore.clone();
        async move {
            let _permit = semaphore.acquire().await.unwrap();
            parser_ref
                .parse_file_with_edges(&file_path.to_string_lossy())
                .await
        }
    }))
    .buffer_unordered(4);

    while let Some(result) = stream.next().await {
        match result {
            Ok(extraction_result) => {
                let node_count = extraction_result.nodes.len();
                let edge_count = extraction_result.edges.len();

                if node_count > 0 {
                    debug!(
                        "🌳 AST extraction: {} nodes, {} edges from file",
                        node_count, edge_count
                    );
                }

                all_nodes.extend(extraction_result.nodes);
                all_edges.extend(extraction_result.edges);
                parsed_files += 1;
            }
            Err(e) => {
                failed_files += 1;
                warn!("Failed to parse file: {}", e);
            }
        }
    }

    let parsing_duration = start_time.elapsed();
    let files_per_second = if parsing_duration.as_secs_f64() > 0.0 {
        parsed_files as f64 / parsing_duration.as_secs_f64()
    } else {
        0.0
    };
    let lines_per_second = if parsing_duration.as_secs_f64() > 0.0 {
        total_lines as f64 / parsing_duration.as_secs_f64()
    } else {
        0.0
    };

    let stats = ParsingStatistics {
        total_files: total_files.try_into().unwrap_or(usize::MAX),
        parsed_files,
        failed_files,
        total_lines,
        parsing_duration,
        files_per_second,
        lines_per_second,
    };

    info!("🌳 UNIFIED AST EXTRACTION COMPLETE:");
    info!(
        "   📊 Files processed: {}/{} ({:.1}% success rate)",
        parsed_files,
        total_files,
        if total_files > 0 {
            parsed_files as f64 / total_files as f64 * 100.0
        } else {
            100.0
        }
    );
    info!(
        "   🌳 Semantic nodes extracted: {} (functions, structs, classes, imports, etc.)",
        all_nodes.len()
    );
    info!(
        "   🔗 Code relationships found: {} (function calls, imports, dependencies)",
        all_edges.len()
    );
    info!(
        "   ⚡ Processing performance: {:.1} files/s | {:.0} lines/s",
        files_per_second, lines_per_second
    );
    info!(
        "   🎯 Extraction efficiency: {:.1} nodes/file | {:.1} edges/file",
        if parsed_files > 0 {
            all_nodes.len() as f64 / parsed_files as f64
        } else {
            0.0
        },
        if parsed_files > 0 {
            all_edges.len() as f64 / parsed_files as f64
        } else {
            0.0
        }
    );

    if failed_files > 0 {
        warn!(
            "   ⚠️ Parse failures: {} files failed TreeSitter analysis",
            failed_files
        );
    }

    Ok((all_nodes, all_edges, stats))
}
