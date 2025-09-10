use std::sync::Arc;
use crate::types::{Endpoint, EndpointPool};
use super::Balancer;
use sha2::{Digest, Sha256};

pub struct HrwHashing;
impl HrwHashing { pub fn new() -> Self { Self } }
impl Default for HrwHashing { fn default() -> Self { Self::new() } }

fn hrw_score(key: &[u8], endpoint: &Endpoint) -> u64 {
    let mut hasher = Sha256::new();
    hasher.update(key);
    hasher.update(endpoint.id.0.as_bytes());
    let digest = hasher.finalize();
    // take first 8 bytes as u64
    let mut arr = [0u8; 8];
    arr.copy_from_slice(&digest[0..8]);
    u64::from_be_bytes(arr)
}

impl Balancer for HrwHashing {
    fn name(&self) -> &'static str { "hrw_hashing" }

    fn pick(&self, pool: &EndpointPool, key: Option<&[u8]>) -> Option<Arc<Endpoint>> {
        let eps = pool.healthy_endpoints();
        if eps.is_empty() { return None; }
        let key = key.unwrap_or(b"");
        let mut best = None::<Arc<Endpoint>>;
        let mut best_score = 0u64;
        for ep in eps {
            let score = hrw_score(key, &ep);
            if best.is_none() || score > best_score {
                best_score = score;
                best = Some(ep);
            }
        }
        best
    }
}

