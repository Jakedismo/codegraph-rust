use codegraph_core::{CodeNode, Language, Location, NodeType};
use codegraph_vector::{OptimizedKnnEngine, SearchConfig};
use std::time::Instant;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::init();
    
    println!("🚀 CodeGraph Optimized KNN Search Engine Demo");
    println!("============================================\n");

    // Configuration
    let dimension = 384;
    let config = SearchConfig {
        k: 5,
        precision_recall_tradeoff: 0.7,
        enable_clustering: true,
        cluster_threshold: 0.8,
        max_parallel_queries: 4,
        context_weight: 0.3,
        language_boost: 0.2,
        type_boost: 0.1,
    };

    // Initialize KNN engine
    println!("📊 Initializing KNN engine with dimension {}...", dimension);
    let engine = OptimizedKnnEngine::new(dimension, config.clone())?;

    // Create sample dataset
    println!("🏗️  Creating sample dataset...");
    let mut nodes = Vec::new();

    // Generate Rust functions
    for i in 0..50 {
        let embedding = generate_sample_embedding(dimension, i as u64);
        let node = create_sample_node(
            &format!("rust_function_{}", i),
            NodeType::Function,
            Language::Rust,
            embedding,
        );
        nodes.push(node);
    }

    // Generate Python functions
    for i in 0..30 {
        let embedding = generate_sample_embedding(dimension, (100 + i) as u64);
        let node = create_sample_node(
            &format!("python_function_{}", i),
            NodeType::Function,
            Language::Python,
            embedding,
        );
        nodes.push(node);
    }

    // Generate structs and classes
    for i in 0..20 {
        let embedding = generate_sample_embedding(dimension, (200 + i) as u64);
        let node = create_sample_node(
            &format!("data_structure_{}", i),
            if i % 2 == 0 { NodeType::Struct } else { NodeType::Class },
            if i % 2 == 0 { Language::Rust } else { Language::Python },
            embedding,
        );
        nodes.push(node);
    }

    println!("📈 Dataset created with {} nodes", nodes.len());

    // Build indices
    println!("🔍 Building search indices...");
    let build_start = Instant::now();
    engine.build_indices(&nodes).await?;
    let build_duration = build_start.elapsed();
    println!("✅ Indices built in {:?}", build_duration);

    // Display clustering information
    let cluster_info = engine.get_cluster_info().await;
    println!("\n🎯 Clustering Information:");
    println!("   Total clusters: {}", cluster_info.len());
    for (i, cluster) in cluster_info.iter().enumerate().take(5) {
        println!(
            "   Cluster {}: {} nodes, Language: {:?}, Type: {:?}",
            i, cluster.size, cluster.language, cluster.node_type
        );
    }

    // Perform single search
    println!("\n🔍 Single Similarity Search:");
    let query_embedding = generate_sample_embedding(dimension, 999);
    let search_start = Instant::now();
    
    let results = engine.single_similarity_search(query_embedding, config.clone()).await?;
    let search_duration = search_start.elapsed();
    
    println!("   Search completed in {:?}", search_duration);
    println!("   Found {} results:", results.len());
    
    for (i, result) in results.iter().enumerate().take(3) {
        if let Some(node) = &result.node {
            println!(
                "   {}. {} (Score: {:.4}, Context: {:.4})",
                i + 1,
                node.name,
                result.similarity_score,
                result.context_score
            );
            println!("      Type: {:?}, Language: {:?}",
                node.node_type, node.language);
            if let Some(cluster_id) = result.cluster_id {
                println!("      Cluster: {}", cluster_id);
            }
        }
    }

    // Perform parallel search
    println!("\n⚡ Parallel Similarity Search:");
    let queries = vec![
        generate_sample_embedding(dimension, 1001),
        generate_sample_embedding(dimension, 1002),
        generate_sample_embedding(dimension, 1003),
    ];
    
    let parallel_start = Instant::now();
    let parallel_results = engine.parallel_similarity_search(queries, Some(config.clone())).await?;
    let parallel_duration = parallel_start.elapsed();
    
    println!("   Parallel search ({} queries) completed in {:?}", 
        parallel_results.len(), parallel_duration);
    println!("   Average per query: {:?}", 
        parallel_duration / parallel_results.len() as u32);

    // Test batch function search
    println!("\n🔧 Batch Function Search:");
    let function_nodes: Vec<_> = nodes.iter()
        .filter(|n| matches!(n.node_type, Some(NodeType::Function)))
        .take(5)
        .cloned()
        .collect();
    
    let batch_start = Instant::now();
    let batch_results = engine.batch_search_similar_functions(&function_nodes, Some(config)).await?;
    let batch_duration = batch_start.elapsed();
    
    println!("   Batch search for {} functions completed in {:?}",
        function_nodes.len(), batch_duration);

    // Display performance statistics
    let stats = engine.get_performance_stats();
    println!("\n📊 Performance Statistics:");
    println!("   Total nodes: {}", stats.total_nodes);
    println!("   Total clusters: {}", stats.total_clusters);
    println!("   Index type: {}", stats.index_type);
    println!("   Max parallel queries: {}", stats.max_parallel_queries);
    println!("   Cache hit rate: {:.2}%", stats.cache_hit_rate * 100.0);

    // Test cache warmup
    println!("\n🔥 Cache Warmup:");
    let sample_nodes: Vec<_> = nodes.iter().take(10).map(|n| n.id).collect();
    let warmup_start = Instant::now();
    engine.warmup_cache(&sample_nodes).await?;
    let warmup_duration = warmup_start.elapsed();
    println!("   Cache warmed up in {:?}", warmup_duration);

    // Show improved performance after warmup
    let query_embedding_2 = generate_sample_embedding(dimension, 1234);
    let warm_search_start = Instant::now();
    let _warm_results = engine.single_similarity_search(query_embedding_2, SearchConfig::default()).await?;
    let warm_search_duration = warm_search_start.elapsed();
    println!("   Search after warmup: {:?}", warm_search_duration);

    println!("\n✨ Demo completed successfully!");
    Ok(())
}

fn create_sample_node(
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
            file_path: format!("{}.{}", name, get_file_extension(&language)),
            line: 1,
            column: 1,
            end_line: Some(10),
            end_column: Some(80),
        },
    )
    .with_embedding(embedding)
    .with_complexity(fastrand::f32() * 0.8 + 0.2) // Random complexity between 0.2 and 1.0
}

fn get_file_extension(language: &Language) -> &'static str {
    match language {
        Language::Rust => "rs",
        Language::Python => "py",
        Language::JavaScript => "js",
        Language::TypeScript => "ts",
        Language::Go => "go",
        Language::Java => "java",
        Language::Cpp => "cpp",
        Language::Other(_) => "txt",
    }
}

fn generate_sample_embedding(dimension: usize, seed: u64) -> Vec<f32> {
    let mut embedding = vec![0.0; dimension];
    let mut state = seed;
    
    // Use a simple PRNG for reproducible embeddings
    for i in 0..dimension {
        state = state.wrapping_mul(1103515245).wrapping_add(12345);
        embedding[i] = ((state as f32 / u32::MAX as f32) - 0.5) * 2.0;
    }
    
    // Normalize the embedding
    let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 0.0 {
        for x in &mut embedding {
            *x /= norm;
        }
    }
    
    embedding
}