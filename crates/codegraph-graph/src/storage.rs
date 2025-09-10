use async_trait::async_trait;
use codegraph_core::{CodeGraphError, CodeNode, GraphStore, NodeId, Result};
use rocksdb::{
    BlockBasedOptions, ColumnFamily, ColumnFamilyDescriptor, DBWithThreadMode, IteratorMode,
    MultiThreaded, Options, ReadOptions, SliceTransform, WriteBatch, WriteOptions, Cache, DBCompressionType
};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    path::Path,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
};
use parking_lot::{RwLock, Mutex};
use dashmap::DashMap;
use memmap2::{Mmap, MmapOptions};
use std::fs::File;
use chrono;
use std::time::{Duration, Instant};

use crate::io_batcher::{BatchingConfig, ReadCoalescer, WriteBatchOptimizer};

type DB = DBWithThreadMode<MultiThreaded>;

const NODES_CF: &str = "nodes";
const EDGES_CF: &str = "edges";
const NODE_NAME_INDEX_CF: &str = "node_name_idx";
const EDGE_FROM_INDEX_CF: &str = "edge_from_idx";
const EDGE_TO_INDEX_CF: &str = "edge_to_idx";
const METADATA_CF: &str = "metadata";

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SerializableCodeNode {
    pub id: NodeId,
    pub name: String,
    pub node_type: String,
    pub file_path: Option<String>,
    pub start_line: Option<u32>,
    pub end_line: Option<u32>,
    pub metadata: HashMap<String, String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
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
            name: node.name,
            node_type: node.node_type,
            file_path: node.file_path,
            start_line: node.start_line,
            end_line: node.end_line,
            metadata: node.metadata.attributes,
        }
    }
}

impl From<SerializableCodeNode> for CodeNode {
    fn from(node: SerializableCodeNode) -> Self {
        use codegraph_core::Metadata;
        let now = chrono::Utc::now();
        Self {
            id: node.id,
            name: node.name,
            node_type: node.node_type,
            file_path: node.file_path,
            start_line: node.start_line,
            end_line: node.end_line,
            metadata: Metadata {
                attributes: node.metadata,
                created_at: now,
                updated_at: now,
            },
        }
    }
}

pub struct HighPerformanceRocksDbStorage {
    db: Arc<DB>,
    read_cache: Arc<DashMap<NodeId, Arc<CodeNode>>>,
    edge_cache: Arc<DashMap<NodeId, Arc<Vec<SerializableEdge>>>>,
    node_counter: AtomicU64,
    edge_counter: AtomicU64,
    write_options: WriteOptions,
    read_options: ReadOptions,
    memory_tables: Arc<RwLock<HashMap<String, Mmap>>>,
    batch_writes: Arc<Mutex<WriteBatch>>,
    batching_config: BatchingConfig,
    write_optimizer: Arc<Mutex<WriteBatchOptimizer>>,
    read_coalescer: ReadCoalescer,
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
        // Reduce syscall overhead where available
        db_opts.set_use_direct_reads(true);
        db_opts.set_use_direct_io_for_flush_and_compaction(true);
        
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
        block_opts.set_prefix_extractor(prefix_extractor);
        
        db_opts.set_block_based_table_factory(&block_opts);
        
        let cf_descriptors = vec![
            Self::create_nodes_cf_descriptor(),
            Self::create_edges_cf_descriptor(),
            Self::create_index_cf_descriptor(NODE_NAME_INDEX_CF),
            Self::create_index_cf_descriptor(EDGE_FROM_INDEX_CF),
            Self::create_index_cf_descriptor(EDGE_TO_INDEX_CF),
            Self::create_metadata_cf_descriptor(),
        ];
        
        let db = DB::open_cf_descriptors(&db_opts, &path, cf_descriptors)
            .map_err(|e| CodeGraphError::Database(format!("Failed to open database: {}", e)))?;
        
        let mut write_options = WriteOptions::default();
        write_options.set_sync(false);
        write_options.disable_wal(false);
        
        let mut read_options = ReadOptions::default();
        read_options.set_verify_checksums(false);
        read_options.set_fill_cache(true);
        read_options.set_prefix_same_as_start(true);
        read_options.set_readahead_size(2 * 1024 * 1024);

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
            read_cache: read_cache,
            edge_cache: Arc::new(DashMap::with_capacity(50_000)),
            node_counter: AtomicU64::new(1),
            edge_counter: AtomicU64::new(1),
            write_options,
            read_options,
            memory_tables: Arc::new(RwLock::new(HashMap::new())),
            batch_writes: Arc::new(Mutex::new(WriteBatch::default())),
            batching_config: batching_config.clone(),
            write_optimizer: Arc::new(Mutex::new(WriteBatchOptimizer::new(&batching_config))),
            read_coalescer,
        };
        
        storage.initialize_counters()?;
        
        Ok(storage)
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
    
    fn create_index_cf_descriptor(name: &str) -> ColumnFamilyDescriptor {
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
        
        ColumnFamilyDescriptor::new(name, opts)
    }
    
    fn create_metadata_cf_descriptor() -> ColumnFamilyDescriptor {
        let mut opts = Options::default();
        opts.set_write_buffer_size(16 * 1024 * 1024);
        opts.set_compression_type(DBCompressionType::Zstd);
        
        ColumnFamilyDescriptor::new(METADATA_CF, opts)
    }
    
    fn initialize_counters(&self) -> Result<()> {
        let metadata_cf = self.get_cf_handle(METADATA_CF)?;
        
        if let Some(node_count_bytes) = self.db.get_cf(&metadata_cf, b"node_counter")?
            .map_err(|e| CodeGraphError::Database(e.to_string()))? {
            if let Ok(count) = bincode::deserialize::<u64>(&node_count_bytes) {
                self.node_counter.store(count, Ordering::Relaxed);
            }
        }
        
        if let Some(edge_count_bytes) = self.db.get_cf(&metadata_cf, b"edge_counter")?
            .map_err(|e| CodeGraphError::Database(e.to_string()))? {
            if let Ok(count) = bincode::deserialize::<u64>(&edge_count_bytes) {
                self.edge_counter.store(count, Ordering::Relaxed);
            }
        }
        
        Ok(())
    }
    
    fn get_cf_handle(&self, name: &str) -> Result<&ColumnFamily> {
        self.db.cf_handle(name)
            .ok_or_else(|| CodeGraphError::Database(format!("Column family '{}' not found", name)))
    }
    
    fn node_key(id: NodeId) -> [u8; 8] {
        id.to_be_bytes()
    }
    
    fn edge_key(id: u64) -> [u8; 8] {
        id.to_be_bytes()
    }
    
    fn index_key(prefix: &[u8], value: &str, id: u64) -> Vec<u8> {
        let mut key = Vec::with_capacity(prefix.len() + value.len() + 8);
        key.extend_from_slice(prefix);
        key.extend_from_slice(value.as_bytes());
        key.extend_from_slice(&id.to_be_bytes());
        key
    }
    
    pub fn flush_batch_writes(&self) -> Result<()> {
        let mut batch = self.batch_writes.lock();
        if !batch.is_empty() {
            let start = Instant::now();
            let ops = batch.len();
            self.db
                .write_opt(&*batch, &self.write_options)
                .map_err(|e| CodeGraphError::Database(e.to_string()))?;
            batch.clear();
            let mut opt = self.write_optimizer.lock();
            opt.on_flushed(ops, start.elapsed());
        }
        Ok(())
    }

    fn maybe_flush_writes(&self) -> Result<()> {
        let threshold = { self.write_optimizer.lock().ops_threshold };
        let interval = self.batching_config.write_flush_interval;
        let mut do_flush = false;
        {
            let batch = self.batch_writes.lock();
            if !batch.is_empty() && batch.len() >= threshold {
                do_flush = true;
            }
        }
        if !do_flush {
            let mut opt = self.write_optimizer.lock();
            if opt.should_time_flush(interval) {
                drop(opt);
                return self.flush_batch_writes();
            }
            return Ok(());
        }
        self.flush_batch_writes()
    }
    
    pub async fn add_edge(&self, edge: SerializableEdge) -> Result<()> {
        let edge_id = self.edge_counter.fetch_add(1, Ordering::Relaxed);
        let mut edge = edge;
        edge.id = edge_id;
        
        let edges_cf = self.get_cf_handle(EDGES_CF)?;
        let from_index_cf = self.get_cf_handle(EDGE_FROM_INDEX_CF)?;
        let to_index_cf = self.get_cf_handle(EDGE_TO_INDEX_CF)?;
        
        let edge_key = Self::edge_key(edge_id);
        let edge_bytes = bincode::serialize(&edge)
            .map_err(|e| CodeGraphError::Database(e.to_string()))?;
        
        let from_index_key = Self::index_key(b"from:", &edge.from.to_string(), edge_id);
        let to_index_key = Self::index_key(b"to:", &edge.to.to_string(), edge_id);
        
        {
            let mut batch = self.batch_writes.lock();
            batch.put_cf(&edges_cf, edge_key, edge_bytes);
            batch.put_cf(&from_index_cf, from_index_key, b"");
            batch.put_cf(&to_index_cf, to_index_key, b"");
        }
        self.maybe_flush_writes()?;
        
        self.edge_cache.remove(&edge.from);
        
        Ok(())
    }
    
    pub async fn get_edges_from(&self, node_id: NodeId) -> Result<Vec<SerializableEdge>> {
        if let Some(cached) = self.edge_cache.get(&node_id) {
            return Ok(cached.as_ref().clone());
        }
        
        let from_index_cf = self.get_cf_handle(EDGE_FROM_INDEX_CF)?;
        let edges_cf = self.get_cf_handle(EDGES_CF)?;
        
        let prefix = format!("from:{}", node_id);
        let mut read_opts = self.read_options.clone();
        read_opts.set_prefix_same_as_start(true);
        
        let iter = self.db.prefix_iterator_cf(&from_index_cf, &prefix);
        let mut edges = Vec::new();
        
        for item in iter {
            let (key, _) = item.map_err(|e| CodeGraphError::Database(e.to_string()))?;
            
            if key.len() >= 8 {
                let edge_id_bytes = &key[key.len()-8..];
                let edge_id = u64::from_be_bytes(edge_id_bytes.try_into().unwrap_or_default());
                
                let edge_key = Self::edge_key(edge_id);
                if let Some(edge_data) = self.db.get_cf(&edges_cf, edge_key)
                    .map_err(|e| CodeGraphError::Database(e.to_string()))? {
                    
                    let edge: SerializableEdge = bincode::deserialize(&edge_data)
                        .map_err(|e| CodeGraphError::Database(e.to_string()))?;
                    
                    edges.push(edge);
                }
            }
        }
        
        let edges_arc = Arc::new(edges.clone());
        self.edge_cache.insert(node_id, edges_arc);
        
        Ok(edges)
    }
    
    pub async fn create_memory_mapped_view<P: AsRef<Path>>(&self, file_path: P) -> Result<()> {
        let file = File::open(&file_path)
            .map_err(|e| CodeGraphError::Database(format!("Failed to open file: {}", e)))?;
        
        let mmap = unsafe {
            MmapOptions::new().map(&file)
                .map_err(|e| CodeGraphError::Database(format!("Failed to create mmap: {}", e)))?
        };
        
        let path_str = file_path.as_ref().to_string_lossy().to_string();
        let mut memory_tables = self.memory_tables.write();
        memory_tables.insert(path_str, mmap);
        
        Ok(())
    }
}

#[async_trait]
impl GraphStore for HighPerformanceRocksDbStorage {
    async fn add_node(&mut self, node: CodeNode) -> Result<()> {
        let node_id = node.id;
        let serializable_node = SerializableCodeNode::from(node.clone());
        
        let nodes_cf = self.get_cf_handle(NODES_CF)?;
        let name_index_cf = self.get_cf_handle(NODE_NAME_INDEX_CF)?;
        
        let node_key = Self::node_key(node_id);
        let node_bytes = bincode::serialize(&serializable_node)
            .map_err(|e| CodeGraphError::Database(e.to_string()))?;
        
        let name_index_key = Self::index_key(b"name:", &node.name, node_id);
        
        let mut batch = self.batch_writes.lock();
        batch.put_cf(&nodes_cf, node_key, node_bytes);
        batch.put_cf(&name_index_cf, name_index_key, b"");
        
        if batch.len() > 1000 {
            self.db.write_opt(&*batch, &self.write_options)
                .map_err(|e| CodeGraphError::Database(e.to_string()))?;
            batch.clear();
        }
        
        let node_arc = Arc::new(node);
        self.read_cache.insert(node_id, node_arc);
        
        self.node_counter.fetch_max(node_id + 1, Ordering::Relaxed);
        
        Ok(())
    }
    
    async fn get_node(&self, id: NodeId) -> Result<Option<CodeNode>> {
        if let Some(cached) = self.read_cache.get(&id) {
            return Ok(Some(cached.as_ref().clone()));
        }
        
        let nodes_cf = self.get_cf_handle(NODES_CF)?;
        let node_key = Self::node_key(id);
        
        match self.db.get_cf(&nodes_cf, node_key) {
            Ok(Some(data)) => {
                let serializable_node: SerializableCodeNode = bincode::deserialize(&data)
                    .map_err(|e| CodeGraphError::Database(e.to_string()))?;
                
                let node = CodeNode::from(serializable_node);
                let node_arc = Arc::new(node.clone());
                self.read_cache.insert(id, node_arc);
                
                Ok(Some(node))
            }
            Ok(None) => Ok(None),
            Err(e) => Err(CodeGraphError::Database(e.to_string())),
        }
    }
    
    async fn update_node(&mut self, node: CodeNode) -> Result<()> {
        self.add_node(node).await
    }
    
    async fn remove_node(&mut self, id: NodeId) -> Result<()> {
        let nodes_cf = self.get_cf_handle(NODES_CF)?;
        let name_index_cf = self.get_cf_handle(NODE_NAME_INDEX_CF)?;
        
        if let Some(node) = self.get_node(id).await? {
            let node_key = Self::node_key(id);
            let name_index_key = Self::index_key(b"name:", &node.name, id);
            
            let mut batch = self.batch_writes.lock();
            batch.delete_cf(&nodes_cf, node_key);
            batch.delete_cf(&name_index_cf, name_index_key);
            
            self.db.write_opt(&*batch, &self.write_options)
                .map_err(|e| CodeGraphError::Database(e.to_string()))?;
            batch.clear();
        }
        
        self.read_cache.remove(&id);
        self.edge_cache.remove(&id);
        
        Ok(())
    }
    
    async fn find_nodes_by_name(&self, name: &str) -> Result<Vec<CodeNode>> {
        let name_index_cf = self.get_cf_handle(NODE_NAME_INDEX_CF)?;
        
        let prefix = format!("name:{}", name);
        let mut read_opts = self.read_options.clone();
        read_opts.set_prefix_same_as_start(true);
        
        let iter = self.db.prefix_iterator_cf(&name_index_cf, &prefix);
        let mut nodes = Vec::new();
        
        for item in iter {
            let (key, _) = item.map_err(|e| CodeGraphError::Database(e.to_string()))?;
            
            if key.len() >= 8 {
                let node_id_bytes = &key[key.len()-8..];
                let node_id = u64::from_be_bytes(node_id_bytes.try_into().unwrap_or_default());
                
                if let Some(node) = self.get_node(node_id).await? {
                    nodes.push(node);
                }
            }
        }
        
        Ok(nodes)
    }
}

pub type RocksDbStorage = HighPerformanceRocksDbStorage;
