#![allow(dead_code, unused_variables, unused_imports)]

use crate::{
    DeltaProcessorImpl, FileWatcherImpl, GraphUpdaterImpl, ProgressTrackerImpl, UpdateSchedulerImpl,
};
use codegraph_core::traits::{
    DeltaProcessor, FileWatcher, GraphUpdater, ProgressTracker, UpdateScheduler,
};
use crossbeam_channel::unbounded;
use std::thread;

pub async fn run_pipeline(path: &str) -> codegraph_core::Result<()> {
    let (change_tx, change_rx) = unbounded();
    let (update_tx, update_rx) = unbounded();
    let (delta_tx, delta_rx) = unbounded();

    let file_watcher = FileWatcherImpl::new(path);
    let update_scheduler = UpdateSchedulerImpl;
    let delta_processor = DeltaProcessorImpl;
    let graph_updater = GraphUpdaterImpl;
    let progress_tracker = ProgressTrackerImpl;

    let path_clone = path.to_string();
    let watcher_thread = thread::spawn(move || {
        file_watcher.watch(change_tx).unwrap();
    });

    let scheduler_thread = tokio::spawn(async move {
        update_scheduler
            .schedule(change_rx, update_tx)
            .await
            .unwrap();
    });

    let processor_thread = tokio::spawn(async move {
        delta_processor.process(update_rx, delta_tx).await.unwrap();
    });

    let updater_thread = tokio::spawn(async move {
        graph_updater.update(delta_rx).await.unwrap();
    });

    let tracker_thread = tokio::spawn(async move {
        progress_tracker.track().await.unwrap();
    });

    watcher_thread.join().unwrap();
    scheduler_thread
        .await
        .map_err(|e| codegraph_core::CodeGraphError::Threading(e.to_string()))?;
    processor_thread
        .await
        .map_err(|e| codegraph_core::CodeGraphError::Threading(e.to_string()))?;
    updater_thread
        .await
        .map_err(|e| codegraph_core::CodeGraphError::Threading(e.to_string()))?;
    tracker_thread
        .await
        .map_err(|e| codegraph_core::CodeGraphError::Threading(e.to_string()))?;

    Ok(())
}
