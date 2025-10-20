#[cfg(feature = "faiss")]
use crate::{CacheConfig, EmbeddingGenerator, FaissVectorStore, QueryHash, SearchCacheManager};
#[cfg(feature = "faiss")]
use codegraph_core::{CodeGraphError, Result, VectorStore};
use codegraph_core::{CodeNode, Language, NodeId, NodeType};
use std::collections::{HashMap, HashSet};
#[cfg(feature = "faiss")]
use std::{sync::Arc, time::Duration};
#[cfg(feature = "faiss")]
use futures::future::try_join_all;

#[derive(Clone)]
pub struct SearchResult {
    pub node_id: NodeId,
    pub score: f32,
    pub node: Option<CodeNode>,
}

#[cfg(feature = "faiss")]
pub struct SemanticSearch {
    vector_store: Arc<FaissVectorStore>,
    embedding_generator: Arc<EmbeddingGenerator>,
    cache: Arc<SearchCacheManager>,
    node_metadata: Arc<dashmap::DashMap<NodeId, CodeNode>>, // optional, for filters/ranking
}

/// Metadata-aware filters for hybrid search
#[derive(Clone, Debug, Default)]
pub struct SearchFilters {
    pub languages: Option<HashSet<Language>>,
    pub node_types: Option<HashSet<NodeType>>,
    pub attribute_equals: HashMap<String, String>,
    pub path_prefixes: Vec<String>,
}

/// Combination mode for multi-vector queries
#[derive(Clone, Copy, Debug)]
pub enum CombineMode {
    OrMax,
    AndAverage,
}

#[cfg(feature = "faiss")]
impl SemanticSearch {
    pub fn new(
        vector_store: Arc<FaissVectorStore>,
        embedding_generator: Arc<EmbeddingGenerator>,
    ) -> Self {
        let cache = Arc::new(SearchCacheManager::new(
            CacheConfig {
                max_entries: 10_000,
                ttl: Duration::from_secs(1800),
                cleanup_interval: Duration::from_secs(60),
                enable_stats: true,
            },
            CacheConfig {
                max_entries: 50_000,
                ttl: Duration::from_secs(3600),
                cleanup_interval: Duration::from_secs(120),
                enable_stats: true,
            },
            CacheConfig {
                max_entries: 5_000,
                ttl: Duration::from_secs(900),
                cleanup_interval: Duration::from_secs(60),
                enable_stats: true,
            },
        ));

        Self {
            vector_store,
            embedding_generator,
            cache,
            node_metadata: Arc::new(dashmap::DashMap::new()),
        }
    }

    /// Optionally seed node metadata for filter/hybrid ranking without external lookups.
    pub fn upsert_node_metadata(&self, node: CodeNode) {
        self.node_metadata.insert(node.id, node);
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
        // Use cache first (basic signature)
        let config_str = format!("basic_limit:{}", limit);
        let qh = QueryHash::new(query_embedding, limit, &config_str);
        if let Some(cached) = self.cache.get_query_results(&qh) {
            let mut results: Vec<SearchResult> = cached
                .into_iter()
                .map(|(node_id, score)| SearchResult {
                    node_id,
                    score,
                    node: None,
                })
                .collect();
            normalize_scores(&mut results);
            return Ok(results);
        }

        // Prefetch more to allow later filters/hybrid steps to prune
        let prefetch_k = (limit.saturating_mul(3)).max(limit + 10);
        let node_ids = self
            .vector_store
            .search_similar(query_embedding, prefetch_k)
            .await?;

        let mut results = Vec::with_capacity(node_ids.len());
        for node_id in node_ids {
            let score = self
                .calculate_similarity_score(query_embedding, node_id)
                .await?;
            results.push(SearchResult {
                node_id,
                score,
                node: None,
            });
        }

        // Sort desc by score, truncate and normalize
        results.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        results.truncate(limit);
        normalize_scores(&mut results);

        // Cache NodeId->score pairs
        let to_cache: Vec<(NodeId, f32)> = results.iter().map(|r| (r.node_id, r.score)).collect();
        self.cache.cache_query_results(qh, to_cache);
        Ok(results)
    }

    pub async fn find_similar_functions(
        &self,
        function_node: &CodeNode,
        limit: usize,
    ) -> Result<Vec<SearchResult>> {
        if !matches!(
            function_node.node_type,
            Some(codegraph_core::NodeType::Function)
        ) {
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
            return Err(CodeGraphError::Vector(
                "No embeddings to combine".to_string(),
            ));
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

#[cfg(feature = "faiss")]
impl SemanticSearch {
    /// Primary semantic search with optional metadata filters and score normalization.
    pub async fn semantic_search(
        &self,
        query_embedding: &[f32],
        filters: Option<&SearchFilters>,
        limit: usize,
    ) -> Result<Vec<SearchResult>> {
        let cfg_sig = build_filter_signature(filters);
        let qh = QueryHash::new(query_embedding, limit, &cfg_sig);
        if let Some(cached) = self.cache.get_query_results(&qh) {
            let mut results: Vec<SearchResult> = cached
                .into_iter()
                .map(|(node_id, score)| SearchResult {
                    node_id,
                    score,
                    node: None,
                })
                .collect();
            normalize_scores(&mut results);
            return Ok(results);
        }

        let prefetch_k = (limit.saturating_mul(4)).max(limit + 25);
        let base = self
            .search_by_embedding(query_embedding, prefetch_k)
            .await?;

        // Apply filters if provided
        let mut filtered = if let Some(f) = filters {
            base.into_iter()
                .filter(|r| self.node_matches_filters(r.node_id, f))
                .collect::<Vec<_>>()
        } else {
            base
        };

        filtered.truncate(limit);
        normalize_scores(&mut filtered);

        // Cache final results
        let to_cache: Vec<(NodeId, f32)> = filtered.iter().map(|r| (r.node_id, r.score)).collect();
        self.cache.cache_query_results(qh, to_cache);
        Ok(filtered)
    }

    /// Hybrid search: combine vector similarity with metadata match score.
    /// `vector_weight` in [0,1]; metadata weight = 1 - vector_weight.
    pub async fn hybrid_search(
        &self,
        query_embedding: &[f32],
        filters: &SearchFilters,
        vector_weight: f32,
        limit: usize,
    ) -> Result<Vec<SearchResult>> {
        let vw = vector_weight.clamp(0.0, 1.0);
        let mw = 1.0 - vw;

        let prefetch_k = (limit.saturating_mul(4)).max(limit + 25);
        let mut candidates = self
            .search_by_embedding(query_embedding, prefetch_k)
            .await?;

        for r in &mut candidates {
            let meta_score = self.metadata_match_score(r.node_id, filters);
            r.score = vw * r.score + mw * meta_score;
        }
        candidates.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        candidates.truncate(limit);
        normalize_scores(&mut candidates);
        Ok(candidates)
    }

    /// Multi-vector query support with OR/AND combination.
    pub async fn multi_vector_search(
        &self,
        queries: &[Vec<f32>],
        mode: CombineMode,
        filters: Option<&SearchFilters>,
        limit: usize,
    ) -> Result<Vec<SearchResult>> {
        if queries.is_empty() {
            return Ok(Vec::new());
        }

        let futs = queries
            .iter()
            .map(|q| self.semantic_search(q, filters, limit));
        let lists: Vec<Vec<SearchResult>> = try_join_all(futs).await?;

        use std::collections::hash_map::Entry;
        let mut agg: HashMap<NodeId, (f32, usize)> = HashMap::new(); // (acc or max, count)
        match mode {
            CombineMode::OrMax => {
                for list in lists {
                    for r in list {
                        agg.entry(r.node_id)
                            .and_modify(|(s, _)| {
                                if r.score > *s {
                                    *s = r.score;
                                }
                            })
                            .or_insert((r.score, 1));
                    }
                }
            }
            CombineMode::AndAverage => {
                for list in lists {
                    for r in list {
                        match agg.entry(r.node_id) {
                            Entry::Occupied(mut e) => {
                                let v = e.get_mut();
                                v.0 += r.score;
                                v.1 += 1;
                            }
                            Entry::Vacant(e) => {
                                e.insert((r.score, 1));
                            }
                        }
                    }
                }
                let qn = queries.len();
                agg.retain(|_, &mut (_, c)| c == qn);
                for (_, v) in agg.iter_mut() {
                    v.0 /= qn as f32;
                }
            }
        }

        let mut combined: Vec<SearchResult> = agg
            .into_iter()
            .map(|(node_id, (score, _))| SearchResult {
                node_id,
                score,
                node: None,
            })
            .collect();
        combined.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        combined.truncate(limit);
        normalize_scores(&mut combined);
        Ok(combined)
    }

    fn node_matches_filters(&self, node_id: NodeId, filters: &SearchFilters) -> bool {
        let node = match self.node_metadata.get(&node_id) {
            Some(n) => n,
            None => return false,
        };

        if let Some(ref langs) = filters.languages {
            if !node
                .language
                .as_ref()
                .map(|l| langs.contains(l))
                .unwrap_or(false)
            {
                return false;
            }
        }
        if let Some(ref types) = filters.node_types {
            if !node
                .node_type
                .as_ref()
                .map(|t| types.contains(t))
                .unwrap_or(false)
            {
                return false;
            }
        }
        for (k, v) in &filters.attribute_equals {
            match node.metadata.attributes.get(k) {
                Some(val) if val == v => {}
                _ => return false,
            }
        }
        if !filters.path_prefixes.is_empty() {
            let p = &node.location.file_path;
            if !filters.path_prefixes.iter().any(|pre| p.starts_with(pre)) {
                return false;
            }
        }
        true
    }

    fn metadata_match_score(&self, node_id: NodeId, filters: &SearchFilters) -> f32 {
        let node = match self.node_metadata.get(&node_id) {
            Some(n) => n,
            None => return 0.0,
        };
        let mut score = 0.0f32;
        let mut denom = 0.0f32;

        if let Some(ref langs) = filters.languages {
            denom += 1.0;
            if node
                .language
                .as_ref()
                .map(|l| langs.contains(l))
                .unwrap_or(false)
            {
                score += 1.0;
            }
        }
        if let Some(ref types) = filters.node_types {
            denom += 1.0;
            if node
                .node_type
                .as_ref()
                .map(|t| types.contains(t))
                .unwrap_or(false)
            {
                score += 1.0;
            }
        }
        if !filters.attribute_equals.is_empty() {
            denom += 1.0;
            let all_match = filters.attribute_equals.iter().all(|(k, v)| {
                node.metadata
                    .attributes
                    .get(k)
                    .map(|val| val == v)
                    .unwrap_or(false)
            });
            if all_match {
                score += 1.0;
            }
        }
        if !filters.path_prefixes.is_empty() {
            denom += 1.0;
            let p = &node.location.file_path;
            if filters.path_prefixes.iter().any(|pre| p.starts_with(pre)) {
                score += 1.0;
            }
        }
        if denom == 0.0 {
            0.0
        } else {
            score / denom
        }
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

fn build_filter_signature(filters: Option<&SearchFilters>) -> String {
    if let Some(f) = filters {
        let mut langs: Vec<String> = f
            .languages
            .as_ref()
            .map(|s| s.iter().map(|l| format!("{:?}", l)).collect())
            .unwrap_or_else(Vec::new);
        langs.sort();
        let mut types: Vec<String> = f
            .node_types
            .as_ref()
            .map(|s| s.iter().map(|t| format!("{:?}", t)).collect())
            .unwrap_or_else(Vec::new);
        types.sort();
        let mut attrs: Vec<(String, String)> = f
            .attribute_equals
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        attrs.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));
        let mut paths = f.path_prefixes.clone();
        paths.sort();
        format!(
            "langs={:?};types={:?};attrs={:?};paths={:?}",
            langs, types, attrs, paths
        )
    } else {
        "nofilters".to_string()
    }
}

fn normalize_scores(results: &mut [SearchResult]) {
    if results.is_empty() {
        return;
    }
    let mut min_s = f32::INFINITY;
    let mut max_s = f32::NEG_INFINITY;
    for r in results.iter() {
        if r.score < min_s {
            min_s = r.score;
        }
        if r.score > max_s {
            max_s = r.score;
        }
    }
    let range = (max_s - min_s).max(1e-12);
    for r in results.iter_mut() {
        r.score = (r.score - min_s) / range;
    }
}
