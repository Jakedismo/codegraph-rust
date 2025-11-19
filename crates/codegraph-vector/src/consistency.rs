use codegraph_core::{CodeGraphError, NodeId, Result};
use parking_lot::{Mutex, RwLock};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::watch;
use tracing::{debug, error, info, warn};

/// Transaction isolation levels for vector operations
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum IsolationLevel {
    /// Read uncommitted - fastest but least consistent
    ReadUncommitted,
    /// Read committed - prevents dirty reads
    ReadCommitted,
    /// Repeatable read - prevents dirty and non-repeatable reads
    RepeatableRead,
    /// Serializable - strongest consistency guarantees
    Serializable,
}

/// Transaction state
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TransactionState {
    Active,
    Preparing,
    Prepared,
    Committed,
    Aborted,
    Failed,
}

/// Vector operation within a transaction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum VectorOperation {
    Insert {
        node_id: NodeId,
        vector: Vec<f32>,
    },
    Update {
        node_id: NodeId,
        old_vector: Option<Vec<f32>>,
        new_vector: Vec<f32>,
    },
    Delete {
        node_id: NodeId,
        vector: Option<Vec<f32>>, // For rollback
    },
}

impl VectorOperation {
    pub fn node_id(&self) -> NodeId {
        match self {
            Self::Insert { node_id, .. }
            | Self::Update { node_id, .. }
            | Self::Delete { node_id, .. } => *node_id,
        }
    }

    /// Create the inverse operation for rollback
    pub fn inverse(&self) -> Option<Self> {
        match self {
            Self::Insert { node_id, .. } => Some(Self::Delete {
                node_id: *node_id,
                vector: None,
            }),
            Self::Update {
                node_id,
                old_vector,
                ..
            } => old_vector.as_ref().map(|old| Self::Update {
                node_id: *node_id,
                old_vector: None,
                new_vector: old.clone(),
            }),
            Self::Delete { node_id, vector } => vector.as_ref().map(|vec| Self::Insert {
                node_id: *node_id,
                vector: vec.clone(),
            }),
        }
    }
}

/// Transaction metadata
#[derive(Debug, Clone)]
pub struct Transaction {
    pub id: u64,
    pub isolation_level: IsolationLevel,
    pub state: TransactionState,
    pub operations: Vec<VectorOperation>,
    pub read_set: HashSet<NodeId>,
    pub write_set: HashSet<NodeId>,
    pub start_time: SystemTime,
    pub commit_time: Option<SystemTime>,
    pub timeout: Option<Duration>,
}

impl Transaction {
    pub fn new(id: u64, isolation_level: IsolationLevel) -> Self {
        Self {
            id,
            isolation_level,
            state: TransactionState::Active,
            operations: Vec::new(),
            read_set: HashSet::new(),
            write_set: HashSet::new(),
            start_time: SystemTime::now(),
            commit_time: None,
            timeout: Some(Duration::from_secs(30)), // Default 30s timeout
        }
    }

    pub fn add_operation(&mut self, operation: VectorOperation) {
        let node_id = operation.node_id();
        match &operation {
            VectorOperation::Insert { .. } | VectorOperation::Update { .. } => {
                self.write_set.insert(node_id);
            }
            VectorOperation::Delete { .. } => {
                self.write_set.insert(node_id);
            }
        }
        self.operations.push(operation);
    }

    pub fn add_read(&mut self, node_id: NodeId) {
        self.read_set.insert(node_id);
    }

    pub fn is_expired(&self) -> bool {
        if let Some(timeout) = self.timeout {
            self.start_time.elapsed().unwrap_or_default() > timeout
        } else {
            false
        }
    }

    pub fn conflicts_with(&self, other: &Transaction) -> bool {
        // Check for write-write conflicts
        if !self.write_set.is_disjoint(&other.write_set) {
            return true;
        }

        // Check for read-write conflicts based on isolation level
        match self.isolation_level.max(other.isolation_level) {
            IsolationLevel::ReadUncommitted => false,
            IsolationLevel::ReadCommitted => false,
            IsolationLevel::RepeatableRead => {
                !self.read_set.is_disjoint(&other.write_set)
                    || !other.read_set.is_disjoint(&self.write_set)
            }
            IsolationLevel::Serializable => {
                !self.read_set.is_disjoint(&other.write_set)
                    || !other.read_set.is_disjoint(&self.write_set)
            }
        }
    }
}

/// Lock modes for fine-grained concurrency control
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LockMode {
    Shared,
    Exclusive,
    IntentionShared,
    IntentionExclusive,
    SharedIntentionExclusive,
}

impl LockMode {
    pub fn is_compatible(self, other: LockMode) -> bool {
        use LockMode::*;
        match (self, other) {
            (Shared, Shared)
            | (Shared, IntentionShared)
            | (IntentionShared, Shared)
            | (IntentionShared, IntentionShared)
            | (IntentionShared, IntentionExclusive)
            | (IntentionExclusive, IntentionShared) => true,
            _ => false,
        }
    }

    pub fn is_stronger_than(self, other: LockMode) -> bool {
        use LockMode::*;
        match (self, other) {
            (Exclusive, _) => true,
            (SharedIntentionExclusive, Shared) | (SharedIntentionExclusive, IntentionShared) => {
                true
            }
            (IntentionExclusive, IntentionShared) => true,
            _ => false,
        }
    }
}

/// Lock information
#[derive(Debug, Clone)]
pub struct Lock {
    pub transaction_id: u64,
    pub mode: LockMode,
    pub acquired_at: SystemTime,
}

/// Consistency checkpoint for recovery
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsistencyCheckpoint {
    pub checkpoint_id: u64,
    pub timestamp: u64,
    pub committed_transactions: Vec<u64>,
    pub vector_checksums: HashMap<NodeId, u64>,
    pub metadata_checksum: u64,
}

/// Consistency manager for vector storage
#[derive(Clone)]
pub struct ConsistencyManager {
    /// Active transactions
    active_transactions: Arc<RwLock<HashMap<u64, Transaction>>>,
    /// Lock table for fine-grained concurrency control
    lock_table: Arc<RwLock<HashMap<NodeId, Vec<Lock>>>>,
    /// Transaction log for recovery
    transaction_log: Arc<Mutex<Vec<TransactionLogEntry>>>,
    /// Next transaction ID
    next_transaction_id: Arc<RwLock<u64>>,
    /// Committed transaction IDs for visibility
    committed_transactions: Arc<RwLock<HashSet<u64>>>,
    /// Checkpoints for recovery
    checkpoints: Arc<RwLock<Vec<ConsistencyCheckpoint>>>,
    /// Configuration
    config: ConsistencyConfig,
    /// Notification channel for transaction commits
    commit_notifier: Arc<watch::Sender<u64>>,
    _commit_receiver: watch::Receiver<u64>,
}

#[derive(Debug, Clone)]
pub struct ConsistencyConfig {
    /// Maximum number of active transactions
    pub max_active_transactions: usize,
    /// Transaction timeout
    pub transaction_timeout: Duration,
    /// Checkpoint interval
    pub checkpoint_interval: Duration,
    /// Maximum log entries before forced checkpoint
    pub max_log_entries: usize,
    /// Lock timeout
    pub lock_timeout: Duration,
    /// Enable deadlock detection
    pub enable_deadlock_detection: bool,
    /// Deadlock detection interval
    pub deadlock_detection_interval: Duration,
}

impl Default for ConsistencyConfig {
    fn default() -> Self {
        Self {
            max_active_transactions: 1000,
            transaction_timeout: Duration::from_secs(30),
            checkpoint_interval: Duration::from_secs(300), // 5 minutes
            max_log_entries: 10000,
            lock_timeout: Duration::from_secs(10),
            enable_deadlock_detection: true,
            deadlock_detection_interval: Duration::from_secs(5),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionLogEntry {
    pub transaction_id: u64,
    pub operation: VectorOperation,
    pub timestamp: u64,
    pub state: TransactionState,
}

impl ConsistencyManager {
    pub fn new(config: ConsistencyConfig) -> Self {
        let (commit_sender, commit_receiver) = watch::channel(0);

        let manager = Self {
            active_transactions: Arc::new(RwLock::new(HashMap::new())),
            lock_table: Arc::new(RwLock::new(HashMap::new())),
            transaction_log: Arc::new(Mutex::new(Vec::new())),
            next_transaction_id: Arc::new(RwLock::new(1)),
            committed_transactions: Arc::new(RwLock::new(HashSet::new())),
            checkpoints: Arc::new(RwLock::new(Vec::new())),
            config: config.clone(),
            commit_notifier: Arc::new(commit_sender),
            _commit_receiver: commit_receiver,
        };

        // Start background tasks
        if config.enable_deadlock_detection {
            manager.start_deadlock_detector();
        }
        manager.start_checkpoint_manager();
        manager.start_cleanup_task();

        manager
    }

    /// Begin a new transaction
    pub fn begin_transaction(&self, isolation_level: IsolationLevel) -> Result<u64> {
        let transaction_id = {
            let mut next_id = self.next_transaction_id.write();
            let id = *next_id;
            *next_id += 1;
            id
        };

        let mut active_txns = self.active_transactions.write();

        // Check transaction limit
        if active_txns.len() >= self.config.max_active_transactions {
            return Err(CodeGraphError::Vector(
                "Maximum number of active transactions reached".to_string(),
            ));
        }

        let transaction = Transaction::new(transaction_id, isolation_level);
        active_txns.insert(transaction_id, transaction);

        debug!(
            "Started transaction {} with isolation level {:?}",
            transaction_id, isolation_level
        );
        Ok(transaction_id)
    }

    /// Add an operation to a transaction
    pub fn add_operation(&self, transaction_id: u64, operation: VectorOperation) -> Result<()> {
        let mut active_txns = self.active_transactions.write();

        let transaction = active_txns
            .get_mut(&transaction_id)
            .ok_or_else(|| CodeGraphError::Vector("Transaction not found".to_string()))?;

        if transaction.state != TransactionState::Active {
            return Err(CodeGraphError::Vector(
                "Transaction is not active".to_string(),
            ));
        }

        if transaction.is_expired() {
            transaction.state = TransactionState::Failed;
            return Err(CodeGraphError::Vector(
                "Transaction has expired".to_string(),
            ));
        }

        // Check for conflicts with other transactions
        let transaction_clone = transaction.clone();


        let conflicts = active_txns
            .values()
            .filter(|other| other.id != transaction_id && other.conflicts_with(&transaction_clone))
            .count();

        let transaction = active_txns
            .get_mut(&transaction_id)
            .ok_or_else(|| CodeGraphError::Vector("Transaction not found".to_string()))?;

        if conflicts > 0 {
            match transaction.isolation_level {
                IsolationLevel::Serializable => {
                    return Err(CodeGraphError::Vector(
                        "Serialization conflict detected".to_string(),
                    ));
                }
                _ => {
                    // For lower isolation levels, we might wait or proceed
                    warn!(
                        "Conflict detected in transaction {} but proceeding due to isolation level",
                        transaction_id
                    );
                }
            }
        }

        transaction.add_operation(operation.clone());

        // Log the operation
        let log_entry = TransactionLogEntry {
            transaction_id,
            operation,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            state: TransactionState::Active,
        };

        self.transaction_log.lock().push(log_entry);

        Ok(())
    }

    /// Acquire a lock on a node
    pub async fn acquire_lock(
        &self,
        transaction_id: u64,
        node_id: NodeId,
        mode: LockMode,
    ) -> Result<()> {
        let timeout = tokio::time::sleep(self.config.lock_timeout);
        tokio::pin!(timeout);

        loop {
            tokio::select! {
                _ = &mut timeout => {
                    return Err(CodeGraphError::Vector("Lock acquisition timeout".to_string()));
                }
                result = async { self.try_acquire_lock(transaction_id, node_id, mode) } => {
                    match result {
                        Ok(()) => return Ok(()),
                        Err(_) => {
                            // Wait a bit before retrying
                            tokio::time::sleep(Duration::from_millis(10)).await;
                        }
                    }
                }
            }
        }
    }

    fn try_acquire_lock(&self, transaction_id: u64, node_id: NodeId, mode: LockMode) -> Result<()> {
        let mut lock_table = self.lock_table.write();
        let locks = lock_table.entry(node_id).or_insert_with(Vec::new);

        // Check if any existing locks are incompatible
        for existing_lock in locks.iter() {
            if existing_lock.transaction_id != transaction_id
                && !mode.is_compatible(existing_lock.mode)
            {
                return Err(CodeGraphError::Vector("Lock conflict".to_string()));
            }
        }

        // Check if we already have a compatible or stronger lock
        if let Some(existing) = locks
            .iter_mut()
            .find(|l| l.transaction_id == transaction_id)
        {
            if mode.is_stronger_than(existing.mode) {
                existing.mode = mode;
                existing.acquired_at = SystemTime::now();
            }
        } else {
            // Acquire new lock
            locks.push(Lock {
                transaction_id,
                mode,
                acquired_at: SystemTime::now(),
            });
        }

        debug!(
            "Acquired {:?} lock on node {} for transaction {}",
            mode, node_id, transaction_id
        );
        Ok(())
    }

    /// Release all locks held by a transaction
    pub fn release_locks(&self, transaction_id: u64) {
        let mut lock_table = self.lock_table.write();

        for locks in lock_table.values_mut() {
            locks.retain(|lock| lock.transaction_id != transaction_id);
        }

        // Remove empty entries
        lock_table.retain(|_, locks| !locks.is_empty());

        debug!("Released all locks for transaction {}", transaction_id);
    }

    /// Prepare a transaction for two-phase commit
    pub fn prepare_transaction(&self, transaction_id: u64) -> Result<()> {
        let mut active_txns = self.active_transactions.write();

        let transaction = active_txns
            .get_mut(&transaction_id)
            .ok_or_else(|| CodeGraphError::Vector("Transaction not found".to_string()))?;

        if transaction.state != TransactionState::Active {
            return Err(CodeGraphError::Vector(
                "Transaction is not active".to_string(),
            ));
        }

        if transaction.is_expired() {
            transaction.state = TransactionState::Failed;
            return Err(CodeGraphError::Vector(
                "Transaction has expired".to_string(),
            ));
        }

        transaction.state = TransactionState::Preparing;

        // Validate transaction can be committed
        let validation_result = self.validate_transaction(transaction);

        if validation_result.is_ok() {
            transaction.state = TransactionState::Prepared;
            info!("Prepared transaction {} for commit", transaction_id);
        } else {
            transaction.state = TransactionState::Failed;
            return validation_result;
        }

        Ok(())
    }

    /// Commit a prepared transaction
    pub fn commit_transaction(&self, transaction_id: u64) -> Result<()> {
        let mut active_txns = self.active_transactions.write();

        let mut transaction = active_txns
            .remove(&transaction_id)
            .ok_or_else(|| CodeGraphError::Vector("Transaction not found".to_string()))?;

        if transaction.state != TransactionState::Prepared {
            return Err(CodeGraphError::Vector(
                "Transaction is not prepared".to_string(),
            ));
        }

        transaction.state = TransactionState::Committed;
        transaction.commit_time = Some(SystemTime::now());

        // Add to committed transactions
        {
            let mut committed = self.committed_transactions.write();
            committed.insert(transaction_id);
        }

        // Log commit
        let log_entry = TransactionLogEntry {
            transaction_id,
            operation: VectorOperation::Insert {
                node_id: NodeId::nil(),
                vector: vec![],
            }, // Dummy operation
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            state: TransactionState::Committed,
        };

        self.transaction_log.lock().push(log_entry);

        // Release locks
        self.release_locks(transaction_id);

        // Notify other transactions
        let _ = self.commit_notifier.send(transaction_id);

        info!("Committed transaction {}", transaction_id);
        Ok(())
    }

    /// Abort a transaction
    pub fn abort_transaction(&self, transaction_id: u64) -> Result<Vec<VectorOperation>> {
        let mut active_txns = self.active_transactions.write();

        let mut transaction = active_txns
            .remove(&transaction_id)
            .ok_or_else(|| CodeGraphError::Vector("Transaction not found".to_string()))?;

        transaction.state = TransactionState::Aborted;

        // Generate rollback operations
        let rollback_operations: Vec<VectorOperation> = transaction
            .operations
            .iter()
            .rev() // Reverse order for rollback
            .filter_map(|op| op.inverse())
            .collect();

        // Log abort
        let log_entry = TransactionLogEntry {
            transaction_id,
            operation: VectorOperation::Delete {
                node_id: NodeId::nil(),
                vector: None,
            }, // Dummy operation
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            state: TransactionState::Aborted,
        };

        self.transaction_log.lock().push(log_entry);

        // Release locks
        self.release_locks(transaction_id);

        warn!("Aborted transaction {}", transaction_id);
        Ok(rollback_operations)
    }

    /// Validate a transaction for commit
    fn validate_transaction(&self, transaction: &Transaction) -> Result<()> {
        // Check for expired transaction
        if transaction.is_expired() {
            return Err(CodeGraphError::Vector(
                "Transaction has expired".to_string(),
            ));
        }

        // Check for conflicts based on isolation level
        match transaction.isolation_level {
            IsolationLevel::Serializable => {
                // Strict validation - check all read/write sets
                let active_txns = self.active_transactions.read();
                let conflicts = active_txns
                    .values()
                    .filter(|other| other.id != transaction.id && transaction.conflicts_with(other))
                    .count();

                if conflicts > 0 {
                    return Err(CodeGraphError::Vector(
                        "Serialization conflict detected".to_string(),
                    ));
                }
            }
            _ => {
                // Less strict validation for lower isolation levels
            }
        }

        Ok(())
    }

    /// Check if a node is visible to a transaction
    pub fn is_visible(
        &self,
        _node_id: NodeId,
        transaction_id: u64,
        committed_by: Option<u64>,
    ) -> bool {
        let active_txns = self.active_transactions.read();
        let transaction = match active_txns.get(&transaction_id) {
            Some(txn) => txn,
            None => return true, // Transaction not found, assume visible
        };

        match transaction.isolation_level {
            IsolationLevel::ReadUncommitted => true, // See all changes
            IsolationLevel::ReadCommitted => {
                // Only see committed changes
                committed_by.map_or(true, |commit_txn| {
                    let committed = self.committed_transactions.read();
                    committed.contains(&commit_txn)
                })
            }
            IsolationLevel::RepeatableRead | IsolationLevel::Serializable => {
                // Only see changes committed before this transaction started
                committed_by.map_or(true, |commit_txn| {
                    let committed = self.committed_transactions.read();
                    committed.contains(&commit_txn) && commit_txn < transaction_id
                })
            }
        }
    }

    /// Create a consistency checkpoint
    pub fn create_checkpoint(&self) -> Result<ConsistencyCheckpoint> {
        let checkpoint_id = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let committed_txns: Vec<u64> = {
            let committed = self.committed_transactions.read();
            committed.iter().cloned().collect()
        };

        // For now, create empty checksums - in production this would
        // calculate actual checksums of all vectors
        let vector_checksums = HashMap::new();
        let metadata_checksum = 0;

        let checkpoint = ConsistencyCheckpoint {
            checkpoint_id,
            timestamp: checkpoint_id,
            committed_transactions: committed_txns,
            vector_checksums,
            metadata_checksum,
        };

        {
            let mut checkpoints = self.checkpoints.write();
            checkpoints.push(checkpoint.clone());

            // Keep only recent checkpoints
            checkpoints.sort_by_key(|cp| cp.timestamp);
            let checkpoint_count = checkpoints.len();
            if checkpoint_count > 10 {
                checkpoints.drain(0..checkpoint_count - 10);
            }
        }

        info!("Created consistency checkpoint {}", checkpoint_id);
        Ok(checkpoint)
    }

    /// Get the latest checkpoint
    pub fn get_latest_checkpoint(&self) -> Option<ConsistencyCheckpoint> {
        let checkpoints = self.checkpoints.read();
        checkpoints.last().cloned()
    }

    /// Start deadlock detection task
    fn start_deadlock_detector(&self) {
        let active_transactions = Arc::clone(&self.active_transactions);
        let lock_table = Arc::clone(&self.lock_table);
        let interval = self.config.deadlock_detection_interval;

        tokio::spawn(async move {
            let mut interval_timer = tokio::time::interval(interval);

            loop {
                interval_timer.tick().await;

                // Simple deadlock detection using wait-for graph
                let deadlocks = Self::detect_deadlocks(&active_transactions, &lock_table);

                if !deadlocks.is_empty() {
                    warn!("Detected {} deadlocks", deadlocks.len());
                    // In a real implementation, we would resolve deadlocks
                    // by aborting one of the transactions in each cycle
                }
            }
        });
    }

    /// Detect deadlocks using wait-for graph
    fn detect_deadlocks(
        active_transactions: &Arc<RwLock<HashMap<u64, Transaction>>>,
        lock_table: &Arc<RwLock<HashMap<NodeId, Vec<Lock>>>>,
    ) -> Vec<Vec<u64>> {
        let _active_txns = active_transactions.read();
        let _locks = lock_table.read();

        // Simplified deadlock detection - in production this would
        // build a proper wait-for graph and detect cycles
        Vec::new()
    }

    /// Start checkpoint manager task
    fn start_checkpoint_manager(&self) {
        let manager = self.clone();
        let interval = manager.config.checkpoint_interval;
        let max_log_entries = manager.config.max_log_entries;

        tokio::spawn(async move {
            let mut interval_timer = tokio::time::interval(interval);

            loop {
                interval_timer.tick().await;

                let should_checkpoint = {
                    let log = manager.transaction_log.lock();
                    log.len() >= max_log_entries
                };

                if should_checkpoint {
                    if let Err(e) = manager.create_checkpoint() {
                        error!("Failed to create checkpoint: {}", e);
                    } else {
                        // Clear old log entries after successful checkpoint
                        let mut log = manager.transaction_log.lock();
                        log.clear();
                    }
                }
            }
        });
    }

    /// Start cleanup task for expired transactions
    fn start_cleanup_task(&self) {
        let active_transactions = Arc::clone(&self.active_transactions);
        let cleanup_interval = Duration::from_secs(60); // Clean up every minute

        tokio::spawn(async move {
            let mut interval_timer = tokio::time::interval(cleanup_interval);

            loop {
                interval_timer.tick().await;

                let expired_txns: Vec<u64> = {
                    let active_txns = active_transactions.read();
                    active_txns
                        .values()
                        .filter(|txn| txn.is_expired())
                        .map(|txn| txn.id)
                        .collect()
                };

                if !expired_txns.is_empty() {
                    warn!("Cleaning up {} expired transactions", expired_txns.len());

                    let mut active_txns = active_transactions.write();
                    for txn_id in expired_txns {
                        if let Some(mut txn) = active_txns.remove(&txn_id) {
                            txn.state = TransactionState::Failed;
                        }
                    }
                }
            }
        });
    }

    /// Get statistics about active transactions
    pub fn get_transaction_stats(&self) -> TransactionStats {
        let active_txns = self.active_transactions.read();
        let committed_txns = self.committed_transactions.read();
        let log = self.transaction_log.lock();

        let active_count = active_txns.len();
        let committed_count = committed_txns.len();
        let log_entries = log.len();

        let isolation_level_counts = {
            let mut counts = HashMap::new();
            for txn in active_txns.values() {
                *counts.entry(txn.isolation_level).or_insert(0) += 1;
            }
            counts
        };

        TransactionStats {
            active_transactions: active_count,
            committed_transactions: committed_count,
            log_entries,
            isolation_level_counts,
        }
    }
}

/// Transaction statistics
#[derive(Debug, Clone)]
pub struct TransactionStats {
    pub active_transactions: usize,
    pub committed_transactions: usize,
    pub log_entries: usize,
    pub isolation_level_counts: HashMap<IsolationLevel, usize>,
}

#[cfg(test)]
mod tests {
    use super::*;


    #[tokio::test]
    async fn test_transaction_lifecycle() {
        let config = ConsistencyConfig::default();
        let manager = ConsistencyManager::new(config);

        // Begin transaction
        let txn_id = manager
            .begin_transaction(IsolationLevel::ReadCommitted)
            .unwrap();
        assert!(txn_id > 0);

        // Add operations
        let nid = NodeId::new_v4();
        let operation = VectorOperation::Insert {
            node_id: nid,
            vector: vec![1.0, 2.0, 3.0],
        };
        manager.add_operation(txn_id, operation).unwrap();

        // Prepare transaction
        manager.prepare_transaction(txn_id).unwrap();

        // Commit transaction
        manager.commit_transaction(txn_id).unwrap();

        let stats = manager.get_transaction_stats();
        assert_eq!(stats.committed_transactions, 1);
    }

    #[tokio::test]
    async fn test_lock_acquisition() {
        let config = ConsistencyConfig::default();
        let manager = ConsistencyManager::new(config);

        let txn_id = manager
            .begin_transaction(IsolationLevel::ReadCommitted)
            .unwrap();

        // Acquire shared lock
        let nid = NodeId::new_v4();
        manager
            .acquire_lock(txn_id, nid, LockMode::Shared)
            .await
            .unwrap();

        // Try to acquire incompatible lock from different transaction
        let txn_id2 = manager
            .begin_transaction(IsolationLevel::ReadCommitted)
            .unwrap();
        let result = tokio::time::timeout(
            Duration::from_millis(100),
            manager.acquire_lock(txn_id2, nid, LockMode::Exclusive),
        )
        .await;

        assert!(result.is_err()); // Should timeout due to lock conflict
    }

    #[tokio::test]
    async fn test_transaction_abort() {
        let config = ConsistencyConfig::default();
        let manager = ConsistencyManager::new(config);

        let txn_id = manager
            .begin_transaction(IsolationLevel::ReadCommitted)
            .unwrap();

        let nid = NodeId::new_v4();
        let operation = VectorOperation::Insert {
            node_id: nid,
            vector: vec![1.0, 2.0, 3.0],
        };
        manager.add_operation(txn_id, operation).unwrap();

        // Abort transaction
        let rollback_ops = manager.abort_transaction(txn_id).unwrap();
        assert_eq!(rollback_ops.len(), 1);

        match &rollback_ops[0] {
            VectorOperation::Delete { node_id, .. } => {
                assert_eq!(*node_id, nid);
            }
            _ => panic!("Expected delete operation for rollback"),
        }
    }

    #[tokio::test]
    async fn test_consistency_checkpoint() {
        let config = ConsistencyConfig::default();
        let manager = ConsistencyManager::new(config);

        let checkpoint = manager.create_checkpoint().unwrap();
        assert!(checkpoint.checkpoint_id > 0);

        let latest = manager.get_latest_checkpoint().unwrap();
        assert_eq!(latest.checkpoint_id, checkpoint.checkpoint_id);
    }
}
