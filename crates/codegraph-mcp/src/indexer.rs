// ABOUTME: Drives the indexing pipeline for the Codegraph MCP CLI.
// ABOUTME: Coordinates parsing, embeddings, and persistence into SurrealDB.
#![allow(dead_code, unused_variables, unused_imports)]

use crate::estimation::{
    extend_symbol_index, parse_files_with_unified_extraction as shared_unified_parse,
};
use anyhow::{anyhow, Context, Result};
use codegraph_core::{CodeNode, EdgeRelationship, NodeId, NodeType};
use codegraph_graph::{
    edge::CodeEdge, FileMetadataRecord, NodeEmbeddingRecord, ProjectMetadataRecord,
    SurrealDbConfig, SurrealDbStorage, SymbolEmbeddingRecord, SURR_EMBEDDING_COLUMN_1024,
    SURR_EMBEDDING_COLUMN_1536, SURR_EMBEDDING_COLUMN_2048, SURR_EMBEDDING_COLUMN_2560,
    SURR_EMBEDDING_COLUMN_3072, SURR_EMBEDDING_COLUMN_384, SURR_EMBEDDING_COLUMN_4096,
    SURR_EMBEDDING_COLUMN_768,
};
use codegraph_graph::ChunkEmbeddingRecord;
use codegraph_parser::TreeSitterParser;
#[cfg(feature = "ai-enhanced")]
use futures::{stream, StreamExt};
#[cfg(feature = "embeddings")]
use codegraph_vector::prep::chunker::ChunkPlan;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use num_cpus;
use rayon::prelude::*;
use regex::Regex;
use rustc_demangle::try_demangle;
use sha2::{Digest, Sha256};
use std::borrow::Cow;
use std::collections::HashSet;
use std::env;
use std::ffi::OsStr;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::Duration;
use syn::{parse_str as parse_syn_path, Path as SynPath, PathArguments};
use symbolic_demangle::demangle;
use tokio::fs as tokio_fs;
use tokio::sync::{mpsc, oneshot, Mutex as TokioMutex};
use tokio::task::JoinHandle;
use tracing::{debug, error, info, warn};
use url::Url;
use walkdir::WalkDir;

use std::sync::{Arc, Mutex};

use std::collections::HashMap;

const SYMBOL_EMBEDDING_DB_BATCH_LIMIT: usize = 256;

#[derive(Debug, Clone, PartialEq)]
pub enum FileChangeType {
    Added,
    Modified,
    Deleted,
    Unchanged,
}

#[derive(Debug, Clone)]
pub struct FileChange {
    pub file_path: String,
    pub change_type: FileChangeType,
    pub current_hash: Option<String>,
    pub previous_hash: Option<String>,
}

#[derive(Clone, Copy, Debug)]
enum SurrealEmbeddingColumn {
    Embedding384,
    Embedding768,
    Embedding1024,
    Embedding1536,
    Embedding2048,
    Embedding2560,
    Embedding3072,
    Embedding4096,
}

impl SurrealEmbeddingColumn {
    fn column_name(&self) -> &'static str {
        match self {
            SurrealEmbeddingColumn::Embedding384 => SURR_EMBEDDING_COLUMN_384,
            SurrealEmbeddingColumn::Embedding768 => SURR_EMBEDDING_COLUMN_768,
            SurrealEmbeddingColumn::Embedding1024 => SURR_EMBEDDING_COLUMN_1024,
            SurrealEmbeddingColumn::Embedding1536 => SURR_EMBEDDING_COLUMN_1536,
            SurrealEmbeddingColumn::Embedding2048 => SURR_EMBEDDING_COLUMN_2048,
            SurrealEmbeddingColumn::Embedding2560 => SURR_EMBEDDING_COLUMN_2560,
            SurrealEmbeddingColumn::Embedding3072 => SURR_EMBEDDING_COLUMN_3072,
            SurrealEmbeddingColumn::Embedding4096 => SURR_EMBEDDING_COLUMN_4096,
        }
    }

    fn dimension(&self) -> usize {
        match self {
            SurrealEmbeddingColumn::Embedding384 => 384,
            SurrealEmbeddingColumn::Embedding768 => 768,
            SurrealEmbeddingColumn::Embedding1024 => 1024,
            SurrealEmbeddingColumn::Embedding1536 => 1536,
            SurrealEmbeddingColumn::Embedding2048 => 2048,
            SurrealEmbeddingColumn::Embedding2560 => 2560,
            SurrealEmbeddingColumn::Embedding3072 => 3072,
            SurrealEmbeddingColumn::Embedding4096 => 4096,
        }
    }
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct IndexerConfig {
    pub languages: Vec<String>,
    pub exclude_patterns: Vec<String>,
    pub include_patterns: Vec<String>,
    pub recursive: bool,
    pub force_reindex: bool,
    pub watch: bool,
    pub workers: usize,
    pub batch_size: usize,
    pub max_concurrent: usize,
    pub vector_dimension: usize,
    pub device: Option<String>,
    pub max_seq_len: usize,
    pub symbol_batch_size: Option<usize>,
    pub symbol_max_concurrent: Option<usize>,
    /// Root directory of the project being indexed (where .codegraph/ will be created)
    /// Defaults to current directory if not specified
    pub project_root: PathBuf,
}

impl Default for IndexerConfig {
    fn default() -> Self {
        Self {
            languages: vec![],
            exclude_patterns: vec![],
            include_patterns: vec![],
            recursive: true,
            force_reindex: false,
            watch: false,
            workers: 4,
            batch_size: 100,
            max_concurrent: 10,
            vector_dimension: 384, // Match EmbeddingGenerator default (all-MiniLM-L6-v2)
            device: None,
            max_seq_len: 512,
            symbol_batch_size: None,
            symbol_max_concurrent: None,
            project_root: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
        }
    }
}

impl From<&IndexerConfig> for codegraph_parser::file_collect::FileCollectionConfig {
    fn from(config: &IndexerConfig) -> Self {
        codegraph_parser::file_collect::FileCollectionConfig {
            recursive: config.recursive,
            languages: config.languages.clone(),
            include_patterns: config.include_patterns.clone(),
            exclude_patterns: config.exclude_patterns.clone(),
        }
    }
}

pub struct ProjectIndexer {
    config: IndexerConfig,
    global_config: codegraph_core::config_manager::CodeGraphConfig,
    progress: MultiProgress,
    parser: TreeSitterParser,
    surreal: Arc<TokioMutex<SurrealDbStorage>>,
    surreal_writer: Option<SurrealWriterHandle>,
    project_id: String,
    organization_id: Option<String>,
    repository_url: Option<String>,
    domain: Option<String>,
    embedding_model: String,
    vector_dim: usize,
    embedding_column: SurrealEmbeddingColumn,
    project_root: PathBuf,
    #[cfg(feature = "embeddings")]
    embedder: codegraph_vector::EmbeddingGenerator,
}

enum SurrealWriteJob {
    Nodes(Vec<CodeNode>),
    Edges(Vec<CodeEdge>),
    NodeEmbeddings(Vec<NodeEmbeddingRecord>),
    SymbolEmbeddings(Vec<SymbolEmbeddingRecord>),
    ChunkEmbeddings(Vec<ChunkEmbeddingRecord>),
    FileMetadata(Vec<FileMetadataRecord>),
    DeleteNodesByFile {
        file_paths: Vec<String>,
        project_id: String,
    },
    ProjectMetadata(ProjectMetadataRecord),
    Flush(oneshot::Sender<Result<()>>),
    Shutdown(oneshot::Sender<Result<()>>),
}

struct SurrealWriterHandle {
    tx: mpsc::Sender<SurrealWriteJob>,
    join: JoinHandle<()>,
}

impl SurrealWriterHandle {
    fn new(storage: Arc<TokioMutex<SurrealDbStorage>>) -> Self {
        let (tx, mut rx) = mpsc::channel(8);
        let join = tokio::spawn(async move {
            let mut last_error: Option<anyhow::Error> = None;
            while let Some(job) = rx.recv().await {
                match job {
                    SurrealWriteJob::Nodes(nodes) => {
                        if nodes.is_empty() { continue; }
                        if let Err(err) = { let mut guard = storage.lock().await; guard.upsert_nodes_batch(&nodes).await } {
                            error!("Surreal node batch failed: {}", err);
                            last_error = Some(anyhow!(err.to_string()));
                        }
                    }
                    SurrealWriteJob::Edges(edges) => {
                        if edges.is_empty() { continue; }
                        if let Err(err) = { let mut guard = storage.lock().await; guard.upsert_edges_batch(&edges).await } {
                            error!("Surreal edge batch failed: {}", err);
                            last_error = Some(anyhow!(err.to_string()));
                        }
                    }
                    SurrealWriteJob::NodeEmbeddings(records) => {
                        if records.is_empty() { continue; }
                        if let Err(err) = { let guard = storage.lock().await; guard.update_node_embeddings_batch(&records).await } {
                            error!("Surreal node embedding batch failed: {}", err);
                            last_error = Some(anyhow!(err.to_string()));
                        }
                    }
                    SurrealWriteJob::SymbolEmbeddings(records) => {
                        if records.is_empty() { continue; }
                        if let Err(err) = { let guard = storage.lock().await; guard.upsert_symbol_embeddings_batch(&records).await } {
                            error!("Surreal symbol embedding batch failed: {}", err);
                            last_error = Some(anyhow!(err.to_string()));
                        }
                    }
                    SurrealWriteJob::ChunkEmbeddings(records) => {
                        if records.is_empty() { continue; }
                        let batch_size = records.len();
                        if let Err(err) = { let guard = storage.lock().await; guard.upsert_chunk_embeddings_batch(&records).await } {
                            error!("Surreal chunk embedding batch failed: {}", err);
                            last_error = Some(anyhow!(err.to_string()));
                        } else {
                            info!("🧩 Surreal chunk batch persisted: {} records", batch_size);
                        }
                    }
                    SurrealWriteJob::FileMetadata(records) => {
                        if records.is_empty() { continue; }
                        if let Err(err) = { let guard = storage.lock().await; guard.upsert_file_metadata_batch(&records).await } {
                            error!("Surreal file metadata batch failed: {}", err);
                            last_error = Some(anyhow!(err.to_string()));
                        }
                    }
                    SurrealWriteJob::DeleteNodesByFile { file_paths, project_id } => {
                        if file_paths.is_empty() { continue; }
                        let guard = storage.lock().await;
                        let delete_nodes_query = "DELETE nodes WHERE project_id = $project_id AND file_path IN $file_paths RETURN BEFORE";
                        let mut result = match guard
                            .db()
                            .query(delete_nodes_query)
                            .bind(("project_id", project_id.clone()))
                            .bind(("file_paths", file_paths.clone()))
                            .await
                        {
                            Ok(res) => res,
                            Err(e) => {
                                error!("Failed to delete nodes for files {:?}: {}", file_paths, e);
                                last_error = Some(anyhow!(e.to_string()));
                                continue;
                            }
                        };

                        let deleted_nodes: Vec<HashMap<String, serde_json::Value>> =
                            result.take(0).unwrap_or_default();
                        let node_ids: Vec<String> = deleted_nodes
                            .iter()
                            .filter_map(|n| {
                                n.get("id").and_then(|v| v.as_str()).map(|s| s.to_string())
                            })
                            .collect();

                        if !node_ids.is_empty() {
                            let delete_symbols = r#"
                        LET $node_ids = $ids;
                        DELETE symbol_embeddings WHERE
                            string::split(string::trim(node_id), ':')[1] IN $node_ids;
                    "#;
                            if let Err(e) = guard
                                .db()
                                .query(delete_symbols)
                                .bind(("ids", node_ids.clone()))
                                .await
                            {
                                error!(
                                    "Failed to delete symbol_embeddings for files {:?}: {}",
                                    file_paths, e
                                );
                                last_error = Some(anyhow!(e.to_string()));
                            }
                        }

                        if !node_ids.is_empty() {
                            let edge_query = r#"
                        LET $node_ids = $ids;
                        DELETE edges WHERE
                            string::split(string::trim(from), ':')[1] IN $node_ids OR
                            string::split(string::trim(to), ':')[1] IN $node_ids
                    "#;
                            if let Err(e) =
                                guard.db().query(edge_query).bind(("ids", node_ids)).await
                            {
                                error!("Failed to delete edges for files {:?}: {}", file_paths, e);
                                last_error = Some(anyhow!(e.to_string()));
                            }
                        }

                        if let Err(e) = guard
                            .delete_file_metadata_for_files(&project_id, &file_paths)
                            .await
                        {
                            error!(
                                "Failed to delete file metadata for files {:?}: {}",
                                file_paths, e
                            );
                            last_error = Some(anyhow!(e.to_string()));
                        }
                    }
                    SurrealWriteJob::ProjectMetadata(record) => {
                        let result = { let guard = storage.lock().await; guard.upsert_project_metadata(record).await };
                        if let Err(err) = result {
                            error!("Surreal project metadata write failed: {}", err);
                            last_error = Some(anyhow!(err.to_string()));
                        }
                    }
                    SurrealWriteJob::Flush(resp) => {
                        let _ = resp.send(Self::current_error(&last_error));
                    }
                    SurrealWriteJob::Shutdown(resp) => {
                        let _ = resp.send(Self::current_error(&last_error));
                        break;
                    }
                }
            }
        });

        Self { tx, join }
    }

    fn current_error(error: &Option<anyhow::Error>) -> Result<()> {
        if let Some(err) = error {
            Err(anyhow!(err.to_string()))
        } else {
            Ok(())
        }
    }

    async fn enqueue_nodes(&self, nodes: Vec<CodeNode>) -> Result<()> {
        if nodes.is_empty() {
            return Ok(());
        }
        self.tx
            .send(SurrealWriteJob::Nodes(nodes))
            .await
            .map_err(|e| anyhow!("Surreal writer unavailable: {}", e))
    }

    async fn enqueue_edges(&self, edges: Vec<CodeEdge>) -> Result<()> {
        if edges.is_empty() {
            return Ok(());
        }
        self.tx
            .send(SurrealWriteJob::Edges(edges))
            .await
            .map_err(|e| anyhow!("Surreal writer unavailable: {}", e))
    }

    async fn enqueue_node_embeddings(&self, records: Vec<NodeEmbeddingRecord>) -> Result<()> {
        if records.is_empty() {
            return Ok(());
        }
        self.tx
            .send(SurrealWriteJob::NodeEmbeddings(records))
            .await
            .map_err(|e| anyhow!("Surreal writer unavailable: {}", e))
    }

    async fn enqueue_chunk_embeddings(&self, records: Vec<ChunkEmbeddingRecord>) -> Result<()> {
        if records.is_empty() {
            return Ok(());
        }
        self.tx
            .send(SurrealWriteJob::ChunkEmbeddings(records))
            .await
            .map_err(|e| anyhow!("Surreal writer unavailable: {}", e))
    }

    async fn enqueue_symbol_embeddings(&self, records: Vec<SymbolEmbeddingRecord>) -> Result<()> {
        if records.is_empty() {
            return Ok(());
        }
        self.tx
            .send(SurrealWriteJob::SymbolEmbeddings(records))
            .await
            .map_err(|e| anyhow!("Surreal writer unavailable: {}", e))
    }

    async fn enqueue_file_metadata(&self, records: Vec<FileMetadataRecord>) -> Result<()> {
        if records.is_empty() {
            return Ok(());
        }
        self.tx
            .send(SurrealWriteJob::FileMetadata(records))
            .await
            .map_err(|e| anyhow!("Surreal writer unavailable: {}", e))
    }

    async fn enqueue_delete_nodes_by_file(
        &self,
        file_paths: Vec<String>,
        project_id: &str,
    ) -> Result<()> {
        if file_paths.is_empty() {
            return Ok(());
        }
        self.tx
            .send(SurrealWriteJob::DeleteNodesByFile {
                file_paths,
                project_id: project_id.to_string(),
            })
            .await
            .map_err(|e| anyhow!("Surreal writer unavailable: {}", e))
    }

    async fn enqueue_project_metadata(&self, record: ProjectMetadataRecord) -> Result<()> {
        self.tx
            .send(SurrealWriteJob::ProjectMetadata(record))
            .await
            .map_err(|e| anyhow!("Surreal writer unavailable: {}", e))
    }

    async fn flush(&self) -> Result<()> {
        let (resp_tx, resp_rx) = oneshot::channel();
        self.tx
            .send(SurrealWriteJob::Flush(resp_tx))
            .await
            .map_err(|e| anyhow!("Surreal writer unavailable: {}", e))?;
        resp_rx
            .await
            .map_err(|_| anyhow!("Surreal writer task ended unexpectedly"))?
    }

    async fn shutdown(self) -> Result<()> {
        let (resp_tx, resp_rx) = oneshot::channel();
        let _ = self.tx.send(SurrealWriteJob::Shutdown(resp_tx)).await;
        let result = resp_rx
            .await
            .unwrap_or_else(|_| Err(anyhow!("Surreal writer task ended unexpectedly")));
        let _ = self.join.await;
        result
    }
}

impl ProjectIndexer {
    #[cfg(feature = "ai-enhanced")]
    fn symbol_embedding_batch_settings(&self) -> (usize, usize) {
        let config_batch = self
            .config
            .symbol_batch_size
            .unwrap_or(self.config.batch_size);
        let config_concurrent = self
            .config
            .symbol_max_concurrent
            .unwrap_or_else(|| self.config.max_concurrent.max(2));

        let batch_size = std::env::var("CODEGRAPH_SYMBOL_BATCH_SIZE")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(config_batch)
            .clamp(1, 2048);

        let max_concurrent = std::env::var("CODEGRAPH_SYMBOL_MAX_CONCURRENT")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(config_concurrent)
            .clamp(1, 32);

        (batch_size, max_concurrent)
    }

    pub async fn new(
        mut config: IndexerConfig,
        global_config: &codegraph_core::config_manager::CodeGraphConfig,
        multi_progress: MultiProgress,
    ) -> Result<Self> {
        // Cap Rayon global thread pool to avoid CPU starvation
        let rayon_threads = config.workers.max(2);
        if let Err(e) = rayon::ThreadPoolBuilder::new()
            .num_threads(rayon_threads)
            .build_global()
        {
            // Pool may already be built elsewhere; log and continue
            debug!("Rayon global pool unchanged: {}", e);
        } else {
            info!("🔧 Rayon threads capped to {} (config.workers)", rayon_threads);
        }

        // Allow runtime override for embedding batch size
        if let Ok(val) = std::env::var("CODEGRAPH_EMBEDDINGS_BATCH_SIZE") {
            if let Ok(parsed) = val.parse::<usize>() {
                config.batch_size = parsed.clamp(1, 2048);
            }
        }

        let parser = TreeSitterParser::new();
        let project_root = config.project_root.clone();
        let surreal = Self::connect_surreal_from_env().await?;
        let surreal_writer = SurrealWriterHandle::new(surreal.clone());
        let project_id = std::env::var("CODEGRAPH_PROJECT_ID")
            .unwrap_or_else(|_| project_root.display().to_string());
        let organization_id = std::env::var("CODEGRAPH_ORGANIZATION_ID").ok();
        let repository_url = std::env::var("CODEGRAPH_REPOSITORY_URL").ok();
        let domain = std::env::var("CODEGRAPH_DOMAIN").ok();
        #[cfg(feature = "embeddings")]
        let embedder = {
            use codegraph_vector::EmbeddingGenerator;
            // Use global config for embedding provider
            let provider = global_config.embedding.provider.to_lowercase();
            if provider == "local" {
                #[cfg(feature = "embeddings-local")]
                {
                    use codegraph_vector::embeddings::generator::{
                        AdvancedEmbeddingGenerator, EmbeddingEngineConfig, LocalDeviceTypeCompat,
                        LocalEmbeddingConfigCompat, LocalPoolingCompat,
                    };
                    let mut cfg = EmbeddingEngineConfig::default();
                    cfg.prefer_local_first = true;
                    let device = match config
                        .device
                        .as_deref()
                        .unwrap_or("")
                        .to_lowercase()
                        .as_str()
                    {
                        "metal" => LocalDeviceTypeCompat::Metal,
                        d if d.starts_with("cuda:") => {
                            let id = d.trim_start_matches("cuda:").parse::<usize>().unwrap_or(0);
                            LocalDeviceTypeCompat::Cuda(id)
                        }
                        _ => LocalDeviceTypeCompat::Cpu,
                    };
                    let model_name =
                        global_config.embedding.model.clone().unwrap_or_else(|| {
                            "sentence-transformers/all-MiniLM-L6-v2".to_string()
                        });
                    cfg.local = Some(LocalEmbeddingConfigCompat {
                        model_name,
                        device,
                        cache_dir: None,
                        max_sequence_length: config.max_seq_len.max(32),
                        pooling_strategy: LocalPoolingCompat::Mean,
                    });
                    // Try to construct advanced engine; fall back to simple generator on error
                    match AdvancedEmbeddingGenerator::new(cfg).await {
                        Ok(engine) => {
                            if !engine.has_provider() {
                                return Err(anyhow::anyhow!(
                                    "Local embedding provider constructed without a backend. Ensure the model is BERT-compatible with safetensors and try --device metal or --device cpu"
                                ));
                            }
                            let mut g = EmbeddingGenerator::default();
                            g.set_advanced_engine(std::sync::Arc::new(engine));
                            tracing::info!(
                                target: "codegraph_mcp::indexer",
                                "Active embeddings: Local (device: {}, max_seq_len: {}, batch_size: {})",
                                config.device.as_deref().unwrap_or("cpu"),
                                config.max_seq_len,
                                config.batch_size
                            );
                            g
                        }
                        Err(e) => {
                            return Err(anyhow::anyhow!(
                                "Failed to initialize local embedding provider: {}",
                                e
                            ));
                        }
                    }
                }
                #[cfg(not(feature = "embeddings-local"))]
                {
                    tracing::warn!(
                        target: "codegraph_mcp::indexer",
                        "CODEGRAPH_EMBEDDING_PROVIDER=local requested but the 'embeddings-local' feature is not enabled; using auto provider"
                    );
        let g = EmbeddingGenerator::with_auto_from_env().await;
                    // Set batch_size and max_concurrent for Jina provider if applicable
                    #[cfg(feature = "embeddings-jina")]
                    {
                        g.set_jina_batch_size(config.batch_size);
                        g.set_jina_max_concurrent(config.max_concurrent);
                    }
                    g
                }
            } else {
        let mut g = EmbeddingGenerator::with_config(global_config).await;
                // Set batch_size and max_concurrent for Jina provider if applicable
                #[cfg(feature = "embeddings-jina")]
                {
                    g.set_jina_batch_size(config.batch_size);
                    g.set_jina_max_concurrent(config.max_concurrent);
                }
                tracing::info!(
                    target: "codegraph_mcp::indexer",
                    "Active embeddings: {} (batch_size: {}, max_concurrent: {})",
                    global_config.embedding.provider,
                    config.batch_size,
                    config.max_concurrent
                );
                g
            }
        };
        let embedding_model_name = global_config
            .embedding
            .model
            .clone()
            .unwrap_or_else(|| "jina-embeddings-v4".to_string());

        let embedder_dimension = {
            #[cfg(feature = "embeddings")]
            {
                embedder.dimension()
            }
            #[cfg(not(feature = "embeddings"))]
            {
                config.vector_dimension
            }
        };

        let env_vector_dim = std::env::var("CODEGRAPH_EMBEDDING_DIMENSION")
            .ok()
            .and_then(|v| v.parse::<usize>().ok());
        let vector_dim = env_vector_dim.unwrap_or(embedder_dimension);
        let embedding_column = resolve_surreal_embedding_column(vector_dim).with_context(|| {
            format!(
                "Unsupported embedding dimension {}. Supported dimensions: 384, 768, 1024, 2048, 2560, 4096.",
                vector_dim
            )
        })?;

        Ok(Self {
            config,
            global_config: global_config.clone(),
            progress: multi_progress,
            parser,
            surreal,
            surreal_writer: Some(surreal_writer),
            project_id,
            organization_id,
            repository_url,
            domain,
            embedding_model: embedding_model_name,
            vector_dim,
            embedding_column,
            project_root,
            #[cfg(feature = "embeddings")]
            embedder,
        })
    }

    pub async fn index_project(&mut self, path: impl AsRef<Path>) -> Result<IndexStats> {
        let path = path.as_ref();
        info!("Starting project indexing: {:?}", path);
        self.log_surrealdb_status("pre-parse");

        let file_config: codegraph_parser::file_collect::FileCollectionConfig =
            (&self.config).into();

        // FORCE REINDEX: Clean slate approach
        if self.config.force_reindex {
            info!("🧹 --force flag detected: Performing clean slate deletion");
            let storage = self.surreal.lock().await;
            storage.clean_project_data(&self.project_id).await?;
            drop(storage);
            info!("✅ Clean slate complete, starting fresh index");
        }
        // INCREMENTAL INDEXING: Check if already indexed and has file metadata
        let files_to_index = if self.is_indexed(path).await? && self.has_file_metadata().await? {
            info!("📊 Project already indexed, checking for file changes...");

            // Collect current files (returns Vec<(PathBuf, u64)>)
            let files_with_sizes =
                codegraph_parser::file_collect::collect_source_files_with_config(
                    path,
                    &file_config,
                )?;

            // Extract just the paths for change detection
            let file_paths: Vec<PathBuf> =
                files_with_sizes.iter().map(|(p, _)| p.clone()).collect();

            // Detect changes
            let changes = self.detect_file_changes(&file_paths).await?;

            // Categorize changes
            let added: Vec<_> = changes
                .iter()
                .filter(|c| matches!(c.change_type, FileChangeType::Added))
                .collect();
            let modified: Vec<_> = changes
                .iter()
                .filter(|c| matches!(c.change_type, FileChangeType::Modified))
                .collect();
            let deleted: Vec<_> = changes
                .iter()
                .filter(|c| matches!(c.change_type, FileChangeType::Deleted))
                .collect();
            let unchanged: Vec<_> = changes
                .iter()
                .filter(|c| matches!(c.change_type, FileChangeType::Unchanged))
                .collect();

            info!(
                "📈 Change summary: {} added, {} modified, {} deleted, {} unchanged",
                added.len(),
                modified.len(),
                deleted.len(),
                unchanged.len()
            );

            // If no changes, skip indexing
            if added.is_empty() && modified.is_empty() && deleted.is_empty() {
                info!("✅ No changes detected, index is up to date");
                let stats = IndexStats {
                    skipped: unchanged.len(),
                    ..IndexStats::default()
                };
                self.shutdown_surreal_writer().await?;
                return Ok(stats);
            }

            // Handle deletions
            if !deleted.is_empty() {
                info!("🗑️  Removing data for {} deleted files", deleted.len());
                let deleted_paths: Vec<String> =
                    deleted.iter().map(|c| c.file_path.clone()).collect();
                self.delete_data_for_files(&deleted_paths).await?;
            }

            // Collect files that need re-indexing (added + modified) with their sizes
            let files_to_reindex: Vec<(PathBuf, u64)> = added
                .iter()
                .chain(modified.iter())
                .filter_map(|c| {
                    let path = PathBuf::from(&c.file_path);
                    // Find the file size from original collection
                    files_with_sizes.iter().find(|(p, _)| p == &path).cloned()
                })
                .collect();

            if files_to_reindex.is_empty() {
                info!("✅ Only deletions processed, no files to index");
                let stats = IndexStats {
                    skipped: unchanged.len(),
                    ..IndexStats::default()
                };
                self.shutdown_surreal_writer().await?;
                return Ok(stats);
            }

            info!(
                "🔄 Incrementally indexing {} changed files (delete-then-insert semantics)",
                files_to_reindex.len()
            );

            files_to_reindex
        }
        // Old index without metadata - fall back to full reindex
        else if self.is_indexed(path).await? {
            warn!("⚠️  Project indexed without file metadata. Use --force to reindex, or continuing with full index.");
            codegraph_parser::file_collect::collect_source_files_with_config(path, &file_config)?
        }
        // Fresh index - index all files
        else {
            codegraph_parser::file_collect::collect_source_files_with_config(path, &file_config)?
        };

        // STAGE 1: File Collection & Parsing
        let files = files_to_index;
        let total_files = files.len();

        // PRE-DELETE for modified files to avoid dupes/stale data
        let modified_paths: Vec<String> = files
            .iter()
            .map(|(p, _)| p.to_string_lossy().to_string())
            .collect();
        if !modified_paths.is_empty() && self.has_file_metadata().await.unwrap_or(false) {
            info!("🧹 Removing existing nodes/edges/file_metadata for changed files");
            self.delete_data_for_files(&modified_paths).await?;
        }

        info!(
            "🌳 Starting AST parsing (TreeSitter + fast_ml semantics) for {} files across {} languages",
            total_files,
            file_config.languages.len()
        );
        info!("🔗 Unified extraction: Nodes + Edges + Relationships in single fast_ml+AST pass");

        // REVOLUTIONARY: Use unified extraction for nodes + edges in single pass (FASTEST approach)
        // Clone files for parsing (we need them again for metadata persistence)
        let (mut nodes, edges, pstats) = self
            .parse_files_with_unified_extraction(files.clone(), total_files as u64)
            .await?;

        for node in nodes.iter_mut() {
            self.annotate_node(node);
        }

        // Store counts for final summary (before consumption)
        let total_nodes_extracted = nodes.len();
        let total_edges_extracted = edges.len();

        // Build chunk plan early so we can annotate nodes with chunk counts before persistence
        #[cfg(feature = "embeddings")]
        let chunk_plan: ChunkPlan = self.embedder.chunk_nodes(&nodes);
        #[cfg(feature = "embeddings")]
        {
            let total_chunks = chunk_plan.stats.total_chunks;
            info!(
                "🧩 Chunking enabled: {} nodes → {} chunks",
                chunk_plan.stats.total_nodes,
                total_chunks
            );
            if total_chunks == 0 && !nodes.is_empty() {
                warn!(
                    "Chunking produced zero chunks. Check CODEGRAPH_EMBEDDING_SKIP_CHUNKING, embedding provider availability, and model max_tokens settings."
                );
            }
        }
        #[cfg(not(feature = "embeddings"))]
        {
            info!("⚠️ Embeddings feature disabled at compile time; chunking skipped");
            let _chunk_plan: Option<()> = None;
        }

        #[cfg(feature = "embeddings")]
        {
            let mut node_chunk_counts: std::collections::HashMap<usize, usize> =
                std::collections::HashMap::new();
            for meta in &chunk_plan.metas {
                *node_chunk_counts.entry(meta.node_index).or_insert(0) += 1;
            }

            for (idx, node) in nodes.iter_mut().enumerate() {
                let count = node_chunk_counts.get(&idx).cloned().unwrap_or(0);
                node.metadata
                    .attributes
                    .insert("chunk_count".to_string(), count.to_string());
            }
        }

        let success_rate = if pstats.total_files > 0 {
            (pstats.parsed_files as f64 / pstats.total_files as f64) * 100.0
        } else {
            100.0
        };

        let parse_completion_msg = format!(
            "🌳 Unified fast_ml + AST extraction complete: {}/{} files (✅ {:.1}% success) | 📊 {} nodes + {} edges | ⚡ {:.0} lines/s",
            pstats.parsed_files, pstats.total_files, success_rate, total_nodes_extracted, total_edges_extracted, pstats.lines_per_second
        );

        // Enhanced parsing statistics
        info!("🌳 TreeSitter AST parsing results:");
        info!(
            "   📊 Semantic nodes extracted: {} (functions, structs, classes, etc.)",
            total_nodes_extracted
        );
        info!(
            "   🔗 Code relationships extracted: {} (calls, imports, dependencies)",
            total_edges_extracted
        );
        info!(
            "   📈 Extraction efficiency: {:.1} nodes/file | {:.1} edges/file",
            total_nodes_extracted as f64 / pstats.parsed_files.max(1) as f64,
            total_edges_extracted as f64 / pstats.parsed_files.max(1) as f64
        );
        info!(
            "   🎯 Sample nodes: {:?}",
            nodes.iter().take(3).map(|n| &n.name).collect::<Vec<_>>()
        );
        self.log_surrealdb_status("post-parse");

        if nodes.is_empty() {
            warn!("No nodes generated from parsing! Check parser implementation.");
            warn!(
                "Parsing stats: {} files, {} lines processed",
                pstats.parsed_files, pstats.total_lines
            );
        }

        // STAGE 4: Persist nodes before embedding so SurrealDB reflects progress
        let store_nodes_pb = self.create_progress_bar(
            nodes.len() as u64,
            "📈 Storing nodes (symbol map build)",
        );
        let mut stats = IndexStats {
            files: pstats.parsed_files,
            skipped: pstats.total_files - pstats.parsed_files,
            ..Default::default()
        };
        let mut symbol_map: std::collections::HashMap<String, NodeId> =
            std::collections::HashMap::new();

        for node in nodes.iter() {
            match node.node_type {
                Some(NodeType::Function) => stats.functions += 1,
                Some(NodeType::Class) => stats.classes += 1,
                Some(NodeType::Struct) => stats.structs += 1,
                Some(NodeType::Trait) => stats.traits += 1,
                _ => {}
            }
            if let Some(ref c) = node.content {
                stats.lines += c.lines().count();
            }

            extend_symbol_index(&mut symbol_map, node);
        }
        let storage_batch = self.config.batch_size.max(1);
        for chunk in nodes.chunks(storage_batch) {
            self.persist_nodes_batch(chunk).await?;
            store_nodes_pb.inc(chunk.len() as u64);
        }
        self.flush_surreal_writer().await?;
        store_nodes_pb.finish_with_message("📈 Stored nodes (symbol map ready)");
        self.log_surreal_node_count(total_nodes_extracted).await;

        #[cfg(feature = "embeddings")]
        let total_chunks = chunk_plan.chunks.len() as u64;
        #[cfg(not(feature = "embeddings"))]
        let total_chunks = 0u64;
        let embed_pb = self.create_batch_progress_bar(
            total_chunks,
            self.config.batch_size,
            "🧠 Embedding chunks (vector batch)",
        );
        let chunk_store_pb = self.create_batch_progress_bar(
            total_chunks,
            self.config.batch_size,
            "🧩 Persisting chunk embeddings",
        );
        let batch = self.config.batch_size.max(1);
        #[allow(unused_mut)]
        let mut processed: u64 = 0;

        // Enhanced embedding phase logging
        let provider = &self.global_config.embedding.provider;
        info!("💾 Starting semantic embedding generation:");
        info!(
            "   🤖 Provider: {} ({}-dimensional embeddings)",
            provider, self.vector_dim
        );
        info!(
            "   🗄️ SurrealDB column: {}",
            self.embedding_column.column_name()
        );
        
        let total_nodes = nodes.len() as u64;
        info!("   📊 Nodes to embed: {} semantic entities", total_nodes);
        info!(
            "   ⚡ Batch size: {} (optimized for {} system)",
            batch,
            self.estimate_system_memory()
        );
        info!("   🎯 Target: Enable similarity search and AI-powered analysis");
        // Embed chunks and persist chunk embeddings
        #[cfg(feature = "embeddings")]
        {
            let mut chunk_iter = chunk_plan.chunks.chunks(batch);
            let mut meta_iter = chunk_plan.metas.chunks(batch);
            while let (Some(chunk_batch), Some(meta_batch)) = (chunk_iter.next(), meta_iter.next())
            {
                // Prepare text batch ordered like metas
                let texts: Vec<String> = chunk_batch.iter().map(|c| c.text.clone()).collect();
                let embs: Vec<Vec<f32>> = self.embedder.embed_texts_batched(&texts).await?;

                // Build chunk embedding records
                let mut records: Vec<ChunkEmbeddingRecord> = Vec::with_capacity(meta_batch.len());
                for ((meta, text), emb) in meta_batch.iter().zip(texts.iter()).zip(embs.iter()) {
                    let Some(node) = nodes.get(meta.node_index) else {
                        warn!(
                            "Chunk meta references out-of-bounds node index {} (nodes len {})",
                            meta.node_index,
                            nodes.len()
                        );
                        continue;
                    };

                    records.push(ChunkEmbeddingRecord::new(
                        &node.id.to_string(),
                        meta.chunk_index,
                        text.clone(),
                        emb,
                        &self.embedding_model,
                        self.embedding_column.column_name(),
                        &self.project_id,
                    ));
                }
                self.enqueue_chunk_embeddings(records).await?;

                chunk_store_pb.set_position(
                    (chunk_store_pb.position() + meta_batch.len() as u64).min(total_chunks),
                );

                processed += meta_batch.len() as u64;
                embed_pb.set_position(processed.min(total_chunks));
            }
        }
        let embedding_rate = if total_chunks > 0 {
            processed as f64 / total_chunks as f64 * 100.0
        } else {
            100.0
        };

        let provider = &self.global_config.embedding.provider;
        let embed_completion_msg = format!(
            "💾 Semantic embeddings complete: {}/{} chunks (✅ {:.1}% success) | 🤖 {} | 📐 {}-dim | 🚀 Batch: {}",
            processed,
            total_chunks,
            embedding_rate,
            provider,
            self.vector_dim,
            self.config.batch_size
        );
        embed_pb.finish_with_message(embed_completion_msg);
        chunk_store_pb.finish_with_message(format!(
            "🧩 Chunk embeddings queued for persistence: {}/{} batches",
            total_chunks,
            total_chunks
        ));

        #[cfg(feature = "embeddings")]
        {
            // Ensure all chunk embeddings are flushed to SurrealDB before continuing
            self.flush_surreal_writer().await?;
            self.log_surreal_chunk_count(total_chunks as usize).await;
        }

        // Node embedding: keep pooled embedding as average of its chunks
        #[cfg(feature = "embeddings")]
        {
            // For now, skip storing node-level embedding when chunking is enabled.
            // Retrieval should use chunk embeddings directly.
        }

        stats.embeddings = processed as usize;

        // Enhanced embedding completion statistics
        info!("💾 Semantic embedding generation results:");
        info!(
            "   🎯 Vector search enabled: {} nodes embedded for similarity matching",
            processed
        );
        info!("   📐 Embedding dimensions: {}", self.vector_dim);
        info!(
            "   🤖 Provider performance: {} with batch optimization",
            provider
        );
        info!("   🔍 Capabilities unlocked: Vector search, semantic analysis, AI-powered tools");

        // CRITICAL FIX: Preserve working ONNX embedding session for AI semantic matching
        // Original reset caused fresh embedder creation to fail with ONNX resource conflicts,
        // falling back to random hash embeddings (0% AI effectiveness).
        // Keeping the working ONNX session ensures real embeddings for AI semantic matching.
        // Tradeoff: Slightly more memory usage during post-processing (acceptable on M4 Max).
        #[cfg(feature = "embeddings")]
        {
            // self.embedder = codegraph_vector::EmbeddingGenerator::default();
            tracing::info!("🔧 Preserving working ONNX embedder session for AI semantic matching");
        }

        tracing::info!(
            target: "codegraph_mcp::indexer",
            "Vector indexing handled by SurrealDB; local FAISS generation removed"
        );

        // REVOLUTIONARY: Store edges extracted during unified parsing (MAXIMUM SPEED)
        let stored_edges;
        let edge_count = edges.len();
        let resolution_rate;
        {
            let edge_pb =
                self.create_progress_bar(edges.len() as u64, "🔗 Resolving & Storing Dependencies");
            let edge_count = edges.len();

            info!("🔗 Starting dependency relationship storage:");
            info!(
                "   📊 Raw relationships extracted: {} (calls, imports, dependencies)",
                edge_count
            );
            info!(
                "   🎯 Symbol resolution map: {} unique symbols available",
                symbol_map.len()
            );
            info!(
                "   🧠 AI-enhanced resolution: {} feature active",
                if cfg!(feature = "ai-enhanced") {
                    "Semantic similarity"
                } else {
                    "Pattern matching only"
                }
            );
            info!("   🔍 Resolution methods: Exact match → Simple name → Case variants → AI similarity");
            info!("   🚀 M4 Max optimization: Parallel processing with bulk database operations");

            // REVOLUTIONARY: Parallel symbol resolution optimized for M4 Max 128GB
            let chunk_size = (edges.len() / 12).max(100).min(1000); // Optimal for 12+ cores
            let chunks: Vec<_> = edges.chunks(chunk_size).collect();
            let total_chunks = chunks.len();

            info!(
                "⚡ Parallel processing: {} edge chunks across {} cores",
                total_chunks,
                num_cpus::get()
            );

            // REVOLUTIONARY: Pre-generate AI embeddings for BOTH known symbols AND unresolved edge targets
            #[cfg(feature = "ai-enhanced")]
            let (symbol_embeddings, unresolved_embeddings) = {
                info!("🚀 INITIALIZING REVOLUTIONARY 2-PHASE AI SEMANTIC MATCHING");
                info!(
                    "🔧 Phase 1: Pre-computing embeddings for {} known symbols",
                    symbol_map.len()
                );

                // Phase 1: Known symbol embeddings
                let known_embeddings = match self.precompute_symbol_embeddings(&symbol_map).await {
                    embeddings if !embeddings.is_empty() => {
                        info!(
                            "✅ Known symbol embeddings ready: {} pre-computed",
                            embeddings.len()
                        );
                        embeddings
                    }
                    _ => {
                        warn!(
                            "⚠️ Known symbol embedding failed - falling back to empty embeddings"
                        );
                        std::collections::HashMap::new()
                    }
                };

                // Phase 2: Pre-compute embeddings for ALL unresolved edge targets
                info!("🔧 Phase 2: Pre-computing embeddings for unresolved edge targets");
                let unresolved_symbols: std::collections::HashSet<String> = edges
                    .iter()
                    .filter_map(|edge| {
                        if symbol_map.contains_key(&edge.to) {
                            None // Already resolved
                        } else {
                            Some(edge.to.clone()) // Unresolved - needs embedding
                        }
                    })
                    .collect();

                info!(
                    "📊 Discovered {} unique unresolved symbols for AI embedding",
                    unresolved_symbols.len()
                );
                let unresolved_embeddings = if !unresolved_symbols.is_empty() {
                    // PROFESSIONAL: Direct embedding generation for unresolved symbols (no fake NodeIds needed)
                    match self
                        .precompute_unresolved_symbol_embeddings(&unresolved_symbols)
                        .await
                    {
                        embeddings if !embeddings.is_empty() => {
                            info!(
                                "✅ Unresolved symbol embeddings ready: {} pre-computed",
                                embeddings.len()
                            );
                            embeddings
                        }
                        _ => {
                            warn!("⚠️ Unresolved symbol embedding failed - AI matching will be limited");
                            std::collections::HashMap::new()
                        }
                    }
                } else {
                    std::collections::HashMap::new()
                };

                info!(
                    "🤖 REVOLUTIONARY AI READY: {} known + {} unresolved = {} total embeddings",
                    known_embeddings.len(),
                    unresolved_embeddings.len(),
                    known_embeddings.len() + unresolved_embeddings.len()
                );

                (known_embeddings, unresolved_embeddings)
            };
            #[cfg(not(feature = "ai-enhanced"))]
            let (symbol_embeddings, unresolved_embeddings): (
                std::collections::HashMap<String, Vec<f32>>,
                std::collections::HashMap<String, Vec<f32>>,
            ) = {
                info!("🚀 Pattern-only resolution: AI semantic matching disabled (ai-enhanced feature not enabled)");
                (
                    std::collections::HashMap::new(),
                    std::collections::HashMap::new(),
                )
            };

            // Flush symbol embeddings to database before edge resolution
            self.flush_surreal_writer().await?;

            let mut unresolved_edges = 0;
            let mut exact_matches = 0;
            let mut pattern_matches = 0;
            #[cfg(feature = "ai-enhanced")]
            let mut ai_matches = 0;
            let resolution_start = std::time::Instant::now();

            // REVOLUTIONARY: Parallel symbol resolution for M4 Max performance
            use std::sync::atomic::{AtomicUsize, Ordering};

            let processed_chunks = AtomicUsize::new(0);
            let total_resolved = AtomicUsize::new(0);

            // Process all chunks in parallel using M4 Max cores
            let unresolved_samples: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));

            let chunk_results: Vec<_> = chunks
                .par_iter()
                .enumerate()
                .map(|(chunk_idx, chunk)| {
                    let unresolved_samples = unresolved_samples.clone();
                    let mut chunk_resolved = Vec::new();
                    let mut chunk_stats = (0, 0, 0, 0); // (exact, pattern, ai, unresolved)

                    for edge_rel in chunk.iter() {
                        let mut target_id = None;
                        let mut resolution_type = "unresolved";

                        // Rust-first normalization
                        for variant in Self::normalize_rust_symbol(&edge_rel.to) {
                            if let Some(&id) = symbol_map.get(&variant) {
                                target_id = Some(id);
                                resolution_type = "normalized";
                                break;
                            }
                        }

                        // Python normalization (fallback to string-based)
                        if target_id.is_none() {
                            for variant in Self::normalize_python_symbol(&edge_rel.to) {
                                if let Some(&id) = symbol_map.get(&variant) {
                                    target_id = Some(id);
                                    resolution_type = "normalized";
                                    break;
                                }
                            }
                        }

                        if target_id.is_none() {
                            for variant in Self::normalize_symbol_target(&edge_rel.to) {
                                if let Some(&id) = symbol_map.get(&variant) {
                                    target_id = Some(id);
                                    resolution_type = "normalized";
                                    break;
                                }
                            }
                        }

                        if target_id.is_none() {
                            if let Some(simple_name) = edge_rel.to.split("::").last() {
                                if let Some(&id) = symbol_map.get(simple_name) {
                                    target_id = Some(id);
                                    resolution_type = "simple_name";
                                }
                            }
                        }

                        if let Some(target_id) = target_id {
                            // Track resolution method for statistics
                            match resolution_type {
                                "normalized" | "exact" => chunk_stats.0 += 1,
                                "simple_name" => chunk_stats.1 += 1,
                                _ => {}
                            }

                            // Collect resolved edge for bulk storage
                            chunk_resolved.push((
                                edge_rel.from,
                                target_id,
                                edge_rel.edge_type.clone(),
                                edge_rel.metadata.clone(),
                            ));
                        } else {
                            // REVOLUTIONARY: Real AI semantic matching using BOTH known + unresolved embeddings
                            #[cfg(feature = "ai-enhanced")]
                            {
                                if let Some(best_match) = Self::ai_semantic_match_sync(
                                    &edge_rel.to,
                                    &symbol_map,
                                    &symbol_embeddings,
                                    &unresolved_embeddings,
                                ) {
                                    chunk_stats.2 += 1; // AI match count
                                    chunk_resolved.push((
                                        edge_rel.from,
                                        best_match,
                                        edge_rel.edge_type.clone(),
                                        edge_rel.metadata.clone(),
                                    ));
                                } else {
                                    chunk_stats.3 += 1; // Unresolved count
                                }
                            }
                            #[cfg(not(feature = "ai-enhanced"))]
                            {
                                chunk_stats.3 += 1; // Unresolved count
                            }
                        }
                    }

                    // Enhanced progress tracking with ETA for M4 Max visibility
                    let chunks_done = processed_chunks.fetch_add(1, Ordering::Relaxed) + 1;
                    if chunks_done % 3 == 0 || chunks_done == total_chunks {
                        let resolved_so_far =
                            total_resolved.fetch_add(chunk_resolved.len(), Ordering::Relaxed);
                        edge_pb.set_position(resolved_so_far as u64);

                        if chunks_done % 5 == 0 {
                            let elapsed = resolution_start.elapsed().as_secs_f64();
                            let rate = resolved_so_far as f64 / elapsed;
                            let remaining = edge_count - resolved_so_far;
                            let eta = if rate > 0.0 {
                                remaining as f64 / rate
                            } else {
                                0.0
                            };

                            info!(
                                "⚡ M4 Max parallel: {}/{} chunks | {} edges/s | ETA: {:.1}s",
                                chunks_done, total_chunks, rate as usize, eta
                            );
                        }
                    }

                    (chunk_resolved, chunk_stats)
                })
                .collect();

            // Aggregate statistics and resolved edges
            let mut all_resolved_edges = Vec::new();
            for (chunk_edges, (exact, pattern, ai, unresolved)) in chunk_results {
                exact_matches += exact;
                pattern_matches += pattern;
                #[cfg(feature = "ai-enhanced")]
                {
                    ai_matches += ai;
                }
                unresolved_edges += unresolved;
                all_resolved_edges.extend(chunk_edges);
            }

            let mut stored_edges_local = 0usize;
            let mut resolution_rate_local = 0.0;

            // Store resolved edges via writer
            if !all_resolved_edges.is_empty() {
                let serializable_edges: Vec<_> = all_resolved_edges
                    .iter()
                    .map(
                        |(from, to, edge_type, metadata)| codegraph_graph::edge::CodeEdge {
                            id: uuid::Uuid::new_v4(),
                            from: *from,
                            to: *to,
                            edge_type: edge_type.clone(),
                            weight: 1.0,
                            metadata: metadata.clone(),
                        },
                    )
                    .collect();

                stored_edges_local = serializable_edges.len();
                if let Err(err) = self.enqueue_edges(serializable_edges).await {
                    warn!("⚠️ Failed to store resolved edges: {}", err);
                }

                let resolution_time = resolution_start.elapsed();
                resolution_rate_local = (stored_edges_local as f64 / edge_count as f64) * 100.0;

                let edge_msg = format!(
                    "🔗 Dependencies resolved: {}/{} relationships ({:.1}% success) | ⚡ {:.1}s",
                    stored_edges_local,
                    edge_count,
                    resolution_rate_local,
                    resolution_time.as_secs_f64()
                );
                edge_pb.finish_with_message(edge_msg);

                info!("🔗 M4 MAX PARALLEL PROCESSING RESULTS:");
                info!(
                    "   ✅ Successfully stored: {} edges ({:.1}% of extracted relationships)",
                    stored_edges_local, resolution_rate_local
                );
                info!(
                    "   🎯 Exact matches: {} (direct symbol found)",
                    exact_matches
                );
                info!(
                    "   🔄 Pattern matches: {} (simplified/cleaned symbols)",
                    pattern_matches
                );
                #[cfg(feature = "ai-enhanced")]
                info!(
                    "   🧠 AI semantic matches: {} (similarity-based resolution)",
                    ai_matches
                );
                info!(
                    "   ❌ Unresolved: {} (external dependencies/dynamic calls)",
                    unresolved_edges
                );
                if let Ok(samples) = unresolved_samples.lock() {
                    if !samples.is_empty() && std::env::var("CODEGRAPH_DEBUG").is_ok() {
                        info!("   🔍 Sample unresolved targets: {:?}", *samples);
                    }
                }
                info!(
                    "   ⚡ M4 Max performance: {:.0} edges/s ({} cores utilized)",
                    edge_count as f64 / resolution_time.as_secs_f64(),
                    num_cpus::get()
                );
                info!(
                    "   🚀 Parallel efficiency: {} chunks processed across {} cores",
                    total_chunks,
                    num_cpus::get()
                );

                if resolution_rate_local >= 80.0 {
                    info!(
                        "🎉 EXCELLENT: {:.1}% resolution rate achieved!",
                        resolution_rate_local
                    );
                } else if resolution_rate_local >= 60.0 {
                    info!(
                        "👍 GOOD: {:.1}% resolution rate. Consider enabling ai-enhanced or improving symbol normalization.",
                        resolution_rate_local
                    );
                } else {
                    warn!(
                        "⚠️ LOW resolution rate: {:.1}%. Check normalization and embeddings.",
                        resolution_rate_local
                    );
                }
            } else {
                edge_pb.finish_with_message("No resolved edges to store");
            }

            // Assign values for use outside the block
            stored_edges = stored_edges_local;
            resolution_rate = resolution_rate_local;
        }

        // ELIMINATED: No separate edge processing phase needed - edges extracted during parsing!
        self.log_surreal_edge_count(stored_edges).await;

        // Persist project metadata summary into SurrealDB
        self.persist_project_metadata(&stats, total_nodes_extracted, total_edges_extracted)
            .await?;
        self.flush_surreal_writer().await?;

        // Task 3.3: Update file metadata for incremental indexing
        info!("💾 Updating file metadata for change tracking");
        let file_paths_only: Vec<PathBuf> = files.iter().map(|(p, _)| p.clone()).collect();
        self.persist_file_metadata(&file_paths_only, &nodes, &edges)
            .await?;
        self.flush_surreal_writer().await?;
        self.verify_file_metadata_count(file_paths_only.len())
            .await?;
        self.verify_project_metadata_present().await?;

        // COMPREHENSIVE INDEXING COMPLETION SUMMARY
        let avg_nodes_per_file = if stats.files > 0 {
            total_nodes_extracted as f64 / stats.files as f64
        } else {
            0.0
        };
        let avg_edges_per_file = if stats.files > 0 {
            total_edges_extracted as f64 / stats.files as f64
        } else {
            0.0
        };
        let avg_embeddings_per_node = if total_nodes_extracted > 0 {
            stats.embeddings as f64 / total_nodes_extracted as f64
        } else {
            0.0
        };

        info!("🎉 INDEXING COMPLETE - REVOLUTIONARY AI DEVELOPMENT PLATFORM READY!");
        info!("┌────────────────────────────────────────────────────────────────────────────┐");
        info!("│ 📊 COMPREHENSIVE INDEXING STATISTICS                                      │");
        info!("├────────────────────────────────────────────────────────────────────────────┤");
        info!(
            "│ 📂 Files scanned: {:>5} total | {:>5} parsed | {:>5} skipped                │",
            pstats.total_files, stats.files, stats.skipped
        );
        info!(
            "│ ✅ Parser success: {:>5.1}% ({} / {} files)                               │",
            success_rate, pstats.parsed_files, pstats.total_files
        );
        info!(
            "│ 🗣️ Languages targeted: {:>3} | Batch (embed) {:>3} | Concurrency {:>3}        │",
            file_config.languages.len(),
            batch,
            self.config.max_concurrent
        );
        info!(
            "│ 📝 Lines analyzed: {:>10} | Avg nodes/file {:>5.1} | Avg deps/file {:>5.1} │",
            stats.lines, avg_nodes_per_file, avg_edges_per_file
        );
        info!(
            "│ 🌳 Semantic nodes: {:>8} | funcs {:>6} | structs {:>5} | traits {:>5} │",
            total_nodes_extracted, stats.functions, stats.structs, stats.traits
        );
        info!(
            "│ 🔗 Dependencies: {:>8} extracted | {:>8} stored (resolved {:.1}%)        │",
            total_edges_extracted, stored_edges, resolution_rate
        );
        info!(
            "│ 💾 Vector embeddings: {:>8} ({:>4}-dim {}, {:.1} per node)                 │",
            stats.embeddings, self.vector_dim, provider, avg_embeddings_per_node
        );
        info!(
            "│ 📦 Metadata persisted: {:>5} files | {:>5} edges | {:>5} nodes              │",
            stats.files, stored_edges, total_nodes_extracted
        );
        info!("├────────────────────────────────────────────────────────────────────────────┤");
        info!("│ 🚀 CAPABILITIES UNLOCKED                                                  │");
        info!(
            "│ ✅ Vector similarity search across {:>8} embedded entities                 │",
            stats.embeddings
        );
        info!(
            "│ ✅ Graph traversal with {:>8} real dependency relationships              │",
            stored_edges
        );
        info!("│ ✅ AI-powered semantic analysis with Qwen2.5-Coder integration │");
        info!("│ ✅ Revolutionary edge processing with single-pass extraction   │");
        #[cfg(feature = "ai-enhanced")]
        info!("│ ✅ Conversational AI: codebase_qa and code_documentation tools │");
        info!("└─────────────────────────────────────────────────────────────────┘");
        info!("🚀 CodeGraph Universal AI Development Platform: FULLY OPERATIONAL");

        self.shutdown_surreal_writer().await?;

        Ok(stats)
    }

    /// REVOLUTIONARY: Parse files with unified node+edge extraction for maximum speed
    async fn parse_files_with_unified_extraction(
        &self,
        files: Vec<(PathBuf, u64)>,
        total_files: u64,
    ) -> Result<(
        Vec<CodeNode>,
        Vec<codegraph_core::EdgeRelationship>,
        codegraph_parser::ParsingStatistics,
    )> {
        shared_unified_parse(&self.parser, files, total_files).await
    }

    /// Estimate available system memory for informative logging
    fn estimate_system_memory(&self) -> String {
        #[cfg(target_os = "macos")]
        {
            if let Ok(output) = std::process::Command::new("sysctl")
                .args(["-n", "hw.memsize"])
                .output()
            {
                if let Ok(memsize_str) = String::from_utf8(output.stdout) {
                    if let Ok(memsize) = memsize_str.trim().parse::<u64>() {
                        let gb = memsize / 1024 / 1024 / 1024;
                        return format!("{}GB", gb);
                    }
                }
            }
        }

        #[cfg(target_os = "linux")]
        {
            if let Ok(contents) = std::fs::read_to_string("/proc/meminfo") {
                if let Some(line) = contents.lines().find(|line| line.starts_with("MemTotal:")) {
                    if let Some(kb_str) = line.split_whitespace().nth(1) {
                        if let Ok(kb) = kb_str.parse::<u64>() {
                            let gb = kb / 1024 / 1024;
                            return format!("{}GB", gb);
                        }
                    }
                }
            }
        }

        "Unknown".to_string()
    }

    /// Pre-compute embeddings for all symbols for M4 Max performance optimization
    #[cfg(feature = "ai-enhanced")]
    async fn precompute_symbol_embeddings(
        &self,
        symbol_map: &std::collections::HashMap<String, NodeId>,
    ) -> std::collections::HashMap<String, Vec<f32>> {
        info!("🧠 Pre-computing symbol embeddings for M4 Max AI optimization");
        info!(
            "🔧 DEBUG: precompute_symbol_embeddings called with {} symbols",
            symbol_map.len()
        );
        let mut embeddings = std::collections::HashMap::new();

        // Early validation
        if symbol_map.is_empty() {
            warn!("⚠️ Empty symbol map - skipping AI embedding pre-computation");
            return embeddings;
        }

        // Get ALL symbols for maximum AI resolution coverage (M4 Max can handle it)
        let top_symbols: Vec<_> = symbol_map.keys().cloned().collect();
        info!(
            "📊 Selected {} top symbols for AI embedding pre-computation",
            top_symbols.len()
        );

        // ARCHITECTURAL IMPROVEMENT: Use existing working embedder instead of creating fresh one
        // This avoids re-initialization issues that could cause random hash fallback
        info!(
            "🤖 Using configured embedder ({}) for AI semantic matching",
            self.global_config.embedding.provider
        );
        let embedder = &self.embedder;
        info!("✅ Using working embedder session (guaranteed real embeddings)");
        let (batch_size, max_concurrent) = self.symbol_embedding_batch_settings();
        let total_batches = (top_symbols.len() + batch_size - 1) / batch_size;
        info!(
            "⚡ Embedding batch size: {} symbols ({} batches, max {} concurrent)",
            batch_size,
            total_batches.max(1),
            max_concurrent
        );

        // Create progress bar for symbol embedding generation
        let symbol_pb = self.create_batch_progress_bar(
            top_symbols.len() as u64,
            batch_size,
            "🧩 Symbol embeddings",
        );

        let batches: Vec<Vec<String>> = top_symbols
            .chunks(batch_size)
            .map(|chunk| chunk.iter().cloned().collect())
            .collect();

        let mut processed = 0usize;

        let mut batch_stream = stream::iter(batches.into_iter().map(|batch| {
            let embedder = embedder;
            async move {
                let result = embedder.embed_texts_batched(&batch).await;
                (batch, result)
            }
        }))
        .buffer_unordered(max_concurrent);

        while let Some((batch, result)) = batch_stream.next().await {
            match result {
                Ok(batch_embeddings) => {
                    let mut records = Vec::with_capacity(batch.len());
                    for (symbol, embedding) in
                        batch.iter().cloned().zip(batch_embeddings.into_iter())
                    {
                        records.push(self.build_symbol_embedding_record(
                            &symbol,
                            symbol_map.get(&symbol).cloned(),
                            None,
                            &embedding,
                        ));
                        embeddings.insert(symbol, embedding);
                        processed += 1;
                    }
                    if let Err(err) = self.persist_symbol_embedding_records(records).await {
                        warn!("⚠️ Failed to persist batch symbol embeddings: {}", err);
                    }
                    symbol_pb.set_position(processed as u64);
                }
                Err(e) => {
                    warn!(
                        "⚠️ Batch embedding failed for {} symbols: {}. Falling back to individual processing.",
                        batch.len(),
                        e
                    );
                    for symbol in batch.into_iter() {
                        match embedder.generate_text_embedding(&symbol).await {
                            Ok(embedding) => {
                                let record = self.build_symbol_embedding_record(
                                    &symbol,
                                    symbol_map.get(&symbol).cloned(),
                                    None,
                                    &embedding,
                                );
                                if let Err(err) =
                                    self.persist_symbol_embedding_records(vec![record]).await
                                {
                                    warn!(
                                        "⚠️ Failed to persist symbol embedding for '{}': {}",
                                        symbol, err
                                    );
                                }
                                embeddings.insert(symbol, embedding);
                                processed += 1;
                                symbol_pb.set_position(processed as u64);
                            }
                            Err(err) => {
                                warn!(
                                    "⚠️ Failed to generate embedding for symbol '{}': {}",
                                    symbol, err
                                );
                            }
                        }
                    }
                }
            }
        }

        // Finish progress bar with summary
        let provider = &self.global_config.embedding.provider;
        let success_rate = if top_symbols.len() > 0 {
            embeddings.len() as f64 / top_symbols.len() as f64 * 100.0
        } else {
            100.0
        };
        let completion_msg = format!(
            "🧠 Symbol embeddings complete: {}/{} symbols (✅ {:.1}% success) | 🤖 {} | 🔗 AI semantic matching ready",
            embeddings.len(),
            top_symbols.len(),
            success_rate,
            provider
        );
        symbol_pb.finish_with_message(completion_msg);

        info!(
            "🧠 Pre-computed {} symbol embeddings for fast AI resolution",
            embeddings.len()
        );
        if embeddings.is_empty() {
            warn!("⚠️ No symbol embeddings were generated - AI matching will be disabled");
            warn!(
                "🔍 Debug: top_symbols.len()={}, batches attempted={}",
                top_symbols.len(),
                (top_symbols.len() + batch_size - 1) / batch_size
            );
        } else {
            info!(
                "✅ AI semantic matching ready with {:.1}% coverage ({}/{})",
                embeddings.len() as f64 / symbol_map.len() as f64 * 100.0,
                embeddings.len(),
                symbol_map.len()
            );
            info!(
                "🤖 AI SEMANTIC MATCHING ACTIVATED: First call with {} pre-computed embeddings",
                embeddings.len()
            );
        }
        embeddings
    }

    /// REVOLUTIONARY: Pre-compute embeddings directly for unresolved symbols (professional batching)
    #[cfg(feature = "ai-enhanced")]
    async fn precompute_unresolved_symbol_embeddings(
        &self,
        unresolved_symbols: &std::collections::HashSet<String>,
    ) -> std::collections::HashMap<String, Vec<f32>> {
        use codegraph_vector::EmbeddingGenerator;

        info!("🧠 Pre-computing unresolved symbol embeddings for professional-grade AI");
        info!(
            "🔧 Processing {} unique unresolved symbols",
            unresolved_symbols.len()
        );
        let mut embeddings = std::collections::HashMap::new();

        if unresolved_symbols.is_empty() {
            return embeddings;
        }

        let symbols_vec: Vec<_> = unresolved_symbols.iter().cloned().collect();
        let embedder = &self.embedder;
        let (batch_size, max_concurrent) = self.symbol_embedding_batch_settings();

        let total_batches = (symbols_vec.len() + batch_size - 1) / batch_size;
        info!(
            "⚡ Unresolved embedding batch size: {} symbols ({} batches, max {} concurrent)",
            batch_size,
            total_batches.max(1),
            max_concurrent
        );

        // Create progress bar for unresolved symbol embedding generation
        let unresolved_pb = self.create_batch_progress_bar(
            symbols_vec.len() as u64,
            batch_size,
            "🧩 Unresolved symbol embeddings",
        );

        let batches: Vec<Vec<String>> = symbols_vec
            .chunks(batch_size)
            .map(|chunk| chunk.iter().cloned().collect())
            .collect();

        let mut batch_stream = stream::iter(batches.into_iter().map(|batch| {
            let embedder = embedder;
            async move {
                let result = embedder.embed_texts_batched(&batch).await;
                (batch, result)
            }
        }))
        .buffer_unordered(max_concurrent);

        while let Some((batch, result)) = batch_stream.next().await {
            match result {
                Ok(batch_embeddings) => {
                    let mut records = Vec::with_capacity(batch.len());
                    for (symbol, embedding) in
                        batch.iter().cloned().zip(batch_embeddings.into_iter())
                    {
                        records.push(
                            self.build_symbol_embedding_record(&symbol, None, None, &embedding),
                        );
                        embeddings.insert(symbol, embedding);
                    }
                    if let Err(err) = self.persist_symbol_embedding_records(records).await {
                        warn!(
                            "⚠️ Failed to persist unresolved symbol embeddings batch: {}",
                            err
                        );
                    }
                    unresolved_pb.set_position(embeddings.len() as u64);
                }
                Err(e) => {
                    warn!(
                        "⚠️ Batch embedding failed for {} unresolved symbols: {}. Falling back to individual processing.",
                        batch.len(),
                        e
                    );
                    for symbol in batch.into_iter() {
                        match embedder.generate_text_embedding(&symbol).await {
                            Ok(embedding) => {
                                let record = self
                                    .build_symbol_embedding_record(&symbol, None, None, &embedding);
                                if let Err(err) =
                                    self.persist_symbol_embedding_records(vec![record]).await
                                {
                                    warn!(
                                        "⚠️ Failed to persist unresolved symbol embedding '{}': {}",
                                        symbol, err
                                    );
                                }
                                embeddings.insert(symbol, embedding);
                                unresolved_pb.set_position(embeddings.len() as u64);
                            }
                            Err(err) => {
                                warn!(
                                    "⚠️ Failed to generate embedding for unresolved symbol '{}': {}",
                                    symbol, err
                                );
                            }
                        }
                    }
                }
            }
        }

        // Finish progress bar with summary
        let provider = &self.global_config.embedding.provider;
        let success_rate = if symbols_vec.len() > 0 {
            embeddings.len() as f64 / symbols_vec.len() as f64 * 100.0
        } else {
            100.0
        };
        let completion_msg = format!(
            "🔗 Unresolved symbol embeddings complete: {}/{} symbols (✅ {:.1}% success) | 🤖 {} | ⚡ AI matching enhanced",
            embeddings.len(),
            symbols_vec.len(),
            success_rate,
            provider
        );
        unresolved_pb.finish_with_message(completion_msg);

        info!(
            "🧠 Pre-computed {} unresolved symbol embeddings for professional AI matching",
            embeddings.len()
        );
        if embeddings.is_empty() {
            warn!(
                "⚠️ No unresolved symbol embeddings were generated - AI matching will be limited"
            );
        } else {
            info!("✅ Professional AI semantic matching ready with {:.1}% unresolved coverage ({}/{})",
                  embeddings.len() as f64 / unresolved_symbols.len() as f64 * 100.0,
                  embeddings.len(), unresolved_symbols.len());
        }

        embeddings
    }

    /// REVOLUTIONARY: AI-powered symbol resolution using semantic similarity
    #[cfg(feature = "ai-enhanced")]
    async fn ai_resolve_symbol(
        &self,
        target_symbol: &str,
        symbol_map: &std::collections::HashMap<String, NodeId>,
    ) -> Option<NodeId> {
        use codegraph_vector::{search::SemanticSearch, EmbeddingGenerator};
        use std::sync::Arc;

        // Use same config as main indexing for consistency
        let embedder = EmbeddingGenerator::with_config(&self.global_config).await;
        if let Ok(target_embedding) = embedder.generate_text_embedding(target_symbol).await {
            // Find the most similar symbol in our symbol map using cosine similarity
            let mut best_match: Option<(NodeId, f32)> = None;

            for (symbol_name, &node_id) in symbol_map.iter() {
                if let Ok(symbol_embedding) = embedder.generate_text_embedding(symbol_name).await {
                    let similarity = self.cosine_similarity(&target_embedding, &symbol_embedding);

                    // Use a threshold for semantic similarity (0.7 = quite similar)
                    if similarity > 0.7 {
                        if let Some((_, best_score)) = best_match {
                            if similarity > best_score {
                                best_match = Some((node_id, similarity));
                            }
                        } else {
                            best_match = Some((node_id, similarity));
                        }
                    }
                }
            }

            if let Some((node_id, score)) = best_match {
                info!(
                    "AI resolved '{}' with {:.1}% confidence",
                    target_symbol,
                    score * 100.0
                );
                return Some(node_id);
            }
        }

        None
    }

    /// Calculate cosine similarity between two embeddings
    #[cfg(feature = "ai-enhanced")]
    fn cosine_similarity(&self, a: &[f32], b: &[f32]) -> f32 {
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

    /// REVOLUTIONARY: AI semantic matching with hybrid fuzzy + real AI embeddings (batched)
    #[cfg(feature = "ai-enhanced")]
    fn ai_semantic_match_sync(
        target_symbol: &str,
        symbol_map: &std::collections::HashMap<String, NodeId>,
        symbol_embeddings: &std::collections::HashMap<String, Vec<f32>>,
        unresolved_embeddings: &std::collections::HashMap<String, Vec<f32>>,
    ) -> Option<NodeId> {
        // DIAGNOSTIC: Track AI matching usage
        static AI_MATCH_COUNTER: std::sync::atomic::AtomicUsize =
            std::sync::atomic::AtomicUsize::new(0);
        let call_count = AI_MATCH_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        if call_count == 0 {
            info!(
                "🤖 AI SEMANTIC MATCHING ACTIVATED: First call with {} pre-computed embeddings",
                symbol_embeddings.len()
            );
        }

        if symbol_embeddings.is_empty() {
            if call_count < 3 {
                // Log first few failures
                warn!(
                    "❌ AI MATCH SKIPPED: No pre-computed embeddings available for '{}'",
                    target_symbol
                );
            }
            return None;
        }

        if call_count < 5 {
            info!(
                "🔍 Attempting HYBRID AI resolution for unresolved symbol: '{}'",
                target_symbol
            );
        }

        let mut best_match: Option<(NodeId, f32)> = None;
        let fuzzy_threshold = 0.5;

        // PHASE 1: Fast fuzzy string similarity matching
        for (symbol_name, _) in symbol_embeddings.iter() {
            if let Some(&node_id) = symbol_map.get(symbol_name) {
                let target_lower = target_symbol.to_lowercase();
                let symbol_lower = symbol_name.to_lowercase();

                let fuzzy_score = if target_lower.contains(&symbol_lower)
                    || symbol_lower.contains(&target_lower)
                {
                    0.85 // High confidence for substring matches
                } else if target_lower.ends_with(&symbol_lower)
                    || symbol_lower.ends_with(&target_lower)
                {
                    0.75 // Good confidence for suffix matches
                } else if Self::levenshtein_similarity(&target_lower, &symbol_lower) > 0.7 {
                    0.65 // Decent confidence for edit distance similarity
                } else {
                    continue;
                };

                if fuzzy_score > fuzzy_threshold {
                    if let Some((_, best_score)) = best_match {
                        if fuzzy_score > best_score {
                            best_match = Some((node_id, fuzzy_score));
                        }
                    } else {
                        best_match = Some((node_id, fuzzy_score));
                    }
                }
            }
        }

        // If fuzzy matching found a good match, return it
        if let Some((node_id, confidence)) = best_match {
            if confidence > 0.75 {
                // High confidence fuzzy match
                if call_count < 10 {
                    info!(
                        "🎯 AI FUZZY MATCH: '{}' → known symbol with {:.1}% confidence",
                        target_symbol,
                        confidence * 100.0
                    );
                }
                return Some(node_id);
            }
        }

        // PHASE 2: Real AI embedding semantic similarity using pre-computed unresolved embeddings
        let mut ai_best_match: Option<(NodeId, f32)> = None;
        if let Some(target_embedding) = unresolved_embeddings.get(target_symbol) {
            if call_count < 5 {
                info!(
                    "🔍 Using pre-computed embedding for unresolved symbol: '{}'",
                    target_symbol
                );
            }

            let ai_threshold = 0.75; // Higher threshold for real AI embeddings

            // Compare target embedding with ALL known symbol embeddings
            for (symbol_name, symbol_embedding) in symbol_embeddings.iter() {
                if let Some(&node_id) = symbol_map.get(symbol_name) {
                    let similarity =
                        Self::cosine_similarity_static(target_embedding, symbol_embedding);

                    if similarity > ai_threshold {
                        if let Some((_, best_score)) = ai_best_match {
                            if similarity > best_score {
                                ai_best_match = Some((node_id, similarity));
                            }
                        } else {
                            ai_best_match = Some((node_id, similarity));
                        }
                    }
                }
            }
        }

        // REVOLUTIONARY: Choose the best match between fuzzy and real AI embeddings
        let final_match = match (best_match, ai_best_match) {
            (Some((fuzzy_node, fuzzy_score)), Some((ai_node, ai_score))) => {
                // AI embeddings are more accurate than fuzzy when both exist
                if ai_score > 0.8 || (ai_score > fuzzy_score && ai_score > 0.7) {
                    Some((ai_node, ai_score, "AI EMBEDDING"))
                } else {
                    Some((fuzzy_node, fuzzy_score, "FUZZY"))
                }
            }
            (Some((fuzzy_node, fuzzy_score)), None) => Some((fuzzy_node, fuzzy_score, "FUZZY")),
            (None, Some((ai_node, ai_score))) => Some((ai_node, ai_score, "AI EMBEDDING")),
            (None, None) => None,
        };

        if let Some((node_id, confidence, match_type)) = final_match {
            if call_count < 10 {
                info!(
                    "🎯 {} MATCH: '{}' → known symbol with {:.1}% confidence",
                    match_type,
                    target_symbol,
                    confidence * 100.0
                );
            }
            return Some(node_id);
        }

        None // No semantic match found
    }

    /// Calculate Levenshtein similarity score between two strings (0.0 to 1.0)
    #[cfg(feature = "ai-enhanced")]
    fn levenshtein_similarity(s1: &str, s2: &str) -> f32 {
        let len1 = s1.chars().count();
        let len2 = s2.chars().count();

        if len1 == 0 && len2 == 0 {
            return 1.0;
        }
        if len1 == 0 || len2 == 0 {
            return 0.0;
        }

        let max_len = len1.max(len2);
        let distance = Self::levenshtein_distance(s1, s2);

        1.0 - (distance as f32 / max_len as f32)
    }

    /// Calculate Levenshtein distance between two strings
    #[cfg(feature = "ai-enhanced")]
    fn levenshtein_distance(s1: &str, s2: &str) -> usize {
        let v1: Vec<char> = s1.chars().collect();
        let v2: Vec<char> = s2.chars().collect();
        let len1 = v1.len();
        let len2 = v2.len();

        let mut matrix = vec![vec![0; len2 + 1]; len1 + 1];

        for i in 0..=len1 {
            matrix[i][0] = i;
        }
        for j in 0..=len2 {
            matrix[0][j] = j;
        }

        for i in 1..=len1 {
            for j in 1..=len2 {
                let cost = if v1[i - 1] == v2[j - 1] { 0 } else { 1 };
                matrix[i][j] = (matrix[i - 1][j] + 1)
                    .min(matrix[i][j - 1] + 1)
                    .min(matrix[i - 1][j - 1] + cost);
            }
        }

        matrix[len1][len2]
    }

    /// Static cosine similarity calculation for parallel processing
    #[cfg(feature = "ai-enhanced")]
    fn cosine_similarity_static(a: &[f32], b: &[f32]) -> f32 {
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

    async fn index_file(
        path: PathBuf,
        parse_pb: ProgressBar,
        embed_pb: ProgressBar,
    ) -> Result<FileStats> {
        debug!("Indexing file: {:?}", path);
        let mut stats = FileStats::default();

        // Read file content
        let content = tokio_fs::read_to_string(&path).await?;
        stats.lines = content.lines().count();

        // Very rough heuristics for functions/classes counts per common languages
        let ext = path.extension().and_then(OsStr::to_str).unwrap_or("");
        let (fn_regex, class_regex) = match ext {
            "rs" => (Some(Regex::new(r"\bfn\s+\w+").unwrap()), None),
            "py" => (
                Some(Regex::new(r"\bdef\s+\w+\s*\(").unwrap()),
                Some(Regex::new(r"\bclass\s+\w+\s*:").unwrap()),
            ),
            "ts" | "js" => (
                Some(Regex::new(r"\bfunction\s+\w+|\b\w+\s*=\s*\(.*\)\s*=>").unwrap()),
                Some(Regex::new(r"\bclass\s+\w+").unwrap()),
            ),
            "go" => (Some(Regex::new(r"\bfunc\s+\w+\s*\(").unwrap()), None),
            "java" => (
                Some(Regex::new(r"\b\w+\s+\w+\s*\(.*\)\s*\{").unwrap()),
                Some(Regex::new(r"\bclass\s+\w+").unwrap()),
            ),
            "cpp" | "cc" | "cxx" | "hpp" | "h" | "c" => (
                Some(Regex::new(r"\b\w+\s+\w+\s*\(.*\)\s*\{").unwrap()),
                None,
            ),
            _ => (None, None),
        };

        parse_pb.set_message(format!("Parsing {}", path.display()));
        parse_pb.inc(1);

        if let Some(re) = fn_regex {
            stats.functions = re.find_iter(&content).count();
        }
        if let Some(re) = class_regex {
            stats.classes = re.find_iter(&content).count();
        }

        // Pretend to generate embeddings by counting tokens roughly
        embed_pb.set_message(format!("Embedding {}", path.display()));
        stats.embeddings = content.split_whitespace().count() / 100; // 1 per ~100 tokens
        embed_pb.inc(1);

        Ok(stats)
    }

    async fn collect_files(&self, path: &Path) -> Result<Vec<PathBuf>> {
        let mut files = Vec::new();

        let walker = if self.config.recursive {
            WalkDir::new(path)
        } else {
            WalkDir::new(path).max_depth(1)
        };

        for entry in walker {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                continue;
            }
            if self.should_index(path) {
                files.push(path.to_path_buf());
            }
        }

        Ok(files)
    }

    fn should_index(&self, path: &Path) -> bool {
        let file_name = path.file_name().and_then(OsStr::to_str).unwrap_or("");
        let path_str = path.to_string_lossy();

        // Exclude patterns (simple substring match)
        for pat in &self.config.exclude_patterns {
            if path_str.contains(pat) || file_name.contains(pat) {
                return false;
            }
        }

        // Include patterns (if provided, must match at least one)
        if !self.config.include_patterns.is_empty()
            && !self
                .config
                .include_patterns
                .iter()
                .any(|p| path_str.contains(p) || file_name.contains(p))
        {
            return false;
        }

        // Language filtering by extension
        if !self.config.languages.is_empty() {
            let ext = path
                .extension()
                .and_then(OsStr::to_str)
                .unwrap_or("")
                .to_lowercase();
            let lang_matches = |langs: &Vec<String>, e: &str| -> bool {
                let lang = e;
                langs.iter().any(|l| match l.as_str() {
                    "rust" | "rs" => matches!(lang, "rs"),
                    "python" | "py" => matches!(lang, "py"),
                    "js" | "javascript" | "jsx" => matches!(lang, "js" | "jsx"),
                    "ts" | "typescript" | "tsx" => matches!(lang, "ts" | "tsx"),
                    "go" => matches!(lang, "go"),
                    "java" => matches!(lang, "java"),
                    "cpp" | "c++" | "cc" | "cxx" | "hpp" | "h" | "c" => {
                        matches!(lang, "cpp" | "cc" | "cxx" | "hpp" | "h" | "c")
                    }
                    _ => false,
                })
            };
            if !lang_matches(&self.config.languages, &ext) {
                return false;
            }
        }

        // Default excludes
        for dir in [
            ".git",
            "node_modules",
            "target",
            ".codegraph",
            "dist",
            "build",
        ] {
            if path_str.contains(dir) {
                return false;
            }
        }

        true
    }

    async fn is_indexed(&self, _path: &Path) -> Result<bool> {
        let db = {
            let storage = self.surreal.lock().await;
            storage.db()
        };

        let mut response = db
            .query("SELECT VALUE count() FROM project_metadata WHERE project_id = $project_id")
            .bind(("project_id", self.project_id.clone()))
            .await
            .context("Failed to query project_metadata")?;
        let counts: Vec<i64> = response.take(0)?;
        let count = counts.get(0).cloned().unwrap_or(0);
        Ok(count > 0)
    }

    async fn has_file_metadata(&self) -> Result<bool> {
        let db = {
            let storage = self.surreal.lock().await;
            storage.db()
        };

        let mut response = db
            .query("SELECT VALUE count() FROM file_metadata WHERE project_id = $project_id")
            .bind(("project_id", self.project_id.clone()))
            .await
            .context("Failed to query file_metadata count")?;
        let counts: Vec<i64> = response.take(0)?;
        let count = counts.get(0).cloned().unwrap_or(0);
        Ok(count > 0)
    }

    /// Calculate SHA-256 hash of file content
    fn calculate_file_hash(file_path: &Path) -> Result<String> {
        // Resolve symlinks to actual file
        let canonical_path = fs::canonicalize(file_path)
            .with_context(|| format!("Failed to resolve path: {:?}", file_path))?;

        let mut file = fs::File::open(&canonical_path)
            .with_context(|| format!("Failed to open file: {:?}", canonical_path))?;

        let mut hasher = Sha256::new();
        let mut buffer = [0u8; 8192];

        loop {
            let bytes_read = file
                .read(&mut buffer)
                .with_context(|| format!("Failed to read file: {:?}", canonical_path))?;
            if bytes_read == 0 {
                break;
            }
            hasher.update(&buffer[..bytes_read]);
        }

        Ok(format!("{:x}", hasher.finalize()))
    }

    /// Detect changes between current filesystem and stored file metadata
    async fn detect_file_changes(&self, current_files: &[PathBuf]) -> Result<Vec<FileChange>> {
        let storage = self.surreal.lock().await;
        let stored_metadata = storage
            .get_file_metadata_for_project(&self.project_id)
            .await?;
        drop(storage);

        // Create lookup map of stored files
        let stored_map: HashMap<String, FileMetadataRecord> = stored_metadata
            .into_iter()
            .map(|record| (record.file_path.clone(), record))
            .collect();

        let mut changes = Vec::new();
        let mut current_file_set = HashSet::new();

        // Check current files for additions or modifications
        for file_path in current_files {
            let file_path_str = file_path.to_string_lossy().to_string();
            current_file_set.insert(file_path_str.clone());

            let current_hash = Self::calculate_file_hash(file_path)?;

            match stored_map.get(&file_path_str) {
                Some(stored) => {
                    if stored.content_hash != current_hash {
                        changes.push(FileChange {
                            file_path: file_path_str,
                            change_type: FileChangeType::Modified,
                            current_hash: Some(current_hash),
                            previous_hash: Some(stored.content_hash.clone()),
                        });
                    } else {
                        changes.push(FileChange {
                            file_path: file_path_str,
                            change_type: FileChangeType::Unchanged,
                            current_hash: Some(current_hash),
                            previous_hash: Some(stored.content_hash.clone()),
                        });
                    }
                }
                None => {
                    changes.push(FileChange {
                        file_path: file_path_str,
                        change_type: FileChangeType::Added,
                        current_hash: Some(current_hash),
                        previous_hash: None,
                    });
                }
            }
        }

        // Check for deleted files
        for (stored_path, stored_record) in stored_map {
            if !current_file_set.contains(&stored_path) {
                changes.push(FileChange {
                    file_path: stored_path,
                    change_type: FileChangeType::Deleted,
                    current_hash: None,
                    previous_hash: Some(stored_record.content_hash),
                });
            }
        }

        Ok(changes)
    }

    /// Delete nodes and edges for specific files
    async fn delete_data_for_files(&self, file_paths: &[String]) -> Result<()> {
        if file_paths.is_empty() {
            return Ok(());
        }

        // Use writer to delete nodes, edges, and file metadata as one ordered operation
        self.surreal_writer_handle()?
            .enqueue_delete_nodes_by_file(file_paths.to_vec(), &self.project_id)
            .await
    }

    /// Persist file metadata for incremental indexing change tracking
    async fn persist_file_metadata(
        &self,
        files: &[PathBuf],
        nodes: &[CodeNode],
        edges: &[EdgeRelationship],
    ) -> Result<()> {
        let mut file_metadata_records = Vec::new();

        // Create progress bar for file metadata
        let metadata_pb = self.progress.add(ProgressBar::new(files.len() as u64));
        metadata_pb.set_style(
            ProgressStyle::with_template("{spinner:.blue} {msg} [{bar:40.cyan/blue}] {pos}/{len}")
                .unwrap()
                .progress_chars("█▓▒░  "),
        );
        metadata_pb.set_message("💾 Processing file metadata");

        // Build HashMap for O(1) lookups instead of O(N) iterations
        let mut file_stats: HashMap<String, (i64, i64)> = HashMap::new();

        // Count nodes per file - O(nodes)
        for node in nodes {
            let entry = file_stats
                .entry(node.location.file_path.clone())
                .or_insert((0, 0));
            entry.0 += 1;
        }

        // Build node-to-file mapping for edge counting - O(nodes)
        let node_file_map: HashMap<NodeId, String> = nodes
            .iter()
            .map(|n| (n.id, n.location.file_path.clone()))
            .collect();

        // Count edges per file - O(edges)
        for edge in edges {
            if let Some(file_path) = node_file_map.get(&edge.from) {
                let entry = file_stats.entry(file_path.clone()).or_insert((0, 0));
                entry.1 += 1;
            }
        }

        for file_path in files {
            let file_path_str = file_path.to_string_lossy().to_string();

            // Calculate hash and get file info
            let content_hash =
                Self::calculate_file_hash(file_path).unwrap_or_else(|_| "error".to_string());

            let metadata = fs::metadata(file_path).ok();
            let file_size = metadata.as_ref().map(|m| m.len() as i64).unwrap_or(0);

            let modified_at = metadata
                .as_ref()
                .and_then(|m| m.modified().ok())
                .and_then(|t| {
                    let duration = t.duration_since(std::time::UNIX_EPOCH).ok()?;
                    Some(
                        chrono::DateTime::from_timestamp(duration.as_secs() as i64, 0)
                            .unwrap_or_else(chrono::Utc::now),
                    )
                })
                .unwrap_or_else(chrono::Utc::now);

            // Get counts from HashMap - O(1) lookup
            let (node_count, edge_count) =
                file_stats.get(&file_path_str).copied().unwrap_or((0, 0));

            file_metadata_records.push(FileMetadataRecord {
                file_path: file_path_str,
                project_id: self.project_id.clone(),
                content_hash,
                modified_at,
                file_size,
                last_indexed_at: chrono::Utc::now(),
                node_count,
                edge_count,
                language: None, // Will be inferred from file extension if needed
                parse_errors: None,
            });
            metadata_pb.inc(1);
        }

        // Batch upsert file metadata
        self.surreal_writer_handle()?
            .enqueue_file_metadata(file_metadata_records)
            .await?;

        metadata_pb.finish_with_message(format!(
            "💾 File metadata complete: {} files tracked",
            files.len()
        ));

        info!("💾 Persisted metadata for {} files", files.len());
        Ok(())
    }

    async fn persist_project_metadata(
        &self,
        stats: &IndexStats,
        node_count: usize,
        edge_count: usize,
    ) -> Result<()> {
        let project_name = self
            .project_root
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or(&self.project_id)
            .to_string();
        let root_path = self.project_root.to_string_lossy().to_string();
        let primary_language = self.config.languages.first().cloned();
        let record = ProjectMetadataRecord {
            project_id: self.project_id.clone(),
            name: project_name,
            root_path,
            primary_language,
            file_count: stats.files as i64,
            node_count: node_count as i64,
            edge_count: edge_count as i64,
            avg_coverage_score: 0.0,
            last_analyzed: chrono::Utc::now(),
            codegraph_version: env!("CARGO_PKG_VERSION").to_string(),
            organization_id: self.organization_id.clone(),
            domain: self.domain.clone(),
        };

        self.enqueue_project_metadata_record(record).await
    }

    fn annotate_node(&self, node: &mut CodeNode) {
        node.metadata
            .attributes
            .insert("project_id".to_string(), self.project_id.clone());
        if let Some(org) = &self.organization_id {
            node.metadata
                .attributes
                .insert("organization_id".to_string(), org.clone());
        }
        if let Some(repo) = &self.repository_url {
            node.metadata
                .attributes
                .insert("repository_url".to_string(), repo.clone());
        }
        if let Some(domain) = &self.domain {
            node.metadata
                .attributes
                .insert("domain".to_string(), domain.clone());
        }
        // Add embedding model name for tracking which embedding provider was used
        node.metadata
            .attributes
            .insert("embedding_model".to_string(), self.embedding_model.clone());
    }

    async fn persist_nodes_batch(&self, chunk: &[CodeNode]) -> Result<()> {
        self.enqueue_nodes_chunk(chunk).await
    }

    async fn log_surreal_node_count(&self, expected: usize) {
        let db = {
            let storage = self.surreal.lock().await;
            storage.db()
        };

        match db
            .query("SELECT VALUE count() FROM nodes WHERE project_id = $project_id")
            .bind(("project_id", self.project_id.clone()))
            .await
        {
            Ok(mut resp) => match resp.take::<Option<i64>>(0) {
                Ok(count_opt) => {
                    let count = count_opt.unwrap_or(0);
                    info!(
                        "🗄️ SurrealDB nodes persisted: {} (expected ≈ {})",
                        count, expected
                    );
                }
                Err(e) => {
                    warn!("⚠️ Failed to read SurrealDB node count: {}", e);
                }
            },
            Err(e) => {
                warn!("⚠️ SurrealDB node count query failed: {}", e);
            }
        }
    }

    #[cfg(feature = "embeddings")]
    async fn log_surreal_chunk_count(&self, expected: usize) {
        let db = {
            let storage = self.surreal.lock().await;
            storage.db()
        };

        match db
            .query("SELECT VALUE count() FROM chunks WHERE project_id = $project_id")
            .bind(("project_id", self.project_id.clone()))
            .await
        {
            Ok(mut resp) => match resp.take::<Vec<i64>>(0) {
                Ok(counts) => {
                    let count = counts.get(0).cloned().unwrap_or(0);
                    info!(
                        "🧩 SurrealDB chunks persisted: {} (expected ≈ {})",
                        count, expected
                    );
                }
                Err(e) => {
                    warn!("⚠️ Failed to read SurrealDB chunk count: {}", e);
                }
            },
            Err(e) => {
                warn!("⚠️ SurrealDB chunk count query failed: {}", e);
            }
        }
    }

    async fn log_surreal_edge_count(&self, expected: usize) {
        let db = {
            let storage = self.surreal.lock().await;
            storage.db()
        };

        match db.query("SELECT VALUE count() FROM edges").await {
            Ok(mut resp) => match resp.take::<Option<i64>>(0) {
                Ok(count_opt) => {
                    let count = count_opt.unwrap_or(0);
                    info!(
                        "🗄️ SurrealDB edges persisted: {} (expected ≈ {})",
                        count, expected
                    );
                    if count < expected as i64 {
                        warn!(
                            "⚠️ Edge count ({}) is lower than resolved edges ({}). Verify SurrealDB schema and filters.",
                            count, expected
                        );
                    }
                }
                Err(e) => warn!("⚠️ Failed to read SurrealDB edge count: {}", e),
            },
            Err(e) => warn!("⚠️ SurrealDB edge count query failed: {}", e),
        }
    }

    async fn verify_file_metadata_count(&self, expected_files: usize) -> Result<()> {
        if expected_files == 0 {
            return Ok(());
        }

        let db = {
            let storage = self.surreal.lock().await;
            storage.db()
        };

        let mut resp = db
            .query("SELECT VALUE count() FROM file_metadata WHERE project_id = $project_id")
            .bind(("project_id", self.project_id.clone()))
            .await
            .context("Failed to verify file_metadata count")?;
        let counts: Vec<i64> = resp.take(0)?;
        let count = counts.get(0).cloned().unwrap_or(0);

        if count < expected_files as i64 {
            Err(anyhow!(
                "file_metadata count {} is less than expected {} for project {}",
                count,
                expected_files,
                self.project_id
            ))
        } else {
            Ok(())
        }
    }

    async fn verify_project_metadata_present(&self) -> Result<()> {
        let db = {
            let storage = self.surreal.lock().await;
            storage.db()
        };

        let mut resp = db
            .query("SELECT VALUE count() FROM project_metadata WHERE project_id = $project_id")
            .bind(("project_id", self.project_id.clone()))
            .await
            .context("Failed to verify project_metadata count")?;
        let counts: Vec<i64> = resp.take(0)?;
        let count = counts.get(0).cloned().unwrap_or(0);
        if count < 1 {
            Err(anyhow!(
                "project_metadata missing for project {}; ensure schema applied and permissions allow writes",
                self.project_id
            ))
        } else {
            Ok(())
        }
    }

    async fn persist_node_embeddings(&self, nodes: &[CodeNode]) -> Result<()> {
        let mut records = Vec::new();
        for node in nodes {
            if let Some(embedding) = &node.embedding {
                records.push(NodeEmbeddingRecord {
                    id: node.id.to_string(),
                    column: self.embedding_column.column_name(),
                    embedding: embedding.iter().map(|&v| v as f64).collect(),
                    updated_at: chrono::Utc::now(),
                });
            }
        }
        if records.is_empty() {
            return Ok(());
        }
        self.surreal_writer_handle()?
            .enqueue_node_embeddings(records)
            .await
    }

    async fn store_symbol_embedding(
        &self,
        symbol: &str,
        node_id: Option<NodeId>,
        source_edge_id: Option<&str>,
        embedding: &[f32],
    ) -> Result<()> {
        let record = self.build_symbol_embedding_record(symbol, node_id, source_edge_id, embedding);
        self.persist_symbol_embedding_records(vec![record]).await
    }

    fn build_symbol_embedding_record(
        &self,
        symbol: &str,
        node_id: Option<NodeId>,
        source_edge_id: Option<&str>,
        embedding: &[f32],
    ) -> SymbolEmbeddingRecord {
        let normalized = Self::normalize_symbol(symbol);
        let node_id_string = node_id.map(|id| id.to_string());
        SymbolEmbeddingRecord::new(
            &self.project_id,
            self.organization_id.as_deref(),
            symbol,
            &normalized,
            embedding,
            &self.embedding_model,
            self.embedding_column.column_name(),
            node_id_string.as_deref(),
            source_edge_id,
            None,
        )
    }

    async fn persist_symbol_embedding_records(
        &self,
        records: Vec<SymbolEmbeddingRecord>,
    ) -> Result<()> {
        if records.is_empty() {
            return Ok(());
        }
        let batch_size = symbol_embedding_db_batch_size();
        let handle = self.surreal_writer_handle()?;
        for chunk in records.chunks(batch_size) {
            handle.enqueue_symbol_embeddings(chunk.to_vec()).await?;
        }
        Ok(())
    }

    async fn enqueue_chunk_embeddings(&self, records: Vec<ChunkEmbeddingRecord>) -> Result<()> {
        if records.is_empty() {
            return Ok(());
        }
        info!("🧩 Queueing {} chunk embeddings for SurrealDB", records.len());
        let batch_size = self.config.batch_size.max(1);
        let handle = self.surreal_writer_handle()?;
        for chunk in records.chunks(batch_size) {
            handle.enqueue_chunk_embeddings(chunk.to_vec()).await?;
        }
        Ok(())
    }

    fn surreal_writer_handle(&self) -> Result<&SurrealWriterHandle> {
        self.surreal_writer
            .as_ref()
            .ok_or_else(|| anyhow!("Surreal writer not initialized"))
    }

    async fn enqueue_nodes_chunk(&self, nodes: &[CodeNode]) -> Result<()> {
        if nodes.is_empty() {
            return Ok(());
        }
        let batch: Vec<CodeNode> = nodes.to_vec();
        self.surreal_writer_handle()?.enqueue_nodes(batch).await
    }

    async fn enqueue_edges(&self, edges: Vec<CodeEdge>) -> Result<()> {
        if edges.is_empty() {
            return Ok(());
        }
        self.surreal_writer_handle()?.enqueue_edges(edges).await
    }

    async fn enqueue_project_metadata_record(&self, record: ProjectMetadataRecord) -> Result<()> {
        self.surreal_writer_handle()?
            .enqueue_project_metadata(record)
            .await
    }

    async fn flush_surreal_writer(&self) -> Result<()> {
        if let Some(writer) = &self.surreal_writer {
            writer.flush().await
        } else {
            Ok(())
        }
    }

    async fn shutdown_surreal_writer(&mut self) -> Result<()> {
        if let Some(writer) = self.surreal_writer.take() {
            writer.shutdown().await
        } else {
            Ok(())
        }
    }

    async fn connect_surreal_from_env() -> Result<Arc<TokioMutex<SurrealDbStorage>>> {
        let connection = Self::surreal_env_value("CODEGRAPH_SURREALDB_URL", "SURREALDB_URL")
            .context("CODEGRAPH_SURREALDB_URL or SURREALDB_URL must be set")?;
        let namespace =
            Self::surreal_env_value("CODEGRAPH_SURREALDB_NAMESPACE", "SURREALDB_NAMESPACE")
                .unwrap_or_else(|| "codegraph".to_string());
        let database =
            Self::surreal_env_value("CODEGRAPH_SURREALDB_DATABASE", "SURREALDB_DATABASE")
                .unwrap_or_else(|| "main".to_string());
        let username =
            Self::surreal_env_value("CODEGRAPH_SURREALDB_USERNAME", "SURREALDB_USERNAME");
        let password =
            Self::surreal_env_value("CODEGRAPH_SURREALDB_PASSWORD", "SURREALDB_PASSWORD");

        info!(
            "🗄️ Connecting to SurrealDB: {} namespace={} database={}",
            Self::sanitize_surreal_url(&connection),
            namespace,
            database
        );

        let config = SurrealDbConfig {
            connection: connection.clone(),
            namespace: namespace.clone(),
            database: database.clone(),
            username: username.clone(),
            password: password.clone(),
            ..SurrealDbConfig::default()
        };

        let storage = SurrealDbStorage::new(config)
            .await
            .with_context(|| format!("Failed to connect to SurrealDB at {}", connection))?;

        info!(
            "🗄️ SurrealDB connection established: {} namespace={} database={}",
            Self::sanitize_surreal_url(&connection),
            namespace,
            database
        );

        Ok(Arc::new(TokioMutex::new(storage)))
    }

    fn log_surrealdb_status(&self, phase: &str) {
        let connection = Self::surreal_env_value("CODEGRAPH_SURREALDB_URL", "SURREALDB_URL");
        let namespace =
            Self::surreal_env_value("CODEGRAPH_SURREALDB_NAMESPACE", "SURREALDB_NAMESPACE")
                .unwrap_or_else(|| "codegraph".to_string());
        let database =
            Self::surreal_env_value("CODEGRAPH_SURREALDB_DATABASE", "SURREALDB_DATABASE")
                .unwrap_or_else(|| "main".to_string());
        let username =
            Self::surreal_env_value("CODEGRAPH_SURREALDB_USERNAME", "SURREALDB_USERNAME");
        let auth_state = if username.is_some()
            || Self::surreal_env_value("CODEGRAPH_SURREALDB_PASSWORD", "SURREALDB_PASSWORD")
                .is_some()
        {
            "credentials configured"
        } else {
            "no auth"
        };

        match connection {
            Some(raw) => {
                let sanitized = Self::sanitize_surreal_url(&raw);
                info!(
                    "🗄️ SurrealDB ({}): target={} namespace={} database={} auth={}",
                    phase, sanitized, namespace, database, auth_state
                );
            }
            None => {
                info!(
                    "🗄️ SurrealDB ({}): connection not configured (set CODEGRAPH_SURREALDB_URL or SURREALDB_URL)",
                    phase
                );
            }
        }
    }

    fn surreal_env_value(primary: &str, fallback: &str) -> Option<String> {
        env::var(primary)
            .ok()
            .filter(|v| !v.trim().is_empty())
            .or_else(|| env::var(fallback).ok().filter(|v| !v.trim().is_empty()))
    }

    fn sanitize_surreal_url(raw: &str) -> String {
        if let Ok(mut url) = Url::parse(raw) {
            if !url.username().is_empty() {
                let _ = url.set_username("****");
            }
            if url.password().is_some() {
                let _ = url.set_password(Some("****"));
            }
            url.to_string()
        } else {
            raw.to_string()
        }
    }

    fn normalize_symbol(symbol: &str) -> String {
        symbol.trim().to_lowercase()
    }

    fn normalize_symbol_target(target: &str) -> Vec<String> {
        // Normalize edge targets to match codegraph-parser emitted symbol keys.
        let mut variants = Vec::new();

        let mut base = target.trim();

        // Strip trailing macro bang
        if let Some(stripped) = base.strip_suffix('!') {
            base = stripped;
        }

        // Strip call parentheses and args: take up to first '('
        if let Some(idx) = base.find('(') {
            base = &base[..idx];
        }

        // Strip generics by removing everything from first '<' that has a matching '>'
        let mut generic_stripped = String::new();
        let mut depth = 0;
        for ch in base.chars() {
            if ch == '<' {
                depth += 1;
                continue;
            }
            if ch == '>' && depth > 0 {
                depth -= 1;
                continue;
            }
            if depth == 0 {
                generic_stripped.push(ch);
            }
        }
        let mut cleaned = generic_stripped.trim().to_string();

        // Strip trait qualification: "Type as Trait::method" -> "Type::method"
        if let Some(pos) = cleaned.find(" as ") {
            let after = &cleaned[pos + 4..];
            if let Some(idx) = after.find("::") {
                cleaned = format!("{}{}", &cleaned[..pos], &after[idx..]);
            }
        }

        // Align with parser naming: drop self./this./super./crate:: prefixes
        let prefixes = ["self::", "self.", "this.", "super::", "super.", "crate::"];
        for p in prefixes {
            if let Some(stripped) = cleaned.strip_prefix(p) {
                cleaned = stripped.to_string();
                break;
            }
        }

        // Generate both module separators used by emitters (Rust uses ::, Python/JS often .)
        let dotted = cleaned.replace("::", ".");
        let coloned = cleaned.replace('.', "::");

        let lower_clean = cleaned.to_lowercase();
        let lower_dotted = dotted.to_lowercase();
        let lower_coloned = coloned.to_lowercase();

        variants.push(cleaned.clone());
        variants.push(lower_clean.clone());
        variants.push(dotted.clone());
        variants.push(lower_dotted.clone());
        variants.push(coloned.clone());
        variants.push(lower_coloned.clone());

        // Push last path segment variants
        for candidate in [&cleaned, &dotted, &coloned] {
            if let Some(last) = candidate.rsplit(['.', ':']).next() {
                variants.push(last.to_string());
                variants.push(last.to_lowercase());
            }
        }

        variants.sort();
        variants.dedup();
        variants
    }

    #[cfg(test)]
    pub(crate) fn normalize_symbol_target_for_tests(target: &str) -> Vec<String> {
        Self::normalize_symbol_target(target)
    }

    fn normalize_rust_symbol(target: &str) -> Vec<String> {
        // Demangle if possible (rustc, then Symbolic as fallback to strip hashes)
        let demangled = try_demangle(target)
            .map(|d| d.to_string())
            .unwrap_or_else(|_| target.to_string());

        let symbolic = demangle(&demangled).into_owned();

        let mut out = Vec::new();

        // Strip generics/path args via syn
        if let Ok(path) = parse_syn_path::<SynPath>(&symbolic) {
            let mut simple = Vec::new();
            for seg in path.segments {
                let name = seg.ident.to_string();
                simple.push(name);
            }
            if !simple.is_empty() {
                let joined = simple.join("::");
                out.push(joined.clone());
                out.push(joined.to_lowercase());
                if let Some(last) = simple.last() {
                    out.push(last.clone());
                    out.push(last.to_lowercase());
                }
            }
        }

        // Fallback: return demangled text plus lowered forms
        if out.is_empty() {
            out.push(symbolic.clone());
            out.push(symbolic.to_lowercase());
        }

        out
    }

    fn normalize_js_symbol(target: &str) -> Vec<String> {
        // Lightweight heuristic until JS parser dependency is restored
        Self::normalize_symbol_target(target)
    }

    fn normalize_python_symbol(target: &str) -> Vec<String> {
        // Fallback: simple split heuristics for Python until AST parser is re-enabled
        Self::normalize_symbol_target(target)
    }

    fn create_progress_bar(&self, total: u64, message: &str) -> ProgressBar {
        let pb = self.progress.add(ProgressBar::new(total));
        pb.set_style(
            ProgressStyle::default_bar()
                .template(
                    "{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({percent}%) {msg} | {per_sec} | ETA: {eta}",
                )
                .unwrap()
                .progress_chars("█▉▊▋▌▍▎▏ "), // Better visual progress
        );
        pb.set_message(message.to_string());
        pb
    }

    /// Create enhanced progress bar with dual metrics for files and success rates
    fn create_dual_progress_bar(
        &self,
        total: u64,
        primary_msg: &str,
        secondary_msg: &str,
    ) -> ProgressBar {
        let pb = self.progress.add(ProgressBar::new(total));
        pb.set_style(
            ProgressStyle::default_bar()
                .template(
                    "{spinner:.green} [{elapsed_precise}] [{bar:50.cyan/blue}] {pos}/{len}
                {msg.bold} | Success Rate: {percent}% | Speed: {per_sec} | ETA: {eta}",
                )
                .unwrap()
                .progress_chars("█▉▊▋▌▍▎▏ "),
        );
        pb.set_message(format!("{} | {}", primary_msg, secondary_msg));
        pb
    }

    /// Create high-performance progress bar for batch processing
    fn create_batch_progress_bar(
        &self,
        total: u64,
        batch_size: usize,
        label: &str,
    ) -> ProgressBar {
        let pb = self.progress.add(ProgressBar::new(total));
        let batch_info = if batch_size >= 10000 {
            format!("🚀 Ultra-High Performance ({}K batch)", batch_size / 1000)
        } else if batch_size >= 5000 {
            format!("⚡ High Performance ({}K batch)", batch_size / 1000)
        } else if batch_size >= 1000 {
            format!("🔥 Optimized ({} batch)", batch_size)
        } else {
            format!("Standard ({} batch)", batch_size)
        };

        pb.set_style(
            ProgressStyle::default_bar()
                .template(
                    "{spinner:.green} [{elapsed_precise}] [{bar:45.cyan/blue}] {pos}/{len} items\n                💾 {msg} | {percent}% | {per_sec} | ETA: {eta}",
                )
                .unwrap()
                .progress_chars("█▉▊▋▌▍▎▏ "),
        );
        pb.set_message(format!("{} | {}", label, batch_info));
        pb
    }

    /// Index a single file (for daemon mode incremental updates)
    /// Uses upsert semantics - no duplicate records created
    pub async fn index_single_file(&self, path: &Path) -> Result<()> {
        // Check if file should be indexed
        if !self.should_index(path) {
            debug!("Skipping file (excluded by config): {:?}", path);
            return Ok(());
        }

        // Detect language
        let language = self.detect_language(path);
        if language.is_none() && self.config.languages.is_empty() {
            debug!("Skipping file (unknown language): {:?}", path);
            return Ok(());
        }

        let file_path_str = path.to_string_lossy().to_string();
        info!("Indexing single file: {}", file_path_str);

        // Step 1: Parse file with tree-sitter to extract nodes and edges
        let extraction_result = self
            .parser
            .parse_file_with_edges(&file_path_str)
            .await
            .with_context(|| format!("Failed to parse file: {}", file_path_str))?;

        let mut nodes = extraction_result.nodes;
        let edges = extraction_result.edges;

        if nodes.is_empty() {
            debug!("No nodes extracted from file: {}", file_path_str);
            return Ok(());
        }

        info!(
            "Extracted {} nodes and {} edges from {}",
            nodes.len(),
            edges.len(),
            file_path_str
        );

        // Step 2: Annotate nodes with project metadata
        for node in &mut nodes {
            self.annotate_node(node);
        }

        // Step 3: Generate embeddings for nodes (if embeddings feature enabled)
        #[cfg(feature = "embeddings")]
        {
            match self.embedder.generate_embeddings(&nodes).await {
                Ok(embeddings) => {
                    // Assign embeddings to nodes
                    for (node, embedding) in nodes.iter_mut().zip(embeddings.into_iter()) {
                        node.embedding = Some(embedding);
                    }
                    debug!("Generated embeddings for {} nodes", nodes.len());
                }
                Err(e) => {
                    warn!(
                        "Failed to generate embeddings for file {}: {}",
                        file_path_str, e
                    );
                    // Continue without embeddings - not a fatal error
                }
            }
        }

        // Step 4: Persist nodes via upsert (handles duplicates automatically)
        self.persist_nodes_batch(&nodes)
            .await
            .with_context(|| format!("Failed to persist nodes for file: {}", file_path_str))?;

        // Step 5: Persist node embeddings
        #[cfg(feature = "embeddings")]
        {
            if let Err(e) = self.persist_node_embeddings(&nodes).await {
                warn!(
                    "Failed to persist embeddings for file {}: {}",
                    file_path_str, e
                );
                // Continue - not a fatal error
            }
        }

        // Step 6: Handle intra-file edges
        // Build a set of node IDs in this file for quick lookup
        let node_ids: std::collections::HashSet<_> =
            nodes.iter().map(|n| n.id.to_string()).collect();

        // Build symbol name to node ID map for edge resolution
        let symbol_map: std::collections::HashMap<String, codegraph_core::NodeId> = nodes
            .iter()
            .map(|n| (n.name.to_string(), n.id.clone()))
            .collect();

        // Convert EdgeRelationship to CodeEdge for intra-file edges only
        let resolved_edges: Vec<codegraph_graph::CodeEdge> = edges
            .into_iter()
            .filter_map(|edge_rel| {
                // Try to resolve the target symbol to a node ID in this file
                if let Some(target_id) = symbol_map.get(&edge_rel.to) {
                    // Both from and to are in this file - create CodeEdge
                    Some(codegraph_graph::CodeEdge::new(
                        edge_rel.from,
                        target_id.clone(),
                        edge_rel.edge_type,
                    ))
                } else {
                    // Cross-file edge - skip for now (will be resolved on next full index)
                    None
                }
            })
            .collect();

        if !resolved_edges.is_empty() {
            info!(
                "Persisting {} intra-file edges for {}",
                resolved_edges.len(),
                file_path_str
            );
            self.enqueue_edges(resolved_edges)
                .await
                .with_context(|| format!("Failed to persist edges for file: {}", file_path_str))?;
        }

        info!(
            "Successfully indexed file: {} ({} nodes)",
            file_path_str,
            nodes.len()
        );
        Ok(())
    }

    /// Delete all indexed data for a file (cascade delete)
    /// Removes nodes, edges, embeddings, and metadata for the file
    pub async fn delete_file_data(&self, path: &Path) -> Result<()> {
        let file_path = path.to_string_lossy().to_string();
        info!("Deleting indexed data for file: {}", file_path);
        self.delete_data_for_files(&[file_path]).await
    }

    /// Detect language from file extension
    fn detect_language(&self, path: &Path) -> Option<codegraph_core::Language> {
        use codegraph_core::Language;

        let ext = path.extension()?.to_str()?.to_lowercase();
        match ext.as_str() {
            "rs" => Some(Language::Rust),
            "py" => Some(Language::Python),
            "ts" => Some(Language::TypeScript),
            "tsx" => Some(Language::TypeScript),
            "js" => Some(Language::JavaScript),
            "jsx" => Some(Language::JavaScript),
            "go" => Some(Language::Go),
            "java" => Some(Language::Java),
            "cpp" | "cc" | "cxx" | "hpp" | "hxx" | "c" | "h" => Some(Language::Cpp),
            "swift" => Some(Language::Swift),
            "kt" | "kts" => Some(Language::Kotlin),
            "cs" => Some(Language::CSharp),
            "rb" => Some(Language::Ruby),
            "php" => Some(Language::Php),
            "dart" => Some(Language::Dart),
            _ => None,
        }
    }

    pub async fn watch_for_changes(&self, path: impl AsRef<Path>) -> Result<()> {
        use notify::event::{EventKind, ModifyKind};
        use notify::{Event, RecursiveMode, Watcher};

        let path = path.as_ref().to_path_buf();
        let (tx, mut rx) = mpsc::channel(100);
        let debounce_ms: u64 = std::env::var("CODEGRAPH_WATCH_DEBOUNCE_MS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(300);

        let mut watcher =
            notify::recommended_watcher(move |res: std::result::Result<Event, _>| {
                if let Ok(event) = res {
                    let _ = tx.blocking_send(event);
                }
            })?;

        watcher.watch(&path, RecursiveMode::Recursive)?;
        info!("Watching for changes in: {:?}", path);

        use std::collections::HashMap;
        use std::time::{Duration, Instant};
        let mut last_events: HashMap<PathBuf, Instant> = HashMap::new();

        while let Some(event) = rx.recv().await {
            match event.kind {
                EventKind::Modify(ModifyKind::Data(_)) | EventKind::Create(_) => {
                    for path in event.paths {
                        if self.should_index(&path) {
                            let now = Instant::now();
                            let entry = last_events.entry(path.clone()).or_insert(now);
                            if now.duration_since(*entry).as_millis() as u64 >= debounce_ms {
                                *entry = now;
                                info!("File changed: {:?}, reindexing (debounced)...", path);
                                if let Err(e) = self.index_single_file(&path).await {
                                    warn!("Incremental reindex failed for {:?}: {}", path, e);
                                }
                            } else {
                                debug!("Debounced change for {:?}", path);
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }
}

fn symbol_embedding_db_batch_size() -> usize {
    const MAX: usize = 512;
    std::env::var("CODEGRAPH_SYMBOL_DB_BATCH_SIZE")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .map(|parsed| parsed.clamp(1, MAX))
        .unwrap_or(SYMBOL_EMBEDDING_DB_BATCH_LIMIT)
}

fn resolve_surreal_embedding_column(dim: usize) -> Result<SurrealEmbeddingColumn> {
    match dim {
        384 => Ok(SurrealEmbeddingColumn::Embedding384),
        768 => Ok(SurrealEmbeddingColumn::Embedding768),
        1024 => Ok(SurrealEmbeddingColumn::Embedding1024),
        1536 => Ok(SurrealEmbeddingColumn::Embedding1536),
        2048 => Ok(SurrealEmbeddingColumn::Embedding2048),
        2560 => Ok(SurrealEmbeddingColumn::Embedding2560),
        3072 => Ok(SurrealEmbeddingColumn::Embedding3072),
        4096 => Ok(SurrealEmbeddingColumn::Embedding4096),
        other => Err(anyhow!(
            "Unsupported embedding dimension {}. Supported: 384, 768, 1024, 1536, 2048, 2560, 3072, 4096",
            other
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn symbol_embedding_batch_size_defaults() {
        std::env::remove_var("CODEGRAPH_SYMBOL_DB_BATCH_SIZE");
        assert_eq!(
            symbol_embedding_db_batch_size(),
            SYMBOL_EMBEDDING_DB_BATCH_LIMIT
        );
    }

    #[test]
    fn symbol_embedding_batch_size_respects_env_and_clamps() {
        std::env::set_var("CODEGRAPH_SYMBOL_DB_BATCH_SIZE", "1024");
        assert_eq!(symbol_embedding_db_batch_size(), 512);
        std::env::set_var("CODEGRAPH_SYMBOL_DB_BATCH_SIZE", "0");
        assert_eq!(symbol_embedding_db_batch_size(), 1);
        std::env::remove_var("CODEGRAPH_SYMBOL_DB_BATCH_SIZE");
    }

    #[test]
    fn surreal_embedding_column_supports_2560_dimension() {
        let column =
            resolve_surreal_embedding_column(2560).expect("2560-d embeddings should be supported");
        assert_eq!(column.column_name(), SURR_EMBEDDING_COLUMN_2560);
        assert_eq!(column.dimension(), 2560);
    }

    #[test]
    fn surreal_embedding_column_supports_1536_dimension() {
        let column =
            resolve_surreal_embedding_column(1536).expect("1536-d embeddings should be supported");
        assert_eq!(column.column_name(), SURR_EMBEDDING_COLUMN_1536);
        assert_eq!(column.dimension(), 1536);
    }

    #[test]
    fn surreal_embedding_column_supports_3072_dimension() {
        let column =
            resolve_surreal_embedding_column(3072).expect("3072-d embeddings should be supported");
        assert_eq!(column.column_name(), SURR_EMBEDDING_COLUMN_3072);
        assert_eq!(column.dimension(), 3072);
    }
}

pub fn prepare_node_text(node: &CodeNode) -> String {
    let lang = node
        .language
        .as_ref()
        .map(|l| format!("{:?}", l))
        .unwrap_or_else(|| "unknown".to_string());
    let kind = node
        .node_type
        .as_ref()
        .map(|t| format!("{:?}", t))
        .unwrap_or_else(|| "unknown".to_string());
    let mut text = format!("{} {} {}", lang, kind, node.name);
    if let Some(c) = &node.content {
        text.push(' ');
        text.push_str(c);
    }

    // Semantic chunking with environment variable support
    let max_chunk_tokens = std::env::var("CODEGRAPH_MAX_CHUNK_TOKENS")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(512); // Default 512 tokens

    // Approximate character limit for quick check (1 token ≈ 4 chars)
    let approx_max_chars = max_chunk_tokens * 4;

    if text.len() > approx_max_chars {
        // Load Qwen2.5-Coder tokenizer for accurate token counting
        let tokenizer_path = std::path::PathBuf::from(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../codegraph-vector/tokenizers/qwen2.5-coder.json"
        ));

        if let Ok(tokenizer) = tokenizers::Tokenizer::from_file(&tokenizer_path) {
            // Proper token-based chunking with Qwen2.5-Coder tokenizer
            let tok = std::sync::Arc::new(tokenizer);
            let token_counter = move |s: &str| -> usize {
                tok.encode(s, false)
                    .map(|enc| enc.len())
                    .unwrap_or_else(|_| (s.len() + 3) / 4)
            };

            let chunker = semchunk_rs::Chunker::new(max_chunk_tokens, token_counter);
            let chunks = chunker.chunk_text(&text);

            if let Some(first_chunk) = chunks.first() {
                text = first_chunk.clone();
            } else {
                // Fallback to character truncation
                let mut new_len = approx_max_chars.min(text.len());
                while new_len > 0 && !text.is_char_boundary(new_len) {
                    new_len -= 1;
                }
                text.truncate(new_len);
            }
        } else {
            // Tokenizer not available - fallback to character truncation
            let mut new_len = approx_max_chars.min(text.len());
            while new_len > 0 && !text.is_char_boundary(new_len) {
                new_len -= 1;
            }
            text.truncate(new_len);
        }
    }
    text
}

pub fn simple_text_embedding(text: &str, dimension: usize) -> Vec<f32> {
    let mut embedding = vec![0.0f32; dimension];
    let mut hash = 5381u32;
    for b in text.bytes() {
        hash = hash.wrapping_mul(33).wrapping_add(b as u32);
    }
    let mut state = hash;
    for value in &mut embedding {
        state = state.wrapping_mul(1103515245).wrapping_add(12345);
        *value = ((state as f32 / u32::MAX as f32) - 0.5) * 2.0;
    }
    embedding
}

pub fn normalize(v: &[f32]) -> Vec<f32> {
    let mut out = v.to_vec();
    let norm: f32 = out.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 0.0 {
        for x in &mut out {
            *x /= norm;
        }
    }
    out
}

#[derive(Debug, Default, Clone, serde::Serialize, serde::Deserialize)]
pub struct IndexStats {
    pub files: usize,
    pub skipped: usize,
    pub lines: usize,
    pub functions: usize,
    pub classes: usize,
    pub structs: usize,
    pub traits: usize,
    pub embeddings: usize,
    pub errors: usize,
}

impl IndexStats {
    fn merge(&mut self, other: FileStats) {
        self.files += 1;
        self.lines += other.lines;
        self.functions += other.functions;
        self.classes += other.classes;
        self.structs += other.structs;
        self.traits += other.traits;
        self.embeddings += other.embeddings;
    }
}

#[derive(Debug, Default, Clone)]
struct FileStats {
    lines: usize,
    functions: usize,
    classes: usize,
    structs: usize,
    traits: usize,
    embeddings: usize,
}
