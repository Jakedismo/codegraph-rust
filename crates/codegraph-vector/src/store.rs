use async_trait::async_trait;
use codegraph_core::{CodeGraphError, CodeNode, NodeId, Result, VectorStore};
use faiss::{index::IndexImpl, Index, MetricType};
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;

pub struct FaissVectorStore {
    index: Arc<RwLock<Option<IndexImpl>>>,
    id_mapping: Arc<RwLock<HashMap<i64, NodeId>>>,
    reverse_mapping: Arc<RwLock<HashMap<NodeId, i64>>>,
    embeddings: Arc<RwLock<HashMap<NodeId, Vec<f32>>>>,
    dimension: usize,
    next_id: Arc<RwLock<i64>>,
}

impl FaissVectorStore {
    pub fn new(dimension: usize) -> Result<Self> {
        Ok(Self {
            index: Arc::new(RwLock::new(None)),
            id_mapping: Arc::new(RwLock::new(HashMap::new())),
            reverse_mapping: Arc::new(RwLock::new(HashMap::new())),
            embeddings: Arc::new(RwLock::new(HashMap::new())),
            dimension,
            next_id: Arc::new(RwLock::new(0)),
        })
    }

    fn ensure_index(&self) -> Result<()> {
        let mut index_guard = self.index.write();
        if index_guard.is_none() {
            let index =
                faiss::index_factory(self.dimension as u32, "Flat", MetricType::InnerProduct)
                    .map_err(|e| CodeGraphError::Vector(e.to_string()))?;
            *index_guard = Some(index);
        }
        Ok(())
    }

    fn get_next_id(&self) -> i64 {
        let mut next_id = self.next_id.write();
        let id = *next_id;
        *next_id += 1;
        id
    }

    pub async fn build_index(&mut self, nodes: &[CodeNode]) -> Result<()> {
        self.ensure_index()?;

        let embeddings: Vec<_> = nodes
            .iter()
            .filter_map(|node| node.embedding.as_ref().map(|emb| (node.id, emb.clone())))
            .collect();

        if embeddings.is_empty() {
            return Ok(());
        }

        let vectors: Vec<f32> = embeddings
            .iter()
            .flat_map(|(_, emb)| emb.iter().cloned())
            .collect();

        let mut index_guard = self.index.write();
        let index = index_guard.as_mut().unwrap();

        index
            .add(&vectors)
            .map_err(|e| CodeGraphError::Vector(e.to_string()))?;

        let mut id_mapping = self.id_mapping.write();
        let mut reverse_mapping = self.reverse_mapping.write();
        let mut stored_embeddings = self.embeddings.write();

        for (node_id, embedding) in embeddings {
            let faiss_id = self.get_next_id();
            id_mapping.insert(faiss_id, node_id);
            reverse_mapping.insert(node_id, faiss_id);
            stored_embeddings.insert(node_id, embedding);
        }

        Ok(())
    }
}

#[async_trait]
impl VectorStore for FaissVectorStore {
    async fn store_embeddings(&mut self, nodes: &[CodeNode]) -> Result<()> {
        self.build_index(nodes).await
    }

    async fn search_similar(&self, query_embedding: &[f32], limit: usize) -> Result<Vec<NodeId>> {
        self.ensure_index()?;

        if query_embedding.len() != self.dimension {
            return Err(CodeGraphError::Vector(format!(
                "Query embedding dimension {} doesn't match index dimension {}",
                query_embedding.len(),
                self.dimension
            )));
        }

        let mut index_guard = self.index.write();
        let index = index_guard
            .as_mut()
            .ok_or_else(|| CodeGraphError::Vector("Index not initialized".to_string()))?;

        let results = index
            .search(query_embedding, limit)
            .map_err(|e| CodeGraphError::Vector(e.to_string()))?;

        let id_mapping = self.id_mapping.read();
        let node_ids: Vec<NodeId> = results
            .labels
            .into_iter()
            .filter_map(|faiss_id| {
                faiss_id
                    .get()
                    .and_then(|v| id_mapping.get(&(v as i64)).cloned())
            })
            .collect();

        Ok(node_ids)
    }

    async fn get_embedding(&self, node_id: NodeId) -> Result<Option<Vec<f32>>> {
        let embeddings = self.embeddings.read();
        Ok(embeddings.get(&node_id).cloned())
    }
}
