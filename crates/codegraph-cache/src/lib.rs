pub mod cache;
pub mod embedding_cache;
pub mod invalidation;
pub mod metrics;
pub mod query_cache;
pub mod storage;

// Re-enabled after fixing compilation issues
#[cfg(feature = "readahead")]
pub mod readahead_integration;
#[cfg(feature = "readahead")]
pub mod readahead_optimizer;

// Still temporarily disabled - may need similar fixes
// pub mod cache_optimized;
// pub mod memory;
// pub mod profiler;
// pub mod dashboard;

// Temporarily disabled due to FAISS Send/Sync issues
// #[cfg(feature = "faiss")]
// pub mod faiss_cache;

pub use cache::*;
pub use embedding_cache::*;
pub use invalidation::*;
pub use metrics::*;
pub use query_cache::*;
pub use storage::*;

// Re-enabled after fixing compilation issues
#[cfg(feature = "readahead")]
pub use readahead_integration::*;
#[cfg(feature = "readahead")]
pub use readahead_optimizer::*;

// Still temporarily disabled - re-enable after verification
// Re-export select optimized structures without colliding type names
// pub use cache_optimized::{CacheOptimizedHashMap, CacheEntriesSoA, PaddedAtomicUsize};
// pub use memory::*;
// pub use profiler::*;
// pub use dashboard::*;

// #[cfg(feature = "faiss")]
// pub use faiss_cache::*;

// Re-export common types for convenience
pub use codegraph_core::{CodeGraphError, CodeNode, NodeId, Result};
