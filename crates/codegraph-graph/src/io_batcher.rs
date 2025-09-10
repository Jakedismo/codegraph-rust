use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use dashmap::DashMap;
use tokio::sync::{mpsc, oneshot};

use codegraph_core::{CodeGraphError, CodeNode, NodeId, Result};
use rocksdb::{ColumnFamily, DBWithThreadMode, MultiThreaded};

type DB = DBWithThreadMode<MultiThreaded>;

/// Configuration for I/O batching behavior
#[derive(Debug, Clone)]
pub struct BatchingConfig {
    // Read coalescing
    pub max_read_batch: usize,
    pub read_coalesce_delay: Duration,

    // Write batching thresholds
    pub initial_write_ops_threshold: usize,
    pub min_write_ops_threshold: usize,
    pub max_write_ops_threshold: usize,
    pub write_flush_interval: Duration,
    pub target_flush_latency_ms: f64,
    pub ewma_alpha: f64,
}

impl Default for BatchingConfig {
    fn default() -> Self {
        Self {
            max_read_batch: 256,
            read_coalesce_delay: Duration::from_micros(300),
            initial_write_ops_threshold: 1000,
            min_write_ops_threshold: 128,
            max_write_ops_threshold: 10_000,
            write_flush_interval: Duration::from_millis(5),
            target_flush_latency_ms: 2.0,
            ewma_alpha: 0.2,
        }
    }
}

/// Adaptive optimizer for write batch size based on observed flush latency
#[derive(Debug)]
pub struct WriteBatchOptimizer {
    pub ops_threshold: usize,
    pub min_ops_threshold: usize,
    pub max_ops_threshold: usize,
    pub target_latency_ms: f64,
    pub ewma_alpha: f64,
    pub last_flush_at: Instant,
    pub ewma_latency_ms: f64,
}

impl WriteBatchOptimizer {
    pub fn new(config: &BatchingConfig) -> Self {
        Self {
            ops_threshold: config.initial_write_ops_threshold,
            min_ops_threshold: config.min_write_ops_threshold,
            max_ops_threshold: config.max_write_ops_threshold,
            target_latency_ms: config.target_flush_latency_ms,
            ewma_alpha: config.ewma_alpha,
            last_flush_at: Instant::now(),
            ewma_latency_ms: config.target_flush_latency_ms,
        }
    }

    pub fn should_time_flush(&self, interval: Duration) -> bool {
        self.last_flush_at.elapsed() >= interval
    }

    pub fn on_flushed(&mut self, ops_flushed: usize, flush_latency: Duration) {
        let ms = flush_latency.as_secs_f64() * 1000.0;
        // EWMA update
        self.ewma_latency_ms = self.ewma_alpha * ms + (1.0 - self.ewma_alpha) * self.ewma_latency_ms;

        // Simple PID-like adjustment heuristic
        if self.ewma_latency_ms > self.target_latency_ms * 1.25 {
            self.ops_threshold = (self.ops_threshold.saturating_mul(3) / 4).max(self.min_ops_threshold);
        } else if self.ewma_latency_ms < self.target_latency_ms * 0.6 {
            // grow moderately when well below target
            let increased = (self.ops_threshold as f64 * 1.25) as usize;
            self.ops_threshold = increased.min(self.max_ops_threshold);
        }

        // Prevent pathological thresholds
        self.ops_threshold = self
            .ops_threshold
            .clamp(self.min_ops_threshold, self.max_ops_threshold);
        self.last_flush_at = Instant::now();
        let _ = ops_flushed; // reserved for future, e.g., bytes per op feedback
    }
}

/// Request to fetch a node via read coalescer
struct ReadReq {
    id: NodeId,
    tx: oneshot::Sender<Result<Option<CodeNode>>>,
}

/// Read coalescer groups get_node requests and serves them in batches
pub struct ReadCoalescer {
    tx: mpsc::Sender<ReadReq>,
}

impl ReadCoalescer {
    pub fn new(
        db: Arc<DB>,
        nodes_cf_name: &'static str,
        read_cache: Arc<DashMap<NodeId, Arc<CodeNode>>>,
        cfg: BatchingConfig,
    ) -> Self {
        let (tx, mut rx) = mpsc::channel::<ReadReq>(4096);
        let _handle = tokio::spawn(async move {
            // Main aggregation loop
            loop {
                let Some(first) = rx.recv().await else { break };

                // Accumulate within coalesce window
                let mut batch: Vec<ReadReq> = vec![first];
                let start = Instant::now();
                while batch.len() < cfg.max_read_batch {
                    let remaining = cfg
                        .read_coalesce_delay
                        .checked_sub(start.elapsed())
                        .unwrap_or(Duration::from_micros(0));
                    if remaining.is_zero() {
                        break;
                    }
                    tokio::select! {
                        maybe_req = rx.recv() => {
                            if let Some(req) = maybe_req { batch.push(req); }
                            else { break; }
                        }
                        _ = tokio::time::sleep(remaining) => { break; }
                    }
                }

                // Map ids to request senders
                let mut id_to_senders: HashMap<NodeId, Vec<oneshot::Sender<Result<Option<CodeNode>>>>> = HashMap::new();
                let mut miss_ids: Vec<NodeId> = Vec::with_capacity(batch.len());
                for req in batch {
                    // Fast-path cache
                    if let Some(cached) = read_cache.get(&req.id) {
                        let _ = req.tx.send(Ok(Some(cached.as_ref().clone())));
                    } else {
                        id_to_senders.entry(req.id).or_default().push(req.tx);
                        miss_ids.push(req.id);
                    }
                }

                if miss_ids.is_empty() { continue; }

                // Fetch misses from RocksDB
                // Note: multi_get may be used in future; for compatibility, loop with get_cf
                let nodes_cf = match db.cf_handle(nodes_cf_name) {
                    Some(cf) => cf,
                    None => {
                        for (_, senders) in id_to_senders.into_iter() {
                            for tx in senders {
                                let _ = tx.send(Err(CodeGraphError::Database("Column family not found".to_string())));
                            }
                        }
                        continue;
                    }
                };

                for id in miss_ids {
                    let key = id.to_be_bytes();
                    let resp = match db.get_cf(&nodes_cf, key) {
                        Ok(Some(bytes)) => {
                            let ser: Result<CodeNode> = (|| {
                                let serializable: super::storage::SerializableCodeNode = bincode::deserialize(&bytes)
                                    .map_err(|e| CodeGraphError::Database(e.to_string()))?;
                                Ok(CodeNode::from(serializable))
                            })();
                            match ser {
                                Ok(node) => {
                                    let node_arc = Arc::new(node.clone());
                                    read_cache.insert(id, node_arc);
                                    Ok(Some(node))
                                }
                                Err(e) => Err(e),
                            }
                        }
                        Ok(None) => Ok(None),
                        Err(e) => Err(CodeGraphError::Database(e.to_string())),
                    };

                    if let Some(senders) = id_to_senders.remove(&id) {
                        for tx in senders {
                            let _ = tx.send(resp.clone());
                        }
                    }
                }
            }
        });

        Self { tx }
    }

    pub async fn get_node(&self, id: NodeId) -> Result<Option<CodeNode>> {
        let (tx, rx) = oneshot::channel();
        let req = ReadReq { id, tx };
        if let Err(_e) = self.tx.send(req).await {
            return Err(CodeGraphError::Database("read coalescer unavailable".into()));
        }
        rx.await.map_err(|_| CodeGraphError::Database("read coalescer dropped".into()))?
    }
}

