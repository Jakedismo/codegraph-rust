use super::Balancer;
use crate::types::{Endpoint, EndpointPool};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

pub struct RoundRobin {
    idx: AtomicUsize,
}

impl RoundRobin {
    pub fn new() -> Self {
        Self {
            idx: AtomicUsize::new(0),
        }
    }
}

impl Default for RoundRobin {
    fn default() -> Self {
        Self::new()
    }
}

impl Balancer for RoundRobin {
    fn name(&self) -> &'static str {
        "round_robin"
    }

    fn pick(&self, pool: &EndpointPool, _key: Option<&[u8]>) -> Option<Arc<Endpoint>> {
        let healthy = pool.healthy_endpoints();
        if healthy.is_empty() {
            return None;
        }
        let i = self.idx.fetch_add(1, Ordering::Relaxed) % healthy.len();
        Some(healthy[i].clone())
    }
}
