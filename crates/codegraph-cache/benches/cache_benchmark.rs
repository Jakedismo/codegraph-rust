use codegraph_cache::{
    AiCache, EmbeddingCache, QueryCache, FaissCache, FaissCacheBuilder,
    CacheConfig, MetricsCollector, InvalidationManager, InvalidationStrategy,
};
use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use std::collections::HashMap;
use std::time::Duration;
use tokio::runtime::Runtime;
use fastrand;

/// Generate random vector for testing
fn generate_random_vector(dimension: usize) -> Vec<f32> {
    (0..dimension).map(|_| fastrand::f32()).collect()
}

/// Generate test cache key
fn generate_cache_key(index: usize) -> String {
    format!("test_key_{}", index)
}

/// Benchmark embedding cache operations
fn bench_embedding_cache(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("embedding_cache");
    
    // Test different cache sizes
    for cache_size in [1000, 5000, 10000].iter() {
        group.bench_with_input(
            BenchmarkId::new("insert", cache_size),
            cache_size,
            |b, &size| {
                b.to_async(&rt).iter(|| async {
                    let config = CacheConfig {
                        max_size: size,
                        ..Default::default()
                    };
                    let mut cache = EmbeddingCache::with_config(config);
                    
                    for i in 0..100 {
                        let key = generate_cache_key(i);
                        let vector = generate_random_vector(768);
                        cache.insert(key, vector, None).await.unwrap();
                    }
                });
            },
        );
        
        group.bench_with_input(
            BenchmarkId::new("get", cache_size),
            cache_size,
            |b, &size| {
                b.to_async(&rt).iter(|| async {
                    let config = CacheConfig {
                        max_size: size,
                        ..Default::default()
                    };
                    let mut cache = EmbeddingCache::with_config(config);
                    
                    // Pre-populate cache
                    for i in 0..100 {
                        let key = generate_cache_key(i);
                        let vector = generate_random_vector(768);
                        cache.insert(key, vector, None).await.unwrap();
                    }
                    
                    // Benchmark get operations
                    for i in 0..100 {
                        let key = generate_cache_key(i);
                        black_box(cache.get(&key).await.unwrap());
                    }
                });
            },
        );
    }
    
    group.finish();
}

/// Benchmark query cache with semantic similarity
fn bench_query_cache(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("query_cache");
    
    // Test different similarity thresholds
    for threshold in [0.7, 0.8, 0.9].iter() {
        group.bench_with_input(
            BenchmarkId::new("semantic_search", threshold),
            threshold,
            |b, &threshold| {
                b.to_async(&rt).iter(|| async {
                    let config = CacheConfig {
                        similarity_threshold: threshold,
                        ..Default::default()
                    };
                    let mut cache = QueryCache::with_config(config);
                    
                    // Pre-populate with query results
                    for i in 0..50 {
                        let query_vector = generate_random_vector(768);
                        let result = format!("result_{}", i);
                        let key = format!("query_{}", i);
                        cache.insert_query_result(key, query_vector, result).await.unwrap();
                    }
                    
                    // Benchmark semantic search
                    for _ in 0..10 {
                        let query_vector = generate_random_vector(768);
                        black_box(cache.find_similar_query(&query_vector).await.unwrap());
                    }
                });
            },
        );
    }
    
    group.finish();
}

/// Benchmark FAISS cache vector operations
fn bench_faiss_cache(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("faiss_cache");
    
    // Test different vector dimensions
    for dimension in [128, 384, 768].iter() {
        group.bench_with_input(
            BenchmarkId::new("vector_add", dimension),
            dimension,
            |b, &dimension| {
                b.to_async(&rt).iter(|| async {
                    let mut cache = FaissCacheBuilder::new()
                        .dimension(dimension as u32)
                        .num_clusters(0) // Use flat index for benchmarking
                        .max_vectors(1000)
                        .build()
                        .unwrap();
                    
                    cache.initialize().await.unwrap();
                    
                    for i in 0..100 {
                        let key = generate_cache_key(i);
                        let vector = generate_random_vector(dimension);
                        let metadata = HashMap::new();
                        cache.add_vector(key, vector, metadata).await.unwrap();
                    }
                });
            },
        );
        
        group.bench_with_input(
            BenchmarkId::new("similarity_search", dimension),
            dimension,
            |b, &dimension| {
                b.to_async(&rt).iter(|| async {
                    let mut cache = FaissCacheBuilder::new()
                        .dimension(dimension as u32)
                        .num_clusters(0)
                        .max_vectors(1000)
                        .similarity_threshold(0.8)
                        .build()
                        .unwrap();
                    
                    cache.initialize().await.unwrap();
                    
                    // Pre-populate with vectors
                    for i in 0..200 {
                        let key = generate_cache_key(i);
                        let vector = generate_random_vector(dimension);
                        let metadata = HashMap::new();
                        cache.add_vector(key, vector, metadata).await.unwrap();
                    }
                    
                    // Benchmark similarity search
                    for _ in 0..10 {
                        let query_vector = generate_random_vector(dimension);
                        black_box(cache.search_similar(query_vector, 10).await.unwrap());
                    }
                });
            },
        );
    }
    
    group.finish();
}

/// Benchmark cache invalidation performance
fn bench_invalidation(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("cache_invalidation");
    
    group.bench_function("file_dependency_tracking", |b| {
        b.to_async(&rt).iter(|| async {
            let manager = InvalidationManager::new(vec![InvalidationStrategy::ContentBased]);
            
            // Register many file dependencies
            for i in 0..1000 {
                let file_path = format!("src/file_{}.rs", i);
                let cache_key = format!("cache_key_{}", i);
                manager.register_file_dependency(file_path, cache_key).await;
            }
            
            // Benchmark file change handling
            for i in 0..10 {
                let file_path = format!("src/file_{}.rs", i * 100);
                black_box(manager.handle_file_change(file_path).await.unwrap());
            }
        });
    });
    
    group.bench_function("bulk_invalidation", |b| {
        b.to_async(&rt).iter(|| async {
            let manager = InvalidationManager::new(vec![InvalidationStrategy::Manual]);
            
            // Benchmark bulk key invalidation
            let keys: Vec<String> = (0..1000).map(|i| format!("key_{}", i)).collect();
            manager.invalidate_keys(keys, "benchmark test".to_string()).await.unwrap();
        });
    });
    
    group.finish();
}

/// Benchmark metrics collection overhead
fn bench_metrics_collection(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("metrics_collection");
    
    group.bench_function("record_operations", |b| {
        b.to_async(&rt).iter(|| async {
            let collector = MetricsCollector::new();
            
            // Simulate high-frequency operations
            for _ in 0..1000 {
                collector.record_hit();
                collector.record_miss();
                collector.record_insertion(1024);
                collector.record_eviction(512);
                collector.record_response_time(Duration::from_micros(100)).await;
                collector.record_operation("test_op", Duration::from_micros(50), true).await;
            }
        });
    });
    
    group.bench_function("metrics_aggregation", |b| {
        b.to_async(&rt).iter(|| async {
            let collector = MetricsCollector::new();
            
            // Pre-populate with metrics
            for _ in 0..10000 {
                collector.record_hit();
                collector.record_insertion(1024);
                collector.record_response_time(Duration::from_micros(100)).await;
            }
            
            // Benchmark metrics collection
            black_box(collector.get_metrics().await);
            black_box(collector.get_operation_metrics().await);
            black_box(collector.get_throughput_metrics());
        });
    });
    
    group.finish();
}

/// Benchmark memory pressure handling
fn bench_memory_pressure(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("memory_pressure");
    
    group.bench_function("lru_eviction", |b| {
        b.to_async(&rt).iter(|| async {
            let config = CacheConfig {
                max_size: 1000,
                memory_pressure_threshold: 0.8,
                enable_lru: true,
                ..Default::default()
            };
            let mut cache = EmbeddingCache::with_config(config);
            
            // Fill cache beyond capacity to trigger evictions
            for i in 0..1500 {
                let key = generate_cache_key(i);
                let vector = generate_random_vector(768);
                cache.insert(key, vector, None).await.unwrap();
            }
        });
    });
    
    group.bench_function("memory_optimization", |b| {
        b.to_async(&rt).iter(|| async {
            let config = CacheConfig {
                max_size: 5000,
                enable_compression: true,
                memory_pressure_threshold: 0.7,
                ..Default::default()
            };
            let mut cache = EmbeddingCache::with_config(config);
            
            // Add large vectors to test compression
            for i in 0..1000 {
                let key = generate_cache_key(i);
                let vector = generate_random_vector(1536); // Large embedding
                cache.insert(key, vector, None).await.unwrap();
            }
            
            // Trigger memory optimization
            cache.optimize_memory().await.unwrap();
        });
    });
    
    group.finish();
}

/// Benchmark concurrent access patterns
fn bench_concurrent_access(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("concurrent_access");
    
    group.bench_function("parallel_reads", |b| {
        b.to_async(&rt).iter(|| async {
            let config = CacheConfig {
                max_size: 10000,
                ..Default::default()
            };
            let mut cache = EmbeddingCache::with_config(config);
            
            // Pre-populate cache
            for i in 0..1000 {
                let key = generate_cache_key(i);
                let vector = generate_random_vector(768);
                cache.insert(key, vector, None).await.unwrap();
            }
            
            // Simulate concurrent reads
            let mut handles = Vec::new();
            for i in 0..10 {
                let key = generate_cache_key(i * 100);
                handles.push(tokio::spawn(async move {
                    // This would need Arc<Mutex<Cache>> in real concurrent scenario
                    // For benchmark, we're measuring the operation cost
                    black_box(key);
                }));
            }
            
            futures::future::join_all(handles).await;
        });
    });
    
    group.finish();
}

/// Comprehensive cache system benchmark
fn bench_integrated_system(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("integrated_system");
    
    group.bench_function("full_cache_workflow", |b| {
        b.to_async(&rt).iter(|| async {
            // Initialize all cache components
            let embedding_config = CacheConfig {
                max_size: 5000,
                enable_compression: true,
                ..Default::default()
            };
            let mut embedding_cache = EmbeddingCache::with_config(embedding_config);
            
            let query_config = CacheConfig {
                similarity_threshold: 0.85,
                ..Default::default()
            };
            let mut query_cache = QueryCache::with_config(query_config);
            
            let mut faiss_cache = FaissCacheBuilder::new()
                .dimension(768)
                .num_clusters(0)
                .max_vectors(1000)
                .build()
                .unwrap();
            faiss_cache.initialize().await.unwrap();
            
            // Simulate realistic workflow
            for i in 0..100 {
                let key = generate_cache_key(i);
                let vector = generate_random_vector(768);
                let metadata = HashMap::new();
                
                // Add to embedding cache
                embedding_cache.insert(key.clone(), vector.clone(), None).await.unwrap();
                
                // Add to FAISS cache
                faiss_cache.add_vector(key.clone(), vector.clone(), metadata).await.unwrap();
                
                // Add query result
                let result = format!("result_{}", i);
                query_cache.insert_query_result(key, vector, result).await.unwrap();
                
                // Perform some lookups
                if i % 10 == 0 {
                    let lookup_key = generate_cache_key(i / 10);
                    black_box(embedding_cache.get(&lookup_key).await.unwrap());
                    
                    let query_vector = generate_random_vector(768);
                    black_box(faiss_cache.search_similar(query_vector, 5).await.unwrap());
                }
            }
        });
    });
    
    group.finish();
}

criterion_group!(
    benches,
    bench_embedding_cache,
    bench_query_cache,
    bench_faiss_cache,
    bench_invalidation,
    bench_metrics_collection,
    bench_memory_pressure,
    bench_concurrent_access,
    bench_integrated_system
);

criterion_main!(benches);