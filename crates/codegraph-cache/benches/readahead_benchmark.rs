use codegraph_cache::{ReadAheadConfig, ReadAheadIntegration, ReadAheadOptimizer};
use codegraph_core::{CacheType, CompactCacheKey};
use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::time::Duration;
use tokio::runtime::Runtime;

fn bench_sequential_access(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let integration = ReadAheadIntegration::new();

    let mut group = c.benchmark_group("sequential_access");

    for size in [10, 50, 100, 500].iter() {
        group.throughput(Throughput::Elements(*size as u64));
        group.bench_with_input(
            BenchmarkId::new("with_readahead", size),
            size,
            |b, &size| {
                b.to_async(&rt).iter(|| async {
                    let base_hash = fastrand::u64(1000..100000);
                    for i in 0..size {
                        let key = CompactCacheKey {
                            hash: base_hash + i as u64,
                            cache_type: CacheType::Node,
                        };
                        let _ = integration.get_data(key).await;
                    }
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("without_readahead", size),
            size,
            |b, &size| {
                b.to_async(&rt).iter(|| async {
                    let base_hash = fastrand::u64(1000..100000);
                    for i in 0..size {
                        let key = CompactCacheKey {
                            hash: base_hash + i as u64,
                            cache_type: CacheType::Node,
                        };
                        // Simulate direct access without optimization
                        tokio::time::sleep(Duration::from_micros(100)).await;
                        let _data = format!("data_{}", key.hash).into_bytes();
                    }
                });
            },
        );
    }
    group.finish();
}

fn bench_predictive_loading(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let mut group = c.benchmark_group("predictive_loading");

    group.bench_function("pattern_training", |b| {
        b.to_async(&rt).iter(|| async {
            let integration = ReadAheadIntegration::new();

            // Train with common pattern: Node -> Embedding -> Query
            for i in 0..10 {
                let base_hash = 1000 + (i * 3);

                let node_key = CompactCacheKey {
                    hash: base_hash,
                    cache_type: CacheType::Node,
                };
                let embedding_key = CompactCacheKey {
                    hash: base_hash + 1,
                    cache_type: CacheType::Embedding,
                };
                let query_key = CompactCacheKey {
                    hash: base_hash + 2,
                    cache_type: CacheType::Query,
                };

                let _ = integration.get_data(node_key).await;
                let _ = integration.get_data(embedding_key).await;
                let _ = integration.get_data(query_key).await;
            }
        });
    });

    group.bench_function("pattern_prediction", |b| {
        b.to_async(&rt).iter(|| async {
            let integration = ReadAheadIntegration::new();

            // First, train the pattern
            for i in 0..5 {
                let base_hash = 2000 + (i * 3);
                let node_key = CompactCacheKey {
                    hash: base_hash,
                    cache_type: CacheType::Node,
                };
                let embedding_key = CompactCacheKey {
                    hash: base_hash + 1,
                    cache_type: CacheType::Embedding,
                };
                let query_key = CompactCacheKey {
                    hash: base_hash + 2,
                    cache_type: CacheType::Query,
                };

                let _ = integration.get_data(node_key).await;
                let _ = integration.get_data(embedding_key).await;
                let _ = integration.get_data(query_key).await;
            }

            // Now test prediction
            let test_key = CompactCacheKey {
                hash: 5000,
                cache_type: CacheType::Node,
            };
            let _ = integration.get_data(test_key).await;

            // These should benefit from prediction
            let predicted_embedding = CompactCacheKey {
                hash: 5001,
                cache_type: CacheType::Embedding,
            };
            let predicted_query = CompactCacheKey {
                hash: 5002,
                cache_type: CacheType::Query,
            };

            let _ = integration.get_data(predicted_embedding).await;
            let _ = integration.get_data(predicted_query).await;
        });
    });

    group.finish();
}

fn bench_cache_warming(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let mut group = c.benchmark_group("cache_warming");

    group.bench_function("hot_data_access", |b| {
        b.to_async(&rt).iter(|| async {
            let integration = ReadAheadIntegration::new();

            // Define hot keys
            let hot_keys = vec![
                CompactCacheKey {
                    hash: 100,
                    cache_type: CacheType::Node,
                },
                CompactCacheKey {
                    hash: 101,
                    cache_type: CacheType::Embedding,
                },
                CompactCacheKey {
                    hash: 102,
                    cache_type: CacheType::Query,
                },
            ];

            // Simulate repeated access to establish patterns
            for _ in 0..5 {
                for &key in &hot_keys {
                    let _ = integration.get_data(key).await;
                }
            }

            // Now measure performance of accessing hot data
            for &key in &hot_keys {
                let _ = integration.get_data(key).await;
            }
        });
    });

    group.finish();
}

fn bench_access_pattern_analysis(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let mut group = c.benchmark_group("pattern_analysis");

    for pattern_size in [10, 50, 100].iter() {
        group.bench_with_input(
            BenchmarkId::new("analyze_patterns", pattern_size),
            pattern_size,
            |b, &pattern_size| {
                b.to_async(&rt).iter(|| async {
                    let config = ReadAheadConfig::default();
                    let optimizer = ReadAheadOptimizer::new(config);

                    // Generate access pattern
                    for i in 0..pattern_size {
                        let key = CompactCacheKey {
                            hash: i as u64,
                            cache_type: CacheType::Node,
                        };
                        let _ = optimizer.optimize_read(key).await;
                    }
                });
            },
        );
    }

    group.finish();
}

fn bench_memory_efficiency(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let mut group = c.benchmark_group("memory_efficiency");

    group.bench_function("compact_cache_key_creation", |b| {
        b.iter(|| {
            for i in 0..1000 {
                let _key = CompactCacheKey {
                    hash: i,
                    cache_type: CacheType::Node,
                };
            }
        });
    });

    group.bench_function("optimizer_memory_footprint", |b| {
        b.to_async(&rt).iter(|| async {
            let config = ReadAheadConfig {
                max_pattern_history: 1000,
                prediction_window_size: 20,
                sequential_threshold: 3,
                cache_warming_interval: Duration::from_secs(60),
                prefetch_depth: 10,
                pattern_decay_factor: 0.9,
                min_confidence_threshold: 0.7,
                adaptive_learning_rate: 0.1,
            };

            let optimizer = ReadAheadOptimizer::new(config);

            // Exercise the optimizer to measure memory usage
            for i in 0..100 {
                let key = CompactCacheKey {
                    hash: i,
                    cache_type: CacheType::Node,
                };
                let _ = optimizer.optimize_read(key).await;
            }

            let _metrics = optimizer.get_metrics().await;
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_sequential_access,
    bench_predictive_loading,
    bench_cache_warming,
    bench_access_pattern_analysis,
    bench_memory_efficiency
);
criterion_main!(benches);
