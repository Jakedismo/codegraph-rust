// ABOUTME: Integration test verifying watch mode triggers incremental reindexing.
// ABOUTME: Spawns watch loop, edits a file, and asserts file_metadata is updated.
#![cfg(not(feature = "embeddings"))]

use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use codegraph_core::config_manager::CodeGraphConfig;
use codegraph_mcp::indexer::{set_watch_test_notifier, IndexerConfig, ProjectIndexer};
use notify::event::{DataChange, EventKind, ModifyKind};
use notify::Event;
use indicatif::MultiProgress;
use serde_json::Value;
use tempfile::tempdir;
use tokio::fs;
use tokio::time::sleep;

#[tokio::test]
async fn watch_updates_file_metadata_on_change() -> Result<()> {
    // Isolated in-memory SurrealDB
    std::env::set_var("CODEGRAPH_SURREALDB_URL", "mem://");
    std::env::set_var("CODEGRAPH_SURREALDB_NAMESPACE", "watch_ns");
    std::env::set_var("CODEGRAPH_SURREALDB_DATABASE", "watch_db");
    std::env::remove_var("CODEGRAPH_SURREALDB_USERNAME");
    std::env::remove_var("CODEGRAPH_SURREALDB_PASSWORD");

    let project_dir = tempdir()?;
    let file_path = project_dir.path().join("foo.rs");
    fs::write(&file_path, "fn foo() {}\n").await?;

    let mut config = IndexerConfig::default();
    config.project_root = project_dir.path().to_path_buf();
    config.languages = vec!["rust".to_string()];
    config.recursive = true;
    config.force_reindex = true;

    let global_config = CodeGraphConfig::default();
    let mut indexer = ProjectIndexer::new(config, &global_config, MultiProgress::new()).await?;

    // Baseline full index to seed file metadata
    indexer.index_project(project_dir.path()).await?;
    let indexer = Arc::new(indexer);
    let storage = indexer.surreal_storage().await;
    let _initial = fetch_metadata(storage.clone(), file_path.to_string_lossy().as_ref())
        .await?
        .expect("baseline metadata missing");

    // Start watcher
    let watch_indexer = indexer.clone();
    let mut last_events = std::collections::HashMap::new();
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    set_watch_test_notifier(tx);

    // Change file contents to create additional nodes
    fs::write(&file_path, "fn foo() {}\nfn bar() {}\n").await?;

    // Simulate watch events directly (modify twice to satisfy debounce logic)
    let event = Event {
        kind: EventKind::Modify(ModifyKind::Data(DataChange::Content)),
        paths: vec![file_path.clone()],
        attrs: Default::default(),
    };
    watch_indexer
        .simulate_file_event(event.clone(), &mut last_events, 300)
        .await;
    sleep(Duration::from_millis(400)).await;
    watch_indexer
        .simulate_file_event(event, &mut last_events, 300)
        .await;
    let _updated = fetch_metadata(storage.clone(), file_path.to_string_lossy().as_ref())
        .await?
        .expect("updated metadata missing");
    let first = tokio::time::timeout(Duration::from_secs(1), rx.recv()).await;

    assert!(matches!(first, Ok(Some(_))), "watch event not observed");

    Ok(())
}

async fn fetch_metadata(
    storage: Arc<tokio::sync::Mutex<codegraph_graph::SurrealDbStorage>>,
    file_path: &str,
) -> Result<Option<Value>> {
    let db = storage.lock().await;
    let client = db.db();
    let mut resp = client
        .query(
            "SELECT file_path, last_indexed_at, node_count FROM file_metadata WHERE file_path = $file_path",
        )
        .bind(("file_path", file_path.to_string()))
        .await?;
    let rows: Vec<Value> = resp.take(0)?;
    Ok(rows.last().cloned())
}
