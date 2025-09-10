use async_graphql::SimpleBroker;
use chrono::Utc;
use std::sync::atomic::{AtomicU64, Ordering};
use parking_lot::RwLock;
use std::collections::VecDeque;

use crate::subscriptions::{GraphUpdateEvent, GraphUpdateType, IndexingProgressEvent};

static GRAPH_UPDATE_SEQ: AtomicU64 = AtomicU64::new(0);
static INDEXING_PROGRESS_SEQ: AtomicU64 = AtomicU64::new(0);

const BUFFER_CAPACITY: usize = 1024;
static GRAPH_UPDATE_BUFFER: RwLock<VecDeque<GraphUpdateEvent>> = RwLock::new(VecDeque::new());
static INDEXING_BUFFER: RwLock<VecDeque<IndexingProgressEvent>> = RwLock::new(VecDeque::new());

pub fn next_graph_update_seq() -> u64 {
    GRAPH_UPDATE_SEQ.fetch_add(1, Ordering::Relaxed) + 1
}

pub fn next_indexing_seq() -> u64 {
    INDEXING_PROGRESS_SEQ.fetch_add(1, Ordering::Relaxed) + 1
}

pub fn publish_graph_update(
    update_type: GraphUpdateType,
    affected_nodes: Vec<String>,
    affected_relations: Vec<String>,
    change_count: i32,
    details: Option<String>,
) {
    let event = GraphUpdateEvent {
        seq: next_graph_update_seq(),
        update_type,
        affected_nodes,
        affected_relations,
        change_count,
        timestamp: Utc::now(),
        details,
    };
    // Store in ring buffer for reconnection catch-up
    {
        let mut buf = GRAPH_UPDATE_BUFFER.write();
        if buf.len() >= BUFFER_CAPACITY { buf.pop_front(); }
        buf.push_back(event.clone());
    }
    SimpleBroker::publish(event);
}

pub fn publish_indexing_progress(
    job_id: String,
    progress: f32,
    current_stage: String,
    estimated_time_remaining_secs: Option<f32>,
    message: Option<String>,
) {
    let event = IndexingProgressEvent {
        seq: next_indexing_seq(),
        job_id,
        progress,
        current_stage,
        estimated_time_remaining_secs,
        message,
        timestamp: Utc::now(),
    };
    {
        let mut buf = INDEXING_BUFFER.write();
        if buf.len() >= BUFFER_CAPACITY { buf.pop_front(); }
        buf.push_back(event.clone());
    }
    SimpleBroker::publish(event);
}

pub fn recent_graph_updates_since(seq: u64, limit: usize) -> Vec<GraphUpdateEvent> {
    let buf = GRAPH_UPDATE_BUFFER.read();
    buf.iter()
        .filter(|e| e.seq > seq)
        .take(limit)
        .cloned()
        .collect()
}

pub fn recent_indexing_progress_since(seq: u64, limit: usize) -> Vec<IndexingProgressEvent> {
    let buf = INDEXING_BUFFER.read();
    buf.iter()
        .filter(|e| e.seq > seq)
        .take(limit)
        .cloned()
        .collect()
}
