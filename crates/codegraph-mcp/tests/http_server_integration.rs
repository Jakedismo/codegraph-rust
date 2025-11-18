//! Integration test for HTTP server with MCP protocol
#![cfg(feature = "server-http")]

use codegraph_mcp::{
    http_config::HttpServerConfig, http_server::start_http_server,
    official_server::CodeGraphMCPServer,
};
use serde_json::json;
use tokio::time::{sleep, Duration};

#[tokio::test]
async fn test_http_server_health_check() {
    // Start server in background
    let server = CodeGraphMCPServer::new();
    let config = HttpServerConfig {
        host: "127.0.0.1".to_string(),
        port: 13000, // Use non-standard port for testing
        keep_alive_seconds: 5,
    };

    let server_handle = tokio::spawn(async move {
        if let Err(e) = start_http_server(server, config).await {
            eprintln!("Server error: {}", e);
        }
    });

    // Wait for server to start
    sleep(Duration::from_millis(100)).await;

    // Test health endpoint
    let client = reqwest::Client::new();
    let response = client
        .get("http://127.0.0.1:13000/health")
        .send()
        .await
        .expect("Failed to send health check request");

    assert_eq!(response.status(), 200);
    assert_eq!(response.text().await.unwrap(), "OK");

    // Cleanup
    server_handle.abort();
}

#[tokio::test]
async fn test_http_mcp_initialize_request() {
    // Start server in background
    let server = CodeGraphMCPServer::new();
    let config = HttpServerConfig {
        host: "127.0.0.1".to_string(),
        port: 13001,
        keep_alive_seconds: 5,
    };

    let server_handle = tokio::spawn(async move {
        if let Err(e) = start_http_server(server, config).await {
            eprintln!("Server error: {}", e);
        }
    });

    sleep(Duration::from_millis(100)).await;

    // Send initialize request
    let client = reqwest::Client::new();
    let initialize_request = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": "2025-06-18",
            "capabilities": {},
            "clientInfo": {
                "name": "test-client",
                "version": "1.0.0"
            }
        }
    });

    let response = client
        .post("http://127.0.0.1:13001/mcp")
        .header("Content-Type", "application/json")
        .json(&initialize_request)
        .send()
        .await
        .expect("Failed to send initialize request");

    // Should get SSE stream with session ID
    assert!(response.headers().contains_key("mcp-session-id"));
    assert_eq!(
        response.headers().get("content-type").unwrap(),
        "text/event-stream"
    );

    // Cleanup
    server_handle.abort();
}
