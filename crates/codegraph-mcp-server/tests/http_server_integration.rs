// ABOUTME: Integration tests for the HTTP MCP server endpoints.
// ABOUTME: Validates health checks and basic MCP initialization over SSE.

#![cfg(feature = "server-http")]

use codegraph_mcp_server::{
    http_config::HttpServerConfig, http_server::build_http_app,
    official_server::CodeGraphMCPServer,
};
use http_body_util::BodyExt;
use serde_json::json;
use tower::ServiceExt;

#[tokio::test]
async fn test_http_server_health_check() {
    let server = CodeGraphMCPServer::new();
    let config = HttpServerConfig {
        host: "127.0.0.1".to_string(),
        port: 13000, // Use non-standard port for testing
        keep_alive_seconds: 5,
    };

    let app = build_http_app(server, &config);
    let response = app
        .oneshot(
            axum::http::Request::builder()
                .method("GET")
                .uri("/health")
                .body(axum::body::Body::empty())
                .expect("request"),
        )
        .await
        .expect("health response");

    assert_eq!(response.status(), axum::http::StatusCode::OK);
    let body = response
        .into_body()
        .collect()
        .await
        .expect("body")
        .to_bytes();
    assert_eq!(std::str::from_utf8(&body).expect("utf8"), "OK");
}

#[tokio::test]
async fn test_http_mcp_initialize_request() {
    let server = CodeGraphMCPServer::new();
    let config = HttpServerConfig {
        host: "127.0.0.1".to_string(),
        port: 13001,
        keep_alive_seconds: 5,
    };

    let app = build_http_app(server, &config);

    // Send initialize request
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

    let response = app
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/mcp")
                .header("Accept", "application/json, text/event-stream")
                .header("Content-Type", "application/json")
                .body(axum::body::Body::from(initialize_request.to_string()))
                .expect("request"),
        )
        .await
        .expect("initialize response");

    let status = response.status();
    let headers = response.headers().clone();
    let content_type = headers
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    let body = response
        .into_body()
        .collect()
        .await
        .expect("body")
        .to_bytes();
    let body_str = String::from_utf8_lossy(&body);

    if std::env::var("CODEGRAPH_DEBUG_HTTP_TEST").is_ok() {
        eprintln!("status={}", status);
        eprintln!("content-type={}", content_type);
        eprintln!("headers={:?}", headers);
        eprintln!("body={}", body_str);
    }

    assert!(
        status.is_success(),
        "initialize should succeed (status={})",
        status
    );
    assert!(
        !content_type.is_empty(),
        "initialize should set content-type header"
    );

    let has_session_header = headers.contains_key("mcp-session-id");
    let has_session_in_body = body_str.contains("session")
        || body_str.contains("mcp-session-id")
        || body_str.contains("mcpSessionId");

    assert!(
        has_session_header || has_session_in_body,
        "initialize should return a session identifier (header or body)"
    );
}
