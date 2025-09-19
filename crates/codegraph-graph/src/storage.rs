use async_trait::async_trait;
use chrono;
use codegraph_core::{
    CodeGraphError, CodeNode, GraphStore, Language, Location, NodeId, NodeType, Result,
};
use dashmap::DashMap;
use memmap2::{Mmap, MmapOptions};
use parking_lot::RwLock;
use rocksdb::{
    BlockBasedOptions, BoundColumnFamily, Cache, ColumnFamilyDescriptor, DBCompressionType,
    DBWithThreadMode, IteratorMode, MultiThreaded, Options, ReadOptions, SliceTransform,
    WriteBatch, WriteOptions,
};
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
};

use crate::io_batcher::{BatchingConfig, ReadCoalescer};

type DB = DBWithThreadMode<MultiThreaded>;

const NODES_CF: &str = "nodes";
const EDGES_CF: &str = "edges";
const INDICES_CF: &str = "indices";
const METADATA_CF: &str = "metadata";

#[derive(Serialize, Deserialize, Clone, Debug, bincode::Encode, bincode::Decode)]
pub struct SerializableCodeNode {
    pub id: NodeId,
    pub name: String,
    pub node_type: Option<NodeType>,
    pub language: Option<Language>,
    pub location: Location,
    pub content: Option<String>,
    pub metadata: HashMap<String, String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, bincode::Encode, bincode::Decode)]
pub struct SerializableEdge {
    pub id: u64,
    pub from: NodeId,
    pub to: NodeId,
    pub edge_type: String,
    pub weight: f64,
    pub metadata: HashMap<String, String>,
}

impl From<CodeNode> for SerializableCodeNode {
    fn from(node: CodeNode) -> Self {
        Self {
            id: node.id,
            name: node.name.into_string(),
            node_type: node.node_type,
            language: node.language,
            location: node.location,
            content: node.content.as_ref().map(|s| s.to_string()),
            metadata: node.metadata.attributes,
        }
    }
}

impl From<SerializableCodeNode> for CodeNode {
    fn from(node: SerializableCodeNode) -> Self {
        use codegraph_core::{Metadata, SharedStr};
        let now = chrono::Utc::now();
        Self {
            id: node.id,
            name: node.name.into(),
            node_type: node.node_type,
            language: node.language,
            location: node.location,
            content: node.content.map(SharedStr::from),
            metadata: Metadata {
                attributes: node.metadata,
                created_at: now,
                updated_at: now,
            },
            embedding: None,
            complexity: None,
        }
    }
}

#[derive(Debug)]
pub struct HighPerformanceRocksDbStorage {
    db: Arc<DB>,
    db_path: PathBuf,
    read_cache: Arc<DashMap<NodeId, Arc<CodeNode>>>,
    edge_cache: Arc<DashMap<NodeId, Arc<Vec<SerializableEdge>>>>,
    edge_counter: AtomicU64,
    // Options created per-operation to avoid non-Send/Sync fields
    memory_tables: Arc<RwLock<HashMap<String, Mmap>>>,
    batching_config: BatchingConfig,
    read_coalescer: ReadCoalescer,
    // Transaction state disabled (no-op)
}

impl HighPerformanceRocksDbStorage {
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self> {
        let mut db_opts = Options::default();

        db_opts.create_if_missing(true);
        db_opts.create_missing_column_families(true);

        db_opts.set_max_background_jobs(num_cpus::get() as i32);
        db_opts.set_max_subcompactions(4);

        db_opts.set_write_buffer_size(128 * 1024 * 1024); // 128MB
        db_opts.set_max_write_buffer_number(6);
        db_opts.set_min_write_buffer_number_to_merge(2);

        db_opts.set_level_zero_file_num_compaction_trigger(4);
        db_opts.set_level_zero_slowdown_writes_trigger(20);
        db_opts.set_level_zero_stop_writes_trigger(36);

        db_opts.set_target_file_size_base(64 * 1024 * 1024); // 64MB
        db_opts.set_target_file_size_multiplier(2);

        db_opts.set_max_bytes_for_level_base(256 * 1024 * 1024); // 256MB
        db_opts.set_max_bytes_for_level_multiplier(10.0);

        db_opts.set_bytes_per_sync(1048576);
        db_opts.set_wal_bytes_per_sync(1048576);

        db_opts.set_compression_type(DBCompressionType::Lz4);
        db_opts.set_bottommost_compression_type(DBCompressionType::Zstd);
        // Reduce syscall overhead where available, but avoid invalid combinations.
        // RocksDB requires that if allow_mmap_reads is enabled, use_direct_reads must be disabled.
        db_opts.set_use_direct_reads(false);
        // Direct I/O for flush/compaction is incompatible with mmap writes.
        // Prefer mmap for throughput; disable direct I/O here.
        db_opts.set_use_direct_io_for_flush_and_compaction(false);

        db_opts.set_allow_mmap_reads(true);
        db_opts.set_allow_mmap_writes(true);

        let block_cache = Cache::new_lru_cache(512 * 1024 * 1024); // 512MB

        let mut block_opts = BlockBasedOptions::default();
        block_opts.set_block_size(64 * 1024); // 64KB blocks
        block_opts.set_block_cache(&block_cache);
        block_opts.set_cache_index_and_filter_blocks(true);
        block_opts.set_pin_l0_filter_and_index_blocks_in_cache(true);
        block_opts.set_bloom_filter(10.0, false);
        block_opts.set_whole_key_filtering(false);

        let prefix_extractor = SliceTransform::create_fixed_prefix(8);
        db_opts.set_prefix_extractor(prefix_extractor);
        db_opts.set_block_based_table_factory(&block_opts);

        // Exactly 4 column families: nodes, edges, indices, metadata
        let cf_descriptors = vec![
            Self::create_nodes_cf_descriptor(),
            Self::create_edges_cf_descriptor(),
            Self::create_indices_cf_descriptor(),
            Self::create_metadata_cf_descriptor(),
        ];

        let db = DB::open_cf_descriptors(&db_opts, &path, cf_descriptors)
            .map_err(|e| CodeGraphError::Database(format!("Failed to open database: {}", e)))?;

        // Use per-operation options; avoid storing in struct to keep Sync

        let batching_config = BatchingConfig::default();
        let db_arc = Arc::new(db);
        let read_cache = Arc::new(DashMap::with_capacity(100_000));
        let read_coalescer = ReadCoalescer::new(
            db_arc.clone(),
            NODES_CF,
            read_cache.clone(),
            batching_config.clone(),
        );

        let storage = Self {
            db: db_arc,
            db_path: path.as_ref().to_path_buf(),
            read_cache: read_cache,
            edge_cache: Arc::new(DashMap::with_capacity(50_000)),
            edge_counter: AtomicU64::new(1),

            memory_tables: Arc::new(RwLock::new(HashMap::new())),
            batching_config: batching_config.clone(),
            read_coalescer,
        };

        storage.initialize_counters()?;

        Ok(storage)
    }

    pub fn new_read_only<P: AsRef<Path>>(path: P) -> Result<Self> {
        let mut db_opts = Options::default();
        // Do not create missing; expect an existing DB schema
        db_opts.set_compression_type(DBCompressionType::Lz4);
        db_opts.set_bottommost_compression_type(DBCompressionType::Zstd);
        db_opts.set_use_direct_reads(false);
        db_opts.set_use_direct_io_for_flush_and_compaction(false);
        db_opts.set_allow_mmap_reads(true);
        db_opts.set_allow_mmap_writes(false);

        // Open existing column families in read-only mode
        let cf_names = vec![NODES_CF, EDGES_CF, INDICES_CF, METADATA_CF];
        let db = DB::open_cf_for_read_only(&db_opts, &path, cf_names, false)
            .map_err(|e| CodeGraphError::Database(format!("Failed to open database (read-only): {}", e)))?;

        let batching_config = BatchingConfig::default();
        let db_arc = Arc::new(db);
        let read_cache = Arc::new(DashMap::with_capacity(100_000));
        let read_coalescer = ReadCoalescer::new(
            db_arc.clone(),
            NODES_CF,
            read_cache.clone(),
            batching_config.clone(),
        );

        let storage = Self {
            db: db_arc,
            db_path: path.as_ref().to_path_buf(),
            read_cache,
            edge_cache: Arc::new(DashMap::with_capacity(50_000)),
            edge_counter: AtomicU64::new(1),
            memory_tables: Arc::new(RwLock::new(HashMap::new())),
            batching_config: batching_config.clone(),
            read_coalescer,
        };

        // Safe to attempt reading counters in read-only mode
        let _ = storage.initialize_counters();

        Ok(storage)
    }

    pub(crate) fn add_node_inner(&self, node: &CodeNode) -> Result<()> {
        let node_id = node.id;
        let serializable_node = SerializableCodeNode::from(node.clone());
        let node_key = Self::node_key(node_id);
        let node_bytes = bincode::encode_to_vec(&serializable_node, bincode::config::standard())
            .map_err(|e| CodeGraphError::Database(e.to_string()))?;
        let name_index_key = Self::index_key(b"name:", node.name.as_str(), node_id);
        self.with_batch(None, |batch| {
            let nodes_cf = self.get_cf_handle(NODES_CF)?;
            let indices_cf = self.get_cf_handle(INDICES_CF)?;
            batch.put_cf(&nodes_cf, node_key, node_bytes);
            batch.put_cf(&indices_cf, name_index_key, b"");
            Ok(())
        })?;
        // writes committed immediately
        Ok(())
    }

    pub(crate) fn remove_node_inner(&self, id: NodeId) -> Result<()> {
        if let Some(node) = self.read_coalescer.get_node(id)? {
            let node_key = Self::node_key(id);
            let name_index_key = Self::index_key(b"name:", &node.name, id);
            self.with_batch(None, |batch| {
                let nodes_cf = self.get_cf_handle(NODES_CF)?;
                let indices_cf = self.get_cf_handle(INDICES_CF)?;
                batch.delete_cf(&nodes_cf, node_key);
                batch.delete_cf(&indices_cf, name_index_key);
                Ok(())
            })?;
            // writes committed immediately
        }
        Ok(())
    }

    pub(crate) fn scan_node_ids_by_name(&self, name: &str) -> Result<Vec<NodeId>> {
        let prefix = format!("name:{}", name);
        let name_index_cf = self.get_cf_handle(INDICES_CF)?;
        let mut read_opts = ReadOptions::default();
        read_opts.set_prefix_same_as_start(true);
        read_opts.set_readahead_size(2 * 1024 * 1024);
        let iter = self.db.iterator_cf_opt(
            &name_index_cf,
            read_opts,
            IteratorMode::From(prefix.as_bytes(), rocksdb::Direction::Forward),
        );
        let mut ids = Vec::new();
        for item in iter {
            let (key, _) = item.map_err(|e| CodeGraphError::Database(e.to_string()))?;
            if !key.starts_with(prefix.as_bytes()) {
                break;
            }
            if key.len() >= 16 {
                let node_id_bytes = &key[key.len() - 16..];
                if let Ok(uuid) = uuid::Uuid::from_slice(node_id_bytes) {
                    ids.push(uuid);
                }
            }
        }
        Ok(ids)
    }

    fn create_nodes_cf_descriptor() -> ColumnFamilyDescriptor {
        let mut opts = Options::default();
        opts.set_write_buffer_size(64 * 1024 * 1024);
        opts.set_max_write_buffer_number(3);
        opts.set_compression_type(DBCompressionType::Lz4);

        let mut block_opts = BlockBasedOptions::default();
        let cache = Cache::new_lru_cache(256 * 1024 * 1024);
        block_opts.set_block_cache(&cache);
        block_opts.set_block_size(32 * 1024);
        opts.set_block_based_table_factory(&block_opts);

        ColumnFamilyDescriptor::new(NODES_CF, opts)
    }

    fn create_edges_cf_descriptor() -> ColumnFamilyDescriptor {
        let mut opts = Options::default();
        opts.set_write_buffer_size(128 * 1024 * 1024);
        opts.set_max_write_buffer_number(4);
        opts.set_compression_type(DBCompressionType::Lz4);

        let mut block_opts = BlockBasedOptions::default();
        let cache = Cache::new_lru_cache(512 * 1024 * 1024);
        block_opts.set_block_cache(&cache);
        block_opts.set_block_size(64 * 1024);
        opts.set_block_based_table_factory(&block_opts);

        ColumnFamilyDescriptor::new(EDGES_CF, opts)
    }

    fn create_indices_cf_descriptor() -> ColumnFamilyDescriptor {
        let mut opts = Options::default();
        opts.set_write_buffer_size(32 * 1024 * 1024);
        opts.set_max_write_buffer_number(2);
        opts.set_compression_type(DBCompressionType::Lz4);

        let prefix_extractor = SliceTransform::create_fixed_prefix(16);
        opts.set_prefix_extractor(prefix_extractor);

        let mut block_opts = BlockBasedOptions::default();
        let cache = Cache::new_lru_cache(128 * 1024 * 1024);
        block_opts.set_block_cache(&cache);
        block_opts.set_bloom_filter(15.0, false);
        opts.set_block_based_table_factory(&block_opts);

        ColumnFamilyDescriptor::new(INDICES_CF, opts)
    }

    fn create_metadata_cf_descriptor() -> ColumnFamilyDescriptor {
        let mut opts = Options::default();
        opts.set_write_buffer_size(16 * 1024 * 1024);
        opts.set_compression_type(DBCompressionType::Zstd);

        ColumnFamilyDescriptor::new(METADATA_CF, opts)
    }

    fn initialize_counters(&self) -> Result<()> {
        let metadata_cf = self.get_cf_handle(METADATA_CF)?;

        // Node counter is not tracked for UUID-based NodeId

        if let Some(edge_count_bytes) = self
            .db
            .get_cf(&metadata_cf, b"edge_counter")
            .map_err(|e| CodeGraphError::Database(e.to_string()))?
        {
            if let Ok(count) = bincode::decode_from_slice(&edge_count_bytes, bincode::config::standard()).map(|(count, _)| count) {
                self.edge_counter.store(count, Ordering::Relaxed);
            }
        }

        Ok(())
    }

    fn get_cf_handle(&self, name: &str) -> Result<std::sync::Arc<BoundColumnFamily<'_>>> {
        self.db
            .cf_handle(name)
            .ok_or_else(|| CodeGraphError::Database(format!("Column family '{}' not found", name)))
    }

    fn node_key(id: NodeId) -> [u8; 16] {
        *id.as_bytes()
    }

    fn edge_key(id: u64) -> [u8; 8] {
        id.to_be_bytes()
    }

    fn index_key(prefix: &[u8], value: &str, id: NodeId) -> Vec<u8> {
        let mut key = Vec::with_capacity(prefix.len() + value.len() + 16);
        key.extend_from_slice(prefix);
        key.extend_from_slice(value.as_bytes());
        key.extend_from_slice(id.as_bytes());
        key
    }

    fn index_edge_key(prefix: &[u8], value: &str, edge_id: u64) -> Vec<u8> {
        let mut key = Vec::with_capacity(prefix.len() + value.len() + 8);
        key.extend_from_slice(prefix);
        key.extend_from_slice(value.as_bytes());
        key.extend_from_slice(&edge_id.to_be_bytes());
        key
    }

    // Transaction API disabled (use global batch)
    pub fn begin(&self) -> u64 {
        0
    }
    pub fn commit(&self, _tx_id: u64) -> Result<()> {
        Ok(())
    }
    pub fn rollback(&self, _tx_id: u64) -> Result<()> {
        Ok(())
    }

    fn with_batch<F>(&self, _tx_id: Option<u64>, mutator: F) -> Result<()>
    where
        F: FnOnce(&mut WriteBatch) -> Result<()>,
    {
        let mut wb = WriteBatch::default();
        mutator(&mut wb)?;
        let mut opts = WriteOptions::default();
        opts.set_sync(false);
        opts.disable_wal(false);
        self.db
            .write_opt(wb, &opts)
            .map_err(|e| CodeGraphError::Database(e.to_string()))?;
        Ok(())
    }

    pub async fn add_edge(&self, edge: SerializableEdge) -> Result<()> {
        let edge_id = self.edge_counter.fetch_add(1, Ordering::Relaxed);
        let mut edge = edge;
        edge.id = edge_id;

        let edges_cf = self.get_cf_handle(EDGES_CF)?;
        let indices_cf = self.get_cf_handle(INDICES_CF)?;

        let edge_key = Self::edge_key(edge_id);
        let edge_bytes =
            bincode::encode_to_vec(&edge, bincode::config::standard()).map_err(|e| CodeGraphError::Database(e.to_string()))?;

        let from_index_key = Self::index_edge_key(b"from:", &edge.from.to_string(), edge_id);
        let to_index_key = Self::index_edge_key(b"to:", &edge.to.to_string(), edge_id);

        self.with_batch(None, |batch| {
            batch.put_cf(&edges_cf, edge_key, edge_bytes);
            batch.put_cf(&indices_cf, from_index_key, b"");
            batch.put_cf(&indices_cf, to_index_key, b"");
            Ok(())
        })?;
        // writes committed immediately

        self.edge_cache.remove(&edge.from);

        Ok(())
    }

    pub async fn add_edge_tx(&self, tx_id: u64, edge: SerializableEdge) -> Result<()> {
        let edge_id = edge.id;
        let edges_cf = self.get_cf_handle(EDGES_CF)?;
        let indices_cf = self.get_cf_handle(INDICES_CF)?;
        let edge_key = Self::edge_key(edge_id);
        let edge_bytes =
            bincode::encode_to_vec(&edge, bincode::config::standard()).map_err(|e| CodeGraphError::Database(e.to_string()))?;
        let from_index_key = Self::index_edge_key(b"from:", &edge.from.to_string(), edge_id);
        let to_index_key = Self::index_edge_key(b"to:", &edge.to.to_string(), edge_id);

        self.with_batch(Some(tx_id), |batch| {
            batch.put_cf(&edges_cf, edge_key, edge_bytes);
            batch.put_cf(&indices_cf, from_index_key, b"");
            batch.put_cf(&indices_cf, to_index_key, b"");
            Ok(())
        })?;
        Ok(())
    }

    pub async fn get_edges_from(&self, node_id: NodeId) -> Result<Vec<SerializableEdge>> {
        if let Some(cached) = self.edge_cache.get(&node_id) {
            return Ok(cached.as_ref().clone());
        }

        let from_index_cf = self.get_cf_handle(INDICES_CF)?;
        let edges_cf = self.get_cf_handle(EDGES_CF)?;

        let prefix = format!("from:{}", node_id);
        let mut read_opts = ReadOptions::default();
        read_opts.set_prefix_same_as_start(true);
        read_opts.set_readahead_size(2 * 1024 * 1024);

        let iter = self.db.iterator_cf_opt(
            &from_index_cf,
            read_opts,
            IteratorMode::From(prefix.as_bytes(), rocksdb::Direction::Forward),
        );
        let mut edge_ids: Vec<u64> = Vec::new();
        for item in iter {
            let (key, _) = item.map_err(|e| CodeGraphError::Database(e.to_string()))?;
            if !key.starts_with(prefix.as_bytes()) {
                break;
            }
            if key.len() >= 8 {
                let edge_id_bytes = &key[key.len() - 8..];
                let edge_id = u64::from_be_bytes(edge_id_bytes.try_into().unwrap_or_default());
                edge_ids.push(edge_id);
            }
        }

        let mut edges = Vec::with_capacity(edge_ids.len());
        for edge_id in edge_ids {
            let edge_key = Self::edge_key(edge_id);
            if let Some(edge_data) = self
                .db
                .get_cf(&edges_cf, edge_key)
                .map_err(|e| CodeGraphError::Database(e.to_string()))?
            {
                let edge: SerializableEdge = bincode::decode_from_slice(&edge_data, bincode::config::standard())
                    .map_err(|e| CodeGraphError::Database(e.to_string()))?
                    .0;
                edges.push(edge);
            }
        }

        let edges_arc = Arc::new(edges.clone());
        self.edge_cache.insert(node_id, edges_arc);

        Ok(edges)
    }

    /// Get edges incoming to the specified node (where `to == node_id`).
    /// Uses the `to:` index for efficient prefix scanning.
    pub async fn get_edges_to(&self, node_id: NodeId) -> Result<Vec<SerializableEdge>> {
        let to_index_cf = self.get_cf_handle(INDICES_CF)?;
        let edges_cf = self.get_cf_handle(EDGES_CF)?;

        let prefix = format!("to:{}", node_id);
        let mut read_opts = ReadOptions::default();
        read_opts.set_prefix_same_as_start(true);
        read_opts.set_readahead_size(2 * 1024 * 1024);

        let iter = self.db.iterator_cf_opt(
            &to_index_cf,
            read_opts,
            IteratorMode::From(prefix.as_bytes(), rocksdb::Direction::Forward),
        );
        let mut edge_ids: Vec<u64> = Vec::new();
        for item in iter {
            let (key, _) = item.map_err(|e| CodeGraphError::Database(e.to_string()))?;
            if !key.starts_with(prefix.as_bytes()) {
                break;
            }
            if key.len() >= 8 {
                let edge_id_bytes = &key[key.len() - 8..];
                let edge_id = u64::from_be_bytes(edge_id_bytes.try_into().unwrap_or_default());
                edge_ids.push(edge_id);
            }
        }

        let mut edges = Vec::with_capacity(edge_ids.len());
        for edge_id in edge_ids {
            let edge_key = Self::edge_key(edge_id);
            if let Some(edge_data) = self
                .db
                .get_cf(&edges_cf, edge_key)
                .map_err(|e| CodeGraphError::Database(e.to_string()))?
            {
                let edge: SerializableEdge = bincode::decode_from_slice(&edge_data, bincode::config::standard())
                    .map_err(|e| CodeGraphError::Database(e.to_string()))?
                    .0;
                edges.push(edge);
            }
        }

        Ok(edges)
    }

    /// Remove edges matching from->to and optional edge_type; returns number removed.
    pub async fn remove_edges(
        &self,
        from: NodeId,
        to: NodeId,
        edge_type: Option<&str>,
    ) -> Result<usize> {
        let from_index_cf = self.get_cf_handle(INDICES_CF)?;
        let edges_cf = self.get_cf_handle(EDGES_CF)?;
        let indices_cf = self.get_cf_handle(INDICES_CF)?;

        let prefix = format!("from:{}", from);
        let mut read_opts = ReadOptions::default();
        read_opts.set_prefix_same_as_start(true);
        read_opts.set_readahead_size(2 * 1024 * 1024);

        let iter = self.db.iterator_cf_opt(
            &from_index_cf,
            read_opts,
            IteratorMode::From(prefix.as_bytes(), rocksdb::Direction::Forward),
        );

        let mut to_delete: Vec<u64> = Vec::new();
        for item in iter {
            let (key, _) = item.map_err(|e| CodeGraphError::Database(e.to_string()))?;
            if !key.starts_with(prefix.as_bytes()) {
                break;
            }
            if key.len() >= 8 {
                let edge_id_bytes = &key[key.len() - 8..];
                let edge_id = u64::from_be_bytes(edge_id_bytes.try_into().unwrap_or_default());
                // Load edge to inspect
                if let Some(edge_data) = self
                    .db
                    .get_cf(&edges_cf, Self::edge_key(edge_id))
                    .map_err(|e| CodeGraphError::Database(e.to_string()))?
                {
                    let edge: SerializableEdge = bincode::decode_from_slice(&edge_data, bincode::config::standard())
                        .map_err(|e| CodeGraphError::Database(e.to_string()))?
                    .0;
                    let type_match = match edge_type {
                        Some(t) => edge.edge_type == t,
                        None => true,
                    };
                    if edge.to == to && type_match {
                        to_delete.push(edge_id);
                    }
                }
            }
        }

        if to_delete.is_empty() {
            return Ok(0);
        }

        // Delete in a single batch
        self.with_batch(None, |batch| {
            for edge_id in &to_delete {
                batch.delete_cf(&edges_cf, Self::edge_key(*edge_id));
                let from_key = Self::index_edge_key(b"from:", &from.to_string(), *edge_id);
                let to_key = Self::index_edge_key(b"to:", &to.to_string(), *edge_id);
                batch.delete_cf(&indices_cf, from_key);
                batch.delete_cf(&indices_cf, to_key);
            }
            Ok(())
        })?;

        // Invalidate cache for 'from'
        self.edge_cache.remove(&from);
        Ok(to_delete.len())
    }

    pub async fn create_memory_mapped_view<P: AsRef<Path>>(&self, file_path: P) -> Result<()> {
        let file = File::open(&file_path)
            .map_err(|e| CodeGraphError::Database(format!("Failed to open file: {}", e)))?;

        let mmap = unsafe {
            MmapOptions::new()
                .map(&file)
                .map_err(|e| CodeGraphError::Database(format!("Failed to create mmap: {}", e)))?
        };

        let path_str = file_path.as_ref().to_string_lossy().to_string();
        let mut memory_tables = self.memory_tables.write();
        memory_tables.insert(path_str, mmap);

        Ok(())
    }

    // Convenience for tests/ops: list column families
    pub fn list_cf_names(&self) -> Result<Vec<String>> {
        let opts = Options::default();
        rocksdb::DB::list_cf(&opts, &self.db_path)
            .map_err(|e| CodeGraphError::Database(format!("List CF failed: {}", e)))
    }

    pub fn db_path(&self) -> &Path {
        &self.db_path
    }

    pub async fn add_node_tx(&self, tx_id: u64, node: CodeNode) -> Result<()> {
        let node_id = node.id;
        let serializable_node = SerializableCodeNode::from(node.clone());

        let nodes_cf = self.get_cf_handle(NODES_CF)?;
        let indices_cf = self.get_cf_handle(INDICES_CF)?;

        let node_key = Self::node_key(node_id);
        let node_bytes = bincode::encode_to_vec(&serializable_node, bincode::config::standard())
            .map_err(|e| CodeGraphError::Database(e.to_string()))?;

        let name_index_key = Self::index_key(b"name:", node.name.as_str(), node_id);

        self.with_batch(Some(tx_id), |batch| {
            batch.put_cf(&nodes_cf, node_key, node_bytes);
            batch.put_cf(&indices_cf, name_index_key, b"");
            Ok(())
        })?;

        self.read_cache.insert(node_id, Arc::new(node));
        // NodeId is a UUID; skip numeric counter maintenance

        Ok(())
    }

    // Bulk operations (min 1000 ops/batch)
    pub async fn bulk_insert_nodes(&self, nodes: Vec<CodeNode>) -> Result<BulkInsertStats> {
        let mut ops_in_batch: usize = 0;
        let mut batches: usize = 0;
        let nodes_cf = self.get_cf_handle(NODES_CF)?;
        let indices_cf = self.get_cf_handle(INDICES_CF)?;
        for node in nodes.into_iter() {
            let node_id = node.id;
            let serializable_node = SerializableCodeNode::from(node.clone());
            let node_key = Self::node_key(node_id);
            let node_bytes = bincode::encode_to_vec(&serializable_node, bincode::config::standard())
                .map_err(|e| CodeGraphError::Database(e.to_string()))?;
            let name_index_key = Self::index_key(b"name:", node.name.as_str(), node_id);

            self.with_batch(None, |batch| {
                batch.put_cf(&nodes_cf, node_key, node_bytes);
                batch.put_cf(&indices_cf, name_index_key, b"");
                Ok(())
            })?;
            ops_in_batch += 2; // two ops per node
            if ops_in_batch >= 1000 {
                batches += 1;
                ops_in_batch = 0;
            }
            self.read_cache.insert(node_id, Arc::new(node));
        }
        if ops_in_batch > 0 {
            batches += 1;
        }
        Ok(BulkInsertStats {
            batches,
            total_ops: (batches * 1000) as u64,
        })
    }

    pub async fn bulk_insert_edges(&self, edges: Vec<SerializableEdge>) -> Result<BulkInsertStats> {
        let mut ops_in_batch: usize = 0;
        let mut batches: usize = 0;
        let edges_cf = self.get_cf_handle(EDGES_CF)?;
        let indices_cf = self.get_cf_handle(INDICES_CF)?;
        for edge in edges.into_iter() {
            let edge_key = Self::edge_key(edge.id);
            let edge_bytes =
                bincode::encode_to_vec(&edge, bincode::config::standard()).map_err(|e| CodeGraphError::Database(e.to_string()))?;
            let from_index_key = Self::index_edge_key(b"from:", &edge.from.to_string(), edge.id);
            let to_index_key = Self::index_edge_key(b"to:", &edge.to.to_string(), edge.id);
            self.with_batch(None, |batch| {
                batch.put_cf(&edges_cf, edge_key, edge_bytes);
                batch.put_cf(&indices_cf, from_index_key, b"");
                batch.put_cf(&indices_cf, to_index_key, b"");
                Ok(())
            })?;
            ops_in_batch += 3;
            if ops_in_batch >= 1000 {
                batches += 1;
                ops_in_batch = 0;
            }
        }
        if ops_in_batch > 0 {
            batches += 1;
        }
        Ok(BulkInsertStats {
            batches,
            total_ops: (batches * 1000) as u64,
        })
    }

    // Backup and restore using timestamped directory snapshots
    pub fn backup_snapshot<P: AsRef<Path>>(&self, backups_root: P) -> Result<std::path::PathBuf> {
        use chrono::Utc;
        use std::fs;
        // writes committed immediately
        self.db
            .flush()
            .map_err(|e| CodeGraphError::Database(e.to_string()))?;
        fs::create_dir_all(&backups_root)?;
        let ts = Utc::now().format("%Y%m%d%H%M%S");
        let backup_dir = backups_root.as_ref().join(format!("snapshot-{}", ts));
        fs::create_dir_all(&backup_dir)?;
        copy_dir_all(&self.db_path, &backup_dir)
            .map_err(|e| CodeGraphError::Database(format!("Backup copy failed: {}", e)))?;
        Ok(backup_dir)
    }

    pub fn restore_from_snapshot<P: AsRef<Path>, Q: AsRef<Path>>(
        snapshot_dir: P,
        dest_path: Q,
    ) -> Result<()> {
        use std::fs;
        if dest_path.as_ref().exists() {
            fs::remove_dir_all(&dest_path)?;
        }
        fs::create_dir_all(&dest_path)?;
        copy_dir_all(&snapshot_dir, &dest_path)
            .map_err(|e| CodeGraphError::Database(format!("Restore copy failed: {}", e)))?;
        Ok(())
    }
}

impl Clone for HighPerformanceRocksDbStorage {
    fn clone(&self) -> Self {
        use std::sync::atomic::Ordering;
        Self {
            db: self.db.clone(),
            db_path: self.db_path.clone(),
            read_cache: self.read_cache.clone(),
            edge_cache: self.edge_cache.clone(),
            edge_counter: AtomicU64::new(self.edge_counter.load(Ordering::Relaxed)),
            memory_tables: self.memory_tables.clone(),
            batching_config: self.batching_config.clone(),
            read_coalescer: self.read_coalescer.clone(),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct BulkInsertStats {
    pub batches: usize,
    pub total_ops: u64,
}

fn copy_dir_all<P: AsRef<Path>, Q: AsRef<Path>>(src: P, dst: Q) -> std::io::Result<()> {
    use std::fs;
    fs::create_dir_all(&dst)?;
    for entry in fs::read_dir(&src)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let src_path = entry.path();
        let dst_path = dst.as_ref().join(entry.file_name());
        if file_type.is_dir() {
            copy_dir_all(&src_path, &dst_path)?;
        } else if file_type.is_file() {
            fs::copy(&src_path, &dst_path)?;
        }
    }
    Ok(())
}

#[async_trait]
impl GraphStore for HighPerformanceRocksDbStorage {
    async fn add_node(&mut self, node: CodeNode) -> Result<()> {
        let node_id = node.id;
        self.add_node_inner(&node)?;
        let node_arc = std::sync::Arc::new(node);
        self.read_cache.insert(node_id, node_arc);
        Ok(())
    }

    async fn get_node(&self, id: NodeId) -> Result<Option<CodeNode>> {
        if let Some(cached) = self.read_cache.get(&id) {
            return Ok(Some(cached.as_ref().clone()));
        }

        // Delegate to read coalescer for batched DB access
        self.read_coalescer.get_node(id)
    }

    async fn update_node(&mut self, node: CodeNode) -> Result<()> {
        self.add_node(node).await
    }

    async fn remove_node(&mut self, id: NodeId) -> Result<()> {
        self.remove_node_inner(id)?;
        self.read_cache.remove(&id);
        self.edge_cache.remove(&id);
        Ok(())
    }

    async fn find_nodes_by_name(&self, name: &str) -> Result<Vec<CodeNode>> {
        let ids = self.scan_node_ids_by_name(name)?;
        let mut out = Vec::new();
        for id in ids {
            if let Some(n) = self.get_node(id).await? {
                out.push(n);
            }
        }
        Ok(out)
    }
}

pub type RocksDbStorage = HighPerformanceRocksDbStorage;
