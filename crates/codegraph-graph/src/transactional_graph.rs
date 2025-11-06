#![allow(dead_code, unused_variables, unused_imports)]

use async_trait::async_trait;
use codegraph_core::{
    CodeGraphError, CodeNode, GraphStore, IsolationLevel, NodeId, Result, SnapshotId,
    TransactionId, TransactionManager, VersionId, VersionedStore, WriteOperation,
};
use parking_lot::RwLock;
use std::sync::Arc;

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
    async fn add_node(&mut self, node: CodeNode) -> Result<()> {
        let transaction_id = self.get_current_transaction_id()?;

        // Serialize the node for content-addressed storage
        let serialized =
            serde_json::to_vec(&node).map_err(|e| CodeGraphError::Database(e.to_string()))?;

        // Store the content and get the hash
        let content_hash = {
            let storage = self.storage.clone();
            let serialized_clone = serialized.clone();
            tokio::task::spawn_blocking(move || {
                let handle = tokio::runtime::Handle::current();
                handle.block_on(async move {
                    let mut guard = storage.write();
                    guard.store_content(&serialized_clone).await
                })
            })
            .await
            .map_err(|e| CodeGraphError::Threading(e.to_string()))??
        };

        // Add to transaction's write set with the content hash
        let write_op = WriteOperation::Update {
            old_content_hash: String::new(), // New insert has no old content
            new_content_hash: content_hash,
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
        let transaction = {
            let storage = self.storage.clone();
            tokio::task::spawn_blocking(move || {
                let handle = tokio::runtime::Handle::current();
                handle.block_on(async move {
                    let guard = storage.read();
                    guard.get_transaction(transaction_id).await
                })
            })
            .await
            .map_err(|e| CodeGraphError::Threading(e.to_string()))??
        };

        if let Some(tx) = transaction {
            if let Some(write_op) = tx.write_set.get(&id) {
                // Node is being modified in this transaction
                match write_op {
                    WriteOperation::Delete(_) => {
                        // Node was deleted in this transaction
                        return Ok(None);
                    }
                    WriteOperation::Update { new_content_hash, .. } => {
                        // Retrieve the modified version from content store
                        let content_opt = {
                            let storage = self.storage.clone();
                            let hash = new_content_hash.clone();
                            tokio::task::spawn_blocking(move || {
                                let handle = tokio::runtime::Handle::current();
                                handle.block_on(async move {
                                    let guard = storage.read();
                                    guard.get_content(&hash).await
                                })
                            })
                            .await
                            .map_err(|e| CodeGraphError::Threading(e.to_string()))??
                        };

                        if let Some(content) = content_opt {
                            let node: CodeNode = serde_json::from_slice(&content)
                                .map_err(|e| CodeGraphError::Database(e.to_string()))?;
                            return Ok(Some(node));
                        }
                    }
                    WriteOperation::Insert(_) => {
                        // This shouldn't happen as we use Update for inserts now
                    }
                }
            }

            // If not in write set, read from the transaction's snapshot
            let snapshot_id = tx.snapshot_id;
            let node_opt = {
                let storage = self.storage.clone();
                tokio::task::spawn_blocking(move || {
                    let handle = tokio::runtime::Handle::current();
                    handle.block_on(async move {
                        let guard = storage.read();
                        guard.read_node_at_snapshot(id, snapshot_id).await
                    })
                })
                .await
                .map_err(|e| CodeGraphError::Threading(e.to_string()))??
            };

            return Ok(node_opt);
        }

        Ok(None)
    }

    async fn update_node(&mut self, node: CodeNode) -> Result<()> {
        let transaction_id = self.get_current_transaction_id()?;

        // Get the current version of the node to create before/after images
        let old_node = self.get_node(node.id).await?;

        // Get old hash if node exists
        let old_hash = if let Some(ref old) = old_node {
            let serialized =
                serde_json::to_vec(old).map_err(|e| CodeGraphError::Database(e.to_string()))?;
            let storage = self.storage.clone();
            let serialized_clone = serialized.clone();
            tokio::task::spawn_blocking(move || {
                let handle = tokio::runtime::Handle::current();
                handle.block_on(async move {
                    let mut guard = storage.write();
                    guard.store_content(&serialized_clone).await
                })
            })
            .await
            .map_err(|e| CodeGraphError::Threading(e.to_string()))??
        } else {
            String::new()
        };

        // Serialize and store the new node content
        let new_serialized =
            serde_json::to_vec(&node).map_err(|e| CodeGraphError::Database(e.to_string()))?;

        let new_hash = {
            let storage = self.storage.clone();
            let serialized_clone = new_serialized.clone();
            tokio::task::spawn_blocking(move || {
                let handle = tokio::runtime::Handle::current();
                handle.block_on(async move {
                    let mut guard = storage.write();
                    guard.store_content(&serialized_clone).await
                })
            })
            .await
            .map_err(|e| CodeGraphError::Threading(e.to_string()))??
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
        let transaction_id = self.get_current_transaction_id()?;

        // Get transaction to access snapshot and write set
        let transaction = {
            let storage = self.storage.clone();
            tokio::task::spawn_blocking(move || {
                let handle = tokio::runtime::Handle::current();
                handle.block_on(async move {
                    let guard = storage.read();
                    guard.get_transaction(transaction_id).await
                })
            })
            .await
            .map_err(|e| CodeGraphError::Threading(e.to_string()))??
        };

        if let Some(tx) = transaction {
            let snapshot_id = tx.snapshot_id;
            let write_set = tx.write_set.clone();

            // Get the snapshot to iterate through all nodes
            let snapshot = {
                let storage = self.storage.clone();
                tokio::task::spawn_blocking(move || {
                    let handle = tokio::runtime::Handle::current();
                    handle.block_on(async move {
                        let guard = storage.read();
                        guard.get_snapshot(snapshot_id).await
                    })
                })
                .await
                .map_err(|e| CodeGraphError::Threading(e.to_string()))??
            };

            if let Some(snap) = snapshot {
                let mut result_nodes = Vec::new();
                let storage = self.storage.clone();
                let name_owned = name.to_string();

                // Iterate through all nodes in the snapshot
                for (node_id, _content_hash) in snap.node_versions {
                    // Check if this node was deleted in the write set
                    if let Some(WriteOperation::Delete(_)) = write_set.get(&node_id) {
                        continue; // Skip deleted nodes
                    }

                    // Try to get the node (will check write set first, then snapshot)
                    if let Some(node) = self.get_node(node_id).await? {
                        if node.name == name_owned {
                            result_nodes.push(node);
                        }
                    }
                }

                // Check write set for newly inserted nodes not in the snapshot
                for (node_id, write_op) in write_set {
                    if !snap.node_versions.contains_key(&node_id) {
                        // This is a new node added in this transaction
                        if let WriteOperation::Update { new_content_hash, .. } = write_op {
                            // Retrieve the node from content store
                            let content_opt = {
                                let storage = storage.clone();
                                let hash = new_content_hash.clone();
                                tokio::task::spawn_blocking(move || {
                                    let handle = tokio::runtime::Handle::current();
                                    handle.block_on(async move {
                                        let guard = storage.read();
                                        guard.get_content(&hash).await
                                    })
                                })
                                .await
                                .map_err(|e| CodeGraphError::Threading(e.to_string()))??
                            };

                            if let Some(content) = content_opt {
                                let node: CodeNode = serde_json::from_slice(&content)
                                    .map_err(|e| CodeGraphError::Database(e.to_string()))?;
                                if node.name == name_owned {
                                    result_nodes.push(node);
                                }
                            }
                        }
                    }
                }

                return Ok(result_nodes);
            }
        }

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
        // Read node from the specific snapshot
        let storage = self.storage.clone();
        let snapshot_id = self.snapshot_id;

        let node_opt = tokio::task::spawn_blocking(move || {
            let handle = tokio::runtime::Handle::current();
            handle.block_on(async move {
                let guard = storage.read();
                guard.read_node_at_snapshot(id, snapshot_id).await
            })
        })
        .await
        .map_err(|e| CodeGraphError::Threading(e.to_string()))??;

        Ok(node_opt)
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

    async fn find_nodes_by_name(&self, name: &str) -> Result<Vec<CodeNode>> {
        let storage = self.storage.clone();
        let snapshot_id = self.snapshot_id;
        let name_owned = name.to_string();

        // Get the snapshot
        let snapshot = tokio::task::spawn_blocking(move || {
            let handle = tokio::runtime::Handle::current();
            handle.block_on(async move {
                let guard = storage.read();
                guard.get_snapshot(snapshot_id).await
            })
        })
        .await
        .map_err(|e| CodeGraphError::Threading(e.to_string()))??;

        if let Some(snap) = snapshot {
            let mut result_nodes = Vec::new();

            // Iterate through all nodes in the snapshot
            for (node_id, _content_hash) in snap.node_versions {
                if let Some(node) = self.get_node(node_id).await? {
                    if node.name == name_owned {
                        result_nodes.push(node);
                    }
                }
            }

            return Ok(result_nodes);
        }

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
