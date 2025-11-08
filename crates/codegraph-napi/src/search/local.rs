// ABOUTME: Local FAISS vector search implementation
// ABOUTME: Delegates to codegraph-ai SemanticSearchEngine

use codegraph_api::state::AppState;
use codegraph_core::{GraphStore, NodeType};
use napi::bindgen_prelude::*;

use crate::{SearchOptions, SearchResult};

/// Local FAISS search implementation
pub async fn search_local(
    state: &AppState,
    query: &str,
    opts: &SearchOptions,
) -> Result<Vec<SearchResult>> {
    let limit = opts.limit.unwrap_or(10) as usize;
    let min_similarity = opts.min_similarity.unwrap_or(0.0) as f32;

    // Perform semantic search using FAISS
    let results = state
        .semantic_search
        .search_by_text(query, limit * 2) // Oversample for filtering
        .await
        .map_err(|e| Error::from_reason(format!("Search failed: {}", e)))?;

    // Apply filters
    let mut filtered_results = Vec::new();
    let graph = state.graph.read().await;

    for result in results {
        // Apply similarity threshold
        if result.score < min_similarity {
            continue;
        }

        // Get node details
        let node = match graph.get_node(result.node_id).await {
            Ok(Some(node)) => node,
            Ok(None) => continue,
            Err(e) => {
                tracing::warn!("Failed to get node {}: {}", result.node_id, e);
                continue;
            }
        };

        // Apply type filter if specified
        if let Some(ref filter_type) = opts.filter_by_type {
            let matches = match filter_type.to_lowercase().as_str() {
                "function" => matches!(node.node_type, Some(NodeType::Function)),
                "class" => matches!(node.node_type, Some(NodeType::Class)),
                "module" => matches!(node.node_type, Some(NodeType::Module)),
                "variable" => matches!(node.node_type, Some(NodeType::Variable)),
                _ => true,
            };
            if !matches {
                continue;
            }
        }

        // Convert to SearchResult
        filtered_results.push(SearchResult {
            id: node.id.to_string(),
            name: node.name.to_string(),
            description: node.content.as_ref().map(|s| s.to_string()),
            similarity: result.score as f64,
            metadata: Some(
                serde_json::to_string(&node.metadata).unwrap_or_else(|_| "{}".to_string()),
            ),
        });

        // Stop once we have enough results
        if filtered_results.len() >= limit {
            break;
        }
    }

    Ok(filtered_results)
}
