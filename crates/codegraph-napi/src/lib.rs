#![deny(clippy::all)]

use napi::bindgen_prelude::*;
use napi_derive::napi;

use codegraph_api::state::AppState;
use codegraph_core::{ConfigManager, IsolationLevel};
use std::sync::Arc;
use tokio::sync::Mutex;

// Global state - lazy initialized
static STATE: tokio::sync::OnceCell<Arc<Mutex<AppState>>> = tokio::sync::OnceCell::const_new();

async fn get_or_init_state() -> Result<Arc<Mutex<AppState>>> {
    STATE
        .get_or_try_init(|| async {
            let config = ConfigManager::new(None)
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
// Transaction Management
// ========================================

/// Begin a new transaction
#[napi]
pub async fn begin_transaction(isolation_level: Option<String>) -> Result<TransactionResult> {
    let state = get_or_init_state().await?;
    let state = state.lock().await;

    let iso_level = match isolation_level.as_deref() {
        Some("read-uncommitted") => IsolationLevel::ReadUncommitted,
        Some("read-committed") => IsolationLevel::ReadCommitted,
        Some("repeatable-read") => IsolationLevel::RepeatableRead,
        Some("serializable") => IsolationLevel::Serializable,
        _ => IsolationLevel::ReadCommitted,
    };

    let tx_id = state
        .transactional_graph
        .transaction_manager
        .begin_transaction(iso_level)
        .await
        .map_err(|e| Error::from_reason(format!("Failed to begin transaction: {}", e)))?;

    Ok(TransactionResult {
        transaction_id: tx_id.to_string(),
        isolation_level: format!("{:?}", iso_level),
        status: "active".to_string(),
    })
}

/// Commit a transaction
#[napi]
pub async fn commit_transaction(transaction_id: String) -> Result<TransactionResult> {
    let state = get_or_init_state().await?;
    let state = state.lock().await;

    let tx_id = uuid::Uuid::parse_str(&transaction_id)
        .map_err(|e| Error::from_reason(format!("Invalid transaction ID: {}", e)))?;

    state
        .transactional_graph
        .transaction_manager
        .commit_transaction(tx_id)
        .await
        .map_err(|e| Error::from_reason(format!("Failed to commit transaction: {}", e)))?;

    Ok(TransactionResult {
        transaction_id,
        isolation_level: "N/A".to_string(),
        status: "committed".to_string(),
    })
}

/// Rollback a transaction
#[napi]
pub async fn rollback_transaction(transaction_id: String) -> Result<TransactionResult> {
    let state = get_or_init_state().await?;
    let state = state.lock().await;

    let tx_id = uuid::Uuid::parse_str(&transaction_id)
        .map_err(|e| Error::from_reason(format!("Invalid transaction ID: {}", e)))?;

    state
        .transactional_graph
        .transaction_manager
        .rollback_transaction(tx_id)
        .await
        .map_err(|e| Error::from_reason(format!("Failed to rollback transaction: {}", e)))?;

    Ok(TransactionResult {
        transaction_id,
        isolation_level: "N/A".to_string(),
        status: "rolled_back".to_string(),
    })
}

/// Get transaction statistics
#[napi]
pub async fn get_transaction_stats() -> Result<TransactionStats> {
    let state = get_or_init_state().await?;
    let state = state.lock().await;

    let stats = state
        .transactional_graph
        .transaction_manager
        .get_transaction_stats()
        .await
        .map_err(|e| Error::from_reason(format!("Failed to get stats: {}", e)))?;

    Ok(TransactionStats {
        active_transactions: stats.active_transactions as u32,
        committed_transactions: stats.committed_transactions.to_string(),
        aborted_transactions: stats.aborted_transactions.to_string(),
        average_commit_time_ms: stats.average_commit_time_ms,
    })
}

// ========================================
// Version Management
// ========================================

/// Create a new version
#[napi]
pub async fn create_version(params: CreateVersionParams) -> Result<VersionResult> {
    let state = get_or_init_state().await?;
    let state = state.lock().await;

    let parent_ids: Vec<uuid::Uuid> = params
        .parents
        .unwrap_or_default()
        .iter()
        .map(|id| {
            uuid::Uuid::parse_str(id)
                .map_err(|e| Error::from_reason(format!("Invalid parent ID: {}", e)))
        })
        .collect::<Result<Vec<_>>>()?;

    let version_id = state
        .transactional_graph
        .version_manager
        .create_version(
            params.name.clone(),
            params.description.clone(),
            params.author.clone(),
            parent_ids,
        )
        .await
        .map_err(|e| Error::from_reason(format!("Failed to create version: {}", e)))?;

    Ok(VersionResult {
        version_id: version_id.to_string(),
        name: params.name,
        description: params.description,
        author: params.author,
        created_at: chrono::Utc::now().to_rfc3339(),
    })
}

/// List versions
#[napi]
pub async fn list_versions(limit: Option<u32>) -> Result<Vec<VersionResult>> {
    let state = get_or_init_state().await?;
    let state = state.lock().await;

    let versions = state
        .transactional_graph
        .version_manager
        .list_versions()
        .await
        .map_err(|e| Error::from_reason(format!("Failed to list versions: {}", e)))?;

    let limit = limit.unwrap_or(50) as usize;
    let results: Vec<VersionResult> = versions
        .into_iter()
        .take(limit)
        .map(|v| VersionResult {
            version_id: v.id.to_string(),
            name: v.name,
            description: v.description,
            author: v.author,
            created_at: v.timestamp.to_rfc3339(),
        })
        .collect();

    Ok(results)
}

/// Get version by ID
#[napi]
pub async fn get_version(version_id: String) -> Result<VersionResult> {
    let state = get_or_init_state().await?;
    let state = state.lock().await;

    let id = uuid::Uuid::parse_str(&version_id)
        .map_err(|e| Error::from_reason(format!("Invalid version ID: {}", e)))?;

    let version = state
        .transactional_graph
        .version_manager
        .get_version(id)
        .await
        .map_err(|e| Error::from_reason(format!("Failed to get version: {}", e)))?
        .ok_or_else(|| Error::from_reason("Version not found"))?;

    Ok(VersionResult {
        version_id: version.id.to_string(),
        name: version.name,
        description: version.description,
        author: version.author,
        created_at: version.timestamp.to_rfc3339(),
    })
}

/// Tag a version
#[napi]
pub async fn tag_version(version_id: String, tag: String) -> Result<bool> {
    let state = get_or_init_state().await?;
    let state = state.lock().await;

    let id = uuid::Uuid::parse_str(&version_id)
        .map_err(|e| Error::from_reason(format!("Invalid version ID: {}", e)))?;

    state
        .transactional_graph
        .version_manager
        .tag_version(id, tag)
        .await
        .map_err(|e| Error::from_reason(format!("Failed to tag version: {}", e)))?;

    Ok(true)
}

/// Compare two versions
#[napi]
pub async fn compare_versions(from_version: String, to_version: String) -> Result<VersionDiff> {
    let state = get_or_init_state().await?;
    let state = state.lock().await;

    let from_id = uuid::Uuid::parse_str(&from_version)
        .map_err(|e| Error::from_reason(format!("Invalid from version ID: {}", e)))?;

    let to_id = uuid::Uuid::parse_str(&to_version)
        .map_err(|e| Error::from_reason(format!("Invalid to version ID: {}", e)))?;

    let diff = state
        .transactional_graph
        .version_manager
        .compare_versions(from_id, to_id)
        .await
        .map_err(|e| Error::from_reason(format!("Failed to compare versions: {}", e)))?;

    Ok(VersionDiff {
        from_version,
        to_version,
        added_nodes: diff.added_nodes.len() as u32,
        modified_nodes: diff.modified_nodes.len() as u32,
        deleted_nodes: diff.deleted_nodes.len() as u32,
    })
}

// ========================================
// Branch Management
// ========================================

/// Create a new branch
#[napi]
pub async fn create_branch(params: CreateBranchParams) -> Result<BranchResult> {
    let state = get_or_init_state().await?;
    let state = state.lock().await;

    let from_id = uuid::Uuid::parse_str(&params.from)
        .map_err(|e| Error::from_reason(format!("Invalid version ID: {}", e)))?;

    state
        .transactional_graph
        .version_manager
        .create_branch(params.name.clone(), from_id)
        .await
        .map_err(|e| Error::from_reason(format!("Failed to create branch: {}", e)))?;

    Ok(BranchResult {
        name: params.name,
        head: params.from,
        created_at: chrono::Utc::now().to_rfc3339(),
        created_by: params.author,
    })
}

/// List branches
#[napi]
pub async fn list_branches() -> Result<Vec<BranchResult>> {
    let state = get_or_init_state().await?;
    let state = state.lock().await;

    let branches = state
        .transactional_graph
        .version_manager
        .list_branches()
        .await
        .map_err(|e| Error::from_reason(format!("Failed to list branches: {}", e)))?;

    let results: Vec<BranchResult> = branches
        .into_iter()
        .map(|b| BranchResult {
            name: b.name,
            head: b.head.to_string(),
            created_at: b.created_at.to_rfc3339(),
            created_by: b.created_by,
        })
        .collect();

    Ok(results)
}

/// Get branch by name
#[napi]
pub async fn get_branch(name: String) -> Result<BranchResult> {
    let state = get_or_init_state().await?;
    let state = state.lock().await;

    let branch = state
        .transactional_graph
        .version_manager
        .get_branch(name.clone())
        .await
        .map_err(|e| Error::from_reason(format!("Failed to get branch: {}", e)))?
        .ok_or_else(|| Error::from_reason("Branch not found"))?;

    Ok(BranchResult {
        name: branch.name,
        head: branch.head.to_string(),
        created_at: branch.created_at.to_rfc3339(),
        created_by: branch.created_by,
    })
}

/// Delete a branch
#[napi]
pub async fn delete_branch(name: String) -> Result<bool> {
    let state = get_or_init_state().await?;
    let state = state.lock().await;

    state
        .transactional_graph
        .version_manager
        .delete_branch(name)
        .await
        .map_err(|e| Error::from_reason(format!("Failed to delete branch: {}", e)))?;

    Ok(true)
}

/// Merge branches
#[napi]
pub async fn merge_branches(params: MergeBranchesParams) -> Result<MergeResult> {
    let state = get_or_init_state().await?;
    let state = state.lock().await;

    let result = state
        .transactional_graph
        .version_manager
        .merge_branches(params.source, params.target)
        .await
        .map_err(|e| Error::from_reason(format!("Failed to merge branches: {}", e)))?;

    Ok(MergeResult {
        success: result.success,
        conflicts: result.conflicts.len() as u32,
        merged_version_id: result.merged_version_id.map(|id| id.to_string()),
        merge_commit_message: result.merge_commit_message,
    })
}

// ========================================
// Utility Functions
// ========================================

/// Get the version of the CodeGraph addon
#[napi]
pub fn get_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

/// Initialize the CodeGraph system (optional - happens automatically on first call)
#[napi]
pub async fn initialize() -> Result<bool> {
    get_or_init_state().await?;
    Ok(true)
}
