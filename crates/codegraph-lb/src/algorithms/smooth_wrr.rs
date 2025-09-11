use super::Balancer;
use crate::types::{Endpoint, EndpointPool};
use parking_lot::Mutex;
use std::sync::Arc;

// Smooth Weighted Round Robin per Nginx algorithm
#[derive(Default)]
pub struct SmoothWeightedRR {
    inner: Mutex<State>,
}

#[derive(Default)]
struct State {
    current: Vec<i64>, // current effective weights
    weights: Vec<i64>,
}

impl SmoothWeightedRR {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(State::default()),
        }
    }
}

impl Balancer for SmoothWeightedRR {
    fn name(&self) -> &'static str {
        "smooth_wrr"
    }

    fn pick(&self, pool: &EndpointPool, _key: Option<&[u8]>) -> Option<Arc<Endpoint>> {
        let eps = pool.healthy_endpoints();
        if eps.is_empty() {
            return None;
        }
        let mut st = self.inner.lock();
        if st.weights.len() != eps.len() {
            st.weights = eps
                .iter()
                .map(|e| e.weight.load(std::sync::atomic::Ordering::Relaxed) as i64)
                .collect();
            st.current = vec![0; st.weights.len()];
        }
        let total: i64 = st.weights.iter().sum();
        let mut best = 0usize;
        for i in 0..eps.len() {
            st.current[i] += st.weights[i];
            if st.current[i] > st.current[best] {
                best = i;
            }
        }
        st.current[best] -= total;
        Some(eps[best].clone())
    }
}
