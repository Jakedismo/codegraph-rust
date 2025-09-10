use crate::{create_router, AppState};
use codegraph_core::Result;
use std::net::SocketAddr;
use tokio::signal;
use tower::ServiceBuilder;
use tracing::{info, warn};

pub struct Server {
    state: AppState,
    addr: SocketAddr,
}

impl Server {
    pub async fn new(addr: SocketAddr) -> Result<Self> {
        crate::metrics::register_metrics();
        let state = AppState::new().await?;
        Ok(Self { state, addr })
    }

    pub async fn run(self) -> Result<()> {
        let router = create_router(self.state);

        info!("Starting CodeGraph API server on {}", self.addr);

        let listener = tokio::net::TcpListener::bind(self.addr)
            .await
            .map_err(|e| codegraph_core::CodeGraphError::Io(e))?;

        info!("Server listening on http://{}", self.addr);
        info!("Health check available at http://{}/health", self.addr);
        info!("GraphQL endpoint available at http://{}/graphql", self.addr);
        info!("GraphiQL UI available at http://{}/graphiql", self.addr);
        info!("GraphQL subscriptions over WebSocket at ws://{}/graphql/ws", self.addr);
        info!("API documentation:");
        info!("  GET /health - Health check");
        info!("  POST /parse - Parse source file");
        info!("  GET /nodes/:id - Get node by ID");
        info!("  GET /nodes/:id/similar - Find similar nodes");
        info!("  GET /search?query=<text>&limit=<n> - Search nodes");

        axum::serve(listener, router)
            .with_graceful_shutdown(shutdown_signal())
            .await
            .map_err(|e| codegraph_core::CodeGraphError::Io(e.into()))?;

        Ok(())
    }
}

async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {
            info!("Received Ctrl+C, shutting down gracefully");
        },
        _ = terminate => {
            info!("Received SIGTERM, shutting down gracefully");
        },
    }
}
