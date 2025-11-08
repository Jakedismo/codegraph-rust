// ABOUTME: NAPI type definitions for TypeScript bindings
// ABOUTME: All #[napi(object)] structs for auto-generated .d.ts

use napi_derive::napi;

// ========================================
// Transaction Types
// ========================================

#[napi(object)]
pub struct TransactionResult {
    pub transaction_id: String,
    pub isolation_level: String,
    pub status: String,
}

#[napi(object)]
pub struct TransactionStats {
    pub active_transactions: u32,
    pub committed_transactions: String,
    pub aborted_transactions: String,
    pub average_commit_time_ms: f64,
}

// ========================================
// Version Types
// ========================================

#[napi(object)]
pub struct VersionResult {
    pub version_id: String,
    pub name: String,
    pub description: String,
    pub author: String,
    pub created_at: String,
}

#[napi(object)]
pub struct CreateVersionParams {
    pub name: String,
    pub description: String,
    pub author: String,
    pub parents: Option<Vec<String>>,
}

#[napi(object)]
pub struct VersionDiff {
    pub from_version: String,
    pub to_version: String,
    pub added_nodes: u32,
    pub modified_nodes: u32,
    pub deleted_nodes: u32,
}

// ========================================
// Branch Types
// ========================================

#[napi(object)]
pub struct BranchResult {
    pub name: String,
    pub head: String,
    pub created_at: String,
    pub created_by: String,
}

#[napi(object)]
pub struct CreateBranchParams {
    pub name: String,
    pub from: String,
    pub author: String,
    pub description: Option<String>,
}

#[napi(object)]
pub struct MergeBranchesParams {
    pub source: String,
    pub target: String,
    pub author: String,
    pub message: Option<String>,
}

#[napi(object)]
pub struct MergeResult {
    pub success: bool,
    pub conflicts: u32,
    pub merged_version_id: Option<String>,
    pub merge_commit_message: String,
}

// ========================================
// Search Types
// ========================================

#[napi(object)]
pub struct SearchResult {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub similarity: f64,
    pub metadata: Option<String>,
}

#[napi(object)]
#[derive(Default)]
pub struct SearchOptions {
    pub query: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
    pub min_similarity: Option<f64>,
    pub filter_by_type: Option<String>,
}

#[napi(object)]
pub struct DualModeSearchResult {
    pub local_results: Vec<SearchResult>,
    pub cloud_results: Option<Vec<SearchResult>>,
    pub reranked_results: Option<Vec<SearchResult>>,
    pub total_count: u32,
    pub search_time_ms: f64,
}

// ========================================
// Configuration Types
// ========================================

#[napi(object)]
pub struct CloudConfig {
    pub jina_enabled: bool,
    pub jina_model: String,
    pub jina_reranking_enabled: bool,
    pub surrealdb_enabled: bool,
    pub surrealdb_url: Option<String>,
}

#[napi(object)]
pub struct EmbeddingStats {
    pub provider: String,
    pub model: String,
    pub dimension: u32,
    pub total_embeddings: u32,
    pub cache_hit_rate: f64,
}
