use codegraph_core::{CodeNode, EdgeType, NodeId, NodeType};
use codegraph_graph::{CodeEdge, CodeGraph};
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use std::time::Duration;
use tokio::runtime::Runtime;
use uuid::Uuid;

/// Generate a test graph with specified number of nodes and edges
async fn create_simple_test_graph(
    node_count: usize,
    edge_density: f64,
) -> (CodeGraph, Vec<NodeId>) {
    let mut graph = CodeGraph::new();
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

/// Create a simple path graph for shortest path testing
async fn create_path_graph(path_length: usize) -> (CodeGraph, NodeId, NodeId) {
    let mut graph = CodeGraph::new();
    let mut node_ids = Vec::new();

    // Create nodes in a line
    for i in 0..path_length {
        let node_id = Uuid::new_v4();
        let node = CodeNode::new(
            node_id,
            format!("node_{}", i),
            NodeType::Function,
            format!("/test/path_{}.rs", i),
        );
        graph.add_node(node).await.unwrap();
        node_ids.push(node_id);
    }

    // Create edges to form a path
    for i in 0..path_length - 1 {
        let edge = CodeEdge::new(node_ids[i], node_ids[i + 1], EdgeType::Calls);
        graph.add_edge(edge).await.unwrap();
    }

    let start = node_ids[0];
    let end = node_ids[path_length - 1];

    (graph, start, end)
}

fn bench_neighbor_queries(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let mut group = c.benchmark_group("neighbor_queries");
    group.measurement_time(Duration::from_secs(5));

    for size in [100, 500, 1000].iter() {
        group.bench_with_input(BenchmarkId::new("get_neighbors", size), size, |b, &size| {
            b.to_async(&rt).iter(|| async {
                let (graph, node_ids) = create_simple_test_graph(size, 2.0).await;
                let start_node = node_ids[0];

                let result = graph.get_neighbors(start_node).await.unwrap();
                black_box(result);
            });
        });
    }

    group.finish();
}

fn bench_shortest_path(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let mut group = c.benchmark_group("shortest_path");
    group.measurement_time(Duration::from_secs(10));

    for length in [10, 50, 100, 200].iter() {
        group.bench_with_input(
            BenchmarkId::new("path_length", length),
            length,
            |b, &length| {
                b.to_async(&rt).iter(|| async {
                    let (graph, start, end) = create_path_graph(length).await;

                    let result = graph.shortest_path(start, end).await.unwrap();
                    black_box(result);
                });
            },
        );
    }

    group.finish();
}

fn bench_graph_operations(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let mut group = c.benchmark_group("graph_operations");
    group.measurement_time(Duration::from_secs(8));

    // Test node addition performance
    group.bench_function("add_nodes_100", |b| {
        b.to_async(&rt).iter(|| async {
            let mut graph = CodeGraph::new();

            for i in 0..100 {
                let node_id = Uuid::new_v4();
                let node = CodeNode::new(
                    node_id,
                    format!("node_{}", i),
                    NodeType::Function,
                    format!("/test/file_{}.rs", i),
                );
                graph.add_node(node).await.unwrap();
            }

            black_box(graph);
        });
    });

    // Test edge addition performance
    group.bench_function("add_edges_100", |b| {
        b.to_async(&rt).iter(|| async {
            let mut graph = CodeGraph::new();
            let node_id1 = Uuid::new_v4();
            let node_id2 = Uuid::new_v4();

            let node1 = CodeNode::new(
                node_id1,
                "node1".to_string(),
                NodeType::Function,
                "/test/file1.rs".to_string(),
            );
            let node2 = CodeNode::new(
                node_id2,
                "node2".to_string(),
                NodeType::Function,
                "/test/file2.rs".to_string(),
            );

            graph.add_node(node1).await.unwrap();
            graph.add_node(node2).await.unwrap();

            for _ in 0..100 {
                let edge = CodeEdge::new(node_id1, node_id2, EdgeType::Calls);
                graph.add_edge(edge).await.unwrap();
            }

            black_box(graph);
        });
    });

    group.finish();
}

// Performance target benchmarks - should complete in <50ms
fn bench_performance_targets(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let mut group = c.benchmark_group("performance_targets");
    group.significance_level(0.1).sample_size(20);

    // Target: Get neighbors on 1000-node graph < 50ms
    group.bench_function("neighbors_1000_nodes_target", |b| {
        b.to_async(&rt).iter(|| async {
            let (graph, node_ids) = create_simple_test_graph(1000, 1.5).await;
            let start = node_ids[0];

            let result = graph.get_neighbors(start).await.unwrap();
            black_box(result);
        });
    });

    // Target: Shortest path on 100-node path < 50ms
    group.bench_function("shortest_path_100_nodes_target", |b| {
        b.to_async(&rt).iter(|| async {
            let (graph, start, end) = create_path_graph(100).await;

            let result = graph.shortest_path(start, end).await.unwrap();
            black_box(result);
        });
    });

    // Target: Graph with 500 nodes, 1000 edges creation < 50ms
    group.bench_function("graph_creation_500_nodes_target", |b| {
        b.to_async(&rt).iter(|| async {
            let (graph, _) = create_simple_test_graph(500, 2.0).await;
            black_box(graph);
        });
    });

    group.finish();
}

fn bench_caching_performance(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let mut group = c.benchmark_group("caching");
    group.measurement_time(Duration::from_secs(8));

    // Compare cached vs non-cached performance
    group.bench_function("neighbors_with_cache", |b| {
        b.to_async(&rt).iter(|| async {
            let (graph, node_ids) = create_simple_test_graph(200, 2.0).await;

            // Make multiple calls to the same node to test caching
            let test_node = node_ids[0];
            for _ in 0..10 {
                let result = graph.get_neighbors(test_node).await.unwrap();
                black_box(result);
            }
        });
    });

    group.bench_function("neighbors_without_cache", |b| {
        b.to_async(&rt).iter(|| async {
            let (graph, node_ids) = create_simple_test_graph(200, 2.0).await;

            // Make calls to different nodes to avoid caching benefits
            for &node_id in &node_ids[0..10] {
                let result = graph.get_neighbors(node_id).await.unwrap();
                black_box(result);
            }
        });
    });

    group.finish();
}

criterion_group!(
    simple_traversal_benches,
    bench_neighbor_queries,
    bench_shortest_path,
    bench_graph_operations,
    bench_performance_targets,
    bench_caching_performance
);

criterion_main!(simple_traversal_benches);
