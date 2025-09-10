use codegraph_core::{NodeId, EdgeType};

#[derive(Debug, Clone)]
pub struct CodeEdge {
    pub from: NodeId,
    pub to: NodeId,
    pub edge_type: EdgeType,
    pub metadata: std::collections::HashMap<String, String>,
}
impl CodeEdge {
    pub fn new(from: NodeId, to: NodeId, edge_type: EdgeType) -> Self {
        Self {
            from,
            to,
            edge_type,
            metadata: std::collections::HashMap::new(),
        }
    }

    pub fn with_metadata(mut self, key: String, value: String) -> Self {
        self.metadata.insert(key, value);
        self
    }
}
