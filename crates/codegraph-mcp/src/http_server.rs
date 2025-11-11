// ABOUTME: HTTP server implementation using rmcp StreamableHttpService
// ABOUTME: Provides session-based HTTP transport with SSE streaming for progress notifications

#[cfg(feature = "server-http")]
use crate::http_config::HttpServerConfig;
use crate::official_server::CodeGraphMCPServer;
use axum::Router;
use rmcp::transport::streamable_http_server::{
    session::local::LocalSessionManager, StreamableHttpService, StreamableHttpServerConfig,
};
use std::net::SocketAddr;
use std::sync::Arc;
use tracing::info;

/// Start HTTP server with CodeGraph MCP service
pub async fn start_http_server(
    server: CodeGraphMCPServer,
    config: HttpServerConfig,
) -> Result<(), Box<dyn std::error::Error>> {
    info!("Starting HTTP server at {}", config.bind_address());

    // Create session manager for stateful HTTP connections
    let session_manager = Arc::new(LocalSessionManager::default());

    // Clone server for the factory closure
    let server_clone = server.clone();

    // Create streamable HTTP service with CodeGraph MCP server factory
    let http_service = StreamableHttpService::new(
        move || Ok(server_clone.clone()),
        session_manager,
        StreamableHttpServerConfig {
            sse_keep_alive: Some(std::time::Duration::from_secs(config.keep_alive_seconds)),
            stateful_mode: true,
        },
    );

    // Build Axum router with MCP endpoints
    let app = Router::new()
        .route("/mcp", axum::routing::post(handle_mcp_request))
        .route("/sse", axum::routing::get(handle_sse_stream))
        .route("/health", axum::routing::get(health_check))
        .with_state(http_service);

    // Parse bind address
    let addr: SocketAddr = config
        .bind_address()
        .parse()
        .map_err(|e| format!("Invalid bind address: {}", e))?;

    info!("CodeGraph MCP HTTP server listening on http://{}", addr);
    info!("Endpoints:");
    info!("  POST http://{}/mcp - Send MCP requests", addr);
    info!("  GET  http://{}/sse - Connect to SSE stream (requires Mcp-Session-Id header)", addr);
    info!("  GET  http://{}/health - Health check", addr);

    // Start server
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

/// Health check endpoint
async fn health_check() -> &'static str {
    "OK"
}

/// Handle MCP POST requests
async fn handle_mcp_request(
    axum::extract::State(service): axum::extract::State<
        StreamableHttpService<CodeGraphMCPServer, LocalSessionManager>,
    >,
    headers: axum::http::HeaderMap,
    body: axum::body::Bytes,
) -> axum::response::Response {
    use axum::response::IntoResponse;
    use http_body_util::Full;
    use tower::ServiceExt;
    use tracing::warn;

    // Build HTTP request for Tower service
    let mut builder = axum::http::Request::builder()
        .method(axum::http::Method::POST)
        .uri("/mcp")
        .header(axum::http::header::CONTENT_TYPE, "application/json")
        .header(
            axum::http::header::ACCEPT,
            "application/json, text/event-stream",
        );

    // Forward session ID header if present
    if let Some(session_id) = headers.get("Mcp-Session-Id") {
        builder = builder.header("Mcp-Session-Id", session_id);
    }

    // Create request with body
    let http_request = match builder.body(Full::new(body)) {
        Ok(req) => req,
        Err(e) => {
            warn!("Failed to build HTTP request: {}", e);
            return (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                format!("Request build error: {}", e),
            )
                .into_response();
        }
    };

    // Call Tower service using oneshot
    match service.oneshot(http_request).await {
        Ok(response) => {
            // Response is already in the correct format (BoxBody)
            // Just need to convert it to an Axum response
            let (parts, body) = response.into_parts();
            axum::http::Response::from_parts(parts, body).into_response()
        }
        Err(e) => {
            warn!("Service call failed: {:?}", e);
            (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                format!("Service error: {:?}", e),
            )
                .into_response()
        }
    }
}

/// Handle SSE streaming connections (reconnection support)
async fn handle_sse_stream(
    axum::extract::State(service): axum::extract::State<
        StreamableHttpService<CodeGraphMCPServer, LocalSessionManager>,
    >,
    headers: axum::http::HeaderMap,
) -> axum::response::Response {
    use axum::response::IntoResponse;
    use http_body_util::Empty;
    use tower::ServiceExt;
    use tracing::warn;

    // Extract session ID (REQUIRED for SSE reconnection)
    let session_id = match headers.get("Mcp-Session-Id").and_then(|v| v.to_str().ok()) {
        Some(sid) => sid,
        None => {
            warn!("SSE connection missing Mcp-Session-Id header");
            return (
                axum::http::StatusCode::BAD_REQUEST,
                "Missing Mcp-Session-Id header",
            )
                .into_response();
        }
    };

    // Build HTTP request for Tower service
    let mut builder = axum::http::Request::builder()
        .method(axum::http::Method::GET)
        .uri("/sse")
        .header("Mcp-Session-Id", session_id);

    // Extract Last-Event-Id for resumption (optional)
    if let Some(last_event_id) = headers.get("Last-Event-Id") {
        builder = builder.header("Last-Event-Id", last_event_id);
    }

    // Create request with empty body for GET
    let http_request = match builder.body(Empty::<axum::body::Bytes>::new()) {
        Ok(req) => req,
        Err(e) => {
            warn!("Failed to build SSE request: {}", e);
            return (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                format!("Request build error: {}", e),
            )
                .into_response();
        }
    };

    // Call Tower service using oneshot
    match service.oneshot(http_request).await {
        Ok(response) => {
            // Response is already in the correct format (BoxBody)
            // Just need to convert it to an Axum response
            let (parts, body) = response.into_parts();
            axum::http::Response::from_parts(parts, body).into_response()
        }
        Err(e) => {
            warn!("SSE service call failed: {:?}", e);
            (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                format!("Service error: {:?}", e),
            )
                .into_response()
        }
    }
}
