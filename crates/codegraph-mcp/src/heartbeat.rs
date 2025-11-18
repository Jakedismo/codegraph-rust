use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tokio::time::{interval, Instant, MissedTickBehavior};
use tracing::{debug, error, warn};

#[derive(Debug, Clone)]
pub struct HeartbeatConfig {
    pub interval: Duration,
    pub timeout: Duration,
    pub max_missed: u32,
}

impl Default for HeartbeatConfig {
    fn default() -> Self {
        Self {
            interval: Duration::from_secs(30),
            timeout: Duration::from_secs(10),
            max_missed: 3,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeartbeatState {
    Healthy,
    Degraded,
    Failed,
}

#[derive(Debug, Clone)]
pub struct HeartbeatMonitor {
    config: HeartbeatConfig,
    state: Arc<RwLock<HeartbeatState>>,
    last_heartbeat: Arc<AtomicU64>,
    missed_count: Arc<AtomicU64>,
    sequence_number: Arc<AtomicU64>,
}

impl HeartbeatMonitor {
    pub fn new(config: HeartbeatConfig) -> Self {
        let now = Instant::now().elapsed().as_millis() as u64;
        Self {
            config,
            state: Arc::new(RwLock::new(HeartbeatState::Healthy)),
            last_heartbeat: Arc::new(AtomicU64::new(now)),
            missed_count: Arc::new(AtomicU64::new(0)),
            sequence_number: Arc::new(AtomicU64::new(0)),
        }
    }

    pub async fn start_monitoring<F>(&self, ping_sender: F) -> crate::Result<()>
    where
        F: Fn(u64) -> tokio::task::JoinHandle<()> + Send + Sync + 'static,
    {
        let mut interval = interval(self.config.interval);
        interval.set_missed_tick_behavior(MissedTickBehavior::Skip);

        let state = Arc::clone(&self.state);
        let last_heartbeat = Arc::clone(&self.last_heartbeat);
        let missed_count = Arc::clone(&self.missed_count);
        let sequence_number = Arc::clone(&self.sequence_number);
        let config = self.config.clone();

        tokio::spawn(async move {
            loop {
                interval.tick().await;

                let seq = sequence_number.fetch_add(1, Ordering::SeqCst);
                let now = Instant::now().elapsed().as_millis() as u64;

                ping_sender(seq);
                debug!("Sent heartbeat ping with sequence {}", seq);

                tokio::time::sleep(config.timeout).await;

                let last_received = last_heartbeat.load(Ordering::SeqCst);
                let time_since_last = now.saturating_sub(last_received);

                if time_since_last > config.timeout.as_millis() as u64 {
                    let missed = missed_count.fetch_add(1, Ordering::SeqCst) + 1;
                    warn!(
                        "Heartbeat missed ({}), time since last: {}ms",
                        missed, time_since_last
                    );

                    let mut current_state = state.write().await;
                    if missed >= config.max_missed as u64 {
                        *current_state = HeartbeatState::Failed;
                        error!("Heartbeat failed after {} missed beats", missed);
                        break;
                    } else if missed > 1 {
                        *current_state = HeartbeatState::Degraded;
                    }
                } else {
                    missed_count.store(0, Ordering::SeqCst);
                    let mut current_state = state.write().await;
                    if *current_state != HeartbeatState::Healthy {
                        *current_state = HeartbeatState::Healthy;
                        debug!("Heartbeat recovered");
                    }
                }
            }
        });

        Ok(())
    }

    pub async fn on_pong_received(&self, sequence: u64) {
        let now = Instant::now().elapsed().as_millis() as u64;
        let expected_seq = self.sequence_number.load(Ordering::SeqCst);

        if sequence + 5 >= expected_seq {
            self.last_heartbeat.store(now, Ordering::SeqCst);
            debug!("Received valid pong for sequence {}", sequence);
        } else {
            warn!(
                "Received outdated pong sequence {} (expected around {})",
                sequence, expected_seq
            );
        }
    }

    pub async fn state(&self) -> HeartbeatState {
        *self.state.read().await
    }

    pub fn last_heartbeat_time(&self) -> u64 {
        self.last_heartbeat.load(Ordering::SeqCst)
    }

    pub fn missed_count(&self) -> u64 {
        self.missed_count.load(Ordering::SeqCst)
    }

    pub fn is_healthy(&self) -> impl std::future::Future<Output = bool> + Send {
        let state = Arc::clone(&self.state);
        async move { *state.read().await == HeartbeatState::Healthy }
    }
}

#[derive(Debug, Clone)]
pub struct HeartbeatManager {
    monitor: Option<HeartbeatMonitor>,
}

impl HeartbeatManager {
    pub fn new() -> Self {
        Self { monitor: None }
    }

    pub fn with_config(config: HeartbeatConfig) -> Self {
        Self {
            monitor: Some(HeartbeatMonitor::new(config)),
        }
    }

    pub async fn start<F>(&mut self, ping_sender: F) -> crate::Result<()>
    where
        F: Fn(u64) -> tokio::task::JoinHandle<()> + Send + Sync + 'static,
    {
        if let Some(monitor) = &self.monitor {
            monitor.start_monitoring(ping_sender).await
        } else {
            Ok(())
        }
    }

    pub async fn on_pong(&self, sequence: u64) {
        if let Some(monitor) = &self.monitor {
            monitor.on_pong_received(sequence).await;
        }
    }

    pub async fn state(&self) -> HeartbeatState {
        if let Some(monitor) = &self.monitor {
            monitor.state().await
        } else {
            HeartbeatState::Healthy
        }
    }

    pub async fn is_healthy(&self) -> bool {
        if let Some(monitor) = &self.monitor {
            monitor.is_healthy().await
        } else {
            true
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.monitor.is_some()
    }
}

impl Default for HeartbeatManager {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone)]
pub struct HeartbeatStats {
    pub state: HeartbeatState,
    pub last_heartbeat: u64,
    pub missed_count: u64,
    pub uptime_ms: u64,
}

impl HeartbeatManager {
    pub async fn stats(&self) -> HeartbeatStats {
        if let Some(monitor) = &self.monitor {
            HeartbeatStats {
                state: monitor.state().await,
                last_heartbeat: monitor.last_heartbeat_time(),
                missed_count: monitor.missed_count(),
                uptime_ms: Instant::now().elapsed().as_millis() as u64,
            }
        } else {
            HeartbeatStats {
                state: HeartbeatState::Healthy,
                last_heartbeat: Instant::now().elapsed().as_millis() as u64,
                missed_count: 0,
                uptime_ms: Instant::now().elapsed().as_millis() as u64,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::sleep;

    #[tokio::test]
    async fn test_heartbeat_config_default() {
        let config = HeartbeatConfig::default();
        assert_eq!(config.interval, Duration::from_secs(30));
        assert_eq!(config.timeout, Duration::from_secs(10));
        assert_eq!(config.max_missed, 3);
    }

    #[tokio::test]
    async fn test_heartbeat_monitor_creation() {
        let config = HeartbeatConfig::default();
        let monitor = HeartbeatMonitor::new(config);
        assert_eq!(monitor.state().await, HeartbeatState::Healthy);
        assert_eq!(monitor.missed_count(), 0);
    }

    #[tokio::test]
    async fn test_heartbeat_manager() {
        let mut manager = HeartbeatManager::new();
        assert!(!manager.is_enabled());
        assert!(manager.is_healthy().await);

        let config = HeartbeatConfig {
            interval: Duration::from_millis(100),
            timeout: Duration::from_millis(50),
            max_missed: 2,
        };
        manager = HeartbeatManager::with_config(config);
        assert!(manager.is_enabled());
    }

    #[tokio::test]
    async fn test_pong_handling() {
        let config = HeartbeatConfig {
            interval: Duration::from_millis(100),
            timeout: Duration::from_millis(50),
            max_missed: 2,
        };
        let monitor = HeartbeatMonitor::new(config);

        monitor.on_pong_received(1).await;
        assert_eq!(monitor.state().await, HeartbeatState::Healthy);

        sleep(Duration::from_millis(200)).await;

        monitor.on_pong_received(100).await;
        assert_eq!(monitor.state().await, HeartbeatState::Healthy);
    }
}
