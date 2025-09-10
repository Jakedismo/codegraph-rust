pub mod cache;
pub mod cache_optimized;
pub mod embedding_cache;
pub mod query_cache;
pub mod invalidation;
pub mod memory;
pub mod storage;
pub mod metrics;
#[cfg(feature = "readahead")]
pub mod readahead_optimizer;
#[cfg(feature = "readahead")]
pub mod readahead_integration;
pub mod profiler;
pub mod dashboard;

// Temporarily disabled due to FAISS Send/Sync issues
// #[cfg(feature = "faiss")]
// pub mod faiss_cache;

pub use cache::*;
// Re-export select optimized structures without colliding type names
pub use cache_optimized::{CacheOptimizedHashMap, CacheEntriesSoA, PaddedAtomicUsize};
pub use embedding_cache::*;
pub use query_cache::*;
pub use invalidation::*;
pub use memory::*;
pub use storage::*;
pub use metrics::*;
#[cfg(feature = "readahead")]
pub use readahead_optimizer::*;
#[cfg(feature = "readahead")]
pub use readahead_integration::*;
pub use profiler::*;
pub use dashboard::*;

// #[cfg(feature = "faiss")]
// pub use faiss_cache::*;

// Re-export common types for convenience
pub use codegraph_core::{CodeGraphError, NodeId, Result, CodeNode};
