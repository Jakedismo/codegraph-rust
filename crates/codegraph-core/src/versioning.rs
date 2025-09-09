use crate::{CodeGraphError, NodeId, Result};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;
use chrono::{DateTime, Utc};

pub type SnapshotId = Uuid;
pub type TransactionId = Uuid;
pub type VersionId = Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum IsolationLevel {
    ReadUncommitted,
    ReadCommitted,
    RepeatableRead,
    Serializable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TransactionStatus {
    Active,
    Preparing,
    Committed,
    Aborted,
    RollingBack,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transaction {
    pub id: TransactionId,
    pub isolation_level: IsolationLevel,
    pub status: TransactionStatus,
    pub started_at: DateTime<Utc>,
    pub committed_at: Option<DateTime<Utc>>,
    pub snapshot_id: SnapshotId,
    pub read_set: HashSet<NodeId>,
    pub write_set: HashMap<NodeId, WriteOperation>,
    pub parent_version: Option<VersionId>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WriteOperation {
    Insert(NodeId),
    Update { old_content_hash: String, new_content_hash: String },
    Delete(NodeId),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Snapshot {
    pub id: SnapshotId,
    pub created_at: DateTime<Utc>,
    pub transaction_id: TransactionId,
    pub node_versions: HashMap<NodeId, String>,
    pub parent_snapshot: Option<SnapshotId>,
    pub children_snapshots: Vec<SnapshotId>,
    pub ref_count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Version {
    pub id: VersionId,
    pub name: String,
    pub description: String,
    pub author: String,
    pub created_at: DateTime<Utc>,
    pub snapshot_id: SnapshotId,
    pub parent_versions: Vec<VersionId>,
    pub tags: Vec<String>,
    pub metadata: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeVersion {
    pub node_id: NodeId,
    pub version_id: VersionId,
    pub content_hash: String,
    pub created_at: DateTime<Utc>,
    pub transaction_id: TransactionId,
    pub is_tombstone: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WriteAheadLogEntry {
    pub id: Uuid,
    pub transaction_id: TransactionId,
    pub sequence_number: u64,
    pub operation: WriteOperation,
    pub node_id: NodeId,
    pub before_image: Option<String>,
    pub after_image: Option<String>,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Checkpoint {
    pub id: Uuid,
    pub created_at: DateTime<Utc>,
    pub last_committed_transaction: TransactionId,
    pub last_wal_sequence: u64,
    pub snapshot_id: SnapshotId,
}

pub trait VersionedStore {
    async fn create_snapshot(&mut self, transaction_id: TransactionId) -> Result<SnapshotId>;
    
    async fn get_snapshot(&self, snapshot_id: SnapshotId) -> Result<Option<Snapshot>>;
    
    async fn create_version(
        &mut self,
        name: String,
        description: String,
        author: String,
        snapshot_id: SnapshotId,
        parent_versions: Vec<VersionId>,
    ) -> Result<VersionId>;
    
    async fn get_version(&self, version_id: VersionId) -> Result<Option<Version>>;
    
    async fn list_versions(&self, limit: Option<u32>) -> Result<Vec<Version>>;
    
    async fn tag_version(&mut self, version_id: VersionId, tag: String) -> Result<()>;
    
    async fn get_version_by_tag(&self, tag: &str) -> Result<Option<Version>>;
    
    async fn merge_versions(
        &mut self,
        base_version: VersionId,
        source_version: VersionId,
        target_version: VersionId,
        author: String,
        message: String,
    ) -> Result<VersionId>;
    
    async fn branch_from_version(
        &mut self,
        source_version: VersionId,
        branch_name: String,
        author: String,
    ) -> Result<VersionId>;
    
    async fn compare_versions(
        &self,
        version1: VersionId,
        version2: VersionId,
    ) -> Result<VersionDiff>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionDiff {
    pub added_nodes: Vec<NodeId>,
    pub modified_nodes: Vec<NodeId>,
    pub deleted_nodes: Vec<NodeId>,
    pub node_changes: HashMap<NodeId, NodeDiff>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeDiff {
    pub old_content_hash: Option<String>,
    pub new_content_hash: Option<String>,
    pub change_type: ChangeType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChangeType {
    Added,
    Modified,
    Deleted,
}

pub trait TransactionManager {
    async fn begin_transaction(
        &mut self,
        isolation_level: IsolationLevel,
    ) -> Result<TransactionId>;
    
    async fn commit_transaction(&mut self, transaction_id: TransactionId) -> Result<()>;
    
    async fn rollback_transaction(&mut self, transaction_id: TransactionId) -> Result<()>;
    
    async fn get_transaction(&self, transaction_id: TransactionId) -> Result<Option<Transaction>>;
    
    async fn add_to_read_set(&mut self, transaction_id: TransactionId, node_id: NodeId) -> Result<()>;
    
    async fn add_to_write_set(
        &mut self,
        transaction_id: TransactionId,
        node_id: NodeId,
        operation: WriteOperation,
    ) -> Result<()>;
    
    async fn validate_transaction(&self, transaction_id: TransactionId) -> Result<bool>;
    
    async fn prepare_transaction(&mut self, transaction_id: TransactionId) -> Result<bool>;
}

pub trait WriteAheadLog {
    async fn append_entry(&mut self, entry: WriteAheadLogEntry) -> Result<u64>;
    
    async fn get_entries_after(&self, sequence: u64) -> Result<Vec<WriteAheadLogEntry>>;
    
    async fn get_entries_for_transaction(
        &self,
        transaction_id: TransactionId,
    ) -> Result<Vec<WriteAheadLogEntry>>;
    
    async fn truncate_before(&mut self, sequence: u64) -> Result<()>;
    
    async fn create_checkpoint(&mut self) -> Result<Checkpoint>;
    
    async fn get_last_checkpoint(&self) -> Result<Option<Checkpoint>>;
}

pub trait CrashRecovery {
    async fn recover_from_crash(&mut self) -> Result<()>;
    
    async fn replay_transactions(&mut self, from_checkpoint: Option<Checkpoint>) -> Result<()>;
    
    async fn verify_data_integrity(&self) -> Result<Vec<String>>;
    
    async fn repair_corruption(&mut self, corruption_reports: Vec<String>) -> Result<()>;
}