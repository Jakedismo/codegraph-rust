use codegraph_core::{EdgeId, EdgeType, NodeId};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};

static EDGE_COUNTER: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeEdge {
    pub id: EdgeId,
    pub from: NodeId,
    pub to: NodeId,
    pub edge_type: EdgeType,
    pub weight: f64,
    pub metadata: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HighPerformanceEdge {
    pub id: u64,
    pub from: NodeId,
    pub to: NodeId,
    pub edge_type: String,
    pub weight: f64,
    pub metadata: HashMap<String, String>,
}

impl CodeEdge {
    pub fn new(from: NodeId, to: NodeId, edge_type: EdgeType) -> Self {
        Self {
            id: EdgeId::new_v4(),
            from,
            to,
            edge_type,
            weight: 1.0,
            metadata: HashMap::new(),
        }
    }

    pub fn with_weight(mut self, weight: f64) -> Self {
        self.weight = weight;
        self
    }

    pub fn with_metadata(mut self, key: String, value: String) -> Self {
        self.metadata.insert(key, value);
        self
    }
}

impl HighPerformanceEdge {
    pub fn new(from: NodeId, to: NodeId, edge_type: String) -> Self {
        Self {
            id: EDGE_COUNTER.fetch_add(1, Ordering::SeqCst),
            from,
            to,
            edge_type,
            weight: 1.0,
            metadata: HashMap::new(),
        }
    }

    pub fn with_weight(mut self, weight: f64) -> Self {
        self.weight = weight;
        self
    }

    pub fn with_metadata(mut self, key: String, value: String) -> Self {
        self.metadata.insert(key, value);
        self
    }
}

impl From<CodeEdge> for HighPerformanceEdge {
    fn from(edge: CodeEdge) -> Self {
        Self {
            id: EDGE_COUNTER.fetch_add(1, Ordering::SeqCst),
            from: edge.from,
            to: edge.to,
            edge_type: edge.edge_type.to_string(),
            weight: edge.weight,
            metadata: edge.metadata,
        }
    }
}