// ABOUTME: Manual test for Ollama reranking with real API calls
// ABOUTME: Run with: cargo run -p codegraph-vector --example test_ollama_rerank --features ollama

use codegraph_core::{OllamaRerankConfig, RerankConfig, RerankProvider};
use codegraph_vector::reranking::{ollama::OllamaReranker, RerankDocument, Reranker};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Load .env file if present (for CODEGRAPH_OLLAMA_RERANK_MODEL etc)
    let _ = dotenvy::dotenv();

    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter("info,codegraph_vector=debug")
        .init();

    println!("=== Ollama Reranking Test ===\n");

    // Use OllamaRerankConfig::default() which reads from env vars:
    // - CODEGRAPH_OLLAMA_RERANK_MODEL or OLLAMA_RERANK_MODEL for model
    // - CODEGRAPH_OLLAMA_URL or OLLAMA_URL for api_base
    let ollama_config = OllamaRerankConfig::default();

    println!("Model: {}", ollama_config.model);
    println!("API Base: {}\n", ollama_config.api_base);

    // Create config with overridden timeout for test
    let config = RerankConfig {
        provider: RerankProvider::Ollama,
        top_n: 5,
        jina: None,
        ollama: Some(OllamaRerankConfig {
            timeout_secs: 60, // Longer timeout for test
            max_retries: 2,
            ..ollama_config
        }),
    };

    // Create reranker
    let reranker = OllamaReranker::new(&config)?;
    println!("✓ Reranker created: {} ({})\n", reranker.model_name(), reranker.provider_name());

    // Test query
    let query = "How do I handle errors in Rust?";
    println!("Query: \"{}\"\n", query);

    // Test documents - mix of relevant and irrelevant
    let documents = vec![
        RerankDocument {
            id: "doc1".to_string(),
            text: "The Result type in Rust is used for error handling. \
                   It has two variants: Ok(T) for success and Err(E) for errors. \
                   You can use the ? operator to propagate errors.".to_string(),
            metadata: Some(serde_json::json!({"relevance": "high"})),
        },
        RerankDocument {
            id: "doc2".to_string(),
            text: "Python uses try/except blocks for exception handling. \
                   You can catch specific exceptions or use a bare except clause.".to_string(),
            metadata: Some(serde_json::json!({"relevance": "low"})),
        },
        RerankDocument {
            id: "doc3".to_string(),
            text: "The anyhow crate provides ergonomic error handling in Rust. \
                   It offers the anyhow::Error type and the bail! macro for early returns.".to_string(),
            metadata: Some(serde_json::json!({"relevance": "high"})),
        },
        RerankDocument {
            id: "doc4".to_string(),
            text: "Chocolate chip cookies are delicious. Mix flour, sugar, butter, \
                   and chocolate chips. Bake at 350°F for 12 minutes.".to_string(),
            metadata: Some(serde_json::json!({"relevance": "none"})),
        },
        RerankDocument {
            id: "doc5".to_string(),
            text: "The thiserror crate helps define custom error types with derive macros. \
                   Use #[derive(Error)] and #[error(\"message\")] attributes.".to_string(),
            metadata: Some(serde_json::json!({"relevance": "high"})),
        },
        RerankDocument {
            id: "doc6".to_string(),
            text: "Java uses checked exceptions that must be declared in method signatures. \
                   RuntimeException and its subclasses are unchecked.".to_string(),
            metadata: Some(serde_json::json!({"relevance": "low"})),
        },
    ];

    println!("Documents to rerank: {}", documents.len());
    for doc in &documents {
        println!("  - [{}] {} chars", doc.id, doc.text.len());
    }
    println!();

    // Perform reranking
    println!("Reranking...\n");
    let start = std::time::Instant::now();
    let results = reranker.rerank(query, documents, 5).await?;
    let elapsed = start.elapsed();

    println!("=== Results (took {:.2}s) ===\n", elapsed.as_secs_f64());

    for (i, result) in results.iter().enumerate() {
        let expected = result.metadata
            .as_ref()
            .and_then(|m| m.get("relevance"))
            .and_then(|v| v.as_str())
            .unwrap_or("?");

        let status = match expected {
            "high" if result.score > 0.5 => "✓",
            "low" if result.score < 0.7 => "✓",
            "none" if result.score < 0.3 => "✓",
            _ => "?",
        };

        println!(
            "{}. [{}] score={:.3} (expected: {}) {}",
            i + 1,
            result.id,
            result.score,
            expected,
            status
        );
    }

    println!("\n=== Test Summary ===");

    // Check if high-relevance docs are ranked higher than low/none
    let high_relevance_in_top3 = results.iter().take(3).any(|r| {
        r.metadata
            .as_ref()
            .and_then(|m| m.get("relevance"))
            .and_then(|v| v.as_str()) == Some("high")
    });

    let irrelevant_not_top = results.iter().take(2).all(|r| {
        r.metadata
            .as_ref()
            .and_then(|m| m.get("relevance"))
            .and_then(|v| v.as_str()) != Some("none")
    });

    if high_relevance_in_top3 {
        println!("✓ High-relevance documents ranked in top 3");
    } else {
        println!("✗ High-relevance documents NOT in top 3 - check model");
    }

    if irrelevant_not_top {
        println!("✓ Irrelevant documents excluded from top 2");
    } else {
        println!("✗ Irrelevant documents in top 2 - check model");
    }

    println!("\nDone!");
    Ok(())
}
