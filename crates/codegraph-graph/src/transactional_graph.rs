use async_trait::async_trait;
use codegraph_core::{
    CodeGraphError, CodeNode, GraphStore, IsolationLevel, NodeId, Result, SnapshotId, Transaction,
    TransactionId, TransactionManager, TransactionStatus, VersionId, VersionedStore,
    WriteOperation,
};
use parking_lot::RwLock;
use std::{collections::HashMap, sync::Arc, time::Duration};
use tokio::time::timeout;
use uuid::Uuid;

use crate::VersionedRocksDbStorage;

pub struct TransactionalGraph {
    storage: Arc<RwLock<VersionedRocksDbStorage>>,
    current_transaction: Option<TransactionId>,
    isolation_level: IsolationLevel,
}

impl TransactionalGraph {
    pub async fn new(storage_path: &str) -> Result<Self> {
        let storage = VersionedRocksDbStorage::new(storage_path).await?;
        Ok(Self {
            storage: Arc::new(RwLock::new(storage)),
            current_transaction: None,
            isolation_level: IsolationLevel::ReadCommitted,
        })
    }

    pub async fn begin_transaction(&mut self) -> Result<TransactionId> {
        self.begin_transaction_with_isolation(self.isolation_level)
            .await
    }

    pub async fn begin_transaction_with_isolation(
        &mut self,
        isolation_level: IsolationLevel,
    ) -> Result<TransactionId> {
        if self.current_transaction.is_some() {
            return Err(CodeGraphError::Transaction(
                "Transaction already active".to_string(),
            ));
        }

        let transaction_id = {
            let mut storage = self.storage.write();
            storage.begin_transaction(isolation_level).await?
        };

        self.current_transaction = Some(transaction_id);
        self.isolation_level = isolation_level;

        Ok(transaction_id)
    }

    pub async fn commit(&mut self) -> Result<()> {
        let transaction_id = self
            .current_transaction
            .take()
            .ok_or_else(|| CodeGraphError::Transaction("No active transaction".to_string()))?;
        // Run commit in a blocking task to avoid holding non-Send guards across .await
        let storage = self.storage.clone();
        let res: Result<()> = tokio::task::spawn_blocking(move || {
            let handle = tokio::runtime::Handle::current();
            handle.block_on(async move {
                let mut guard = storage.write();
                guard.commit_transaction(transaction_id).await
            })
        })
        .await
        .map_err(|e| CodeGraphError::Threading(e.to_string()))?;
        res
    }

    pub async fn rollback(&mut self) -> Result<()> {
        let transaction_id = self
            .current_transaction
            .take()
            .ok_or_else(|| CodeGraphError::Transaction("No active transaction".to_string()))?;

        let storage = self.storage.clone();
        tokio::task::spawn_blocking(move || {
            let handle = tokio::runtime::Handle::current();
            handle.block_on(async move {
                let mut guard = storage.write();
                guard.rollback_transaction(transaction_id).await
            })
        })
        .await
        .map_err(|e| CodeGraphError::Threading(e.to_string()))?
    }

    pub async fn create_savepoint(&self) -> Result<SnapshotId> {
        let transaction_id = self
            .current_transaction
            .ok_or_else(|| CodeGraphError::Transaction("No active transaction".to_string()))?;

        self.storage.write().create_snapshot(transaction_id).await
    }

    pub async fn rollback_to_savepoint(&mut self, snapshot_id: SnapshotId) -> Result<()> {
        let _transaction_id = self
            .current_transaction
            .ok_or_else(|| CodeGraphError::Transaction("No active transaction".to_string()))?;

        // TODO: Implement rollback to specific savepoint
        // This would involve:
        // 1. Identify the write operations performed after the savepoint
        // 2. Reverse those operations
        // 3. Update the transaction's write set

        Ok(())
    }

    pub async fn create_version(
        &mut self,
        name: String,
        description: String,
        author: String,
        parent_versions: Vec<VersionId>,
    ) -> Result<VersionId> {
        let transaction_id = self
            .current_transaction
            .ok_or_else(|| CodeGraphError::Transaction("No active transaction".to_string()))?;

        // First commit the current transaction to create a snapshot
        let snapshot_id = {
            let storage = self.storage.clone();
            tokio::task::spawn_blocking(move || {
                let handle = tokio::runtime::Handle::current();
                handle.block_on(async move {
                    let mut guard = storage.write();
                    guard.create_snapshot(transaction_id).await
                })
            })
            .await
            .map_err(|e| CodeGraphError::Threading(e.to_string()))??
        };

        // Create a version pointing to this snapshot
        self.storage
            .write()
            .create_version(name, description, author, snapshot_id, parent_versions)
            .await
    }

    pub async fn checkout_version(&mut self, version_id: VersionId) -> Result<()> {
        if self.current_transaction.is_some() {
            return Err(CodeGraphError::Transaction(
                "Cannot checkout version with active transaction".to_string(),
            ));
        }

        let storage = self.storage.read();
        let version = storage
            .get_version(version_id)
            .await?
            .ok_or_else(|| CodeGraphError::Transaction("Version not found".to_string()))?;

        // TODO: Update the current working state to reflect this version
        // This would involve setting up the graph to read from the specific snapshot

        Ok(())
    }

    fn get_current_transaction_id(&self) -> Result<TransactionId> {
        self.current_transaction
            .ok_or_else(|| CodeGraphError::Transaction("No active transaction".to_string()))
    }
}

#[async_trait]
impl GraphStore for TransactionalGraph {
    async fn add_node(&mut self, mut node: CodeNode) -> Result<()> {
        let transaction_id = self.get_current_transaction_id()?;

        // Generate content hash for the node
        let content_hash = {
            let serialized =
                bincode::serialize(&node).map_err(|e| CodeGraphError::Database(e.to_string()))?;

            use sha2::{Digest, Sha256};
            let mut hasher = Sha256::new();
            hasher.update(&serialized);
            format!("{:x}", hasher.finalize())
        };

        // Add to transaction's write set
        let write_op = WriteOperation::Insert(node.id);
        {
            let storage = self.storage.clone();
            let node_id = node.id;
            let write_op_cloned = write_op.clone();
            tokio::task::spawn_blocking(move || {
                let handle = tokio::runtime::Handle::current();
                handle.block_on(async move {
                    let mut guard = storage.write();
                    guard
                        .add_to_write_set(transaction_id, node_id, write_op_cloned)
                        .await
                })
            })
            .await
            .map_err(|e| CodeGraphError::Threading(e.to_string()))??;
        }

        // Store the actual content in the storage with the hash
        // TODO: Implement content-addressed storage for the node data

        Ok(())
    }

    async fn get_node(&self, id: NodeId) -> Result<Option<CodeNode>> {
        let transaction_id = self.get_current_transaction_id()?;
        // Add to read set for isolation level validation without holding non-Send guards across .await
        {
            let storage = self.storage.clone();
            tokio::task::spawn_blocking(move || {
                let handle = tokio::runtime::Handle::current();
                handle.block_on(async move {
                    let mut guard = storage.write();
                    guard.add_to_read_set(transaction_id, id).await
                })
            })
            .await
            .map_err(|e| CodeGraphError::Threading(e.to_string()))??;
        }

        // Check if this node is in the current transaction's write set first
        let is_in_write = {
            let storage = self.storage.clone();
            tokio::task::spawn_blocking(move || {
                let handle = tokio::runtime::Handle::current();
                handle.block_on(async move {
                    let guard = storage.read();
                    Ok::<_, CodeGraphError>(guard.get_transaction(transaction_id).await?)
                })
            })
            .await
            .map_err(|e| CodeGraphError::Threading(e.to_string()))??
        };
        if let Some(transaction) = is_in_write {
            if transaction.write_set.contains_key(&id) {
                // Node is being modified in this transaction
                // TODO: Return the modified version from the write set
            }
        }

        // If not in write set, read from the snapshot
        let maybe_tx = {
            let storage = self.storage.clone();
            tokio::task::spawn_blocking(move || {
                let handle = tokio::runtime::Handle::current();
                handle.block_on(async move {
                    let guard = storage.read();
                    Ok::<_, CodeGraphError>(guard.get_transaction(transaction_id).await?)
                })
            })
            .await
            .map_err(|e| CodeGraphError::Threading(e.to_string()))??
        };
        if let Some(_transaction) = maybe_tx {
            // TODO: Read node from the transaction's snapshot
            // This would involve reading from the snapshot's content store
        }

        // For now, return None as placeholder
        Ok(None)
    }

    async fn update_node(&mut self, node: CodeNode) -> Result<()> {
        let transaction_id = self.get_current_transaction_id()?;

        // Get the current version of the node to create before/after images
        let old_node = self.get_node(node.id).await?;

        let (old_hash, new_hash) = {
            let old_hash = if let Some(ref old) = old_node {
                let serialized =
                    bincode::serialize(old).map_err(|e| CodeGraphError::Database(e.to_string()))?;

                use sha2::{Digest, Sha256};
                let mut hasher = Sha256::new();
                hasher.update(&serialized);
                format!("{:x}", hasher.finalize())
            } else {
                String::new()
            };

            let new_serialized =
                bincode::serialize(&node).map_err(|e| CodeGraphError::Database(e.to_string()))?;

            use sha2::{Digest, Sha256};
            let mut hasher = Sha256::new();
            hasher.update(&new_serialized);
            let new_hash = format!("{:x}", hasher.finalize());

            (old_hash, new_hash)
        };

        // Add to transaction's write set
        let write_op = WriteOperation::Update {
            old_content_hash: old_hash,
            new_content_hash: new_hash,
        };
        {
            let storage = self.storage.clone();
            let node_id = node.id;
            let write_op_cloned = write_op.clone();
            tokio::task::spawn_blocking(move || {
                let handle = tokio::runtime::Handle::current();
                handle.block_on(async move {
                    let mut guard = storage.write();
                    guard
                        .add_to_write_set(transaction_id, node_id, write_op_cloned)
                        .await
                })
            })
            .await
            .map_err(|e| CodeGraphError::Threading(e.to_string()))??;
        }

        Ok(())
    }

    async fn remove_node(&mut self, id: NodeId) -> Result<()> {
        let transaction_id = self.get_current_transaction_id()?;

        // Add to transaction's write set
        let write_op = WriteOperation::Delete(id);
        {
            let storage = self.storage.clone();
            tokio::task::spawn_blocking(move || {
                let handle = tokio::runtime::Handle::current();
                handle.block_on(async move {
                    let mut guard = storage.write();
                    guard.add_to_write_set(transaction_id, id, write_op).await
                })
            })
            .await
            .map_err(|e| CodeGraphError::Threading(e.to_string()))??;
        }

        Ok(())
    }

    async fn find_nodes_by_name(&self, name: &str) -> Result<Vec<CodeNode>> {
        let _transaction_id = self.get_current_transaction_id()?;

        // TODO: Implement transactional node search
        // This would involve:
        // 1. Reading from the current transaction's snapshot
        // 2. Applying any pending writes from the transaction
        // 3. Returning the merged view

        Ok(Vec::new())
    }
}

pub struct ReadOnlyTransactionalGraph {
    storage: Arc<RwLock<VersionedRocksDbStorage>>,
    snapshot_id: SnapshotId,
}

impl ReadOnlyTransactionalGraph {
    pub async fn new(
        storage: Arc<RwLock<VersionedRocksDbStorage>>,
        snapshot_id: SnapshotId,
    ) -> Self {
        Self {
            storage,
            snapshot_id,
        }
    }

    pub async fn at_version(
        storage: Arc<RwLock<VersionedRocksDbStorage>>,
        version_id: VersionId,
    ) -> Result<Self> {
        let snapshot_id = {
            let storage = storage.read();
            let version = storage
                .get_version(version_id)
                .await?
                .ok_or_else(|| CodeGraphError::Transaction("Version not found".to_string()))?;
            version.snapshot_id
        };

        Ok(Self::new(storage, snapshot_id).await)
    }
}

#[async_trait]
impl GraphStore for ReadOnlyTransactionalGraph {
    async fn add_node(&mut self, _node: CodeNode) -> Result<()> {
        Err(CodeGraphError::Transaction(
            "Cannot modify read-only graph".to_string(),
        ))
    }

    async fn get_node(&self, id: NodeId) -> Result<Option<CodeNode>> {
        // TODO: Read node from the specific snapshot
        // This involves looking up the content hash for this node in the snapshot
        // and then retrieving the content from the content store

        Ok(None)
    }

    async fn update_node(&mut self, _node: CodeNode) -> Result<()> {
        Err(CodeGraphError::Transaction(
            "Cannot modify read-only graph".to_string(),
        ))
    }

    async fn remove_node(&mut self, _id: NodeId) -> Result<()> {
        Err(CodeGraphError::Transaction(
            "Cannot modify read-only graph".to_string(),
        ))
    }

    async fn find_nodes_by_name(&self, _name: &str) -> Result<Vec<CodeNode>> {
        // TODO: Implement snapshot-based node search
        Ok(Vec::new())
    }
}

pub struct ConcurrentTransactionManager {
    storage: Arc<RwLock<VersionedRocksDbStorage>>,
    max_concurrent_transactions: usize,
    deadlock_detection_enabled: bool,
}

impl ConcurrentTransactionManager {
    pub fn new(
        storage: Arc<RwLock<VersionedRocksDbStorage>>,
        max_concurrent_transactions: usize,
    ) -> Self {
        Self {
            storage,
            max_concurrent_transactions,
            deadlock_detection_enabled: true,
        }
    }

    pub async fn create_transaction(
        &self,
        isolation_level: IsolationLevel,
    ) -> Result<TransactionalGraph> {
        let mut graph = TransactionalGraph {
            storage: self.storage.clone(),
            current_transaction: None,
            isolation_level,
        };

        graph
            .begin_transaction_with_isolation(isolation_level)
            .await?;
        Ok(graph)
    }

    pub async fn create_read_only_transaction_at_version(
        &self,
        version_id: VersionId,
    ) -> Result<ReadOnlyTransactionalGraph> {
        ReadOnlyTransactionalGraph::at_version(self.storage.clone(), version_id).await
    }

    pub async fn detect_deadlocks(&self) -> Result<Vec<TransactionId>> {
        if !self.deadlock_detection_enabled {
            return Ok(Vec::new());
        }

        // TODO: Implement deadlock detection algorithm
        // This would involve:
        // 1. Building a wait-for graph of transactions
        // 2. Detecting cycles in the graph
        // 3. Selecting victim transactions to abort

        Ok(Vec::new())
    }

    pub async fn get_transaction_statistics(&self) -> Result<TransactionStatistics> {
        // TODO: Collect statistics about active transactions
        Ok(TransactionStatistics {
            active_transactions: 0,
            committed_transactions: 0,
            aborted_transactions: 0,
            average_commit_time_ms: 0.0,
            deadlocks_detected: 0,
        })
    }
}

#[derive(Debug, Clone)]
pub struct TransactionStatistics {
    pub active_transactions: usize,
    pub committed_transactions: u64,
    pub aborted_transactions: u64,
    pub average_commit_time_ms: f64,
    pub deadlocks_detected: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use tokio_test;

    #[tokio::test]
    async fn test_basic_transaction() -> Result<()> {
        let temp_dir = tempdir().unwrap();
        let mut graph = TransactionalGraph::new(temp_dir.path().to_str().unwrap()).await?;

        // Begin transaction
        let tx_id = graph.begin_transaction().await?;
        assert!(graph.current_transaction.is_some());

        // Add a node
        let node = CodeNode::new(
            "test_function".to_string(),
            Some(codegraph_core::NodeType::Function),
            Some(codegraph_core::Language::Rust),
            codegraph_core::Location {
                file_path: "test.rs".to_string(),
                line: 1,
                column: 0,
                end_line: Some(10),
                end_column: Some(0),
            },
        );

        graph.add_node(node.clone()).await?;

        // Commit transaction
        graph.commit().await?;
        assert!(graph.current_transaction.is_none());

        Ok(())
    }

    #[tokio::test]
    async fn test_rollback_transaction() -> Result<()> {
        let temp_dir = tempdir().unwrap();
        let mut graph = TransactionalGraph::new(temp_dir.path().to_str().unwrap()).await?;

        // Begin transaction
        graph.begin_transaction().await?;

        // Add a node
        let node = CodeNode::new(
            "test_function".to_string(),
            Some(codegraph_core::NodeType::Function),
            Some(codegraph_core::Language::Rust),
            codegraph_core::Location {
                file_path: "test.rs".to_string(),
                line: 1,
                column: 0,
                end_line: Some(10),
                end_column: Some(0),
            },
        );

        graph.add_node(node.clone()).await?;

        // Rollback transaction
        graph.rollback().await?;
        assert!(graph.current_transaction.is_none());

        Ok(())
    }

    #[tokio::test]
    async fn test_isolation_levels() -> Result<()> {
        let temp_dir = tempdir().unwrap();
        let mut graph1 = TransactionalGraph::new(temp_dir.path().to_str().unwrap()).await?;
        let mut graph2 = TransactionalGraph::new(temp_dir.path().to_str().unwrap()).await?;

        // Test READ_COMMITTED isolation
        graph1
            .begin_transaction_with_isolation(IsolationLevel::ReadCommitted)
            .await?;
        graph2
            .begin_transaction_with_isolation(IsolationLevel::ReadCommitted)
            .await?;

        // TODO: Add tests for different isolation behaviors

        graph1.commit().await?;
        graph2.commit().await?;

        Ok(())
    }
}
