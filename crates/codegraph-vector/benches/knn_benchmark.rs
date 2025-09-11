use codegraph_core::{CodeNode, Language, Location, NodeType};
use codegraph_vector::{OptimizedKnnEngine, SearchConfig};
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::time::Duration;
use tokio::runtime::Runtime;

// Helper function to create test nodes
fn create_test_node(
    name: &str,
    node_type: NodeType,
    language: Language,
    embedding: Vec<f32>,
) -> CodeNode {
    CodeNode::new(
        name.to_string(),
        Some(node_type),
        Some(language),
        Location {
            file_path: "test.rs".to_string(),
            line: 1,
            column: 1,
            end_line: None,
            end_column: None,
        },
    )
    .with_embedding(embedding)
    .with_complexity(0.5)
}

// Generate normalized random embedding
fn generate_random_embedding(dimension: usize, seed: u64) -> Vec<f32> {
    let mut embedding = vec![0.0; dimension];
    let mut state = seed;

    for i in 0..dimension {
        state = state.wrapping_mul(1103515245).wrapping_add(12345);
        embedding[i] = ((state as f32 / u32::MAX as f32) - 0.5) * 2.0;
    }

    // Normalize
    let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 0.0 {
        for x in &mut embedding {
            *x /= norm;
        }
    }

    embedding
}

// Create dataset for benchmarking
fn create_dataset(size: usize, dimension: usize) -> Vec<CodeNode> {
    let mut nodes = Vec::with_capacity(size);
    let languages = [
        Language::Rust,
        Language::Python,
        Language::JavaScript,
        Language::Go,
    ];
    let node_types = [
        NodeType::Function,
        NodeType::Struct,
        NodeType::Class,
        NodeType::Module,
    ];

    for i in 0..size {
        let language = languages[i % languages.len()].clone();
        let node_type = node_types[i % node_types.len()].clone();

        nodes.push(create_test_node(
            &format!("node_{}", i),
            node_type,
            language,
            generate_random_embedding(dimension, i as u64),
        ));
    }

    nodes
}

fn benchmark_single_search(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let dimension = 384; // Common embedding dimension

    let dataset_sizes = [100, 500, 1000, 5000, 10000];

    for &size in &dataset_sizes {
        let config = SearchConfig::default();
        let engine = rt.block_on(async {
            let engine = OptimizedKnnEngine::new(dimension, config).unwrap();
            let dataset = create_dataset(size, dimension);
            engine.build_indices(&dataset).await.unwrap();
            engine
        });

        let query_embedding = generate_random_embedding(dimension, 999);

        c.bench_with_input(BenchmarkId::new("single_search", size), &size, |b, _| {
            b.to_async(&rt).iter(|| async {
                let result = engine
                    .single_similarity_search(
                        black_box(query_embedding.clone()),
                        black_box(SearchConfig::default()),
                    )
                    .await
                    .unwrap();
                black_box(result);
            });
        });
    }
}

fn benchmark_parallel_search(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let dimension = 384;
    let dataset_size = 5000;

    let parallelism_levels = [1, 2, 4, 8, 16];

    for &max_parallel in &parallelism_levels {
        let config = SearchConfig {
            max_parallel_queries: max_parallel,
            ..SearchConfig::default()
        };

        let engine = rt.block_on(async {
            let engine = OptimizedKnnEngine::new(dimension, config).unwrap();
            let dataset = create_dataset(dataset_size, dimension);
            engine.build_indices(&dataset).await.unwrap();
            engine
        });

        // Create multiple queries
        let queries: Vec<_> = (0..max_parallel)
            .map(|i| generate_random_embedding(dimension, 1000 + i as u64))
            .collect();

        c.bench_with_input(
            BenchmarkId::new("parallel_search", max_parallel),
            &max_parallel,
            |b, _| {
                b.to_async(&rt).iter(|| async {
                    let result = engine
                        .parallel_similarity_search(black_box(queries.clone()), None)
                        .await
                        .unwrap();
                    black_box(result);
                });
            },
        );
    }
}

fn benchmark_precision_recall_tradeoff(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let dimension = 384;
    let dataset_size = 10000;

    let precision_levels = [0.0, 0.3, 0.5, 0.7, 0.9, 1.0];

    for &precision in &precision_levels {
        let config = SearchConfig {
            precision_recall_tradeoff: precision,
            ..SearchConfig::default()
        };

        let engine = rt.block_on(async {
            let engine = OptimizedKnnEngine::new(dimension, config).unwrap();
            let dataset = create_dataset(dataset_size, dimension);
            engine.build_indices(&dataset).await.unwrap();
            engine
        });

        let query_embedding = generate_random_embedding(dimension, 999);

        c.bench_with_input(
            BenchmarkId::new("precision_recall", (precision * 100.0) as i32),
            &precision,
            |b, _| {
                b.to_async(&rt).iter(|| async {
                    let result = engine
                        .single_similarity_search(
                            black_box(query_embedding.clone()),
                            black_box(SearchConfig {
                                precision_recall_tradeoff: precision,
                                ..SearchConfig::default()
                            }),
                        )
                        .await
                        .unwrap();
                    black_box(result);
                });
            },
        );
    }
}

fn benchmark_clustering_impact(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let dimension = 384;
    let dataset_size = 5000;

    for clustering_enabled in [false, true] {
        let config = SearchConfig {
            enable_clustering: clustering_enabled,
            ..SearchConfig::default()
        };

        let engine = rt.block_on(async {
            let engine = OptimizedKnnEngine::new(dimension, config).unwrap();
            let dataset = create_dataset(dataset_size, dimension);
            engine.build_indices(&dataset).await.unwrap();
            engine
        });

        let query_embedding = generate_random_embedding(dimension, 999);
        let bench_name = if clustering_enabled {
            "with_clustering"
        } else {
            "without_clustering"
        };

        c.bench_function(bench_name, |b| {
            b.to_async(&rt).iter(|| async {
                let result = engine
                    .single_similarity_search(
                        black_box(query_embedding.clone()),
                        black_box(SearchConfig {
                            enable_clustering: clustering_enabled,
                            ..SearchConfig::default()
                        }),
                    )
                    .await
                    .unwrap();
                black_box(result);
            });
        });
    }
}

fn benchmark_cache_performance(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let dimension = 384;
    let dataset_size = 2000;

    let engine = rt.block_on(async {
        let engine = OptimizedKnnEngine::new(dimension, SearchConfig::default()).unwrap();
        let dataset = create_dataset(dataset_size, dimension);
        engine.build_indices(&dataset).await.unwrap();
        engine
    });

    let query_embeddings: Vec<_> = (0..100)
        .map(|i| generate_random_embedding(dimension, 2000 + i as u64))
        .collect();

    // Warm up cache with some queries
    rt.block_on(async {
        for (i, embedding) in query_embeddings.iter().enumerate().take(50) {
            if i % 10 == 0 {
                let _ = engine
                    .single_similarity_search(embedding.clone(), SearchConfig::default())
                    .await
                    .unwrap();
            }
        }
    });

    c.bench_function("cache_hit_rate", |b| {
        let mut query_index = 0;
        b.to_async(&rt).iter(|| async {
            // Mix of cache hits and misses
            let embedding = if query_index % 2 == 0 {
                // Cache hit (repeat previous queries)
                query_embeddings[query_index % 50].clone()
            } else {
                // Cache miss (new queries)
                query_embeddings[50 + (query_index % 50)].clone()
            };

            query_index += 1;

            let result = engine
                .single_similarity_search(black_box(embedding), black_box(SearchConfig::default()))
                .await
                .unwrap();
            black_box(result);
        });
    });
}

fn benchmark_batch_operations(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let dimension = 384;
    let dataset_size = 5000;

    let engine = rt.block_on(async {
        let engine = OptimizedKnnEngine::new(dimension, SearchConfig::default()).unwrap();
        let dataset = create_dataset(dataset_size, dimension);
        engine.build_indices(&dataset).await.unwrap();
        engine
    });

    let batch_sizes = [10, 50, 100];

    for &batch_size in &batch_sizes {
        let function_nodes: Vec<_> = (0..batch_size)
            .map(|i| {
                create_test_node(
                    &format!("func_{}", i),
                    NodeType::Function,
                    Language::Rust,
                    generate_random_embedding(dimension, 3000 + i as u64),
                )
            })
            .collect();

        c.bench_with_input(
            BenchmarkId::new("batch_function_search", batch_size),
            &batch_size,
            |b, _| {
                b.to_async(&rt).iter(|| async {
                    let result = engine
                        .batch_search_similar_functions(black_box(&function_nodes), None)
                        .await
                        .unwrap();
                    black_box(result);
                });
            },
        );
    }
}

fn benchmark_index_building(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let dimension = 384;

    let dataset_sizes = [500, 1000, 2000, 5000];

    for &size in &dataset_sizes {
        let dataset = create_dataset(size, dimension);

        c.bench_with_input(BenchmarkId::new("index_building", size), &size, |b, _| {
            b.to_async(&rt).iter(|| async {
                let engine = OptimizedKnnEngine::new(dimension, SearchConfig::default()).unwrap();
                engine.build_indices(black_box(&dataset)).await.unwrap();
                black_box(engine);
            });
        });
    }
}

fn benchmark_memory_usage(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let dimension = 384;

    // Test different dataset sizes to measure memory scaling
    let dataset_sizes = [1000, 5000, 10000];

    for &size in &dataset_sizes {
        let dataset = create_dataset(size, dimension);

        c.bench_with_input(BenchmarkId::new("memory_footprint", size), &size, |b, _| {
            b.to_async(&rt).iter(|| async {
                let engine = OptimizedKnnEngine::new(dimension, SearchConfig::default()).unwrap();
                engine.build_indices(black_box(&dataset)).await.unwrap();

                // Perform some searches to populate caches
                for i in 0..10 {
                    let query = generate_random_embedding(dimension, 4000 + i);
                    let _ = engine
                        .single_similarity_search(query, SearchConfig::default())
                        .await
                        .unwrap();
                }

                let stats = engine.get_performance_stats();
                black_box(stats);
            });
        });
    }
}

criterion_group!(
    benches,
    benchmark_single_search,
    benchmark_parallel_search,
    benchmark_precision_recall_tradeoff,
    benchmark_clustering_impact,
    benchmark_cache_performance,
    benchmark_batch_operations,
    benchmark_index_building,
    benchmark_memory_usage
);

criterion_main!(benches);
