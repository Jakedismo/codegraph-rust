use crate::types::{Endpoint, EndpointPool};
use std::sync::Arc;

#[derive(Debug, Clone, Copy)]
pub enum FailoverStrategy {
    NextHealthy,
    // Future: ZoneAware, FallbackPool
}

pub fn next_healthy<'a>(
    pool: &'a EndpointPool,
    preferred: &Arc<Endpoint>,
) -> Option<Arc<Endpoint>> {
    let eps = pool.healthy_endpoints();
    for ep in eps {
        if ep.id != preferred.id {
            return Some(ep);
        }
    }
    None
}
