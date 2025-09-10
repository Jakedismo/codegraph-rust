pub mod error;
pub mod node;
pub mod types;
pub mod traits;
pub mod versioning;
pub mod shared;
pub mod buffer_pool;
pub mod optimized_types;
pub mod performance_monitor;
pub mod optimization_coordinator;
pub mod mmap;
pub mod memory;
pub mod config;
pub mod integration;
pub mod watch;
pub mod propagation;
pub mod incremental;

pub use error::*;
pub use node::*;
pub use types::*;
pub use traits::*;
pub use versioning::*;
pub use shared::*;
pub use buffer_pool::*;
pub use optimized_types::*;
pub use performance_monitor::*;
pub use optimization_coordinator::*;
pub use mmap::*;
pub use config::*;
pub use integration::*;
pub use watch::*;
pub use propagation::*;
pub use incremental::*;

// Use jemalloc as the global allocator when the feature is enabled
#[cfg(feature = "jemalloc")]
#[global_allocator]
static GLOBAL: tikv_jemallocator::Jemalloc = tikv_jemallocator::Jemalloc;
