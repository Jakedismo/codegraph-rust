use crate::{generate_node_id, Language, Location, Metadata, NodeId, NodeType, SharedStr};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeNode {
    pub id: NodeId,
    pub name: SharedStr,
    pub node_type: Option<NodeType>,
    pub language: Option<Language>,
    pub location: Location,
    pub span: Option<crate::Span>,
    pub content: Option<SharedStr>,
    pub metadata: Metadata,
    pub embedding: Option<Vec<f32>>,
    pub complexity: Option<f32>,
}

impl CodeNode {
    pub fn new<T: Into<SharedStr>>(
        name: T,
        node_type: Option<NodeType>,
        language: Option<Language>,
        location: Location,
    ) -> Self {
        let now = chrono::Utc::now();
        Self {
            id: NodeId::new_v4(),
            name: name.into(),
            node_type,
            language,
            location,
            span: None,
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

    pub fn with_content<T: Into<SharedStr>>(mut self, content: T) -> Self {
        self.content = Some(content.into());
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

    /// Regenerate the node ID deterministically based on project context.
    /// This enables proper UPSERT behavior - same code entity = same ID across indexing runs.
    pub fn with_deterministic_id(mut self, project_id: &str) -> Self {
        self.set_deterministic_id(project_id);
        self
    }

    /// Regenerate the node ID deterministically in place.
    /// This enables proper UPSERT behavior - same code entity = same ID across indexing runs.
    pub fn set_deterministic_id(&mut self, project_id: &str) {
        let node_type_str = match &self.node_type {
            Some(nt) => format!("{:?}", nt),
            None => "Unknown".to_string(),
        };
        self.id = generate_node_id(
            project_id,
            &self.location.file_path,
            &self.name,
            &node_type_str,
            self.location.line,
        );
    }

    pub fn new_test() -> Self {
        let location = crate::Location {
            file_path: "test".to_string(),
            line: 1,
            column: 0,
            end_line: Some(1),
            end_column: Some(0),
        };
        CodeNode::new("test", None, None, location)
    }
}
