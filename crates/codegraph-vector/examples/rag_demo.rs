use codegraph_core::{CodeNode, NodeType, Language, Location, Metadata};
use codegraph_vector::rag::{RAGSystem, RAGConfig};
use std::collections::HashMap;
use uuid::Uuid;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🚀 RAG System Demo");
    println!("================");

    // Initialize RAG system
    println!("\n1. Initializing RAG system...");
    let config = RAGConfig::default();
    let mut rag_system = RAGSystem::new(config).await?;
    
    // Create some sample code nodes
    println!("\n2. Adding sample code context...");
    let nodes = create_sample_nodes();
    
    for node in &nodes {
        rag_system.add_context(node.clone()).await?;
        println!("   Added: {} ({})", node.name, format!("{:?}", node.node_type.as_ref().unwrap()));
    }

    // Test queries
    println!("\n3. Testing RAG queries...");
    let test_queries = vec![
        "How do I read files?",
        "Find functions that handle errors",
        "What are the async operations available?",
        "Show me database functions",
        "How do I process user input?",
    ];

    for (i, query) in test_queries.iter().enumerate() {
        println!("\n   Query {}: \"{}\"", i + 1, query);
        
        let start_time = std::time::Instant::now();
        let result = rag_system.process_query(query).await?;
        let duration = start_time.elapsed();
        
        println!("   ⏱️  Processing time: {}ms", duration.as_millis());
        println!("   🎯 Confidence: {:.2}", result.confidence_score);
        println!("   📝 Response: {}", result.response);
        println!("   📊 Performance metrics:");
        println!("      - Query processing: {}ms", result.performance_metrics.query_processing_ms);
        println!("      - Context retrieval: {}ms", result.performance_metrics.context_retrieval_ms);
        println!("      - Result ranking: {}ms", result.performance_metrics.result_ranking_ms);
        println!("      - Response generation: {}ms", result.performance_metrics.response_generation_ms);
        
        if duration.as_millis() <= 200 {
            println!("   ✅ Performance target met (≤200ms)");
        } else {
            println!("   ⚠️  Performance target missed ({}ms > 200ms)", duration.as_millis());
        }
    }

    // Test system metrics
    println!("\n4. System metrics:");
    let metrics = rag_system.get_system_metrics().await;
    println!("   - Total queries: {}", metrics.total_queries);
    println!("   - Average response time: {:.2}ms", metrics.average_response_time_ms);
    println!("   - Cache hit rate: {:.2}%", metrics.cache_hit_rate * 100.0);
    println!("   - Successful queries: {}", metrics.successful_queries);
    println!("   - Failed queries: {}", metrics.failed_queries);

    // Test caching
    println!("\n5. Testing query caching...");
    let cache_test_query = "How do I read files?";
    
    let start1 = std::time::Instant::now();
    let _result1 = rag_system.process_query(cache_test_query).await?;
    let duration1 = start1.elapsed();
    
    let start2 = std::time::Instant::now();
    let _result2 = rag_system.process_query(cache_test_query).await?;
    let duration2 = start2.elapsed();
    
    println!("   First query: {}ms", duration1.as_millis());
    println!("   Cached query: {}ms", duration2.as_millis());
    
    if duration2 < duration1 {
        println!("   ✅ Caching working effectively");
    }

    println!("\n🎉 RAG System Demo completed successfully!");
    println!("✅ All four RAG deliverables implemented:");
    println!("   1. Query processing pipeline for natural language analysis");
    println!("   2. Context retrieval algorithms with relevance scoring");
    println!("   3. Result ranking system based on semantic similarity");
    println!("   4. Response generation logic with answer validation");
    
    Ok(())
}

fn create_sample_nodes() -> Vec<CodeNode> {
    let now = chrono::Utc::now();
    let location = Location {
        file_path: "sample.rs".to_string(),
        line: 1,
        column: 1,
        end_line: None,
        end_column: None,
    };
    let metadata = Metadata {
        attributes: HashMap::new(),
        created_at: now,
        updated_at: now,
    };

    vec![
        CodeNode {
            id: Uuid::new_v4(),
            name: "read_file".to_string(),
            node_type: Some(NodeType::Function),
            language: Some(Language::Rust),
            content: Some("async fn read_file(path: &str) -> Result<String, std::io::Error> { tokio::fs::read_to_string(path).await }".to_string()),
            embedding: None,
            location: location.clone(),
            metadata: metadata.clone(),
            complexity: Some(0.3),
        },
        CodeNode {
            id: Uuid::new_v4(),
            name: "write_file".to_string(),
            node_type: Some(NodeType::Function),
            language: Some(Language::Rust),
            content: Some("async fn write_file(path: &str, content: &str) -> Result<(), std::io::Error> { tokio::fs::write(path, content).await }".to_string()),
            embedding: None,
            location: location.clone(),
            metadata: metadata.clone(),
            complexity: Some(0.4),
        },
        CodeNode {
            id: Uuid::new_v4(),
            name: "handle_error".to_string(),
            node_type: Some(NodeType::Function),
            language: Some(Language::Rust),
            content: Some("fn handle_error<T, E: std::fmt::Display>(result: Result<T, E>) -> Option<T> { match result { Ok(value) => Some(value), Err(e) => { eprintln!(\"Error: {}\", e); None } } }".to_string()),
            embedding: None,
            location: location.clone(),
            metadata: metadata.clone(),
            complexity: Some(0.6),
        },
        CodeNode {
            id: Uuid::new_v4(),
            name: "fetch_data".to_string(),
            node_type: Some(NodeType::Function),
            language: Some(Language::Rust),
            content: Some("async fn fetch_data(url: &str) -> Result<String, reqwest::Error> { reqwest::get(url).await?.text().await }".to_string()),
            embedding: None,
            location: location.clone(),
            metadata: metadata.clone(),
            complexity: Some(0.5),
        },
        CodeNode {
            id: Uuid::new_v4(),
            name: "process_input".to_string(),
            node_type: Some(NodeType::Function),
            language: Some(Language::Rust),
            content: Some("fn process_input(input: &str) -> String { input.trim().to_lowercase() }".to_string()),
            embedding: None,
            location: location.clone(),
            metadata: metadata.clone(),
            complexity: Some(0.2),
        },
        CodeNode {
            id: Uuid::new_v4(),
            name: "connect_database".to_string(),
            node_type: Some(NodeType::Function),
            language: Some(Language::Rust),
            content: Some("async fn connect_database(url: &str) -> Result<Database, sqlx::Error> { sqlx::PgPool::connect(url).await }".to_string()),
            embedding: None,
            location: location.clone(),
            metadata: metadata.clone(),
            complexity: Some(0.7),
        },
        CodeNode {
            id: Uuid::new_v4(),
            name: "UserService".to_string(),
            node_type: Some(NodeType::Struct),
            language: Some(Language::Rust),
            content: Some("struct UserService { db: Database, cache: Cache }".to_string()),
            embedding: None,
            location: location.clone(),
            metadata: metadata.clone(),
            complexity: Some(0.3),
        },
        CodeNode {
            id: Uuid::new_v4(),
            name: "validate_user".to_string(),
            node_type: Some(NodeType::Function),
            language: Some(Language::Rust),
            content: Some("async fn validate_user(user_id: u64) -> Result<bool, ValidationError> { /* validation logic */ Ok(true) }".to_string()),
            embedding: None,
            location: location.clone(),
            metadata: metadata.clone(),
            complexity: Some(0.5),
        },
    ]
}