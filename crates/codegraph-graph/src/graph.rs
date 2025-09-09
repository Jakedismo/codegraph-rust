use crate::{CodeEdge, RocksDbStorage};
use codegraph_core::{CodeGraphError, CodeNode, GraphStore, NodeId, Result};
use async_trait::async_trait;
use dashmap::DashMap;
use std::sync::Arc;

pub struct CodeGraph {
    storage: Box<dyn GraphStore + Send + Sync>,
    node_cache: Arc<DashMap<NodeId, CodeNode>>,
    edge_cache: Arc<DashMap<NodeId, Vec<CodeEdge>>>,
}

impl CodeGraph {
    pub fn new() -> Self {
        Self {
            storage: Box::new(RocksDbStorage::new("./data/graph.db").unwrap()),
            node_cache: Arc::new(DashMap::new()),
            edge_cache: Arc::new(DashMap::new()),
        }
    }

    pub async fn add_edge(&mut self, edge: CodeEdge) -> Result<()> {
        let from_node = edge.from;
        self.edge_cache
            .entry(from_node)
            .or_insert_with(Vec::new)
            .push(edge.clone());

        Ok(())
    }

    pub async fn get_edges_from(&self, node_id: NodeId) -> Result<Vec<CodeEdge>> {
        if let Some(edges) = self.edge_cache.get(&node_id) {
            Ok(edges.clone())
        } else {
            Ok(Vec::new())
        }
    }

    pub async fn get_neighbors(&self, node_id: NodeId) -> Result<Vec<NodeId>> {
        let edges = self.get_edges_from(node_id).await?;
        Ok(edges.into_iter().map(|e| e.to).collect())
    }

    pub async fn shortest_path(&self, from: NodeId, to: NodeId) -> Result<Option<Vec<NodeId>>> {
        use std::collections::{HashMap, VecDeque};

        let mut queue = VecDeque::new();
        let mut visited = std::collections::HashSet::new();
        let mut parent: HashMap<NodeId, NodeId> = HashMap::new();

        queue.push_back(from);
        visited.insert(from);

        while let Some(current) = queue.pop_front() {
            if current == to {
                let mut path = Vec::new();
                let mut node = to;
                path.push(node);

                while let Some(&prev) = parent.get(&node) {
                    path.push(prev);
                    node = prev;
                }

                path.reverse();
                return Ok(Some(path));
            }

            let neighbors = self.get_neighbors(current).await?;
            for neighbor in neighbors {
                if !visited.contains(&neighbor) {
                    visited.insert(neighbor);
                    parent.insert(neighbor, current);
                    queue.push_back(neighbor);
                }
            }
        }

        Ok(None)
    }
}

#[async_trait]
impl GraphStore for CodeGraph {
    async fn add_node(&mut self, node: CodeNode) -> Result<()> {
        let id = node.id;
        self.storage.add_node(node.clone()).await?;
        self.node_cache.insert(id, node);
        Ok(())
    }

    async fn get_node(&self, id: NodeId) -> Result<Option<CodeNode>> {
        if let Some(node) = self.node_cache.get(&id) {
            Ok(Some(node.clone()))
        } else {
            let node = self.storage.get_node(id).await?;
            if let Some(ref n) = node {
                self.node_cache.insert(id, n.clone());
            }
            Ok(node)
        }
    }

    async fn update_node(&mut self, node: CodeNode) -> Result<()> {
        let id = node.id;
        self.storage.update_node(node.clone()).await?;
        self.node_cache.insert(id, node);
        Ok(())
    }

    async fn remove_node(&mut self, id: NodeId) -> Result<()> {
        self.storage.remove_node(id).await?;
        self.node_cache.remove(&id);
        self.edge_cache.remove(&id);
        Ok(())
    }

    async fn find_nodes_by_name(&self, name: &str) -> Result<Vec<CodeNode>> {
        self.storage.find_nodes_by_name(name).await
    }
}