#![cfg(feature = "faiss")]
use codegraph_core::NodeId;
use codegraph_vector::{
    FaissIndexManager, IndexConfig, IndexType, MetricType,
};
#[cfg(feature = "persistent")]
use codegraph_vector::PersistentStorage;
#[cfg(feature = "persistent")]
use std::collections::HashMap;
#[cfg(feature = "persistent")]
use tempfile::TempDir;

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
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
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
        (
            "IVF",
            IndexType::IVF {
                nlist: 10,
                nprobe: 5,
            },
        ),
        (
            "HNSW",
            IndexType::HNSW {
                m: 8,
                ef_construction: 40,
                ef_search: 16,
            },
        ),
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
        let (distances, labels): (Vec<f32>, Vec<i64>) = index_manager.search(query, 10)?;

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
#[cfg(feature = "persistent")]
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

        let (loaded_id_mapping, loaded_reverse_mapping): (HashMap<i64, NodeId>, HashMap<NodeId, i64>) = storage.load_id_mapping()?;
        assert_eq!(loaded_id_mapping.len(), num_vectors);
        assert_eq!(loaded_reverse_mapping.len(), num_vectors);

        // Verify data integrity
        for i in 0..num_vectors {
            let node_id = node_id_from_index(i);
            let original_vector = &vectors[i];
            let loaded_vector = loaded_embeddings.get(&node_id).unwrap();

            assert_eq!(original_vector.len(), loaded_vector.len());
            for (a, b) in original_vector.iter().zip(loaded_vector.iter()) {
                assert!((*a - *b).abs() < 1e-6);
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
        index_type: IndexType::HNSW {
            m: 16,
            ef_construction: 200,
            ef_search: 100,
        },
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

        let (_flat_distances, flat_labels): (Vec<f32>, Vec<i64>) = flat_manager.search(query, k)?;
        let (_hnsw_distances, hnsw_labels): (Vec<f32>, Vec<i64>) = hnsw_manager.search(query, k)?;

        // Calculate recall@k (how many true neighbors HNSW found)
        let flat_set: std::collections::HashSet<i64> = flat_labels.iter().copied().collect::<std::collections::HashSet<i64>>();
        let hnsw_set: std::collections::HashSet<i64> = hnsw_labels.iter().copied().collect::<std::collections::HashSet<i64>>();

        let intersection = flat_set.intersection(&hnsw_set).count();
        let recall = intersection as f64 / k as f64;
        accuracy_sum += recall;

        println!("Query {}: Recall@{} = {:.3}", i, k, recall);
    }

    let average_accuracy = accuracy_sum / num_test_queries as f64;
    println!("Average Recall@{}: {:.3}", k, average_accuracy);

    // HNSW should have decent accuracy (>80% for this configuration)
    assert!(
        average_accuracy > 0.8,
        "HNSW accuracy too low: {:.3}",
        average_accuracy
    );

    Ok(())
}

#[tokio::test]
async fn test_memory_efficiency() -> Result<(), Box<dyn std::error::Error>> {
    let dimension = 256;
    let num_vectors = 5000;
    let vectors = generate_test_vectors(num_vectors, dimension, 55555);

    // Test different configurations for memory usage
    let configs = vec![
        (
            "Flat",
            IndexConfig {
                index_type: IndexType::Flat,
                metric_type: MetricType::L2,
                dimension,
                training_size_threshold: 100,
                gpu_enabled: false,
                compression_level: 0,
            },
        ),
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
        println!(
            "  Memory usage: {:.2} MB",
            stats.memory_usage as f64 / 1024.0 / 1024.0
        );
        println!(
            "  Memory per vector: {:.1} bytes",
            stats.memory_usage as f64 / stats.num_vectors as f64
        );

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
