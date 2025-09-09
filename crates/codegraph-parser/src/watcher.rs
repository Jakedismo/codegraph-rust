use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::Result;
use chrono::{DateTime, Utc};
use crossbeam_channel::{unbounded, Receiver, Sender};
use dashmap::DashMap;
use futures::stream::StreamExt;
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use parking_lot::{Mutex, RwLock};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio::fs;
use tracing::{debug, error, info, warn};

use codegraph_core::{CodeGraphError, Language};
use crate::{LanguageRegistry, TreeSitterParser};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FileId(pub String);

impl From<&Path> for FileId {
    fn from(path: &Path) -> Self {
        Self(path.to_string_lossy().to_string())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileMetadata {
    pub path: PathBuf,
    pub size: u64,
    pub modified: SystemTime,
    pub content_hash: String,
    pub language: Option<Language>,
}

#[derive(Debug, Clone)]
pub enum FileChangeEvent {
    Created(FileId, FileMetadata),
    Modified(FileId, FileMetadata, FileMetadata), // New, Old
    Deleted(FileId, FileMetadata),
    Renamed(FileId, FileId, FileMetadata), // From, To, Metadata
}

#[derive(Debug, Clone)]
pub struct BatchedChanges {
    pub changes: Vec<FileChangeEvent>,
    pub timestamp: DateTime<Utc>,
    pub batch_id: String,
}

pub struct FileSystemWatcher {
    watcher: Option<RecommendedWatcher>,
    file_registry: Arc<DashMap<FileId, FileMetadata>>,
    language_registry: Arc<LanguageRegistry>,
    event_sender: Sender<FileChangeEvent>,
    event_receiver: Receiver<FileChangeEvent>,
    watched_directories: Arc<RwLock<HashSet<PathBuf>>>,
    debounce_duration: Duration,
    batch_timeout: Duration,
    file_filters: Arc<RwLock<HashSet<String>>>, // File extensions to watch
}

impl FileSystemWatcher {
    pub fn new() -> Result<Self> {
        let (event_sender, event_receiver) = unbounded();
        
        Ok(Self {
            watcher: None,
            file_registry: Arc::new(DashMap::new()),
            language_registry: Arc::new(LanguageRegistry::new()),
            event_sender,
            event_receiver,
            watched_directories: Arc::new(RwLock::new(HashSet::new())),
            debounce_duration: Duration::from_millis(100),
            batch_timeout: Duration::from_millis(500),
            file_filters: Arc::new(RwLock::new(Self::default_file_filters())),
        })
    }

    fn default_file_filters() -> HashSet<String> {
        [
            "rs", "py", "js", "ts", "tsx", "jsx", "go", "java", "cpp", "hpp", "c", "h"
        ].iter().map(|s| s.to_string()).collect()
    }

    pub async fn add_watch_directory<P: AsRef<Path>>(&mut self, path: P) -> Result<()> {
        let path = path.as_ref().to_path_buf();
        info!("Adding watch directory: {:?}", path);

        // Initialize file registry for this directory
        self.scan_directory(&path).await?;

        // Setup file system watcher
        if self.watcher.is_none() {
            let event_sender = self.event_sender.clone();
            let file_registry = self.file_registry.clone();
            let language_registry = self.language_registry.clone();
            let file_filters = self.file_filters.clone();

            let mut watcher = notify::recommended_watcher(move |result| {
                match result {
                    Ok(event) => {
                        Self::handle_fs_event(
                            event,
                            &event_sender,
                            &file_registry,
                            &language_registry,
                            &file_filters,
                        );
                    }
                    Err(e) => error!("File watcher error: {:?}", e),
                }
            })?;

            watcher.watch(&path, RecursiveMode::Recursive)?;
            self.watcher = Some(watcher);
        } else if let Some(ref mut watcher) = self.watcher {
            watcher.watch(&path, RecursiveMode::Recursive)?;
        }

        self.watched_directories.write().insert(path);
        Ok(())
    }

    pub fn remove_watch_directory<P: AsRef<Path>>(&mut self, path: P) -> Result<()> {
        let path = path.as_ref().to_path_buf();
        
        if let Some(ref mut watcher) = self.watcher {
            watcher.unwatch(&path)?;
        }

        self.watched_directories.write().remove(&path);
        
        // Remove files from registry
        let to_remove: Vec<FileId> = self.file_registry
            .iter()
            .filter_map(|entry| {
                if entry.value().path.starts_with(&path) {
                    Some(entry.key().clone())
                } else {
                    None
                }
            })
            .collect();

        for file_id in to_remove {
            self.file_registry.remove(&file_id);
        }

        info!("Removed watch directory: {:?}", path);
        Ok(())
    }

    async fn scan_directory(&self, dir_path: &Path) -> Result<()> {
        let mut stack = vec![dir_path.to_path_buf()];

        while let Some(current_dir) = stack.pop() {
            let mut entries = fs::read_dir(&current_dir).await?;

            while let Some(entry) = entries.next_entry().await? {
                let path = entry.path();
                
                if path.is_dir() {
                    stack.push(path);
                } else if self.should_track_file(&path) {
                    if let Ok(metadata) = self.create_file_metadata(&path).await {
                        let file_id = FileId::from(path.as_path());
                        self.file_registry.insert(file_id, metadata);
                    }
                }
            }
        }

        Ok(())
    }

    fn should_track_file(&self, path: &Path) -> bool {
        if let Some(extension) = path.extension() {
            if let Some(ext_str) = extension.to_str() {
                return self.file_filters.read().contains(ext_str);
            }
        }
        false
    }

    async fn create_file_metadata(&self, path: &Path) -> Result<FileMetadata> {
        let file_metadata = fs::metadata(path).await?;
        let content = fs::read(path).await?;
        
        let mut hasher = Sha256::new();
        hasher.update(&content);
        let content_hash = format!("{:x}", hasher.finalize());

        let language = self.language_registry.detect_language(&path.to_string_lossy());

        Ok(FileMetadata {
            path: path.to_path_buf(),
            size: file_metadata.len(),
            modified: file_metadata.modified()?,
            content_hash,
            language,
        })
    }

    fn handle_fs_event(
        event: Event,
        event_sender: &Sender<FileChangeEvent>,
        file_registry: &DashMap<FileId, FileMetadata>,
        language_registry: &LanguageRegistry,
        file_filters: &RwLock<HashSet<String>>,
    ) {
        tokio::spawn(async move {
            if let Err(e) = Self::process_fs_event(
                event,
                event_sender,
                file_registry,
                language_registry,
                file_filters,
            ).await {
                error!("Error processing file system event: {:?}", e);
            }
        });
    }

    async fn process_fs_event(
        event: Event,
        event_sender: &Sender<FileChangeEvent>,
        file_registry: &DashMap<FileId, FileMetadata>,
        language_registry: &LanguageRegistry,
        file_filters: &RwLock<HashSet<String>>,
    ) -> Result<()> {
        debug!("Processing file system event: {:?}", event);

        for path in event.paths {
            if !Self::should_track_file_static(&path, file_filters) {
                continue;
            }

            let file_id = FileId::from(path.as_path());

            match event.kind {
                EventKind::Create(_) => {
                    if let Ok(metadata) = Self::create_file_metadata_static(&path, language_registry).await {
                        file_registry.insert(file_id.clone(), metadata.clone());
                        let _ = event_sender.send(FileChangeEvent::Created(file_id, metadata));
                    }
                }
                EventKind::Modify(_) => {
                    if let Ok(new_metadata) = Self::create_file_metadata_static(&path, language_registry).await {
                        if let Some(old_metadata) = file_registry.get(&file_id) {
                            let old_metadata = old_metadata.value().clone();
                            if old_metadata.content_hash != new_metadata.content_hash {
                                file_registry.insert(file_id.clone(), new_metadata.clone());
                                let _ = event_sender.send(FileChangeEvent::Modified(file_id, new_metadata, old_metadata));
                            }
                        } else {
                            // File wasn't tracked before, treat as creation
                            file_registry.insert(file_id.clone(), new_metadata.clone());
                            let _ = event_sender.send(FileChangeEvent::Created(file_id, new_metadata));
                        }
                    }
                }
                EventKind::Remove(_) => {
                    if let Some((_, old_metadata)) = file_registry.remove(&file_id) {
                        let _ = event_sender.send(FileChangeEvent::Deleted(file_id, old_metadata));
                    }
                }
                _ => {}
            }
        }

        Ok(())
    }

    fn should_track_file_static(path: &Path, file_filters: &RwLock<HashSet<String>>) -> bool {
        if let Some(extension) = path.extension() {
            if let Some(ext_str) = extension.to_str() {
                return file_filters.read().contains(ext_str);
            }
        }
        false
    }

    async fn create_file_metadata_static(path: &Path, language_registry: &LanguageRegistry) -> Result<FileMetadata> {
        let file_metadata = fs::metadata(path).await?;
        let content = fs::read(path).await?;
        
        let mut hasher = Sha256::new();
        hasher.update(&content);
        let content_hash = format!("{:x}", hasher.finalize());

        let language = language_registry.detect_language(&path.to_string_lossy());

        Ok(FileMetadata {
            path: path.to_path_buf(),
            size: file_metadata.len(),
            modified: file_metadata.modified()?,
            content_hash,
            language,
        })
    }

    pub async fn next_batch(&self) -> Option<BatchedChanges> {
        let mut changes = Vec::new();
        let start_time = std::time::Instant::now();

        // Collect changes within the batch timeout
        while start_time.elapsed() < self.batch_timeout {
            match tokio::time::timeout(self.debounce_duration, self.receive_change()).await {
                Ok(Some(change)) => {
                    changes.push(change);
                    // Continue collecting if we have more changes coming quickly
                    while let Ok(Ok(Some(additional_change))) = 
                        tokio::time::timeout(Duration::from_millis(10), self.receive_change()).await {
                        changes.push(additional_change);
                    }
                }
                Ok(None) => break,
                Err(_) => {
                    // Timeout - check if we have any changes to return
                    if !changes.is_empty() {
                        break;
                    }
                }
            }
        }

        if changes.is_empty() {
            return None;
        }

        Some(BatchedChanges {
            changes,
            timestamp: Utc::now(),
            batch_id: uuid::Uuid::new_v4().to_string(),
        })
    }

    async fn receive_change(&self) -> Option<FileChangeEvent> {
        match self.event_receiver.try_recv() {
            Ok(change) => Some(change),
            Err(_) => {
                // No immediate changes, wait a bit
                tokio::time::sleep(Duration::from_millis(1)).await;
                match self.event_receiver.try_recv() {
                    Ok(change) => Some(change),
                    Err(_) => None,
                }
            }
        }
    }

    pub fn get_file_metadata(&self, file_id: &FileId) -> Option<FileMetadata> {
        self.file_registry.get(file_id).map(|entry| entry.value().clone())
    }

    pub fn get_tracked_files(&self) -> Vec<(FileId, FileMetadata)> {
        self.file_registry
            .iter()
            .map(|entry| (entry.key().clone(), entry.value().clone()))
            .collect()
    }

    pub fn add_file_filter(&self, extension: String) {
        self.file_filters.write().insert(extension);
    }

    pub fn remove_file_filter(&self, extension: &str) {
        self.file_filters.write().remove(extension);
    }

    pub fn set_debounce_duration(&mut self, duration: Duration) {
        self.debounce_duration = duration;
    }

    pub fn set_batch_timeout(&mut self, duration: Duration) {
        self.batch_timeout = duration;
    }
}

impl Drop for FileSystemWatcher {
    fn drop(&mut self) {
        if let Some(mut watcher) = self.watcher.take() {
            // Unwatch all directories
            for dir in self.watched_directories.read().iter() {
                let _ = watcher.unwatch(dir);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use tokio::fs;

    #[tokio::test]
    async fn test_file_system_watcher_creation() {
        let watcher = FileSystemWatcher::new();
        assert!(watcher.is_ok());
    }

    #[tokio::test]
    async fn test_add_watch_directory() {
        let temp_dir = TempDir::new().unwrap();
        let mut watcher = FileSystemWatcher::new().unwrap();
        
        let result = watcher.add_watch_directory(temp_dir.path()).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_file_tracking() {
        let temp_dir = TempDir::new().unwrap();
        let mut watcher = FileSystemWatcher::new().unwrap();
        
        // Create a test file
        let test_file = temp_dir.path().join("test.rs");
        fs::write(&test_file, "fn main() {}").await.unwrap();
        
        // Add watch directory
        watcher.add_watch_directory(temp_dir.path()).await.unwrap();
        
        // Check if file is tracked
        let file_id = FileId::from(test_file.as_path());
        let metadata = watcher.get_file_metadata(&file_id);
        assert!(metadata.is_some());
        assert_eq!(metadata.unwrap().language, Some(Language::Rust));
    }

    #[tokio::test]
    async fn test_file_change_detection() {
        let temp_dir = TempDir::new().unwrap();
        let mut watcher = FileSystemWatcher::new().unwrap();
        
        // Create a test file
        let test_file = temp_dir.path().join("test.rs");
        fs::write(&test_file, "fn main() {}").await.unwrap();
        
        // Add watch directory
        watcher.add_watch_directory(temp_dir.path()).await.unwrap();
        
        // Get initial hash
        let file_id = FileId::from(test_file.as_path());
        let initial_metadata = watcher.get_file_metadata(&file_id).unwrap();
        
        // Wait a bit to ensure different modification time
        tokio::time::sleep(Duration::from_millis(100)).await;
        
        // Modify the file
        fs::write(&test_file, "fn main() { println!(\"Hello\"); }").await.unwrap();
        
        // Give the watcher time to detect the change
        tokio::time::sleep(Duration::from_millis(200)).await;
        
        // Check for changes
        if let Some(batch) = watcher.next_batch().await {
            assert!(!batch.changes.is_empty());
            if let FileChangeEvent::Modified(_, new_metadata, old_metadata) = &batch.changes[0] {
                assert_ne!(new_metadata.content_hash, old_metadata.content_hash);
            }
        }
    }
}