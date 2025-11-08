// ABOUTME: Cloud SurrealDB HNSW vector search implementation
// ABOUTME: Feature-gated cloud search with Jina reranking

#[cfg(feature = "cloud")]
use codegraph_api::state::AppState;
#[cfg(feature = "cloud")]
use napi::bindgen_prelude::*;

#[cfg(feature = "cloud")]
use crate::{SearchOptions, SearchResult};

/// Cloud search implementation (feature-gated)
#[cfg(feature = "cloud")]
pub async fn search_cloud(
    _state: &AppState,
    _query: &str,
    _opts: &SearchOptions,
) -> Result<Vec<SearchResult>> {
    // Cloud search is not yet implemented
    Err(Error::from_reason(
        "Cloud search is not yet implemented. Set CODEGRAPH_CLOUD_ENABLED=false to use local search only.",
    ))
}

/// Stub for when cloud feature is not enabled
#[cfg(not(feature = "cloud"))]
pub async fn search_cloud(
    _state: &codegraph_api::state::AppState,
    _query: &str,
    _opts: &crate::SearchOptions,
) -> Result<Vec<crate::SearchResult>> {
    Err(napi::Error::from_reason(
        "Cloud search is not available. Compile with --features cloud to enable cloud search.",
    ))
}
