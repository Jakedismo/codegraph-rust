use crate::{CodeNode, NodeId, Result};
use async_trait::async_trait;

#[async_trait]
pub trait CodeParser {
    async fn parse_file(&self, file_path: &str) -> Result<Vec<CodeNode>>;
    fn supported_languages(&self) -> Vec<crate::Language>;
}

#[async_trait]
pub trait VectorStore {
    async fn store_embeddings(&mut self, nodes: &[CodeNode]) -> Result<()>;
    async fn search_similar(&self, query_embedding: &[f32], limit: usize) -> Result<Vec<NodeId>>;
    async fn get_embedding(&self, node_id: NodeId) -> Result<Option<Vec<f32>>>;
}

#[async_trait]
pub trait GraphStore {
    async fn add_node(&mut self, node: CodeNode) -> Result<()>;
    async fn get_node(&self, id: NodeId) -> Result<Option<CodeNode>>;
    async fn update_node(&mut self, node: CodeNode) -> Result<()>;
    async fn remove_node(&mut self, id: NodeId) -> Result<()>;
    async fn find_nodes_by_name(&self, name: &str) -> Result<Vec<CodeNode>>;
}