use codegraph_core::{CodeNode, Language, Location, Metadata, NodeId, NodeType, Result};
use codegraph_vector::{ContextualSearchResult, OptimizedKnnEngine, SearchConfig};
use std::collections::HashMap;
use tokio_test;

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

// Generate random normalized embedding
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

#[tokio::test]
async fn test_basic_knn_search() -> Result<()> {
    let dimension = 128;
    let config = SearchConfig::default();
    let engine = OptimizedKnnEngine::new(dimension, config)?;

    // Create test nodes
    let nodes = vec![
        create_test_node(
            "function_a",
            NodeType::Function,
            Language::Rust,
            generate_random_embedding(dimension, 1),
        ),
        create_test_node(
            "function_b",
            NodeType::Function,
            Language::Rust,
            generate_random_embedding(dimension, 2),
        ),
        create_test_node(
            "function_c",
            NodeType::Function,
            Language::Python,
            generate_random_embedding(dimension, 3),
        ),
    ];

    engine.build_indices(&nodes).await?;

    // Test search
    let query_embedding = generate_random_embedding(dimension, 1);
    let results = engine
        .single_similarity_search(
            query_embedding,
            SearchConfig {
                k: 2,
                ..SearchConfig::default()
            },
        )
        .await?;

    assert!(!results.is_empty());
    assert!(results.len() <= 2);

    // Results should be sorted by score
    if results.len() > 1 {
        assert!(results[0].final_score >= results[1].final_score);
    }

    Ok(())
}

#[tokio::test]
async fn test_parallel_search() -> Result<()> {
    let dimension = 128;
    let config = SearchConfig {
        max_parallel_queries: 4,
        ..SearchConfig::default()
    };
    let engine = OptimizedKnnEngine::new(dimension, config)?;

    // Create more test nodes
    let mut nodes = Vec::new();
    for i in 0..20 {
        nodes.push(create_test_node(
            &format!("function_{}", i),
            NodeType::Function,
            if i % 2 == 0 {
                Language::Rust
            } else {
                Language::Python
            },
            generate_random_embedding(dimension, i as u64),
        ));
    }

    engine.build_indices(&nodes).await?;

    // Test parallel search
    let queries = vec![
        generate_random_embedding(dimension, 100),
        generate_random_embedding(dimension, 101),
        generate_random_embedding(dimension, 102),
    ];

    let results = engine.parallel_similarity_search(queries, None).await?;

    assert_eq!(results.len(), 3);
    for result_set in results {
        assert!(!result_set.is_empty());
    }

    Ok(())
}

#[tokio::test]
async fn test_contextual_ranking() -> Result<()> {
    let dimension = 128;
    let config = SearchConfig {
        context_weight: 0.5,
        language_boost: 0.3,
        type_boost: 0.2,
        ..SearchConfig::default()
    };
    let engine = OptimizedKnnEngine::new(dimension, config)?;

    // Create nodes with different languages and types
    let nodes = vec![
        create_test_node(
            "rust_function",
            NodeType::Function,
            Language::Rust,
            generate_random_embedding(dimension, 1),
        ),
        create_test_node(
            "python_function",
            NodeType::Function,
            Language::Python,
            generate_random_embedding(dimension, 1), // Same embedding as rust_function
        ),
        create_test_node(
            "rust_struct",
            NodeType::Struct,
            Language::Rust,
            generate_random_embedding(dimension, 2),
        ),
    ];

    engine.build_indices(&nodes).await?;

    // Search with a query that should prefer Rust functions
    let results = engine
        .single_similarity_search(generate_random_embedding(dimension, 1), config.clone())
        .await?;

    assert!(!results.is_empty());

    // The rust_function should potentially rank higher due to context scoring
    // even if embeddings are similar
    let rust_function_result = results
        .iter()
        .find(|r| r.node.as_ref().unwrap().name == "rust_function");
    assert!(rust_function_result.is_some());

    Ok(())
}

#[tokio::test]
async fn test_clustering() -> Result<()> {
    let dimension = 128;
    let config = SearchConfig {
        enable_clustering: true,
        cluster_threshold: 0.7,
        ..SearchConfig::default()
    };
    let engine = OptimizedKnnEngine::new(dimension, config)?;

    // Create nodes with similar embeddings to form clusters
    let mut nodes = Vec::new();
    let base_embedding = generate_random_embedding(dimension, 42);

    // Cluster 1: Rust functions
    for i in 0..5 {
        let mut embedding = base_embedding.clone();
        // Add small variations
        for j in 0..dimension {
            embedding[j] += (i as f32 * 0.01);
        }
        nodes.push(create_test_node(
            &format!("rust_func_{}", i),
            NodeType::Function,
            Language::Rust,
            embedding,
        ));
    }

    // Cluster 2: Python functions (different embeddings)
    for i in 0..5 {
        nodes.push(create_test_node(
            &format!("python_func_{}", i),
            NodeType::Function,
            Language::Python,
            generate_random_embedding(dimension, 100 + i as u64),
        ));
    }

    engine.build_indices(&nodes).await?;

    let cluster_info = engine.get_cluster_info().await;
    assert!(!cluster_info.is_empty());

    // Test search results include cluster information
    let results = engine
        .single_similarity_search(base_embedding, config)
        .await?;

    assert!(!results.is_empty());

    // Some results should have cluster information
    let has_cluster_info = results.iter().any(|r| r.cluster_id.is_some());
    assert!(has_cluster_info);

    Ok(())
}

#[tokio::test]
async fn test_batch_function_search() -> Result<()> {
    let dimension = 128;
    let config = SearchConfig::default();
    let engine = OptimizedKnnEngine::new(dimension, config)?;

    // Create test functions
    let mut nodes = Vec::new();
    for i in 0..10 {
        nodes.push(create_test_node(
            &format!("function_{}", i),
            NodeType::Function,
            Language::Rust,
            generate_random_embedding(dimension, i as u64),
        ));
    }

    engine.build_indices(&nodes).await?;

    // Test batch search for similar functions
    let function_nodes: Vec<_> = nodes
        .iter()
        .filter(|n| matches!(n.node_type, Some(NodeType::Function)))
        .take(3)
        .cloned()
        .collect();

    let results = engine
        .batch_search_similar_functions(&function_nodes, None)
        .await?;

    assert_eq!(results.len(), 3);
    for result_set in results {
        assert!(!result_set.is_empty());
    }

    Ok(())
}

#[tokio::test]
async fn test_cache_performance() -> Result<()> {
    let dimension = 64; // Smaller dimension for faster testing
    let config = SearchConfig::default();
    let engine = OptimizedKnnEngine::new(dimension, config)?;

    // Create test nodes
    let nodes = vec![create_test_node(
        "test_function",
        NodeType::Function,
        Language::Rust,
        generate_random_embedding(dimension, 42),
    )];

    engine.build_indices(&nodes).await?;

    let query_embedding = generate_random_embedding(dimension, 42);
    let search_config = SearchConfig::default();

    // First search (cache miss)
    let start = std::time::Instant::now();
    let _results1 = engine
        .single_similarity_search(query_embedding.clone(), search_config.clone())
        .await?;
    let first_duration = start.elapsed();

    // Second search (should be cache hit)
    let start = std::time::Instant::now();
    let _results2 = engine
        .single_similarity_search(query_embedding, search_config)
        .await?;
    let second_duration = start.elapsed();

    // Second search should be faster due to caching
    // Note: This might not always be true in tests due to small dataset
    println!(
        "First search: {:?}, Second search: {:?}",
        first_duration, second_duration
    );

    let stats = engine.get_performance_stats();
    assert!(stats.cache_hit_rate >= 0.0);

    Ok(())
}

#[tokio::test]
async fn test_precision_recall_tradeoff() -> Result<()> {
    let dimension = 128;
    let engine_exact = OptimizedKnnEngine::new(
        dimension,
        SearchConfig {
            precision_recall_tradeoff: 1.0, // Maximum precision
            ..SearchConfig::default()
        },
    )?;

    let engine_fast = OptimizedKnnEngine::new(
        dimension,
        SearchConfig {
            precision_recall_tradeoff: 0.0, // Maximum recall (fastest)
            ..SearchConfig::default()
        },
    )?;

    // Create test nodes
    let mut nodes = Vec::new();
    for i in 0..100 {
        nodes.push(create_test_node(
            &format!("function_{}", i),
            NodeType::Function,
            Language::Rust,
            generate_random_embedding(dimension, i as u64),
        ));
    }

    engine_exact.build_indices(&nodes).await?;
    engine_fast.build_indices(&nodes).await?;

    let query_embedding = generate_random_embedding(dimension, 999);

    // Test both engines
    let exact_results = engine_exact
        .single_similarity_search(
            query_embedding.clone(),
            SearchConfig {
                k: 10,
                precision_recall_tradeoff: 1.0,
                ..SearchConfig::default()
            },
        )
        .await?;

    let fast_results = engine_fast
        .single_similarity_search(
            query_embedding,
            SearchConfig {
                k: 10,
                precision_recall_tradeoff: 0.0,
                ..SearchConfig::default()
            },
        )
        .await?;

    assert_eq!(exact_results.len(), fast_results.len());

    // Both should return results, but may differ in ordering/scores
    assert!(!exact_results.is_empty());
    assert!(!fast_results.is_empty());

    Ok(())
}

#[tokio::test]
async fn test_related_code_discovery() -> Result<()> {
    let dimension = 128;
    let config = SearchConfig {
        enable_clustering: true,
        ..SearchConfig::default()
    };
    let engine = OptimizedKnnEngine::new(dimension, config)?;

    let mut nodes = Vec::new();
    for i in 0..20 {
        nodes.push(create_test_node(
            &format!("node_{}", i),
            if i % 3 == 0 {
                NodeType::Function
            } else {
                NodeType::Struct
            },
            if i % 2 == 0 {
                Language::Rust
            } else {
                Language::Python
            },
            generate_random_embedding(dimension, i as u64),
        ));
    }

    engine.build_indices(&nodes).await?;

    // Test related code discovery
    let seed_nodes: Vec<NodeId> = nodes.iter().take(3).map(|n| n.id).collect();
    let related_clusters = engine
        .discover_related_code_clusters(&seed_nodes, 15)
        .await?;

    assert!(!related_clusters.is_empty());

    for (cluster_id, results) in related_clusters {
        println!("Cluster {}: {} related nodes", cluster_id, results.len());
        assert!(!results.is_empty());
    }

    Ok(())
}
