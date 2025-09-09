use crate::{SearchCacheManager, QueryHash, ContextHash, CacheConfig};
use codegraph_core::{CodeGraphError, CodeNode, Language, NodeId, NodeType, Result};
use dashmap::DashMap;
use faiss::{Index, IndexImpl, MetricType};
use parking_lot::{RwLock, Mutex};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, BinaryHeap};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Semaphore;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchConfig {
    pub k: usize,
    pub precision_recall_tradeoff: f32, // 0.0 = max recall, 1.0 = max precision
    pub enable_clustering: bool,
    pub cluster_threshold: f32,
    pub max_parallel_queries: usize,
    pub context_weight: f32,
    pub language_boost: f32,
    pub type_boost: f32,
}

impl Default for SearchConfig {
    fn default() -> Self {
        Self {
            k: 10,
            precision_recall_tradeoff: 0.5,
            enable_clustering: true,
            cluster_threshold: 0.8,
            max_parallel_queries: 8,
            context_weight: 0.3,
            language_boost: 0.2,
            type_boost: 0.1,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ContextualSearchResult {
    pub node_id: NodeId,
    pub similarity_score: f32,
    pub context_score: f32,
    pub final_score: f32,
    pub node: Option<CodeNode>,
    pub cluster_id: Option<usize>,
    pub explanation: String,
}

#[derive(Debug, Clone)]
struct CodeCluster {
    id: usize,
    centroid: Vec<f32>,
    nodes: HashSet<NodeId>,
    language: Option<Language>,
    node_type: Option<NodeType>,
    avg_similarity: f32,
}

pub struct OptimizedKnnEngine {
    // Core FAISS indices with different configurations
    exact_index: Arc<RwLock<Option<IndexImpl>>>,
    approximate_index: Arc<RwLock<Option<IndexImpl>>>,
    
    // Node mappings and metadata
    id_mapping: Arc<DashMap<i64, NodeId>>,
    reverse_mapping: Arc<DashMap<NodeId, i64>>,
    node_metadata: Arc<DashMap<NodeId, CodeNode>>,
    embeddings: Arc<DashMap<NodeId, Vec<f32>>>,
    
    // Clustering system
    clusters: Arc<RwLock<Vec<CodeCluster>>>,
    node_to_cluster: Arc<DashMap<NodeId, usize>>,
    
    // Performance optimization
    dimension: usize,
    next_id: Arc<Mutex<i64>>,
    query_semaphore: Arc<Semaphore>,
    cache_manager: Arc<SearchCacheManager>,
    
    // Configuration
    config: SearchConfig,
}

impl OptimizedKnnEngine {
    pub fn new(dimension: usize, config: SearchConfig) -> Result<Self> {
        let query_semaphore = Arc::new(Semaphore::new(config.max_parallel_queries));
        
        // Initialize cache manager with optimized configurations
        let cache_manager = Arc::new(SearchCacheManager::new(
            CacheConfig {
                max_entries: 5000,
                ttl: Duration::from_secs(1800), // 30 minutes
                cleanup_interval: Duration::from_secs(60),
                enable_stats: true,
            },
            CacheConfig {
                max_entries: 10000,
                ttl: Duration::from_secs(3600), // 1 hour
                cleanup_interval: Duration::from_secs(120),
                enable_stats: true,
            },
            CacheConfig {
                max_entries: 2000,
                ttl: Duration::from_secs(900), // 15 minutes
                cleanup_interval: Duration::from_secs(30),
                enable_stats: true,
            },
        ));
        
        Ok(Self {
            exact_index: Arc::new(RwLock::new(None)),
            approximate_index: Arc::new(RwLock::new(None)),
            id_mapping: Arc::new(DashMap::new()),
            reverse_mapping: Arc::new(DashMap::new()),
            node_metadata: Arc::new(DashMap::new()),
            embeddings: Arc::new(DashMap::new()),
            clusters: Arc::new(RwLock::new(Vec::new())),
            node_to_cluster: Arc::new(DashMap::new()),
            dimension,
            next_id: Arc::new(Mutex::new(0)),
            query_semaphore,
            cache_manager,
            config,
        })
    }

    pub async fn build_indices(&self, nodes: &[CodeNode]) -> Result<()> {
        let start_time = Instant::now();
        
        // Filter nodes with embeddings
        let embedded_nodes: Vec<_> = nodes
            .iter()
            .filter(|node| node.embedding.is_some())
            .collect();

        if embedded_nodes.is_empty() {
            return Ok(());
        }

        // Build exact index for high precision queries
        self.build_exact_index(&embedded_nodes).await?;
        
        // Build approximate index for fast queries
        self.build_approximate_index(&embedded_nodes).await?;
        
        // Build clustering if enabled
        if self.config.enable_clustering {
            self.build_clusters(&embedded_nodes).await?;
        }
        
        tracing::info!(
            "Built KNN indices for {} nodes in {:?}",
            embedded_nodes.len(),
            start_time.elapsed()
        );
        
        Ok(())
    }

    async fn build_exact_index(&self, nodes: &[&CodeNode]) -> Result<()> {
        let mut exact_index_guard = self.exact_index.write();
        let exact_index = faiss::index_factory(
            self.dimension,
            "Flat",
            MetricType::InnerProduct,
        )
        .map_err(|e| CodeGraphError::Vector(e.to_string()))?;

        let (vectors, mappings) = self.prepare_vectors_and_mappings(nodes)?;
        
        exact_index
            .add(&vectors)
            .map_err(|e| CodeGraphError::Vector(e.to_string()))?;

        self.store_mappings(mappings);
        *exact_index_guard = Some(exact_index);
        
        Ok(())
    }

    async fn build_approximate_index(&self, nodes: &[&CodeNode]) -> Result<()> {
        let mut approx_index_guard = self.approximate_index.write();
        
        // Choose index type based on dataset size and precision/recall tradeoff
        let index_description = self.get_optimal_index_description(nodes.len());
        
        let approx_index = faiss::index_factory(
            self.dimension,
            &index_description,
            MetricType::InnerProduct,
        )
        .map_err(|e| CodeGraphError::Vector(e.to_string()))?;

        let (vectors, _) = self.prepare_vectors_and_mappings(nodes)?;
        
        approx_index
            .add(&vectors)
            .map_err(|e| CodeGraphError::Vector(e.to_string()))?;

        *approx_index_guard = Some(approx_index);
        
        Ok(())
    }

    fn get_optimal_index_description(&self, node_count: usize) -> String {
        let precision_factor = self.config.precision_recall_tradeoff;
        
        match node_count {
            0..=1000 => "Flat".to_string(),
            1001..=10000 => {
                if precision_factor > 0.7 {
                    "IVF100,Flat".to_string()
                } else {
                    "IVF50,Flat".to_string()
                }
            }
            10001..=100000 => {
                if precision_factor > 0.7 {
                    "IVF1000,PQ8".to_string()
                } else {
                    "IVF500,PQ8".to_string()
                }
            }
            _ => {
                if precision_factor > 0.7 {
                    "IVF4000,PQ16".to_string()
                } else {
                    "IVF2000,PQ16".to_string()
                }
            }
        }
    }

    fn prepare_vectors_and_mappings(&self, nodes: &[&CodeNode]) -> Result<(Vec<f32>, Vec<(NodeId, Vec<f32>)>)> {
        let mut vectors = Vec::new();
        let mut mappings = Vec::new();
        
        for node in nodes {
            if let Some(embedding) = &node.embedding {
                vectors.extend_from_slice(embedding);
                mappings.push((node.id, embedding.clone()));
                
                // Store node metadata for context scoring
                self.node_metadata.insert(node.id, (*node).clone());
            }
        }
        
        Ok((vectors, mappings))
    }

    fn store_mappings(&self, mappings: Vec<(NodeId, Vec<f32>)>) {
        let mut next_id_guard = self.next_id.lock();
        
        for (node_id, embedding) in mappings {
            let faiss_id = *next_id_guard;
            *next_id_guard += 1;
            
            self.id_mapping.insert(faiss_id, node_id);
            self.reverse_mapping.insert(node_id, faiss_id);
            self.embeddings.insert(node_id, embedding);
        }
    }

    pub async fn parallel_similarity_search(
        &self,
        queries: Vec<Vec<f32>>,
        config: Option<SearchConfig>,
    ) -> Result<Vec<Vec<ContextualSearchResult>>> {
        let search_config = config.unwrap_or_else(|| self.config.clone());
        let start_time = Instant::now();
        
        // Process queries in parallel with semaphore-controlled concurrency
        let results: Result<Vec<_>> = futures::future::try_join_all(
            queries.into_iter().map(|query| {
                let engine = self;
                let config = search_config.clone();
                async move {
                    let _permit = engine.query_semaphore.acquire().await
                        .map_err(|e| CodeGraphError::Vector(e.to_string()))?;
                    engine.single_similarity_search(query, config).await
                }
            })
        ).await;
        
        let results = results?;
        
        tracing::debug!(
            "Parallel search completed for {} queries in {:?}",
            results.len(),
            start_time.elapsed()
        );
        
        Ok(results)
    }

    pub async fn single_similarity_search(
        &self,
        query_embedding: Vec<f32>,
        config: SearchConfig,
    ) -> Result<Vec<ContextualSearchResult>> {
        if query_embedding.len() != self.dimension {
            return Err(CodeGraphError::Vector(format!(
                "Query embedding dimension {} doesn't match index dimension {}",
                query_embedding.len(),
                self.dimension
            )));
        }

        // Create cache key for the query
        let config_str = format!("{:?}", config);
        let query_hash = QueryHash::new(&query_embedding, config.k, &config_str);

        // Check cache first for performance
        if let Some(cached_results) = self.cache_manager.get_query_results(&query_hash) {
            tracing::debug!("Cache hit for query");
            return self.convert_to_contextual_results(cached_results, &query_embedding, &config).await;
        }

        let start_time = Instant::now();

        // Choose index based on precision/recall tradeoff
        let use_exact = config.precision_recall_tradeoff > 0.8;
        let raw_results = if use_exact {
            self.search_exact_index(&query_embedding, config.k * 2).await?
        } else {
            self.search_approximate_index(&query_embedding, config.k * 2).await?
        };

        // Cache the raw results for future use
        self.cache_manager.cache_query_results(query_hash, raw_results.clone());

        // Apply contextual ranking and filtering
        let mut contextual_results = self.apply_contextual_ranking(
            &query_embedding,
            raw_results,
            &config,
        ).await?;

        // Apply clustering-based filtering if enabled
        if config.enable_clustering {
            contextual_results = self.apply_clustering_filter(
                contextual_results,
                &config,
            ).await?;
        }

        // Sort by final score and limit results
        contextual_results.sort_by(|a, b| {
            b.final_score.partial_cmp(&a.final_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        
        contextual_results.truncate(config.k);
        
        tracing::debug!(
            "Single similarity search completed in {:?}",
            start_time.elapsed()
        );
        
        Ok(contextual_results)
    }

    async fn convert_to_contextual_results(
        &self,
        raw_results: Vec<(NodeId, f32)>,
        query_embedding: &[f32],
        config: &SearchConfig,
    ) -> Result<Vec<ContextualSearchResult>> {
        let contextual_results = self.apply_contextual_ranking(
            query_embedding,
            raw_results,
            config,
        ).await?;

        Ok(contextual_results)
    }

    async fn search_exact_index(&self, query: &[f32], k: usize) -> Result<Vec<(NodeId, f32)>> {
        let index_guard = self.exact_index.read();
        let index = index_guard
            .as_ref()
            .ok_or_else(|| CodeGraphError::Vector("Exact index not initialized".to_string()))?;

        self.perform_faiss_search(index, query, k).await
    }

    async fn search_approximate_index(&self, query: &[f32], k: usize) -> Result<Vec<(NodeId, f32)>> {
        let index_guard = self.approximate_index.read();
        let index = index_guard
            .as_ref()
            .ok_or_else(|| CodeGraphError::Vector("Approximate index not initialized".to_string()))?;

        self.perform_faiss_search(index, query, k).await
    }

    async fn perform_faiss_search(
        &self,
        index: &IndexImpl,
        query: &[f32],
        k: usize,
    ) -> Result<Vec<(NodeId, f32)>> {
        let results = tokio::task::spawn_blocking({
            let query = query.to_vec();
            let index_ptr = index as *const IndexImpl;
            
            move || unsafe {
                let index_ref = &*index_ptr;
                index_ref.search(&query, k)
            }
        })
        .await
        .map_err(|e| CodeGraphError::Vector(e.to_string()))?
        .map_err(|e| CodeGraphError::Vector(e.to_string()))?;

        let mut node_results = Vec::new();
        for (i, &faiss_id) in results.labels.iter().enumerate() {
            if let Some(node_id) = self.id_mapping.get(&faiss_id) {
                let score = results.distances[i];
                node_results.push((*node_id, score));
            }
        }

        Ok(node_results)
    }

    async fn apply_contextual_ranking(
        &self,
        query_embedding: &[f32],
        raw_results: Vec<(NodeId, f32)>,
        config: &SearchConfig,
    ) -> Result<Vec<ContextualSearchResult>> {
        let query_context = self.extract_query_context(query_embedding).await?;
        
        let contextual_results: Vec<_> = raw_results
            .par_iter()
            .filter_map(|&(node_id, similarity_score)| {
                let node = self.node_metadata.get(&node_id)?;
                let context_score = self.calculate_context_score(&node, &query_context, config);
                
                let final_score = similarity_score * (1.0 - config.context_weight) 
                    + context_score * config.context_weight;

                let explanation = format!(
                    "Similarity: {:.3}, Context: {:.3}, Language boost: {}, Type boost: {}",
                    similarity_score,
                    context_score,
                    node.language.as_ref().map_or("none", |_| "applied"),
                    node.node_type.as_ref().map_or("none", |_| "applied")
                );

                Some(ContextualSearchResult {
                    node_id,
                    similarity_score,
                    context_score,
                    final_score,
                    node: Some(node.clone()),
                    cluster_id: self.node_to_cluster.get(&node_id).map(|v| *v),
                    explanation,
                })
            })
            .collect();

        Ok(contextual_results)
    }

    async fn extract_query_context(&self, _query_embedding: &[f32]) -> Result<QueryContext> {
        // In a real implementation, this would analyze the query embedding
        // to infer likely language, node type, etc.
        Ok(QueryContext {
            inferred_language: None,
            inferred_node_type: None,
            complexity_score: 0.5,
        })
    }

    fn calculate_context_score(
        &self,
        node: &CodeNode,
        query_context: &QueryContext,
        config: &SearchConfig,
    ) -> f32 {
        let mut score = 1.0;

        // Language boost
        if let (Some(node_lang), Some(query_lang)) = (&node.language, &query_context.inferred_language) {
            if node_lang == query_lang {
                score += config.language_boost;
            }
        }

        // Node type boost
        if let (Some(node_type), Some(query_type)) = (&node.node_type, &query_context.inferred_node_type) {
            if node_type == query_type {
                score += config.type_boost;
            }
        }

        // Complexity matching
        let complexity_diff = (node.complexity.unwrap_or(0.5) - query_context.complexity_score).abs();
        score *= 1.0 - complexity_diff * 0.2;

        score.min(2.0).max(0.0)
    }

    async fn apply_clustering_filter(
        &self,
        mut results: Vec<ContextualSearchResult>,
        config: &SearchConfig,
    ) -> Result<Vec<ContextualSearchResult>> {
        if results.is_empty() {
            return Ok(results);
        }

        // Group results by clusters
        let mut cluster_groups: HashMap<Option<usize>, Vec<ContextualSearchResult>> = HashMap::new();
        
        for result in results {
            let cluster_id = result.cluster_id;
            cluster_groups.entry(cluster_id).or_default().push(result);
        }

        // Select best representatives from each cluster
        let mut filtered_results = Vec::new();
        
        for (cluster_id, mut cluster_results) in cluster_groups {
            cluster_results.sort_by(|a, b| {
                b.final_score.partial_cmp(&a.final_score)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });

            // Take top results from each cluster
            let take_count = if cluster_id.is_some() {
                (config.k / self.clusters.read().len().max(1)).max(1)
            } else {
                cluster_results.len()
            };

            filtered_results.extend(cluster_results.into_iter().take(take_count));
        }

        Ok(filtered_results)
    }

    async fn build_clusters(&self, nodes: &[&CodeNode]) -> Result<()> {
        let embeddings: Vec<_> = nodes
            .iter()
            .filter_map(|node| {
                node.embedding.as_ref().map(|emb| (node.id, emb.clone(), node))
            })
            .collect();

        if embeddings.len() < 2 {
            return Ok(());
        }

        let clusters = self.perform_clustering(&embeddings).await?;
        
        // Update cluster mappings
        let mut clusters_guard = self.clusters.write();
        *clusters_guard = clusters;
        
        for (i, cluster) in clusters_guard.iter().enumerate() {
            for &node_id in &cluster.nodes {
                self.node_to_cluster.insert(node_id, i);
            }
        }

        tracing::info!("Built {} clusters", clusters_guard.len());
        Ok(())
    }

    async fn perform_clustering(
        &self,
        embeddings: &[(NodeId, Vec<f32>, &CodeNode)],
    ) -> Result<Vec<CodeCluster>> {
        let k = (embeddings.len() as f32).sqrt() as usize;
        let k = k.max(2).min(embeddings.len() / 2);

        // Simple k-means clustering implementation
        let mut clusters = Vec::new();
        let mut assignments = vec![0usize; embeddings.len()];
        
        // Initialize centroids randomly
        for i in 0..k {
            let idx = i * embeddings.len() / k;
            clusters.push(CodeCluster {
                id: i,
                centroid: embeddings[idx].1.clone(),
                nodes: HashSet::new(),
                language: None,
                node_type: None,
                avg_similarity: 0.0,
            });
        }

        // Iterate until convergence
        for _iteration in 0..20 {
            // Assign points to nearest centroids
            for (point_idx, (node_id, embedding, _)) in embeddings.iter().enumerate() {
                let mut best_cluster = 0;
                let mut best_distance = f32::MAX;
                
                for (cluster_idx, cluster) in clusters.iter().enumerate() {
                    let distance = self.euclidean_distance(embedding, &cluster.centroid);
                    if distance < best_distance {
                        best_distance = distance;
                        best_cluster = cluster_idx;
                    }
                }
                
                assignments[point_idx] = best_cluster;
            }

            // Update centroids
            for cluster in &mut clusters {
                cluster.nodes.clear();
            }

            let mut cluster_sums: Vec<Vec<f32>> = clusters.iter()
                .map(|c| vec![0.0; self.dimension])
                .collect();
            let mut cluster_counts = vec![0; clusters.len()];

            for (point_idx, (node_id, embedding, node)) in embeddings.iter().enumerate() {
                let cluster_idx = assignments[point_idx];
                clusters[cluster_idx].nodes.insert(*node_id);
                
                for (i, &val) in embedding.iter().enumerate() {
                    cluster_sums[cluster_idx][i] += val;
                }
                cluster_counts[cluster_idx] += 1;

                // Update cluster metadata
                if clusters[cluster_idx].language.is_none() {
                    clusters[cluster_idx].language = node.language.clone();
                }
                if clusters[cluster_idx].node_type.is_none() {
                    clusters[cluster_idx].node_type = node.node_type.clone();
                }
            }

            // Update centroids
            for (cluster_idx, cluster) in clusters.iter_mut().enumerate() {
                if cluster_counts[cluster_idx] > 0 {
                    for (i, sum) in cluster_sums[cluster_idx].iter().enumerate() {
                        cluster.centroid[i] = sum / cluster_counts[cluster_idx] as f32;
                    }
                }
            }
        }

        Ok(clusters)
    }

    fn euclidean_distance(&self, a: &[f32], b: &[f32]) -> f32 {
        a.iter()
            .zip(b.iter())
            .map(|(x, y)| (x - y).powi(2))
            .sum::<f32>()
            .sqrt()
    }

    pub async fn get_cluster_info(&self) -> Vec<ClusterInfo> {
        let clusters_guard = self.clusters.read();
        clusters_guard
            .iter()
            .map(|cluster| ClusterInfo {
                id: cluster.id,
                size: cluster.nodes.len(),
                language: cluster.language.clone(),
                node_type: cluster.node_type.clone(),
                avg_similarity: cluster.avg_similarity,
            })
            .collect()
    }

    pub fn get_performance_stats(&self) -> PerformanceStats {
        let cache_stats = self.cache_manager.get_cache_stats();
        
        PerformanceStats {
            total_nodes: self.embeddings.len(),
            total_clusters: self.clusters.read().len(),
            index_type: if self.config.precision_recall_tradeoff > 0.8 {
                "Exact".to_string()
            } else {
                "Approximate".to_string()
            },
            max_parallel_queries: self.config.max_parallel_queries,
            cache_hit_rate: cache_stats.get("query_cache")
                .map(|stats| stats.hit_ratio)
                .unwrap_or(0.0),
            avg_search_latency_ms: 0.0, // Would be updated by metrics collection
        }
    }

    pub fn clear_caches(&self) {
        self.cache_manager.clear_all();
    }

    pub async fn warmup_cache(&self, sample_nodes: &[NodeId]) -> Result<()> {
        tracing::info!("Starting cache warmup with {} sample nodes", sample_nodes.len());
        
        let warmup_config = SearchConfig {
            k: 5,
            precision_recall_tradeoff: 0.5,
            enable_clustering: false,
            ..self.config.clone()
        };

        for &node_id in sample_nodes {
            if let Some(embedding) = self.embeddings.get(&node_id) {
                let _ = self.single_similarity_search(embedding.clone(), warmup_config.clone()).await;
            }
        }

        tracing::info!("Cache warmup completed");
        Ok(())
    }

    pub async fn optimize_index_configuration(&mut self) -> Result<()> {
        let node_count = self.embeddings.len();
        let avg_query_latency = self.measure_average_query_latency().await?;
        
        // Adjust precision/recall tradeoff based on performance
        if avg_query_latency > Duration::from_millis(100) && node_count > 10000 {
            tracing::info!("High latency detected, switching to more approximate index");
            self.config.precision_recall_tradeoff *= 0.8;
        } else if avg_query_latency < Duration::from_millis(20) && node_count < 1000 {
            tracing::info!("Low latency detected, switching to more exact index");
            self.config.precision_recall_tradeoff = (self.config.precision_recall_tradeoff * 1.2).min(1.0);
        }

        Ok(())
    }

    async fn measure_average_query_latency(&self) -> Result<Duration> {
        let sample_size = 10.min(self.embeddings.len());
        let mut total_latency = Duration::default();
        
        let sample_embeddings: Vec<_> = self.embeddings
            .iter()
            .take(sample_size)
            .map(|entry| entry.value().clone())
            .collect();

        for embedding in sample_embeddings {
            let start = Instant::now();
            let _ = self.single_similarity_search(embedding, self.config.clone()).await?;
            total_latency += start.elapsed();
        }

        Ok(total_latency / sample_size as u32)
    }
}

#[derive(Debug, Clone)]
struct QueryContext {
    inferred_language: Option<Language>,
    inferred_node_type: Option<NodeType>,
    complexity_score: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterInfo {
    pub id: usize,
    pub size: usize,
    pub language: Option<Language>,
    pub node_type: Option<NodeType>,
    pub avg_similarity: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceStats {
    pub total_nodes: usize,
    pub total_clusters: usize,
    pub index_type: String,
    pub max_parallel_queries: usize,
    pub cache_hit_rate: f64,
    pub avg_search_latency_ms: f64,
}

// Batch search operations for high-throughput scenarios
impl OptimizedKnnEngine {
    pub async fn batch_search_similar_functions(
        &self,
        function_nodes: &[CodeNode],
        config: Option<SearchConfig>,
    ) -> Result<Vec<Vec<ContextualSearchResult>>> {
        let search_config = config.unwrap_or_else(|| self.config.clone());
        
        let queries: Result<Vec<_>> = function_nodes
            .iter()
            .filter(|node| matches!(node.node_type, Some(NodeType::Function)))
            .map(|node| {
                node.embedding
                    .clone()
                    .ok_or_else(|| CodeGraphError::Vector("Node missing embedding".to_string()))
            })
            .collect();

        let queries = queries?;
        self.parallel_similarity_search(queries, Some(search_config)).await
    }

    pub async fn discover_related_code_clusters(
        &self,
        seed_nodes: &[NodeId],
        expansion_factor: usize,
    ) -> Result<HashMap<usize, Vec<ContextualSearchResult>>> {
        let mut cluster_results: HashMap<usize, Vec<ContextualSearchResult>> = HashMap::new();
        
        for &seed_node in seed_nodes {
            if let Some(embedding) = self.embeddings.get(&seed_node) {
                let results = self.single_similarity_search(
                    embedding.clone(),
                    SearchConfig {
                        k: expansion_factor,
                        enable_clustering: true,
                        ..self.config.clone()
                    }
                ).await?;

                for result in results {
                    if let Some(cluster_id) = result.cluster_id {
                        cluster_results.entry(cluster_id).or_default().push(result);
                    }
                }
            }
        }

        Ok(cluster_results)
    }
}