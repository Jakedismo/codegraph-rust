use super::Balancer;
use crate::types::{Endpoint, EndpointPool};
use std::sync::Arc;

pub struct LeastConnections;

impl LeastConnections {
    pub fn new() -> Self {
        Self
    }
}
impl Default for LeastConnections {
    fn default() -> Self {
        Self::new()
    }
}

impl Balancer for LeastConnections {
    fn name(&self) -> &'static str {
        "least_conn"
    }

    fn pick(&self, pool: &EndpointPool, _key: Option<&[u8]>) -> Option<Arc<Endpoint>> {
        let eps = pool.healthy_endpoints();
        if eps.is_empty() {
            return None;
        }
        let mut best = None::<Arc<Endpoint>>;
        let mut best_conn = usize::MAX;
        for ep in eps {
            let c = ep
                .open_connections
                .load(std::sync::atomic::Ordering::Relaxed);
            if c < best_conn {
                best_conn = c;
                best = Some(ep);
            }
        }
        best
    }
}
