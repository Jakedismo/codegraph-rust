use codegraph_core::*;
use codegraph_graph::*;
use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use std::time::Duration;
use tokio::runtime::Runtime;

async fn create_test_nodes(count: usize) -> Vec<CodeNode> {
    let mut nodes = Vec::new();
    for i in 0..count {
        let location = Location {
            file_path: format!("test{}.rs", i),
            line: 1,
            column: 1,
            end_line: Some(10),
            end_column: Some(20),
        };

        let node = CodeNode::new(
            format!("test_function_{}", i),
            NodeType::Function,
            Language::Rust,
            location,
        );
        nodes.push(node);
    }
    nodes
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

fn bench_graph_operations(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("graph_operations");
    group.measurement_time(Duration::from_secs(10));
    
    for size in [100, 1000, 5000].iter() {
        group.bench_with_input(
            BenchmarkId::new("add_nodes", size),
            size,
            |b, &size| {
                b.to_async(&rt).iter(|| async {
                    let mut graph = CodeGraph::new();
                    let nodes = create_test_nodes(size).await;
                    benchmark_add_nodes(black_box(&mut graph), black_box(&nodes)).await
                });
            },
        );
        
        group.bench_with_input(
            BenchmarkId::new("get_nodes", size),
            size,
            |b, &size| {
                b.to_async(&rt).iter_with_setup(
                    || async {
                        let mut graph = CodeGraph::new();
                        let nodes = create_test_nodes(size).await;
                        let mut node_ids = Vec::new();
                        
                        for node in &nodes {
                            let _ = graph.add_node(node.clone()).await;
                            node_ids.push(node.id);
                        }
                        
                        (graph, node_ids)
                    },
                    |(graph, node_ids)| async move {
                        benchmark_get_nodes(black_box(&graph), black_box(&node_ids)).await
                    },
                );
            },
        );
    }
    
    group.finish();
}

criterion_group!(benches, bench_graph_operations);
criterion_main!(benches);