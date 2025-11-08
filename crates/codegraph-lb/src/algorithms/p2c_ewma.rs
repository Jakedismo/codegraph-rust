use super::Balancer;
use crate::types::{Endpoint, EndpointPool};
use parking_lot::Mutex;
use rand::{rngs::StdRng, Rng, SeedableRng};
use rand_core::SeedableRng as _;
use std::sync::Arc;

pub struct PowerOfTwoChoicesEwma {
    rng: Mutex<StdRng>,
}

impl PowerOfTwoChoicesEwma {
    pub fn new() -> Self {
        Self {
            rng: Mutex::new(StdRng::seed_from_u64(0xACE1u64)),
        }
    }
}

impl Default for PowerOfTwoChoicesEwma {
    fn default() -> Self {
        Self::new()
    }
}

impl Balancer for PowerOfTwoChoicesEwma {
    fn name(&self) -> &'static str {
        "p2c_ewma"
    }

    fn pick(&self, pool: &EndpointPool, _key: Option<&[u8]>) -> Option<Arc<Endpoint>> {
        let eps = pool.healthy_endpoints();
        let len = eps.len();
        if len == 0 {
            return None;
        }
        if len == 1 {
            return Some(eps[0].clone());
        }
        let mut rng = self.rng.lock();
        let a = rng.gen_range(0..len);
        let mut b = rng.gen_range(0..len);
        if a == b {
            b = (b + 1) % len;
        }
        let ea = eps[a]
            .ewma_latency_micros
            .load(std::sync::atomic::Ordering::Relaxed);
        let eb = eps[b]
            .ewma_latency_micros
            .load(std::sync::atomic::Ordering::Relaxed);
        if ea <= eb {
            Some(eps[a].clone())
        } else {
            Some(eps[b].clone())
        }
    }
}
