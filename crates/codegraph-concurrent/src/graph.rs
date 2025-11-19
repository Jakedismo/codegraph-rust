use arc_swap::ArcSwap;
use async_trait::async_trait;
use codegraph_core::GraphStore;
use codegraph_core::{CodeNode, NodeId, Result as CgResult};
use crossbeam_skiplist::SkipMap;
use std::sync::Arc;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum GraphError {
    #[error("node not found: {0}")]
    NotFound(NodeId),
    #[error("internal error: {0}")]
    Internal(String),
}

/// Lock-free adjacency graph using lock-free concurrent maps and ArcSwap for RCU-style updates.
/// - Nodes stored in `SkipMap` for lock-free gets/inserts.
/// - Adjacency lists per node are stored behind `ArcSwap<Vec<NodeId>>` to allow
///   lock-free reads and atomic updates (copy-on-write on update path).
#[derive(Debug, Default)]
pub struct LockFreeAdjacencyGraph {
    nodes: SkipMap<NodeId, Arc<CodeNode>>, // lock-free map
    adjacency: SkipMap<NodeId, Arc<ArcSwap<Vec<NodeId>>>>, // lock-free map of lock-free lists
}

impl LockFreeAdjacencyGraph {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a directed edge from -> to. Creates adjacency entry if missing.
    pub fn add_edge(&self, from: NodeId, to: NodeId) {
        let entry = self
            .adjacency
            .get(&from)
            .map(|e| e.value().clone())
            .unwrap_or_else(|| {
                let swap = Arc::new(ArcSwap::from_pointee(Vec::<NodeId>::new()));
                self.adjacency.insert(from, swap.clone());
                swap
            });

        // RCU-style copy-on-write update: retry on contention
        entry.rcu(|current| {
            let mut next = (**current).clone();
            next.push(to);
            next
        });
    }

    /// Get neighbors as a cloned Vec. Readers are lock-free.
    pub fn neighbors(&self, from: NodeId) -> Vec<NodeId> {
        self.adjacency
            .get(&from)
            .map(|e| (*e.value().load().as_ref()).clone())
            .unwrap_or_default()
    }
}

#[async_trait]
impl GraphStore for LockFreeAdjacencyGraph {
    async fn add_node(&mut self, node: CodeNode) -> CgResult<()> {
        self.nodes.insert(node.id, Arc::new(node));
        Ok(())
    }

    async fn get_node(&self, id: NodeId) -> CgResult<Option<CodeNode>> {
        Ok(self.nodes.get(&id).map(|e| e.value().as_ref().clone()))
    }

    async fn update_node(&mut self, node: CodeNode) -> CgResult<()> {
        self.nodes.insert(node.id, Arc::new(node));
        Ok(())
    }

    async fn remove_node(&mut self, id: NodeId) -> CgResult<()> {
        self.nodes.remove(&id);
        self.adjacency.remove(&id);
        Ok(())
    }

    async fn find_nodes_by_name(&self, name: &str) -> CgResult<Vec<CodeNode>> {
        let mut out = Vec::new();
        for e in self.nodes.iter() {
            if e.value().name.as_str() == name {
                out.push(e.value().as_ref().clone());
            }
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use codegraph_core::{Language, Location, NodeType};
    use uuid::Uuid;

    fn sample_node(name: &str) -> CodeNode {
        CodeNode::new(
            name.to_string(),
            Some(NodeType::Function),
            Some(Language::Rust),
            Location {
                file_path: "x".into(),
                line: 1,
                column: 1,
                end_line: None,
                end_column: None,
            },
        )
    }

    #[tokio::test]
    async fn lockfree_graph_nodes() {
        let mut g = LockFreeAdjacencyGraph::new();
        let n = sample_node("a");
        let id = n.id;
        g.add_node(n.clone()).await.unwrap();
        assert_eq!(g.get_node(id).await.unwrap().unwrap().name, "a".into());
        let found = g.find_nodes_by_name("a").await.unwrap();
        assert_eq!(found.len(), 1);
    }

    #[test]
    fn lockfree_graph_edges() {
        let g = LockFreeAdjacencyGraph::new();
        let a = Uuid::new_v4();
        let b = Uuid::new_v4();
        let c = Uuid::new_v4();
        g.add_edge(a, b);
        g.add_edge(a, c);
        let mut n = g.neighbors(a);
        n.sort();
        let mut expected = vec![b, c];
        expected.sort();
        assert_eq!(n, expected);
    }
}
