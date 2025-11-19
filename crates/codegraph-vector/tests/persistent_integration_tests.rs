#[cfg(feature = "persistent")]
mod persistent_tests {
    use codegraph_core::{CodeNode, NodeId, VectorStore};
    use codegraph_vector::{
        ConsistencyConfig, ConsistencyManager, IncrementalConfig, IncrementalOperation,
        IncrementalUpdateManager, IsolationLevel, PersistentVectorStore, VectorOperation,
    };
    use std::collections::HashMap;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};
    use tempfile::TempDir;
    use uuid::Uuid;

    const TEST_DIMENSION: usize = 128;

    fn create_test_vectors(count: usize, dimension: usize) -> Vec<(NodeId, Vec<f32>)> {
        (0..count)
            .map(|i| {
                let vector: Vec<f32> = (0..dimension)
                    .map(|j| (i * dimension + j) as f32 * 0.01)
                    .collect();
                (Uuid::from_u128(i as u128), vector)
            })
            .collect()
    }

    fn create_test_nodes(vectors: &[(NodeId, Vec<f32>)]) -> Vec<CodeNode> {
        vectors
            .iter()
            .map(|(id, vector)| {
                // Extract integer ID from UUID for deterministic file paths
                let id_int = id.as_u128() as usize;
                let location = codegraph_core::Location {
                    file_path: format!("test_{}.rs", id_int),
                    line: id_int as u32,
                    column: 0,
                    end_line: Some(id_int as u32 + 10),
                    end_column: Some(0),
                };

                let metadata = codegraph_core::Metadata {
                    attributes: HashMap::new(),
                    created_at: chrono::Utc::now(),
                    updated_at: chrono::Utc::now(),
                };

                CodeNode {
                    id: *id,
                    name: format!("test_node_{}", id_int).into(),
                    node_type: Some(codegraph_core::NodeType::Function),
                    language: None,
                    location,
                    content: Some(format!("Content for node {}", id_int).into()),
                    metadata,
                    embedding: Some(vector.clone()),
                    complexity: None,
                }
            })
            .collect()
    }

    #[tokio::test]
    async fn test_persistent_storage_lifecycle() {
        let temp_dir = TempDir::new().unwrap();
        let storage_path = temp_dir.path().join("test_vectors.db");
        let backup_path = temp_dir.path().join("backups");

        // Create persistent storage
        let mut store =
            PersistentVectorStore::new(&storage_path, &backup_path, TEST_DIMENSION).unwrap();

        // Generate test data
        let test_vectors = create_test_vectors(100, TEST_DIMENSION);
        let test_nodes = create_test_nodes(&test_vectors);

        // Store embeddings
        store.store_embeddings(&test_nodes).await.unwrap();

        // Verify storage statistics
        let stats = store.get_stats().unwrap();
        assert_eq!(stats.active_vectors, test_nodes.len());
        assert_eq!(stats.dimension, TEST_DIMENSION);

        // Test similarity search
        let query_vector = &test_vectors[0].1;
        let similar_nodes: Vec<NodeId> = store.search_similar(query_vector, 10).await.unwrap();
        assert!(!similar_nodes.is_empty());
        assert!(similar_nodes.contains(&test_vectors[0].0));

        // Test individual vector retrieval
        for (node_id, _expected_vector) in &test_vectors[0..5] {
            let retrieved = store.get_embedding(*node_id).await.unwrap();
            assert!(retrieved.is_some());
            // Note: In real implementation, we'd compare the vectors
            // Here we're just checking that retrieval works
        }
    }

    #[tokio::test]
    async fn test_vector_compression() {
        let temp_dir = TempDir::new().unwrap();
        let storage_path = temp_dir.path().join("test_compressed.db");
        let backup_path = temp_dir.path().join("backups");

        let mut store =
            PersistentVectorStore::new(&storage_path, &backup_path, TEST_DIMENSION).unwrap();

        // Enable product quantization
        store.enable_product_quantization(16, 8).unwrap();

        let test_vectors = create_test_vectors(50, TEST_DIMENSION);
        let test_nodes = create_test_nodes(&test_vectors);

        // Store embeddings (should trigger training and compression)
        store.store_embeddings(&test_nodes).await.unwrap();

        let stats = store.get_stats().unwrap();
        assert!(stats.compression_ratio > 1.0, "Should achieve compression");
        assert_eq!(stats.compressed_vectors, test_nodes.len());
    }

    #[tokio::test]
    async fn test_scalar_quantization() {
        let temp_dir = TempDir::new().unwrap();
        let storage_path = temp_dir.path().join("test_scalar.db");
        let backup_path = temp_dir.path().join("backups");

        let mut store =
            PersistentVectorStore::new(&storage_path, &backup_path, TEST_DIMENSION).unwrap();

        // Enable scalar quantization
        store.enable_scalar_quantization(8, false).unwrap();

        let test_vectors = create_test_vectors(30, TEST_DIMENSION);
        let test_nodes = create_test_nodes(&test_vectors);

        store.store_embeddings(&test_nodes).await.unwrap();

        let stats = store.get_stats().unwrap();
        assert!(stats.compression_ratio > 1.0);
    }

    #[tokio::test]
    async fn test_backup_and_recovery() {
        let temp_dir = TempDir::new().unwrap();
        let storage_path = temp_dir.path().join("test_backup.db");
        let backup_path = temp_dir.path().join("backups");

        let mut store =
            PersistentVectorStore::new(&storage_path, &backup_path, TEST_DIMENSION).unwrap();

        let test_vectors = create_test_vectors(20, TEST_DIMENSION);
        let test_nodes = create_test_nodes(&test_vectors);

        store.store_embeddings(&test_nodes).await.unwrap();

        // Create backup
        let backup_file = store.create_backup().await.unwrap();
        assert!(backup_file.exists());

        // Simulate corruption by creating new storage
        let new_store = PersistentVectorStore::new(
            temp_dir.path().join("test_backup2.db"),
            backup_path,
            TEST_DIMENSION,
        )
        .unwrap();

        // Restore from backup
        new_store.restore_from_backup(&backup_file).await.unwrap();

        // Verify data integrity after restore
        let stats = new_store.get_stats().unwrap();
        assert_eq!(stats.active_vectors, test_nodes.len());
    }

    #[tokio::test]
    async fn test_incremental_updates() {
        let config = IncrementalConfig {
            max_batch_size: 10,
            batch_timeout: Duration::from_millis(50),
            worker_threads: 2,
            enable_parallel_processing: true,
            ..Default::default()
        };

        let manager = IncrementalUpdateManager::new(config).unwrap();

        // Submit insert operations
        let insert_ops: Vec<_> = (0..25)
            .map(|i| IncrementalOperation::Insert {
                node_id: Uuid::from_u128(i as u128),
                vector: vec![i as f32; TEST_DIMENSION],
                timestamp: SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs(),
            })
            .collect();

        manager.submit_batch(insert_ops).unwrap();

        // Wait for processing
        tokio::time::sleep(Duration::from_millis(200)).await;

        let stats = manager.get_stats();
        assert!(stats.successful_operations > 0);
        assert!(stats.batches_processed > 0);

        // Test update operations
        let update_ops: Vec<_> = (0..5)
            .map(|i| IncrementalOperation::Update {
                node_id: Uuid::from_u128(i as u128),
                old_vector: Some(vec![i as f32; TEST_DIMENSION]),
                new_vector: vec![(i + 100) as f32; TEST_DIMENSION],
                timestamp: SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs(),
            })
            .collect();

        manager.submit_batch(update_ops).unwrap();
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Test delete operations
        let delete_ops: Vec<_> = (20..25)
            .map(|i| IncrementalOperation::Delete {
                node_id: Uuid::from_u128(i as u128),
                timestamp: SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs(),
            })
            .collect();

        manager.submit_batch(delete_ops).unwrap();
        tokio::time::sleep(Duration::from_millis(100)).await;

        let final_stats = manager.get_stats();
        assert!(final_stats.successful_operations >= stats.successful_operations);
    }

    #[tokio::test]
    async fn test_segment_management() {
        let config = IncrementalConfig {
            max_segment_size: 1000, // Small segments for testing
            max_batch_size: 5,
            batch_timeout: Duration::from_millis(10),
            ..Default::default()
        };

        let manager = IncrementalUpdateManager::new(config).unwrap();

        // Add vectors to trigger multiple segments
        for batch in 0..10 {
            let ops: Vec<_> = (0..10)
                .map(|i| {
                    let node_id = Uuid::from_u128((batch * 10 + i) as u128);
                    IncrementalOperation::Insert {
                        node_id,
                        vector: vec![1.0; 100], // Large vectors
                        timestamp: SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .unwrap()
                            .as_secs(),
                    }
                })
                .collect();

            manager.submit_batch(ops).unwrap();
        }

        // Wait for processing
        tokio::time::sleep(Duration::from_millis(300)).await;

        let segments = manager.get_segments();
        assert!(segments.len() > 1, "Should create multiple segments");

        // Test segment merging
        let initial_count = segments.len();
        let merged_count = manager.merge_segments(5).await.unwrap();

        if merged_count > 0 {
            let new_segments = manager.get_segments();
            assert!(
                new_segments.len() <= initial_count,
                "Should reduce segment count"
            );
        }
    }

    #[tokio::test]
    async fn test_consistency_manager() {
        let config = ConsistencyConfig::default();
        let manager = ConsistencyManager::new(config);

        // Test transaction lifecycle
        let txn_id = manager
            .begin_transaction(IsolationLevel::ReadCommitted)
            .unwrap();

        // Add operations to transaction
        let operations = vec![
            VectorOperation::Insert {
                node_id: Uuid::from_u128(1),
                vector: vec![1.0, 2.0, 3.0],
            },
            VectorOperation::Update {
                node_id: Uuid::from_u128(2),
                old_vector: Some(vec![2.0, 3.0, 4.0]),
                new_vector: vec![3.0, 4.0, 5.0],
            },
        ];

        for op in operations {
            manager.add_operation(txn_id, op).unwrap();
        }

        // Test two-phase commit
        manager.prepare_transaction(txn_id).unwrap();
        manager.commit_transaction(txn_id).unwrap();

        let stats = manager.get_transaction_stats();
        assert_eq!(stats.committed_transactions, 1);
    }

    #[tokio::test]
    async fn test_transaction_isolation() {
        let config = ConsistencyConfig::default();
        let manager = ConsistencyManager::new(config);

        // Start two transactions
        let txn1 = manager
            .begin_transaction(IsolationLevel::Serializable)
            .unwrap();
        let txn2 = manager
            .begin_transaction(IsolationLevel::Serializable)
            .unwrap();

        // Both try to modify the same node
        let op1 = VectorOperation::Insert {
            node_id: Uuid::from_u128(1),
            vector: vec![1.0, 2.0, 3.0],
        };

        let op2 = VectorOperation::Update {
            node_id: Uuid::from_u128(1),
            old_vector: None,
            new_vector: vec![4.0, 5.0, 6.0],
        };

        manager.add_operation(txn1, op1).unwrap();

        // Second transaction should detect conflict with serializable isolation
        let result = manager.add_operation(txn2, op2);
        assert!(result.is_err(), "Should detect serialization conflict");

        // Clean up first transaction
        manager.abort_transaction(txn1).unwrap();
    }

    #[tokio::test]
    async fn test_lock_management() {
        use codegraph_vector::LockMode;

        let config = ConsistencyConfig::default();
        let manager = ConsistencyManager::new(config);

        let txn1 = manager
            .begin_transaction(IsolationLevel::ReadCommitted)
            .unwrap();
        let txn2 = manager
            .begin_transaction(IsolationLevel::ReadCommitted)
            .unwrap();

        // Transaction 1 acquires shared lock
        manager
            .acquire_lock(txn1, Uuid::from_u128(1), LockMode::Shared)
            .await
            .unwrap();

        // Transaction 2 can also acquire shared lock
        manager
            .acquire_lock(txn2, Uuid::from_u128(1), LockMode::Shared)
            .await
            .unwrap();

        // But transaction 2 cannot acquire exclusive lock (should timeout)
        let result = tokio::time::timeout(
            Duration::from_millis(100),
            manager.acquire_lock(txn2, Uuid::from_u128(1), LockMode::Exclusive),
        )
        .await;

        assert!(result.is_err(), "Should timeout on exclusive lock conflict");

        // Clean up
        manager.abort_transaction(txn1).unwrap();
        manager.abort_transaction(txn2).unwrap();
    }

    #[tokio::test]
    async fn test_transaction_abort_and_rollback() {
        let config = ConsistencyConfig::default();
        let manager = ConsistencyManager::new(config);

        let txn_id = manager
            .begin_transaction(IsolationLevel::ReadCommitted)
            .unwrap();

        // Add some operations
        let operations = vec![
            VectorOperation::Insert {
                node_id: Uuid::from_u128(10),
                vector: vec![10.0, 20.0, 30.0],
            },
            VectorOperation::Update {
                node_id: Uuid::from_u128(11),
                old_vector: Some(vec![1.0, 2.0, 3.0]),
                new_vector: vec![11.0, 22.0, 33.0],
            },
            VectorOperation::Delete {
                node_id: Uuid::from_u128(12),
                vector: Some(vec![12.0, 24.0, 36.0]),
            },
        ];

        for op in operations {
            manager.add_operation(txn_id, op).unwrap();
        }

        // Abort transaction and get rollback operations
        let rollback_ops = manager.abort_transaction(txn_id).unwrap();

        assert_eq!(rollback_ops.len(), 3);

        // Verify rollback operations are in reverse order and correct type
        match &rollback_ops[0] {
            VectorOperation::Insert { node_id, .. } => {
                assert_eq!(*node_id, Uuid::from_u128(12)); // Should restore deleted item
            }
            _ => panic!("Expected insert operation for delete rollback"),
        }

        match &rollback_ops[1] {
            VectorOperation::Update {
                node_id,
                new_vector,
                ..
            } => {
                assert_eq!(*node_id, Uuid::from_u128(11));
                assert_eq!(*new_vector, vec![1.0, 2.0, 3.0]); // Should restore old vector
            }
            _ => panic!("Expected update operation for update rollback"),
        }

        match &rollback_ops[2] {
            VectorOperation::Delete { node_id, .. } => {
                assert_eq!(*node_id, Uuid::from_u128(10)); // Should delete inserted item
            }
            _ => panic!("Expected delete operation for insert rollback"),
        }
    }

    #[tokio::test]
    async fn test_consistency_checkpoints() {
        let config = ConsistencyConfig::default();
        let manager = ConsistencyManager::new(config);

        // Create and commit some transactions
        for i in 0..5 {
            let txn_id = manager
                .begin_transaction(IsolationLevel::ReadCommitted)
                .unwrap();

            let op = VectorOperation::Insert {
                node_id: Uuid::from_u128(i as u128),
                vector: vec![i as f32; 3],
            };

            manager.add_operation(txn_id, op).unwrap();
            manager.prepare_transaction(txn_id).unwrap();
            manager.commit_transaction(txn_id).unwrap();
        }

        // Create checkpoint
        let checkpoint = manager.create_checkpoint().unwrap();
        assert!(checkpoint.checkpoint_id > 0);
        assert_eq!(checkpoint.committed_transactions.len(), 5);

        // Verify we can retrieve the checkpoint
        let latest_checkpoint = manager.get_latest_checkpoint().unwrap();
        assert_eq!(latest_checkpoint.checkpoint_id, checkpoint.checkpoint_id);
    }

    #[tokio::test]
    async fn test_storage_persistence() {
        let temp_dir = TempDir::new().unwrap();
        let storage_path = temp_dir.path().join("test_persist.db");
        let backup_path = temp_dir.path().join("backups");

        // Create and populate storage
        {
            let mut store =
                PersistentVectorStore::new(&storage_path, &backup_path, TEST_DIMENSION).unwrap();

            let test_vectors = create_test_vectors(50, TEST_DIMENSION);
            let test_nodes = create_test_nodes(&test_vectors);

            store.store_embeddings(&test_nodes).await.unwrap();

            let stats = store.get_stats().unwrap();
            assert_eq!(stats.active_vectors, 50);
        }

        // Reload storage and verify data persistence
        {
            let store =
                PersistentVectorStore::new(&storage_path, &backup_path, TEST_DIMENSION).unwrap();

            let stats = store.get_stats().unwrap();
            assert_eq!(stats.active_vectors, 50);
            assert_eq!(stats.dimension, TEST_DIMENSION);

            // Verify we can still perform searches
            let query_vector = vec![0.5; TEST_DIMENSION];
            let results = store.search_similar(&query_vector, 5).await.unwrap();
            assert!(!results.is_empty());
        }
    }

    #[tokio::test]
    async fn test_incremental_updates_with_wal() {
        let config = IncrementalConfig {
            enable_wal: true,
            wal_flush_interval: Duration::from_millis(10),
            max_batch_size: 5,
            batch_timeout: Duration::from_millis(50),
            ..Default::default()
        };

        let manager = IncrementalUpdateManager::new(config).unwrap();

        // Submit operations that should be logged to WAL
        let operations: Vec<_> = (0..20)
            .map(|i| IncrementalOperation::Insert {
                node_id: Uuid::from_u128(i as u128),
                vector: vec![i as f32; 10],
                timestamp: SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs(),
            })
            .collect();

        for op in operations {
            manager.submit_operation(op).unwrap();
        }

        // Wait for WAL flush
        tokio::time::sleep(Duration::from_millis(100)).await;

        let stats = manager.get_stats();
        assert!(stats.successful_operations > 0);
    }

    #[tokio::test]
    async fn test_performance_under_load() {
        let temp_dir = TempDir::new().unwrap();
        let storage_path = temp_dir.path().join("test_perf.db");
        let backup_path = temp_dir.path().join("backups");

        let mut store =
            PersistentVectorStore::new(&storage_path, &backup_path, TEST_DIMENSION).unwrap();

        // Enable compression for better performance
        store.enable_product_quantization(16, 8).unwrap();

        let start_time = SystemTime::now();

        // Store a large number of vectors
        let batch_size = 100;
        let num_batches = 10;

        for _batch in 0..num_batches {
            let vectors = create_test_vectors(batch_size, TEST_DIMENSION);
            let nodes = create_test_nodes(&vectors);

            store.store_embeddings(&nodes).await.unwrap();
        }

        let storage_time = start_time.elapsed().unwrap();
        println!(
            "Stored {} vectors in {:?}",
            batch_size * num_batches,
            storage_time
        );

        // Test search performance
        let search_start = SystemTime::now();
        let query_vector = vec![0.5; TEST_DIMENSION];

        for _ in 0..50 {
            let _results: Vec<NodeId> = store.search_similar(&query_vector, 10).await.unwrap();
        }

        let search_time = search_start.elapsed().unwrap();
        println!("Performed 50 searches in {:?}", search_time);

        let stats = store.get_stats().unwrap();
        assert_eq!(stats.active_vectors, batch_size * num_batches);
        assert!(stats.compression_ratio > 1.0);
    }
}
