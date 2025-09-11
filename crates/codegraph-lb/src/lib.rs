//! CodeGraph Load Balancer
//!
//! Provides:
//! - Load distribution algorithms (RR, Smooth WRR, Least-Connections, P2C+EWMA, HRW hashing)
//! - Active/passive health checks with circuit breaking
//! - Failover strategies
//! - Simple traffic shaping (rate limit per route)
//! - Tower `Service` for reverse proxying to upstreams

pub mod algorithms;
pub mod failover;
pub mod health;
pub mod metrics;
pub mod shaping;
pub mod types;

pub use algorithms::*;
pub use failover::*;
pub use health::*;
pub use metrics::*;
pub use shaping::*;
pub use types::*;
