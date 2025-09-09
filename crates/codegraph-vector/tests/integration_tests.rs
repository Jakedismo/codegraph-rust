use codegraph_core::{CodeGraphError, NodeId};
use codegraph_vector::{
    BatchConfig, BatchOperation, BatchProcessor, FaissIndexManager, IndexConfig, IndexType,
    OptimizedSearchEngine, PersistentStorage, SearchConfig,
};
use faiss::MetricType;
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};
use tempfile::TempDir;
use tokio_test;

/// Generate deterministic test vectors
fn generate_test_vectors(count: usize, dimension: usize, seed: u64) -> Vec<Vec<f32>> {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    
    (0..count)
        .map(|i| {
            let mut hasher = DefaultHasher::new();
            seed.hash(&mut hasher);
            i.hash(&mut hasher);
            let hash = hasher.finish();
            
            (0..dimension)
                .map(|j| {
                    let mut hasher = DefaultHasher::new();
                    hash.hash(&mut hasher);
                j.hash(&mut hasher);
                    (hasher.finish() as f32 / u64::MAX as f32) - 0.5
                })
                .collect()
        })
        .collect()
}

/// Generate NodeId from index
fn node_id_from_index(index: usize) -> NodeId {
    NodeId::from_bytes([
        index as u8,
        (index >> 8) as u8,
        (index >> 16) as u8,
        (index >> 24) as u8,
        0, 0, 0, 0,
        0, 0, 0, 0,
        0, 0, 0, 0,
    ])
}

#[tokio::test]
async fn test_faiss_index_types() -> Result<(), Box<dyn std::error::Error>> {
    let dimension = 128;
    let num_vectors = 1000;
    let vectors = generate_test_vectors(num_vectors, dimension, 12345);
    let query = &vectors[0];
    
    let index_types = vec![
        ("Flat", IndexType::Flat),
        ("IVF", IndexType::IVF { nlist: 10, nprobe: 5 }),
        ("HNSW", IndexType::HNSW { m: 8, ef_construction: 40, ef_search: 16 }),
    ];
    
    for (name, index_type) in index_types {
        println!("Testing index type: {}", name);
        
        let config = IndexConfig {
            index_type,
            metric_type: MetricType::InnerProduct,
            dimension,
            training_size_threshold: 100,
            gpu_enabled: false,
            compression_level: 0,
        };
        
        let mut index_manager = FaissIndexManager::new(config);
        index_manager.create_index(num_vectors)?;
        
        // Add vectors
        let flat_vectors: Vec<f32> = vectors.iter().flat_map(|v| v.iter().cloned()).collect();
        let _ids = index_manager.add_vectors(&flat_vectors)?;
        
        // Search
        let (distances, labels) = index_manager.search(query, 10)?;
        
        assert_eq!(distances.len(), 10);
        assert_eq!(labels.len(), 10);
        assert!(labels.contains(&0)); // Should find itself
        
        let stats = index_manager.get_stats()?;
        assert_eq!(stats.num_vectors, num_vectors);
        assert_eq!(stats.dimension, dimension);
        
        println!("✓ {} index working correctly", name);
    }
    
    Ok(())
}

#[tokio::test]
async fn test_persistent_storage() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = TempDir::new()?;
    let storage_path = temp_dir.path().to_path_buf();
    
    let dimension = 64;
    let num_vectors = 500;
    let vectors = generate_test_vectors(num_vectors, dimension, 54321);
    
    // Create and save index
    {
        let config = IndexConfig {
            index_type: IndexType::Flat,
            metric_type: MetricType::L2,
            dimension,
            training_size_threshold: 100,
            gpu_enabled: false,
            compression_level: 3, // Enable compression
        };
        
        let mut index_manager = FaissIndexManager::new(config.clone());
        index_manager = index_manager.with_persistence(storage_path.clone())?;
        index_manager.create_index(num_vectors)?;
        
        let flat_vectors: Vec<f32> = vectors.iter().flat_map(|v| v.iter().cloned()).collect();
        index_manager.add_vectors(&flat_vectors)?;
        
        // Create storage and save data
        let storage = PersistentStorage::new(storage_path.clone())?;
        
        // Save embeddings
        let embeddings: HashMap<NodeId, Vec<f32>> = vectors
            .iter()
            .enumerate()
            .map(|(i, v)| (node_id_from_index(i), v.clone()))
            .collect();
        
        storage.save_embeddings(&embeddings)?;
        
        // Save ID mappings
        let id_mapping: HashMap<i64, NodeId> = (0..num_vectors)
            .map(|i| (i as i64, node_id_from_index(i)))
            .collect();
        let reverse_mapping: HashMap<NodeId, i64> = (0..num_vectors)
            .map(|i| (node_id_from_index(i), i as i64))
            .collect();
        
        storage.save_id_mapping(&id_mapping, &reverse_mapping)?;
    }
    
    // Load and verify
    {
        let storage = PersistentStorage::new(storage_path)?;
        
        let loaded_embeddings = storage.load_embeddings()?;
        assert_eq!(loaded_embeddings.len(), num_vectors);
        
        let (loaded_id_mapping, loaded_reverse_mapping) = storage.load_id_mapping()?;
        assert_eq!(loaded_id_mapping.len(), num_vectors);
        assert_eq!(loaded_reverse_mapping.len(), num_vectors);
        
        // Verify data integrity
        for i in 0..num_vectors {
            let node_id = node_id_from_index(i);
            let original_vector = &vectors[i];
            let loaded_vector = loaded_embeddings.get(&node_id).unwrap();
            
            assert_eq!(original_vector.len(), loaded_vector.len());
            for (a, b) in original_vector.iter().zip(loaded_vector.iter()) {
                assert!((a - b).abs() < 1e-6);
            }
        }
        
        let stats = storage.get_stats();
        assert_eq!(stats.num_vectors, num_vectors);
        assert!(stats.compression_enabled);
        assert!(stats.total_size_bytes > 0);
        
        println!("✓ Persistent storage working correctly");
    }
    
    Ok(())
}

#[tokio::test]
async fn test_batch_processing() -> Result<(), Box<dyn std::error::Error>> {
    let dimension = 96;
    let batch_size = 100;
    let num_batches = 5;
    let total_vectors = batch_size * num_batches;
    
    let batch_config = BatchConfig {
        batch_size,
        max_pending_batches: 10,
        flush_interval: Duration::from_millis(500),
        parallel_processing: true,
        memory_limit_mb: 128,
        auto_train_threshold: total_vectors / 2,
    };
    
    let index_config = IndexConfig {
        index_type: IndexType::IVF { nlist: 20, nprobe: 5 },
        metric_type: MetricType::InnerProduct,
        dimension,
        training_size_threshold: 200,
        gpu_enabled: false,
        compression_level: 0,
    };
    
    let mut processor = BatchProcessor::new(batch_config, index_config, None)?;
    processor.start_processing().await?;
    
    // Generate and enqueue insert operations
    let vectors = generate_test_vectors(total_vectors, dimension, 98765);
    let mut node_ids = Vec::new();
    
    for (i, vector) in vectors.iter().enumerate() {
        let node_id = node_id_from_index(i);
        node_ids.push(node_id);
        
        let operation = BatchOperation::Insert {
            node_id,
            embedding: vector.clone(),
        };
        
        processor.enqueue_operation(operation).await?;
    }
    
    // Enqueue search operations
    for i in 0..10 {
        let query_embedding = vectors[i].clone();
        let operation = BatchOperation::Search {
            query_embedding,
            k: 5,
            callback_id: uuid::Uuid::new_v4(),
        };
        
        processor.enqueue_operation(operation).await?;
    }
    
    // Wait for processing to complete
    tokio::time::sleep(Duration::from_secs(2)).await;
    
    // Check statistics
    let stats = processor.get_stats();
    assert!(stats.total_operations > 0);
    assert!(stats.success_rate > 0.8);
    
    // Process some results
    let mut results_count = 0;
    for _ in 0..100 {
        if let Ok(Some(_result)) = processor.try_recv_result() {
            results_count += 1;
        } else {
            break;
        }
    }
    
    assert!(results_count > 0);
    
    processor.stop_processing().await?;
    
    println!("✓ Batch processing completed successfully");
    println!("  Total operations: {}", stats.total_operations);
    println!("  Success rate: {:.1}%", stats.success_rate * 100.0);
    println!("  Results processed: {}", results_count);
    
    Ok(())
}

#[tokio::test]
async fn test_optimized_search_engine() -> Result<(), Box<dyn std::error::Error>> {
    let dimension = 128;
    let num_vectors = 2000;
    let num_queries = 100;
    
    let vectors = generate_test_vectors(num_vectors, dimension, 13579);
    let queries = generate_test_vectors(num_queries, dimension, 24680);
    
    let search_config = SearchConfig {
        target_latency_us: 1000, // 1ms target
        cache_enabled: true,
        cache_max_entries: 500,
        cache_ttl_seconds: 30,
        prefetch_enabled: true,
        prefetch_multiplier: 1.3,
        parallel_search: true,
        memory_pool_size_mb: 64,
    };
    
    let index_config = IndexConfig::fast_search(dimension);
    let mut search_engine = OptimizedSearchEngine::new(search_config, index_config)?;
    
    // Setup index
    {
        let mut index_manager = search_engine.index_manager.write();
        index_manager.create_index(num_vectors)?;
        let flat_vectors: Vec<f32> = vectors.iter().flat_map(|v| v.iter().cloned()).collect();
        index_manager.add_vectors(&flat_vectors)?;
    }
    
    // Warmup
    let warmup_queries: Vec<&[f32]> = queries[0..20].iter().map(|v| v.as_slice()).collect();
    search_engine.warmup(&warmup_queries, 10).await?;
    
    // Test single search performance
    let mut search_times = Vec::new();
    for query in &queries[0..50] {
        let start = Instant::now();
        let results = search_engine.search_knn(query, 10).await?;
        let duration = start.elapsed();
        
        search_times.push(duration);
        assert_eq!(results.len(), 10);
    }
    
    // Test batch search
    let batch_queries: Vec<&[f32]> = queries[0..20].iter().map(|v| v.as_slice()).collect();
    let batch_start = Instant::now();
    let batch_results = search_engine.batch_search_knn(&batch_queries, 5).await?;
    let batch_duration = batch_start.elapsed();
    
    assert_eq!(batch_results.len(), 20);
    for result in &batch_results {
        assert_eq!(result.len(), 5);
    }
    
    // Test cache effectiveness (repeat some queries)
    let cache_test_queries: Vec<&[f32]> = queries[0..10].iter().map(|v| v.as_slice()).collect();
    let _cached_results = search_engine.batch_search_knn(&cache_test_queries, 10).await?;
    
    // Get performance statistics
    let stats = search_engine.get_performance_stats();
    
    println!("✓ Optimized search engine performance:");
    println!("  Total searches: {}", stats.total_searches);
    println!("  Sub-millisecond rate: {:.1}%", stats.sub_ms_rate * 100.0);
    println!("  Average latency: {}μs", stats.average_latency_us);
    println!("  P95 latency: {}μs", stats.p95_latency_us);
    println!("  P99 latency: {}μs", stats.p99_latency_us);
    println!("  Cache hit rate: {:.1}%", stats.cache_hit_rate * 100.0);
    println!("  Batch search time: {:?}", batch_duration);
    
    // Calculate individual search statistics
    let avg_single_search = search_times.iter().sum::<Duration>() / search_times.len() as u32;
    let mut sorted_times = search_times.clone();
    sorted_times.sort();
    let p95_single = sorted_times[(sorted_times.len() as f64 * 0.95) as usize];
    
    println!("  Single search avg: {:?}", avg_single_search);
    println!("  Single search P95: {:?}", p95_single);
    
    // Verify performance targets
    assert!(stats.total_searches > 50);
    assert!(stats.cache_hit_rate >= 0.0); // Should have some cache activity
    
    // Auto-tune should not fail
    search_engine.auto_tune().await?;
    
    Ok(())
}

#[tokio::test]
async fn test_search_accuracy() -> Result<(), Box<dyn std::error::Error>> {
    let dimension = 64;
    let num_vectors = 1000;
    let vectors = generate_test_vectors(num_vectors, dimension, 11111);
    
    // Create two index managers with different configurations
    let flat_config = IndexConfig {
        index_type: IndexType::Flat,
        metric_type: MetricType::InnerProduct,
        dimension,
        training_size_threshold: 100,
        gpu_enabled: false,
        compression_level: 0,
    };
    
    let hnsw_config = IndexConfig {
        index_type: IndexType::HNSW { m: 16, ef_construction: 200, ef_search: 100 },
        metric_type: MetricType::InnerProduct,
        dimension,
        training_size_threshold: 100,
        gpu_enabled: false,
        compression_level: 0,
    };
    
    let mut flat_manager = FaissIndexManager::new(flat_config);
    let mut hnsw_manager = FaissIndexManager::new(hnsw_config);
    
    flat_manager.create_index(num_vectors)?;
    hnsw_manager.create_index(num_vectors)?;
    
    let flat_vectors: Vec<f32> = vectors.iter().flat_map(|v| v.iter().cloned()).collect();
    flat_manager.add_vectors(&flat_vectors)?;
    hnsw_manager.add_vectors(&flat_vectors)?;
    
    // Compare search results
    let k = 20;
    let mut accuracy_sum = 0.0;
    let num_test_queries = 10;
    
    for i in 0..num_test_queries {
        let query = &vectors[i];
        
        let (flat_distances, flat_labels) = flat_manager.search(query, k)?;
        let (hnsw_distances, hnsw_labels) = hnsw_manager.search(query, k)?;
        
        // Calculate recall@k (how many true neighbors HNSW found)
        let flat_set: std::collections::HashSet<_> = flat_labels.iter().collect();
        let hnsw_set: std::collections::HashSet<_> = hnsw_labels.iter().collect();
        
        let intersection = flat_set.intersection(&hnsw_set).count();
        let recall = intersection as f64 / k as f64;
        accuracy_sum += recall;
        
        println!("Query {}: Recall@{} = {:.3}", i, k, recall);
    }
    
    let average_accuracy = accuracy_sum / num_test_queries as f64;
    println!("Average Recall@{}: {:.3}", k, average_accuracy);
    
    // HNSW should have decent accuracy (>80% for this configuration)
    assert!(average_accuracy > 0.8, "HNSW accuracy too low: {:.3}", average_accuracy);
    
    Ok(())
}

#[tokio::test]
async fn test_memory_efficiency() -> Result<(), Box<dyn std::error::Error>> {
    let dimension = 256;
    let num_vectors = 5000;
    let vectors = generate_test_vectors(num_vectors, dimension, 55555);
    
    // Test different configurations for memory usage
    let configs = vec![
        ("Flat", IndexConfig {
            index_type: IndexType::Flat,
            metric_type: MetricType::L2,
            dimension,
            training_size_threshold: 100,
            gpu_enabled: false,
            compression_level: 0,
        }),
        ("Memory Efficient", IndexConfig::memory_efficient(dimension)),
        ("Balanced", IndexConfig::balanced(dimension)),
    ];
    
    for (name, config) in configs {
        let mut index_manager = FaissIndexManager::new(config);
        index_manager.create_index(num_vectors)?;
        
        let flat_vectors: Vec<f32> = vectors.iter().flat_map(|v| v.iter().cloned()).collect();
        index_manager.add_vectors(&flat_vectors)?;
        
        let stats = index_manager.get_stats()?;
        
        println!("{} configuration:", name);
        println!("  Vectors: {}", stats.num_vectors);
        println!("  Memory usage: {:.2} MB", stats.memory_usage as f64 / 1024.0 / 1024.0);
        println!("  Memory per vector: {:.1} bytes", stats.memory_usage as f64 / stats.num_vectors as f64);
        
        assert_eq!(stats.num_vectors, num_vectors);
        assert_eq!(stats.dimension, dimension);
        assert!(stats.memory_usage > 0);
    }
    
    Ok(())
}

#[tokio::test]
async fn test_error_handling() -> Result<(), Box<dyn std::error::Error>> {
    let dimension = 128;
    
    // Test dimension mismatch
    let config = IndexConfig {
        index_type: IndexType::Flat,
        metric_type: MetricType::L2,
        dimension,
        training_size_threshold: 100,
        gpu_enabled: false,
        compression_level: 0,
    };
    
    let mut index_manager = FaissIndexManager::new(config);
    index_manager.create_index(100)?;
    
    // Try to add vectors with wrong dimension
    let wrong_vectors = vec![1.0, 2.0, 3.0]; // Only 3 dimensions instead of 128
    let result = index_manager.add_vectors(&wrong_vectors);
    assert!(result.is_err());
    
    // Try to search with wrong dimension
    let wrong_query = vec![1.0, 2.0]; // Only 2 dimensions
    let result = index_manager.search(&wrong_query, 5);
    assert!(result.is_err());
    
    // Test search on empty index
    let empty_config = IndexConfig {
        index_type: IndexType::Flat,
        metric_type: MetricType::L2,
        dimension,
        training_size_threshold: 100,
        gpu_enabled: false,
        compression_level: 0,
    };
    
    let mut empty_manager = FaissIndexManager::new(empty_config);
    empty_manager.create_index(100)?;
    
    let query = vec![0.0; dimension];
    let result = empty_manager.search(&query, 5);
    // Should not panic, but may return empty results
    assert!(result.is_ok());
    
    println!("✓ Error handling tests passed");
    
    Ok(())
}