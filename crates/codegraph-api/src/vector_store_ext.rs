// Extension methods for FaissVectorStore
use codegraph_core::{NodeId, Result};
use codegraph_vector::FaissVectorStore;
use std::sync::Arc;
use uuid::Uuid;

pub trait FaissVectorStoreExt {
    async fn search_similar(&self, embedding: &[f32], k: usize) -> Result<Vec<NodeId>>;
    async fn get_embedding(&self, node_id: NodeId) -> Result<Option<Vec<f32>>>;
}

impl FaissVectorStoreExt for Arc<FaissVectorStore> {
    async fn search_similar(&self, _embedding: &[f32], k: usize) -> Result<Vec<NodeId>> {
        // Return dummy results for now
        let mut results = Vec::new();
        for _ in 0..k.min(5) {
            results.push(Uuid::new_v4());
        }
        Ok(results)
    }

    async fn get_embedding(&self, _node_id: NodeId) -> Result<Option<Vec<f32>>> {
        // Return a dummy embedding
        Ok(Some(vec![0.0; 384]))
    }
}
