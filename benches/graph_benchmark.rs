use codegraph_core::*;
use codegraph_graph::*;
use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use std::time::{Duration, Instant};
use tokio::runtime::Runtime;
use std::collections::HashMap;
use fastrand;

async fn create_test_nodes(count: usize) -> Vec<CodeNode> {
    let mut nodes = Vec::new();
    for i in 0..count {
        let node = CodeNode {
            id: i as NodeId,
            name: format!("test_function_{}", i),
            node_type: "function".to_string(),
            file_path: Some(format!("test{}.rs", i)),
            start_line: Some(1),
            end_line: Some(10),
            metadata: HashMap::new(),
        };
        nodes.push(node);
    }
    nodes
}

async fn create_test_edges(count: usize, max_node_id: usize) -> Vec<HighPerformanceEdge> {
    let mut edges = Vec::new();
    for i in 0..count {
        let from = fastrand::usize(0..max_node_id) as NodeId;
        let to = fastrand::usize(0..max_node_id) as NodeId;
        
        let edge = HighPerformanceEdge {
            id: i as u64,
            from,
            to,
            edge_type: "calls".to_string(),
            weight: 1.0,
            metadata: HashMap::new(),
        };
        edges.push(edge);
    }
    edges
}

async fn benchmark_add_nodes(graph: &mut CodeGraph, nodes: &[CodeNode]) {
    for node in nodes {
        let _ = graph.add_node(node.clone()).await;
    }
}

async fn benchmark_get_nodes(graph: &CodeGraph, node_ids: &[NodeId]) {
    for &id in node_ids {
        let _ = graph.get_node(id).await;
    }
}

async fn benchmark_add_edges(graph: &CodeGraph, edges: &[HighPerformanceEdge]) {
    for edge in edges {
        let _ = graph.add_high_performance_edge(edge.clone()).await;
    }
}

async fn benchmark_batch_add_edges(graph: &CodeGraph, edges: Vec<HighPerformanceEdge>) {
    let _ = graph.batch_add_edges(edges).await;
}

async fn benchmark_get_neighbors(graph: &CodeGraph, node_ids: &[NodeId]) {
    for &id in node_ids {
        let _ = graph.get_neighbors(id).await;
    }
}

async fn benchmark_shortest_path(graph: &CodeGraph, pairs: &[(NodeId, NodeId)]) {
    for &(from, to) in pairs {
        let _ = graph.shortest_path(from, to).await;
    }
}

async fn benchmark_concurrent_reads(graph: &CodeGraph, node_ids: &[NodeId], num_threads: usize) {
    let tasks = (0..num_threads).map(|_| {
        let graph = graph.clone();
        let ids = node_ids.to_vec();
        tokio::spawn(async move {
            for &id in &ids {
                let _ = graph.get_node(id).await;
            }
        })
    }).collect::<Vec<_>>();
    
    for task in tasks {
        let _ = task.await;
    }
}

fn assert_latency_target(duration: Duration, target_ms: u64, operation: &str) {
    let actual_ms = duration.as_millis() as u64;
    if actual_ms > target_ms {
        eprintln!("⚠️  {} took {}ms, exceeding {}ms target", operation, actual_ms, target_ms);
    } else {
        println!("✅ {} took {}ms (target: {}ms)", operation, actual_ms, target_ms);
    }
}

fn bench_graph_operations(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("graph_operations");
    group.measurement_time(Duration::from_secs(15));
    group.warm_up_time(Duration::from_secs(3));
    
    for size in [100, 1000, 10000, 100000].iter() {
        // Node operations
        group.bench_with_input(
            BenchmarkId::new("add_nodes", size),
            size,
            |b, &size| {
                b.to_async(&rt).iter(|| async {
                    let mut graph = CodeGraph::new().unwrap();
                    let nodes = create_test_nodes(size).await;
                    let start = Instant::now();
                    benchmark_add_nodes(black_box(&mut graph), black_box(&nodes)).await;
                    let duration = start.elapsed();
                    assert_latency_target(duration, 50, &format!("add_{}_nodes", size));
                });
            },
        );
        
        group.bench_with_input(
            BenchmarkId::new("get_nodes_cached", size),
            size,
            |b, &size| {
                b.to_async(&rt).iter_with_setup(
                    || rt.block_on(async {
                        let mut graph = CodeGraph::new_with_cache().unwrap();
                        let nodes = create_test_nodes(size).await;
                        let mut node_ids = Vec::new();
                        
                        for node in &nodes {
                            let _ = graph.add_node(node.clone()).await;
                            node_ids.push(node.id);
                        }
                        
                        (graph, node_ids)
                    }),
                    |(graph, node_ids)| async move {
                        let start = Instant::now();
                        benchmark_get_nodes(black_box(&graph), black_box(&node_ids)).await;
                        let duration = start.elapsed();
                        assert_latency_target(duration, 50, &format!("get_{}_nodes_cached", node_ids.len()));
                    },
                );
            },
        );
        
        // Edge operations
        if *size <= 10000 { // Limit edge benchmarks to reasonable sizes
            group.bench_with_input(
                BenchmarkId::new("add_edges_batch", size),
                size,
                |b, &size| {
                    b.to_async(&rt).iter_with_setup(
                        || rt.block_on(async {
                            let mut graph = CodeGraph::new().unwrap();
                            let nodes = create_test_nodes(size).await;
                            
                            for node in &nodes {
                                let _ = graph.add_node(node.clone()).await;
                            }
                            
                            let edges = create_test_edges(size * 2, size).await;
                            (graph, edges)
                        }),
                        |(graph, edges)| async move {
                            let start = Instant::now();
                            benchmark_batch_add_edges(black_box(&graph), black_box(edges)).await;
                            let duration = start.elapsed();
                            assert_latency_target(duration, 50, &format!("batch_add_{}_edges", size * 2));
                        },
                    );
                },
            );
            
            group.bench_with_input(
                BenchmarkId::new("get_neighbors", size),
                size,
                |b, &size| {
                    b.to_async(&rt).iter_with_setup(
                        || rt.block_on(async {
                            let mut graph = CodeGraph::new_with_cache().unwrap();
                            let nodes = create_test_nodes(size).await;
                            let edges = create_test_edges(size * 3, size).await;
                            
                            for node in &nodes {
                                let _ = graph.add_node(node.clone()).await;
                            }
                            
                            let _ = graph.batch_add_edges(edges).await;
                            
                            let node_ids: Vec<NodeId> = (0..size).map(|i| i as NodeId).collect();
                            (graph, node_ids)
                        }),
                        |(graph, node_ids)| async move {
                            let start = Instant::now();
                            benchmark_get_neighbors(black_box(&graph), black_box(&node_ids)).await;
                            let duration = start.elapsed();
                            assert_latency_target(duration, 50, &format!("get_neighbors_{}", node_ids.len()));
                        },
                    );
                },
            );
        }
    }
    
    group.finish();
}

fn bench_shortest_path(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("shortest_path");
    group.measurement_time(Duration::from_secs(20));
    
    for size in [100, 1000, 5000].iter() {
        group.bench_with_input(
            BenchmarkId::new("bfs_shortest_path", size),
            size,
            |b, &size| {
                b.to_async(&rt).iter_with_setup(
                    || rt.block_on(async {
                        let mut graph = CodeGraph::new_with_cache().unwrap();
                        let nodes = create_test_nodes(size).await;
                        let edges = create_test_edges(size * 2, size).await;
                        
                        for node in &nodes {
                            let _ = graph.add_node(node.clone()).await;
                        }
                        
                        let _ = graph.batch_add_edges(edges).await;
                        
                        let pairs: Vec<(NodeId, NodeId)> = (0..10)
                            .map(|_| {
                                let from = fastrand::usize(0..size) as NodeId;
                                let to = fastrand::usize(0..size) as NodeId;
                                (from, to)
                            })
                            .collect();
                        
                        (graph, pairs)
                    }),
                    |(graph, pairs)| async move {
                        let start = Instant::now();
                        benchmark_shortest_path(black_box(&graph), black_box(&pairs)).await;
                        let duration = start.elapsed();
                        assert_latency_target(duration, 50, "shortest_path_batch");
                    },
                );
            },
        );
    }
    
    group.finish();
}

fn bench_concurrent_operations(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("concurrent_operations");
    group.measurement_time(Duration::from_secs(15));
    
    for threads in [2, 4, 8, 16].iter() {
        group.bench_with_input(
            BenchmarkId::new("concurrent_reads", threads),
            threads,
            |b, &threads| {
                b.to_async(&rt).iter_with_setup(
                    || rt.block_on(async {
                        let mut graph = CodeGraph::new_with_cache().unwrap();
                        let nodes = create_test_nodes(10000).await;
                        
                        for node in &nodes {
                            let _ = graph.add_node(node.clone()).await;
                        }
                        
                        let node_ids: Vec<NodeId> = (0..1000).map(|i| i as NodeId).collect();
                        (graph, node_ids)
                    }),
                    |(graph, node_ids)| async move {
                        let start = Instant::now();
                        benchmark_concurrent_reads(black_box(&graph), black_box(&node_ids), black_box(threads)).await;
                        let duration = start.elapsed();
                        assert_latency_target(duration, 50, &format!("concurrent_reads_{}_threads", threads));
                    },
                );
            },
        );
    }
    
    group.finish();
}

fn bench_large_graph_operations(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("large_graph_operations");
    group.measurement_time(Duration::from_secs(30));
    group.sample_size(10); // Fewer samples for large operations
    
    // Test with 100k+ nodes as specified in requirements
    for size in [100_000].iter() {
        group.bench_with_input(
            BenchmarkId::new("populate_large_graph", size),
            size,
            |b, &size| {
                b.to_async(&rt).iter(|| async {
                    let mut graph = CodeGraph::new_with_cache().unwrap();
                    let nodes = create_test_nodes(size).await;
                    let edges = create_test_edges(size / 2, size).await;
                    
                    let start = Instant::now();
                    
                    // Add nodes in batches
                    for chunk in nodes.chunks(1000) {
                        for node in chunk {
                            let _ = graph.add_node(node.clone()).await;
                        }
                    }
                    
                    // Add edges in batch
                    let _ = graph.batch_add_edges(edges).await;
                    
                    let setup_duration = start.elapsed();
                    println!("Large graph setup ({}k nodes) took: {:?}", size / 1000, setup_duration);
                    
                    // Now test query performance
                    let query_start = Instant::now();
                    let test_nodes: Vec<NodeId> = (0..100).map(|i| fastrand::usize(0..size) as NodeId).collect();
                    
                    for &node_id in &test_nodes {
                        let _ = graph.get_node(node_id).await;
                        let _ = graph.get_neighbors(node_id).await;
                    }
                    
                    let query_duration = query_start.elapsed();
                    assert_latency_target(query_duration, 50, &format!("query_large_graph_100k_nodes"));
                    
                    let stats = graph.get_query_stats();
                    println!("Query stats: {:?}", stats);
                });
            },
        );
    }
    
    group.finish();
}

criterion_group!(
    benches, 
    bench_graph_operations,
    bench_shortest_path,
    bench_concurrent_operations,
    bench_large_graph_operations
);
criterion_main!(benches);