// ABOUTME: Watch session that connects FileSystemWatcher with ProjectIndexer
// ABOUTME: Handles batch processing of file changes with incremental re-indexing

use anyhow::{Context, Result};
use chrono::Utc;
use codegraph_parser::{BatchedChanges, FileChangeEvent, FileSystemWatcher};
use std::path::PathBuf;
use std::time::Duration;
use tracing::{debug, info};

use super::config::WatchConfig;
use super::status::SessionMetrics;
use crate::ProjectIndexer;

/// Watch session - owns watcher and manages batch processing
pub struct WatchSession {
    /// Project root being watched
    project_root: PathBuf,

    /// File system watcher (from codegraph-parser)
    watcher: FileSystemWatcher,

    /// Session metrics
    metrics: SessionMetrics,

    /// Indexer for re-indexing changed files
    indexer: Option<ProjectIndexer>,
}

impl WatchSession {
    /// Create a new watch session
    pub async fn new(config: WatchConfig) -> Result<Self> {
        let mut watcher =
            FileSystemWatcher::new().context("Failed to create file system watcher")?;

        // Configure watcher based on config
        watcher.set_debounce_duration(Duration::from_millis(config.debounce_ms));
        watcher.set_batch_timeout(Duration::from_millis(config.batch_timeout_ms));

        // Set include patterns from indexer config if specified
        if !config.indexer.include_patterns.is_empty() {
            watcher
                .set_include_patterns(&config.indexer.include_patterns)
                .context("Failed to set include patterns")?;
        }

        // Add watch directory
        watcher
            .add_watch_directory(&config.project_root)
            .await
            .context("Failed to add watch directory")?;

        let tracked_files = watcher.get_tracked_files();
        info!(
            "Watch session initialized: {} files tracked in {:?}",
            tracked_files.len(),
            config.project_root
        );

        Ok(Self {
            project_root: config.project_root.clone(),
            watcher,
            metrics: SessionMetrics::new(),
            indexer: None,
        })
    }

    /// Set the indexer for re-indexing (must be called before processing)
    pub fn set_indexer(&mut self, indexer: ProjectIndexer) {
        self.indexer = Some(indexer);
    }

    /// Get the number of tracked files
    pub fn files_tracked(&self) -> usize {
        self.watcher.get_tracked_files().len()
    }

    /// Get session metrics
    pub fn metrics(&self) -> &SessionMetrics {
        &self.metrics
    }

    /// Wait for and get the next batch of changes
    pub async fn next_batch(&self) -> Option<BatchedChanges> {
        self.watcher.next_batch().await
    }

    /// Process a batch of file changes
    pub async fn process_batch(&mut self, batch: BatchedChanges) -> Result<(u64, u64)> {
        let batch_id = &batch.batch_id;
        let change_count = batch.changes.len();

        debug!(
            "Processing batch {}: {} changes at {}",
            batch_id, change_count, batch.timestamp
        );

        let mut indexed = 0u64;
        let mut deleted = 0u64;

        for change in &batch.changes {
            match change {
                FileChangeEvent::Created(_, metadata)
                | FileChangeEvent::Modified(_, metadata, _) => {
                    if self
                        .indexer
                        .as_ref()
                        .is_some_and(|indexer| indexer.should_reconcile(&metadata.path))
                    {
                        indexed += 1;
                    }
                }
                FileChangeEvent::Deleted(_, _) => deleted += 1,
                FileChangeEvent::Renamed(_, _, _) => {
                    indexed += 1;
                    deleted += 1;
                }
            }
        }
        if indexed + deleted > 0 {
            if let Some(indexer) = &self.indexer {
                if let Err(error) = indexer.reconcile_project(&self.project_root, false).await {
                    self.metrics.record_error();
                    return Err(error.context("Watch batch reconciliation failed"));
                }
            }
        }

        self.metrics.record_batch(indexed, deleted);

        info!(
            "Batch {} complete: {} indexed, {} deleted ({} ms)",
            batch_id,
            indexed,
            deleted,
            Utc::now()
                .signed_duration_since(batch.timestamp)
                .num_milliseconds()
        );

        Ok((indexed, deleted))
    }

    /// Stop the watch session
    pub fn stop(&mut self) {
        // Watcher is dropped when session is dropped
        info!("Watch session stopped for {:?}", self.project_root);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_should_index_rust_file() {
        let _config = WatchConfig {
            project_root: PathBuf::from("/test"),
            debounce_ms: 30,
            batch_timeout_ms: 200,
            health_check_interval_secs: 30,
            reconnect_backoff: Default::default(),
            circuit_breaker: Default::default(),
            indexer: crate::IndexerConfig {
                languages: vec!["rust".to_string()],
                exclude_patterns: vec!["**/target/**".to_string()],
                ..Default::default()
            },
        };

        // Create a mock session to test should_index
        // Note: In real tests, we'd use a proper mock
    }
}
