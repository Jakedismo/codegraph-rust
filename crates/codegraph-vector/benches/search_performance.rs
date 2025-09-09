use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use codegraph_vector::{
    IndexConfig, IndexType, FaissIndexManager, OptimizedSearchEngine, SearchConfig, BatchProcessor, BatchConfig
};
use faiss::MetricType;
use std::time::Duration;

/// Generate random vectors for benchmarking
fn generate_random_vectors(count: usize, dimension: usize) -> Vec<Vec<f32>> {
    use fastrand::Rng;
    let mut rng = Rng::new();
    
    (0..count)
        .map(|_| {
            (0..dimension)
                .map(|_| rng.f32() - 0.5)
                .collect()
        })
        .collect()
}

/// Benchmark different FAISS index types for search performance
fn bench_index_types(c: &mut Criterion) {
    let dimension = 768;
    let num_vectors = 10000;
    let num_queries = 100;
    let k = 10;
    
    let vectors = generate_random_vectors(num_vectors, dimension);
    let queries = generate_random_vectors(num_queries, dimension);
    
    let index_types = vec![
        ("Flat", IndexType::Flat),
        ("IVF", IndexType::IVF { nlist: 100, nprobe: 10 }),
        ("HNSW", IndexType::HNSW { m: 16, ef_construction: 200, ef_search: 50 }),
        ("LSH", IndexType::LSH { nbits: 1024 }),
    ];
    
    let mut group = c.benchmark_group("index_types");
    group.measurement_time(Duration::from_secs(10));
    
    for (name, index_type) in index_types {
        group.bench_with_input(BenchmarkId::new("search", name), &index_type, |b, index_type| {
            let rt = tokio::runtime::Runtime::new().unwrap();
            
            b.to_async(&rt).iter(|| async {
                let config = IndexConfig {
                    index_type: index_type.clone(),
                    metric_type: MetricType::InnerProduct,
                    dimension,
                    training_size_threshold: 5000,
                    gpu_enabled: false,
                    compression_level: 0,
                };
                
                let mut index_manager = FaissIndexManager::new(config);
                index_manager.create_index(num_vectors).unwrap();
                
                // Add vectors
                let flat_vectors: Vec<f32> = vectors.iter().flat_map(|v| v.iter().cloned()).collect();
                index_manager.add_vectors(&flat_vectors).unwrap();
                
                // Perform searches
                for query in &queries {
                    let _results = black_box(index_manager.search(query, k).unwrap());
                }
            });
        });
    }
    
    group.finish();
}

/// Benchmark search latency for sub-millisecond performance
fn bench_search_latency(c: &mut Criterion) {
    let dimension = 768;
    let num_vectors = 50000;
    let k = 10;
    
    let vectors = generate_random_vectors(num_vectors, dimension);
    let query = generate_random_vectors(1, dimension)[0].clone();
    
    let rt = tokio::runtime::Runtime::new().unwrap();
    
    // Setup HNSW index optimized for speed
    let config = IndexConfig::fast_search(dimension);
    let mut index_manager = FaissIndexManager::new(config);
    
    rt.block_on(async {
        index_manager.create_index(num_vectors).unwrap();
        let flat_vectors: Vec<f32> = vectors.iter().flat_map(|v| v.iter().cloned()).collect();
        index_manager.add_vectors(&flat_vectors).unwrap();
    });
    
    c.bench_function("single_search_latency", |b| {
        b.to_async(&rt).iter(|| async {
            let _results = black_box(index_manager.search(&query, k).unwrap());
        });
    });
}

/// Benchmark optimized search engine with caching
fn bench_optimized_search(c: &mut Criterion) {
    let dimension = 768;
    let num_vectors = 20000;
    let num_queries = 1000;
    let k = 10;
    
    let vectors = generate_random_vectors(num_vectors, dimension);
    let queries = generate_random_vectors(num_queries, dimension);
    
    let rt = tokio::runtime::Runtime::new().unwrap();
    
    let search_config = SearchConfig {
        target_latency_us: 500,
        cache_enabled: true,
        cache_max_entries: 1000,
        cache_ttl_seconds: 60,
        prefetch_enabled: true,
        prefetch_multiplier: 1.5,
        parallel_search: true,
        memory_pool_size_mb: 128,
    };
    
    let index_config = IndexConfig::fast_search(dimension);
    let mut search_engine = OptimizedSearchEngine::new(search_config, index_config).unwrap();
    
    // Setup index
    rt.block_on(async {
        let mut index_manager = search_engine.index_manager.write();
        index_manager.create_index(num_vectors).unwrap();
        let flat_vectors: Vec<f32> = vectors.iter().flat_map(|v| v.iter().cloned()).collect();
        index_manager.add_vectors(&flat_vectors).unwrap();
    });
    
    c.bench_function("optimized_search_cold", |b| {
        b.to_async(&rt).iter(|| async {
            let query_refs: Vec<&[f32]> = queries.iter().map(|v| v.as_slice()).collect();
            let _results = black_box(search_engine.batch_search_knn(&query_refs[0..10], k).await.unwrap());
        });
    });
    
    // Warm up cache
    rt.block_on(async {
        let query_refs: Vec<&[f32]> = queries.iter().map(|v| v.as_slice()).collect();
        let _ = search_engine.batch_search_knn(&query_refs[0..100], k).await.unwrap();
    });
    
    c.bench_function("optimized_search_warm", |b| {
        b.to_async(&rt).iter(|| async {
            let query_refs: Vec<&[f32]> = queries[0..10].iter().map(|v| v.as_slice()).collect();
            let _results = black_box(search_engine.batch_search_knn(&query_refs, k).await.unwrap());
        });
    });
}

/// Benchmark batch operations
fn bench_batch_operations(c: &mut Criterion) {
    let dimension = 768;
    let batch_sizes = vec![100, 500, 1000, 2000];
    
    let rt = tokio::runtime::Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("batch_operations");
    group.measurement_time(Duration::from_secs(15));
    
    for batch_size in batch_sizes {
        group.bench_with_input(
            BenchmarkId::new("insert", batch_size), 
            &batch_size, 
            |b, &batch_size| {
                b.to_async(&rt).iter(|| async {
                    let batch_config = BatchConfig {
                        batch_size,
                        max_pending_batches: 5,
                        flush_interval: Duration::from_secs(1),
                        parallel_processing: true,
                        memory_limit_mb: 512,
                        auto_train_threshold: batch_size * 2,
                    };
                    
                    let index_config = IndexConfig::balanced(dimension);
                    let mut processor = BatchProcessor::new(batch_config, index_config, None).unwrap();
                    processor.start_processing().await.unwrap();
                    
                    let vectors = generate_random_vectors(batch_size, dimension);
                    
                    for (i, vector) in vectors.into_iter().enumerate() {
                        use codegraph_vector::BatchOperation;
                        use codegraph_core::NodeId;
                        
                        let node_id = NodeId::from_bytes([
                            i as u8, (i >> 8) as u8, (i >> 16) as u8, (i >> 24) as u8,
                            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                        ]);
                        
                        let operation = BatchOperation::Insert { node_id, embedding: vector };
                        black_box(processor.enqueue_operation(operation).await.unwrap());
                    }
                    
                    processor.stop_processing().await.unwrap();
                });
            }
        );
    }
    
    group.finish();
}

/// Benchmark memory usage and efficiency
fn bench_memory_efficiency(c: &mut Criterion) {
    let dimension = 768;
    let vector_counts = vec![1000, 5000, 10000, 50000];
    
    let rt = tokio::runtime::Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("memory_efficiency");
    group.measurement_time(Duration::from_secs(20));
    
    for num_vectors in vector_counts {
        group.bench_with_input(
            BenchmarkId::new("build_index", num_vectors),
            &num_vectors,
            |b, &num_vectors| {
                b.to_async(&rt).iter(|| async {
                    let vectors = generate_random_vectors(num_vectors, dimension);
                    
                    // Test different index types for memory efficiency
                    let configs = vec![
                        IndexConfig::balanced(dimension),
                        IndexConfig::memory_efficient(dimension),
                    ];
                    
                    for config in configs {
                        let mut index_manager = FaissIndexManager::new(config);
                        index_manager.create_index(num_vectors).unwrap();
                        
                        let flat_vectors: Vec<f32> = vectors.iter().flat_map(|v| v.iter().cloned()).collect();
                        black_box(index_manager.add_vectors(&flat_vectors).unwrap());
                        
                        let _stats = black_box(index_manager.get_stats().unwrap());
                    }
                });
            }
        );
    }
    
    group.finish();
}

/// Benchmark parallel vs sequential processing
fn bench_parallel_vs_sequential(c: &mut Criterion) {
    let dimension = 768;
    let num_vectors = 10000;
    let num_queries = 100;
    let k = 20;
    
    let vectors = generate_random_vectors(num_vectors, dimension);
    let queries = generate_random_vectors(num_queries, dimension);
    
    let rt = tokio::runtime::Runtime::new().unwrap();
    
    // Setup search engines
    let search_config_parallel = SearchConfig {
        parallel_search: true,
        ..SearchConfig::default()
    };
    
    let search_config_sequential = SearchConfig {
        parallel_search: false,
        ..SearchConfig::default()
    };
    
    let index_config = IndexConfig::fast_search(dimension);
    
    let setup_engine = |parallel: bool| {
        let config = if parallel { search_config_parallel.clone() } else { search_config_sequential.clone() };
        let mut engine = OptimizedSearchEngine::new(config, index_config.clone()).unwrap();
        
        rt.block_on(async {
            let mut index_manager = engine.index_manager.write();
            index_manager.create_index(num_vectors).unwrap();
            let flat_vectors: Vec<f32> = vectors.iter().flat_map(|v| v.iter().cloned()).collect();
            index_manager.add_vectors(&flat_vectors).unwrap();
        });
        
        engine
    };
    
    let mut group = c.benchmark_group("parallel_vs_sequential");
    
    let engine_parallel = setup_engine(true);
    group.bench_function("parallel_search", |b| {
        b.to_async(&rt).iter(|| async {
            let query_refs: Vec<&[f32]> = queries[0..20].iter().map(|v| v.as_slice()).collect();
            let _results = black_box(engine_parallel.batch_search_knn(&query_refs, k).await.unwrap());
        });
    });
    
    let engine_sequential = setup_engine(false);
    group.bench_function("sequential_search", |b| {
        b.to_async(&rt).iter(|| async {
            let query_refs: Vec<&[f32]> = queries[0..20].iter().map(|v| v.as_slice()).collect();
            let _results = black_box(engine_sequential.batch_search_knn(&query_refs, k).await.unwrap());
        });
    });
    
    group.finish();
}

criterion_group!(
    benches,
    bench_index_types,
    bench_search_latency,
    bench_optimized_search,
    bench_batch_operations,
    bench_memory_efficiency,
    bench_parallel_vs_sequential
);

criterion_main!(benches);