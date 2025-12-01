// ABOUTME: Watch daemon module for automatic re-indexing of file changes
// ABOUTME: Provides lifecycle management with SurrealDB health monitoring and circuit breaker

pub mod config;
pub mod health;
pub mod manager;
pub mod pid;
pub mod session;
pub mod status;

pub use config::{BackoffConfig, CircuitBreakerConfig, WatchConfig};
pub use health::{CircuitState, HealthMonitor};
pub use manager::{DaemonManager, DaemonManagerState};
pub use pid::PidFile;
pub use session::WatchSession;
pub use status::{DaemonState, DaemonStatus, SessionMetrics};

use anyhow::{Context, Result};
use std::sync::Arc;
use tokio::signal;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

use crate::ProjectIndexer;

/// Watch daemon - manages lifecycle for single project watching
pub struct WatchDaemon {
    /// Configuration
    config: WatchConfig,

    /// Active watch session
    session: Option<WatchSession>,

    /// Health monitor for SurrealDB
    health_monitor: Arc<HealthMonitor>,

    /// Current daemon state
    state: Arc<RwLock<DaemonState>>,

    /// PID file manager
    pid_file: PidFile,

    /// Indexer for re-indexing
    indexer: Option<ProjectIndexer>,
}

impl WatchDaemon {
    /// Create a new daemon with configuration
    pub fn new(config: WatchConfig) -> Result<Self> {
        let pid_path = PidFile::default_path(&config.project_root);
        let pid_file = PidFile::new(pid_path);

        // Clean up stale PID file if present
        pid_file.cleanup_stale()?;

        let health_monitor = Arc::new(HealthMonitor::new(
            config.circuit_breaker.clone(),
            config.reconnect_backoff.clone(),
        ));

        Ok(Self {
            config,
            session: None,
            health_monitor,
            state: Arc::new(RwLock::new(DaemonState::Stopped)),
            pid_file,
            indexer: None,
        })
    }

    /// Set the indexer for re-indexing operations
    pub fn set_indexer(&mut self, indexer: ProjectIndexer) {
        self.indexer = Some(indexer);
    }

    /// Start watching (blocking - runs event loop)
    pub async fn start(&mut self) -> Result<()> {
        // Check if already running
        if self.pid_file.is_process_running()? {
            anyhow::bail!(
                "Daemon already running (PID file: {:?})",
                self.pid_file.path()
            );
        }

        // Update state
        *self.state.write().await = DaemonState::Starting;

        // Write PID file
        self.pid_file.write()?;

        // Create watch session
        let mut session = WatchSession::new(self.config.clone())
            .await
            .context("Failed to create watch session")?;

        // Set indexer on session
        if let Some(indexer) = self.indexer.take() {
            session.set_indexer(indexer);
        }

        self.session = Some(session);

        // Update state to running
        *self.state.write().await = DaemonState::Running;

        info!(
            "Daemon started: watching {:?} ({} files)",
            self.config.project_root,
            self.session.as_ref().map(|s| s.files_tracked()).unwrap_or(0)
        );

        // Run event loop
        self.run_event_loop().await
    }

    /// Stop daemon gracefully
    pub async fn stop(&mut self) -> Result<()> {
        info!("Initiating graceful shutdown...");

        *self.state.write().await = DaemonState::Stopping;

        // Stop the session
        if let Some(ref mut session) = self.session {
            session.stop();
        }

        // Remove PID file
        self.pid_file.remove()?;

        *self.state.write().await = DaemonState::Stopped;

        info!("Graceful shutdown complete");
        Ok(())
    }

    /// Get current status
    pub async fn status(&self) -> DaemonStatus {
        let state = *self.state.read().await;
        let pid = self.pid_file.read().ok().flatten();
        let circuit_state = self.health_monitor.circuit_state().await;

        let (files_watched, metrics) = match &self.session {
            Some(session) => (session.files_tracked(), session.metrics().clone()),
            None => (0, SessionMetrics::new()),
        };

        DaemonStatus {
            state,
            pid,
            started_at: if state == DaemonState::Running {
                Some(chrono::Utc::now())
            } else {
                None
            },
            project_root: self.config.project_root.clone(),
            files_watched,
            metrics,
            surrealdb_connected: circuit_state == CircuitState::Closed,
            circuit_state,
        }
    }

    /// Main event loop
    async fn run_event_loop(&mut self) -> Result<()> {
        let mut ctrl_c = Box::pin(signal::ctrl_c());

        loop {
            tokio::select! {
                // Handle Ctrl+C
                _ = &mut ctrl_c => {
                    info!("Received SIGINT, shutting down...");
                    break;
                }

                // Process file change batches
                batch = async {
                    if let Some(session) = &self.session {
                        session.next_batch().await
                    } else {
                        // No session, wait indefinitely
                        std::future::pending::<Option<codegraph_parser::BatchedChanges>>().await
                    }
                } => {
                    if let Some(batch) = batch {
                        // Check circuit breaker before processing
                        if !self.health_monitor.should_allow_request().await {
                            warn!("Circuit breaker open, skipping batch");
                            continue;
                        }

                        if let Some(ref mut session) = self.session {
                            match session.process_batch(batch).await {
                                Ok((indexed, deleted)) => {
                                    debug!("Batch processed: {} indexed, {} deleted", indexed, deleted);
                                    self.health_monitor.record_success().await;
                                }
                                Err(e) => {
                                    error!("Batch processing failed: {}", e);
                                    self.health_monitor.record_failure().await;
                                }
                            }
                        }
                    }
                }

                // Health check interval
                _ = tokio::time::sleep(std::time::Duration::from_secs(
                    self.config.health_check_interval_secs
                )) => {
                    debug!("Health check: circuit state = {:?}",
                           self.health_monitor.circuit_state().await);
                }
            }
        }

        self.stop().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_daemon_creation() {
        let temp_dir = TempDir::new().unwrap();
        let config = WatchConfig {
            project_root: temp_dir.path().to_path_buf(),
            debounce_ms: 30,
            batch_timeout_ms: 200,
            health_check_interval_secs: 30,
            reconnect_backoff: BackoffConfig::default(),
            circuit_breaker: CircuitBreakerConfig::default(),
            indexer: crate::IndexerConfig::default(),
        };

        let daemon = WatchDaemon::new(config);
        assert!(daemon.is_ok());
    }
}
