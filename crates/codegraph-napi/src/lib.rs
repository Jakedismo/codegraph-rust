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
    BranchResult,
    CallChainNode,
    // Graph types
    CallerInfo,
    CircularDependency,
    CloudConfig,
    CouplingMetrics,
    CouplingMetricsResult,
    CreateBranchParams,
    CreateVersionParams,
    DependencyNode,
    DualModeSearchResult,
    EdgeTypeCount,
    EmbeddingStats,
    HubNode,
    MergeBranchesParams,
    MergeResult,
    NodeInfo,
    NodeLocation,
    NodeReference,
    SearchOptions,
    SearchResult,
    TransactionResult,
    TransactionStats,
    VersionDiff,
    VersionResult,
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

fn transactional_graph_removed(function: &str) -> Error {
    Error::from_reason(format!(
        "{} is unavailable because the transactional graph backend has been removed; use SurrealDB storage instead.",
        function
    ))
}

// ========================================
// Transaction Management
// ========================================

/// Begin a new transaction
#[napi]
pub async fn begin_transaction(isolation_level: Option<String>) -> Result<TransactionResult> {
    Err(transactional_graph_removed("begin_transaction"))
}

/// Commit a transaction
#[napi]
pub async fn commit_transaction(transaction_id: String) -> Result<TransactionResult> {
    Err(transactional_graph_removed("commit_transaction"))
}

/// Rollback a transaction
#[napi]
pub async fn rollback_transaction(transaction_id: String) -> Result<TransactionResult> {
    Err(transactional_graph_removed("rollback_transaction"))
}

/// Get transaction statistics
#[napi]
pub async fn get_transaction_stats() -> Result<TransactionStats> {
    Err(transactional_graph_removed("get_transaction_stats"))
}

// ========================================
// Version Management
// ========================================

/// Create a new version
#[napi]
pub async fn create_version(params: CreateVersionParams) -> Result<VersionResult> {
    Err(transactional_graph_removed("create_version"))
}

/// List versions
#[napi]
pub async fn list_versions(limit: Option<u32>) -> Result<Vec<VersionResult>> {
    Err(transactional_graph_removed("list_versions"))
}

/// Get version by ID
#[napi]
pub async fn get_version(version_id: String) -> Result<VersionResult> {
    Err(transactional_graph_removed("get_version"))
}

/// Tag a version
#[napi]
pub async fn tag_version(version_id: String, tag: String) -> Result<bool> {
    Err(transactional_graph_removed("tag_version"))
}

/// Compare two versions
#[napi]
pub async fn compare_versions(from_version: String, to_version: String) -> Result<VersionDiff> {
    Err(transactional_graph_removed("compare_versions"))
}

// ========================================
// Branch Management
// ========================================

/// Create a new branch
#[napi]
pub async fn create_branch(params: CreateBranchParams) -> Result<BranchResult> {
    Err(transactional_graph_removed("create_branch"))
}

/// List branches
#[napi]
pub async fn list_branches() -> Result<Vec<BranchResult>> {
    Err(transactional_graph_removed("list_branches"))
}

/// Get branch by name
#[napi]
pub async fn get_branch(name: String) -> Result<BranchResult> {
    Err(transactional_graph_removed("get_branch"))
}

/// Delete a branch
#[napi]
pub async fn delete_branch(name: String) -> Result<bool> {
    Err(transactional_graph_removed("delete_branch"))
}

/// Merge branches
#[napi]
pub async fn merge_branches(params: MergeBranchesParams) -> Result<MergeResult> {
    Err(transactional_graph_removed("merge_branches"))
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
// Graph Analysis Functions
// ========================================

#[cfg(feature = "cloud-surrealdb")]
pub use graph::{
    calculate_coupling_metrics, detect_circular_dependencies, get_hub_nodes,
    get_reverse_dependencies, get_transitive_dependencies, trace_call_chain,
};
