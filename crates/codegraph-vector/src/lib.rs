pub mod embedding;
pub mod search;
pub mod providers;

pub mod optimization;
pub mod gpu;
pub mod memory;

#[cfg(feature = "openai")]
pub mod openai_provider;

#[cfg(feature = "local-embeddings")]
pub mod local_provider;

#[cfg(feature = "faiss")]
pub mod store;
#[cfg(feature = "faiss")]
pub mod faiss_manager;
#[cfg(feature = "faiss")]
pub mod index;
#[cfg(feature = "faiss")]
pub mod serde_utils;

#[cfg(feature = "persistent")]
pub mod storage;
#[cfg(feature = "persistent")]
pub mod persistent;
#[cfg(feature = "persistent")]
pub mod incremental;
#[cfg(feature = "persistent")]
pub mod consistency;

pub mod rag;

pub use embedding::*;
pub use search::*;
pub use providers::*;

pub use optimization::*;
pub use gpu::*;
pub use memory::*;

#[cfg(feature = "openai")]
pub use openai_provider::*;

#[cfg(feature = "local-embeddings")]
pub use local_provider::*;

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

pub use rag::*;

// Re-export common types for convenience
pub use codegraph_core::{CodeGraphError, NodeId, Result};

#[cfg(feature = "faiss")]
pub use faiss::MetricType;
