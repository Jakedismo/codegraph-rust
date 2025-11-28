// ABOUTME: Configuration structures for the watch daemon
// ABOUTME: Extends IndexerConfig with watch-specific settings for debouncing, batching, and resilience

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::IndexerConfig;

/// Watch daemon configuration - extends IndexerConfig with watch-specific settings
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WatchConfig {
    /// Project root to watch
    pub project_root: PathBuf,

    /// Debounce duration for file changes (default: 30ms)
    #[serde(default = "default_debounce")]
    pub debounce_ms: u64,

    /// Batch timeout for collecting changes (default: 200ms)
    #[serde(default = "default_batch_timeout")]
    pub batch_timeout_ms: u64,

    /// Health check interval (default: 30s)
    #[serde(default = "default_health_interval")]
    pub health_check_interval_secs: u64,

    /// SurrealDB reconnection backoff
    #[serde(default)]
    pub reconnect_backoff: BackoffConfig,

    /// Circuit breaker configuration
    #[serde(default)]
    pub circuit_breaker: CircuitBreakerConfig,

    /// Inherit from IndexerConfig for indexing behavior
    #[serde(flatten)]
    pub indexer: IndexerConfig,
}

impl WatchConfig {
    /// Create a new WatchConfig from an IndexerConfig and project root
    pub fn from_indexer_config(indexer: IndexerConfig, project_root: PathBuf) -> Self {
        Self {
            project_root,
            debounce_ms: default_debounce(),
            batch_timeout_ms: default_batch_timeout(),
            health_check_interval_secs: default_health_interval(),
            reconnect_backoff: BackoffConfig::default(),
            circuit_breaker: CircuitBreakerConfig::default(),
            indexer,
        }
    }
}

/// Reconnection backoff configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct BackoffConfig {
    /// Initial backoff duration (default: 1s)
    #[serde(default = "default_backoff_initial")]
    pub initial_secs: u64,

    /// Maximum backoff duration (default: 60s)
    #[serde(default = "default_backoff_max")]
    pub max_secs: u64,

    /// Multiplier for exponential backoff (default: 2.0)
    #[serde(default = "default_backoff_multiplier")]
    pub multiplier: f64,
}

impl Default for BackoffConfig {
    fn default() -> Self {
        Self {
            initial_secs: default_backoff_initial(),
            max_secs: default_backoff_max(),
            multiplier: default_backoff_multiplier(),
        }
    }
}

/// Circuit breaker configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CircuitBreakerConfig {
    /// Failure threshold before opening circuit (default: 5)
    #[serde(default = "default_failure_threshold")]
    pub failure_threshold: u32,

    /// Success threshold to close circuit (default: 2)
    #[serde(default = "default_success_threshold")]
    pub success_threshold: u32,

    /// Timeout before attempting half-open (default: 10s)
    #[serde(default = "default_circuit_timeout")]
    pub timeout_secs: u64,
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: default_failure_threshold(),
            success_threshold: default_success_threshold(),
            timeout_secs: default_circuit_timeout(),
        }
    }
}

// Default value functions
fn default_debounce() -> u64 {
    30
}
fn default_batch_timeout() -> u64 {
    200
}
fn default_health_interval() -> u64 {
    30
}
fn default_backoff_initial() -> u64 {
    1
}
fn default_backoff_max() -> u64 {
    60
}
fn default_backoff_multiplier() -> f64 {
    2.0
}
fn default_failure_threshold() -> u32 {
    5
}
fn default_success_threshold() -> u32 {
    2
}
fn default_circuit_timeout() -> u64 {
    10
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_backoff_config() {
        let config = BackoffConfig::default();
        assert_eq!(config.initial_secs, 1);
        assert_eq!(config.max_secs, 60);
        assert!((config.multiplier - 2.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_default_circuit_breaker_config() {
        let config = CircuitBreakerConfig::default();
        assert_eq!(config.failure_threshold, 5);
        assert_eq!(config.success_threshold, 2);
        assert_eq!(config.timeout_secs, 10);
    }
}
