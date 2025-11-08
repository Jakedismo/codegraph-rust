pub mod embedding;
pub mod embeddings;
pub mod error;
pub mod providers;
pub mod search;

pub mod cache;
pub mod gpu;
pub mod memory;
pub mod optimization;
pub mod prep;
pub mod simd_ops;

#[cfg(feature = "openai")]
pub mod openai_provider;

#[cfg(feature = "local-embeddings")]
pub mod local_provider;

#[cfg(feature = "onnx")]
pub mod onnx_provider;

#[cfg(feature = "ollama")]
pub mod ollama_embedding_provider;

#[cfg(feature = "jina")]
pub mod jina_provider;

#[cfg(feature = "faiss")]
pub mod faiss_manager;
#[cfg(feature = "faiss")]
pub mod index;
#[cfg(feature = "faiss")]
pub mod serde_utils;
#[cfg(feature = "faiss")]
pub mod store;

#[cfg(feature = "persistent")]
pub mod consistency;
#[cfg(feature = "persistent")]
pub mod incremental;
#[cfg(feature = "persistent")]
pub mod persistent;
#[cfg(feature = "persistent")]
pub mod storage;

pub mod insights_generator;
pub mod ml;
pub mod rag;
pub mod reranker; // NEW: Fast reranking pipeline for insights generation // NEW: High-performance insights with reranking

pub use embedding::*;
pub use embeddings::generator::AdvancedEmbeddingGenerator;
pub use providers::*;
pub use search::*;

pub use cache::*;
pub use gpu::*;
pub use memory::*;
pub use optimization::*;

#[cfg(feature = "openai")]
pub use openai_provider::*;

#[cfg(feature = "local-embeddings")]
pub use local_provider::*;

#[cfg(feature = "onnx")]
pub use onnx_provider::*;

#[cfg(feature = "ollama")]
pub use ollama_embedding_provider::*;

#[cfg(feature = "jina")]
pub use jina_provider::*;

#[cfg(feature = "faiss")]
pub use faiss_manager::*;
#[cfg(feature = "faiss")]
pub use store::*;

#[cfg(feature = "persistent")]
pub use consistency::*;
#[cfg(feature = "persistent")]
pub use incremental::*;
#[cfg(feature = "persistent")]
pub use persistent::*;
#[cfg(feature = "persistent")]
pub use storage::*;

pub use insights_generator::*;
pub use rag::*;
pub use reranker::*; // Re-export reranker types // Re-export insights types

// Re-export common types for convenience
pub use codegraph_core::{CodeGraphError, NodeId, Result};
pub use error::VectorError;

#[cfg(feature = "faiss")]
pub use faiss::MetricType;
