// ABOUTME: Search API dispatcher with dual-mode support
// ABOUTME: Routes queries to local Surreal vector store or cloud SurrealDB based on config

use napi::bindgen_prelude::*;
use std::time::Instant;

use crate::{DualModeSearchResult, SearchOptions, SearchResult};
use codegraph_core::GraphStore;

mod local;

#[cfg(feature = "cloud")]
mod cloud;

/// Semantic search with dual-mode support (local Surreal vector search or cloud SurrealDB)
pub async fn semantic_search(
    query: String,
    options: Option<SearchOptions>,
) -> Result<DualModeSearchResult> {
    let start = Instant::now();
    let opts = options.unwrap_or_default();

    // Get cloud config from environment
    let _cloud_enabled = std::env::var("CODEGRAPH_CLOUD_ENABLED")
        .unwrap_or_else(|_| "false".to_string())
        .parse::<bool>()
        .unwrap_or(false);

    let state = crate::get_or_init_state().await?;
    let state = state.lock().await;

    // Execute local search
    let local_results = local::search_local(&state, &query, &opts).await?;

    // Execute cloud search if enabled and feature is compiled
    #[cfg(feature = "cloud")]
    let cloud_results = if cloud_enabled {
        match cloud::search_cloud(&state, &query, &opts).await {
            Ok(results) => Some(results),
            Err(e) => {
                tracing::warn!("Cloud search failed: {}, falling back to local only", e);
                None
            }
        }
    } else {
        None
    };

    #[cfg(not(feature = "cloud"))]
    let cloud_results: Option<Vec<SearchResult>> = None;

    // Reranking will be implemented in Phase 3
    let reranked_results = None;

    let total_count = local_results.len() as u32;
    let search_time_ms = start.elapsed().as_secs_f64() * 1000.0;

    Ok(DualModeSearchResult {
        local_results,
        cloud_results,
        reranked_results,
        total_count,
        search_time_ms,
    })
}

/// Find similar functions by node ID
pub async fn search_similar_functions(
    node_id: String,
    limit: Option<u32>,
) -> Result<Vec<SearchResult>> {
    let state = crate::get_or_init_state().await?;
    let state = state.lock().await;

    let limit = limit.unwrap_or(10) as usize;

    // Parse node ID
    let id = uuid::Uuid::parse_str(&node_id)
        .map_err(|e| Error::from_reason(format!("Invalid node ID: {}", e)))?;

    // Get the node from graph
    let node = state
        .graph
        .read()
        .await
        .get_node(id)
        .await
        .map_err(|e| Error::from_reason(format!("Failed to get node: {}", e)))?
        .ok_or_else(|| Error::from_reason("Node not found"))?;

    // Use semantic search to find similar embeddings
    let results = state
        .semantic_search
        .find_similar_functions(&node, limit)
        .await
        .map_err(|e| Error::from_reason(format!("Failed to find similar functions: {}", e)))?;

    // Convert to SearchResult
    Ok(results
        .into_iter()
        .map(|r| SearchResult {
            id: r.node_id.to_string(),
            name: r
                .node
                .as_ref()
                .map(|n| n.name.to_string())
                .unwrap_or_default(),
            description: r
                .node
                .as_ref()
                .and_then(|n| n.content.as_ref().map(|s| s.to_string())),
            similarity: r.score as f64,
            metadata: None,
        })
        .collect())
}
