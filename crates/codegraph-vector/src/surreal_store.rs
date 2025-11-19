// ABOUTME: Provides SurrealDB-backed vector store abstractions for embeddings and search.
// ABOUTME: Bridges Surreal storage with the generic VectorStore trait and helper utilities.

use async_trait::async_trait;
use codegraph_core::{CodeGraphError, CodeNode, GraphStore, NodeId, Result, VectorStore};
use codegraph_graph::{surreal_embedding_column_for_dimension, SurrealDbStorage};
use std::sync::Arc;
use tokio::sync::Mutex as TokioMutex;

/// Backend abstraction to allow mocking Surreal vector operations in tests.
#[async_trait]
pub trait SurrealVectorBackend: Send + Sync {
    async fn upsert_nodes(&self, nodes: &[CodeNode]) -> Result<()>;
    async fn vector_knn(
        &self,
        column: &str,
        query_embedding: Vec<f32>,
        limit: usize,
        ef_search: usize,
    ) -> Result<Vec<(String, f32)>>;
    async fn get_node_embedding(&self, node_id: NodeId) -> Result<Option<Vec<f32>>>;
}

/// Surreal-backed vector store that fulfills the VectorStore trait.
#[derive(Clone)]
pub struct SurrealVectorStore {
    backend: Arc<dyn SurrealVectorBackend>,
    ef_search: usize,
}

impl SurrealVectorStore {
    pub fn new(backend: Arc<dyn SurrealVectorBackend>, ef_search: usize) -> Self {
        Self { backend, ef_search }
    }

    pub fn with_surreal_storage(
        storage: Arc<TokioMutex<SurrealDbStorage>>,
        ef_search: usize,
    ) -> Self {
        let backend = Arc::new(SurrealStorageBackend::new(storage));
        Self::new(backend, ef_search)
    }
}

pub struct SurrealStorageBackend {
    storage: Arc<TokioMutex<SurrealDbStorage>>,
}

impl SurrealStorageBackend {
    pub fn new(storage: Arc<TokioMutex<SurrealDbStorage>>) -> Self {
        Self { storage }
    }
}

#[async_trait]
impl VectorStore for SurrealVectorStore {
    async fn store_embeddings(&mut self, nodes: &[CodeNode]) -> Result<()> {
        self.backend.upsert_nodes(nodes).await
    }

    async fn search_similar(&self, query_embedding: &[f32], limit: usize) -> Result<Vec<NodeId>> {
        if query_embedding.is_empty() || limit == 0 {
            return Ok(Vec::new());
        }

        let column = surreal_embedding_column_for_dimension(query_embedding.len());
        let neighbors = self
            .backend
            .vector_knn(column, query_embedding.to_vec(), limit, self.ef_search)
            .await?;

        let mut node_ids = Vec::with_capacity(neighbors.len());
        for (raw_id, _) in neighbors {
            let normalized = normalize_surreal_node_id(&raw_id);
            let node_id = NodeId::parse_str(normalized).map_err(|err| {
                CodeGraphError::Vector(format!(
                    "Invalid node id '{}' returned by Surreal search: {}",
                    raw_id, err
                ))
            })?;
            node_ids.push(node_id);
        }

        Ok(node_ids)
    }

    async fn get_embedding(&self, node_id: NodeId) -> Result<Option<Vec<f32>>> {
        self.backend.get_node_embedding(node_id).await
    }
}

#[async_trait]
impl SurrealVectorBackend for SurrealStorageBackend {
    async fn upsert_nodes(&self, nodes: &[CodeNode]) -> Result<()> {
        if nodes.is_empty() {
            return Ok(());
        }

        let mut guard = self.storage.lock().await;
        guard.upsert_nodes_batch(nodes).await
    }

    async fn vector_knn(
        &self,
        column: &str,
        query_embedding: Vec<f32>,
        limit: usize,
        ef_search: usize,
    ) -> Result<Vec<(String, f32)>> {
        let guard = self.storage.lock().await;
        guard
            .vector_search_knn(column, query_embedding, limit, ef_search)
            .await
    }

    async fn get_node_embedding(&self, node_id: NodeId) -> Result<Option<Vec<f32>>> {
        let guard = self.storage.lock().await;
        let node = guard.get_node(node_id).await?;
        Ok(node.and_then(|n| n.embedding))
    }
}

fn normalize_surreal_node_id(raw_id: &str) -> &str {
    raw_id
        .rsplit_once(':')
        .map(|(_, tail)| tail)
        .unwrap_or(raw_id)
}

#[cfg(test)]
mod tests {
    use super::{normalize_surreal_node_id, SurrealVectorBackend, SurrealVectorStore};
    use async_trait::async_trait;
    use codegraph_core::{CodeNode, NodeId, Result, VectorStore};
    use parking_lot::Mutex;
    use std::sync::Arc;

    #[test]
    fn strips_table_prefix_from_ids() {
        let raw = "nodes:018f3b7d-a82d-4f40-9127-2db4beefabcd";
        let normalized = normalize_surreal_node_id(raw);
        assert_eq!(normalized, "018f3b7d-a82d-4f40-9127-2db4beefabcd");
    }

    #[test]
    fn keeps_clean_ids_intact() {
        let raw = "018f3b7d-a82d-4f40-9127-2db4beefabcd";
        assert_eq!(normalize_surreal_node_id(raw), raw);
    }

    #[tokio::test]
    async fn search_similar_uses_surreal_backend() {
        let uuid = "018f3b7d-a82d-4f40-9127-2db4beefabcd".to_string();
        let backend = Arc::new(MockBackend::new(vec![(format!("nodes:{uuid}"), 0.42)]));
        let store = SurrealVectorStore::new(backend.clone(), 128);
        let embedding = vec![0.0f32; 2560];
        let results = store.search_similar(&embedding, 3).await.unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].to_string(), uuid);
        assert_eq!(
            backend.recorded_columns(),
            vec!["embedding_2560".to_string()]
        );
    }

    struct MockBackend {
        results: Mutex<Vec<(String, f32)>>,
        columns: Mutex<Vec<String>>,
    }

    impl MockBackend {
        fn new(results: Vec<(String, f32)>) -> Self {
            Self {
                results: Mutex::new(results),
                columns: Mutex::new(Vec::new()),
            }
        }

        fn recorded_columns(&self) -> Vec<String> {
            self.columns.lock().clone()
        }
    }

    #[async_trait]
    impl SurrealVectorBackend for MockBackend {
        async fn upsert_nodes(&self, _nodes: &[CodeNode]) -> Result<()> {
            Ok(())
        }

        async fn vector_knn(
            &self,
            column: &str,
            _query_embedding: Vec<f32>,
            _limit: usize,
            _ef_search: usize,
        ) -> Result<Vec<(String, f32)>> {
            self.columns.lock().push(column.to_string());
            Ok(self.results.lock().clone())
        }

        async fn get_node_embedding(&self, _node_id: NodeId) -> Result<Option<Vec<f32>>> {
            Ok(None)
        }
    }
}
