//! Basic example of running the CodeGraph API server
//!
//! This example demonstrates how to:
//! - Initialize the API server with configuration
//! - Set up the basic routes
//! - Start the server on a specified port
//!
//! Run with: `cargo run --example basic_server`

use codegraph_api::{create_router, AppState};
use codegraph_core::ConfigManager;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;
use tracing::{info, Level};
use tracing_subscriber;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt().with_max_level(Level::INFO).init();

    info!("Starting CodeGraph API Server example...");

    // Create configuration
    let config = Arc::new(ConfigManager::new()?);
    info!("Configuration loaded");

    // Initialize application state
    let state = AppState::new(config).await?;
    info!("Application state initialized");

    // Create the router with all routes
    let app = create_router(state);
    info!("Routes configured");

    // Define the server address
    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    info!("Server will listen on {}", addr);

    // Create TCP listener
    let listener = TcpListener::bind(addr).await?;
    info!("TCP listener bound successfully");

    // Start the server
    info!("CodeGraph API server is running on http://{}", addr);
    info!("Try these endpoints:");
    info!("  - Health check: http://{}/health", addr);
    info!("  - Metrics: http://{}/metrics", addr);
    info!("  - GraphQL playground: http://{}/graphiql", addr);

    axum::serve(listener, app).await?;

    Ok(())
}
