use codegraph_core::*;
use codegraph_vector::*;
use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use std::time::Duration;
use tokio::runtime::Runtime;

async fn create_test_nodes_with_embeddings(count: usize) -> Vec<CodeNode> {
    let embedding_generator = EmbeddingGenerator::default();
    let mut nodes = Vec::new();
    
    for i in 0..count {
        let location = Location {
            file_path: format!("test{}.rs", i),
            line: 1,
            column: 1,
            end_line: Some(10),
            end_column: Some(20),
        };

        let mut node = CodeNode::new(
            format!("test_function_{}", i),
            NodeType::Function,
            Language::Rust,
            location,
        ).with_content(format!("fn test_function_{}() {{ println!(\"test\"); }}", i));

        let embedding = embedding_generator.generate_embedding(&node).await.unwrap();
        node = node.with_embedding(embedding);
        nodes.push(node);
    }
    nodes
}

async fn benchmark_build_index(vector_store: &mut FaissVectorStore, nodes: &[CodeNode]) {
    let _ = vector_store.build_index(nodes).await;
}

async fn benchmark_search(search: &SemanticSearch, query: &str, limit: usize) {
    let _ = search.search_by_text(query, limit).await;
}

fn bench_vector_operations(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("vector_operations");
    group.measurement_time(Duration::from_secs(15));
    
    for size in [100, 1000, 2000].iter() {
        group.bench_with_input(
            BenchmarkId::new("build_index", size),
            size,
            |b, &size| {
                b.to_async(&rt).iter(|| async {
                    let mut vector_store = FaissVectorStore::new(384).unwrap();
                    let nodes = create_test_nodes_with_embeddings(size).await;
                    benchmark_build_index(black_box(&mut vector_store), black_box(&nodes)).await
                });
            },
        );
        
        group.bench_with_input(
            BenchmarkId::new("search", size),
            size,
            |b, &size| {
                b.to_async(&rt).iter_with_setup(
                    || async {
                        let mut vector_store = FaissVectorStore::new(384).unwrap();
                        let embedding_generator = EmbeddingGenerator::default();
                        let nodes = create_test_nodes_with_embeddings(size).await;
                        
                        vector_store.build_index(&nodes).await.unwrap();
                        
                        let search = SemanticSearch::new(
                            std::sync::Arc::new(vector_store),
                            std::sync::Arc::new(embedding_generator),
                        );
                        
                        search
                    },
                    |search| async move {
                        benchmark_search(black_box(&search), black_box("test function"), black_box(10)).await
                    },
                );
            },
        );
    }
    
    group.finish();
}

criterion_group!(benches, bench_vector_operations);
criterion_main!(benches);