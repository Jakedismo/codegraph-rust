use std::sync::Arc;
use std::time::Duration;
use tokio::time::{interval, MissedTickBehavior};
use tracing::{info, warn, debug};
use crate::types::Endpoint;

pub struct HealthCheckConfig {
    pub interval: Duration,
    pub failure_threshold: usize,
    pub recovery_threshold: usize,
    pub path: String,
    pub timeout: Duration,
}

impl Default for HealthCheckConfig {
    fn default() -> Self {
        Self {
            interval: Duration::from_secs(5),
            failure_threshold: 3,
            recovery_threshold: 2,
            path: "/health".to_string(),
            timeout: Duration::from_secs(2),
        }
    }
}

pub async fn start_active_http_checks(endpoints: Vec<Arc<Endpoint>>, cfg: HealthCheckConfig) {
    let client = reqwest::Client::builder().timeout(cfg.timeout).build().unwrap();
    let mut ticker = interval(cfg.interval);
    ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);

    loop {
        ticker.tick().await;
        for ep in &endpoints {
            let base = ep.base_uri.clone();
            let url = format!("{}{}", base, &cfg.path);
            let epc = ep.clone();
            let clientc = client.clone();
            let failure_threshold = cfg.failure_threshold;
            let recovery_threshold = cfg.recovery_threshold;
            tokio::spawn(async move {
                match clientc.get(&url).send().await {
                    Ok(resp) if resp.status().is_success() => {
                        let was_unhealthy = !epc.is_healthy();
                        epc.consecutive_failures.store(0, std::sync::atomic::Ordering::Relaxed);
                        if was_unhealthy {
                            let cur = epc.consecutive_failures.load(std::sync::atomic::Ordering::Relaxed);
                            if cur <= recovery_threshold {
                                epc.set_healthy(true);
                                info!("endpoint recovered: {}", base);
                            }
                        } else {
                            epc.set_healthy(true);
                        }
                    }
                    Ok(resp) => {
                        warn!("health check non-200 for {}: {}", base, resp.status());
                        let f = epc.consecutive_failures.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
                        if f >= failure_threshold { epc.set_healthy(false); }
                    }
                    Err(e) => {
                        warn!("health check failed for {}: {}", base, e);
                        let f = epc.consecutive_failures.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
                        if f >= failure_threshold { epc.set_healthy(false); }
                    }
                }
            });
        }
    }
}

