// ABOUTME: Integration tests for LM Studio embedding provider
// ABOUTME: Tests actual embedding generation with clear error surfacing

use codegraph_core::{CodeNode, Language, Location, NodeType};
use std::time::Duration;

#[cfg(feature = "lmstudio")]
use codegraph_vector::{
    lmstudio_embedding_provider::{LmStudioEmbeddingConfig, LmStudioEmbeddingProvider},
    BatchConfig, EmbeddingProvider,
};

/// Create a test CodeNode for embedding tests
fn create_test_node(name: &str, content: &str) -> CodeNode {
    CodeNode::new(
        name.to_string(),
        Some(NodeType::Function),
        Some(Language::Rust),
        Location {
            file_path: "test.rs".to_string(),
            line: 1,
            column: 1,
            end_line: Some(10),
            end_column: Some(10),
        },
    )
    .with_content(content.to_string())
}

/// Helper to create test configuration with environment variable fallback
#[cfg(feature = "lmstudio")]
fn create_test_config() -> LmStudioEmbeddingConfig {
    let api_base =
        std::env::var("LMSTUDIO_API_BASE").unwrap_or_else(|_| "http://localhost:1234".to_string());

    let model = std::env::var("LMSTUDIO_EMBEDDING_MODEL")
        .unwrap_or_else(|_| "jinaai/jina-embeddings-v3".to_string());

    LmStudioEmbeddingConfig {
        model,
        api_base,
        timeout: Duration::from_secs(60),
        batch_size: 32,
        max_retries: 3,
        max_tokens_per_request: 8192,
    }
}

/// Test 1: LM Studio availability check
///
/// This test verifies that LM Studio is running and accessible.
/// If this fails, check:
/// - LM Studio is running (default: http://localhost:1234)
/// - An embedding model is loaded (any OpenAI-compatible embedding model)
/// - No firewall blocking the connection
#[cfg(feature = "lmstudio")]
#[tokio::test]
async fn test_lmstudio_availability() {
    let config = create_test_config();

    println!("🔍 Testing LM Studio availability at: {}", config.api_base);
    println!("📦 Model: {}", config.model);

    let provider = match LmStudioEmbeddingProvider::new(config.clone()) {
        Ok(p) => {
            println!("✅ LM Studio provider initialized successfully");
            p
        }
        Err(e) => {
            panic!(
                "❌ FAILED to initialize LM Studio provider: {}\n\
                 💡 Make sure LM Studio is running at {}\n\
                 💡 Check that the URL is correct in LMSTUDIO_API_BASE env var",
                e, config.api_base
            );
        }
    };

    let is_available = provider.check_availability().await;

    if !is_available {
        panic!(
            "❌ FAILED: LM Studio is not available at {}\n\
             \n\
             Troubleshooting:\n\
             1. ✅ Is LM Studio running?\n\
             2. ✅ Is an embedding model loaded in LM Studio?\n\
             3. ✅ Can you access {}models in a browser?\n\
             4. ✅ Is the URL correct? Set LMSTUDIO_API_BASE env var if needed\n\
             5. ✅ Check LM Studio logs for errors\n\
             \n\
             Current configuration:\n\
             - API Base: {}\n\
             - Model: {}",
            config.api_base,
            config.api_base.trim_end_matches("/v1"),
            config.api_base,
            config.model
        );
    }

    println!("✅ LM Studio is available and responding");
    println!("📊 Provider: {}", provider.provider_name());
    println!("📏 Dimension: {}", provider.embedding_dimension());
}

/// Test 2: Single text embedding generation
///
/// This test verifies that the provider can generate a single embedding.
/// If this fails, check:
/// - The embedding model is correctly loaded in LM Studio
/// - The model supports the /v1/embeddings endpoint
/// - The model name in config matches what's loaded in LM Studio
#[cfg(feature = "lmstudio")]
#[tokio::test]
async fn test_lmstudio_single_text_embedding() {
    let config = create_test_config();
    let provider = LmStudioEmbeddingProvider::new(config.clone())
        .expect("Failed to create provider - check test_lmstudio_availability first");

    if !provider.check_availability().await {
        eprintln!(
            "⚠️  SKIPPING: LM Studio not available. Run test_lmstudio_availability for diagnosis."
        );
        return;
    }

    println!("🧪 Testing single text embedding generation...");

    let test_text = "fn calculate_sum(a: i32, b: i32) -> i32 { a + b }";

    let result = provider.generate_single_embedding(test_text).await;

    match result {
        Ok(embedding) => {
            println!("✅ Successfully generated embedding");
            println!("📏 Embedding dimension: {}", embedding.len());

            // Validate embedding
            assert!(!embedding.is_empty(), "Embedding should not be empty");

            // Check L2 normalization (embeddings should be normalized)
            let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
            println!("🔢 L2 norm: {:.6}", norm);

            // Most embedding models return normalized vectors (norm ≈ 1.0)
            // Some models might not normalize, so we just check it's reasonable
            assert!(
                norm > 0.0 && norm < 100.0,
                "Embedding norm seems unreasonable: {}",
                norm
            );

            // Check values are reasonable (not all zeros, not all NaN)
            let has_non_zero = embedding.iter().any(|&x| x != 0.0);
            assert!(has_non_zero, "Embedding should have non-zero values");

            let has_nan = embedding.iter().any(|x| x.is_nan());
            assert!(!has_nan, "Embedding should not contain NaN values");

            println!("✅ Embedding validation passed");
            println!(
                "📊 Sample values: {:?}",
                &embedding[..5.min(embedding.len())]
            );
        }
        Err(e) => {
            panic!(
                "❌ FAILED to generate embedding: {}\n\
                 \n\
                 Troubleshooting:\n\
                 1. ✅ Is the embedding model loaded in LM Studio?\n\
                 2. ✅ Does the model support embeddings? (Check LM Studio model info)\n\
                 3. ✅ Is the model name correct? Current: '{}'\n\
                 4. ✅ Check LM Studio logs for API errors\n\
                 5. ✅ Try calling the API manually:\n\
                    curl -X POST {}/embeddings \\\n\
                      -H 'Content-Type: application/json' \\\n\
                      -d '{{\n\
                        \"input\": [\"test\"],\n\
                        \"model\": \"{}\"\n\
                      }}'",
                e, config.model, config.api_base, config.model
            );
        }
    }
}

/// Test 3: CodeNode embedding generation
///
/// This test verifies the full integration with CodeNode objects.
/// If this fails but test_lmstudio_single_text_embedding passes,
/// check the CodeNode formatting logic.
#[cfg(feature = "lmstudio")]
#[tokio::test]
async fn test_lmstudio_codenode_embedding() {
    let config = create_test_config();
    let provider = LmStudioEmbeddingProvider::new(config)
        .expect("Failed to create provider - check test_lmstudio_availability first");

    if !provider.check_availability().await {
        eprintln!(
            "⚠️  SKIPPING: LM Studio not available. Run test_lmstudio_availability for diagnosis."
        );
        return;
    }

    println!("🧪 Testing CodeNode embedding generation...");

    let node = create_test_node(
        "calculate_fibonacci",
        "fn calculate_fibonacci(n: u32) -> u32 {\n\
         if n <= 1 { n } else { calculate_fibonacci(n-1) + calculate_fibonacci(n-2) }\n\
         }",
    );

    let result = provider.generate_embedding(&node).await;

    match result {
        Ok(embedding) => {
            println!("✅ Successfully generated CodeNode embedding");
            println!("📏 Embedding dimension: {}", embedding.len());

            assert!(!embedding.is_empty(), "Embedding should not be empty");

            let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
            println!("🔢 L2 norm: {:.6}", norm);

            assert!(norm > 0.0, "Embedding should have positive norm");

            println!("✅ CodeNode embedding validation passed");
        }
        Err(e) => {
            panic!(
                "❌ FAILED to generate CodeNode embedding: {}\n\
                 This is unexpected if test_lmstudio_single_text_embedding passed.\n\
                 The issue might be in CodeNode formatting.",
                e
            );
        }
    }
}

/// Test 4: Batch embedding generation
///
/// This test verifies that multiple embeddings can be generated efficiently.
/// If this fails, check batch size configuration and API rate limits.
#[cfg(feature = "lmstudio")]
#[tokio::test]
async fn test_lmstudio_batch_embeddings() {
    let config = create_test_config();
    let provider = LmStudioEmbeddingProvider::new(config)
        .expect("Failed to create provider - check test_lmstudio_availability first");

    if !provider.check_availability().await {
        eprintln!(
            "⚠️  SKIPPING: LM Studio not available. Run test_lmstudio_availability for diagnosis."
        );
        return;
    }

    println!("🧪 Testing batch embedding generation...");

    let nodes = vec![
        create_test_node("function1", "fn add(a: i32, b: i32) -> i32 { a + b }"),
        create_test_node("function2", "fn subtract(a: i32, b: i32) -> i32 { a - b }"),
        create_test_node("function3", "fn multiply(a: i32, b: i32) -> i32 { a * b }"),
        create_test_node(
            "function4",
            "fn divide(a: i32, b: i32) -> Option<i32> { if b != 0 { Some(a / b) } else { None } }",
        ),
        create_test_node(
            "function5",
            "fn power(base: i32, exp: u32) -> i32 { base.pow(exp) }",
        ),
    ];

    println!("📦 Generating embeddings for {} nodes...", nodes.len());

    let start = std::time::Instant::now();
    let result = provider.generate_embeddings(&nodes).await;
    let duration = start.elapsed();

    match result {
        Ok(embeddings) => {
            println!(
                "✅ Successfully generated batch embeddings in {:?}",
                duration
            );
            println!("📏 Generated {} embeddings", embeddings.len());

            assert_eq!(
                embeddings.len(),
                nodes.len(),
                "Should generate one embedding per node"
            );

            for (i, embedding) in embeddings.iter().enumerate() {
                assert!(!embedding.is_empty(), "Embedding {} should not be empty", i);

                let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
                assert!(norm > 0.0, "Embedding {} should have positive norm", i);
            }

            // Embeddings should be different for different inputs
            assert_ne!(
                embeddings[0], embeddings[1],
                "Different code should produce different embeddings"
            );

            let throughput = nodes.len() as f64 / duration.as_secs_f64();
            println!("⚡ Throughput: {:.2} embeddings/sec", throughput);

            println!("✅ Batch embedding validation passed");
        }
        Err(e) => {
            panic!(
                "❌ FAILED to generate batch embeddings: {}\n\
                 \n\
                 Troubleshooting:\n\
                 1. ✅ Check LM Studio performance (might be slow under load)\n\
                 2. ✅ Reduce batch size if hitting API limits\n\
                 3. ✅ Check timeout settings (current: 60s)\n\
                 4. ✅ Monitor LM Studio resource usage (CPU/GPU/RAM)",
                e
            );
        }
    }
}

/// Test 5: Batch embedding with config and metrics
///
/// This test verifies the full batch processing pipeline with metrics.
#[cfg(feature = "lmstudio")]
#[tokio::test]
async fn test_lmstudio_batch_with_metrics() {
    let config = create_test_config();
    let provider = LmStudioEmbeddingProvider::new(config)
        .expect("Failed to create provider - check test_lmstudio_availability first");

    if !provider.check_availability().await {
        eprintln!(
            "⚠️  SKIPPING: LM Studio not available. Run test_lmstudio_availability for diagnosis."
        );
        return;
    }

    println!("🧪 Testing batch embedding with metrics...");

    let nodes = vec![
        create_test_node("fn1", "fn test1() { println!(\"Hello\"); }"),
        create_test_node("fn2", "fn test2() { println!(\"World\"); }"),
        create_test_node("fn3", "fn test3() { println!(\"Rust\"); }"),
    ];

    let batch_config = BatchConfig {
        batch_size: 10,
        max_concurrent: 2,
        timeout: Duration::from_secs(30),
        retry_attempts: 3,
    };

    let result = provider
        .generate_embeddings_with_config(&nodes, &batch_config)
        .await;

    match result {
        Ok((embeddings, metrics)) => {
            println!("✅ Successfully generated batch with metrics");
            println!("📊 Metrics:");
            println!("   - Provider: {}", metrics.provider_name);
            println!("   - Texts processed: {}", metrics.texts_processed);
            println!("   - Duration: {:?}", metrics.duration);
            println!("   - Throughput: {:.2} texts/s", metrics.throughput);

            assert_eq!(embeddings.len(), nodes.len());
            assert_eq!(metrics.texts_processed, nodes.len());
            assert_eq!(metrics.provider_name, "LM Studio");
            assert!(metrics.throughput > 0.0, "Throughput should be positive");

            println!("✅ Metrics validation passed");
        }
        Err(e) => {
            panic!(
                "❌ FAILED batch processing with metrics: {}\n\
                 This test combines all features. Check previous tests first.",
                e
            );
        }
    }
}

/// Test 6: Long text chunking
///
/// This test verifies that long text is properly chunked and embedded.
#[cfg(feature = "lmstudio")]
#[tokio::test]
async fn test_lmstudio_long_text_chunking() {
    let config = create_test_config();
    let provider = LmStudioEmbeddingProvider::new(config)
        .expect("Failed to create provider - check test_lmstudio_availability first");

    if !provider.check_availability().await {
        eprintln!(
            "⚠️  SKIPPING: LM Studio not available. Run test_lmstudio_availability for diagnosis."
        );
        return;
    }

    println!("🧪 Testing long text chunking and averaging...");

    // Create a very long text that will require chunking (>8192 tokens)
    let long_code = format!(
        "fn large_function() {{\n{}\n}}",
        (0..500)
            .map(|i| format!("    let var_{} = process_data({});", i, i))
            .collect::<Vec<_>>()
            .join("\n")
    );

    println!("📄 Text length: {} chars", long_code.len());

    let result = provider.generate_single_embedding(&long_code).await;

    match result {
        Ok(embedding) => {
            println!("✅ Successfully generated embedding for long text");
            println!("📏 Embedding dimension: {}", embedding.len());

            assert!(!embedding.is_empty());

            let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
            println!("🔢 L2 norm: {:.6}", norm);

            // For averaged embeddings from chunks, norm should still be reasonable
            assert!(
                norm > 0.0 && norm < 100.0,
                "Averaged embedding norm seems unreasonable: {}",
                norm
            );

            println!("✅ Long text chunking validation passed");
        }
        Err(e) => {
            panic!(
                "❌ FAILED to generate embedding for long text: {}\n\
                 This might indicate an issue with text chunking or averaging.",
                e
            );
        }
    }
}

/// Test 7: Error handling for invalid API base
///
/// This test verifies proper error handling when LM Studio is not available.
#[cfg(feature = "lmstudio")]
#[tokio::test]
async fn test_lmstudio_error_handling() {
    println!("🧪 Testing error handling with invalid API base...");

    let bad_config = LmStudioEmbeddingConfig {
        model: "test-model".to_string(),
        api_base: "http://localhost:9999/v1".to_string(), // Invalid port
        timeout: Duration::from_secs(2),                  // Short timeout
        batch_size: 32,
        max_retries: 1, // Only 1 retry for faster test
        max_tokens_per_request: 8192,
    };

    let provider = LmStudioEmbeddingProvider::new(bad_config)
        .expect("Provider initialization should succeed even with bad URL");

    let is_available = provider.check_availability().await;

    assert!(
        !is_available,
        "Provider should not be available with invalid URL"
    );

    println!("✅ Error handling validation passed");
}

// =============================================================================
// HOW TO RUN THESE TESTS
// =============================================================================
//
// These tests verify that LM Studio embedding generation is working correctly.
//
// Prerequisites:
// 1. LM Studio must be running (default: http://localhost:1234)
// 2. An embedding model must be loaded (e.g., jinaai/jina-embeddings-v3)
// 3. The model must support the /v1/embeddings endpoint
//
// Configuration (optional environment variables):
// - LMSTUDIO_API_BASE: API base URL (default: http://localhost:1234)
// - LMSTUDIO_EMBEDDING_MODEL: Model name (default: jinaai/jina-embeddings-v3)
//
// Run all tests:
// ```bash
// cargo test --package codegraph-vector --test lmstudio_integration_test --features lmstudio -- --nocapture
// ```
//
// Run specific test:
// ```bash
// cargo test --package codegraph-vector --test lmstudio_integration_test --features lmstudio test_lmstudio_availability -- --nocapture
// ```
//
// Test order (run in sequence):
// 1. test_lmstudio_availability - Checks if LM Studio is running
// 2. test_lmstudio_single_text_embedding - Tests basic embedding generation
// 3. test_lmstudio_codenode_embedding - Tests CodeNode integration
// 4. test_lmstudio_batch_embeddings - Tests batch processing
// 5. test_lmstudio_batch_with_metrics - Tests metrics collection
// 6. test_lmstudio_long_text_chunking - Tests chunking for long text
// 7. test_lmstudio_error_handling - Tests error scenarios
//
// ============================================================================
