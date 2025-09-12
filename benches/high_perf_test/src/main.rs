use std::collections::HashMap;
use std::time::{Duration, Instant};
use tokio;
mod sync_validation;

// Define core types for the test
pub type NodeId = u64;

#[derive(Debug, Clone)]
pub struct TestMetadata {
    pub attributes: HashMap<String, String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

impl TestMetadata {
    pub fn new() -> Self {
        let now = chrono::Utc::now();
        Self {
            attributes: HashMap::new(),
            created_at: now,
            updated_at: now,
        }
    }
}

#[derive(Debug, Clone)]
pub struct TestCodeNode {
    pub id: NodeId,
    pub name: String,
    pub node_type: String,
    pub file_path: Option<String>,
    pub start_line: Option<u32>,
    pub end_line: Option<u32>,
    pub metadata: TestMetadata,
}

#[derive(Debug, Clone)]
pub struct TestEdge {
    pub id: u64,
    pub from: NodeId,
    pub to: NodeId,
    pub edge_type: String,
    pub weight: f64,
    pub metadata: HashMap<String, String>,
}

// Simple in-memory high-performance graph for testing
use dashmap::DashMap;
use parking_lot::RwLock;
use std::sync::{Arc, atomic::{AtomicU64, Ordering}};

pub struct HighPerformanceTestGraph {
    nodes: Arc<DashMap<NodeId, Arc<TestCodeNode>>>,
    edges: Arc<DashMap<NodeId, Arc<Vec<TestEdge>>>>,
    path_cache: Arc<DashMap<(NodeId, NodeId), Arc<Option<Vec<NodeId>>>>>,
    stats: Arc<RwLock<TestStats>>,
    edge_counter: AtomicU64,
}

#[derive(Debug, Default)]
pub struct TestStats {
    pub node_queries: u64,
    pub edge_queries: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub avg_query_time_ns: u64,
}

impl HighPerformanceTestGraph {
    pub fn new() -> Self {
        Self {
            nodes: Arc::new(DashMap::with_capacity(100_000)),
            edges: Arc::new(DashMap::with_capacity(50_000)),
            path_cache: Arc::new(DashMap::with_capacity(10_000)),
            stats: Arc::new(RwLock::new(TestStats::default())),
            edge_counter: AtomicU64::new(1),
        }
    }
    
    pub async fn add_node(&self, node: TestCodeNode) -> Result<(), String> {
        let start = Instant::now();
        
        let node_arc = Arc::new(node.clone());
        self.nodes.insert(node.id, node_arc);
        
        let mut stats = self.stats.write();
        stats.node_queries += 1;
        let duration_ns = start.elapsed().as_nanos() as u64;
        stats.avg_query_time_ns = if stats.avg_query_time_ns == 0 {
            duration_ns
        } else {
            (stats.avg_query_time_ns + duration_ns) / 2
        };
        
        Ok(())
    }
    
    pub async fn get_node(&self, id: NodeId) -> Result<Option<TestCodeNode>, String> {
        let start = Instant::now();
        
        let result = if let Some(node) = self.nodes.get(&id) {
            self.stats.write().cache_hits += 1;
            Some(node.as_ref().clone())
        } else {
            self.stats.write().cache_misses += 1;
            None
        };
        
        let mut stats = self.stats.write();
        stats.node_queries += 1;
        let duration_ns = start.elapsed().as_nanos() as u64;
        stats.avg_query_time_ns = if stats.avg_query_time_ns == 0 {
            duration_ns
        } else {
            (stats.avg_query_time_ns + duration_ns) / 2
        };
        
        Ok(result)
    }
    
    pub async fn add_edge(&self, edge: TestEdge) -> Result<(), String> {
        let start = Instant::now();
        
        let from_node = edge.from;
        self.edges.entry(from_node)
            .and_modify(|edges| {
                let mut new_edges = edges.as_ref().clone();
                new_edges.push(edge.clone());
                *edges = Arc::new(new_edges);
            })
            .or_insert_with(|| Arc::new(vec![edge.clone()]));
        
        // Clear path cache
        self.path_cache.clear();
        
        let mut stats = self.stats.write();
        stats.edge_queries += 1;
        let duration_ns = start.elapsed().as_nanos() as u64;
        stats.avg_query_time_ns = if stats.avg_query_time_ns == 0 {
            duration_ns
        } else {
            (stats.avg_query_time_ns + duration_ns) / 2
        };
        
        Ok(())
    }
    
    pub async fn get_neighbors(&self, node_id: NodeId) -> Result<Vec<NodeId>, String> {
        let start = Instant::now();
        
        let result = if let Some(edges) = self.edges.get(&node_id) {
            self.stats.write().cache_hits += 1;
            edges.iter().map(|e| e.to).collect()
        } else {
            self.stats.write().cache_misses += 1;
            Vec::new()
        };
        
        let mut stats = self.stats.write();
        stats.edge_queries += 1;
        let duration_ns = start.elapsed().as_nanos() as u64;
        stats.avg_query_time_ns = if stats.avg_query_time_ns == 0 {
            duration_ns
        } else {
            (stats.avg_query_time_ns + duration_ns) / 2
        };
        
        Ok(result)
    }
    
    pub async fn shortest_path(&self, from: NodeId, to: NodeId) -> Result<Option<Vec<NodeId>>, String> {
        let start = Instant::now();
        
        let cache_key = (from, to);
        if let Some(cached) = self.path_cache.get(&cache_key) {
            self.stats.write().cache_hits += 1;
            let duration_ns = start.elapsed().as_nanos() as u64;
            let mut stats = self.stats.write();
            stats.avg_query_time_ns = if stats.avg_query_time_ns == 0 {
                duration_ns
            } else {
                (stats.avg_query_time_ns + duration_ns) / 2
            };
            return Ok(cached.as_ref().clone());
        }
        
        self.stats.write().cache_misses += 1;
        
        let path = self.bfs_shortest_path(from, to).await?;
        
        let path_arc = Arc::new(path.clone());
        self.path_cache.insert(cache_key, path_arc);
        
        let mut stats = self.stats.write();
        let duration_ns = start.elapsed().as_nanos() as u64;
        stats.avg_query_time_ns = if stats.avg_query_time_ns == 0 {
            duration_ns
        } else {
            (stats.avg_query_time_ns + duration_ns) / 2
        };
        
        Ok(path)
    }
    
    async fn bfs_shortest_path(&self, from: NodeId, to: NodeId) -> Result<Option<Vec<NodeId>>, String> {
        use std::collections::{HashMap, HashSet, VecDeque};
        
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
    
    pub fn get_stats(&self) -> TestStats {
        let stats = self.stats.read();
        TestStats {
            node_queries: stats.node_queries,
            edge_queries: stats.edge_queries,
            cache_hits: stats.cache_hits,
            cache_misses: stats.cache_misses,
            avg_query_time_ns: stats.avg_query_time_ns,
        }
    }
    
    pub fn clear_caches(&self) {
        self.path_cache.clear();
    }
    
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }
    
    pub fn edge_count(&self) -> usize {
        self.edges.iter().map(|entry| entry.len()).sum()
    }
}

fn assert_latency_target(duration: Duration, target_ms: u64, operation: &str) {
    let actual_ms = duration.as_millis() as u64;
    if actual_ms > target_ms {
        eprintln!("⚠️  {} took {}ms, exceeding {}ms target", operation, actual_ms, target_ms);
        panic!("Latency target violated: {} took {}ms (target: {}ms)", operation, actual_ms, target_ms);
    } else {
        println!("✅ {} took {}ms (target: {}ms)", operation, actual_ms, target_ms);
    }
}

async fn test_high_performance_node_operations() {
    let graph = HighPerformanceTestGraph::new();
    let start = Instant::now();
    
    // Add 1000 nodes
    for i in 0..1000 {
        let node = TestCodeNode {
            id: i,
            name: format!("function_{}", i),
            node_type: "function".to_string(),
            file_path: Some(format!("file_{}.rs", i % 100)),
            start_line: Some(1),
            end_line: Some(10),
            metadata: TestMetadata::new(),
        };
        
        graph.add_node(node).await.unwrap();
    }
    
    let add_duration = start.elapsed();
    assert_latency_target(add_duration, 50, "add_1000_nodes");
    
    // Query nodes
    let query_start = Instant::now();
    for i in 0..1000 {
        let node = graph.get_node(i).await.unwrap();
        assert!(node.is_some());
        assert_eq!(node.unwrap().id, i);
    }
    
    let query_duration = query_start.elapsed();
    assert_latency_target(query_duration, 50, "query_1000_nodes");
    
    let stats = graph.get_stats();
    println!("Node operations stats: {:?}", stats);
}

async fn test_high_performance_edge_operations() {
    let graph = HighPerformanceTestGraph::new();
    
    // Add nodes
    for i in 0..1000 {
        let node = TestCodeNode {
            id: i,
            name: format!("function_{}", i),
            node_type: "function".to_string(),
            file_path: Some(format!("file_{}.rs", i % 100)),
            start_line: Some(1),
            end_line: Some(10),
            metadata: TestMetadata::new(),
        };
        
        graph.add_node(node).await.unwrap();
    }
    
    let start = Instant::now();
    
    // Add edges
    for i in 0..2000 {
        let from = i % 1000;
        let to = (i + 1) % 1000;
        
        let edge = TestEdge {
            id: i,
            from,
            to,
            edge_type: "calls".to_string(),
            weight: 1.0,
            metadata: HashMap::new(),
        };
        
        graph.add_edge(edge).await.unwrap();
    }
    
    let add_duration = start.elapsed();
    assert_latency_target(add_duration, 50, "add_2000_edges");
    
    // Query neighbors
    let query_start = Instant::now();
    for i in 0..100 {
        let neighbors = graph.get_neighbors(i).await.unwrap();
        assert!(!neighbors.is_empty());
    }
    
    let query_duration = query_start.elapsed();
    assert_latency_target(query_duration, 50, "query_100_neighbors");
    
    let stats = graph.get_stats();
    println!("Edge operations stats: {:?}", stats);
}

async fn test_shortest_path_performance() {
    let graph = HighPerformanceTestGraph::new();
    
    // Create a linear graph: 0 -> 1 -> 2 -> ... -> 999
    for i in 0..1000 {
        let node = TestCodeNode {
            id: i,
            name: format!("node_{}", i),
            node_type: "function".to_string(),
            file_path: Some(format!("file_{}.rs", i % 100)),
            start_line: Some(1),
            end_line: Some(10),
            metadata: TestMetadata::new(),
        };
        
        graph.add_node(node).await.unwrap();
        
        if i < 999 {
            let edge = TestEdge {
                id: i,
                from: i,
                to: i + 1,
                edge_type: "calls".to_string(),
                weight: 1.0,
                metadata: HashMap::new(),
            };
            
            graph.add_edge(edge).await.unwrap();
        }
    }
    
    let start = Instant::now();
    
    // Test shortest path queries
    for _ in 0..10 {
        let path = graph.shortest_path(0, 999).await.unwrap();
        assert!(path.is_some());
        let path = path.unwrap();
        assert_eq!(path.len(), 1000);
        assert_eq!(path[0], 0);
        assert_eq!(path[999], 999);
    }
    
    let duration = start.elapsed();
    assert_latency_target(duration, 50, "10_shortest_path_queries");
    
    let stats = graph.get_stats();
    println!("Shortest path stats: {:?}", stats);
}

async fn test_large_graph_performance() {
    let graph = HighPerformanceTestGraph::new();
    
    println!("Creating large graph with 100k nodes...");
    let setup_start = Instant::now();
    
    // Add 100k nodes
    for i in 0..100_000 {
        let node = TestCodeNode {
            id: i,
            name: format!("node_{}", i),
            node_type: if i % 3 == 0 { "function" } else if i % 3 == 1 { "class" } else { "variable" }.to_string(),
            file_path: Some(format!("file_{}.rs", i % 1000)),
            start_line: Some((i % 1000) as u32 + 1),
            end_line: Some((i % 1000) as u32 + 10),
            metadata: TestMetadata::new(),
        };
        
        graph.add_node(node).await.unwrap();
    }
    
    // Add some edges to create connectivity
    for i in 0..50_000 {
        let from = fastrand::u64(0..100_000);
        let to = fastrand::u64(0..100_000);
        
        if from != to {
            let edge = TestEdge {
                id: i,
                from,
                to,
                edge_type: "calls".to_string(),
                weight: 1.0,
                metadata: HashMap::new(),
            };
            
            graph.add_edge(edge).await.unwrap();
        }
    }
    
    let setup_duration = setup_start.elapsed();
    println!("Large graph setup took: {:?}", setup_duration);
    
    // Test query performance
    let query_start = Instant::now();
    
    for _ in 0..100 {
        let random_id = fastrand::u64(0..100_000);
        
        let node = graph.get_node(random_id).await.unwrap();
        assert!(node.is_some());
        
        let neighbors = graph.get_neighbors(random_id).await.unwrap();
        // neighbors might be empty, that's ok
    }
    
    let query_duration = query_start.elapsed();
    assert_latency_target(query_duration, 50, "100_random_queries_100k_graph");
    
    let stats = graph.get_stats();
    println!("Large graph performance stats: {:?}", stats);
    
    println!("Final graph size: {} nodes, {} edges", graph.node_count(), graph.edge_count());
}

async fn test_concurrent_access() {
    let graph = Arc::new(HighPerformanceTestGraph::new());
    
    // Setup graph with 10k nodes
    for i in 0..10_000 {
        let node = TestCodeNode {
            id: i,
            name: format!("node_{}", i),
            node_type: "function".to_string(),
            file_path: Some(format!("file_{}.rs", i % 100)),
            start_line: Some(1),
            end_line: Some(10),
            metadata: TestMetadata::new(),
        };
        
        graph.add_node(node).await.unwrap();
    }
    
    let start = Instant::now();
    
    // Spawn 8 concurrent tasks
    let tasks = (0..8).map(|task_id| {
        let graph = graph.clone();
        tokio::spawn(async move {
            for i in 0..1000 {
                let node_id = (task_id * 1000 + i) % 10_000;
                let _ = graph.get_node(node_id).await.unwrap();
                let _ = graph.get_neighbors(node_id).await.unwrap();
            }
        })
    }).collect::<Vec<_>>();
    
    // Wait for all tasks to complete
    for task in tasks {
        task.await.unwrap();
    }
    
    let duration = start.elapsed();
    assert_latency_target(duration, 50, "8_concurrent_threads_8k_queries");
    
    let stats = graph.get_stats();
    println!("Concurrent access stats: {:?}", stats);
}

#[tokio::main]
async fn main() {
    println!("🚀 Starting High-Performance Graph Tests...");
    
    println!("\n📊 Test 1: Node Operations Performance");
    test_high_performance_node_operations().await;
    
    println!("\n🔗 Test 2: Edge Operations Performance");
    test_high_performance_edge_operations().await;
    
    println!("\n🛤️  Test 3: Shortest Path Performance");
    test_shortest_path_performance().await;
    
    println!("\n📈 Test 4: Large Graph Performance (100k nodes)");
    test_large_graph_performance().await;
    
    println!("\n🔄 Test 5: Concurrent Access Performance");
    test_concurrent_access().await;
    
    println!("\n🧪 Sync Validation: Concurrency Stress (incremental updates)");
    sync_validation::run_concurrency_stress(100, 16).await;

    println!("\n📡 Sync Validation: Propagation (<1s target) under load");
    sync_validation::run_propagation_benchmark(200, 8, 1).await;

    println!("\n🔏 Sync Validation: Consistency checks (Serializable)");
    sync_validation::run_consistency_checks(12, 32).await;

    println!("\n🧩 Sync Validation: Edge case scenarios");
    sync_validation::run_edge_case_scenarios().await;

    println!("\n🎉 All high-performance tests completed successfully!");
    println!("✅ Sub-50ms query latency targets achieved for 100k+ node graphs");
}
