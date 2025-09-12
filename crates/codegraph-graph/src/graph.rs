use crate::{
    CacheManager, CodeEdge, GraphQueryCache, HighPerformanceEdge, HighPerformanceRocksDbStorage,
    QueryOptimizer,
};
use async_trait::async_trait;
use codegraph_core::{CodeNode, GraphStore, NodeId, Result};
use dashmap::DashMap;
use parking_lot::RwLock;
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};
use std::time::{Duration, Instant};

pub struct CodeGraph {
    storage: Arc<HighPerformanceRocksDbStorage>,
    node_cache: Arc<DashMap<NodeId, Arc<CodeNode>>>,
    edge_cache: Arc<DashMap<NodeId, Arc<Vec<HighPerformanceEdge>>>>,
    query_optimizer: Option<QueryOptimizer>,
    query_stats: Arc<RwLock<QueryStats>>,
    path_cache: Arc<DashMap<(NodeId, NodeId), Arc<Option<Vec<NodeId>>>>>,
}

#[derive(Debug, Default)]
struct QueryStats {
    node_queries: AtomicU64,
    edge_queries: AtomicU64,
    cache_hits: AtomicU64,
    cache_misses: AtomicU64,
    avg_query_time_ns: AtomicU64,
}

impl CodeGraph {
    pub fn new() -> Result<Self> {
        let storage = Arc::new(HighPerformanceRocksDbStorage::new("./data/graph.db")?);

        Ok(Self {
            storage,
            node_cache: Arc::new(DashMap::with_capacity(100_000)),
            edge_cache: Arc::new(DashMap::with_capacity(50_000)),
            query_optimizer: None,
            query_stats: Arc::new(RwLock::new(QueryStats::default())),
            path_cache: Arc::new(DashMap::with_capacity(10_000)),
        })
    }

    pub fn new_with_cache() -> Result<Self> {
        let cache = GraphQueryCache::new();
        let cache_manager = CacheManager::new(cache, Duration::from_secs(60));
        let query_optimizer = QueryOptimizer::new(cache_manager);

        let storage = Arc::new(HighPerformanceRocksDbStorage::new("./data/graph.db")?);

        Ok(Self {
            storage,
            node_cache: Arc::new(DashMap::with_capacity(100_000)),
            edge_cache: Arc::new(DashMap::with_capacity(50_000)),
            query_optimizer: Some(query_optimizer),
            query_stats: Arc::new(RwLock::new(QueryStats::default())),
            path_cache: Arc::new(DashMap::with_capacity(10_000)),
        })
    }

    pub fn query_optimizer(&self) -> Option<&QueryOptimizer> {
        self.query_optimizer.as_ref()
    }

    pub(crate) fn cached_node_ids(&self) -> Vec<NodeId> {
        self.node_cache.iter().map(|e| *e.key()).collect()
    }

    fn record_query_time(&self, duration_ns: u64) {
        let stats = self.query_stats.read();
        let current_avg = stats.avg_query_time_ns.load(Ordering::Relaxed);
        let new_avg = if current_avg == 0 {
            duration_ns
        } else {
            (current_avg + duration_ns) / 2
        };
        stats.avg_query_time_ns.store(new_avg, Ordering::Relaxed);
    }

    pub async fn add_edge(&mut self, edge: CodeEdge) -> Result<()> {
        let start = Instant::now();
        let hp_edge = HighPerformanceEdge::from(edge);

        let result = self.storage.add_edge(hp_edge.clone().into()).await;

        if result.is_ok() {
            self.edge_cache.remove(&hp_edge.from);
            self.path_cache.clear();
        }

        self.record_query_time(start.elapsed().as_nanos() as u64);
        result
    }

    pub async fn add_high_performance_edge(&self, edge: HighPerformanceEdge) -> Result<()> {
        let start = Instant::now();

        let result = self.storage.add_edge(edge.clone().into()).await;

        if result.is_ok() {
            self.edge_cache.remove(&edge.from);
            self.path_cache.clear();
        }

        self.record_query_time(start.elapsed().as_nanos() as u64);
        result
    }

    pub async fn batch_add_edges(&self, edges: Vec<HighPerformanceEdge>) -> Result<()> {
        let start = Instant::now();

        for edge in &edges {
            self.storage.add_edge(edge.clone().into()).await?;
        }

        // writes are committed in each batch; explicit flush not required here

        self.edge_cache.clear();
        self.path_cache.clear();

        self.record_query_time(start.elapsed().as_nanos() as u64);
        Ok(())
    }

    pub async fn remove_edge(
        &self,
        from: NodeId,
        to: NodeId,
        edge_type: Option<codegraph_core::EdgeType>,
    ) -> Result<usize> {
        let start = Instant::now();
        let removed = self
            .storage
            .remove_edges(
                from,
                to,
                edge_type.as_ref().map(|e| e.to_string()).as_deref(),
            )
            .await?;

        // Invalidate caches potentially impacted
        self.edge_cache.remove(&from);
        self.path_cache.clear();

        self.record_query_time(start.elapsed().as_nanos() as u64);
        Ok(removed)
    }

    pub async fn get_edges_from(&self, node_id: NodeId) -> Result<Vec<CodeEdge>> {
        let start = Instant::now();

        let hp_edges = self.get_high_performance_edges_from(node_id).await?;
        let edges = hp_edges
            .into_iter()
            .map(|e| CodeEdge {
                id: uuid::Uuid::new_v4(),
                from: e.from,
                to: e.to,
                edge_type: e.edge_type.parse().unwrap_or_default(),
                weight: e.weight,
                metadata: e.metadata,
            })
            .collect();

        self.record_query_time(start.elapsed().as_nanos() as u64);
        Ok(edges)
    }

    /// Get incoming edges for a node (edges where `to == node_id`).
    pub async fn get_edges_to(&self, node_id: NodeId) -> Result<Vec<CodeEdge>> {
        let start = Instant::now();

        let hp_edges = self.storage.get_edges_to(node_id).await?;
        let edges = hp_edges
            .into_iter()
            .map(|e| CodeEdge {
                id: uuid::Uuid::new_v4(),
                from: e.from,
                to: e.to,
                edge_type: e.edge_type.parse().unwrap_or_default(),
                weight: e.weight,
                metadata: e.metadata,
            })
            .collect::<Vec<_>>();

        self.record_query_time(start.elapsed().as_nanos() as u64);
        Ok(edges)
    }

    pub async fn get_high_performance_edges_from(
        &self,
        node_id: NodeId,
    ) -> Result<Vec<HighPerformanceEdge>> {
        let start = Instant::now();

        if let Some(cached) = self.edge_cache.get(&node_id) {
            self.query_stats
                .read()
                .cache_hits
                .fetch_add(1, Ordering::Relaxed);
            self.record_query_time(start.elapsed().as_nanos() as u64);
            return Ok(cached.as_ref().clone());
        }

        self.query_stats
            .read()
            .cache_misses
            .fetch_add(1, Ordering::Relaxed);
        let storage_edges = self.storage.get_edges_from(node_id).await?;
        let edges: Vec<HighPerformanceEdge> = storage_edges.into_iter().map(|e| e.into()).collect();

        let edges_arc = Arc::new(edges.clone());
        self.edge_cache.insert(node_id, edges_arc);

        self.query_stats
            .read()
            .edge_queries
            .fetch_add(1, Ordering::Relaxed);
        self.record_query_time(start.elapsed().as_nanos() as u64);

        Ok(edges)
    }

    pub async fn get_neighbors(&self, node_id: NodeId) -> Result<Vec<NodeId>> {
        let start = Instant::now();

        if let Some(optimizer) = &self.query_optimizer {
            if let Some(cached) = optimizer.cache().get_neighbors(node_id) {
                self.query_stats
                    .read()
                    .cache_hits
                    .fetch_add(1, Ordering::Relaxed);
                self.record_query_time(start.elapsed().as_nanos() as u64);
                return Ok(cached);
            }
        }

        self.query_stats
            .read()
            .cache_misses
            .fetch_add(1, Ordering::Relaxed);
        let edges = self.get_high_performance_edges_from(node_id).await?;
        let neighbors: Vec<NodeId> = edges.into_iter().map(|e| e.to).collect();

        if let Some(optimizer) = &self.query_optimizer {
            optimizer
                .cache()
                .cache_neighbors(node_id, neighbors.clone());
        }

        self.record_query_time(start.elapsed().as_nanos() as u64);
        Ok(neighbors)
    }

    /// Get incoming neighbors (nodes that have edges pointing to `node_id`).
    pub async fn get_incoming_neighbors(&self, node_id: NodeId) -> Result<Vec<NodeId>> {
        let start = Instant::now();
        let edges = self.get_edges_to(node_id).await?;
        let neighbors: Vec<NodeId> = edges.into_iter().map(|e| e.from).collect();
        self.record_query_time(start.elapsed().as_nanos() as u64);
        Ok(neighbors)
    }

    pub async fn shortest_path(&self, from: NodeId, to: NodeId) -> Result<Option<Vec<NodeId>>> {
        let start = Instant::now();

        let cache_key = (from, to);
        if let Some(cached) = self.path_cache.get(&cache_key) {
            self.query_stats
                .read()
                .cache_hits
                .fetch_add(1, Ordering::Relaxed);
            self.record_query_time(start.elapsed().as_nanos() as u64);
            return Ok(cached.as_ref().clone());
        }

        self.query_stats
            .read()
            .cache_misses
            .fetch_add(1, Ordering::Relaxed);

        let path = self.bfs_shortest_path(from, to).await?;

        let path_arc = Arc::new(path.clone());
        self.path_cache.insert(cache_key, path_arc);

        self.record_query_time(start.elapsed().as_nanos() as u64);
        Ok(path)
    }

    async fn bfs_shortest_path(&self, from: NodeId, to: NodeId) -> Result<Option<Vec<NodeId>>> {
        if from == to {
            return Ok(Some(vec![from]));
        }

        let mut queue = VecDeque::new();
        let mut visited = HashSet::with_capacity(1000);
        let mut parent: HashMap<NodeId, NodeId> = HashMap::with_capacity(1000);

        queue.push_back(from);
        visited.insert(from);

        while let Some(current) = queue.pop_front() {
            if current == to {
                let mut path = Vec::new();
                let mut node = to;
                path.push(node);

                while let Some(&prev) = parent.get(&node) {
                    path.push(prev);
                    node = prev;
                }

                path.reverse();
                return Ok(Some(path));
            }

            let neighbors = self.get_neighbors(current).await?;
            for neighbor in neighbors {
                if !visited.contains(&neighbor) {
                    visited.insert(neighbor);
                    parent.insert(neighbor, current);
                    queue.push_back(neighbor);

                    if visited.len() > 100_000 {
                        return Ok(None);
                    }
                }
            }
        }

        Ok(None)
    }

    pub fn get_query_stats(&self) -> QueryStatsSnapshot {
        let stats = self.query_stats.read();
        QueryStatsSnapshot {
            node_queries: stats.node_queries.load(Ordering::Relaxed),
            edge_queries: stats.edge_queries.load(Ordering::Relaxed),
            cache_hits: stats.cache_hits.load(Ordering::Relaxed),
            cache_misses: stats.cache_misses.load(Ordering::Relaxed),
            avg_query_time_ns: stats.avg_query_time_ns.load(Ordering::Relaxed),
            cache_size: self.node_cache.len() + self.edge_cache.len() + self.path_cache.len(),
        }
    }

    pub fn clear_caches(&self) {
        self.node_cache.clear();
        self.edge_cache.clear();
        self.path_cache.clear();
    }
}

#[derive(Debug, Clone)]
pub struct QueryStatsSnapshot {
    pub node_queries: u64,
    pub edge_queries: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub avg_query_time_ns: u64,
    pub cache_size: usize,
}

#[async_trait]
impl GraphStore for CodeGraph {
    async fn add_node(&mut self, node: CodeNode) -> Result<()> {
        let start = Instant::now();
        let id = node.id;
        self.storage.add_node_inner(&node)?;

        let node_arc = Arc::new(node);
        self.node_cache.insert(id, node_arc);

        self.query_stats
            .read()
            .node_queries
            .fetch_add(1, Ordering::Relaxed);
        self.record_query_time(start.elapsed().as_nanos() as u64);

        Ok(())
    }

    async fn get_node(&self, id: NodeId) -> Result<Option<CodeNode>> {
        let start = Instant::now();

        if let Some(cached) = self.node_cache.get(&id) {
            self.query_stats
                .read()
                .cache_hits
                .fetch_add(1, Ordering::Relaxed);
            self.record_query_time(start.elapsed().as_nanos() as u64);
            return Ok(Some(cached.as_ref().clone()));
        }

        self.query_stats
            .read()
            .cache_misses
            .fetch_add(1, Ordering::Relaxed);
        let node = self.storage.get_node(id).await?;

        if let Some(ref n) = node {
            let node_arc = Arc::new(n.clone());
            self.node_cache.insert(id, node_arc);
        }

        self.query_stats
            .read()
            .node_queries
            .fetch_add(1, Ordering::Relaxed);
        self.record_query_time(start.elapsed().as_nanos() as u64);

        Ok(node)
    }

    async fn update_node(&mut self, node: CodeNode) -> Result<()> {
        self.add_node(node).await
    }

    async fn remove_node(&mut self, id: NodeId) -> Result<()> {
        let start = Instant::now();
        self.storage.remove_node_inner(id)?;

        self.node_cache.remove(&id);
        self.edge_cache.remove(&id);
        self.path_cache.clear();

        self.record_query_time(start.elapsed().as_nanos() as u64);

        Ok(())
    }

    async fn find_nodes_by_name(&self, name: &str) -> Result<Vec<CodeNode>> {
        let start = Instant::now();

        let ids = self.storage.scan_node_ids_by_name(name)?;
        let mut nodes = Vec::new();
        for id in ids {
            if let Some(n) = self.storage.get_node(id).await? {
                nodes.push(n);
            }
        }

        for node in &nodes {
            let node_arc = Arc::new(node.clone());
            self.node_cache.insert(node.id, node_arc);
        }

        self.record_query_time(start.elapsed().as_nanos() as u64);

        Ok(nodes)
    }
}
