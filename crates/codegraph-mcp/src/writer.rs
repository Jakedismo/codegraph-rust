// ABOUTME: Bounded asynchronous graph writer with durable ordering barriers.
// ABOUTME: Separates database batching and queue memory from inference settings.

use anyhow::{Result, anyhow};
use codegraph_core::CodeNode;
use codegraph_graph::{
    ChunkEmbeddingRecord, FileMetadataRecord, NodeEmbeddingRecord, ProjectMetadataRecord,
    SurrealDbStorage, SymbolEmbeddingRecord, edge::CodeEdge,
};
use futures::{StreamExt, stream::FuturesUnordered};
use serde::Serialize;
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};
use tokio::sync::{Mutex, OwnedSemaphorePermit, Semaphore, mpsc, oneshot};
use tokio::task::JoinHandle;

enum Job {
    Nodes(Vec<CodeNode>),
    Edges(Vec<CodeEdge>),
    NodeEmbeddings(Vec<NodeEmbeddingRecord>),
    SymbolEmbeddings(Vec<SymbolEmbeddingRecord>),
    ChunkEmbeddings(Vec<ChunkEmbeddingRecord>),
    FileMetadata(Vec<FileMetadataRecord>),
    DeleteFiles { paths: Vec<String>, project: String },
    ProjectMetadata(ProjectMetadataRecord),
    Flush(oneshot::Sender<Result<()>>),
    Shutdown(oneshot::Sender<Result<()>>),
}

struct Envelope {
    job: Job,
    _memory: Option<OwnedSemaphorePermit>,
    bytes: u64,
}

#[derive(Clone, Copy)]
struct Limits {
    rows: usize,
    bytes: usize,
    queue_bytes: usize,
}

impl Limits {
    fn from_env() -> Self {
        fn value(name: &str, default: usize) -> usize {
            std::env::var(name)
                .ok()
                .and_then(|s| s.parse().ok())
                .filter(|v| *v > 0)
                .unwrap_or(default)
        }
        let queue_bytes =
            value("CODEGRAPH_WRITE_QUEUE_BYTES", 32 * 1024 * 1024).clamp(1024, u32::MAX as usize);
        Self {
            rows: value("CODEGRAPH_DB_BATCH_ROWS", 512),
            bytes: value("CODEGRAPH_DB_BATCH_BYTES", 4 * 1024 * 1024).min(queue_bytes),
            queue_bytes,
        }
    }
}

struct ByteCounter(usize);
impl std::io::Write for ByteCounter {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0 += bytes.len();
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

fn encoded_size(value: &impl Serialize) -> Result<usize> {
    let mut counter = ByteCounter(0);
    serde_json::to_writer(&mut counter, value)?;
    Ok(counter.0)
}

#[derive(Default)]
struct WriteMetrics {
    jobs: AtomicU64,
    rows: AtomicU64,
    bytes: AtomicU64,
}
impl Job {
    fn rows(&self) -> usize {
        match self {
            Self::Nodes(rows) => rows.len(),
            Self::Edges(rows) => rows.len(),
            Self::NodeEmbeddings(rows) => rows.len(),
            Self::SymbolEmbeddings(rows) => rows.len(),
            Self::ChunkEmbeddings(rows) => rows.len(),
            Self::FileMetadata(rows) => rows.len(),
            Self::ProjectMetadata(_) => 1,
            Self::DeleteFiles { paths, .. } => paths.len(),
            _ => 0,
        }
    }
}
async fn execute(
    job: Job,
    storage: Arc<Mutex<SurrealDbStorage>>,
    metrics: &WriteMetrics,
    bytes: u64,
) -> Result<()> {
    let rows = job.rows();
    let mut storage = storage.lock().await;
    match job {
        Job::Nodes(rows) => storage.upsert_nodes_batch(&rows).await?,
        Job::Edges(rows) => storage.upsert_edges_batch(&rows).await?,
        Job::NodeEmbeddings(rows) => storage.update_node_embeddings_batch(&rows).await?,
        Job::SymbolEmbeddings(rows) => storage.upsert_symbol_embeddings_batch(&rows).await?,
        Job::ChunkEmbeddings(rows) => storage.upsert_chunk_embeddings_resilient(&rows).await?,
        Job::FileMetadata(rows) => storage.upsert_file_metadata_batch(&rows).await?,
        Job::DeleteFiles { paths, project } => {
            storage.delete_data_for_files(&project, &paths).await?
        }
        Job::ProjectMetadata(row) => storage.upsert_project_metadata(row).await?,
        Job::Flush(_) | Job::Shutdown(_) => unreachable!("barriers handled by writer"),
    }
    metrics.jobs.fetch_add(1, Ordering::Relaxed);
    metrics.rows.fetch_add(rows as u64, Ordering::Relaxed);
    metrics.bytes.fetch_add(bytes, Ordering::Relaxed);
    Ok(())
}

fn record_result(
    result: std::result::Result<Result<()>, tokio::task::JoinError>,
    error: &mut Option<String>,
) {
    if let Err(err) = result
        .map_err(anyhow::Error::from)
        .and_then(|result| result)
    {
        tracing::error!("Graph write failed: {err}");
        error.get_or_insert_with(|| err.to_string());
    }
}

pub(crate) struct SurrealWriterHandle {
    tx: mpsc::Sender<Envelope>,
    memory: Arc<Semaphore>,
    limits: Limits,
    join: JoinHandle<()>,
    metrics: Arc<WriteMetrics>,
}

impl SurrealWriterHandle {
    pub(crate) fn new(pool: Vec<Arc<Mutex<SurrealDbStorage>>>) -> Self {
        assert!(!pool.is_empty(), "writer requires a storage handle");
        let limits = Limits::from_env();
        let memory = Arc::new(Semaphore::new(limits.queue_bytes));
        let (tx, mut rx) = mpsc::channel::<Envelope>(8);
        let metrics = Arc::new(WriteMetrics::default());
        let worker_metrics = metrics.clone();
        let join = tokio::spawn(async move {
            let mut running = FuturesUnordered::<JoinHandle<Result<()>>>::new();
            let mut error = None;
            let mut next = 0;
            while let Some(envelope) = rx.recv().await {
                let barrier = matches!(
                    envelope.job,
                    Job::DeleteFiles { .. }
                        | Job::FileMetadata(_)
                        | Job::ProjectMetadata(_)
                        | Job::Flush(_)
                        | Job::Shutdown(_)
                );
                if barrier {
                    while let Some(result) = running.next().await {
                        record_result(result, &mut error);
                    }
                    match envelope.job {
                        Job::Flush(response) => {
                            let _ = response.send(error.take().map_or(Ok(()), |e| Err(anyhow!(e))));
                        }
                        Job::Shutdown(response) => {
                            let _ = response.send(error.take().map_or(Ok(()), |e| Err(anyhow!(e))));
                            break;
                        }
                        job => {
                            // Completion metadata must never mark a failed write set current.
                            if error.is_none() {
                                if let Err(err) =
                                    execute(job, pool[0].clone(), &worker_metrics, envelope.bytes)
                                        .await
                                {
                                    error = Some(err.to_string());
                                }
                            }
                        }
                    }
                } else {
                    while running.len() >= pool.len() {
                        record_result(running.next().await.expect("in-flight write"), &mut error);
                    }
                    let storage = pool[next % pool.len()].clone();
                    next += 1;
                    let metrics = worker_metrics.clone();
                    running.push(tokio::spawn(async move {
                        let Envelope {
                            job,
                            _memory,
                            bytes,
                        } = envelope;
                        let result = execute(job, storage, &metrics, bytes).await;
                        drop(_memory);
                        result
                    }));
                }
            }
            // A dropped indexer closes the channel; finish accepted work before exiting.
            while let Some(result) = running.next().await {
                record_result(result, &mut error);
            }
        });
        Self {
            tx,
            memory,
            limits,
            join,
            metrics,
        }
    }

    pub(crate) fn metrics(&self) -> (u64, u64, u64) {
        (
            self.metrics.jobs.load(Ordering::Relaxed),
            self.metrics.rows.load(Ordering::Relaxed),
            self.metrics.bytes.load(Ordering::Relaxed),
        )
    }

    async fn send(&self, job: Job, bytes: usize) -> Result<()> {
        if bytes > self.limits.queue_bytes {
            return Err(anyhow!("Write exceeds queue byte budget: {bytes}"));
        }
        let permit = self
            .memory
            .clone()
            .acquire_many_owned(bytes.max(1) as u32)
            .await?;
        self.tx
            .send(Envelope {
                job,
                _memory: Some(permit),
                bytes: bytes as u64,
            })
            .await
            .map_err(|_| anyhow!("Graph writer unavailable"))
    }

    async fn batch<T: Serialize>(&self, rows: Vec<T>, wrap: impl Fn(Vec<T>) -> Job) -> Result<()> {
        let mut batch = Vec::new();
        let mut bytes = 2usize;
        for row in rows {
            // Count encoded bytes without allocating another serialized batch.
            let size = encoded_size(&row)? + 1;
            if size + 2 > self.limits.bytes {
                return Err(anyhow!(
                    "One graph record needs {} bytes; increase CODEGRAPH_DB_BATCH_BYTES (current {})",
                    size + 2,
                    self.limits.bytes
                ));
            }
            if !batch.is_empty()
                && (batch.len() >= self.limits.rows || bytes + size > self.limits.bytes)
            {
                self.send(wrap(std::mem::take(&mut batch)), bytes).await?;
                bytes = 2;
            }
            bytes += size;
            batch.push(row);
        }
        if !batch.is_empty() {
            self.send(wrap(batch), bytes).await?;
        }
        Ok(())
    }

    pub(crate) async fn enqueue_nodes(&self, rows: Vec<CodeNode>) -> Result<()> {
        self.batch(rows, Job::Nodes).await
    }
    pub(crate) async fn enqueue_edges(&self, rows: Vec<CodeEdge>) -> Result<()> {
        self.batch(rows, Job::Edges).await
    }
    pub(crate) async fn enqueue_node_embeddings(
        &self,
        rows: Vec<NodeEmbeddingRecord>,
    ) -> Result<()> {
        self.batch(rows, Job::NodeEmbeddings).await
    }
    pub(crate) async fn enqueue_symbol_embeddings(
        &self,
        rows: Vec<SymbolEmbeddingRecord>,
    ) -> Result<()> {
        self.batch(rows, Job::SymbolEmbeddings).await
    }
    pub(crate) async fn enqueue_chunk_embeddings(
        &self,
        rows: Vec<ChunkEmbeddingRecord>,
    ) -> Result<()> {
        self.batch(rows, Job::ChunkEmbeddings).await
    }
    pub(crate) async fn enqueue_file_metadata(&self, rows: Vec<FileMetadataRecord>) -> Result<()> {
        self.batch(rows, Job::FileMetadata).await
    }
    pub(crate) async fn enqueue_delete_nodes_by_file(
        &self,
        paths: Vec<String>,
        project: &str,
    ) -> Result<()> {
        self.batch(paths, |paths| Job::DeleteFiles {
            paths,
            project: project.to_string(),
        })
        .await
    }
    pub(crate) async fn enqueue_project_metadata(&self, row: ProjectMetadataRecord) -> Result<()> {
        let bytes = encoded_size(&row)?;
        self.send(Job::ProjectMetadata(row), bytes).await
    }
    pub(crate) async fn flush(&self) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        self.send(Job::Flush(tx), 1).await?;
        rx.await
            .map_err(|_| anyhow!("Graph writer ended before acknowledgement"))?
    }
    pub(crate) async fn shutdown(self) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        self.send(Job::Shutdown(tx), 1).await?;
        let result = rx
            .await
            .map_err(|_| anyhow!("Graph writer ended before acknowledgement"))?;
        self.join.await?;
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use codegraph_core::{Language, Location, NodeType};
    use codegraph_graph::SurrealDbConfig;

    async fn fixture() -> (SurrealWriterHandle, Arc<Mutex<SurrealDbStorage>>) {
        let dir = tempfile::tempdir().unwrap();
        let mut config = SurrealDbConfig::embedded(dir.path());
        config.connection = "mem://".into();
        config.database = format!(
            "writer_{}",
            dir.path()
                .file_name()
                .unwrap()
                .to_string_lossy()
                .replace('.', "_")
        );
        config.auto_migrate = false;
        let storage = SurrealDbStorage::new(config.clone()).await.unwrap();
        storage.db().query("DEFINE TABLE nodes SCHEMALESS; DEFINE FIELD name ON nodes TYPE string ASSERT $value != 'bad'; DEFINE TABLE project_metadata SCHEMALESS;").await.unwrap().check().unwrap();
        let first = Arc::new(Mutex::new(storage));
        let second = first.clone();
        let mut writer = SurrealWriterHandle::new(vec![first.clone(), second]);
        writer.limits.rows = 1;
        writer.limits.bytes = 2048;
        (writer, first)
    }

    fn node(name: &str) -> CodeNode {
        CodeNode::new(
            name.to_string(),
            Some(NodeType::Function),
            Some(Language::Rust),
            Location {
                file_path: "a.rs".into(),
                line: 1,
                column: 0,
                end_line: None,
                end_column: None,
            },
        )
    }

    fn metadata() -> ProjectMetadataRecord {
        ProjectMetadataRecord {
            metadata: serde_json::json!({}),
            project_id: "test".into(),
            name: "test".into(),
            root_path: "/test".into(),
            primary_language: None,
            file_count: 1,
            node_count: 1,
            edge_count: 0,
            avg_coverage_score: 1.0,
            last_analyzed: chrono::Utc::now(),
            codegraph_version: "test".into(),
            organization_id: None,
            domain: None,
        }
    }

    #[tokio::test]
    async fn failed_writes_do_not_advance_metadata_and_flush_allows_retry() {
        let (writer, storage) = fixture().await;
        writer
            .enqueue_nodes(vec![node("good"), node("bad")])
            .await
            .unwrap();
        writer.enqueue_project_metadata(metadata()).await.unwrap();
        assert!(writer.flush().await.is_err());
        let mut response = storage
            .lock()
            .await
            .db()
            .query("SELECT * FROM project_metadata")
            .await
            .unwrap()
            .check()
            .unwrap();
        let rows: Vec<serde_json::Value> = response.take(0).unwrap();
        assert!(rows.is_empty());
        writer.enqueue_nodes(vec![node("retry")]).await.unwrap();
        writer.enqueue_project_metadata(metadata()).await.unwrap();
        writer.flush().await.unwrap();
        let mut response = storage
            .lock()
            .await
            .db()
            .query("SELECT * FROM project_metadata")
            .await
            .unwrap()
            .check()
            .unwrap();
        let rows: Vec<serde_json::Value> = response.take(0).unwrap();
        assert_eq!(rows.len(), 1);
        writer.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn oversized_records_fail_before_entering_queue() {
        let (writer, _) = fixture().await;
        let large = node("large").with_content("x".repeat(4096));
        assert!(
            writer
                .enqueue_nodes(vec![large])
                .await
                .unwrap_err()
                .to_string()
                .contains("CODEGRAPH_DB_BATCH_BYTES")
        );
        writer.flush().await.unwrap();
        writer.shutdown().await.unwrap();
    }
}
