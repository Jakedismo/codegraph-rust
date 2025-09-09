use crate::storage::PersistentStorage;
use codegraph_core::{CodeGraphError, NodeId, Result};
use faiss::{Index, index::IndexImpl, MetricType};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;
use tracing::{debug, info, warn};

/// Simplified FAISS index configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimpleIndexConfig {
    pub dimension: usize,
    pub index_type: String,
    pub metric_type: MetricType,
    pub training_threshold: usize,
}

impl Default for SimpleIndexConfig {
    fn default() -> Self {
        Self {
            dimension: 768,
            index_type: "HNSW32".to_string(),
            metric_type: MetricType::InnerProduct,
            training_threshold: 10000,
        }
    }
}

/// Simplified FAISS vector manager
pub struct SimpleFaissManager {
    config: SimpleIndexConfig,
    index: Arc<RwLock<Option<IndexImpl>>>,
    id_mapping: Arc<RwLock<HashMap<i64, NodeId>>>,
    reverse_mapping: Arc<RwLock<HashMap<NodeId, i64>>>,
    next_id: Arc<RwLock<i64>>,
}

impl SimpleFaissManager {
    pub fn new(config: SimpleIndexConfig) -> Self {
        Self {
            config,
            index: Arc::new(RwLock::new(None)),
            id_mapping: Arc::new(RwLock::new(HashMap::new())),
            reverse_mapping: Arc::new(RwLock::new(HashMap::new())),
            next_id: Arc::new(RwLock::new(0)),
        }
    }

    /// Initialize the FAISS index
    pub fn create_index(&self) -> Result<()> {
        let mut index_guard = self.index.write();
        if index_guard.is_some() {
            return Ok(());
        }

        let index = faiss::index_factory(
            self.config.dimension as u32,
            &self.config.index_type,
            self.config.metric_type,
        )
        .map_err(|e| CodeGraphError::Vector(format!("Failed to create index: {}", e)))?;

        *index_guard = Some(index);
        info!("Created FAISS index: {}", self.config.index_type);
        Ok(())
    }

    /// Add vectors to the index
    pub fn add_vectors(&self, vectors: Vec<(NodeId, Vec<f32>)>) -> Result<()> {
        self.create_index()?;

        if vectors.is_empty() {
            return Ok(());
        }

        // Prepare flat vector array for FAISS
        let flat_vectors: Vec<f32> = vectors
            .iter()
            .flat_map(|(_, embedding)| embedding.iter().cloned())
            .collect();

        // Add to FAISS index
        let mut index_guard = self.index.write();
        let index = index_guard.as_mut().unwrap();
        
        index
            .add(&flat_vectors)
            .map_err(|e| CodeGraphError::Vector(format!("Failed to add vectors: {}", e)))?;

        // Update ID mappings
        let mut id_mapping = self.id_mapping.write();
        let mut reverse_mapping = self.reverse_mapping.write();
        let mut next_id = self.next_id.write();

        for (node_id, _) in vectors {
            let faiss_id = *next_id;
            *next_id += 1;

            id_mapping.insert(faiss_id, node_id);
            reverse_mapping.insert(node_id, faiss_id);
        }

        info!("Added {} vectors to FAISS index", vectors.len());
        Ok(())
    }

    /// Perform k-nearest neighbor search
    pub fn search(&self, query: &[f32], k: usize) -> Result<Vec<(NodeId, f32)>> {
        let start = Instant::now();

        if query.len() != self.config.dimension {
            return Err(CodeGraphError::Vector(format!(
                "Query dimension {} doesn't match index dimension {}",
                query.len(),
                self.config.dimension
            )));
        }

        let index_guard = self.index.read();
        let index = index_guard
            .as_ref()
            .ok_or_else(|| CodeGraphError::Vector("Index not initialized".to_string()))?;

        let search_result = index
            .search(query, k)
            .map_err(|e| CodeGraphError::Vector(format!("Search failed: {}", e)))?;

        // Convert FAISS results to NodeId results
        let id_mapping = self.id_mapping.read();
        let mut results = Vec::new();

        for (distance, label) in search_result.distances.into_iter().zip(search_result.labels) {
            if let Some(node_id) = id_mapping.get(&label) {
                results.push((*node_id, distance));
            }
        }

        let duration = start.elapsed();
        debug!("Search completed in {:?}", duration);

        Ok(results)
    }

    /// Get index statistics
    pub fn get_stats(&self) -> IndexStats {
        let index_guard = self.index.read();
        let num_vectors = index_guard
            .as_ref()
            .map(|idx| idx.ntotal() as usize)
            .unwrap_or(0);

        IndexStats {
            num_vectors,
            dimension: self.config.dimension,
            index_type: self.config.index_type.clone(),
            memory_usage_bytes: num_vectors * self.config.dimension * 4, // Rough estimate
        }
    }
}

#[derive(Debug)]
pub struct IndexStats {
    pub num_vectors: usize,
    pub dimension: usize,
    pub index_type: String,
    pub memory_usage_bytes: usize,
}

/// Simple performance metrics
#[derive(Debug, Clone)]
pub struct SearchMetrics {
    pub total_searches: u64,
    pub average_latency_ms: f64,
    pub sub_ms_searches: u64,
}

impl Default for SearchMetrics {
    fn default() -> Self {
        Self {
            total_searches: 0,
            average_latency_ms: 0.0,
            sub_ms_searches: 0,
        }
    }
}