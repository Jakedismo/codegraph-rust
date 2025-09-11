//! Concurrent, lock-free and wait-free data structures for CodeGraph
//!
//! - `spsc`: Wait-free single-producer single-consumer queue
//! - `mpmc`: Lock-free bounded multi-producer multi-consumer queue (wrapper)
//! - `graph`: Lock-free adjacency operations using atomics (ArcSwap + SkipMap)

pub mod graph;
pub mod mpmc;
pub mod spsc;
