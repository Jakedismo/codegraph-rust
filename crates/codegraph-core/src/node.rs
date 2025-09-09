use crate::{Location, Metadata, NodeId, NodeType, Language};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeNode {
    pub id: NodeId,
    pub name: String,
    pub node_type: Option<NodeType>,
    pub language: Option<Language>,
    pub location: Location,
    pub content: Option<String>,
    pub metadata: Metadata,
    pub embedding: Option<Vec<f32>>,
    pub complexity: Option<f32>,
}

impl CodeNode {
    pub fn new(
        name: String,
        node_type: Option<NodeType>,
        language: Option<Language>,
        location: Location,
    ) -> Self {
        let now = chrono::Utc::now();
        Self {
            id: NodeId::new_v4(),
            name,
            node_type,
            language,
            location,
            content: None,
            metadata: Metadata {
                attributes: std::collections::HashMap::new(),
                created_at: now,
                updated_at: now,
            },
            embedding: None,
            complexity: None,
        }
    }

    pub fn with_content(mut self, content: String) -> Self {
        self.content = Some(content);
        self
    }

    pub fn with_embedding(mut self, embedding: Vec<f32>) -> Self {
        self.embedding = Some(embedding);
        self
    }

    pub fn with_complexity(mut self, complexity: f32) -> Self {
        self.complexity = Some(complexity);
        self
    }
}