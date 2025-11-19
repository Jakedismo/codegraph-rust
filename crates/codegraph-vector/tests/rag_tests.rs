#[cfg(test)]
mod rag_integration_tests {
    use codegraph_core::{CodeNode, Language, NodeType};
    use codegraph_vector::rag::{RAGConfig, RAGSystem};
    use std::sync::Arc;
    use uuid::Uuid;

    fn create_test_node(name: &str, content: &str, node_type: NodeType) -> CodeNode {
        use codegraph_core::{Location, Metadata};

        let now = chrono::Utc::now();
        CodeNode {
            id: Uuid::new_v4(),
            name: name.to_string().into(),
            node_type: Some(node_type),
            language: Some(Language::Rust),
            content: Some(content.to_string().into()),
            embedding: None,
            location: Location {
                file_path: "test.rs".to_string().into(),
                line: 1,
                column: 1,
                end_line: None,
                end_column: None,
            },
            metadata: Metadata {
                attributes: std::collections::HashMap::new(),
                created_at: now,
                updated_at: now,
            },
            complexity: None,
        }
    }

    #[tokio::test]
    async fn test_query_processing_pipeline() {
        let config = RAGConfig::default();
        let rag_system = RAGSystem::new(config)
            .await
            .expect("Failed to create RAG system");

        let query = "find functions that handle file operations";
        let result = rag_system.process_query(query).await;

        assert!(result.is_ok());
        let query_result = result.unwrap();
        assert!(!query_result.query_id.is_nil());
        assert_eq!(query_result.original_query, query);
    }

    #[tokio::test]
    async fn test_context_retrieval_algorithms() {
        let config = RAGConfig::default();
        let mut rag_system = RAGSystem::new(config)
            .await
            .expect("Failed to create RAG system");

        // Add test nodes
        let nodes = vec![
            create_test_node(
                "read_file",
                "fn read_file(path: &str) -> Result<String>",
                NodeType::Function,
            ),
            create_test_node(
                "write_file",
                "fn write_file(path: &str, content: &str) -> Result<()>",
                NodeType::Function,
            ),
            create_test_node(
                "process_data",
                "fn process_data(data: Vec<u8>) -> Vec<u8>",
                NodeType::Function,
            ),
        ];

        for node in &nodes {
            rag_system
                .add_context(node.clone())
                .await
                .expect("Failed to add context");
        }

        let query = "file handling functions";
        let results = rag_system
            .retrieve_context(query, 2)
            .await
            .expect("Failed to retrieve context");

        assert!(results.len() <= 2);
        assert!(results
            .iter()
            .any(|r| r.node.as_ref().unwrap().name.contains("file")));
    }

    #[tokio::test]
    async fn test_result_ranking_system() {
        let config = RAGConfig::default();
        let mut rag_system = RAGSystem::new(config)
            .await
            .expect("Failed to create RAG system");

        let nodes = vec![
            create_test_node(
                "file_reader",
                "async fn file_reader() -> String",
                NodeType::Function,
            ),
            create_test_node(
                "data_processor",
                "fn data_processor() -> i32",
                NodeType::Function,
            ),
            create_test_node(
                "file_writer",
                "async fn file_writer(content: String)",
                NodeType::Function,
            ),
        ];

        for node in &nodes {
            rag_system
                .add_context(node.clone())
                .await
                .expect("Failed to add context");
        }

        let query = "async file operations";
        let results = rag_system
            .retrieve_context(query, 3)
            .await
            .expect("Failed to retrieve context");

        // Results should be ranked by relevance (file + async should rank higher)
        assert!(!results.is_empty());
        let top_result = &results[0];
        assert!(top_result.relevance_score > 0.0);

        // Verify ranking order (higher scores first)
        for i in 1..results.len() {
            assert!(results[i].relevance_score >= results[i + 1].relevance_score);
        }
    }

    #[tokio::test]
    async fn test_response_generation_logic() {
        let config = RAGConfig::default();
        let mut rag_system = RAGSystem::new(config)
            .await
            .expect("Failed to create RAG system");

        let nodes = vec![
            create_test_node(
                "calculate_sum",
                "fn calculate_sum(a: i32, b: i32) -> i32 { a + b }",
                NodeType::Function,
            ),
            create_test_node(
                "multiply",
                "fn multiply(x: f64, y: f64) -> f64 { x * y }",
                NodeType::Function,
            ),
        ];

        for node in &nodes {
            rag_system
                .add_context(node.clone())
                .await
                .expect("Failed to add context");
        }

        let query = "mathematical operations";
        let result = rag_system
            .generate_response(query)
            .await
            .expect("Failed to generate response");

        assert!(!result.answer.is_empty());
        assert!(!result.sources.is_empty());
        assert!(result.confidence > 0.0);
        assert!(result.processing_time_ms < 200); // Sub-200ms requirement
    }

    #[tokio::test]
    async fn test_end_to_end_performance() {
        let config = RAGConfig::default();
        let mut rag_system = RAGSystem::new(config)
            .await
            .expect("Failed to create RAG system");

        // Add multiple test nodes for realistic scenario
        let nodes: Vec<CodeNode> = (0..50)
            .map(|i| {
                create_test_node(
                    &format!("function_{}", i),
                    &format!("fn function_{}() -> i32 {{ {} }}", i, i),
                    NodeType::Function,
                )
            })
            .collect();

        for node in &nodes {
            rag_system
                .add_context(node.clone())
                .await
                .expect("Failed to add context");
        }

        let start = std::time::Instant::now();
        let query = "function that returns number";
        let result = rag_system
            .process_query(query)
            .await
            .expect("Failed to process query");
        let duration = start.elapsed();

        assert!(duration.as_millis() < 200); // Sub-200ms requirement
        assert!(!result.response.is_empty());
        assert!(result.confidence_score > 0.0);
    }

    #[tokio::test]
    async fn test_answer_validation() {
        let config = RAGConfig::default();
        let rag_system = RAGSystem::new(config)
            .await
            .expect("Failed to create RAG system");

        let query = "non-existent functionality that doesn't exist";
        let result = rag_system
            .generate_response(query)
            .await
            .expect("Failed to generate response");

        // Should handle queries with no relevant context gracefully
        assert!(result.confidence < 0.5); // Low confidence for irrelevant queries
        assert!(result.answer.contains("No relevant") || result.answer.contains("not found"));
    }

    #[tokio::test]
    async fn test_concurrent_queries() {
        let config = RAGConfig::default();
        let rag_system = Arc::new(
            RAGSystem::new(config)
                .await
                .expect("Failed to create RAG system"),
        );

        let mut handles = vec![];

        for i in 0..10 {
            let rag_clone: Arc<RAGSystem> = Arc::clone(&rag_system);
            let handle: tokio::task::JoinHandle<_> = tokio::spawn(async move {
                let query = format!("query number {}", i);
                rag_clone.process_query(&query).await
            });
            handles.push(handle);
        }

        let results = futures::future::join_all(handles).await;

        for result in results {
            let query_result = result.expect("Task panicked").expect("Query failed");
            assert!(!query_result.response.is_empty());
        }
    }
}

#[cfg(test)]
mod rag_unit_tests {

    use codegraph_vector::rag::{
        ContextRetriever, QueryProcessor, ResponseGenerator, ResultRanker,
    };

    #[tokio::test]
    async fn test_query_processor_natural_language_analysis() {
        let processor = QueryProcessor::new();

        let queries = vec![
            "Find functions that handle file I/O operations",
            "Show me async functions for network requests",
            "What are the error handling patterns in this codebase?",
        ];

        for query in queries {
            let processed = processor
                .analyze_query(query)
                .await
                .expect("Failed to analyze query");
            assert!(!processed.keywords.is_empty());
            assert!(!processed.intent.is_empty());
            assert!(processed.query_type.is_some());
        }
    }

    #[tokio::test]
    async fn test_context_retriever_relevance_scoring() {
        let retriever = ContextRetriever::new();

        let test_contexts = vec![
            "async function for file reading".to_string(),
            "synchronous database operation".to_string(),
            "async network request handler".to_string(),
        ];

        let query = "async operations";
        let scores = retriever
            .calculate_relevance_scores(query, &test_contexts)
            .await
            .expect("Failed to calculate scores");

        assert_eq!(scores.len(), 3);
        // First and third should have higher relevance scores due to "async" keyword
        assert!(scores[0] > scores[1]);
        assert!(scores[2] > scores[1]);
    }

    #[tokio::test]
    async fn test_result_ranker_semantic_similarity() {
        let mut ranker = ResultRanker::new();

        let mut results = vec![
            ("function for data processing", 0.3),
            ("async file reader function", 0.8),
            ("network connection handler", 0.5),
        ]
        .into_iter()
        .map(|(content, score)| {
            // Mock result structure
            (content.to_string(), score)
        })
        .collect::<Vec<_>>();

        ranker
            .rank_by_semantic_similarity(&mut results, "async file operations")
            .await
            .expect("Failed to rank results");

        // Results should be sorted by relevance score (highest first)
        assert!(results[0].1 >= results[1].1);
        assert!(results[1].1 >= results[2].1);
    }

    #[tokio::test]
    async fn test_response_generator_answer_validation() {
        let generator = ResponseGenerator::new();

        let relevant_context = vec![
            "fn read_file(path: &str) -> Result<String, Error>".to_string(),
            "async fn write_file(path: &str, content: &str) -> Result<(), Error>".to_string(),
        ];

        let query = "How do I read and write files?";
        let response = generator
            .generate_validated_response(query, &relevant_context)
            .await
            .expect("Failed to generate response");

        assert!(!response.answer.is_empty());
        assert!(response.confidence > 0.5); // Should be confident with relevant context
        assert!(!response.sources.is_empty());

        // Test with irrelevant context
        let irrelevant_context = vec![
            "fn calculate_fibonacci(n: u32) -> u32".to_string(),
            "struct DatabaseConnection { host: String }".to_string(),
        ];

        let response_irrelevant = generator
            .generate_validated_response(query, &irrelevant_context)
            .await
            .expect("Failed to generate response");
        assert!(response_irrelevant.confidence < 0.5); // Should have low confidence
    }
}
