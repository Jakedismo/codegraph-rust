use async_trait::async_trait;
use chrono::{DateTime, Utc};
use codegraph_core::{
    ChangeType, Checkpoint, CodeGraphError, CodeNode, CrashRecovery, IsolationLevel, NodeId,
    NodeVersion, Result, Snapshot, SnapshotId, Transaction, TransactionId, TransactionManager,
    TransactionStatus, Version, VersionDiff, VersionId, VersionedStore, WriteAheadLog,
    WriteAheadLogEntry, WriteOperation,
};
use dashmap::DashMap;
use parking_lot::{Mutex, RwLock};
use rocksdb::{
    BlockBasedOptions, Cache, ColumnFamily, ColumnFamilyDescriptor, DBCompressionType,
    DBWithThreadMode, IteratorMode, MultiThreaded, Options, ReadOptions, WriteBatch, WriteOptions,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{HashMap, HashSet},
    path::Path,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::Duration,
};
use tokio::time::timeout;
use uuid::Uuid;

type DB = DBWithThreadMode<MultiThreaded>;

const SNAPSHOTS_CF: &str = "snapshots";
const VERSIONS_CF: &str = "versions";
const NODE_VERSIONS_CF: &str = "node_versions";
const TRANSACTIONS_CF: &str = "transactions";
const WAL_CF: &str = "write_ahead_log";
const CHECKPOINTS_CF: &str = "checkpoints";
const VERSION_TAGS_CF: &str = "version_tags";
const SNAPSHOT_REFS_CF: &str = "snapshot_refs";
const CONTENT_STORE_CF: &str = "content_store";

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoredSnapshot {
    snapshot: Snapshot,
    content_hashes: HashMap<NodeId, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ContentBlock {
    hash: String,
    data: Vec<u8>,
    created_at: DateTime<Utc>,
    ref_count: u64,
}

pub struct VersionedRocksDbStorage {
    db: Arc<DB>,

    active_transactions: Arc<DashMap<TransactionId, Arc<RwLock<Transaction>>>>,

    snapshot_cache: Arc<DashMap<SnapshotId, Arc<Snapshot>>>,
    version_cache: Arc<DashMap<VersionId, Arc<Version>>>,
    content_cache: Arc<DashMap<String, Arc<Vec<u8>>>>,

    wal_sequence: AtomicU64,
    transaction_counter: AtomicU64,

    write_options: WriteOptions,
    read_options: ReadOptions,

    commit_locks: Arc<DashMap<NodeId, Arc<Mutex<()>>>>,
}

impl VersionedRocksDbStorage {
    pub async fn new<P: AsRef<Path>>(path: P) -> Result<Self> {
        let mut db_opts = Options::default();

        db_opts.create_if_missing(true);
        db_opts.create_missing_column_families(true);

        db_opts.set_max_background_jobs(std::cmp::max(2, num_cpus::get() as i32));
        db_opts.set_max_subcompactions(4);

        db_opts.set_write_buffer_size(256 * 1024 * 1024);
        db_opts.set_max_write_buffer_number(8);
        db_opts.set_min_write_buffer_number_to_merge(2);

        db_opts.set_level_zero_file_num_compaction_trigger(4);
        db_opts.set_level_zero_slowdown_writes_trigger(20);
        db_opts.set_level_zero_stop_writes_trigger(36);

        db_opts.set_target_file_size_base(128 * 1024 * 1024);
        db_opts.set_target_file_size_multiplier(2);

        db_opts.set_max_bytes_for_level_base(512 * 1024 * 1024);
        db_opts.set_max_bytes_for_level_multiplier(10.0);

        db_opts.set_bytes_per_sync(2 * 1024 * 1024);
        db_opts.set_wal_bytes_per_sync(2 * 1024 * 1024);

        db_opts.set_compression_type(DBCompressionType::Lz4);
        db_opts.set_bottommost_compression_type(DBCompressionType::Zstd);

        let block_cache = Cache::new_lru_cache(1024 * 1024 * 1024); // 1GB

        let mut block_opts = BlockBasedOptions::default();
        block_opts.set_block_size(64 * 1024);
        block_opts.set_block_cache(&block_cache);
        block_opts.set_cache_index_and_filter_blocks(true);
        block_opts.set_pin_l0_filter_and_index_blocks_in_cache(true);
        block_opts.set_bloom_filter(12.0, false);

        db_opts.set_block_based_table_factory(&block_opts);

        let cf_descriptors = vec![
            Self::create_cf_descriptor(SNAPSHOTS_CF, 128),
            Self::create_cf_descriptor(VERSIONS_CF, 64),
            Self::create_cf_descriptor(NODE_VERSIONS_CF, 256),
            Self::create_cf_descriptor(TRANSACTIONS_CF, 64),
            Self::create_cf_descriptor(WAL_CF, 512),
            Self::create_cf_descriptor(CHECKPOINTS_CF, 32),
            Self::create_cf_descriptor(VERSION_TAGS_CF, 32),
            Self::create_cf_descriptor(SNAPSHOT_REFS_CF, 64),
            Self::create_cf_descriptor(CONTENT_STORE_CF, 1024),
        ];

        let db = DB::open_cf_descriptors(&db_opts, &path, cf_descriptors)
            .map_err(|e| CodeGraphError::Database(format!("Failed to open database: {}", e)))?;

        let mut write_options = WriteOptions::default();
        write_options.set_sync(false);
        write_options.disable_wal(false);

        let mut read_options = ReadOptions::default();
        read_options.set_verify_checksums(true);
        read_options.fill_cache(true);

        let mut storage = Self {
            db: Arc::new(db),
            active_transactions: Arc::new(DashMap::new()),
            snapshot_cache: Arc::new(DashMap::with_capacity(1000)),
            version_cache: Arc::new(DashMap::with_capacity(1000)),
            content_cache: Arc::new(DashMap::with_capacity(10000)),
            wal_sequence: AtomicU64::new(1),
            transaction_counter: AtomicU64::new(1),
            write_options,
            read_options,
            commit_locks: Arc::new(DashMap::new()),
        };

        storage.initialize_counters().await?;
        storage.recover_from_crash().await?;

        Ok(storage)
    }

    fn create_cf_descriptor(name: &str, write_buffer_mb: usize) -> ColumnFamilyDescriptor {
        let mut opts = Options::default();
        opts.set_write_buffer_size(write_buffer_mb * 1024 * 1024);
        opts.set_max_write_buffer_number(4);
        opts.set_compression_type(DBCompressionType::Lz4);

        let mut block_opts = BlockBasedOptions::default();
        let cache = Cache::new_lru_cache((write_buffer_mb * 2) * 1024 * 1024);
        block_opts.set_block_cache(&cache);
        block_opts.set_block_size(32 * 1024);
        block_opts.set_bloom_filter(10.0, false);
        opts.set_block_based_table_factory(&block_opts);

        ColumnFamilyDescriptor::new(name, opts)
    }

    async fn initialize_counters(&self) -> Result<()> {
        let wal_cf = self.get_cf_handle(WAL_CF)?;
        let transactions_cf = self.get_cf_handle(TRANSACTIONS_CF)?;

        let mut wal_iter = self.db.iterator_cf(&wal_cf, IteratorMode::End);
        if let Some(Ok((key, _))) = wal_iter.next() {
            if let Ok(seq_str) = std::str::from_utf8(&key) {
                if let Ok(seq) = seq_str.parse::<u64>() {
                    self.wal_sequence.store(seq + 1, Ordering::Relaxed);
                }
            }
        }

        let mut tx_iter = self.db.iterator_cf(&transactions_cf, IteratorMode::End);
        if let Some(Ok((key, _))) = tx_iter.next() {
            if let Ok(tx_id) = Uuid::parse_str(&String::from_utf8_lossy(&key)) {
                self.transaction_counter
                    .store(tx_id.as_u128() as u64 + 1, Ordering::Relaxed);
            }
        }

        Ok(())
    }

    fn get_cf_handle(&self, name: &str) -> Result<std::sync::Arc<rocksdb::BoundColumnFamily<'_>>> {
        self.db
            .cf_handle(name)
            .ok_or_else(|| CodeGraphError::Database(format!("Column family '{}' not found", name)))
    }

    fn compute_content_hash(content: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(content);
        format!("{:x}", hasher.finalize())
    }

    async fn store_content(&self, content: &[u8]) -> Result<String> {
        let hash = Self::compute_content_hash(content);

        if self.content_cache.contains_key(&hash) {
            return Ok(hash);
        }

        let content_cf = self.get_cf_handle(CONTENT_STORE_CF)?;

        let existing = self
            .db
            .get_cf(&content_cf, &hash)
            .map_err(|e| CodeGraphError::Database(e.to_string()))?;

        if existing.is_some() {
            let content_arc = Arc::new(content.to_vec());
            self.content_cache.insert(hash.clone(), content_arc);
            return Ok(hash);
        }

        let content_block = ContentBlock {
            hash: hash.clone(),
            data: content.to_vec(),
            created_at: Utc::now(),
            ref_count: 1,
        };

        let serialized = bincode::serialize(&content_block)
            .map_err(|e| CodeGraphError::Database(e.to_string()))?;

        self.db
            .put_cf(&content_cf, &hash, serialized)
            .map_err(|e| CodeGraphError::Database(e.to_string()))?;

        let content_arc = Arc::new(content.to_vec());
        self.content_cache.insert(hash.clone(), content_arc);

        Ok(hash)
    }

    async fn get_content(&self, hash: &str) -> Result<Option<Vec<u8>>> {
        if let Some(cached) = self.content_cache.get(hash) {
            return Ok(Some(cached.as_ref().clone()));
        }

        let content_cf = self.get_cf_handle(CONTENT_STORE_CF)?;

        match self.db.get_cf(&content_cf, hash) {
            Ok(Some(data)) => {
                let content_block: ContentBlock = bincode::deserialize(&data)
                    .map_err(|e| CodeGraphError::Database(e.to_string()))?;

                let content_arc = Arc::new(content_block.data.clone());
                self.content_cache.insert(hash.to_string(), content_arc);

                Ok(Some(content_block.data))
            }
            Ok(None) => Ok(None),
            Err(e) => Err(CodeGraphError::Database(e.to_string())),
        }
    }

    fn get_commit_lock(&self, node_id: NodeId) -> Arc<Mutex<()>> {
        self.commit_locks
            .entry(node_id)
            .or_insert_with(|| Arc::new(Mutex::new(())))
            .clone()
    }

    async fn read_node_at_snapshot(
        &self,
        node_id: NodeId,
        snapshot_id: SnapshotId,
    ) -> Result<Option<CodeNode>> {
        let snapshot = match self.get_snapshot(snapshot_id).await? {
            Some(s) => s,
            None => return Ok(None),
        };

        let content_hash = match snapshot.node_versions.get(&node_id) {
            Some(hash) => hash,
            None => return Ok(None),
        };

        let content = match self.get_content(content_hash).await? {
            Some(c) => c,
            None => return Ok(None),
        };

        let node: CodeNode =
            bincode::deserialize(&content).map_err(|e| CodeGraphError::Database(e.to_string()))?;

        Ok(Some(node))
    }
}

#[async_trait]
impl VersionedStore for VersionedRocksDbStorage {
    async fn create_snapshot(&mut self, transaction_id: TransactionId) -> Result<SnapshotId> {
        let transaction = self
            .active_transactions
            .get(&transaction_id)
            .ok_or_else(|| CodeGraphError::Transaction("Transaction not found".to_string()))?
            .clone();

        // Copy required state out of the lock before awaiting
        let (base_snapshot_id, write_set) = {
            let tx = transaction.read();
            (tx.snapshot_id, tx.write_set.clone())
        };
        let snapshot_id = SnapshotId::new_v4();

        let mut node_versions = HashMap::new();
        for (node_id, write_op) in &write_set {
            match write_op {
                WriteOperation::Insert(_id) => {
                    return Err(CodeGraphError::Transaction(
                        "Invalid insert operation".to_string(),
                    ));
                }
                WriteOperation::Update {
                    new_content_hash, ..
                } => {
                    let hash = new_content_hash.clone();
                    node_versions.insert(*node_id, hash);
                }
                WriteOperation::Delete(_) => {}
            }
        }
        // Now safe to await
        if let Some(parent_snapshot_id) = self.get_snapshot(base_snapshot_id).await? {
            for (node_id, hash) in &parent_snapshot_id.node_versions {
                if !node_versions.contains_key(node_id) && !write_set.contains_key(node_id) {
                    node_versions.insert(*node_id, hash.clone());
                }
            }
        }

        let snapshot = Snapshot {
            id: snapshot_id,
            created_at: Utc::now(),
            transaction_id,
            node_versions,
            parent_snapshot: Some(base_snapshot_id),
            children_snapshots: Vec::new(),
            ref_count: 1,
        };

        let snapshots_cf = self.get_cf_handle(SNAPSHOTS_CF)?;
        let stored_snapshot = StoredSnapshot {
            snapshot: snapshot.clone(),
            content_hashes: snapshot.node_versions.clone(),
        };

        let serialized = bincode::serialize(&stored_snapshot)
            .map_err(|e| CodeGraphError::Database(e.to_string()))?;

        self.db
            .put_cf(&snapshots_cf, snapshot_id.as_bytes(), serialized)
            .map_err(|e| CodeGraphError::Database(e.to_string()))?;

        let snapshot_arc = Arc::new(snapshot);
        self.snapshot_cache.insert(snapshot_id, snapshot_arc);

        Ok(snapshot_id)
    }

    async fn get_snapshot(&self, snapshot_id: SnapshotId) -> Result<Option<Snapshot>> {
        if let Some(cached) = self.snapshot_cache.get(&snapshot_id) {
            return Ok(Some(cached.as_ref().clone()));
        }

        let snapshots_cf = self.get_cf_handle(SNAPSHOTS_CF)?;

        match self.db.get_cf(&snapshots_cf, snapshot_id.as_bytes()) {
            Ok(Some(data)) => {
                let stored_snapshot: StoredSnapshot = bincode::deserialize(&data)
                    .map_err(|e| CodeGraphError::Database(e.to_string()))?;

                let snapshot_arc = Arc::new(stored_snapshot.snapshot.clone());
                self.snapshot_cache.insert(snapshot_id, snapshot_arc);

                Ok(Some(stored_snapshot.snapshot))
            }
            Ok(None) => Ok(None),
            Err(e) => Err(CodeGraphError::Database(e.to_string())),
        }
    }

    async fn create_version(
        &mut self,
        name: String,
        description: String,
        author: String,
        snapshot_id: SnapshotId,
        parent_versions: Vec<VersionId>,
    ) -> Result<VersionId> {
        let version_id = VersionId::new_v4();

        let version = Version {
            id: version_id,
            name,
            description,
            author,
            created_at: Utc::now(),
            snapshot_id,
            parent_versions,
            tags: Vec::new(),
            metadata: HashMap::new(),
        };

        let versions_cf = self.get_cf_handle(VERSIONS_CF)?;
        let serialized =
            bincode::serialize(&version).map_err(|e| CodeGraphError::Database(e.to_string()))?;

        self.db
            .put_cf(&versions_cf, version_id.as_bytes(), serialized)
            .map_err(|e| CodeGraphError::Database(e.to_string()))?;

        let version_arc = Arc::new(version);
        self.version_cache.insert(version_id, version_arc);

        Ok(version_id)
    }

    async fn get_version(&self, version_id: VersionId) -> Result<Option<Version>> {
        if let Some(cached) = self.version_cache.get(&version_id) {
            return Ok(Some(cached.as_ref().clone()));
        }

        let versions_cf = self.get_cf_handle(VERSIONS_CF)?;

        match self.db.get_cf(&versions_cf, version_id.as_bytes()) {
            Ok(Some(data)) => {
                let version: Version = bincode::deserialize(&data)
                    .map_err(|e| CodeGraphError::Database(e.to_string()))?;

                let version_arc = Arc::new(version.clone());
                self.version_cache.insert(version_id, version_arc);

                Ok(Some(version))
            }
            Ok(None) => Ok(None),
            Err(e) => Err(CodeGraphError::Database(e.to_string())),
        }
    }

    async fn list_versions(&self, limit: Option<u32>) -> Result<Vec<Version>> {
        let versions_cf = self.get_cf_handle(VERSIONS_CF)?;
        let iter = self.db.iterator_cf(&versions_cf, IteratorMode::Start);

        let mut versions = Vec::new();
        let limit = limit.unwrap_or(1000) as usize;

        for item in iter.take(limit) {
            let (_, value) = item.map_err(|e| CodeGraphError::Database(e.to_string()))?;
            let version: Version = bincode::deserialize(&value)
                .map_err(|e| CodeGraphError::Database(e.to_string()))?;
            versions.push(version);
        }

        versions.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        Ok(versions)
    }

    async fn tag_version(&mut self, version_id: VersionId, tag: String) -> Result<()> {
        {
            let version_tags_cf = self.get_cf_handle(VERSION_TAGS_CF)?;
            let tag_key = format!("tag:{}", tag);
            self.db
                .put_cf(&version_tags_cf, tag_key.as_bytes(), version_id.as_bytes())
                .map_err(|e| CodeGraphError::Database(e.to_string()))?;
        }
        if let Some(mut version) = self.get_version(version_id).await? {
            version.tags.push(tag);
            let versions_cf = self.get_cf_handle(VERSIONS_CF)?;
            let serialized = bincode::serialize(&version)
                .map_err(|e| CodeGraphError::Database(e.to_string()))?;
            self.db
                .put_cf(&versions_cf, version_id.as_bytes(), serialized)
                .map_err(|e| CodeGraphError::Database(e.to_string()))?;
            let version_arc = Arc::new(version);
            self.version_cache.insert(version_id, version_arc);
        }
        Ok(())
    }

    async fn get_version_by_tag(&self, tag: &str) -> Result<Option<Version>> {
        let version_id_opt = {
            let version_tags_cf = self.get_cf_handle(VERSION_TAGS_CF)?;
            let tag_key = format!("tag:{}", tag);
            match self.db.get_cf(&version_tags_cf, tag_key.as_bytes()) {
                Ok(Some(version_id_bytes)) => {
                    let version_id_str = String::from_utf8_lossy(&version_id_bytes);
                    Some(
                        Uuid::parse_str(&version_id_str)
                            .map_err(|e| CodeGraphError::Database(e.to_string()))?,
                    )
                }
                Ok(None) => None,
                Err(e) => return Err(CodeGraphError::Database(e.to_string())),
            }
        };
        if let Some(version_id) = version_id_opt {
            return self.get_version(version_id).await;
        }
        Ok(None)
    }

    async fn merge_versions(
        &mut self,
        _base_version: VersionId,
        _source_version: VersionId,
        _target_version: VersionId,
        author: String,
        message: String,
    ) -> Result<VersionId> {
        todo!("Implement three-way merge logic")
    }

    async fn branch_from_version(
        &mut self,
        source_version: VersionId,
        branch_name: String,
        author: String,
    ) -> Result<VersionId> {
        let source_version = self
            .get_version(source_version)
            .await?
            .ok_or_else(|| CodeGraphError::Transaction("Source version not found".to_string()))?;

        self.create_version(
            branch_name,
            format!("Branch from version {}", source_version.name),
            author,
            source_version.snapshot_id,
            vec![source_version.id],
        )
        .await
    }

    async fn compare_versions(
        &self,
        version1: VersionId,
        version2: VersionId,
    ) -> Result<codegraph_core::VersionDiff> {
        let v1 = self
            .get_version(version1)
            .await?
            .ok_or_else(|| CodeGraphError::Transaction("Version 1 not found".to_string()))?;
        let v2 = self
            .get_version(version2)
            .await?
            .ok_or_else(|| CodeGraphError::Transaction("Version 2 not found".to_string()))?;

        let s1 = self
            .get_snapshot(v1.snapshot_id)
            .await?
            .ok_or_else(|| CodeGraphError::Transaction("Snapshot 1 not found".to_string()))?;
        let s2 = self
            .get_snapshot(v2.snapshot_id)
            .await?
            .ok_or_else(|| CodeGraphError::Transaction("Snapshot 2 not found".to_string()))?;

        let mut added_nodes = Vec::new();
        let mut modified_nodes = Vec::new();
        let mut deleted_nodes = Vec::new();
        let mut node_changes = HashMap::new();

        let all_nodes: HashSet<NodeId> = s1
            .node_versions
            .keys()
            .chain(s2.node_versions.keys())
            .cloned()
            .collect();

        for node_id in all_nodes {
            let v1_hash = s1.node_versions.get(&node_id);
            let v2_hash = s2.node_versions.get(&node_id);

            match (v1_hash, v2_hash) {
                (None, Some(hash)) => {
                    added_nodes.push(node_id);
                    node_changes.insert(
                        node_id,
                        codegraph_core::NodeDiff {
                            old_content_hash: None,
                            new_content_hash: Some(hash.clone()),
                            change_type: ChangeType::Added,
                        },
                    );
                }
                (Some(hash), None) => {
                    deleted_nodes.push(node_id);
                    node_changes.insert(
                        node_id,
                        codegraph_core::NodeDiff {
                            old_content_hash: Some(hash.clone()),
                            new_content_hash: None,
                            change_type: ChangeType::Deleted,
                        },
                    );
                }
                (Some(hash1), Some(hash2)) if hash1 != hash2 => {
                    modified_nodes.push(node_id);
                    node_changes.insert(
                        node_id,
                        codegraph_core::NodeDiff {
                            old_content_hash: Some(hash1.clone()),
                            new_content_hash: Some(hash2.clone()),
                            change_type: ChangeType::Modified,
                        },
                    );
                }
                _ => {} // No change
            }
        }

        Ok(codegraph_core::VersionDiff {
            added_nodes,
            modified_nodes,
            deleted_nodes,
            node_changes,
        })
    }
}

#[async_trait]
impl TransactionManager for VersionedRocksDbStorage {
    async fn begin_transaction(
        &mut self,
        isolation_level: IsolationLevel,
    ) -> Result<TransactionId> {
        let transaction_id = TransactionId::new_v4();
        let snapshot_id = SnapshotId::new_v4(); // Create empty snapshot for new transaction

        let transaction = Transaction {
            id: transaction_id,
            isolation_level,
            status: TransactionStatus::Active,
            started_at: Utc::now(),
            committed_at: None,
            snapshot_id,
            read_set: HashSet::new(),
            write_set: HashMap::new(),
            parent_version: None,
        };

        let transactions_cf = self.get_cf_handle(TRANSACTIONS_CF)?;
        let serialized = bincode::serialize(&transaction)
            .map_err(|e| CodeGraphError::Database(e.to_string()))?;

        self.db
            .put_cf(&transactions_cf, transaction_id.as_bytes(), serialized)
            .map_err(|e| CodeGraphError::Database(e.to_string()))?;

        let transaction_arc = Arc::new(RwLock::new(transaction));
        self.active_transactions
            .insert(transaction_id, transaction_arc);

        Ok(transaction_id)
    }

    async fn commit_transaction(&mut self, transaction_id: TransactionId) -> Result<()> {
        let transaction = self
            .active_transactions
            .get(&transaction_id)
            .ok_or_else(|| CodeGraphError::Transaction("Transaction not found".to_string()))?
            .clone();

        {
            let mut tx = transaction.write();

            if tx.status != TransactionStatus::Active {
                return Err(CodeGraphError::Transaction(
                    "Transaction not active".to_string(),
                ));
            }

            tx.status = TransactionStatus::Preparing;
        }

        if !self.validate_transaction(transaction_id).await? {
            self.rollback_transaction(transaction_id).await?;
            return Err(CodeGraphError::Transaction(
                "Transaction validation failed".to_string(),
            ));
        }

        let mut batch = WriteBatch::default();
        let node_locks: Vec<_> = {
            let tx = transaction.read();
            tx.write_set
                .keys()
                .map(|node_id| (*node_id, self.get_commit_lock(*node_id)))
                .collect()
        };

        let _locks: Vec<_> = node_locks.iter().map(|(_, lock)| lock.lock()).collect();

        {
            let mut tx = transaction.write();

            for (node_id, write_op) in &tx.write_set {
                let wal_entry = WriteAheadLogEntry {
                    id: Uuid::new_v4(),
                    transaction_id,
                    sequence_number: self.wal_sequence.fetch_add(1, Ordering::SeqCst),
                    operation: write_op.clone(),
                    node_id: *node_id,
                    before_image: None, // TODO: Add before image
                    after_image: None,  // TODO: Add after image
                    timestamp: Utc::now(),
                };

                let wal_cf = self.get_cf_handle(WAL_CF)?;
                let wal_key = format!("{:020}", wal_entry.sequence_number);
                let wal_serialized = bincode::serialize(&wal_entry)
                    .map_err(|e| CodeGraphError::Database(e.to_string()))?;

                batch.put_cf(&wal_cf, wal_key.as_bytes(), wal_serialized);
            }

            tx.status = TransactionStatus::Committed;
            tx.committed_at = Some(Utc::now());

            let transactions_cf = self.get_cf_handle(TRANSACTIONS_CF)?;
            let tx_serialized =
                bincode::serialize(&*tx).map_err(|e| CodeGraphError::Database(e.to_string()))?;

            batch.put_cf(&transactions_cf, transaction_id.as_bytes(), tx_serialized);
        }

        self.db
            .write_opt(batch, &self.write_options)
            .map_err(|e| CodeGraphError::Database(e.to_string()))?;

        self.active_transactions.remove(&transaction_id);

        Ok(())
    }

    async fn rollback_transaction(&mut self, transaction_id: TransactionId) -> Result<()> {
        if let Some(transaction) = self.active_transactions.get(&transaction_id) {
            let mut tx = transaction.write();
            tx.status = TransactionStatus::Aborted;

            let transactions_cf = self.get_cf_handle(TRANSACTIONS_CF)?;
            let serialized =
                bincode::serialize(&*tx).map_err(|e| CodeGraphError::Database(e.to_string()))?;

            self.db
                .put_cf(&transactions_cf, transaction_id.as_bytes(), serialized)
                .map_err(|e| CodeGraphError::Database(e.to_string()))?;
        }

        self.active_transactions.remove(&transaction_id);

        Ok(())
    }

    async fn get_transaction(&self, transaction_id: TransactionId) -> Result<Option<Transaction>> {
        if let Some(transaction) = self.active_transactions.get(&transaction_id) {
            return Ok(Some(transaction.read().clone()));
        }

        let transactions_cf = self.get_cf_handle(TRANSACTIONS_CF)?;

        match self.db.get_cf(&transactions_cf, transaction_id.as_bytes()) {
            Ok(Some(data)) => {
                let transaction: Transaction = bincode::deserialize(&data)
                    .map_err(|e| CodeGraphError::Database(e.to_string()))?;
                Ok(Some(transaction))
            }
            Ok(None) => Ok(None),
            Err(e) => Err(CodeGraphError::Database(e.to_string())),
        }
    }

    async fn add_to_read_set(
        &mut self,
        transaction_id: TransactionId,
        node_id: NodeId,
    ) -> Result<()> {
        if let Some(transaction) = self.active_transactions.get(&transaction_id) {
            let mut tx = transaction.write();
            tx.read_set.insert(node_id);
        }
        Ok(())
    }

    async fn add_to_write_set(
        &mut self,
        transaction_id: TransactionId,
        node_id: NodeId,
        operation: WriteOperation,
    ) -> Result<()> {
        if let Some(transaction) = self.active_transactions.get(&transaction_id) {
            let mut tx = transaction.write();
            tx.write_set.insert(node_id, operation);
        }
        Ok(())
    }

    async fn validate_transaction(&self, transaction_id: TransactionId) -> Result<bool> {
        let transaction = self
            .active_transactions
            .get(&transaction_id)
            .ok_or_else(|| CodeGraphError::Transaction("Transaction not found".to_string()))?;

        let tx = transaction.read();

        match tx.isolation_level {
            IsolationLevel::Serializable => {
                for node_id in &tx.read_set {
                    for other_ref in self.active_transactions.iter() {
                        let other = other_ref.value().read();
                        if other.id != tx.id
                            && other.started_at < tx.started_at
                            && other.write_set.contains_key(node_id)
                        {
                            return Ok(false);
                        }
                    }
                }
                Ok(true)
            }
            IsolationLevel::RepeatableRead => {
                Ok(true) // TODO: Implement proper validation
            }
            IsolationLevel::ReadCommitted => {
                Ok(true) // TODO: Implement proper validation
            }
            IsolationLevel::ReadUncommitted => Ok(true),
        }
    }

    async fn prepare_transaction(&mut self, transaction_id: TransactionId) -> Result<bool> {
        if let Some(transaction) = self.active_transactions.get(&transaction_id) {
            let mut tx = transaction.write();
            if tx.status == TransactionStatus::Active {
                tx.status = TransactionStatus::Preparing;
                return Ok(true);
            }
        }
        Ok(false)
    }
}

#[async_trait]
impl WriteAheadLog for VersionedRocksDbStorage {
    async fn append_entry(&mut self, entry: WriteAheadLogEntry) -> Result<u64> {
        let sequence = self.wal_sequence.fetch_add(1, Ordering::SeqCst);
        let mut entry = entry;
        entry.sequence_number = sequence;

        let wal_cf = self.get_cf_handle(WAL_CF)?;
        let key = format!("{:020}", sequence);
        let serialized =
            bincode::serialize(&entry).map_err(|e| CodeGraphError::Database(e.to_string()))?;

        self.db
            .put_cf(&wal_cf, key.as_bytes(), serialized)
            .map_err(|e| CodeGraphError::Database(e.to_string()))?;

        Ok(sequence)
    }

    async fn get_entries_after(&self, sequence: u64) -> Result<Vec<WriteAheadLogEntry>> {
        let wal_cf = self.get_cf_handle(WAL_CF)?;
        let start_key = format!("{:020}", sequence + 1);

        let mut read_opts = ReadOptions::default();
        read_opts.set_iterate_lower_bound(start_key.as_bytes());

        let iter = self.db.iterator_cf_opt(
            &wal_cf,
            read_opts,
            IteratorMode::From(start_key.as_bytes(), rocksdb::Direction::Forward),
        );
        let mut entries = Vec::new();

        for item in iter {
            let (_, value) = item.map_err(|e| CodeGraphError::Database(e.to_string()))?;
            let entry: WriteAheadLogEntry = bincode::deserialize(&value)
                .map_err(|e| CodeGraphError::Database(e.to_string()))?;
            entries.push(entry);
        }

        Ok(entries)
    }

    async fn get_entries_for_transaction(
        &self,
        transaction_id: TransactionId,
    ) -> Result<Vec<WriteAheadLogEntry>> {
        let entries = self.get_entries_after(0).await?;
        Ok(entries
            .into_iter()
            .filter(|entry| entry.transaction_id == transaction_id)
            .collect())
    }

    async fn truncate_before(&mut self, sequence: u64) -> Result<()> {
        let wal_cf = self.get_cf_handle(WAL_CF)?;
        let end_key = format!("{:020}", sequence);

        let iter = self.db.iterator_cf(&wal_cf, IteratorMode::Start);
        let mut batch = WriteBatch::default();

        for item in iter {
            let (key, _) = item.map_err(|e| CodeGraphError::Database(e.to_string()))?;
            let key_str = String::from_utf8_lossy(&key);

            if key_str.as_ref() >= end_key.as_str() {
                break;
            }

            batch.delete_cf(&wal_cf, &key);
        }

        self.db
            .write_opt(batch, &self.write_options)
            .map_err(|e| CodeGraphError::Database(e.to_string()))?;

        Ok(())
    }

    async fn create_checkpoint(&mut self) -> Result<Checkpoint> {
        let checkpoint_id = Uuid::new_v4();
        let last_sequence = self.wal_sequence.load(Ordering::SeqCst);

        let checkpoint = Checkpoint {
            id: checkpoint_id,
            created_at: Utc::now(),
            last_committed_transaction: TransactionId::new_v4(), // TODO: Track this properly
            last_wal_sequence: last_sequence,
            snapshot_id: SnapshotId::new_v4(), // TODO: Create proper checkpoint snapshot
        };

        let checkpoints_cf = self.get_cf_handle(CHECKPOINTS_CF)?;
        let serialized =
            bincode::serialize(&checkpoint).map_err(|e| CodeGraphError::Database(e.to_string()))?;

        self.db
            .put_cf(&checkpoints_cf, checkpoint_id.as_bytes(), serialized)
            .map_err(|e| CodeGraphError::Database(e.to_string()))?;

        Ok(checkpoint)
    }

    async fn get_last_checkpoint(&self) -> Result<Option<Checkpoint>> {
        let checkpoints_cf = self.get_cf_handle(CHECKPOINTS_CF)?;
        let mut iter = self.db.iterator_cf(&checkpoints_cf, IteratorMode::End);

        if let Some(Ok((_, value))) = iter.next() {
            let checkpoint: Checkpoint = bincode::deserialize(&value)
                .map_err(|e| CodeGraphError::Database(e.to_string()))?;
            Ok(Some(checkpoint))
        } else {
            Ok(None)
        }
    }
}

#[async_trait]
impl CrashRecovery for VersionedRocksDbStorage {
    async fn recover_from_crash(&mut self) -> Result<()> {
        tracing::info!("Starting crash recovery...");

        let last_checkpoint = self.get_last_checkpoint().await?;

        let start_sequence = if let Some(checkpoint) = &last_checkpoint {
            checkpoint.last_wal_sequence
        } else {
            0
        };

        let wal_entries = self.get_entries_after(start_sequence).await?;

        if !wal_entries.is_empty() {
            tracing::info!("Replaying {} WAL entries", wal_entries.len());
            self.replay_transactions(last_checkpoint).await?;
        }

        let integrity_issues = self.verify_data_integrity().await?;

        if !integrity_issues.is_empty() {
            tracing::warn!("Found {} data integrity issues", integrity_issues.len());
            self.repair_corruption(integrity_issues).await?;
        }

        tracing::info!("Crash recovery completed successfully");
        Ok(())
    }

    async fn replay_transactions(&mut self, _from_checkpoint: Option<Checkpoint>) -> Result<()> {
        // TODO: Implement transaction replay logic
        Ok(())
    }

    async fn verify_data_integrity(&self) -> Result<Vec<String>> {
        let mut issues = Vec::new();

        // TODO: Implement integrity checks
        // - Verify that all content hashes exist in content store
        // - Verify that all snapshots have valid parent relationships
        // - Verify that all versions reference valid snapshots

        Ok(issues)
    }

    async fn repair_corruption(&mut self, _corruption_reports: Vec<String>) -> Result<()> {
        // TODO: Implement corruption repair logic
        Ok(())
    }
}
