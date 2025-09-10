use codegraph_core::{Language, Location, NodeId, NodeType};
use codegraph_graph::{CodeEdge, CodeGraph, HighPerformanceEdge};
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use std::time::{Duration, Instant};
use tokio::runtime::Runtime;

fn make_node(idx: usize) -> codegraph_core::CodeNode {
    let location = Location {
        file_path: format!("test{}.rs", idx),
        line: 1,
        column: 1,
        end_line: Some(10),
        end_column: Some(20),
    };
    codegraph_core::CodeNode::new(
        format!("test_function_{}", idx),
        Some(NodeType::Function),
        Some(Language::Rust),
        location,
    )
    .with_content(format!("fn test_function_{}() {{}}", idx))
}

async fn create_test_nodes(count: usize) -> Vec<codegraph_core::CodeNode> {
    (0..count).map(make_node).collect()
}

async fn create_test_edges(count: usize, nodes: &[codegraph_core::CodeNode]) -> Vec<HighPerformanceEdge> {
    let mut edges = Vec::with_capacity(count);
    for _ in 0..count {
        let from_idx = fastrand::usize(0..nodes.len());
        let to_idx = fastrand::usize(0..nodes.len());
        let from = nodes[from_idx].id;
        let to = nodes[to_idx].id;
        let edge = HighPerformanceEdge::new(from, to, "calls".to_string());
        edges.push(edge);
    }
    edges
}

async fn benchmark_add_nodes(graph: &mut CodeGraph, nodes: &[codegraph_core::CodeNode]) {
    for node in nodes {
        let _ = graph.add_node(node.clone()).await;
    }
}

async fn benchmark_get_nodes(graph: &CodeGraph, node_ids: &[NodeId]) {
    for &id in node_ids {
        let _ = graph.get_node(id).await;
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
    let tasks = (0..num_threads)
        .map(|_| {
            let graph = graph.clone();
            let ids = node_ids.to_vec();
            tokio::spawn(async move {
                for &id in &ids {
                    let _ = graph.get_node(id).await;
                }
            })
        })
        .collect::<Vec<_>>();

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

    for size in [100, 1000, 10_000, 50_000].iter() {
        // Node operations (simple)
        group.bench_with_input(BenchmarkId::new("add_nodes", size), size, |b, &size| {
            b.to_async(&rt).iter(|| async {
                let mut graph = CodeGraph::new().unwrap();
                let nodes = create_test_nodes(size).await;
                let start = Instant::now();
                benchmark_add_nodes(black_box(&mut graph), black_box(&nodes)).await;
                let duration = start.elapsed();
                assert_latency_target(duration, 50, &format!("add_{}_nodes", size));
            });
        });

        group.bench_with_input(BenchmarkId::new("get_nodes_cached", size), size, |b, &size| {
            b.to_async(&rt).iter_with_setup(
                || rt.block_on(async {
                    let mut graph = CodeGraph::new_with_cache().unwrap();
                    let nodes = create_test_nodes(size).await;
                    let mut node_ids = Vec::with_capacity(size);
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
        });

        // Edge operations (simple neighbor lookups)
        if *size <= 10_000 {
            group.bench_with_input(BenchmarkId::new("add_edges_batch", size), size, |b, &size| {
                b.to_async(&rt).iter_with_setup(
                    || rt.block_on(async {
                        let mut graph = CodeGraph::new().unwrap();
                        let nodes = create_test_nodes(size).await;
                        for node in &nodes {
                            let _ = graph.add_node(node.clone()).await;
                        }
                        let edges = create_test_edges(size * 2, &nodes).await;
                        (graph, edges)
                    }),
                    |(graph, edges)| async move {
                        let start = Instant::now();
                        benchmark_batch_add_edges(black_box(&graph), black_box(edges)).await;
                        let duration = start.elapsed();
                        assert_latency_target(duration, 50, &format!("batch_add_{}_edges", size * 2));
                    },
                );
            });

            group.bench_with_input(BenchmarkId::new("get_neighbors", size), size, |b, &size| {
                b.to_async(&rt).iter_with_setup(
                    || rt.block_on(async {
                        let mut graph = CodeGraph::new_with_cache().unwrap();
                        let nodes = create_test_nodes(size).await;
                        let edges = create_test_edges(size * 3, &nodes).await;
                        for node in &nodes {
                            let _ = graph.add_node(node.clone()).await;
                        }
                        let _ = graph.batch_add_edges(edges).await;
                        let node_ids: Vec<NodeId> = nodes.iter().map(|n| n.id).collect();
                        (graph, node_ids)
                    }),
                    |(graph, node_ids)| async move {
                        let start = Instant::now();
                        benchmark_get_neighbors(black_box(&graph), black_box(&node_ids)).await;
                        let duration = start.elapsed();
                        assert_latency_target(duration, 50, &format!("get_neighbors_{}", node_ids.len()));
                    },
                );
            });
        }
    }

    group.finish();
}

fn bench_shortest_path(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let mut group = c.benchmark_group("shortest_path");
    group.measurement_time(Duration::from_secs(20));

    for size in [100, 1000, 5000].iter() {
        group.bench_with_input(BenchmarkId::new("bfs_shortest_path", size), size, |b, &size| {
            b.to_async(&rt).iter_with_setup(
                || rt.block_on(async {
                    let mut graph = CodeGraph::new_with_cache().unwrap();
                    let nodes = create_test_nodes(size).await;
                    let edges = create_test_edges(size * 2, &nodes).await;
                    for node in &nodes {
                        let _ = graph.add_node(node.clone()).await;
                    }
                    let _ = graph.batch_add_edges(edges).await;
                    let pairs: Vec<(NodeId, NodeId)> = (0..10)
                        .map(|_| {
                            let a = fastrand::usize(0..size);
                            let b = fastrand::usize(0..size);
                            (nodes[a].id, nodes[b].id)
                        })
                        .collect();
                    (graph, pairs)
                }),
                |(graph, pairs)| async move {
                    let start = Instant::now();
                    benchmark_shortest_path(black_box(&graph), black_box(&pairs)).await;
                    let duration = start.elapsed();
                    // Complex query target per requirements (<200ms)
                    assert_latency_target(duration, 200, "shortest_path_batch");
                },
            );
        });
    }

    group.finish();
}

fn bench_concurrent_operations(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let mut group = c.benchmark_group("concurrent_operations");
    group.measurement_time(Duration::from_secs(15));

    for threads in [2, 4, 8, 16].iter() {
        group.bench_with_input(BenchmarkId::new("concurrent_reads", threads), threads, |b, &threads| {
            b.to_async(&rt).iter_with_setup(
                || rt.block_on(async {
                    let mut graph = CodeGraph::new_with_cache().unwrap();
                    let nodes = create_test_nodes(10_000).await;
                    for node in &nodes {
                        let _ = graph.add_node(node.clone()).await;
                    }
                    let node_ids: Vec<NodeId> = nodes.iter().take(1000).map(|n| n.id).collect();
                    (graph, node_ids)
                }),
                |(graph, node_ids)| async move {
                    let start = Instant::now();
                    benchmark_concurrent_reads(black_box(&graph), black_box(&node_ids), black_box(threads)).await;
                    let duration = start.elapsed();
                    assert_latency_target(duration, 50, &format!("concurrent_reads_{}_threads", threads));
                },
            );
        });
    }

    group.finish();
}

fn bench_large_graph_operations(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let mut group = c.benchmark_group("large_graph_operations");
    group.measurement_time(Duration::from_secs(30));
    group.sample_size(10);

    for size in [100_000].iter() {
        group.bench_with_input(BenchmarkId::new("populate_large_graph", size), size, |b, &size| {
            b.to_async(&rt).iter(|| async {
                let mut graph = CodeGraph::new_with_cache().unwrap();
                let nodes = create_test_nodes(size).await;
                let edges = create_test_edges(size / 2, &nodes).await;

                let start = Instant::now();
                for chunk in nodes.chunks(1000) {
                    for node in chunk {
                        let _ = graph.add_node(node.clone()).await;
                    }
                }
                let _ = graph.batch_add_edges(edges).await;
                let setup_duration = start.elapsed();
                println!("Large graph setup ({}k nodes) took: {:?}", size / 1000, setup_duration);

                let query_start = Instant::now();
                let test_nodes: Vec<NodeId> = (0..100).map(|_| {
                    let idx = fastrand::usize(0..size);
                    nodes[idx].id
                }).collect();
                for &node_id in &test_nodes {
                    let _ = graph.get_node(node_id).await;
                    let _ = graph.get_neighbors(node_id).await;
                }
                let query_duration = query_start.elapsed();
                assert_latency_target(query_duration, 50, "query_large_graph_100k_nodes");

                let stats = graph.get_query_stats();
                println!("Query stats: cache_hits={}, cache_misses={}, avg_ns={}", stats.cache_hits, stats.cache_misses, stats.avg_query_time_ns);
            });
        });
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
