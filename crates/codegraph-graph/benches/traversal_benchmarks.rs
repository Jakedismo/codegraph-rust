use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use codegraph_core::{CodeNode, EdgeType, NodeType, NodeId};
use codegraph_graph::{CodeGraph, CodeEdge, TraversalConfig};
use futures::StreamExt;
use std::collections::HashMap;
use std::time::Duration;
use tokio::runtime::Runtime;
use uuid::Uuid;

/// Generate a test graph with specified number of nodes and edges
async fn create_test_graph(node_count: usize, edge_density: f64) -> (CodeGraph, Vec<NodeId>) {
    let mut graph = CodeGraph::new_with_cache();
    let mut node_ids = Vec::new();
    
    // Create nodes
    for i in 0..node_count {
        let node_id = Uuid::new_v4();
        let node = CodeNode::new(
            node_id,
            format!("node_{}", i),
            NodeType::Function,
            format!("/test/file_{}.rs", i % 10), // Distribute across 10 files
        );
        graph.add_node(node).await.unwrap();
        node_ids.push(node_id);
    }
    
    // Create edges based on density
    let edge_count = (node_count as f64 * edge_density) as usize;
    for _ in 0..edge_count {
        let from_idx = fastrand::usize(..node_count);
        let to_idx = fastrand::usize(..node_count);
        
        if from_idx != to_idx {
            let edge = CodeEdge::new(node_ids[from_idx], node_ids[to_idx], EdgeType::Calls);
            graph.add_edge(edge).await.unwrap();
        }
    }
    
    (graph, node_ids)
}

/// Create a tree-like graph for shortest path testing
async fn create_tree_graph(depth: usize, branching_factor: usize) -> (CodeGraph, NodeId, NodeId) {
    let mut graph = CodeGraph::new_with_cache();
    let mut current_level = vec![Uuid::new_v4()];
    let root = current_level[0];
    
    // Add root node
    let root_node = CodeNode::new(root, "root".to_string(), NodeType::Module, "/test/root.rs".to_string());
    graph.add_node(root_node).await.unwrap();
    
    let mut leaf = root;
    
    for level in 1..=depth {
        let mut next_level = Vec::new();
        
        for parent in &current_level {
            for i in 0..branching_factor {
                let child_id = Uuid::new_v4();
                let child_node = CodeNode::new(
                    child_id,
                    format!("node_{}_{}", level, i),
                    NodeType::Function,
                    format!("/test/level_{}.rs", level),
                );
                
                graph.add_node(child_node).await.unwrap();
                
                let edge = CodeEdge::new(*parent, child_id, EdgeType::Calls);
                graph.add_edge(edge).await.unwrap();
                
                next_level.push(child_id);
                leaf = child_id; // Keep updating to get a deep leaf
            }
        }
        
        current_level = next_level;
    }
    
    (graph, root, leaf)
}

/// Create a cyclic graph for cycle detection testing
async fn create_cyclic_graph(cycle_length: usize, additional_nodes: usize) -> (CodeGraph, Vec<NodeId>) {
    let mut graph = CodeGraph::new_with_cache();
    let mut node_ids = Vec::new();
    
    // Create cycle nodes
    for i in 0..cycle_length {
        let node_id = Uuid::new_v4();
        let node = CodeNode::new(
            node_id,
            format!("cycle_node_{}", i),
            NodeType::Module,
            format!("/test/cycle_{}.rs", i),
        );
        graph.add_node(node).await.unwrap();
        node_ids.push(node_id);
    }
    
    // Create the cycle
    for i in 0..cycle_length {
        let next = (i + 1) % cycle_length;
        let edge = CodeEdge::new(node_ids[i], node_ids[next], EdgeType::Imports);
        graph.add_edge(edge).await.unwrap();
    }
    
    // Add additional nodes
    for i in 0..additional_nodes {
        let node_id = Uuid::new_v4();
        let node = CodeNode::new(
            node_id,
            format!("extra_node_{}", i),
            NodeType::Function,
            format!("/test/extra_{}.rs", i),
        );
        graph.add_node(node).await.unwrap();
        node_ids.push(node_id);
        
        // Connect to a random cycle node
        if !node_ids.is_empty() {
            let target_idx = fastrand::usize(..cycle_length);
            let edge = CodeEdge::new(node_id, node_ids[target_idx], EdgeType::Calls);
            graph.add_edge(edge).await.unwrap();
        }
    }
    
    (graph, node_ids)
}

fn bench_bfs_traversal(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("bfs_traversal");
    
    for size in [100, 500, 1000, 2000].iter() {
        group.bench_with_input(
            BenchmarkId::new("dense_graph", size),
            size,
            |b, &size| {
                b.to_async(&rt).iter(|| async {
                    let (graph, node_ids) = create_test_graph(size, 2.0).await;
                    let start_node = node_ids[0];
                    
                    let config = TraversalConfig {
                        max_depth: Some(5),
                        max_nodes: Some(100),
                        include_start: true,
                        filter: None,
                    };
                    
                    let mut bfs_iter = graph.bfs_iter_with_config(start_node, config);
                    let mut count = 0;
                    
                    while let Some(result) = bfs_iter.next().await {
                        if result.is_ok() {
                            count += 1;
                        }
                        black_box(result);
                    }
                    
                    black_box(count);
                });
            },
        );
    }
    
    group.finish();
}

fn bench_dfs_traversal(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("dfs_traversal");
    
    for size in [100, 500, 1000, 2000].iter() {
        group.bench_with_input(
            BenchmarkId::new("sparse_graph", size),
            size,
            |b, &size| {
                b.to_async(&rt).iter(|| async {
                    let (graph, node_ids) = create_test_graph(size, 1.2).await;
                    let start_node = node_ids[0];
                    
                    let config = TraversalConfig {
                        max_depth: Some(10),
                        max_nodes: Some(200),
                        include_start: true,
                        filter: None,
                    };
                    
                    let mut dfs_iter = graph.dfs_iter_with_config(start_node, config);
                    let mut count = 0;
                    
                    while let Some(result) = dfs_iter.next().await {
                        if result.is_ok() {
                            count += 1;
                        }
                        black_box(result);
                    }
                    
                    black_box(count);
                });
            },
        );
    }
    
    group.finish();
}

fn bench_shortest_path_algorithms(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("shortest_path");
    group.measurement_time(Duration::from_secs(10));
    
    for depth in [5, 10, 15, 20].iter() {
        // BFS shortest path
        group.bench_with_input(
            BenchmarkId::new("bfs", depth),
            depth,
            |b, &depth| {
                b.to_async(&rt).iter(|| async {
                    let (graph, root, leaf) = create_tree_graph(depth, 3).await;
                    
                    let result = graph.shortest_path(root, leaf).await.unwrap();
                    black_box(result);
                });
            },
        );
        
        // Dijkstra
        group.bench_with_input(
            BenchmarkId::new("dijkstra", depth),
            depth,
            |b, &depth| {
                b.to_async(&rt).iter(|| async {
                    let (graph, root, leaf) = create_tree_graph(depth, 3).await;
                    
                    let result = graph.dijkstra_shortest_path(root, leaf).await.unwrap();
                    black_box(result);
                });
            },
        );
        
        // A* with Manhattan distance heuristic
        group.bench_with_input(
            BenchmarkId::new("astar", depth),
            depth,
            |b, &depth| {
                b.to_async(&rt).iter(|| async {
                    let (graph, root, leaf) = create_tree_graph(depth, 3).await;
                    
                    // Simple heuristic: assume each step costs 1.0
                    let heuristic = |_from: NodeId, _to: NodeId| 1.0;
                    
                    let result = graph.astar_shortest_path(root, leaf, heuristic).await.unwrap();
                    black_box(result);
                });
            },
        );
    }
    
    group.finish();
}

fn bench_cycle_detection(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("cycle_detection");
    group.measurement_time(Duration::from_secs(15));
    
    for (cycle_len, extra_nodes) in [(5, 50), (10, 100), (20, 200), (50, 500)].iter() {
        group.bench_with_input(
            BenchmarkId::new("detect_cycles", format!("{}_{}", cycle_len, extra_nodes)),
            &(*cycle_len, *extra_nodes),
            |b, &(cycle_len, extra_nodes)| {
                b.to_async(&rt).iter(|| async {
                    let (graph, _nodes) = create_cyclic_graph(cycle_len, extra_nodes).await;
                    
                    let result = graph.detect_cycles().await.unwrap();
                    black_box(result);
                });
            },
        );
    }
    
    group.finish();
}

fn bench_strongly_connected_components(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("strongly_connected_components");
    group.measurement_time(Duration::from_secs(20));
    
    for size in [100, 250, 500, 1000].iter() {
        group.bench_with_input(
            BenchmarkId::new("tarjan_scc", size),
            size,
            |b, &size| {
                b.to_async(&rt).iter(|| async {
                    let (graph, _nodes) = create_test_graph(size, 1.5).await;
                    
                    let result = graph.find_strongly_connected_components().await.unwrap();
                    black_box(result);
                });
            },
        );
    }
    
    group.finish();
}

fn bench_cache_performance(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("cache_performance");
    
    // Benchmark cached vs non-cached neighbor queries
    group.bench_function("neighbors_cached", |b| {
        b.to_async(&rt).iter(|| async {
            let (graph, node_ids) = create_test_graph(1000, 2.0).await;
            
            // Warm up cache
            for &node_id in &node_ids[0..100] {
                let _ = graph.get_neighbors(node_id).await.unwrap();
            }
            
            // Benchmark cached queries
            for &node_id in &node_ids[0..100] {
                let result = graph.get_neighbors(node_id).await.unwrap();
                black_box(result);
            }
        });
    });
    
    group.bench_function("neighbors_uncached", |b| {
        b.to_async(&rt).iter(|| async {
            let (graph, node_ids) = create_test_graph(1000, 2.0).await;
            
            // Benchmark uncached queries (different nodes each time)
            for i in 0..100 {
                let node_id = node_ids[i % node_ids.len()];
                if let Some(optimizer) = graph.query_optimizer() {
                    optimizer.cache().clear_all();
                }
                let result = graph.get_neighbors(node_id).await.unwrap();
                black_box(result);
            }
        });
    });
    
    group.finish();
}

fn bench_graph_size_scaling(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("graph_scaling");
    group.measurement_time(Duration::from_secs(30));
    
    // Test how algorithms scale with graph size
    for size in [500, 1000, 2000, 4000].iter() {
        // BFS scaling
        group.bench_with_input(
            BenchmarkId::new("bfs_scaling", size),
            size,
            |b, &size| {
                b.to_async(&rt).iter(|| async {
                    let (graph, node_ids) = create_test_graph(size, 1.5).await;
                    let start_node = node_ids[0];
                    
                    let config = TraversalConfig {
                        max_depth: Some(6),
                        max_nodes: Some(200),
                        include_start: true,
                        filter: None,
                    };
                    
                    let mut bfs_iter = graph.bfs_iter_with_config(start_node, config);
                    let mut visited = 0;
                    
                    while let Some(result) = bfs_iter.next().await {
                        if result.is_ok() {
                            visited += 1;
                        }
                    }
                    
                    black_box(visited);
                });
            },
        );
        
        // Shortest path scaling
        group.bench_with_input(
            BenchmarkId::new("shortest_path_scaling", size),
            size,
            |b, &size| {
                b.to_async(&rt).iter(|| async {
                    let (graph, node_ids) = create_test_graph(size, 1.2).await;
                    let start = node_ids[0];
                    let end = node_ids[node_ids.len() / 2];
                    
                    let result = graph.dijkstra_shortest_path(start, end).await.unwrap();
                    black_box(result);
                });
            },
        );
    }
    
    group.finish();
}

// Target: <50ms for most traversal operations
fn bench_performance_targets(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("performance_targets");
    group.significance_level(0.1).sample_size(20);
    
    // Target: BFS on 1000 nodes < 50ms
    group.bench_function("bfs_1000_nodes_target", |b| {
        b.to_async(&rt).iter(|| async {
            let (graph, node_ids) = create_test_graph(1000, 1.5).await;
            let start = node_ids[0];
            
            let config = TraversalConfig {
                max_depth: Some(5),
                max_nodes: Some(100),
                include_start: true,
                filter: None,
            };
            
            let mut bfs_iter = graph.bfs_iter_with_config(start, config);
            let mut count = 0;
            
            while let Some(result) = bfs_iter.next().await {
                if result.is_ok() {
                    count += 1;
                }
            }
            
            black_box(count);
        });
    });
    
    // Target: Shortest path on 500 nodes < 50ms
    group.bench_function("shortest_path_500_nodes_target", |b| {
        b.to_async(&rt).iter(|| async {
            let (graph, node_ids) = create_test_graph(500, 1.8).await;
            let start = node_ids[0];
            let end = node_ids[node_ids.len() - 1];
            
            let result = graph.dijkstra_shortest_path(start, end).await.unwrap();
            black_box(result);
        });
    });
    
    // Target: Cycle detection on 200 nodes < 50ms
    group.bench_function("cycle_detection_200_nodes_target", |b| {
        b.to_async(&rt).iter(|| async {
            let (graph, _) = create_cyclic_graph(10, 190).await;
            
            let result = graph.detect_cycles().await.unwrap();
            black_box(result);
        });
    });
    
    group.finish();
}

criterion_group!(
    traversal_benches,
    bench_bfs_traversal,
    bench_dfs_traversal,
    bench_shortest_path_algorithms,
    bench_cycle_detection,
    bench_strongly_connected_components,
    bench_cache_performance,
    bench_graph_size_scaling,
    bench_performance_targets
);

criterion_main!(traversal_benches);