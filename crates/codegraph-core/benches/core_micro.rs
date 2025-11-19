use codegraph_core::{CodeNode, Language, Location, NodeType};
use criterion::{
    black_box, criterion_group, criterion_main, Bencher, BenchmarkId, Criterion, Throughput,
};
use serde_json;

fn gen_node(i: usize) -> CodeNode {
    CodeNode::new(
        format!("node_{}", i),
        Some(NodeType::Function),
        Some(Language::Rust),
        Location {
            file_path: format!("/tmp/file_{}.rs", i % 10),
            line: 1,
            column: 1,
            end_line: Some(10),
            end_column: Some(1),
        },
    )
    .with_content("fn x() {}".to_string())
}

fn bench_node_creation(c: &mut Criterion) {
    let mut group = c.benchmark_group("core_node_creation");
    for &n in &[1_000usize, 10_000] {
        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(
            BenchmarkId::new("create_nodes", n),
            &n,
            |b: &mut Bencher, &n| {
                b.iter(|| {
                    let nodes: Vec<_> = (0..n).map(gen_node).collect();
                    black_box(nodes)
                })
            },
        );
    }
    group.finish();
}

fn bench_json_serde(c: &mut Criterion) {
    let mut group = c.benchmark_group("core_json_serde");
    for &n in &[100usize, 1_000] {
        let dataset: Vec<_> = (0..n).map(gen_node).collect();
        let encoded = serde_json::to_vec(&dataset).unwrap();

        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(
            BenchmarkId::new("serialize_vec", n),
            &n,
            |b: &mut Bencher, _| {
                b.iter(|| {
                    let bytes = serde_json::to_vec(black_box(&dataset)).unwrap();
                    black_box(bytes)
                })
            },
        );

        group.bench_with_input(
            BenchmarkId::new("deserialize_vec", n),
            &n,
            |b: &mut Bencher, _| {
                b.iter(|| {
                    let v: Vec<CodeNode> = serde_json::from_slice(black_box(&encoded)).unwrap();
                    black_box(v)
                })
            },
        );
    }
    group.finish();
}

criterion_group!(benches, bench_node_creation, bench_json_serde);
criterion_main!(benches);
