// ABOUTME: Status and metrics structures for the watch daemon
// ABOUTME: Tracks daemon state, session metrics, and health information

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use super::health::CircuitState;

/// Current daemon state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DaemonState {
    Stopped,
    Starting,
    Running,
    Stopping,
    Error,
}

impl std::fmt::Display for DaemonState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DaemonState::Stopped => write!(f, "Stopped"),
            DaemonState::Starting => write!(f, "Starting"),
            DaemonState::Running => write!(f, "Running"),
            DaemonState::Stopping => write!(f, "Stopping"),
            DaemonState::Error => write!(f, "Error"),
        }
    }
}

/// Session metrics for tracking daemon activity
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SessionMetrics {
    pub batches_processed: u64,
    pub files_indexed: u64,
    pub files_deleted: u64,
    pub errors: u64,
    pub last_indexed: Option<DateTime<Utc>>,
}

impl SessionMetrics {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn record_batch(&mut self, indexed: u64, deleted: u64) {
        self.batches_processed += 1;
        self.files_indexed += indexed;
        self.files_deleted += deleted;
        self.last_indexed = Some(Utc::now());
    }

    pub fn record_error(&mut self) {
        self.errors += 1;
    }
}

/// Complete daemon status snapshot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DaemonStatus {
    /// Current state
    pub state: DaemonState,

    /// Process ID
    pub pid: Option<u32>,

    /// Start time
    pub started_at: Option<DateTime<Utc>>,

    /// Project being watched
    pub project_root: PathBuf,

    /// Files being watched
    pub files_watched: usize,

    /// Session metrics
    pub metrics: SessionMetrics,

    /// SurrealDB connection status
    pub surrealdb_connected: bool,

    /// Circuit breaker state
    pub circuit_state: CircuitState,
}

impl DaemonStatus {
    pub fn new(project_root: PathBuf) -> Self {
        Self {
            state: DaemonState::Stopped,
            pid: None,
            started_at: None,
            project_root,
            files_watched: 0,
            metrics: SessionMetrics::new(),
            surrealdb_connected: false,
            circuit_state: CircuitState::Closed,
        }
    }

    pub fn uptime(&self) -> Option<chrono::Duration> {
        self.started_at.map(|start| Utc::now() - start)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_metrics_record_batch() {
        let mut metrics = SessionMetrics::new();
        assert_eq!(metrics.batches_processed, 0);

        metrics.record_batch(5, 2);
        assert_eq!(metrics.batches_processed, 1);
        assert_eq!(metrics.files_indexed, 5);
        assert_eq!(metrics.files_deleted, 2);
        assert!(metrics.last_indexed.is_some());
    }

    #[test]
    fn test_daemon_state_display() {
        assert_eq!(DaemonState::Running.to_string(), "Running");
        assert_eq!(DaemonState::Stopped.to_string(), "Stopped");
    }
}
