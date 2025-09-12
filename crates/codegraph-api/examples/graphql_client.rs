//! Example of querying the CodeGraph API using GraphQL
//!
//! This example demonstrates how to:
//! - Connect to a running CodeGraph API server
//! - Execute GraphQL queries
//! - Handle responses
//!
//! Run with: `cargo run --example graphql_client`
//! Note: Requires the API server to be running (see basic_server example)

use reqwest;
use serde_json::json;
use tracing::{info, Level};
use tracing_subscriber;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt().with_max_level(Level::INFO).init();

    // API server endpoint
    let api_url = "http://localhost:3000/graphql";
    info!("Connecting to GraphQL API at: {}", api_url);

    // Create HTTP client
    let client = reqwest::Client::new();

    // Example 1: Health check query
    info!("\n=== Health Check Query ===");
    let health_query = json!({
        "query": r#"
            query {
                health
                version
            }
        "#
    });

    let response = client.post(api_url).json(&health_query).send().await?;

    if response.status().is_success() {
        let result: serde_json::Value = response.json().await?;
        info!(
            "Health check response: {}",
            serde_json::to_string_pretty(&result)?
        );
    } else {
        eprintln!("Health check failed: {}", response.status());
    }

    // Example 2: Code search query
    info!("\n=== Code Search Query ===");
    let search_query = json!({
        "query": r#"
            query SearchCode($input: CodeSearchInput!) {
                searchCode(input: $input) {
                    nodes {
                        id
                        name
                        nodeType
                        language
                        location {
                            filePath
                            line
                            column
                        }
                    }
                    totalCount
                    searchMetadata {
                        queryTimeMs
                        indexUsed
                    }
                }
            }
        "#,
        "variables": {
            "input": {
                "query": "function",
                "limit": 5,
                "languageFilter": ["RUST"],
                "nodeTypeFilter": ["FUNCTION"]
            }
        }
    });

    let response = client.post(api_url).json(&search_query).send().await?;

    if response.status().is_success() {
        let result: serde_json::Value = response.json().await?;

        if let Some(data) = result.get("data") {
            if let Some(search_result) = data.get("searchCode") {
                let total = search_result
                    .get("totalCount")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);
                info!("Found {} results", total);

                if let Some(nodes) = search_result.get("nodes").and_then(|v| v.as_array()) {
                    info!("First {} results:", nodes.len());
                    for (i, node) in nodes.iter().enumerate() {
                        if let (Some(name), Some(node_type)) = (
                            node.get("name").and_then(|v| v.as_str()),
                            node.get("nodeType").and_then(|v| v.as_str()),
                        ) {
                            info!("  {}. {} ({})", i + 1, name, node_type);
                        }
                    }
                }

                if let Some(metadata) = search_result.get("searchMetadata") {
                    if let Some(time) = metadata.get("queryTimeMs").and_then(|v| v.as_f64()) {
                        info!("Query executed in {:.2}ms", time);
                    }
                }
            }
        }
    } else {
        eprintln!("Search query failed: {}", response.status());
    }

    // Example 3: Semantic search query
    info!("\n=== Semantic Search Query ===");
    let semantic_query = json!({
        "query": r#"
            query SemanticSearch($input: SemanticSearchInput!) {
                semanticSearch(input: $input) {
                    nodes {
                        node {
                            id
                            name
                            nodeType
                        }
                        similarityScore
                        rankingScore
                    }
                    totalCandidates
                    searchMetadata {
                        embeddingTimeMs
                        searchTimeMs
                        vectorDimension
                    }
                }
            }
        "#,
        "variables": {
            "input": {
                "query": "error handling and recovery",
                "similarityThreshold": 0.7,
                "limit": 3
            }
        }
    });

    let response = client.post(api_url).json(&semantic_query).send().await?;

    if response.status().is_success() {
        let result: serde_json::Value = response.json().await?;

        if let Some(data) = result.get("data") {
            if let Some(search_result) = data.get("semanticSearch") {
                let total = search_result
                    .get("totalCandidates")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);
                info!("Found {} candidates", total);

                if let Some(nodes) = search_result.get("nodes").and_then(|v| v.as_array()) {
                    info!("Top {} results by similarity:", nodes.len());
                    for (i, result) in nodes.iter().enumerate() {
                        if let (Some(node), Some(score)) = (
                            result.get("node"),
                            result.get("similarityScore").and_then(|v| v.as_f64()),
                        ) {
                            if let Some(name) = node.get("name").and_then(|v| v.as_str()) {
                                info!("  {}. {} (similarity: {:.3})", i + 1, name, score);
                            }
                        }
                    }
                }

                if let Some(metadata) = search_result.get("searchMetadata") {
                    if let (Some(embed_time), Some(search_time)) = (
                        metadata.get("embeddingTimeMs").and_then(|v| v.as_f64()),
                        metadata.get("searchTimeMs").and_then(|v| v.as_f64()),
                    ) {
                        info!(
                            "Embedding: {:.2}ms, Search: {:.2}ms",
                            embed_time, search_time
                        );
                    }
                }
            }
        }
    } else {
        eprintln!("Semantic search failed: {}", response.status());
    }

    info!("\n=== GraphQL client example completed ===");
    Ok(())
}
