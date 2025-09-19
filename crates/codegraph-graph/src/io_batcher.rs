use std::sync::Arc;
use std::time::{Duration, Instant};

use dashmap::DashMap;
use rocksdb::{DBWithThreadMode, MultiThreaded, ReadOptions};

use codegraph_core::{CodeGraphError, CodeNode, NodeId, Result};

#[derive(Debug, Clone)]
pub struct BatchingConfig {
    pub batch_size: usize,
    pub max_wait_time_ms: u64,
    pub write_flush_interval: Duration,
}

impl Default for BatchingConfig {
    fn default() -> Self {
        Self {
            batch_size: 100,
            max_wait_time_ms: 10,
            write_flush_interval: Duration::from_millis(100),
        }
    }
}

type DB = DBWithThreadMode<MultiThreaded>;

#[derive(Debug, Clone)]
pub struct ReadCoalescer {
    db: Arc<DB>,
    cf_name: &'static str,
    cache: Arc<DashMap<NodeId, Arc<CodeNode>>>,
    config: BatchingConfig,
}

impl ReadCoalescer {
    pub fn new(
        db: Arc<DB>,
        cf_name: &'static str,
        cache: Arc<DashMap<NodeId, Arc<CodeNode>>>,
        config: BatchingConfig,
    ) -> Self {
        Self {
            db,
            cf_name,
            cache,
            config,
        }
    }

    #[inline]
    fn node_key(id: NodeId) -> [u8; 16] {
        *id.as_bytes()
    }

    pub fn get_node(&self, id: NodeId) -> Result<Option<CodeNode>> {
        if let Some(cached) = self.cache.get(&id) {
            return Ok(Some(cached.as_ref().clone()));
        }
        let cf = self.db.cf_handle(self.cf_name).ok_or_else(|| {
            CodeGraphError::Database(format!("Column family '{}' not found", self.cf_name))
        })?;
        let key = Self::node_key(id);
        let mut read_opts = ReadOptions::default();
        read_opts.set_readahead_size(2 * 1024 * 1024);
        let data = self
            .db
            .get_cf_opt(&cf, &key, &read_opts)
            .map_err(|e| CodeGraphError::Database(e.to_string()))?;
        if let Some(bytes) = data {
            // SerializableCodeNode is defined in storage.rs module; mimic fields via serde
            let s: crate::storage::SerializableCodeNode = bincode::decode_from_slice(&bytes, bincode::config::standard())
                .map_err(|e| CodeGraphError::Database(e.to_string()))?
                .0;
            let node: CodeNode = s.into();
            self.cache.insert(id, Arc::new(node.clone()));
            Ok(Some(node))
        } else {
            Ok(None)
        }
    }
}

#[derive(Debug)]
pub struct WriteBatchOptimizer {
    config: BatchingConfig,
    pub ops_threshold: usize,
    last_flush: Instant,
}

impl WriteBatchOptimizer {
    pub fn new(config: &BatchingConfig) -> Self {
        Self {
            ops_threshold: config.batch_size,
            config: config.clone(),
            last_flush: Instant::now(),
        }
    }

    pub fn should_time_flush(&self, interval: Duration) -> bool {
        self.last_flush.elapsed() >= interval
    }

    pub fn on_flushed(&mut self, _ops: usize, _elapsed: Duration) {
        self.last_flush = Instant::now();
    }
}
