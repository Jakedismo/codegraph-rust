use crate::types::{Endpoint, EndpointPool};
use async_trait::async_trait;
use std::sync::Arc;

#[async_trait]
pub trait Balancer: Send + Sync {
    fn name(&self) -> &'static str;
    fn pick(&self, pool: &EndpointPool, key: Option<&[u8]>) -> Option<Arc<Endpoint>>;

    fn on_success(&self, _endpoint: &Arc<Endpoint>, _latency_micros: u64) {}
    fn on_failure(&self, _endpoint: &Arc<Endpoint>) {}
}

pub mod hrw;
pub mod least_conn;
pub mod p2c_ewma;
pub mod rr;
pub mod smooth_wrr;

pub use hrw::HrwHashing;
pub use least_conn::LeastConnections;
pub use p2c_ewma::PowerOfTwoChoicesEwma;
pub use rr::RoundRobin;
pub use smooth_wrr::SmoothWeightedRR;
