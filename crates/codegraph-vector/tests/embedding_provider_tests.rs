use codegraph_core::{CodeNode, Language, NodeType, Location};
use codegraph_vector::{
    BatchConfig, EmbeddingProvider, HybridEmbeddingPipeline, FallbackStrategy,
};
use std::time::Duration;

/// Create a test CodeNode for embedding tests
fn create_test_node(name: &str, content: Option<String>) -> CodeNode {
    let mut node = CodeNode::new(
        name.to_string(),
        Some(NodeType::Function),
        Some(Language::Rust),
        Location {
            file_path: "test.rs".to_string(),
            line: 1,
            column: 1,
            end_line: Some(1),
            end_column: Some(10),
        },
    );
    
    if let Some(content) = content {
        node = node.with_content(content);
    }
    
    node
}

/// Mock embedding provider for testing
struct MockEmbeddingProvider {
    name: String,
    dimension: usize,
    fail_on_generate: bool,
    latency: Duration,
}

impl MockEmbeddingProvider {
    fn new(name: &str, dimension: usize) -> Self {
        Self {
            name: name.to_string(),
            dimension,
            fail_on_generate: false,
            latency: Duration::from_millis(10),
        }
    }

    fn with_failure(mut self) -> Self {
        self.fail_on_generate = true;
        self
    }

    fn with_latency(mut self, latency: Duration) -> Self {
        self.latency = latency;
        self
    }
}

#[async_trait::async_trait]
impl EmbeddingProvider for MockEmbeddingProvider {
    async fn generate_embedding(&self, _node: &CodeNode) -> codegraph_core::Result<Vec<f32>> {
        tokio::time::sleep(self.latency).await;
        
        if self.fail_on_generate {
            return Err(codegraph_core::CodeGraphError::External(
                "Mock provider failure".to_string(),
            ));
        }

        // Generate a deterministic but varied embedding based on node name hash
        let hash = simple_hash(&_node.name);
        let mut embedding = vec![0.0f32; self.dimension];
        let mut rng_state = hash;

        for i in 0..self.dimension {
            rng_state = rng_state.wrapping_mul(1103515245).wrapping_add(12345);
            embedding[i] = ((rng_state as f32 / u32::MAX as f32) - 0.5) * 2.0;
        }

        // L2 normalize
        let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 0.0 {
            for x in &mut embedding {
                *x /= norm;
            }
        }

        Ok(embedding)
    }

    async fn generate_embeddings(&self, nodes: &[CodeNode]) -> codegraph_core::Result<Vec<Vec<f32>>> {
        let mut embeddings = Vec::with_capacity(nodes.len());
        for node in nodes {
            embeddings.push(self.generate_embedding(node).await?);
        }
        Ok(embeddings)
    }

    async fn generate_embeddings_with_config(
        &self,
        nodes: &[CodeNode],
        _config: &BatchConfig,
    ) -> codegraph_core::Result<(Vec<Vec<f32>>, codegraph_vector::EmbeddingMetrics)> {
        let start = std::time::Instant::now();
        let embeddings = self.generate_embeddings(nodes).await?;
        let duration = start.elapsed();
        
        let metrics = codegraph_vector::EmbeddingMetrics::new(
            self.name.clone(),
            nodes.len(),
            duration,
        );
        
        Ok((embeddings, metrics))
    }

    fn embedding_dimension(&self) -> usize {
        self.dimension
    }

    fn provider_name(&self) -> &str {
        &self.name
    }

    async fn is_available(&self) -> bool {
        !self.fail_on_generate
    }

    fn performance_characteristics(&self) -> codegraph_vector::ProviderCharacteristics {
        codegraph_vector::ProviderCharacteristics {
            expected_throughput: 100.0,
            typical_latency: self.latency,
            max_batch_size: 32,
            supports_streaming: false,
            requires_network: false,
            memory_usage: codegraph_vector::MemoryUsage::Low,
        }
    }
}

fn simple_hash(text: &str) -> u32 {
    let mut hash = 5381u32;
    for byte in text.bytes() {
        hash = hash.wrapping_mul(33).wrapping_add(byte as u32);
    }
    hash
}

#[tokio::test]
async fn test_mock_provider_single_embedding() {
    let provider = MockEmbeddingProvider::new("test", 384);
    let node = create_test_node("test_function", Some("fn test() {}".to_string()));

    let embedding = provider.generate_embedding(&node).await.unwrap();
    
    assert_eq!(embedding.len(), 384);
    
    // Check L2 normalization
    let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
    assert!((norm - 1.0).abs() < 1e-5, "Embedding should be L2 normalized");
}

#[tokio::test]
async fn test_mock_provider_batch_embeddings() {
    let provider = MockEmbeddingProvider::new("test", 384);
    let nodes = vec![
        create_test_node("function1", Some("fn one() {}".to_string())),
        create_test_node("function2", Some("fn two() {}".to_string())),
        create_test_node("function3", Some("fn three() {}".to_string())),
    ];

    let embeddings = provider.generate_embeddings(&nodes).await.unwrap();
    
    assert_eq!(embeddings.len(), 3);
    for embedding in &embeddings {
        assert_eq!(embedding.len(), 384);
        
        // Check L2 normalization
        let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 1e-5, "Embedding should be L2 normalized");
    }

    // Embeddings should be different for different inputs
    assert_ne!(embeddings[0], embeddings[1]);
    assert_ne!(embeddings[1], embeddings[2]);
}

#[tokio::test]
async fn test_deterministic_embeddings() {
    let provider = MockEmbeddingProvider::new("test", 384);
    let node = create_test_node("test_function", Some("fn test() {}".to_string()));

    let embedding1 = provider.generate_embedding(&node).await.unwrap();
    let embedding2 = provider.generate_embedding(&node).await.unwrap();
    
    // Same input should produce same embedding
    assert_eq!(embedding1, embedding2);
}

#[tokio::test]
async fn test_batch_config_metrics() {
    let provider = MockEmbeddingProvider::new("test", 384);
    let nodes = vec![
        create_test_node("function1", None),
        create_test_node("function2", None),
    ];
    
    let config = BatchConfig {
        batch_size: 10,
        max_concurrent: 2,
        timeout: Duration::from_secs(5),
        retry_attempts: 3,
    };

    let (embeddings, metrics) = provider
        .generate_embeddings_with_config(&nodes, &config)
        .await
        .unwrap();
    
    assert_eq!(embeddings.len(), 2);
    assert_eq!(metrics.texts_processed, 2);
    assert_eq!(metrics.provider_name, "test");
    assert!(metrics.throughput > 0.0);
}

#[tokio::test]
async fn test_provider_failure_handling() {
    let provider = MockEmbeddingProvider::new("failing", 384).with_failure();
    let node = create_test_node("test_function", None);

    let result = provider.generate_embedding(&node).await;
    assert!(result.is_err());
    assert!(!provider.is_available().await);
}

#[tokio::test]
async fn test_hybrid_pipeline_primary_success() {
    let primary = MockEmbeddingProvider::new("primary", 384);
    let fallback = MockEmbeddingProvider::new("fallback", 384);
    
    let pipeline = HybridEmbeddingPipeline::new(
        Box::new(primary),
        FallbackStrategy::Sequential,
    ).add_fallback(Box::new(fallback));

    let node = create_test_node("test_function", None);
    let embedding = pipeline.generate_embedding(&node).await.unwrap();
    
    assert_eq!(embedding.len(), 384);
    assert_eq!(pipeline.provider_name(), "HybridPipeline");
}

#[tokio::test]
async fn test_hybrid_pipeline_fallback_on_failure() {
    let primary = MockEmbeddingProvider::new("primary", 384).with_failure();
    let fallback = MockEmbeddingProvider::new("fallback", 384);
    
    let pipeline = HybridEmbeddingPipeline::new(
        Box::new(primary),
        FallbackStrategy::Sequential,
    ).add_fallback(Box::new(fallback));

    let node = create_test_node("test_function", None);
    let embedding = pipeline.generate_embedding(&node).await.unwrap();
    
    assert_eq!(embedding.len(), 384);
}

#[tokio::test]
async fn test_hybrid_pipeline_fastest_first_strategy() {
    let slow_primary = MockEmbeddingProvider::new("slow", 384)
        .with_latency(Duration::from_millis(100));
    let fast_fallback = MockEmbeddingProvider::new("fast", 384)
        .with_latency(Duration::from_millis(10));
    
    let pipeline = HybridEmbeddingPipeline::new(
        Box::new(slow_primary),
        FallbackStrategy::FastestFirst,
    ).add_fallback(Box::new(fast_fallback));

    let node = create_test_node("test_function", None);
    let start = std::time::Instant::now();
    let embedding = pipeline.generate_embedding(&node).await.unwrap();
    let duration = start.elapsed();
    
    assert_eq!(embedding.len(), 384);
    // Should use the faster provider
    assert!(duration < Duration::from_millis(50));
}

#[tokio::test]
async fn test_empty_input_handling() {
    let provider = MockEmbeddingProvider::new("test", 384);
    let empty_nodes: Vec<CodeNode> = vec![];

    let embeddings = provider.generate_embeddings(&empty_nodes).await.unwrap();
    assert!(embeddings.is_empty());
}

#[tokio::test]
async fn test_performance_target_throughput() {
    let provider = MockEmbeddingProvider::new("test", 384)
        .with_latency(Duration::from_millis(5)); // 5ms per embedding
    
    // Create 100 test nodes
    let nodes: Vec<CodeNode> = (0..100)
        .map(|i| create_test_node(&format!("function_{}", i), None))
        .collect();

    let start = std::time::Instant::now();
    let embeddings = provider.generate_embeddings(&nodes).await.unwrap();
    let duration = start.elapsed();
    
    assert_eq!(embeddings.len(), 100);
    
    let throughput = nodes.len() as f64 / duration.as_secs_f64();
    println!("Achieved throughput: {:.2} texts/s", throughput);
    
    // Target: ≥100 texts/s
    // With 5ms latency, sequential processing gives ~200 texts/s
    assert!(throughput >= 50.0, "Throughput too low: {:.2} texts/s", throughput);
}

#[tokio::test]
async fn test_large_batch_processing() {
    let provider = MockEmbeddingProvider::new("test", 384);
    
    // Create 1000 test nodes for batch processing test
    let nodes: Vec<CodeNode> = (0..1000)
        .map(|i| create_test_node(&format!("node_{}", i), None))
        .collect();

    let config = BatchConfig {
        batch_size: 32,
        max_concurrent: 4,
        timeout: Duration::from_secs(30),
        retry_attempts: 3,
    };

    let start = std::time::Instant::now();
    let (embeddings, metrics) = provider
        .generate_embeddings_with_config(&nodes, &config)
        .await
        .unwrap();
    let duration = start.elapsed();
    
    assert_eq!(embeddings.len(), 1000);
    assert_eq!(metrics.texts_processed, 1000);
    
    // Target: 1k texts ≤30s
    assert!(duration <= Duration::from_secs(30), 
        "Batch processing too slow: {:?}", duration);
    
    println!("1000 text batch processed in {:?} ({:.2} texts/s)", 
        duration, metrics.throughput);
}