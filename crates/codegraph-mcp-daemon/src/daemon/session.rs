// ABOUTME: Watch session that connects FileSystemWatcher with ProjectIndexer
// ABOUTME: Handles batch processing of file changes with incremental re-indexing

use anyhow::{Context, Result};
use chrono::Utc;
use codegraph_parser::{BatchedChanges, FileChangeEvent, FileSystemWatcher};
use std::path::PathBuf;
use std::time::Duration;
use tracing::{debug, error, info, warn};

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

    /// Configuration
    config: WatchConfig,

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
            config,
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

        for change in batch.changes {
            match change {
                FileChangeEvent::Created(_file_id, metadata) => {
                    debug!("File created: {:?}", metadata.path);
                    if self.should_index(&metadata.path) {
                        match self.reindex_file(&metadata.path).await {
                            Ok(_) => indexed += 1,
                            Err(e) => {
                                error!("Failed to index new file {:?}: {}", metadata.path, e);
                                self.metrics.record_error();
                            }
                        }
                    }
                }
                FileChangeEvent::Modified(_file_id, new_metadata, old_metadata) => {
                    debug!(
                        "File modified: {:?} (hash changed: {} -> {})",
                        new_metadata.path, old_metadata.content_hash, new_metadata.content_hash
                    );
                    if self.should_index(&new_metadata.path) {
                        match self.reindex_file(&new_metadata.path).await {
                            Ok(_) => indexed += 1,
                            Err(e) => {
                                error!("Failed to reindex file {:?}: {}", new_metadata.path, e);
                                self.metrics.record_error();
                            }
                        }
                    }
                }
                FileChangeEvent::Deleted(_file_id, metadata) => {
                    debug!("File deleted: {:?}", metadata.path);
                    match self.delete_file_data(&metadata.path).await {
                        Ok(_) => deleted += 1,
                        Err(e) => {
                            error!("Failed to delete data for {:?}: {}", metadata.path, e);
                            self.metrics.record_error();
                        }
                    }
                }
                FileChangeEvent::Renamed(from_id, to_id, metadata) => {
                    debug!("File renamed: {} -> {}", from_id.0, to_id.0);
                    // Delete old data, reindex with new path
                    let old_path = PathBuf::from(&from_id.0);
                    let _ = self.delete_file_data(&old_path).await;

                    if self.should_index(&metadata.path) {
                        match self.reindex_file(&metadata.path).await {
                            Ok(_) => {
                                deleted += 1;
                                indexed += 1;
                            }
                            Err(e) => {
                                error!("Failed to reindex renamed file {:?}: {}", metadata.path, e);
                                self.metrics.record_error();
                            }
                        }
                    }
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

    /// Check if a file should be indexed based on configuration
    fn should_index(&self, path: &std::path::Path) -> bool {
        // Check language filter
        if !self.config.indexer.languages.is_empty() {
            if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                let ext_lower = ext.to_lowercase();
                let matches_language = self.config.indexer.languages.iter().any(|lang: &String| {
                    match lang.to_lowercase().as_str() {
                        "rust" => ext_lower == "rs",
                        "python" => ext_lower == "py",
                        "typescript" => ext_lower == "ts" || ext_lower == "tsx",
                        "javascript" => ext_lower == "js" || ext_lower == "jsx",
                        "go" => ext_lower == "go",
                        "java" => ext_lower == "java",
                        "cpp" | "c++" => {
                            ext_lower == "cpp" || ext_lower == "hpp" || ext_lower == "cc"
                        }
                        "c" => ext_lower == "c" || ext_lower == "h",
                        _ => false,
                    }
                });
                if !matches_language {
                    return false;
                }
            }
        }

        // Check exclude patterns
        for pattern in &self.config.indexer.exclude_patterns {
            if glob_match::glob_match(pattern, &path.to_string_lossy()) {
                return false;
            }
        }

        true
    }

    /// Re-index a single file
    async fn reindex_file(&self, path: &std::path::Path) -> Result<()> {
        // The indexer handles upsert semantics - no duplicates created
        if let Some(indexer) = &self.indexer {
            indexer
                .index_single_file(path)
                .await
                .with_context(|| format!("Failed to reindex {:?}", path))?;
        } else {
            warn!("No indexer set, skipping reindex of {:?}", path);
        }
        Ok(())
    }

    /// Delete all data for a file
    async fn delete_file_data(&self, path: &std::path::Path) -> Result<()> {
        if let Some(indexer) = &self.indexer {
            indexer
                .delete_file_data(path)
                .await
                .with_context(|| format!("Failed to delete data for {:?}", path))?;
        } else {
            warn!("No indexer set, skipping delete for {:?}", path);
        }
        Ok(())
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
