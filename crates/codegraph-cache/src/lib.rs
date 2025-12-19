pub mod cache;
pub mod embedding_cache;
pub mod invalidation;
pub mod metrics;
pub mod query_cache;
pub mod memory;
// Re-enabled after fixing compilation issues
#[cfg(feature = "readahead")]
pub mod readahead_integration;
#[cfg(feature = "readahead")]
pub mod readahead_optimizer;

pub use cache::*;
pub use embedding_cache::*;
pub use invalidation::*;
pub use metrics::*;
pub use query_cache::*;
pub use memory::*;
// Re-enabled after fixing compilation issues
#[cfg(feature = "readahead")]
pub use readahead_integration::*;
#[cfg(feature = "readahead")]
pub use readahead_optimizer::*;

// Re-export common types for convenience
pub use codegraph_core::{CodeGraphError, CodeNode, NodeId, Result};
