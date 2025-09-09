pub mod embedding;
pub mod search;

#[cfg(feature = "faiss")]
pub mod store;
#[cfg(feature = "faiss")]
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
pub use search::*;

#[cfg(feature = "faiss")]
pub use store::*;
#[cfg(feature = "faiss")]
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

#[cfg(feature = "faiss")]
pub use faiss::MetricType;