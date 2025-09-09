pub mod embedding;
pub mod store;
pub mod search;
pub mod faiss_manager;

#[cfg(feature = "persistent")]
pub mod storage;
#[cfg(feature = "persistent")]
pub mod persistent;
#[cfg(feature = "persistent")]
pub mod incremental;
#[cfg(feature = "persistent")]
pub mod consistency;

pub use embedding::*;
pub use store::*;
pub use search::*;
pub use faiss_manager::*;

#[cfg(feature = "persistent")]
pub use storage::*;
#[cfg(feature = "persistent")]
pub use persistent::*;
#[cfg(feature = "persistent")]
pub use incremental::*;
#[cfg(feature = "persistent")]
pub use consistency::*;

// Re-export common types for convenience
pub use codegraph_core::{CodeGraphError, NodeId, Result};
pub use faiss::MetricType;