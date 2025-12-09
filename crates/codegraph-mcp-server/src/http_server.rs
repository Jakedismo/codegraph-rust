#![cfg(feature = "server-http")]
// ABOUTME: HTTP server implementation using rmcp StreamableHttpService
// ABOUTME: Provides session-based HTTP transport with SSE streaming for progress notifications

use crate::http_config::HttpServerConfig;
use crate::official_server::CodeGraphMCPServer;
use axum::Router;
use rmcp::transport::streamable_http_server::{
    session::local::LocalSessionManager, StreamableHttpServerConfig, StreamableHttpService,
};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;
use tracing::info;

/// Start HTTP server with CodeGraph MCP service
pub async fn start_http_server(
    server: CodeGraphMCPServer,
    config: HttpServerConfig,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
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
            cancellation_token: CancellationToken::new(),
        },
    );

    // Build Axum router with MCP service mounted as Tower service
    // IMPORTANT: Use nest_service to mount the StreamableHttpService directly
    // This lets the service handle its own /mcp POST/GET routing internally
    let app = Router::new()
        .nest_service("/mcp", http_service)
        .route("/health", axum::routing::get(health_check));

    // Parse bind address
    let addr: SocketAddr = config
        .bind_address()
        .parse()
        .map_err(|e| format!("Invalid bind address: {}", e))?;

    info!("CodeGraph MCP HTTP server listening on http://{}", addr);
    info!("Endpoints:");
    info!(
        "  POST http://{}/mcp - Initialize session and send MCP requests",
        addr
    );
    info!(
        "  GET  http://{}/mcp - Open SSE stream (requires Mcp-Session-Id header)",
        addr
    );
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
