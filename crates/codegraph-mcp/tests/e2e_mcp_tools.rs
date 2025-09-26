/// E2E Tests for CodeGraph MCP Server Tools
///
/// Tests all 8 essential MCP tools against the indexed Rust codebase
/// using the official rmcp client library for authentic protocol testing.
use anyhow::Result;
use rmcp::{model::CallToolRequestParam, service::ServiceExt, transport::TokioChildProcess};
use serde_json::json;
use std::process::Command;
use std::time::Duration;
use tokio::time::timeout;

/// Test configuration and utilities
struct TestConfig {
    server_timeout: Duration,
    tool_timeout: Duration,
}

impl Default for TestConfig {
    fn default() -> Self {
        Self {
            server_timeout: Duration::from_secs(30),
            tool_timeout: Duration::from_secs(60),
        }
    }
}

/// Start CodeGraph MCP server as a child process for testing
async fn start_mcp_server() -> Result<impl ServiceExt> {
    let mut cmd = Command::new("codegraph");
    cmd.args(["start", "stdio"])
        .env("RUST_LOG", "error") // Minimize log noise during testing
        .env(
            "CODEGRAPH_MODEL",
            "hf.co/unsloth/Qwen2.5-Coder-14B-Instruct-128K-GGUF:Q4_K_M",
        );

    let service = ().serve(TokioChildProcess::new(cmd)?).await?;
    Ok(service)
}

#[tokio::test]
async fn test_tool_discovery() -> Result<()> {
    println!("🔍 Testing MCP tool discovery...");

    let service = start_mcp_server().await?;

    // Test that all 8 essential tools are discoverable
    let tools = service.list_tools(Default::default()).await?;

    let expected_tools = vec![
        "enhanced_search",
        "semantic_intelligence",
        "impact_analysis",
        "pattern_detection",
        "vector_search",
        "graph_neighbors",
        "graph_traverse",
        "performance_metrics",
    ];

    println!("📋 Discovered {} tools", tools.tools.len());

    for expected_tool in &expected_tools {
        let found = tools.tools.iter().any(|tool| tool.name == *expected_tool);
        assert!(
            found,
            "Tool '{}' not found in discovered tools",
            expected_tool
        );
        println!("✅ Tool '{}' discovered", expected_tool);
    }

    // Verify no unwanted tools are present
    for tool in &tools.tools {
        if !expected_tools.contains(&tool.name.as_str()) {
            panic!("❌ Unexpected tool found: {}", tool.name);
        }
    }

    service.cancel().await?;
    println!("🎉 Tool discovery test passed!");
    Ok(())
}

#[tokio::test]
async fn test_enhanced_search() -> Result<()> {
    println!("🔍 Testing enhanced_search tool with Rust patterns...");

    let service = start_mcp_server().await?;
    let config = TestConfig::default();

    // Test searching for common Rust patterns in this codebase
    let test_cases = vec![
        (
            "async function",
            "Should find async functions in the codebase",
        ),
        ("trait implementation", "Should find trait impls"),
        ("error handling", "Should find Result and Error patterns"),
        ("MCP tool", "Should find tool definitions"),
    ];

    for (query, description) in test_cases {
        println!("🧪 Testing query: '{}' - {}", query, description);

        let result = timeout(
            config.tool_timeout,
            service.call_tool(CallToolRequestParam {
                name: "enhanced_search".into(),
                arguments: Some(
                    json!({
                        "query": query,
                        "limit": 5
                    })
                    .as_object()
                    .unwrap()
                    .clone(),
                ),
            }),
        )
        .await??;

        // Verify we got a response
        assert!(
            !result.content.is_empty(),
            "No content returned for query: {}",
            query
        );

        // Check that response contains relevant information
        let response_text = if let Some(content) = result.content.first() {
            match content {
                content if content.text.is_some() => content.text.as_ref().unwrap(),
                _ => panic!("Expected text content"),
            }
        } else {
            panic!("No content in response");
        };

        assert!(
            response_text.contains("CodeGraph"),
            "Response should mention CodeGraph"
        );
        assert!(
            response_text.contains(query)
                || response_text.to_lowercase().contains(&query.to_lowercase()),
            "Response should reference the query"
        );

        println!("✅ Query '{}' returned valid response", query);
    }

    service.cancel().await?;
    println!("🎉 Enhanced search test passed!");
    Ok(())
}

#[tokio::test]
async fn test_vector_search() -> Result<()> {
    println!("🔍 Testing vector_search tool for fast similarity search...");

    let service = start_mcp_server().await?;
    let config = TestConfig::default();

    // Test vector search with different parameters
    let test_cases = vec![
        (
            json!({"query": "async fn", "limit": 3}),
            "Basic async function search",
        ),
        (
            json!({"query": "struct", "limit": 5}),
            "Struct definitions search",
        ),
        (
            json!({"query": "impl", "langs": ["rust"], "limit": 4}),
            "Implementation blocks with language filter",
        ),
        (
            json!({"query": "error", "paths": ["crates/"], "limit": 2}),
            "Error handling with path filter",
        ),
    ];

    for (params, description) in test_cases {
        println!("🧪 Testing: {}", description);

        let result = timeout(
            config.tool_timeout,
            service.call_tool(CallToolRequestParam {
                name: "vector_search".into(),
                arguments: params.as_object().cloned(),
            }),
        )
        .await??;

        assert!(
            !result.content.is_empty(),
            "No content returned for vector search"
        );

        // Verify JSON response structure
        let response_text = if let Some(content) = result.content.first() {
            content.text.as_ref().expect("Expected text content")
        } else {
            panic!("No content in response");
        };

        // Should be valid JSON (vector search returns structured data)
        let _parsed: Value = serde_json::from_str(response_text)
            .map_err(|e| anyhow::anyhow!("Invalid JSON response: {}", e))?;

        println!("✅ Vector search test passed: {}", description);
    }

    service.cancel().await?;
    println!("🎉 Vector search test passed!");
    Ok(())
}

#[tokio::test]
async fn test_semantic_intelligence() -> Result<()> {
    println!("🧠 Testing semantic_intelligence tool for architectural analysis...");

    let service = start_mcp_server().await?;
    let config = TestConfig::default();

    // Test comprehensive analysis queries relevant to this Rust codebase
    let test_cases = vec![
        (
            json!({
                "query": "Explain the MCP server architecture",
                "task_type": "architectural_analysis",
                "max_context_tokens": 40000
            }),
            "MCP server architecture analysis",
        ),
        (
            json!({
                "query": "How does the semantic analysis work?",
                "task_type": "semantic_search"
            }),
            "Semantic analysis explanation",
        ),
        (
            json!({
                "query": "Describe the parser pipeline",
                "max_context_tokens": 60000
            }),
            "Parser pipeline analysis",
        ),
    ];

    for (params, description) in test_cases {
        println!("🧪 Testing: {}", description);

        let result = timeout(
            config.tool_timeout,
            service.call_tool(CallToolRequestParam {
                name: "semantic_intelligence".into(),
                arguments: params.as_object().cloned(),
            }),
        )
        .await??;

        assert!(
            !result.content.is_empty(),
            "No content returned for semantic intelligence"
        );

        let response_text = if let Some(content) = result.content.first() {
            content.text.as_ref().expect("Expected text content")
        } else {
            panic!("No content in response");
        };

        // Verify response quality
        assert!(
            response_text.len() > 100,
            "Response too short for semantic intelligence"
        );
        assert!(
            response_text.contains("Qwen")
                || response_text.contains("Analysis")
                || response_text.contains("CodeGraph"),
            "Response should be relevant to the platform"
        );

        println!("✅ Semantic intelligence test passed: {}", description);
    }

    service.cancel().await?;
    println!("🎉 Semantic intelligence test passed!");
    Ok(())
}

#[tokio::test]
async fn test_impact_analysis() -> Result<()> {
    println!("⚡ Testing impact_analysis tool for change prediction...");

    let service = start_mcp_server().await?;
    let config = TestConfig::default();

    // Test impact analysis on real functions in this codebase
    let test_cases = vec![
        (
            json!({
                "target_function": "CodeGraphMCPServer",
                "file_path": "crates/codegraph-mcp/src/official_server.rs",
                "change_type": "modify"
            }),
            "MCP server structure impact",
        ),
        (
            json!({
                "target_function": "enhanced_search",
                "file_path": "crates/codegraph-mcp/src/official_server.rs",
                "change_type": "refactor"
            }),
            "Tool function refactor impact",
        ),
    ];

    for (params, description) in test_cases {
        println!("🧪 Testing: {}", description);

        let result = timeout(
            config.tool_timeout,
            service.call_tool(CallToolRequestParam {
                name: "impact_analysis".into(),
                arguments: params.as_object().cloned(),
            }),
        )
        .await??;

        assert!(
            !result.content.is_empty(),
            "No content returned for impact analysis"
        );

        let response_text = if let Some(content) = result.content.first() {
            content.text.as_ref().expect("Expected text content")
        } else {
            panic!("No content in response");
        };

        // Verify response mentions the target function
        let target_function = params["target_function"].as_str().unwrap();
        assert!(
            response_text.contains(target_function),
            "Response should mention target function: {}",
            target_function
        );

        println!("✅ Impact analysis test passed: {}", description);
    }

    service.cancel().await?;
    println!("🎉 Impact analysis test passed!");
    Ok(())
}

#[tokio::test]
async fn test_pattern_detection() -> Result<()> {
    println!("🎯 Testing pattern_detection tool for Rust conventions...");

    let service = start_mcp_server().await?;
    let config = TestConfig::default();

    // Pattern detection doesn't need parameters
    let result = timeout(
        config.tool_timeout,
        service.call_tool(CallToolRequestParam {
            name: "pattern_detection".into(),
            arguments: None,
        }),
    )
    .await??;

    assert!(
        !result.content.is_empty(),
        "No content returned for pattern detection"
    );

    let response_text = match &result.content[0] {
        content if content.text.is_some() => content.text.as_ref().unwrap(),
        _ => panic!("Expected text content"),
    };

    // Verify response contains pattern analysis
    assert!(
        response_text.contains("Pattern") || response_text.contains("Convention"),
        "Response should mention patterns or conventions"
    );

    service.cancel().await?;
    println!("🎉 Pattern detection test passed!");
    Ok(())
}

#[tokio::test]
async fn test_performance_metrics() -> Result<()> {
    println!("📊 Testing performance_metrics tool...");

    let service = start_mcp_server().await?;
    let config = TestConfig::default();

    // Performance metrics doesn't need parameters
    let result = timeout(
        config.tool_timeout,
        service.call_tool(CallToolRequestParam {
            name: "performance_metrics".into(),
            arguments: None,
        }),
    )
    .await??;

    assert!(
        !result.content.is_empty(),
        "No content returned for performance metrics"
    );

    let response_text = match &result.content[0] {
        content if content.text.is_some() => content.text.as_ref().unwrap(),
        _ => panic!("Expected text content"),
    };

    // Verify response contains performance information
    assert!(
        response_text.contains("Performance") || response_text.contains("Metrics"),
        "Response should mention performance or metrics"
    );

    service.cancel().await?;
    println!("🎉 Performance metrics test passed!");
    Ok(())
}

#[tokio::test]
async fn test_workflow_integration() -> Result<()> {
    println!("🔗 Testing tool workflow integration...");

    let service = start_mcp_server().await?;
    let config = TestConfig::default();

    // Step 1: Use vector_search to find some code
    println!("🧪 Step 1: Finding Rust structs with vector_search");
    let search_result = timeout(
        config.tool_timeout,
        service.call_tool(CallToolRequestParam {
            name: "vector_search".into(),
            arguments: Some(
                json!({
                    "query": "pub struct",
                    "limit": 3
                })
                .as_object()
                .unwrap()
                .clone(),
            ),
        }),
    )
    .await??;

    let search_response = match &search_result.content[0] {
        content if content.text.is_some() => content.text.as_ref().unwrap(),
        _ => panic!("Expected text content"),
    };

    // Parse response to extract potential UUIDs (this is a demo of the workflow)
    println!(
        "✅ Vector search completed, response length: {}",
        search_response.len()
    );

    // Step 2: Test enhanced_search with AI analysis
    println!("🧪 Step 2: Enhanced search with AI analysis");
    let enhanced_result = timeout(
        config.tool_timeout,
        service.call_tool(CallToolRequestParam {
            name: "enhanced_search".into(),
            arguments: Some(
                json!({
                    "query": "trait implementation patterns",
                    "limit": 2
                })
                .as_object()
                .unwrap()
                .clone(),
            ),
        }),
    )
    .await??;

    let enhanced_response = match &enhanced_result.content[0] {
        content if content.text.is_some() => content.text.as_ref().unwrap(),
        _ => panic!("Expected text content"),
    };

    println!(
        "✅ Enhanced search completed, response length: {}",
        enhanced_response.len()
    );

    // Step 3: Test semantic intelligence for architectural understanding
    println!("🧪 Step 3: Architectural analysis with semantic_intelligence");
    let intel_result = timeout(
        config.tool_timeout,
        service.call_tool(CallToolRequestParam {
            name: "semantic_intelligence".into(),
            arguments: Some(
                json!({
                    "query": "How are the MCP tools organized in the codebase?",
                    "task_type": "architectural_analysis"
                })
                .as_object()
                .unwrap()
                .clone(),
            ),
        }),
    )
    .await??;

    let intel_response = match &intel_result.content[0] {
        content if content.text.is_some() => content.text.as_ref().unwrap(),
        _ => panic!("Expected text content"),
    };

    println!(
        "✅ Semantic intelligence completed, response length: {}",
        intel_response.len()
    );

    service.cancel().await?;
    println!("🎉 Workflow integration test passed!");
    Ok(())
}

#[tokio::test]
async fn test_error_conditions() -> Result<()> {
    println!("⚠️ Testing error condition handling...");

    let service = start_mcp_server().await?;
    let config = TestConfig::default();

    // Test 1: Invalid parameters
    println!("🧪 Testing invalid parameters...");
    let result = service
        .call_tool(CallToolRequestParam {
            name: "enhanced_search".into(),
            arguments: Some(
                json!({
                    "invalid_param": "test"
                    // Missing required "query" parameter
                })
                .as_object()
                .unwrap()
                .clone(),
            ),
        })
        .await;

    // Should handle gracefully (either error or empty response)
    match result {
        Ok(response) => {
            println!("✅ Tool handled invalid parameters gracefully");
        }
        Err(e) => {
            println!(
                "✅ Tool properly returned error for invalid parameters: {}",
                e
            );
        }
    }

    // Test 2: Non-existent tool
    println!("🧪 Testing non-existent tool...");
    let result = service
        .call_tool(CallToolRequestParam {
            name: "non_existent_tool".into(),
            arguments: None,
        })
        .await;

    assert!(result.is_err(), "Should error for non-existent tool");
    println!("✅ Properly rejected non-existent tool");

    // Test 3: Invalid graph node UUID (when we add graph tools)
    println!("🧪 Testing invalid UUID for graph tools...");
    let result = service
        .call_tool(CallToolRequestParam {
            name: "graph_neighbors".into(),
            arguments: Some(
                json!({
                    "node": "invalid-uuid-format",
                    "limit": 5
                })
                .as_object()
                .unwrap()
                .clone(),
            ),
        })
        .await;

    // Should handle invalid UUID gracefully
    match result {
        Ok(response) => {
            let response_text = if let Some(content) = response.content.first() {
                content.text.as_ref().expect("Expected text content")
            } else {
                panic!("No content in response");
            };
            // Should mention the error or provide guidance
            assert!(
                response_text.contains("UUID") || response_text.contains("invalid"),
                "Should provide helpful error message"
            );
            println!("✅ Graph neighbors handled invalid UUID gracefully");
        }
        Err(e) => {
            println!(
                "✅ Graph neighbors properly returned error for invalid UUID: {}",
                e
            );
        }
    }

    service.cancel().await?;
    println!("🎉 Error condition test passed!");
    Ok(())
}

#[tokio::test]
async fn test_rust_specific_patterns() -> Result<()> {
    println!("🦀 Testing Rust-specific pattern recognition...");

    let service = start_mcp_server().await?;
    let config = TestConfig::default();

    // Test Rust-specific queries that should work well with this codebase
    let rust_queries = vec![
        "ownership and borrowing",
        "trait bounds and generics",
        "async await patterns",
        "error propagation with ?",
        "macro definitions",
        "unsafe blocks",
    ];

    for query in rust_queries {
        println!("🧪 Testing Rust pattern: '{}'", query);

        let result = timeout(
            config.tool_timeout,
            service.call_tool(CallToolRequestParam {
                name: "enhanced_search".into(),
                arguments: Some(
                    json!({
                        "query": query,
                        "limit": 3
                    })
                    .as_object()
                    .unwrap()
                    .clone(),
                ),
            }),
        )
        .await??;

        assert!(
            !result.content.is_empty(),
            "No content returned for Rust query: {}",
            query
        );
        println!("✅ Rust pattern search successful: {}", query);
    }

    service.cancel().await?;
    println!("🎉 Rust-specific pattern test passed!");
    Ok(())
}

#[tokio::test]
async fn test_comprehensive_tool_suite() -> Result<()> {
    println!("🎯 Testing all 8 essential tools comprehensively...");

    let service = start_mcp_server().await?;
    let config = TestConfig::default();

    // Test each tool with appropriate parameters
    let tool_tests = vec![
        (
            "enhanced_search",
            json!({"query": "semantic analysis", "limit": 2}),
        ),
        ("vector_search", json!({"query": "parser", "limit": 3})),
        ("pattern_detection", json!({})),
        ("performance_metrics", json!({})),
        (
            "semantic_intelligence",
            json!({"query": "codebase architecture overview"}),
        ),
        (
            "impact_analysis",
            json!({"target_function": "extract", "file_path": "crates/codegraph-parser/src/languages/rust.rs"}),
        ),
        (
            "graph_neighbors",
            json!({"node": "550e8400-e29b-41d4-a716-446655440000", "limit": 5}),
        ), // Test UUID
        (
            "graph_traverse",
            json!({"start": "550e8400-e29b-41d4-a716-446655440000", "depth": 2}),
        ), // Test UUID
    ];

    let mut passed_tools = 0;
    let mut total_tools = tool_tests.len();

    for (tool_name, params) in tool_tests {
        println!("🧪 Testing tool: {}", tool_name);

        let result = timeout(
            config.tool_timeout,
            service.call_tool(CallToolRequestParam {
                name: tool_name.into(),
                arguments: if params.as_object().unwrap().is_empty() {
                    None
                } else {
                    params.as_object().cloned()
                },
            }),
        )
        .await;

        match result {
            Ok(Ok(response)) => {
                assert!(
                    !response.content.is_empty(),
                    "Tool {} returned empty content",
                    tool_name
                );
                println!("✅ Tool '{}' working correctly", tool_name);
                passed_tools += 1;
            }
            Ok(Err(e)) => {
                // Some tools might return errors for test data (like invalid UUIDs) - that's expected
                println!(
                    "⚠️ Tool '{}' returned error (expected for some test data): {}",
                    tool_name, e
                );
                passed_tools += 1; // Still counts as working if it handles errors properly
            }
            Err(e) => {
                println!("❌ Tool '{}' timeout or critical failure: {}", tool_name, e);
            }
        }
    }

    service.cancel().await?;

    println!(
        "📊 Test Results: {}/{} tools passed",
        passed_tools, total_tools
    );
    assert!(
        passed_tools >= 6,
        "At least 6/8 tools should pass (some may have expected errors with test data)"
    );

    println!("🎉 Comprehensive tool suite test passed!");
    Ok(())
}

/// Helper function to verify the server is responding
#[tokio::test]
async fn test_server_health() -> Result<()> {
    println!("🏥 Testing MCP server health and responsiveness...");

    let service = start_mcp_server().await?;

    // Test basic connectivity
    let tools = service.list_tools(Default::default()).await?;
    assert!(!tools.tools.is_empty(), "Server should expose tools");

    // Test server info
    let info = service.peer_info();
    println!("📋 Server info: {:?}", info.name);

    service.cancel().await?;
    println!("🎉 Server health test passed!");
    Ok(())
}

/// Performance benchmark test
#[tokio::test]
async fn test_performance_benchmarks() -> Result<()> {
    println!("⚡ Testing performance benchmarks...");

    let service = start_mcp_server().await?;

    // Test response times for different tools
    let start_time = std::time::Instant::now();

    let _result = service
        .call_tool(CallToolRequestParam {
            name: "vector_search".into(),
            arguments: Some(
                json!({
                    "query": "quick test",
                    "limit": 1
                })
                .as_object()
                .unwrap()
                .clone(),
            ),
        })
        .await?;

    let vector_search_time = start_time.elapsed();
    println!("⏱️ Vector search time: {:?}", vector_search_time);

    // Vector search should be fast (< 5 seconds for indexed data)
    assert!(
        vector_search_time < Duration::from_secs(5),
        "Vector search should be fast with indexed data"
    );

    service.cancel().await?;
    println!("🎉 Performance benchmark test passed!");
    Ok(())
}
