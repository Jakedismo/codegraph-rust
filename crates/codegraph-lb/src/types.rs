use http::Uri;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Instant;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EndpointId(pub Uuid);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EndpointConfig {
    pub uri: String,
    pub weight: u32,
    pub health_check_path: Option<String>,
}

#[derive(Debug)]
pub struct Endpoint {
    pub id: EndpointId,
    pub base_uri: Uri,
    pub weight: AtomicUsize,
    pub healthy: AtomicBool,
    pub consecutive_failures: AtomicUsize,
    pub open_connections: AtomicUsize,
    pub ewma_latency_micros: AtomicU64,
    pub last_check: RwLock<Option<Instant>>,
}

impl Endpoint {
    pub fn new(uri: Uri, weight: u32) -> Arc<Self> {
        Arc::new(Self {
            id: EndpointId(Uuid::new_v4()),
            base_uri: uri,
            weight: AtomicUsize::new(weight as usize),
            healthy: AtomicBool::new(true),
            consecutive_failures: AtomicUsize::new(0),
            open_connections: AtomicUsize::new(0),
            ewma_latency_micros: AtomicU64::new(5000),
            last_check: RwLock::new(None),
        })
    }

    pub fn set_healthy(&self, healthy: bool) {
        self.healthy.store(healthy, Ordering::Relaxed);
    }

    pub fn is_healthy(&self) -> bool {
        self.healthy.load(Ordering::Relaxed)
    }

    pub fn incr_conn(&self) {
        self.open_connections.fetch_add(1, Ordering::Relaxed);
    }
    pub fn decr_conn(&self) {
        self.open_connections.fetch_sub(1, Ordering::Relaxed);
    }

    pub fn record_latency(&self, micros: u64, alpha: f64) {
        // EWMA update stored as u64 micros
        let prev = self.ewma_latency_micros.load(Ordering::Relaxed) as f64;
        let next = alpha * (micros as f64) + (1.0 - alpha) * prev;
        self.ewma_latency_micros
            .store(next as u64, Ordering::Relaxed);
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolConfig {
    pub endpoints: Vec<EndpointConfig>,
}

#[derive(Debug, Default)]
pub struct EndpointPool {
    pub endpoints: Vec<Arc<Endpoint>>,
}

impl EndpointPool {
    pub fn new() -> Self {
        Self {
            endpoints: Vec::new(),
        }
    }

    pub fn from_config(cfg: &PoolConfig) -> anyhow::Result<Self> {
        let mut pool = Self::new();
        for ep in &cfg.endpoints {
            let uri: Uri = ep.uri.parse()?;
            pool.endpoints.push(Endpoint::new(uri, ep.weight));
        }
        Ok(pool)
    }

    pub fn healthy_endpoints(&self) -> Vec<Arc<Endpoint>> {
        self.endpoints
            .iter()
            .filter(|e| e.is_healthy())
            .cloned()
            .collect()
    }
}
