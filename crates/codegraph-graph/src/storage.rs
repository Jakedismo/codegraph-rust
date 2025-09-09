use async_trait::async_trait;
use codegraph_core::{CodeGraphError, CodeNode, GraphStore, NodeId, Result};
use rocksdb::{DB, Options};
use std::path::Path;

pub struct RocksDbStorage {
    db: DB,
}

impl RocksDbStorage {
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self> {
        let mut opts = Options::default();
        opts.create_if_missing(true);
        opts.set_max_background_jobs(6);
        opts.set_bytes_per_sync(1048576);

        let db = DB::open(&opts, path)
            .map_err(|e| CodeGraphError::Database(e.to_string()))?;

        Ok(Self { db })
    }

    fn node_key(id: NodeId) -> String {
        format!("node:{}", id)
    }

    fn name_index_key(name: &str, id: NodeId) -> String {
        format!("name_idx:{}:{}", name, id)
    }
}

#[async_trait]
impl GraphStore for RocksDbStorage {
    async fn add_node(&mut self, node: CodeNode) -> Result<()> {
        let key = Self::node_key(node.id);
        let value = bincode::serialize(&node)
            .map_err(|e| CodeGraphError::Database(e.to_string()))?;

        self.db
            .put(&key, value)
            .map_err(|e| CodeGraphError::Database(e.to_string()))?;

        let name_key = Self::name_index_key(&node.name, node.id);
        self.db
            .put(&name_key, b"")
            .map_err(|e| CodeGraphError::Database(e.to_string()))?;

        Ok(())
    }

    async fn get_node(&self, id: NodeId) -> Result<Option<CodeNode>> {
        let key = Self::node_key(id);
        match self.db.get(&key) {
            Ok(Some(value)) => {
                let node = bincode::deserialize(&value)
                    .map_err(|e| CodeGraphError::Database(e.to_string()))?;
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
        let key = Self::node_key(id);

        if let Some(node_data) = self.db.get(&key)
            .map_err(|e| CodeGraphError::Database(e.to_string()))? {
            let node: CodeNode = bincode::deserialize(&node_data)
                .map_err(|e| CodeGraphError::Database(e.to_string()))?;

            let name_key = Self::name_index_key(&node.name, id);
            self.db
                .delete(&name_key)
                .map_err(|e| CodeGraphError::Database(e.to_string()))?;
        }

        self.db
            .delete(&key)
            .map_err(|e| CodeGraphError::Database(e.to_string()))?;

        Ok(())
    }

    async fn find_nodes_by_name(&self, name: &str) -> Result<Vec<CodeNode>> {
        let prefix = format!("name_idx:{}", name);
        let iter = self.db.prefix_iterator(&prefix);
        
        let mut nodes = Vec::new();
        for item in iter {
            let (key, _) = item.map_err(|e| CodeGraphError::Database(e.to_string()))?;
            let key_str = String::from_utf8_lossy(&key);
            
            if let Some(id_str) = key_str.split(':').nth(2) {
                if let Ok(id) = id_str.parse::<NodeId>() {
                    if let Some(node) = self.get_node(id).await? {
                        nodes.push(node);
                    }
                }
            }
        }

        Ok(nodes)
    }
}