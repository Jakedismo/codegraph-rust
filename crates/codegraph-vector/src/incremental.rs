use codegraph_core::{CodeGraphError, NodeId, Result};
use crossbeam_channel::{unbounded, Receiver, Sender};
use dashmap::DashMap;
use parking_lot::{Mutex, RwLock};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};
use std::io::Write;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::mpsc as tokio_mpsc;
use tokio::time::interval;
use tracing::{debug, error, info, warn};

/// Incremental update operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum IncrementalOperation {
    /// Insert new vector
    Insert {
        node_id: NodeId,
        vector: Vec<f32>,
        timestamp: u64,
    },
    /// Update existing vector
    Update {
        node_id: NodeId,
        old_vector: Option<Vec<f32>>,
        new_vector: Vec<f32>,
        timestamp: u64,
    },
    /// Delete vector
    Delete { node_id: NodeId, timestamp: u64 },
    /// Batch operation containing multiple updates
    Batch {
        operations: Vec<IncrementalOperation>,
        timestamp: u64,
    },
}

impl IncrementalOperation {
    pub fn timestamp(&self) -> u64 {
        match self {
            Self::Insert { timestamp, .. }
            | Self::Update { timestamp, .. }
            | Self::Delete { timestamp, .. }
            | Self::Batch { timestamp, .. } => *timestamp,
        }
    }

    pub fn affected_nodes(&self) -> HashSet<NodeId> {
        match self {
            Self::Insert { node_id, .. }
            | Self::Update { node_id, .. }
            | Self::Delete { node_id, .. } => {
                let mut set = HashSet::new();
                set.insert(*node_id);
                set
            }
            Self::Batch { operations, .. } => operations
                .iter()
                .flat_map(|op| op.affected_nodes())
                .collect(),
        }
    }
}

/// Update batch for efficient processing
#[derive(Debug, Clone)]
pub struct UpdateBatch {
    pub operations: Vec<IncrementalOperation>,
    pub created_at: SystemTime,
    pub priority: BatchPriority,
    pub estimated_cost: f64,
}

#[derive(Debug, Clone, PartialEq, PartialOrd)]
pub enum BatchPriority {
    Low = 0,
    Normal = 1,
    High = 2,
    Critical = 3,
}

/// Index segment for incremental updates
#[derive(Debug, Clone)]
pub struct IndexSegment {
    pub segment_id: u64,
    pub node_ids: HashSet<NodeId>,
    pub vectors: HashMap<NodeId, Vec<f32>>,
    pub created_at: SystemTime,
    pub last_modified: SystemTime,
    pub size_bytes: usize,
    pub is_sealed: bool,
}

impl IndexSegment {
    pub fn new(segment_id: u64) -> Self {
        let now = SystemTime::now();
        Self {
            segment_id,
            node_ids: HashSet::new(),
            vectors: HashMap::new(),
            created_at: now,
            last_modified: now,
            size_bytes: 0,
            is_sealed: false,
        }
    }

    pub fn add_vector(&mut self, node_id: NodeId, vector: Vec<f32>) -> bool {
        if self.is_sealed {
            return false;
        }

        let vector_size = vector.len() * std::mem::size_of::<f32>();
        self.node_ids.insert(node_id);
        self.vectors.insert(node_id, vector);
        self.size_bytes += vector_size + std::mem::size_of::<NodeId>();
        self.last_modified = SystemTime::now();
        true
    }

    pub fn remove_vector(&mut self, node_id: NodeId) -> bool {
        if self.is_sealed {
            return false;
        }

        if let Some(vector) = self.vectors.remove(&node_id) {
            self.node_ids.remove(&node_id);
            let vector_size = vector.len() * std::mem::size_of::<f32>();
            self.size_bytes = self
                .size_bytes
                .saturating_sub(vector_size + std::mem::size_of::<NodeId>());
            self.last_modified = SystemTime::now();
            true
        } else {
            false
        }
    }

    pub fn seal(&mut self) {
        self.is_sealed = true;
        self.last_modified = SystemTime::now();
    }

    pub fn vector_count(&self) -> usize {
        self.vectors.len()
    }

    pub fn contains(&self, node_id: NodeId) -> bool {
        self.node_ids.contains(&node_id)
    }
}

/// Configuration for incremental updates
#[derive(Debug, Clone)]
pub struct IncrementalConfig {
    /// Maximum number of operations in a batch
    pub max_batch_size: usize,
    /// Maximum time to wait for batching operations
    pub batch_timeout: Duration,
    /// Maximum size of a segment before sealing
    pub max_segment_size: usize,
    /// Maximum age of a segment before forced sealing
    pub max_segment_age: Duration,
    /// Number of background worker threads
    pub worker_threads: usize,
    /// Whether to use parallel processing for large batches
    pub enable_parallel_processing: bool,
    /// Minimum operations for parallel processing
    pub parallel_threshold: usize,
    /// Enable write-ahead logging
    pub enable_wal: bool,
    /// WAL flush interval
    pub wal_flush_interval: Duration,
}

impl Default for IncrementalConfig {
    fn default() -> Self {
        Self {
            max_batch_size: 1000,
            batch_timeout: Duration::from_millis(100),
            max_segment_size: 10_000_000,              // 10MB
            max_segment_age: Duration::from_secs(300), // 5 minutes
            worker_threads: std::thread::available_parallelism()
                .map(|n| n.get())
                .unwrap_or(4),
            enable_parallel_processing: true,
            parallel_threshold: 100,
            enable_wal: true,
            wal_flush_interval: Duration::from_millis(50),
        }
    }
}

/// Statistics for incremental updates
#[derive(Debug, Clone, Default)]
pub struct IncrementalStats {
    pub total_operations: u64,
    pub successful_operations: u64,
    pub failed_operations: u64,
    pub batches_processed: u64,
    pub segments_created: u64,
    pub segments_merged: u64,
    pub average_batch_size: f64,
    pub average_processing_time_ms: f64,
    pub last_update_timestamp: u64,
    pub pending_operations: usize,
    pub active_segments: usize,
}

/// Write-Ahead Log for durability
#[derive(Debug)]
struct WriteAheadLog {
    _log_path: std::path::PathBuf,
    _log_file: Arc<Mutex<std::fs::File>>,
    pending_entries: Arc<Mutex<VecDeque<WALEntry>>>,
    flush_sender: tokio_mpsc::UnboundedSender<()>,
    _flush_task: tokio::task::JoinHandle<()>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct WALEntry {
    sequence_number: u64,
    operation: IncrementalOperation,
    timestamp: u64,
    checksum: u64,
}

impl WriteAheadLog {
    fn new<P: AsRef<std::path::Path>>(log_path: P, flush_interval: Duration) -> Result<Self> {
        let log_path = log_path.as_ref().to_path_buf();

        // Ensure parent directory exists
        if let Some(parent) = log_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let log_file = Arc::new(Mutex::new(
            std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&log_path)?,
        ));

        let pending_entries = Arc::new(Mutex::new(VecDeque::new()));
        let (flush_sender, mut flush_receiver) = tokio_mpsc::unbounded_channel();

        // Start flush task
        let flush_task = {
            let log_file = Arc::clone(&log_file);
            let pending_entries = Arc::clone(&pending_entries);

            tokio::spawn(async move {
                let mut interval = interval(flush_interval);

                loop {
                    tokio::select! {
                        _ = interval.tick() => {
                            if let Err(e) = Self::flush_pending(&log_file, &pending_entries) {
                                error!("Failed to flush WAL: {}", e);
                            }
                        }
                        msg = flush_receiver.recv() => {
                            if msg.is_none() {
                                break; // Channel closed
                            }
                            if let Err(e) = Self::flush_pending(&log_file, &pending_entries) {
                                error!("Failed to flush WAL: {}", e);
                            }
                        }
                    }
                }
            })
        };

        Ok(Self {
            _log_path: log_path,
            _log_file: log_file,
            pending_entries,
            flush_sender,
            _flush_task: flush_task,
        })
    }

    fn append(&self, operation: IncrementalOperation) -> Result<()> {
        let entry = WALEntry {
            sequence_number: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos() as u64,
            timestamp: operation.timestamp(),
            checksum: self.calculate_checksum(&operation),
            operation,
        };

        {
            let mut pending = self.pending_entries.lock();
            pending.push_back(entry);

            // Trigger flush if too many pending entries
            if pending.len() >= 100 {
                drop(pending);
                let _ = self.flush_sender.send(());
            }
        }

        Ok(())
    }

    fn flush_pending(
        log_file: &Arc<Mutex<std::fs::File>>,
        pending_entries: &Arc<Mutex<VecDeque<WALEntry>>>,
    ) -> Result<()> {
        let entries_to_flush = {
            let mut pending = pending_entries.lock();
            if pending.is_empty() {
                return Ok(());
            }
            pending.drain(..).collect::<Vec<_>>()
        };

        if entries_to_flush.is_empty() {
            return Ok(());
        }

        let mut file = log_file.lock();
        for entry in entries_to_flush {
            let serialized =
                bincode::serde::encode_to_vec(&entry, bincode::config::standard()).map_err(|e: bincode::error::EncodeError| CodeGraphError::Vector(e.to_string()))?;

            use std::io::Write;
            file.write_all(&(serialized.len() as u32).to_le_bytes())?;
            file.write_all(&serialized)?;
        }

        file.flush()?;
        debug!("Flushed WAL entries");
        Ok(())
    }

    fn calculate_checksum(&self, operation: &IncrementalOperation) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();

        match operation {
            IncrementalOperation::Insert {
                node_id,
                vector,
                timestamp,
            } => {
                0u8.hash(&mut hasher);
                node_id.hash(&mut hasher);
                timestamp.hash(&mut hasher);
                for &val in vector {
                    val.to_bits().hash(&mut hasher);
                }
            }
            IncrementalOperation::Update {
                node_id,
                old_vector,
                new_vector,
                timestamp,
            } => {
                1u8.hash(&mut hasher);
                node_id.hash(&mut hasher);
                timestamp.hash(&mut hasher);
                if let Some(old) = old_vector {
                    for &val in old {
                        val.to_bits().hash(&mut hasher);
                    }
                }
                for &val in new_vector {
                    val.to_bits().hash(&mut hasher);
                }
            }
            IncrementalOperation::Delete { node_id, timestamp } => {
                2u8.hash(&mut hasher);
                node_id.hash(&mut hasher);
                timestamp.hash(&mut hasher);
            }
            IncrementalOperation::Batch {
                operations,
                timestamp,
            } => {
                3u8.hash(&mut hasher);
                timestamp.hash(&mut hasher);
                operations.len().hash(&mut hasher);
                for op in operations {
                    self.calculate_checksum(op).hash(&mut hasher);
                }
            }
        }

        hasher.finish()
    }
}

/// Incremental update manager
pub struct IncrementalUpdateManager {
    config: IncrementalConfig,
    segments: Arc<DashMap<u64, Arc<RwLock<IndexSegment>>>>,
    next_segment_id: Arc<RwLock<u64>>,
    _current_segment: Arc<RwLock<Option<u64>>>,
    operation_sender: Sender<IncrementalOperation>,
    _operation_receiver: Arc<Mutex<Receiver<IncrementalOperation>>>,
    stats: Arc<RwLock<IncrementalStats>>,
    wal: Option<WriteAheadLog>,
    _worker_handles: Vec<tokio::task::JoinHandle<()>>,
}

impl IncrementalUpdateManager {
    pub fn new(config: IncrementalConfig) -> Result<Self> {
        let (operation_sender, operation_receiver) = unbounded();

        let wal = if config.enable_wal {
            Some(WriteAheadLog::new(
                "/tmp/codegraph_incremental.wal",
                config.wal_flush_interval,
            )?)
        } else {
            None
        };

        let segments = Arc::new(DashMap::new());
        let next_segment_id = Arc::new(RwLock::new(0));
        let current_segment = Arc::new(RwLock::new(None));
        let stats = Arc::new(RwLock::new(IncrementalStats::default()));
        let operation_receiver = Arc::new(Mutex::new(operation_receiver));

        // Start worker threads
        let mut worker_handles = Vec::new();
        for worker_id in 0..config.worker_threads {
            let handle = Self::start_worker(
                worker_id,
                Arc::clone(&segments),
                Arc::clone(&next_segment_id),
                Arc::clone(&current_segment),
                Arc::clone(&operation_receiver),
                Arc::clone(&stats),
                config.clone(),
            );
            worker_handles.push(handle);
        }

        Ok(Self {
            config,
            segments,
            next_segment_id,
            _current_segment: current_segment,
            operation_sender,
            _operation_receiver: operation_receiver,
            stats,
            wal,
            _worker_handles: worker_handles,
        })
    }

    fn start_worker(
        worker_id: usize,
        segments: Arc<DashMap<u64, Arc<RwLock<IndexSegment>>>>,
        next_segment_id: Arc<RwLock<u64>>,
        current_segment: Arc<RwLock<Option<u64>>>,
        operation_receiver: Arc<Mutex<Receiver<IncrementalOperation>>>,
        stats: Arc<RwLock<IncrementalStats>>,
        config: IncrementalConfig,
    ) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            debug!("Starting incremental update worker {}", worker_id);

            let mut batch = Vec::new();
            let mut last_batch_time = SystemTime::now();

            loop {
                // Try to receive operations with timeout
                let operation = {
                    let receiver = operation_receiver.lock();
                    receiver.recv_timeout(config.batch_timeout)
                };

                match operation {
                    Ok(op) => {
                        batch.push(op);

                        // Check if we should process the batch
                        let should_process = batch.len() >= config.max_batch_size
                            || last_batch_time.elapsed().unwrap_or_default()
                                >= config.batch_timeout;

                        if should_process && !batch.is_empty() {
                            Self::process_batch(
                                &segments,
                                &next_segment_id,
                                &current_segment,
                                &stats,
                                &config,
                                std::mem::take(&mut batch),
                            )
                            .await;
                            last_batch_time = SystemTime::now();
                        }
                    }
                    Err(_) => {
                        // Timeout - process any pending operations
                        if !batch.is_empty() {
                            Self::process_batch(
                                &segments,
                                &next_segment_id,
                                &current_segment,
                                &stats,
                                &config,
                                std::mem::take(&mut batch),
                            )
                            .await;
                            last_batch_time = SystemTime::now();
                        }
                    }
                }
            }
        })
    }

    async fn process_batch(
        segments: &Arc<DashMap<u64, Arc<RwLock<IndexSegment>>>>,
        next_segment_id: &Arc<RwLock<u64>>,
        current_segment: &Arc<RwLock<Option<u64>>>,
        stats: &Arc<RwLock<IncrementalStats>>,
        config: &IncrementalConfig,
        operations: Vec<IncrementalOperation>,
    ) {
        if operations.is_empty() {
            return;
        }

        let start_time = SystemTime::now();
        let batch_size = operations.len();

        debug!("Processing batch of {} operations", batch_size);

        // Process operations in parallel if enabled and batch is large enough
        let results =
            if config.enable_parallel_processing && operations.len() >= config.parallel_threshold {
                operations
                    .into_par_iter()
                    .map(|op| {
                        Self::process_single_operation(
                            segments,
                            next_segment_id,
                            current_segment,
                            config,
                            op,
                        )
                    })
                    .collect::<Vec<_>>()
            } else {
                operations
                    .into_iter()
                    .map(|op| {
                        Self::process_single_operation(
                            segments,
                            next_segment_id,
                            current_segment,
                            config,
                            op,
                        )
                    })
                    .collect::<Vec<_>>()
            };

        // Update statistics
        {
            let mut stats_guard = stats.write();
            stats_guard.batches_processed += 1;
            stats_guard.total_operations += batch_size as u64;

            let successful = results.iter().filter(|r| r.is_ok()).count() as u64;
            let failed = results.len() as u64 - successful;

            stats_guard.successful_operations += successful;
            stats_guard.failed_operations += failed;

            // Update averages
            let total_batches = stats_guard.batches_processed as f64;
            stats_guard.average_batch_size =
                (stats_guard.average_batch_size * (total_batches - 1.0) + batch_size as f64)
                    / total_batches;

            let processing_time = start_time.elapsed().unwrap_or_default().as_millis() as f64;
            stats_guard.average_processing_time_ms =
                (stats_guard.average_processing_time_ms * (total_batches - 1.0) + processing_time)
                    / total_batches;

            stats_guard.last_update_timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs();

            stats_guard.active_segments = segments.len();
        }

        debug!("Completed batch processing in {:?}", start_time.elapsed());
    }

    fn process_single_operation(
        segments: &Arc<DashMap<u64, Arc<RwLock<IndexSegment>>>>,
        next_segment_id: &Arc<RwLock<u64>>,
        current_segment: &Arc<RwLock<Option<u64>>>,
        config: &IncrementalConfig,
        operation: IncrementalOperation,
    ) -> Result<()> {
        match operation {
            IncrementalOperation::Insert {
                node_id, vector, ..
            } => Self::handle_insert(
                segments,
                next_segment_id,
                current_segment,
                config,
                node_id,
                vector,
            ),
            IncrementalOperation::Update {
                node_id,
                new_vector,
                ..
            } => Self::handle_update(
                segments,
                next_segment_id,
                current_segment,
                config,
                node_id,
                new_vector,
            ),
            IncrementalOperation::Delete { node_id, .. } => Self::handle_delete(segments, node_id),
            IncrementalOperation::Batch { operations, .. } => {
                for op in operations {
                    Self::process_single_operation(
                        segments,
                        next_segment_id,
                        current_segment,
                        config,
                        op,
                    )?;
                }
                Ok(())
            }
        }
    }

    fn handle_insert(
        segments: &Arc<DashMap<u64, Arc<RwLock<IndexSegment>>>>,
        next_segment_id: &Arc<RwLock<u64>>,
        current_segment: &Arc<RwLock<Option<u64>>>,
        config: &IncrementalConfig,
        node_id: NodeId,
        vector: Vec<f32>,
    ) -> Result<()> {
        // Get or create current segment
        let segment_id = {
            let mut current = current_segment.write();

            if let Some(current_id) = *current {
                // Check if current segment has space
                if let Some(segment_ref) = segments.get(&current_id) {
                    let segment = segment_ref.read();
                    if !segment.is_sealed
                        && segment.size_bytes + vector.len() * std::mem::size_of::<f32>()
                            < config.max_segment_size
                    {
                        current_id
                    } else {
                        // Current segment is full, create new one
                        let new_id = {
                            let mut next_id = next_segment_id.write();
                            let id = *next_id;
                            *next_id += 1;
                            id
                        };

                        // Seal the current segment
                        if let Some(segment_ref) = segments.get(&current_id) {
                            segment_ref.write().seal();
                        }

                        *current = Some(new_id);
                        segments.insert(new_id, Arc::new(RwLock::new(IndexSegment::new(new_id))));
                        new_id
                    }
                } else {
                    // Current segment doesn't exist, create new one
                    let new_id = {
                        let mut next_id = next_segment_id.write();
                        let id = *next_id;
                        *next_id += 1;
                        id
                    };

                    *current = Some(new_id);
                    segments.insert(new_id, Arc::new(RwLock::new(IndexSegment::new(new_id))));
                    new_id
                }
            } else {
                // No current segment, create first one
                let new_id = {
                    let mut next_id = next_segment_id.write();
                    let id = *next_id;
                    *next_id += 1;
                    id
                };

                *current = Some(new_id);
                segments.insert(new_id, Arc::new(RwLock::new(IndexSegment::new(new_id))));
                new_id
            }
        };

        // Add vector to segment
        if let Some(segment_ref) = segments.get(&segment_id) {
            let mut segment = segment_ref.write();
            if !segment.add_vector(node_id, vector) {
                return Err(CodeGraphError::Vector(
                    "Failed to add vector to segment".to_string(),
                ));
            }
        }

        Ok(())
    }

    fn handle_update(
        segments: &Arc<DashMap<u64, Arc<RwLock<IndexSegment>>>>,
        next_segment_id: &Arc<RwLock<u64>>,
        current_segment: &Arc<RwLock<Option<u64>>>,
        config: &IncrementalConfig,
        node_id: NodeId,
        new_vector: Vec<f32>,
    ) -> Result<()> {
        // First try to update in existing segments
        for segment_ref in segments.iter() {
            let mut segment = segment_ref.write();
            if segment.contains(node_id) {
                segment.remove_vector(node_id);
                if !segment.is_sealed {
                    if segment.add_vector(node_id, new_vector.clone()) {
                        return Ok(());
                    }
                }
                break;
            }
        }

        // If not found in existing segments, treat as insert
        Self::handle_insert(
            segments,
            next_segment_id,
            current_segment,
            config,
            node_id,
            new_vector,
        )
    }

    fn handle_delete(
        segments: &Arc<DashMap<u64, Arc<RwLock<IndexSegment>>>>,
        node_id: NodeId,
    ) -> Result<()> {
        // Remove from all segments
        for segment_ref in segments.iter() {
            let mut segment = segment_ref.write();
            segment.remove_vector(node_id);
        }
        Ok(())
    }

    /// Submit an operation for incremental processing
    pub fn submit_operation(&self, operation: IncrementalOperation) -> Result<()> {
        // Log to WAL if enabled
        if let Some(ref wal) = self.wal {
            wal.append(operation.clone())?;
        }

        // Send to worker queue
        self.operation_sender
            .send(operation)
            .map_err(|e| CodeGraphError::Vector(format!("Failed to submit operation: {}", e)))?;

        // Update pending count
        {
            let mut stats = self.stats.write();
            stats.pending_operations += 1;
        }

        Ok(())
    }

    /// Submit multiple operations as a batch
    pub fn submit_batch(&self, operations: Vec<IncrementalOperation>) -> Result<()> {
        if operations.is_empty() {
            return Ok(());
        }

        let batch_op = IncrementalOperation::Batch {
            operations,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        };

        self.submit_operation(batch_op)
    }

    /// Get current statistics
    pub fn get_stats(&self) -> IncrementalStats {
        self.stats.read().clone()
    }

    /// Get all segments (for testing/debugging)
    pub fn get_segments(&self) -> Vec<Arc<RwLock<IndexSegment>>> {
        self.segments
            .iter()
            .map(|entry| Arc::clone(entry.value()))
            .collect()
    }

    /// Merge small segments to optimize storage
    pub async fn merge_segments(&self, max_segments_to_merge: usize) -> Result<usize> {
        let segments_to_merge: Vec<_> = self
            .segments
            .iter()
            .filter(|entry| {
                let segment = entry.value().read();
                segment.is_sealed && segment.vector_count() < 1000
            })
            .take(max_segments_to_merge)
            .map(|entry| (*entry.key(), Arc::clone(entry.value())))
            .collect();

        if segments_to_merge.len() < 2 {
            return Ok(0);
        }

        info!("Merging {} small segments", segments_to_merge.len());

        // Create new merged segment
        let merged_id = {
            let mut next_id = self.next_segment_id.write();
            let id = *next_id;
            *next_id += 1;
            id
        };

        let mut merged_segment = IndexSegment::new(merged_id);

        // Collect all vectors from segments to merge
        for (_, segment_ref) in &segments_to_merge {
            let segment = segment_ref.read();
            for (node_id, vector) in &segment.vectors {
                merged_segment.add_vector(*node_id, vector.clone());
            }
        }

        // Seal the merged segment
        merged_segment.seal();

        // Insert merged segment and remove old ones
        self.segments
            .insert(merged_id, Arc::new(RwLock::new(merged_segment)));

        for (segment_id, _) in segments_to_merge {
            self.segments.remove(&segment_id);
        }

        // Update statistics
        {
            let mut stats = self.stats.write();
            stats.segments_merged += 1;
        }

        info!("Merged segments into segment {}", merged_id);
        Ok(1)
    }

    /// Force flush all pending operations
    pub async fn flush(&self) -> Result<()> {
        // Send a special flush operation to ensure all workers process pending operations
        let flush_op = IncrementalOperation::Batch {
            operations: vec![],
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        };

        for _ in 0..self.config.worker_threads {
            self.operation_sender
                .send(flush_op.clone())
                .map_err(|e| CodeGraphError::Vector(format!("Failed to send flush: {}", e)))?;
        }

        // Wait a bit for processing
        tokio::time::sleep(self.config.batch_timeout * 2).await;

        Ok(())
    }
}

impl Drop for IncrementalUpdateManager {
    fn drop(&mut self) {
        // Attempt to flush any remaining operations
        let handle = tokio::runtime::Handle::try_current();
        if let Ok(handle) = handle {
            handle.spawn(async move {
                // Note: This is a simplified drop - in production you'd want proper cleanup
                warn!("IncrementalUpdateManager dropped - some operations may be pending");
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;


    #[tokio::test]
    async fn test_incremental_operations() {
        let config = IncrementalConfig {
            max_batch_size: 10,
            batch_timeout: Duration::from_millis(50),
            ..Default::default()
        };

        let manager = IncrementalUpdateManager::new(config).unwrap();

        // Test insert operations
        let insert_ops = (0..5)
            .map(|i| {
                let nid = NodeId::new_v4();
                IncrementalOperation::Insert {
                    node_id: nid,
                    vector: vec![i as f32; 128],
                    timestamp: SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap()
                        .as_secs(),
                }
            })
            .collect();

        manager.submit_batch(insert_ops).unwrap();

        // Wait for processing
        tokio::time::sleep(Duration::from_millis(100)).await;

        let stats = manager.get_stats();
        assert!(stats.successful_operations > 0);
        assert!(stats.batches_processed > 0);
    }

    #[tokio::test]
    async fn test_segment_management() {
        let config = IncrementalConfig {
            max_segment_size: 1000, // Small segments for testing
            ..Default::default()
        };

        let manager = IncrementalUpdateManager::new(config).unwrap();

        // Add enough vectors to create multiple segments
        for _i in 0..50 {
            let nid = NodeId::new_v4();
            let op = IncrementalOperation::Insert {
                node_id: nid,
                vector: vec![1.0; 100], // Relatively large vectors
                timestamp: SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs(),
            };
            manager.submit_operation(op).unwrap();
        }

        // Wait for processing
        tokio::time::sleep(Duration::from_millis(200)).await;

        let segments = manager.get_segments();
        assert!(segments.len() > 1, "Should create multiple segments");
    }

    #[tokio::test]
    async fn test_segment_merging() {
        let config = IncrementalConfig::default();
        let manager = IncrementalUpdateManager::new(config).unwrap();

        // Create several small segments manually for testing
        for segment_id in 0..5 {
            let mut segment = IndexSegment::new(segment_id);
            for _ in (segment_id * 10)..(segment_id * 10 + 5) {
                let nid = NodeId::new_v4();
                segment.add_vector(nid, vec![1.0; 10]);
            }
            segment.seal();
            manager
                .segments
                .insert(segment_id, Arc::new(RwLock::new(segment)));
        }

        let segments_before = manager.segments.len();
        let merged_count = manager.merge_segments(3).await.unwrap();
        let segments_after = manager.segments.len();

        assert!(merged_count > 0, "Should merge segments");
        assert!(
            segments_after < segments_before,
            "Should reduce segment count"
        );
    }
}
