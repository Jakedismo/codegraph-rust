#![deny(clippy::all)]

mod config;
mod errors;
#[cfg(feature = "cloud-surrealdb")]
mod graph;
mod search;
mod state;
mod types;

use napi::bindgen_prelude::*;
use napi_derive::napi;

use codegraph_api::state::AppState;
use codegraph_core::ConfigManager;
use std::sync::Arc;
use tokio::sync::Mutex;

pub use types::{
    CallChainNode,
    // Graph types
    CallerInfo,
    CircularDependency,
    CloudConfig,
    CouplingMetrics,
    CouplingMetricsResult,
    DependencyNode,
    DualModeSearchResult,
    EdgeTypeCount,
    EmbeddingStats,
    HubNode,
    NodeInfo,
    NodeLocation,
    NodeReference,
    SearchOptions,
    SearchResult,
};

// Global state - lazy initialized
static STATE: tokio::sync::OnceCell<Arc<Mutex<AppState>>> = tokio::sync::OnceCell::const_new();

pub(crate) async fn get_or_init_state() -> Result<Arc<Mutex<AppState>>> {
    STATE
        .get_or_try_init(|| async {
            let config = ConfigManager::load()
                .map_err(|e| Error::from_reason(format!("Failed to load config: {}", e)))?;
            let state = AppState::new(Arc::new(config))
                .await
                .map_err(|e| Error::from_reason(format!("Failed to create state: {}", e)))?;
            Ok(Arc::new(Mutex::new(state)))
        })
        .await
        .cloned()
}

// ========================================
// Utility Functions
// ========================================

/// Get the version of the CodeGraph addon
#[napi]
pub fn get_addon_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

/// Initialize the CodeGraph system (optional - happens automatically on first call)
#[napi]
pub async fn initialize() -> Result<bool> {
    get_or_init_state().await?;
    Ok(true)
}

// ========================================
// Search Functions
// ========================================

/// Semantic search with dual-mode support
#[napi]
pub async fn semantic_search(
    query: String,
    options: Option<SearchOptions>,
) -> Result<DualModeSearchResult> {
    search::semantic_search(query, options).await
}

/// Find similar functions by node ID
#[napi]
pub async fn search_similar_functions(
    node_id: String,
    limit: Option<u32>,
) -> Result<Vec<SearchResult>> {
    search::search_similar_functions(node_id, limit).await
}

// ========================================
// Configuration Helpers
// ========================================

pub use config::{
    get_cloud_config, get_config_path, get_embedding_stats, is_cloud_available, reload_config,
};

// ========================================
// Graph Analysis Functions
// ========================================

#[cfg(feature = "cloud-surrealdb")]
pub use graph::{
    calculate_coupling_metrics, detect_circular_dependencies, get_hub_nodes,
    get_reverse_dependencies, get_transitive_dependencies, trace_call_chain,
};
