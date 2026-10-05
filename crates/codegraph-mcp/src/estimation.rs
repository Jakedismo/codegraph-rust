// ABOUTME: Provides repository counting and embedding time estimation utilities.
// ABOUTME: Shared helpers for CLI planners and indexer symbol handling logic.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::Result;
use codegraph_core::{CodeNode, EdgeRelationship, NodeId};
use codegraph_parser::{
    ParsingStatistics, SourceSnapshots, TreeSitterParser, file_collect,
    file_collect::FileCollectionConfig,
};
use futures::stream::{self, StreamExt};
use serde::Serialize;
use tracing::{debug, info, warn};

use crate::indexer::{IndexerConfig, extraction_policy_for_tier, filter_edges_for_tier};

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
    pub cached_files: usize,
    pub duration_seconds: f64,
    pub files_per_second: f64,
    pub lines_per_second: f64,
}

impl From<&ParsingStatistics> for ParsingSummary {
    fn from(stats: &ParsingStatistics) -> Self {
        Self {
            total_lines: stats.total_lines,
            cached_files: stats.cached_files,
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
            parser: TreeSitterParser::new()
                .with_concurrency(config.workers)
                .with_extraction_policy(extraction_policy_for_tier(config.indexing_tier)),
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
    crate::resolution::SymbolCatalog::new(nodes).unique_aliases()
}

pub(crate) async fn parse_files_with_unified_extraction(
    parser: &TreeSitterParser,
    files: Vec<(PathBuf, u64)>,
    total_files: u64,
) -> Result<(Vec<CodeNode>, Vec<EdgeRelationship>, ParsingStatistics)> {
    let snapshots =
        SourceSnapshots::capture(&files, source_memory_budget(), parser.concurrency()).await?;
    parse_snapshots_with_unified_extraction(parser, files, total_files, &snapshots).await
}

pub(crate) fn source_memory_budget() -> usize {
    std::env::var("CODEGRAPH_SOURCE_MEMORY_MB")
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(256)
        .saturating_mul(1024 * 1024)
}

pub(crate) async fn parse_snapshots_with_unified_extraction(
    parser: &TreeSitterParser,
    files: Vec<(PathBuf, u64)>,
    total_files: u64,
    snapshots: &SourceSnapshots,
) -> Result<(Vec<CodeNode>, Vec<EdgeRelationship>, ParsingStatistics)> {
    parse_snapshots_with_cache(parser, files, total_files, snapshots, None).await
}

pub(crate) async fn parse_snapshots_with_cache(
    parser: &TreeSitterParser,
    mut files: Vec<(PathBuf, u64)>,
    total_files: u64,
    snapshots: &SourceSnapshots,
    cache: Option<&codegraph_core::artifact_cache::ArtifactCache>,
) -> Result<(Vec<CodeNode>, Vec<EdgeRelationship>, ParsingStatistics)> {
    let mut all_nodes = Vec::new();
    let mut all_edges = Vec::new();
    let mut total_lines = 0;
    let mut parsed_files = 0;
    let mut failed_files = 0;
    let mut cached_files = 0;

    let start_time = std::time::Instant::now();

    let parser_ref = parser;
    let registry = codegraph_parser::LanguageRegistry::new();
    let registry = &registry;
    files.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    let mut stream = stream::iter(files.into_iter().map(|(file_path, _)| async move {
        let result = async {
            let snapshot = snapshots
                .get(&file_path)
                .ok_or_else(|| anyhow::anyhow!("Missing snapshot: {}", file_path.display()))?;
            let language = registry
                .detect_language(&file_path.to_string_lossy())
                .ok_or_else(|| anyhow::anyhow!("Unsupported source: {}", file_path.display()))?;
            let policy = parser_ref.extraction_policy();
            let cache_key = codegraph_core::artifact_cache::fingerprint(&(
                "unified-ast-v3",
                &file_path,
                &snapshot.content_hash,
                policy.uses,
                policy.references,
            ))?;
            if let Some(cache) = cache {
                let cache = cache.clone();
                let key = cache_key.clone();
                if let Some(extraction) = tokio::task::spawn_blocking(move || {
                    cache.get::<codegraph_core::ExtractionResult>(&key)
                })
                .await?
                {
                    return Ok((extraction, snapshot.lines, true));
                }
            }
            let extraction = parser_ref
                .parse_source_with_edges(
                    snapshot.contents_async().await?,
                    &file_path.to_string_lossy(),
                    language,
                )
                .await?;
            let extraction = if let Some(cache) = cache {
                let cache = cache.clone();
                tokio::task::spawn_blocking(move || {
                    if let Err(error) = cache.put(&cache_key, &extraction) {
                        warn!("AST cache write failed: {error}");
                    }
                    extraction
                })
                .await?
            } else {
                extraction
            };
            Ok::<_, anyhow::Error>((extraction, snapshot.lines, false))
        }
        .await;
        (file_path, result)
    }))
    .buffer_unordered(parser.concurrency());

    while let Some((file_path, result)) = stream.next().await {
        match result {
            Ok((mut extraction_result, lines, cached)) => {
                cached_files += usize::from(cached);
                total_lines += lines;
                for edge in &mut extraction_result.edges {
                    edge.metadata
                        .entry("source_file".to_string())
                        .or_insert_with(|| file_path.to_string_lossy().to_string());
                }
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
                warn!("Failed to parse file {}: {}", file_path.display(), e);
            }
        }
    }

    all_nodes.sort_by(|a, b| {
        (
            &a.location.file_path,
            a.location.line,
            a.location.column,
            a.name.as_str(),
        )
            .cmp(&(
                &b.location.file_path,
                b.location.line,
                b.location.column,
                b.name.as_str(),
            ))
    });
    all_edges.sort_by(|a, b| {
        (
            a.metadata.get("source_file"),
            a.span.as_ref().map(|s| s.start_byte),
            &a.to,
            format!("{:?}", a.edge_type),
        )
            .cmp(&(
                b.metadata.get("source_file"),
                b.span.as_ref().map(|s| s.start_byte),
                &b.to,
                format!("{:?}", b.edge_type),
            ))
    });

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
        cached_files,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn ast_cache_reuses_unchanged_files_and_invalidates_source_and_tier_changes() {
        let directory = tempfile::tempdir().unwrap();
        let file = directory.path().join("a.rs");
        let files = vec![(file.clone(), 0)];
        std::fs::write(&file, "fn initial() {}\n").unwrap();
        let cache = codegraph_core::artifact_cache::ArtifactCache::new(
            directory.path().join("cache"),
            "test-ast",
        );
        let parser = TreeSitterParser::new();
        for expected_cached in [0, 1] {
            let sources = SourceSnapshots::capture(&files, 0, 2).await.unwrap();
            let (nodes, _, stats) =
                parse_snapshots_with_cache(&parser, files.clone(), 1, &sources, Some(&cache))
                    .await
                    .unwrap();
            assert!(nodes.iter().any(|node| node.name.as_str() == "initial"));
            assert_eq!(stats.cached_files, expected_cached);
        }
        std::fs::write(&file, "fn changed() {}\n").unwrap();
        let sources = SourceSnapshots::capture(&files, 0, 2).await.unwrap();
        let (nodes, _, stats) =
            parse_snapshots_with_cache(&parser, files.clone(), 1, &sources, Some(&cache))
                .await
                .unwrap();
        assert_eq!(stats.cached_files, 0);
        assert!(nodes.iter().any(|node| node.name.as_str() == "changed"));
        let fast = TreeSitterParser::new().with_extraction_policy(
            codegraph_parser::languages::ExtractionPolicy {
                uses: false,
                references: false,
            },
        );
        let (_, _, stats) = parse_snapshots_with_cache(&fast, files, 1, &sources, Some(&cache))
            .await
            .unwrap();
        assert_eq!(stats.cached_files, 0);
    }

    #[tokio::test]
    async fn source_lines_and_node_order_do_not_depend_on_worker_count() {
        let dir = tempfile::tempdir().unwrap();
        let files: Vec<_> = ["b.rs", "a.rs"]
            .into_iter()
            .map(|name| {
                let path = dir.path().join(name);
                std::fs::write(&path, "fn outer() { inner(); }\nfn inner() {}\n").unwrap();
                (path, 0)
            })
            .collect();
        let snapshots = SourceSnapshots::capture(&files, 0, 2).await.unwrap();
        let mut orders = Vec::new();
        for workers in [1, 4] {
            let parser = TreeSitterParser::new().with_concurrency(workers);
            let (nodes, _, stats) =
                parse_snapshots_with_unified_extraction(&parser, files.clone(), 2, &snapshots)
                    .await
                    .unwrap();
            assert_eq!(stats.total_lines, 4);
            assert_eq!(stats.parsed_files, 2);
            orders.push(
                nodes
                    .into_iter()
                    .map(|node| {
                        (
                            node.location.file_path,
                            node.location.line,
                            node.name.to_string(),
                        )
                    })
                    .collect::<Vec<_>>(),
            );
        }
        assert_eq!(orders[0], orders[1]);
    }
}
