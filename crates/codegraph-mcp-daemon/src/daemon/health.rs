// ABOUTME: Health monitoring with circuit breaker pattern for SurrealDB resilience
// ABOUTME: Provides automatic reconnection with exponential backoff

use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

use super::config::{BackoffConfig, CircuitBreakerConfig};

/// Circuit breaker state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CircuitState {
    /// Normal operation - requests allowed
    Closed,
    /// Testing if service recovered - single request allowed
    HalfOpen,
    /// Failing - requests blocked for timeout period
    Open,
}

impl std::fmt::Display for CircuitState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CircuitState::Closed => write!(f, "Closed"),
            CircuitState::HalfOpen => write!(f, "Half-Open"),
            CircuitState::Open => write!(f, "Open"),
        }
    }
}

/// Health monitor with circuit breaker pattern
pub struct HealthMonitor {
    /// Circuit breaker state
    circuit_state: Arc<RwLock<CircuitState>>,

    /// Configuration
    config: CircuitBreakerConfig,

    /// Backoff configuration
    backoff_config: BackoffConfig,

    /// Consecutive failures
    consecutive_failures: Arc<RwLock<u32>>,

    /// Consecutive successes (for half-open recovery)
    consecutive_successes: Arc<RwLock<u32>>,

    /// Time when circuit opened (for timeout)
    circuit_opened_at: Arc<RwLock<Option<Instant>>>,

    /// Current backoff duration
    current_backoff: Arc<RwLock<Duration>>,
}

impl HealthMonitor {
    pub fn new(config: CircuitBreakerConfig, backoff_config: BackoffConfig) -> Self {
        Self {
            circuit_state: Arc::new(RwLock::new(CircuitState::Closed)),
            config,
            backoff_config: backoff_config.clone(),
            consecutive_failures: Arc::new(RwLock::new(0)),
            consecutive_successes: Arc::new(RwLock::new(0)),
            circuit_opened_at: Arc::new(RwLock::new(None)),
            current_backoff: Arc::new(RwLock::new(Duration::from_secs(
                backoff_config.initial_secs,
            ))),
        }
    }

    /// Get current circuit state
    pub async fn circuit_state(&self) -> CircuitState {
        *self.circuit_state.read().await
    }

    /// Check if requests should be allowed
    pub async fn should_allow_request(&self) -> bool {
        let state = self.circuit_state.read().await;
        match *state {
            CircuitState::Closed => true,
            CircuitState::HalfOpen => true,
            CircuitState::Open => {
                // Check if timeout has elapsed
                if let Some(opened_at) = *self.circuit_opened_at.read().await {
                    if opened_at.elapsed() >= Duration::from_secs(self.config.timeout_secs) {
                        drop(state);
                        self.transition_to_half_open().await;
                        return true;
                    }
                }
                debug!("Circuit open, blocking request");
                false
            }
        }
    }

    /// Record a successful operation
    pub async fn record_success(&self) {
        let mut state = self.circuit_state.write().await;
        let mut successes = self.consecutive_successes.write().await;
        let mut failures = self.consecutive_failures.write().await;

        *failures = 0;
        *successes += 1;

        match *state {
            CircuitState::HalfOpen => {
                if *successes >= self.config.success_threshold {
                    info!("Circuit breaker: Half-Open -> Closed (recovered)");
                    *state = CircuitState::Closed;
                    *successes = 0;
                    // Reset backoff
                    *self.current_backoff.write().await =
                        Duration::from_secs(self.backoff_config.initial_secs);
                }
            }
            CircuitState::Closed => {
                debug!("Health check passed");
            }
            CircuitState::Open => {
                // Shouldn't happen, but handle gracefully
                *state = CircuitState::Closed;
            }
        }
    }

    /// Record a failed operation
    pub async fn record_failure(&self) {
        let mut state = self.circuit_state.write().await;
        let mut failures = self.consecutive_failures.write().await;
        let mut successes = self.consecutive_successes.write().await;

        *successes = 0;
        *failures += 1;

        match *state {
            CircuitState::Closed => {
                if *failures >= self.config.failure_threshold {
                    warn!(
                        "Circuit breaker: Closed -> Open ({} consecutive failures)",
                        failures
                    );
                    *state = CircuitState::Open;
                    *self.circuit_opened_at.write().await = Some(Instant::now());
                    self.increase_backoff().await;
                }
            }
            CircuitState::HalfOpen => {
                warn!("Circuit breaker: Half-Open -> Open (test failed)");
                *state = CircuitState::Open;
                *self.circuit_opened_at.write().await = Some(Instant::now());
                self.increase_backoff().await;
            }
            CircuitState::Open => {
                debug!("Circuit already open, failure recorded");
            }
        }
    }

    /// Get current backoff duration for retry
    pub async fn current_backoff(&self) -> Duration {
        *self.current_backoff.read().await
    }

    async fn transition_to_half_open(&self) {
        let mut state = self.circuit_state.write().await;
        if *state == CircuitState::Open {
            info!("Circuit breaker: Open -> Half-Open (testing recovery)");
            *state = CircuitState::HalfOpen;
            *self.consecutive_successes.write().await = 0;
        }
    }

    async fn increase_backoff(&self) {
        let mut backoff = self.current_backoff.write().await;
        let new_backoff = (*backoff).mul_f64(self.backoff_config.multiplier);
        let max_backoff = Duration::from_secs(self.backoff_config.max_secs);
        *backoff = new_backoff.min(max_backoff);
        debug!("Backoff increased to {:?}", *backoff);
    }

    /// Reset the circuit breaker to initial state
    pub async fn reset(&self) {
        *self.circuit_state.write().await = CircuitState::Closed;
        *self.consecutive_failures.write().await = 0;
        *self.consecutive_successes.write().await = 0;
        *self.circuit_opened_at.write().await = None;
        *self.current_backoff.write().await =
            Duration::from_secs(self.backoff_config.initial_secs);
        info!("Circuit breaker reset to initial state");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_circuit_breaker_opens_after_failures() {
        let config = CircuitBreakerConfig {
            failure_threshold: 3,
            success_threshold: 2,
            timeout_secs: 10,
        };
        let backoff = BackoffConfig::default();
        let monitor = HealthMonitor::new(config, backoff);

        assert_eq!(monitor.circuit_state().await, CircuitState::Closed);

        for _ in 0..3 {
            monitor.record_failure().await;
        }

        assert_eq!(monitor.circuit_state().await, CircuitState::Open);
    }

    #[tokio::test]
    async fn test_circuit_breaker_recovers() {
        let config = CircuitBreakerConfig {
            failure_threshold: 2,
            success_threshold: 2,
            timeout_secs: 0, // Immediate timeout for test
        };
        let backoff = BackoffConfig::default();
        let monitor = HealthMonitor::new(config, backoff);

        // Open the circuit
        monitor.record_failure().await;
        monitor.record_failure().await;
        assert_eq!(monitor.circuit_state().await, CircuitState::Open);

        // Wait for timeout and check - should transition to half-open
        tokio::time::sleep(Duration::from_millis(10)).await;
        assert!(monitor.should_allow_request().await);

        // Record successes to close
        monitor.record_success().await;
        monitor.record_success().await;
        assert_eq!(monitor.circuit_state().await, CircuitState::Closed);
    }
}
