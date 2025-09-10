//! CodeGraph Load Balancer
//!
//! Provides:
//! - Load distribution algorithms (RR, Smooth WRR, Least-Connections, P2C+EWMA, HRW hashing)
//! - Active/passive health checks with circuit breaking
//! - Failover strategies
//! - Simple traffic shaping (rate limit per route)
//! - Tower `Service` for reverse proxying to upstreams

pub mod types;
pub mod algorithms;
pub mod health;
pub mod failover;
pub mod shaping;
pub mod metrics;

pub use types::*;
pub use algorithms::*;
pub use health::*;
pub use failover::*;
pub use shaping::*;
pub use metrics::*;
