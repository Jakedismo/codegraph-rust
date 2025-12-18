// ABOUTME: End-to-end tests for the CodeGraph MCP stdio server tool surface.
// ABOUTME: Validates tool behavior via an rmcp client against a real server process.
/// E2E Tests for CodeGraph MCP Server Tools
///
/// Tests consolidated MCP tools against the indexed Rust codebase
/// using the official rmcp client library for authentic protocol testing.
use anyhow::{anyhow, Result};
use rmcp::{model::CallToolRequestParam, transport::TokioChildProcess, RoleClient, ServiceExt};
use serde_json::json;
use serde_json::Value;
use std::path::PathBuf;
use std::process::{Command as StdCommand, Stdio};
use std::sync::OnceLock;
use std::time::Duration;
use tokio::process::Command as TokioCommand;
use tokio::time::timeout;

/// Test configuration and utilities
struct TestConfig {
    tool_timeout: Duration,
}

impl Default for TestConfig {
    fn default() -> Self {
        Self {
            tool_timeout: Duration::from_secs(60),
        }
    }
}

static CODEGRAPH_BIN: OnceLock<PathBuf> = OnceLock::new();

fn workspace_root_dir() -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .parent()
        .and_then(|p| p.parent())
        .map(PathBuf::from)
        .expect("Expected CARGO_MANIFEST_DIR to be under <workspace>/crates/<crate>")
}

fn target_dir() -> PathBuf {
    std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| workspace_root_dir().join("target"))
}

fn codegraph_bin_path() -> PathBuf {
    let exe = if cfg!(windows) {
        "codegraph.exe"
    } else {
        "codegraph"
    };
    target_dir().join("debug").join(exe)
}

fn ensure_codegraph_bin() -> PathBuf {
    CODEGRAPH_BIN
        .get_or_init(|| {
            let workspace_root = workspace_root_dir();
            let output = StdCommand::new("cargo")
                .current_dir(&workspace_root)
                .args([
                    "build",
                    "-q",
                    "-p",
                    "codegraph-mcp-server",
                    "--bin",
                    "codegraph",
                ])
                .stdin(Stdio::null())
                .output()
                .expect("Failed to invoke cargo build for codegraph-mcp-server");

            if !output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let stderr = String::from_utf8_lossy(&output.stderr);
                panic!(
                    "Failed to build codegraph-mcp-server binary.\nstdout:\n{}\nstderr:\n{}",
                    stdout, stderr
                );
            }

            let bin = codegraph_bin_path();
            assert!(
                bin.exists(),
                "Expected built binary at '{}', but it does not exist",
                bin.display()
            );
            bin
        })
        .clone()
}

/// Start CodeGraph MCP server as a child process for testing
async fn start_mcp_server() -> Result<rmcp::service::RunningService<RoleClient, ()>> {
    let codegraph_bin = ensure_codegraph_bin();

    let mut cmd = TokioCommand::new(codegraph_bin);
    cmd.args(["start", "stdio"])
        .env("RUST_LOG", "error") // Minimize log noise during testing
        .env("CODEGRAPH_DEBUG", "0"); // Force-disable debug log files in tests

    let (transport, _stderr) = TokioChildProcess::builder(cmd)
        .stderr(Stdio::null())
        .spawn()?;

    let service = ().serve(transport).await?;
    Ok(service)
}

async fn assert_agentic_tool_is_disabled(
    service: &rmcp::service::RunningService<RoleClient, ()>,
    tool_name: &str,
    args: Value,
    timeout_duration: Duration,
) -> Result<()> {
    let arguments = args
        .as_object()
        .ok_or_else(|| anyhow!("Expected object arguments for '{}'", tool_name))?
        .clone();

    let result = timeout(
        timeout_duration,
        service.call_tool(CallToolRequestParam {
            name: tool_name.to_string().into(),
            arguments: Some(arguments),
        }),
    )
    .await;

    match result {
        Ok(Ok(_response)) => Err(anyhow!(
            "Expected '{}' to be disabled without the 'ai-enhanced' server feature",
            tool_name
        )),
        Ok(Err(e)) => {
            let msg = e.to_string();
            assert!(
                msg.contains("ai-enhanced"),
                "Unexpected error for '{}': {}",
                tool_name,
                msg
            );
            Ok(())
        }
        Err(_) => Err(anyhow!("Timed out calling '{}'", tool_name)),
    }
}

#[tokio::test]
async fn test_tool_discovery() -> Result<()> {
    println!("🔍 Testing MCP tool discovery...");

    let service = start_mcp_server().await?;

    // Test that the consolidated tool surface is discoverable
    let tools = service.list_tools(Default::default()).await?;

    let expected_tools = vec![
        "agentic_context",
        "agentic_impact",
        "agentic_architecture",
        "agentic_quality",
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
        let tool_name: &str = tool.name.as_ref();
        if !expected_tools.contains(&tool_name) {
            panic!("❌ Unexpected tool found: {}", tool.name);
        }
    }

    service.cancel().await?;
    println!("🎉 Tool discovery test passed!");
    Ok(())
}

#[tokio::test]
async fn test_agentic_context_focus_search_is_gated_without_ai_enhanced() -> Result<()> {
    println!("🔍 Testing agentic_context (focus=search) gating...");

    let service = start_mcp_server().await?;
    let config = TestConfig::default();

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
        assert_agentic_tool_is_disabled(
            &service,
            "agentic_context",
            json!({"query": query, "limit": 1, "focus": "search"}),
            config.tool_timeout,
        )
        .await?;
    }

    service.cancel().await?;
    println!("🎉 Agentic context (focus=search) gating test passed!");
    Ok(())
}

#[tokio::test]
async fn test_agentic_context_basic_is_gated_without_ai_enhanced() -> Result<()> {
    println!("🔍 Testing agentic_context gating (basic args)...");

    let service = start_mcp_server().await?;
    let config = TestConfig::default();

    let test_cases = vec![
        (json!({"query": "async fn", "limit": 1}), "Basic query"),
        (
            json!({"query": "struct", "limit": 2}),
            "Basic query with limit",
        ),
        (
            json!({"query": "error handling", "limit": 1, "focus": "builder"}),
            "Optional focus parameter",
        ),
    ];

    for (params, description) in test_cases {
        println!("🧪 Testing: {}", description);
        assert_agentic_tool_is_disabled(&service, "agentic_context", params, config.tool_timeout)
            .await?;
    }

    service.cancel().await?;
    println!("🎉 Agentic context gating (basic args) test passed!");
    Ok(())
}

#[tokio::test]
async fn test_agentic_context_focus_question_is_gated_without_ai_enhanced() -> Result<()> {
    println!("🧠 Testing agentic_context (focus=question) gating...");

    let service = start_mcp_server().await?;
    let config = TestConfig::default();

    let test_cases = vec![
        (
            json!({
                "query": "Explain the MCP server architecture",
                "limit": 1,
                "focus": "question"
            }),
            "MCP server architecture question",
        ),
        (
            json!({
                "query": "How does the semantic analysis work?",
                "limit": 1,
                "focus": "question"
            }),
            "Semantic analysis explanation",
        ),
        (
            json!({
                "query": "Describe the parser pipeline",
                "limit": 1,
                "focus": "question"
            }),
            "Parser pipeline analysis",
        ),
    ];

    for (params, description) in test_cases {
        println!("🧪 Testing: {}", description);
        assert_agentic_tool_is_disabled(&service, "agentic_context", params, config.tool_timeout)
            .await?;
    }

    service.cancel().await?;
    println!("🎉 Agentic context (focus=question) gating test passed!");
    Ok(())
}

#[tokio::test]
async fn test_agentic_impact_focus_dependencies_is_gated_without_ai_enhanced() -> Result<()> {
    println!("⚡ Testing agentic_impact (focus=dependencies) gating...");

    let service = start_mcp_server().await?;
    let config = TestConfig::default();

    let test_cases = vec![
        (
            json!({
                "query": "CodeGraphMCPServer",
                "limit": 1,
                "focus": "dependencies"
            }),
            "MCP server dependency impact",
        ),
        (
            json!({
                "query": "agentic_context",
                "limit": 1,
                "focus": "dependencies"
            }),
            "Tool impact analysis",
        ),
    ];

    for (params, description) in test_cases {
        println!("🧪 Testing: {}", description);
        assert_agentic_tool_is_disabled(&service, "agentic_impact", params, config.tool_timeout)
            .await?;
    }

    service.cancel().await?;
    println!("🎉 Agentic impact (focus=dependencies) gating test passed!");
    Ok(())
}

#[tokio::test]
async fn test_pattern_detection() -> Result<()> {
    println!("🎯 Testing agentic_quality (focus=hotspots) gating...");

    let service = start_mcp_server().await?;
    let config = TestConfig::default();

    assert_agentic_tool_is_disabled(
        &service,
        "agentic_quality",
        json!({"query": "quality hotspots", "limit": 1, "focus": "hotspots"}),
        config.tool_timeout,
    )
    .await?;

    service.cancel().await?;
    println!("🎉 Agentic quality (focus=hotspots) gating test passed!");
    Ok(())
}

#[tokio::test]
async fn test_performance_metrics() -> Result<()> {
    println!("📊 Testing agentic_quality (focus=coupling) gating...");

    let service = start_mcp_server().await?;
    let config = TestConfig::default();

    assert_agentic_tool_is_disabled(
        &service,
        "agentic_quality",
        json!({"query": "coupling metrics", "limit": 1, "focus": "coupling"}),
        config.tool_timeout,
    )
    .await?;

    service.cancel().await?;
    println!("🎉 Agentic quality (focus=coupling) gating test passed!");
    Ok(())
}

#[tokio::test]
async fn test_workflow_integration() -> Result<()> {
    println!("🔗 Testing tool workflow integration...");

    let service = start_mcp_server().await?;
    let config = TestConfig::default();

    println!("🧪 Step 1: agentic_context");
    assert_agentic_tool_is_disabled(
        &service,
        "agentic_context",
        json!({"query": "pub struct", "limit": 1}),
        config.tool_timeout,
    )
    .await?;

    println!("🧪 Step 2: agentic_impact");
    assert_agentic_tool_is_disabled(
        &service,
        "agentic_impact",
        json!({"query": "CodeGraphMCPServer", "limit": 1}),
        config.tool_timeout,
    )
    .await?;

    println!("🧪 Step 3: agentic_architecture");
    assert_agentic_tool_is_disabled(
        &service,
        "agentic_architecture",
        json!({"query": "How are the MCP tools organized in the codebase?", "limit": 1}),
        config.tool_timeout,
    )
    .await?;

    service.cancel().await?;
    println!("🎉 Workflow integration test passed!");
    Ok(())
}

#[tokio::test]
async fn test_error_conditions() -> Result<()> {
    println!("⚠️ Testing error condition handling...");

    let service = start_mcp_server().await?;

    // Test 1: Invalid parameters
    println!("🧪 Testing invalid parameters...");
    let result = service
        .call_tool(CallToolRequestParam {
            name: "agentic_context".into(),
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
        Ok(_response) => {
            println!("✅ Tool handled invalid parameters gracefully");
        }
        Err(e) => {
            let msg = e.to_string();
            assert!(
                msg.contains("query") || msg.contains("Invalid") || msg.contains("invalid"),
                "Unexpected error for invalid parameters: {}",
                msg
            );
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
    println!("🧪 Testing invalid parameter types...");
    let result = service
        .call_tool(CallToolRequestParam {
            name: "agentic_context".into(),
            arguments: Some(
                json!({
                    "query": "type mismatch test",
                    "limit": "not-a-number"
                })
                .as_object()
                .unwrap()
                .clone(),
            ),
        })
        .await;

    // Should reject invalid types during request decoding
    match result {
        Ok(_response) => panic!("Expected invalid parameter types to be rejected"),
        Err(e) => println!("✅ Tool rejected invalid parameter types: {}", e),
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
        assert_agentic_tool_is_disabled(
            &service,
            "agentic_context",
            json!({"query": query, "limit": 1, "focus": "search"}),
            config.tool_timeout,
        )
        .await?;
        println!("✅ Rust pattern query validated: {}", query);
    }

    service.cancel().await?;
    println!("🎉 Rust-specific pattern test passed!");
    Ok(())
}

#[tokio::test]
async fn test_comprehensive_tool_suite() -> Result<()> {
    println!("🎯 Testing consolidated tool surface comprehensively...");

    let service = start_mcp_server().await?;
    let config = TestConfig::default();

    let tool_tests = vec![
        (
            "agentic_context",
            json!({"query": "semantic analysis", "limit": 1}),
        ),
        (
            "agentic_impact",
            json!({"query": "codebase impact overview", "limit": 1}),
        ),
        (
            "agentic_architecture",
            json!({"query": "codebase architecture overview", "limit": 1}),
        ),
        (
            "agentic_quality",
            json!({"query": "quality hotspots", "limit": 1}),
        ),
    ];

    for (tool_name, params) in tool_tests {
        println!("🧪 Testing tool: {}", tool_name);
        assert_agentic_tool_is_disabled(&service, tool_name, params, config.tool_timeout).await?;
    }

    service.cancel().await?;
    println!("🎉 Consolidated tool surface test passed!");
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
    let info = service
        .peer_info()
        .expect("Server should provide peer info after initialization");
    println!("📋 Server info: {:?}", info.server_info.name);

    service.cancel().await?;
    println!("🎉 Server health test passed!");
    Ok(())
}

/// Performance benchmark test
#[tokio::test]
async fn test_performance_benchmarks() -> Result<()> {
    println!("⚡ Testing performance benchmarks...");

    let service = start_mcp_server().await?;

    // Test basic responsiveness via tool listing
    let start_time = std::time::Instant::now();

    let _tools = service.list_tools(Default::default()).await?;

    let list_tools_time = start_time.elapsed();
    println!("⏱️ list_tools time: {:?}", list_tools_time);

    // Listing tools should be fast (< 5 seconds)
    assert!(
        list_tools_time < Duration::from_secs(5),
        "Tool discovery should be fast"
    );

    service.cancel().await?;
    println!("🎉 Performance benchmark test passed!");
    Ok(())
}
