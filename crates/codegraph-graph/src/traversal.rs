#![allow(dead_code, unused_variables, unused_imports)]

use crate::CodeGraph;
use codegraph_core::{NodeId, Result};
use futures::stream::{Stream, StreamExt};
use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap, HashSet, VecDeque};
use std::fmt;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

/// Configuration for traversal algorithms
#[derive(Clone)]
pub struct TraversalConfig {
    /// Maximum depth to traverse (None for unlimited)
    pub max_depth: Option<usize>,
    /// Maximum number of nodes to visit
    pub max_nodes: Option<usize>,
    /// Whether to include the starting node in results
    pub include_start: bool,
    /// Optional filter predicate for nodes
    pub filter: Option<Arc<dyn Fn(NodeId) -> bool + Send + Sync>>,
}

impl fmt::Debug for TraversalConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TraversalConfig")
            .field("max_depth", &self.max_depth)
            .field("max_nodes", &self.max_nodes)
            .field("include_start", &self.include_start)
            .field("filter", &self.filter.as_ref().map(|_| "Some(fn)"))
            .finish()
    }
}

impl Default for TraversalConfig {
    fn default() -> Self {
        Self {
            max_depth: None,
            max_nodes: None,
            include_start: true,
            filter: None,
        }
    }
}

/// Async iterator for breadth-first search
pub struct BfsIterator<'a> {
    graph: &'a CodeGraph,
    queue: VecDeque<(NodeId, usize)>, // (node_id, depth)
    visited: HashSet<NodeId>,
    config: TraversalConfig,
    nodes_visited: usize,
}

impl<'a> BfsIterator<'a> {
    pub fn new(graph: &'a CodeGraph, start: NodeId, config: TraversalConfig) -> Self {
        let mut queue = VecDeque::new();
        let mut visited = HashSet::new();

        if config.include_start {
            queue.push_back((start, 0));
        } else {
            visited.insert(start);
        }

        Self {
            graph,
            queue,
            visited,
            config,
            nodes_visited: 0,
        }
    }
}

impl<'a> Stream for BfsIterator<'a> {
    type Item = Result<NodeId>;

    fn poll_next(mut self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.as_mut().get_mut();

        // Check limits
        if let Some(max_nodes) = this.config.max_nodes {
            if this.nodes_visited >= max_nodes {
                return Poll::Ready(None);
            }
        }

        while let Some((current, depth)) = this.queue.pop_front() {
            // Check depth limit
            if let Some(max_depth) = this.config.max_depth {
                if depth > max_depth {
                    continue;
                }
            }

            if this.visited.contains(&current) {
                continue;
            }

            this.visited.insert(current);
            this.nodes_visited += 1;

            // Apply filter
            if let Some(ref filter) = this.config.filter {
                if !filter(current) {
                    continue;
                }
            }

            // Schedule neighbors for next iteration
            let graph = this.graph;
            let neighbors_future = async move { graph.get_neighbors(current).await };

            // This is a simplified approach - in a real implementation,
            // you'd want to use a proper async stream with futures::Stream
            match futures::executor::block_on(neighbors_future) {
                Ok(neighbors) => {
                    for neighbor in neighbors {
                        if !this.visited.contains(&neighbor) {
                            this.queue.push_back((neighbor, depth + 1));
                        }
                    }
                }
                Err(e) => return Poll::Ready(Some(Err(e))),
            }

            return Poll::Ready(Some(Ok(current)));
        }

        Poll::Ready(None)
    }
}

/// Async iterator for depth-first search
pub struct DfsIterator<'a> {
    graph: &'a CodeGraph,
    stack: Vec<(NodeId, usize)>, // (node_id, depth)
    visited: HashSet<NodeId>,
    config: TraversalConfig,
    nodes_visited: usize,
}

impl<'a> DfsIterator<'a> {
    pub fn new(graph: &'a CodeGraph, start: NodeId, config: TraversalConfig) -> Self {
        let mut stack = Vec::new();
        let mut visited = HashSet::new();

        if config.include_start {
            stack.push((start, 0));
        } else {
            visited.insert(start);
        }

        Self {
            graph,
            stack,
            visited,
            config,
            nodes_visited: 0,
        }
    }
}

impl<'a> Stream for DfsIterator<'a> {
    type Item = Result<NodeId>;

    fn poll_next(mut self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.as_mut().get_mut();

        // Check limits
        if let Some(max_nodes) = this.config.max_nodes {
            if this.nodes_visited >= max_nodes {
                return Poll::Ready(None);
            }
        }

        while let Some((current, depth)) = this.stack.pop() {
            // Check depth limit
            if let Some(max_depth) = this.config.max_depth {
                if depth > max_depth {
                    continue;
                }
            }

            if this.visited.contains(&current) {
                continue;
            }

            this.visited.insert(current);
            this.nodes_visited += 1;

            // Apply filter
            if let Some(ref filter) = this.config.filter {
                if !filter(current) {
                    continue;
                }
            }

            // Schedule neighbors for next iteration (in reverse order for DFS)
            let graph = this.graph;
            match futures::executor::block_on(async move { graph.get_neighbors(current).await }) {
                Ok(neighbors) => {
                    for neighbor in neighbors.into_iter().rev() {
                        if !this.visited.contains(&neighbor) {
                            this.stack.push((neighbor, depth + 1));
                        }
                    }
                }
                Err(e) => return Poll::Ready(Some(Err(e))),
            }

            return Poll::Ready(Some(Ok(current)));
        }

        Poll::Ready(None)
    }
}

/// Node with priority for Dijkstra's algorithm
#[derive(Debug, Clone)]
struct DijkstraNode {
    id: NodeId,
    distance: f64,
    path: Vec<NodeId>,
}

impl PartialEq for DijkstraNode {
    fn eq(&self, other: &Self) -> bool {
        self.distance.partial_cmp(&other.distance) == Some(Ordering::Equal)
    }
}

impl Eq for DijkstraNode {}

impl PartialOrd for DijkstraNode {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        // Reverse ordering for min-heap
        other.distance.partial_cmp(&self.distance)
    }
}

impl Ord for DijkstraNode {
    fn cmp(&self, other: &Self) -> Ordering {
        self.partial_cmp(other).unwrap_or(Ordering::Equal)
    }
}

/// A* node with heuristic
#[derive(Debug, Clone)]
struct AStarNode {
    id: NodeId,
    g_score: f64, // Cost from start
    f_score: f64, // g_score + heuristic
    path: Vec<NodeId>,
}

impl PartialEq for AStarNode {
    fn eq(&self, other: &Self) -> bool {
        self.f_score.partial_cmp(&other.f_score) == Some(Ordering::Equal)
    }
}

impl Eq for AStarNode {}

impl PartialOrd for AStarNode {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        // Reverse ordering for min-heap
        other.f_score.partial_cmp(&self.f_score)
    }
}

impl Ord for AStarNode {
    fn cmp(&self, other: &Self) -> Ordering {
        self.partial_cmp(other).unwrap_or(Ordering::Equal)
    }
}

/// Shortest path algorithms implementation
impl CodeGraph {
    /// Breadth-first search iterator
    pub fn bfs_iter(&self, start: NodeId) -> BfsIterator<'_> {
        BfsIterator::new(self, start, TraversalConfig::default())
    }

    /// Breadth-first search iterator with configuration
    pub fn bfs_iter_with_config(&self, start: NodeId, config: TraversalConfig) -> BfsIterator<'_> {
        BfsIterator::new(self, start, config)
    }

    /// Depth-first search iterator
    pub fn dfs_iter(&self, start: NodeId) -> DfsIterator<'_> {
        DfsIterator::new(self, start, TraversalConfig::default())
    }

    /// Depth-first search iterator with configuration
    pub fn dfs_iter_with_config(&self, start: NodeId, config: TraversalConfig) -> DfsIterator<'_> {
        DfsIterator::new(self, start, config)
    }

    /// Dijkstra's algorithm for shortest path with weights
    pub async fn dijkstra_shortest_path(
        &self,
        start: NodeId,
        target: NodeId,
    ) -> Result<Option<Vec<NodeId>>> {
        let mut distances: HashMap<NodeId, f64> = HashMap::new();
        let mut heap = BinaryHeap::new();

        distances.insert(start, 0.0);
        heap.push(DijkstraNode {
            id: start,
            distance: 0.0,
            path: vec![start],
        });

        while let Some(DijkstraNode {
            id: current,
            distance,
            path,
        }) = heap.pop()
        {
            // If we've reached the target
            if current == target {
                return Ok(Some(path));
            }

            // Skip if we've found a better path
            if let Some(&best_distance) = distances.get(&current) {
                if distance > best_distance {
                    continue;
                }
            }

            // Explore neighbors
            let edges = self.get_edges_from(current).await?;
            for edge in edges {
                let neighbor = edge.to;
                let new_distance = distance + edge.weight;

                let is_better = distances
                    .get(&neighbor)
                    .map_or(true, |&dist| new_distance < dist);

                if is_better {
                    distances.insert(neighbor, new_distance);
                    let mut new_path = path.clone();
                    new_path.push(neighbor);

                    heap.push(DijkstraNode {
                        id: neighbor,
                        distance: new_distance,
                        path: new_path,
                    });
                }
            }
        }

        Ok(None)
    }

    /// A* algorithm with heuristic function
    pub async fn astar_shortest_path<H>(
        &self,
        start: NodeId,
        target: NodeId,
        heuristic: H,
    ) -> Result<Option<Vec<NodeId>>>
    where
        H: Fn(NodeId, NodeId) -> f64 + Send + Sync,
    {
        let mut g_scores: HashMap<NodeId, f64> = HashMap::new();
        let mut f_scores: HashMap<NodeId, f64> = HashMap::new();
        let mut heap = BinaryHeap::new();

        let h_start = heuristic(start, target);
        g_scores.insert(start, 0.0);
        f_scores.insert(start, h_start);

        heap.push(AStarNode {
            id: start,
            g_score: 0.0,
            f_score: h_start,
            path: vec![start],
        });

        while let Some(AStarNode {
            id: current,
            g_score,
            path,
            ..
        }) = heap.pop()
        {
            // If we've reached the target
            if current == target {
                return Ok(Some(path));
            }

            // Skip if we've found a better path
            if let Some(&best_g_score) = g_scores.get(&current) {
                if g_score > best_g_score {
                    continue;
                }
            }

            // Explore neighbors
            let edges = self.get_edges_from(current).await?;
            for edge in edges {
                let neighbor = edge.to;
                let tentative_g_score = g_score + edge.weight;

                let is_better = g_scores
                    .get(&neighbor)
                    .map_or(true, |&score| tentative_g_score < score);

                if is_better {
                    let h_neighbor = heuristic(neighbor, target);
                    let f_neighbor = tentative_g_score + h_neighbor;

                    g_scores.insert(neighbor, tentative_g_score);
                    f_scores.insert(neighbor, f_neighbor);

                    let mut new_path = path.clone();
                    new_path.push(neighbor);

                    heap.push(AStarNode {
                        id: neighbor,
                        g_score: tentative_g_score,
                        f_score: f_neighbor,
                        path: new_path,
                    });
                }
            }
        }

        Ok(None)
    }

    /// Detect cycles using DFS (for import loop detection)
    pub async fn detect_cycles(&self) -> Result<Vec<Vec<NodeId>>> {
        let mut visited = HashSet::new();
        let mut rec_stack = HashSet::new();
        let mut cycles = Vec::new();

        // Get all nodes (this is a simplified approach - in practice you'd want to iterate more efficiently)
        let all_nodes = self.get_all_node_ids().await?;

        for node in all_nodes {
            if !visited.contains(&node) {
                if let Some(cycle) = self
                    .dfs_detect_cycle(node, &mut visited, &mut rec_stack, &mut Vec::new())
                    .await?
                {
                    cycles.push(cycle);
                }
            }
        }

        Ok(cycles)
    }

    /// DFS helper for cycle detection
    async fn dfs_detect_cycle(
        &self,
        node: NodeId,
        visited: &mut HashSet<NodeId>,
        rec_stack: &mut HashSet<NodeId>,
        path: &mut Vec<NodeId>,
    ) -> Result<Option<Vec<NodeId>>> {
        visited.insert(node);
        rec_stack.insert(node);
        path.push(node);

        let neighbors = self.get_neighbors(node).await?;
        for neighbor in neighbors {
            if !visited.contains(&neighbor) {
                if let Some(cycle) =
                    Box::pin(self.dfs_detect_cycle(neighbor, visited, rec_stack, path)).await?
                {
                    return Ok(Some(cycle));
                }
            } else if rec_stack.contains(&neighbor) {
                // Found a back edge - cycle detected
                let cycle_start = path.iter().position(|&n| n == neighbor).unwrap();
                let cycle = path[cycle_start..].to_vec();
                return Ok(Some(cycle));
            }
        }

        rec_stack.remove(&node);
        path.pop();
        Ok(None)
    }

    /// Helper method to get all node IDs (simplified implementation)
    async fn get_all_node_ids(&self) -> Result<Vec<NodeId>> {
        // This is a placeholder - in a real implementation you'd want to
        // iterate through the storage more efficiently
        Ok(self.cached_node_ids())
    }

    /// Find strongly connected components using Tarjan's algorithm
    pub async fn find_strongly_connected_components(&self) -> Result<Vec<Vec<NodeId>>> {
        let mut index_counter = 0usize;
        let mut stack = Vec::new();
        let mut indices = HashMap::new();
        let mut lowlinks = HashMap::new();
        let mut on_stack = HashSet::new();
        let mut components = Vec::new();

        let all_nodes = self.get_all_node_ids().await?;

        for node in all_nodes {
            if !indices.contains_key(&node) {
                self.tarjan_dfs(
                    node,
                    &mut index_counter,
                    &mut stack,
                    &mut indices,
                    &mut lowlinks,
                    &mut on_stack,
                    &mut components,
                )
                .await?;
            }
        }

        Ok(components)
    }

    /// Tarjan's DFS for strongly connected components
    async fn tarjan_dfs(
        &self,
        node: NodeId,
        index_counter: &mut usize,
        stack: &mut Vec<NodeId>,
        indices: &mut HashMap<NodeId, usize>,
        lowlinks: &mut HashMap<NodeId, usize>,
        on_stack: &mut HashSet<NodeId>,
        components: &mut Vec<Vec<NodeId>>,
    ) -> Result<()> {
        indices.insert(node, *index_counter);
        lowlinks.insert(node, *index_counter);
        *index_counter += 1;
        stack.push(node);
        on_stack.insert(node);

        let neighbors = self.get_neighbors(node).await?;
        for neighbor in neighbors {
            if !indices.contains_key(&neighbor) {
                Box::pin(self.tarjan_dfs(
                    neighbor,
                    index_counter,
                    stack,
                    indices,
                    lowlinks,
                    on_stack,
                    components,
                ))
                .await?;
                let neighbor_lowlink = *lowlinks.get(&neighbor).unwrap();
                let current_lowlink = *lowlinks.get(&node).unwrap();
                lowlinks.insert(node, current_lowlink.min(neighbor_lowlink));
            } else if on_stack.contains(&neighbor) {
                let neighbor_index = *indices.get(&neighbor).unwrap();
                let current_lowlink = *lowlinks.get(&node).unwrap();
                lowlinks.insert(node, current_lowlink.min(neighbor_index));
            }
        }

        // If node is a root node, pop the stack and create component
        if lowlinks[&node] == indices[&node] {
            let mut component = Vec::new();
            loop {
                let w = stack.pop().unwrap();
                on_stack.remove(&w);
                component.push(w);
                if w == node {
                    break;
                }
            }
            components.push(component);
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::StreamExt;
    use uuid::Uuid;

    #[tokio::test]
    async fn test_bfs_iterator() {
        let graph = CodeGraph::new();
        let start_node = Uuid::new_v4();

        let mut bfs_iter = graph.bfs_iter(start_node);
        let first = bfs_iter.next().await;
        assert!(first.is_some());
    }

    #[tokio::test]
    async fn test_dfs_iterator() {
        let graph = CodeGraph::new();
        let start_node = Uuid::new_v4();

        let mut dfs_iter = graph.dfs_iter(start_node);
        let first = dfs_iter.next().await;
        assert!(first.is_some());
    }

    #[tokio::test]
    async fn test_dijkstra_shortest_path() {
        let graph = CodeGraph::new();
        let start = Uuid::new_v4();
        let target = Uuid::new_v4();

        let result = graph.dijkstra_shortest_path(start, target).await;
        assert!(result.is_ok());
    }
}
