use crate::index::{FaissIndexManager, IndexConfig};
use crate::storage::PersistentStorage;
use codegraph_core::{CodeGraphError, CodeNode, NodeId, Result};
use crossbeam_channel::{bounded, Receiver, Sender};
use dashmap::DashMap;
use parking_lot::RwLock;
use rayon::prelude::*;
use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::time;
use tracing::{debug, info, warn, error};
use uuid::Uuid;

/// Configuration for batch processing operations
#[derive(Debug, Clone)]
pub struct BatchConfig {
    pub batch_size: usize,
    pub max_pending_batches: usize,
    pub flush_interval: Duration,
    pub parallel_processing: bool,
    pub memory_limit_mb: usize,
    pub auto_train_threshold: usize,
}

impl Default for BatchConfig {
    fn default() -> Self {
        Self {
            batch_size: 1000,
            max_pending_batches: 10,
            flush_interval: Duration::from_secs(30),
            parallel_processing: true,
            memory_limit_mb: 1024, // 1GB
            auto_train_threshold: 10000,
        }
    }
}

/// Batch operation types for efficient processing
#[derive(Debug, Clone)]
pub enum BatchOperation {
    Insert {
        node_id: NodeId,
        embedding: Vec<f32>,
    },
    Update {
        node_id: NodeId,
        embedding: Vec<f32>,
    },
    Delete {
        node_id: NodeId,
    },
    Search {
        query_embedding: Vec<f32>,
        k: usize,
        callback_id: Uuid,
    },
}

/// Result of a batch operation
#[derive(Debug, Clone)]
pub enum BatchResult {
    Inserted { node_id: NodeId, faiss_id: i64 },
    Updated { node_id: NodeId, old_faiss_id: i64, new_faiss_id: i64 },
    Deleted { node_id: NodeId },
    SearchComplete { callback_id: Uuid, results: Vec<(NodeId, f32)> },
    Error { operation_id: Uuid, error: String },
}

/// High-performance batch processor for vector operations
pub struct BatchProcessor {
    config: BatchConfig,
    index_manager: Arc<RwLock<FaissIndexManager>>,
    storage: Option<Arc<PersistentStorage>>,
    
    // Operation queuing
    operation_queue: Arc<RwLock<VecDeque<BatchOperation>>>,
    pending_operations: Arc<AtomicUsize>,
    
    // ID mappings
    id_mapping: Arc<DashMap<i64, NodeId>>,
    reverse_mapping: Arc<DashMap<NodeId, i64>>,
    
    // Batch processing state
    processing_active: Arc<AtomicBool>,
    last_flush: Arc<RwLock<Instant>>,
    
    // Communication channels
    result_sender: Sender<BatchResult>,
    result_receiver: Receiver<BatchResult>,
    
    // Statistics
    total_operations: Arc<AtomicUsize>,
    successful_operations: Arc<AtomicUsize>,
    failed_operations: Arc<AtomicUsize>,
}

impl BatchProcessor {
    pub fn new(
        config: BatchConfig,
        index_config: IndexConfig,
        storage_path: Option<std::path::PathBuf>,
    ) -> Result<Self> {
        let mut index_manager = FaissIndexManager::new(index_config);
        
        let storage = if let Some(path) = storage_path {
            let storage = Arc::new(PersistentStorage::new(path)?);
            index_manager = index_manager.with_persistence(storage.base_path.clone())?;
            Some(storage)
        } else {
            None
        };

        // Initialize GPU if configured
        index_manager.init_gpu()?;

        let (result_sender, result_receiver) = bounded(config.max_pending_batches * config.batch_size);

        let processor = Self {
            config,
            index_manager: Arc::new(RwLock::new(index_manager)),
            storage,
            operation_queue: Arc::new(RwLock::new(VecDeque::new())),
            pending_operations: Arc::new(AtomicUsize::new(0)),
            id_mapping: Arc::new(DashMap::new()),
            reverse_mapping: Arc::new(DashMap::new()),
            processing_active: Arc::new(AtomicBool::new(false)),
            last_flush: Arc::new(RwLock::new(Instant::now())),
            result_sender,
            result_receiver,
            total_operations: Arc::new(AtomicUsize::new(0)),
            successful_operations: Arc::new(AtomicUsize::new(0)),
            failed_operations: Arc::new(AtomicUsize::new(0)),
        };

        // Load existing mappings if available
        if let Some(ref storage) = processor.storage {
            let (id_mapping, reverse_mapping) = storage.load_id_mapping()?;
            for (faiss_id, node_id) in id_mapping {
                processor.id_mapping.insert(faiss_id, node_id);
            }
            for (node_id, faiss_id) in reverse_mapping {
                processor.reverse_mapping.insert(node_id, faiss_id);
            }
        }

        Ok(processor)
    }

    /// Start the batch processing background task
    pub async fn start_processing(&self) -> Result<()> {
        if self.processing_active.load(Ordering::Acquire) {
            return Ok(());
        }

        self.processing_active.store(true, Ordering::Release);
        info!("Starting batch processor with batch size: {}", self.config.batch_size);

        let processor_clone = self.clone_for_background();
        tokio::spawn(async move {
            processor_clone.background_processing_loop().await;
        });

        Ok(())
    }

    /// Stop the batch processing
    pub async fn stop_processing(&self) -> Result<()> {
        info!("Stopping batch processor");
        self.processing_active.store(false, Ordering::Release);
        
        // Flush remaining operations
        self.flush_pending_operations().await?;
        
        Ok(())
    }

    /// Add an operation to the batch queue
    pub async fn enqueue_operation(&self, operation: BatchOperation) -> Result<()> {
        let pending = self.pending_operations.load(Ordering::Acquire);
        if pending >= self.config.max_pending_batches * self.config.batch_size {
            return Err(CodeGraphError::ResourceExhausted(
                "Batch queue is full, too many pending operations".to_string()
            ));
        }

        {
            let mut queue = self.operation_queue.write();
            queue.push_back(operation);
        }
        
        self.pending_operations.fetch_add(1, Ordering::AcqRel);
        self.total_operations.fetch_add(1, Ordering::AcqRel);

        // Trigger immediate processing if batch is full
        if self.pending_operations.load(Ordering::Acquire) >= self.config.batch_size {
            self.process_batch().await?;
        }

        Ok(())
    }

    /// Process a batch of operations efficiently
    async fn process_batch(&self) -> Result<()> {
        let batch_size = self.config.batch_size.min(self.pending_operations.load(Ordering::Acquire));
        if batch_size == 0 {
            return Ok(());
        }

        debug!("Processing batch of {} operations", batch_size);
        let start_time = Instant::now();

        // Extract batch from queue
        let batch: Vec<BatchOperation> = {
            let mut queue = self.operation_queue.write();
            (0..batch_size)
                .filter_map(|_| queue.pop_front())
                .collect()
        };

        self.pending_operations.fetch_sub(batch.len(), Ordering::AcqRel);

        // Group operations by type for efficient processing
        let mut inserts = Vec::new();
        let mut updates = Vec::new();
        let mut deletes = Vec::new();
        let mut searches = Vec::new();

        for op in batch {
            match op {
                BatchOperation::Insert { node_id, embedding } => inserts.push((node_id, embedding)),
                BatchOperation::Update { node_id, embedding } => updates.push((node_id, embedding)),
                BatchOperation::Delete { node_id } => deletes.push(node_id),
                BatchOperation::Search { query_embedding, k, callback_id } => 
                    searches.push((query_embedding, k, callback_id)),
            }
        }

        // Process operations in parallel if configured
        if self.config.parallel_processing {
            let insert_task = self.process_inserts_parallel(inserts);
            let update_task = self.process_updates_parallel(updates);
            let delete_task = self.process_deletes_parallel(deletes);
            let search_task = self.process_searches_parallel(searches);

            tokio::try_join!(insert_task, update_task, delete_task, search_task)?;
        } else {
            self.process_inserts_sequential(inserts).await?;
            self.process_updates_sequential(updates).await?;
            self.process_deletes_sequential(deletes).await?;
            self.process_searches_sequential(searches).await?;
        }

        *self.last_flush.write() = Instant::now();
        
        let duration = start_time.elapsed();
        debug!("Batch processing completed in {:?}", duration);
        
        Ok(())
    }

    /// Process insert operations in parallel
    async fn process_inserts_parallel(&self, inserts: Vec<(NodeId, Vec<f32>)>) -> Result<()> {
        if inserts.is_empty() {
            return Ok(());
        }

        let vectors: Vec<f32> = inserts.iter().flat_map(|(_, emb)| emb.iter().cloned()).collect();
        
        // Add to index
        let faiss_ids = {
            let mut index_manager = self.index_manager.write();
            index_manager.add_vectors(&vectors)?
        };

        // Update mappings in parallel
        inserts.par_iter().zip(faiss_ids.par_iter()).for_each(|((node_id, _), faiss_id)| {
            self.id_mapping.insert(*faiss_id, *node_id);
            self.reverse_mapping.insert(*node_id, *faiss_id);
            
            // Send result
            let _ = self.result_sender.try_send(BatchResult::Inserted {
                node_id: *node_id,
                faiss_id: *faiss_id,
            });
        });

        self.successful_operations.fetch_add(inserts.len(), Ordering::AcqRel);
        info!("Successfully inserted {} vectors", inserts.len());
        
        Ok(())
    }

    /// Process update operations
    async fn process_updates_parallel(&self, updates: Vec<(NodeId, Vec<f32>)>) -> Result<()> {
        if updates.is_empty() {
            return Ok(());
        }

        // For updates, we treat them as delete + insert for simplicity
        // In a more sophisticated implementation, we could use FAISS's update capabilities
        let node_ids: Vec<NodeId> = updates.iter().map(|(id, _)| *id).collect();
        
        self.process_deletes_sequential(node_ids).await?;
        let inserts: Vec<(NodeId, Vec<f32>)> = updates;
        self.process_inserts_parallel(inserts).await?;

        Ok(())
    }

    /// Process delete operations
    async fn process_deletes_parallel(&self, deletes: Vec<NodeId>) -> Result<()> {
        if deletes.is_empty() {
            return Ok(());
        }

        // Remove from mappings
        for node_id in &deletes {
            if let Some((_, faiss_id)) = self.reverse_mapping.remove(node_id) {
                self.id_mapping.remove(&faiss_id);
                
                let _ = self.result_sender.try_send(BatchResult::Deleted {
                    node_id: *node_id,
                });
            }
        }

        // Note: FAISS doesn't directly support deletion, so we mark entries as invalid
        // A full index rebuild would be needed to actually remove deleted vectors
        
        self.successful_operations.fetch_add(deletes.len(), Ordering::AcqRel);
        info!("Successfully deleted {} vectors", deletes.len());
        
        Ok(())
    }

    /// Process search operations in parallel
    async fn process_searches_parallel(&self, searches: Vec<(Vec<f32>, usize, Uuid)>) -> Result<()> {
        if searches.is_empty() {
            return Ok(());
        }

        // Process searches in parallel using rayon
        searches.par_iter().for_each(|(query_embedding, k, callback_id)| {
            let search_result = {
                let index_manager = self.index_manager.read();
                index_manager.search(query_embedding, *k)
            };

            match search_result {
                Ok((distances, labels)) => {
                    let results: Vec<(NodeId, f32)> = labels
                        .into_iter()
                        .zip(distances.into_iter())
                        .filter_map(|(faiss_id, distance)| {
                            self.id_mapping.get(&faiss_id).map(|node_id| (*node_id, distance))
                        })
                        .collect();

                    let _ = self.result_sender.try_send(BatchResult::SearchComplete {
                        callback_id: *callback_id,
                        results,
                    });
                },
                Err(e) => {
                    let _ = self.result_sender.try_send(BatchResult::Error {
                        operation_id: *callback_id,
                        error: e.to_string(),
                    });
                }
            }
        });

        self.successful_operations.fetch_add(searches.len(), Ordering::AcqRel);
        
        Ok(())
    }

    /// Sequential versions for fallback
    async fn process_inserts_sequential(&self, inserts: Vec<(NodeId, Vec<f32>)>) -> Result<()> {
        self.process_inserts_parallel(inserts).await
    }

    async fn process_updates_sequential(&self, updates: Vec<(NodeId, Vec<f32>)>) -> Result<()> {
        self.process_updates_parallel(updates).await
    }

    async fn process_deletes_sequential(&self, deletes: Vec<NodeId>) -> Result<()> {
        self.process_deletes_parallel(deletes).await
    }

    async fn process_searches_sequential(&self, searches: Vec<(Vec<f32>, usize, Uuid)>) -> Result<()> {
        self.process_searches_parallel(searches).await
    }

    /// Background processing loop
    async fn background_processing_loop(&self) {
        let mut flush_interval = time::interval(self.config.flush_interval);
        
        while self.processing_active.load(Ordering::Acquire) {
            tokio::select! {
                _ = flush_interval.tick() => {
                    if let Err(e) = self.flush_pending_operations().await {
                        error!("Error during periodic flush: {}", e);
                    }
                }
                _ = tokio::time::sleep(Duration::from_millis(100)) => {
                    // Check if we should process a batch based on queue size
                    if self.pending_operations.load(Ordering::Acquire) >= self.config.batch_size {
                        if let Err(e) = self.process_batch().await {
                            error!("Error during batch processing: {}", e);
                            self.failed_operations.fetch_add(self.config.batch_size, Ordering::AcqRel);
                        }
                    }
                }
            }
        }
    }

    /// Flush all pending operations
    async fn flush_pending_operations(&self) -> Result<()> {
        while self.pending_operations.load(Ordering::Acquire) > 0 {
            self.process_batch().await?;
        }

        // Persist mappings if storage is available
        if let Some(ref storage) = self.storage {
            let id_mapping: HashMap<i64, NodeId> = self.id_mapping
                .iter()
                .map(|entry| (*entry.key(), *entry.value()))
                .collect();
            
            let reverse_mapping: HashMap<NodeId, i64> = self.reverse_mapping
                .iter()
                .map(|entry| (*entry.key(), *entry.value()))
                .collect();

            storage.save_id_mapping(&id_mapping, &reverse_mapping)?;
        }

        Ok(())
    }

    /// Get batch processing statistics
    pub fn get_stats(&self) -> BatchStats {
        let total = self.total_operations.load(Ordering::Acquire);
        let successful = self.successful_operations.load(Ordering::Acquire);
        let failed = self.failed_operations.load(Ordering::Acquire);
        let pending = self.pending_operations.load(Ordering::Acquire);

        BatchStats {
            total_operations: total,
            successful_operations: successful,
            failed_operations: failed,
            pending_operations: pending,
            success_rate: if total > 0 { successful as f64 / total as f64 } else { 0.0 },
            active: self.processing_active.load(Ordering::Acquire),
        }
    }

    /// Get next batch result
    pub fn try_recv_result(&self) -> Result<Option<BatchResult>> {
        match self.result_receiver.try_recv() {
            Ok(result) => Ok(Some(result)),
            Err(crossbeam_channel::TryRecvError::Empty) => Ok(None),
            Err(crossbeam_channel::TryRecvError::Disconnected) => {
                Err(CodeGraphError::Internal("Result channel disconnected".to_string()))
            }
        }
    }

    /// Helper method for cloning processor for background tasks
    fn clone_for_background(&self) -> Self {
        Self {
            config: self.config.clone(),
            index_manager: Arc::clone(&self.index_manager),
            storage: self.storage.as_ref().map(Arc::clone),
            operation_queue: Arc::clone(&self.operation_queue),
            pending_operations: Arc::clone(&self.pending_operations),
            id_mapping: Arc::clone(&self.id_mapping),
            reverse_mapping: Arc::clone(&self.reverse_mapping),
            processing_active: Arc::clone(&self.processing_active),
            last_flush: Arc::clone(&self.last_flush),
            result_sender: self.result_sender.clone(),
            result_receiver: self.result_receiver.clone(),
            total_operations: Arc::clone(&self.total_operations),
            successful_operations: Arc::clone(&self.successful_operations),
            failed_operations: Arc::clone(&self.failed_operations),
        }
    }
}

#[derive(Debug, Clone)]
pub struct BatchStats {
    pub total_operations: usize,
    pub successful_operations: usize,
    pub failed_operations: usize,
    pub pending_operations: usize,
    pub success_rate: f64,
    pub active: bool,
}