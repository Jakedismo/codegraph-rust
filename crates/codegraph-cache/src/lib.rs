pub mod cache;
pub mod cache_optimized;
pub mod embedding_cache;
pub mod query_cache;
pub mod invalidation;
pub mod memory;
pub mod storage;
pub mod metrics;

// Temporarily disabled due to FAISS Send/Sync issues
// #[cfg(feature = "faiss")]
// pub mod faiss_cache;

pub use cache::*;
pub use cache_optimized::*;
pub use embedding_cache::*;
pub use query_cache::*;
pub use invalidation::*;
pub use memory::*;
pub use storage::*;
pub use metrics::*;

// #[cfg(feature = "faiss")]
// pub use faiss_cache::*;

// Re-export common types for convenience
pub use codegraph_core::{CodeGraphError, NodeId, Result, CodeNode};