use crate::{create_router, AppState};
use codegraph_core::Result;
use std::net::SocketAddr;
use tokio::signal;
use tower::ServiceBuilder;
use tracing::{info, warn};
use std::time::Duration;
use codegraph_core::ConfigManager;
use std::sync::Arc;

pub struct Server {
    state: AppState,
    addr: SocketAddr,
    _config: Arc<ConfigManager>,
}

impl Server {
    pub async fn new(addr: SocketAddr, config: Arc<ConfigManager>) -> Result<Self> {
        crate::metrics::register_metrics();
        let state = AppState::new(config.clone()).await?;
        // Spawn background task for memory/leak metrics if enabled
        // Spawn background task for metrics collection
        {
            tokio::spawn(async {
                use std::time::Duration;
                loop {
                    // Update all system metrics
                    crate::metrics::update_system_metrics();
                    crate::metrics::update_uptime();
                    
                    #[cfg(feature = "leak-detect")]
                    {
                        crate::metrics::update_memory_metrics();
                        // Optional alerting: warn if any leaks detected
                        let leaked_allocs = crate::metrics::MEM_LEAKED_ALLOCATIONS.get();
                        if leaked_allocs > 0 {
                            warn!(
                                leaked_allocations = leaked_allocs,
                                leaked_bytes = crate::metrics::MEM_LEAKED_BYTES.get(),
                                "Potential memory leaks detected by memscope. See /metrics or /memory/leaks."
                            );
                        }
                    }
                    
                    tokio::time::sleep(Duration::from_secs(30)).await;
                }
            });
        }
        Ok(Self { state, addr, _config: config })
    }

    pub async fn run(self) -> Result<()> {
        let router = create_router(self.state);

        info!("Starting CodeGraph API server on {}", self.addr);

        // Bind with tuned socket options for better keep-alive behavior
        let listener = {
            let socket = if self.addr.is_ipv6() {
                tokio::net::TcpSocket::new_v6()
            } else {
                tokio::net::TcpSocket::new_v4()
            }.map_err(|e| codegraph_core::CodeGraphError::Io(e))?;

            // Reuse addr/port to improve rebind under restarts
            let _ = socket.set_reuseaddr(true);
            #[cfg(unix)]
            let _ = socket.set_reuseport(true);

            // Enable OS-level TCP keepalive (interval platform dependent)
            let _ = socket.set_keepalive(Some(Duration::from_secs(60)));

            socket.bind(self.addr).map_err(|e| codegraph_core::CodeGraphError::Io(e))?;
            socket.listen(1024)?
        };

        info!("Server listening on http://{}", self.addr);
        info!("Health endpoints available:");
        info!("  GET /health - Comprehensive health check");
        info!("  GET /health/live - Liveness probe");
        info!("  GET /health/ready - Readiness probe");
        info!("  GET /metrics - Prometheus metrics");
        info!("GraphQL endpoint available at http://{}/graphql", self.addr);
        info!("GraphiQL UI available at http://{}/graphiql", self.addr);
        info!("GraphQL subscriptions over WebSocket at ws://{}/graphql/ws", self.addr);
        info!("API documentation:");
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
