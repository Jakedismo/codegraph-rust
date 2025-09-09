use crate::{EmbeddingGenerator, FaissVectorStore};
use codegraph_core::{CodeGraphError, CodeNode, NodeId, Result, VectorStore};
use std::sync::Arc;

#[derive(Clone)]
pub struct SearchResult {
    pub node_id: NodeId,
    pub score: f32,
    pub node: Option<CodeNode>,
}

pub struct SemanticSearch {
    vector_store: Arc<FaissVectorStore>,
    embedding_generator: Arc<EmbeddingGenerator>,
}

impl SemanticSearch {
    pub fn new(
        vector_store: Arc<FaissVectorStore>,
        embedding_generator: Arc<EmbeddingGenerator>,
    ) -> Self {
        Self {
            vector_store,
            embedding_generator,
        }
    }

    pub async fn search_by_text(&self, query: &str, limit: usize) -> Result<Vec<SearchResult>> {
        let query_embedding = self.encode_query(query).await?;
        self.search_by_embedding(&query_embedding, limit).await
    }

    pub async fn search_by_node(&self, node: &CodeNode, limit: usize) -> Result<Vec<SearchResult>> {
        let query_embedding = if let Some(embedding) = &node.embedding {
            embedding.clone()
        } else {
            self.embedding_generator.generate_embedding(node).await?
        };

        self.search_by_embedding(&query_embedding, limit).await
    }

    pub async fn search_by_embedding(
        &self,
        query_embedding: &[f32],
        limit: usize,
    ) -> Result<Vec<SearchResult>> {
        let node_ids = self.vector_store.search_similar(query_embedding, limit).await?;

        let mut results = Vec::new();
        for (index, node_id) in node_ids.into_iter().enumerate() {
            let score = self.calculate_similarity_score(query_embedding, node_id).await?;
            
            results.push(SearchResult {
                node_id,
                score,
                node: None,
            });
        }

        results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        Ok(results)
    }

    pub async fn find_similar_functions(
        &self,
        function_node: &CodeNode,
        limit: usize,
    ) -> Result<Vec<SearchResult>> {
        if !matches!(function_node.node_type, codegraph_core::NodeType::Function) {
            return Err(CodeGraphError::InvalidOperation(
                "Node must be a function".to_string(),
            ));
        }

        self.search_by_node(function_node, limit).await
    }

    pub async fn find_related_code(
        &self,
        context: &[CodeNode],
        limit: usize,
    ) -> Result<Vec<SearchResult>> {
        if context.is_empty() {
            return Ok(Vec::new());
        }

        let embeddings = self.get_context_embeddings(context).await?;
        let combined_embedding = self.combine_embeddings(&embeddings)?;

        self.search_by_embedding(&combined_embedding, limit).await
    }

    async fn encode_query(&self, query: &str) -> Result<Vec<f32>> {
        tokio::task::spawn_blocking({
            let query = query.to_string();
            move || {
                let dimension = 384;
                let mut embedding = vec![0.0f32; dimension];
                
                let hash = simple_hash(&query);
                let mut rng_state = hash;

                for i in 0..dimension {
                    rng_state = rng_state.wrapping_mul(1103515245).wrapping_add(12345);
                    embedding[i] = ((rng_state as f32 / u32::MAX as f32) - 0.5) * 2.0;
                }

                let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
                if norm > 0.0 {
                    for x in &mut embedding {
                        *x /= norm;
                    }
                }

                embedding
            }
        })
        .await
        .map_err(|e| CodeGraphError::Vector(e.to_string()))
    }

    async fn calculate_similarity_score(
        &self,
        query_embedding: &[f32],
        node_id: NodeId,
    ) -> Result<f32> {
        if let Some(node_embedding) = self.vector_store.get_embedding(node_id).await? {
            Ok(cosine_similarity(query_embedding, &node_embedding))
        } else {
            Ok(0.0)
        }
    }

    async fn get_context_embeddings(&self, nodes: &[CodeNode]) -> Result<Vec<Vec<f32>>> {
        let mut embeddings = Vec::new();
        for node in nodes {
            if let Some(embedding) = &node.embedding {
                embeddings.push(embedding.clone());
            } else {
                let embedding = self.embedding_generator.generate_embedding(node).await?;
                embeddings.push(embedding);
            }
        }
        Ok(embeddings)
    }

    fn combine_embeddings(&self, embeddings: &[Vec<f32>]) -> Result<Vec<f32>> {
        if embeddings.is_empty() {
            return Err(CodeGraphError::Vector("No embeddings to combine".to_string()));
        }

        let dimension = embeddings[0].len();
        let mut combined = vec![0.0f32; dimension];

        for embedding in embeddings {
            if embedding.len() != dimension {
                return Err(CodeGraphError::Vector(
                    "All embeddings must have the same dimension".to_string(),
                ));
            }
            for (i, &value) in embedding.iter().enumerate() {
                combined[i] += value;
            }
        }

        let count = embeddings.len() as f32;
        for value in &mut combined {
            *value /= count;
        }

        let norm: f32 = combined.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 0.0 {
            for x in &mut combined {
                *x /= norm;
            }
        }

        Ok(combined)
    }
}

fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() {
        return 0.0;
    }

    let dot_product: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

    if norm_a == 0.0 || norm_b == 0.0 {
        0.0
    } else {
        dot_product / (norm_a * norm_b)
    }
}

fn simple_hash(text: &str) -> u32 {
    let mut hash = 5381u32;
    for byte in text.bytes() {
        hash = hash.wrapping_mul(33).wrapping_add(byte as u32);
    }
    hash
}