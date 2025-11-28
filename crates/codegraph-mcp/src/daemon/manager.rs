// ABOUTME: Daemon manager for MCP server integration
// ABOUTME: Coordinates background daemon startup and shutdown with MCP server lifecycle

use anyhow::{Context, Result};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};
use tokio::task::JoinHandle;
use tracing::{debug, error, info, warn};

use codegraph_core::config_manager::{CodeGraphConfig, DaemonConfig};

use super::{BackoffConfig, CircuitBreakerConfig, WatchConfig, WatchDaemon};
use crate::{IndexerConfig, ProjectIndexer};

/// Manages daemon lifecycle for MCP server integration
pub struct DaemonManager {
    /// Daemon configuration from global config
    config: DaemonConfig,

    /// Global configuration for creating indexer
    global_config: CodeGraphConfig,

    /// Project root to watch
    project_root: PathBuf,

    /// Background task handle
    daemon_handle: Option<JoinHandle<Result<()>>>,

    /// Shutdown signal sender
    shutdown_tx: Option<broadcast::Sender<()>>,

    /// Current manager state
    state: Arc<RwLock<DaemonManagerState>>,
}

/// State of the daemon manager
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DaemonManagerState {
    /// Daemon not started
    Stopped,
    /// Daemon is starting up
    Starting,
    /// Daemon is running
    Running,
    /// Daemon is shutting down
    Stopping,
    /// Daemon failed to start or crashed
    Failed,
}

impl DaemonManager {
    /// Create a new daemon manager
    ///
    /// # Arguments
    /// * `config` - Daemon configuration
    /// * `global_config` - Global CodeGraph configuration (for creating indexer)
    /// * `project_root` - Project root to watch
    pub fn new(
        config: DaemonConfig,
        global_config: CodeGraphConfig,
        project_root: PathBuf,
    ) -> Self {
        Self {
            config,
            global_config,
            project_root,
            daemon_handle: None,
            shutdown_tx: None,
            state: Arc::new(RwLock::new(DaemonManagerState::Stopped)),
        }
    }

    /// Start daemon in background (non-blocking)
    ///
    /// Returns immediately after spawning the daemon task.
    /// The daemon runs in its own tokio task and processes file changes independently.
    pub async fn start_background(&mut self) -> Result<()> {
        // Check if already running
        let current_state = *self.state.read().await;
        if current_state == DaemonManagerState::Running
            || current_state == DaemonManagerState::Starting
        {
            debug!("Daemon already running or starting, skipping");
            return Ok(());
        }

        *self.state.write().await = DaemonManagerState::Starting;

        // Create shutdown channel
        let (shutdown_tx, shutdown_rx) = broadcast::channel::<()>(1);
        self.shutdown_tx = Some(shutdown_tx);

        // Clone what we need for the spawned task
        let config = self.config.clone();
        let global_config = self.global_config.clone();
        let project_root = self.project_root.clone();
        let state = Arc::clone(&self.state);

        // Spawn daemon in background task
        let handle = tokio::spawn(async move {
            match Self::run_daemon(config, global_config, project_root.clone(), shutdown_rx).await {
                Ok(()) => {
                    info!(
                        target: "codegraph::daemon",
                        project = %project_root.display(),
                        "Daemon stopped gracefully"
                    );
                    *state.write().await = DaemonManagerState::Stopped;
                    Ok(())
                }
                Err(e) => {
                    error!(
                        target: "codegraph::daemon",
                        project = %project_root.display(),
                        error = %e,
                        "Daemon failed"
                    );
                    *state.write().await = DaemonManagerState::Failed;
                    Err(e)
                }
            }
        });

        self.daemon_handle = Some(handle);
        *self.state.write().await = DaemonManagerState::Running;

        info!(
            target: "codegraph::daemon",
            project = %self.project_root.display(),
            "Background daemon started"
        );

        Ok(())
    }

    /// Internal daemon execution logic
    async fn run_daemon(
        config: DaemonConfig,
        global_config: CodeGraphConfig,
        project_root: PathBuf,
        mut shutdown_rx: broadcast::Receiver<()>,
    ) -> Result<()> {
        // Create IndexerConfig from DaemonConfig
        let indexer_config = IndexerConfig {
            languages: config.languages.clone(),
            exclude_patterns: config.exclude_patterns.clone(),
            include_patterns: config.include_patterns.clone(),
            project_root: project_root.clone(),
            ..Default::default()
        };

        // Create WatchConfig
        let watch_config = WatchConfig {
            project_root: project_root.clone(),
            debounce_ms: config.debounce_ms,
            batch_timeout_ms: config.batch_timeout_ms,
            health_check_interval_secs: config.health_check_interval_secs,
            reconnect_backoff: BackoffConfig::default(),
            circuit_breaker: CircuitBreakerConfig::default(),
            indexer: indexer_config.clone(),
        };

        // Create ProjectIndexer
        // Use a hidden progress bar for background daemon
        let progress = indicatif::MultiProgress::with_draw_target(
            indicatif::ProgressDrawTarget::hidden(),
        );

        let indexer = ProjectIndexer::new(indexer_config, &global_config, progress)
            .await
            .context("Failed to create project indexer for daemon")?;

        // Create and configure daemon
        let mut daemon =
            WatchDaemon::new(watch_config).context("Failed to create watch daemon")?;
        daemon.set_indexer(indexer);

        // Run daemon with shutdown coordination
        tokio::select! {
            result = daemon.start() => {
                result
            }
            _ = shutdown_rx.recv() => {
                info!(
                    target: "codegraph::daemon",
                    "Received shutdown signal"
                );
                daemon.stop().await
            }
        }
    }

    /// Stop the daemon gracefully
    ///
    /// Sends shutdown signal and waits for daemon task to complete (with timeout).
    pub async fn stop(&mut self) -> Result<()> {
        let current_state = *self.state.read().await;
        if current_state == DaemonManagerState::Stopped {
            debug!("Daemon already stopped");
            return Ok(());
        }

        *self.state.write().await = DaemonManagerState::Stopping;

        // Send shutdown signal
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }

        // Wait for daemon task to complete (with timeout)
        if let Some(handle) = self.daemon_handle.take() {
            match tokio::time::timeout(std::time::Duration::from_secs(5), handle).await {
                Ok(Ok(Ok(()))) => {
                    info!(
                        target: "codegraph::daemon",
                        "Daemon stopped gracefully"
                    );
                }
                Ok(Ok(Err(e))) => {
                    warn!(
                        target: "codegraph::daemon",
                        error = %e,
                        "Daemon stopped with error"
                    );
                }
                Ok(Err(join_error)) => {
                    warn!(
                        target: "codegraph::daemon",
                        error = %join_error,
                        "Daemon task panicked"
                    );
                }
                Err(_) => {
                    warn!(
                        target: "codegraph::daemon",
                        "Daemon shutdown timed out after 5 seconds"
                    );
                }
            }
        }

        *self.state.write().await = DaemonManagerState::Stopped;
        Ok(())
    }

    /// Get current daemon manager state
    pub async fn state(&self) -> DaemonManagerState {
        *self.state.read().await
    }

    /// Check if daemon is enabled in configuration
    pub fn is_enabled(&self) -> bool {
        self.config.auto_start_with_mcp
    }

    /// Get the project root being watched
    pub fn project_root(&self) -> &PathBuf {
        &self.project_root
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_daemon_manager_creation() {
        let temp_dir = TempDir::new().unwrap();
        let config = DaemonConfig {
            auto_start_with_mcp: true,
            project_path: Some(temp_dir.path().to_path_buf()),
            ..Default::default()
        };
        let global_config = CodeGraphConfig::default();

        let manager = DaemonManager::new(config, global_config, temp_dir.path().to_path_buf());

        assert!(manager.is_enabled());
        assert_eq!(manager.project_root(), temp_dir.path());
    }

    #[tokio::test]
    async fn test_daemon_manager_state_transitions() {
        let temp_dir = TempDir::new().unwrap();
        let config = DaemonConfig::default();
        let global_config = CodeGraphConfig::default();

        let manager = DaemonManager::new(config, global_config, temp_dir.path().to_path_buf());

        assert_eq!(manager.state().await, DaemonManagerState::Stopped);
    }

    #[tokio::test]
    async fn test_daemon_manager_disabled() {
        let temp_dir = TempDir::new().unwrap();
        let config = DaemonConfig {
            auto_start_with_mcp: false,
            ..Default::default()
        };
        let global_config = CodeGraphConfig::default();

        let manager = DaemonManager::new(config, global_config, temp_dir.path().to_path_buf());

        assert!(!manager.is_enabled());
    }
}
