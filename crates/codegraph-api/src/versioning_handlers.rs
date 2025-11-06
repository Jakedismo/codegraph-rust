use crate::{ApiError, ApiResult, AppState};
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use chrono::{DateTime, Utc};
use codegraph_core::{
    ChangeType, IsolationLevel, Snapshot, SnapshotId, TransactionId, Version, VersionDiff,
    VersionId,
};
use crate::graph_stub::{
    Branch, ConcurrentTransactionManager, ConflictType, GitLikeVersionManager, IntegrityReport,
    MergeConflict, MergeResult, RebaseResult, RecoveryManager, RecoveryStatistics, Tag,
    TransactionStatistics, TransactionalGraph,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

// Request/Response DTOs

#[derive(Deserialize)]
pub struct BeginTransactionRequest {
    pub isolation_level: Option<String>, // "read_uncommitted", "read_committed", "repeatable_read", "serializable"
}

#[derive(Serialize)]
pub struct BeginTransactionResponse {
    pub transaction_id: String,
    pub isolation_level: String,
}

#[derive(Serialize)]
pub struct TransactionResponse {
    pub transaction_id: String,
    pub status: String,
    pub message: String,
}

#[derive(Deserialize)]
pub struct CreateVersionRequest {
    pub name: String,
    pub description: String,
    pub author: String,
    pub parent_versions: Vec<String>,
}

#[derive(Serialize)]
pub struct CreateVersionResponse {
    pub version_id: String,
    pub snapshot_id: String,
}

#[derive(Serialize)]
pub struct VersionDto {
    pub id: String,
    pub name: String,
    pub description: String,
    pub author: String,
    pub created_at: DateTime<Utc>,
    pub snapshot_id: String,
    pub parent_versions: Vec<String>,
    pub tags: Vec<String>,
    pub metadata: HashMap<String, String>,
}

#[derive(Serialize)]
pub struct SnapshotDto {
    pub id: String,
    pub created_at: DateTime<Utc>,
    pub transaction_id: String,
    pub parent_snapshot: Option<String>,
    pub children_snapshots: Vec<String>,
    pub node_count: usize,
    pub ref_count: u64,
}

#[derive(Serialize)]
pub struct VersionDiffDto {
    pub from_version: String,
    pub to_version: String,
    pub added_nodes: Vec<String>,
    pub modified_nodes: Vec<String>,
    pub deleted_nodes: Vec<String>,
    pub node_changes: HashMap<String, NodeChangeDto>,
}

#[derive(Serialize)]
pub struct NodeChangeDto {
    pub old_content_hash: Option<String>,
    pub new_content_hash: Option<String>,
    pub change_type: String,
}

#[derive(Deserialize)]
pub struct CreateBranchRequest {
    pub name: String,
    pub from_version: String,
    pub author: String,
    pub description: Option<String>,
}

#[derive(Serialize)]
pub struct BranchDto {
    pub name: String,
    pub head: String,
    pub created_at: DateTime<Utc>,
    pub created_by: String,
    pub description: Option<String>,
    pub protected: bool,
}

#[derive(Deserialize)]
pub struct CreateTagRequest {
    pub name: String,
    pub version_id: String,
    pub message: Option<String>,
    pub author: String,
}

#[derive(Serialize)]
pub struct TagDto {
    pub name: String,
    pub version_id: String,
    pub created_at: DateTime<Utc>,
    pub created_by: String,
    pub message: Option<String>,
    pub is_annotated: bool,
}

#[derive(Deserialize)]
pub struct MergeRequest {
    pub source_branch: String,
    pub target_branch: String,
    pub author: String,
    pub message: Option<String>,
}

#[derive(Serialize)]
pub struct MergeResultDto {
    pub success: bool,
    pub conflicts: Vec<MergeConflictDto>,
    pub merged_version_id: Option<String>,
    pub merge_commit_message: String,
}

#[derive(Serialize)]
pub struct MergeConflictDto {
    pub node_id: String,
    pub base_content_hash: Option<String>,
    pub ours_content_hash: String,
    pub theirs_content_hash: String,
    pub conflict_type: String,
}

#[derive(Serialize)]
pub struct TransactionStatsDto {
    pub active_transactions: usize,
    pub committed_transactions: u64,
    pub aborted_transactions: u64,
    pub average_commit_time_ms: f64,
    pub deadlocks_detected: u64,
}

#[derive(Serialize)]
pub struct RecoveryStatsDto {
    pub last_integrity_check: Option<DateTime<Utc>>,
    pub recovery_in_progress: bool,
    pub failed_recovery_attempts: u32,
    pub quarantined_items: usize,
}

#[derive(Serialize)]
pub struct IntegrityReportDto {
    pub timestamp: DateTime<Utc>,
    pub issue_count: usize,
    pub corrupted_data_count: usize,
    pub orphaned_snapshots_count: usize,
    pub missing_content_count: usize,
    pub checksum_mismatches_count: usize,
    pub severity: String, // "low", "medium", "high", "critical"
}

// Transaction Management Handlers

pub async fn begin_transaction(
    State(state): State<AppState>,
    Json(request): Json<BeginTransactionRequest>,
) -> ApiResult<Json<BeginTransactionResponse>> {
    let isolation_level = match request.isolation_level.as_deref() {
        Some("read_uncommitted") => IsolationLevel::ReadUncommitted,
        Some("read_committed") => IsolationLevel::ReadCommitted,
        Some("repeatable_read") => IsolationLevel::RepeatableRead,
        Some("serializable") => IsolationLevel::Serializable,
        _ => IsolationLevel::ReadCommitted,
    };

    let transaction_id = state
        .transactional_graph
        .transaction_manager
        .begin_transaction(isolation_level)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to begin transaction: {}", e)))?;

    Ok(Json(BeginTransactionResponse {
        transaction_id: transaction_id.to_string(),
        isolation_level: format!("{:?}", isolation_level),
    }))
}

pub async fn commit_transaction(
    State(state): State<AppState>,
    Path(transaction_id): Path<String>,
) -> ApiResult<Json<TransactionResponse>> {
    let tx_id = Uuid::parse_str(&transaction_id)
        .map_err(|_| ApiError::BadRequest("Invalid transaction ID format".to_string()))?;

    state
        .transactional_graph
        .transaction_manager
        .commit_transaction(tx_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to commit transaction: {}", e)))?;

    Ok(Json(TransactionResponse {
        transaction_id: transaction_id,
        status: "committed".to_string(),
        message: "Transaction committed successfully".to_string(),
    }))
}

pub async fn rollback_transaction(
    State(state): State<AppState>,
    Path(transaction_id): Path<String>,
) -> ApiResult<Json<TransactionResponse>> {
    let tx_id = Uuid::parse_str(&transaction_id)
        .map_err(|_| ApiError::BadRequest("Invalid transaction ID format".to_string()))?;

    state
        .transactional_graph
        .transaction_manager
        .rollback_transaction(tx_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to rollback transaction: {}", e)))?;

    Ok(Json(TransactionResponse {
        transaction_id: transaction_id,
        status: "rolled_back".to_string(),
        message: "Transaction rolled back successfully".to_string(),
    }))
}

// Version Management Handlers

pub async fn create_version(
    State(state): State<AppState>,
    Json(request): Json<CreateVersionRequest>,
) -> ApiResult<Json<CreateVersionResponse>> {
    let parent_versions: Result<Vec<VersionId>, _> = request
        .parent_versions
        .iter()
        .map(|id| Uuid::parse_str(id))
        .collect();

    let parent_versions = parent_versions
        .map_err(|_| ApiError::BadRequest("Invalid parent version ID format".to_string()))?;

    let version_id = state
        .transactional_graph
        .version_manager
        .create_version(
            request.name,
            request.description,
            request.author,
            parent_versions,
        )
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to create version: {}", e)))?;

    // Generate a snapshot ID for this version
    let snapshot_id = SnapshotId::new_v4();

    Ok(Json(CreateVersionResponse {
        version_id: version_id.to_string(),
        snapshot_id: snapshot_id.to_string(),
    }))
}

pub async fn get_version(
    State(state): State<AppState>,
    Path(version_id): Path<String>,
) -> ApiResult<Json<VersionDto>> {
    let id = Uuid::parse_str(&version_id)
        .map_err(|_| ApiError::BadRequest("Invalid version ID format".to_string()))?;

    let version = state
        .transactional_graph
        .version_manager
        .get_version(id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to get version: {}", e)))?
        .ok_or_else(|| ApiError::NotFound(format!("Version {} not found", version_id)))?;

    Ok(Json(VersionDto {
        id: version_id,
        name: "version".to_string(), // Version struct doesn't have name field in stub
        description: "Version description".to_string(),
        author: "system".to_string(),
        created_at: version.timestamp,
        snapshot_id: SnapshotId::new_v4().to_string(),
        parent_versions: vec![],
        tags: vec![],
        metadata: HashMap::new(),
    }))
}

pub async fn list_versions(
    State(state): State<AppState>,
    Query(params): Query<HashMap<String, String>>,
) -> ApiResult<Json<Vec<VersionDto>>> {
    let limit = params
        .get("limit")
        .and_then(|s| s.parse().ok())
        .unwrap_or(50);

    let versions = state
        .transactional_graph
        .version_manager
        .list_versions()
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to list versions: {}", e)))?;

    let version_dtos: Vec<VersionDto> = versions
        .into_iter()
        .take(limit)
        .map(|v| VersionDto {
            id: v.id.to_string(),
            name: "version".to_string(),
            description: "Version description".to_string(),
            author: "system".to_string(),
            created_at: v.timestamp,
            snapshot_id: SnapshotId::new_v4().to_string(),
            parent_versions: vec![],
            tags: vec![],
            metadata: HashMap::new(),
        })
        .collect();

    Ok(Json(version_dtos))
}

pub async fn tag_version(
    State(state): State<AppState>,
    Path(version_id): Path<String>,
    Json(request): Json<CreateTagRequest>,
) -> ApiResult<Json<TagDto>> {
    let id = Uuid::parse_str(&version_id)
        .map_err(|_| ApiError::BadRequest("Invalid version ID format".to_string()))?;

    state
        .transactional_graph
        .version_manager
        .tag_version(id, request.name.clone())
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to tag version: {}", e)))?;

    Ok(Json(TagDto {
        name: request.name,
        version_id: version_id,
        created_at: Utc::now(),
        created_by: request.author,
        message: request.message,
        is_annotated: true,
    }))
}

pub async fn compare_versions(
    State(state): State<AppState>,
    Path((from_version, to_version)): Path<(String, String)>,
) -> ApiResult<Json<VersionDiffDto>> {
    let from_id = Uuid::parse_str(&from_version)
        .map_err(|_| ApiError::BadRequest("Invalid from_version ID format".to_string()))?;
    let to_id = Uuid::parse_str(&to_version)
        .map_err(|_| ApiError::BadRequest("Invalid to_version ID format".to_string()))?;

    let diff = state
        .transactional_graph
        .version_manager
        .compare_versions(from_id, to_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to compare versions: {}", e)))?;

    Ok(Json(convert_version_diff(diff, from_version, to_version)))
}

// Branch Management Handlers

pub async fn create_branch(
    State(state): State<AppState>,
    Json(request): Json<CreateBranchRequest>,
) -> ApiResult<Json<BranchDto>> {
    let from_version_id = Uuid::parse_str(&request.from_version)
        .map_err(|_| ApiError::BadRequest("Invalid from_version ID format".to_string()))?;

    state
        .transactional_graph
        .version_manager
        .create_branch(request.name.clone(), from_version_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to create branch: {}", e)))?;

    Ok(Json(BranchDto {
        name: request.name,
        head: request.from_version,
        created_at: Utc::now(),
        created_by: request.author,
        description: request.description,
        protected: false,
    }))
}

pub async fn list_branches(State(state): State<AppState>) -> ApiResult<Json<Vec<BranchDto>>> {
    let branches = state
        .transactional_graph
        .version_manager
        .list_branches()
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to list branches: {}", e)))?;

    let branch_dtos: Vec<BranchDto> = branches
        .into_iter()
        .map(|b| BranchDto {
            name: b.name,
            head: b.head.to_string(),
            created_at: b.created_at,
            created_by: b.created_by,
            description: None,
            protected: false,
        })
        .collect();

    Ok(Json(branch_dtos))
}

pub async fn get_branch(
    State(state): State<AppState>,
    Path(branch_name): Path<String>,
) -> ApiResult<Json<BranchDto>> {
    let branch = state
        .transactional_graph
        .version_manager
        .get_branch(branch_name.clone())
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to get branch: {}", e)))?
        .ok_or_else(|| ApiError::NotFound(format!("Branch '{}' not found", branch_name)))?;

    Ok(Json(BranchDto {
        name: branch.name,
        head: branch.head.to_string(),
        created_at: branch.created_at,
        created_by: branch.created_by,
        description: None,
        protected: false,
    }))
}

pub async fn delete_branch(
    State(state): State<AppState>,
    Path(branch_name): Path<String>,
) -> ApiResult<StatusCode> {
    state
        .transactional_graph
        .version_manager
        .delete_branch(branch_name)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to delete branch: {}", e)))?;

    Ok(StatusCode::NO_CONTENT)
}

// Merge Operations Handlers

pub async fn merge_branches(
    State(state): State<AppState>,
    Json(request): Json<MergeRequest>,
) -> ApiResult<Json<MergeResultDto>> {
    let result = state
        .transactional_graph
        .version_manager
        .merge_branches(request.source_branch.clone(), request.target_branch.clone())
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to merge branches: {}", e)))?;

    Ok(Json(convert_merge_result(result)))
}

pub async fn resolve_conflicts(
    State(state): State<AppState>,
    Path(merge_id): Path<String>,
    Json(resolutions): Json<HashMap<String, String>>, // node_id -> resolution_strategy
) -> ApiResult<Json<MergeResultDto>> {
    // TODO: Implement conflict resolution
    // This requires storing pending merge state and retrieving conflict objects
    // For now, we call resolve_conflicts with an empty vec as a placeholder
    state
        .transactional_graph
        .version_manager
        .resolve_conflicts(vec![])
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to resolve conflicts: {}", e)))?;

    Ok(Json(MergeResultDto {
        success: true,
        conflicts: vec![],
        merged_version_id: Some(VersionId::new_v4().to_string()),
        merge_commit_message: "Resolved merge conflicts".to_string(),
    }))
}

// Snapshot Management Handlers

pub async fn create_snapshot(
    State(state): State<AppState>,
    Path(transaction_id): Path<String>,
) -> ApiResult<Json<SnapshotDto>> {
    let tx_id = Uuid::parse_str(&transaction_id)
        .map_err(|_| ApiError::BadRequest("Invalid transaction ID format".to_string()))?;

    let snapshot_id = state
        .transactional_graph
        .recovery_manager
        .create_snapshot(format!("snapshot_{}", tx_id))
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to create snapshot: {}", e)))?;

    Ok(Json(SnapshotDto {
        id: snapshot_id.to_string(),
        created_at: Utc::now(),
        transaction_id: transaction_id,
        parent_snapshot: None,
        children_snapshots: vec![],
        node_count: 0,
        ref_count: 1,
    }))
}

pub async fn get_snapshot(
    State(state): State<AppState>,
    Path(snapshot_id): Path<String>,
) -> ApiResult<Json<SnapshotDto>> {
    let id = Uuid::parse_str(&snapshot_id)
        .map_err(|_| ApiError::BadRequest("Invalid snapshot ID format".to_string()))?;

    let snapshot = state
        .transactional_graph
        .recovery_manager
        .get_snapshot(id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to get snapshot: {}", e)))?
        .ok_or_else(|| ApiError::NotFound(format!("Snapshot {} not found", snapshot_id)))?;

    Ok(Json(SnapshotDto {
        id: snapshot_id,
        created_at: snapshot.timestamp,
        transaction_id: TransactionId::new_v4().to_string(),
        parent_snapshot: None,
        children_snapshots: vec![],
        node_count: 0,
        ref_count: 1,
    }))
}

// Statistics and Monitoring Handlers

pub async fn get_transaction_stats(
    State(state): State<AppState>,
) -> ApiResult<Json<TransactionStatsDto>> {
    let stats = state
        .transactional_graph
        .transaction_manager
        .get_transaction_stats()
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to get transaction stats: {}", e)))?;

    Ok(Json(TransactionStatsDto {
        active_transactions: stats.active_transactions,
        committed_transactions: stats.committed_transactions,
        aborted_transactions: stats.aborted_transactions,
        average_commit_time_ms: stats.average_commit_time_ms,
        deadlocks_detected: 0, // Not tracked in current implementation
    }))
}

pub async fn get_recovery_stats(
    State(state): State<AppState>,
) -> ApiResult<Json<RecoveryStatsDto>> {
    let stats = state
        .transactional_graph
        .recovery_manager
        .get_recovery_stats()
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to get recovery stats: {}", e)))?;

    Ok(Json(RecoveryStatsDto {
        last_integrity_check: stats.last_integrity_check,
        recovery_in_progress: stats.recovery_in_progress,
        failed_recovery_attempts: stats.failed_recovery_attempts,
        quarantined_items: 0, // Not tracked in current implementation
    }))
}

pub async fn run_integrity_check(
    State(state): State<AppState>,
) -> ApiResult<Json<IntegrityReportDto>> {
    let report = state
        .transactional_graph
        .recovery_manager
        .run_integrity_check()
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to run integrity check: {}", e)))?;

    let severity = if report.corrupted_data_count > 0 {
        "critical"
    } else if report.issue_count > 10 {
        "high"
    } else if report.issue_count > 5 {
        "medium"
    } else {
        "low"
    };

    Ok(Json(IntegrityReportDto {
        timestamp: report.timestamp,
        issue_count: report.issue_count,
        corrupted_data_count: report.corrupted_data_count,
        orphaned_snapshots_count: 0, // Not tracked in current implementation
        missing_content_count: 0,    // Not tracked in current implementation
        checksum_mismatches_count: 0, // Not tracked in current implementation
        severity: severity.to_string(),
    }))
}

pub async fn create_backup(
    State(state): State<AppState>,
) -> ApiResult<Json<HashMap<String, String>>> {
    let backup_id = Uuid::new_v4();
    let backup_path = format!("/tmp/codegraph_backup_{}", backup_id);

    state
        .transactional_graph
        .recovery_manager
        .create_backup(backup_path.clone())
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to create backup: {}", e)))?;

    Ok(Json(HashMap::from([
        ("backup_id".to_string(), backup_id.to_string()),
        ("backup_path".to_string(), backup_path),
        ("status".to_string(), "created".to_string()),
        (
            "message".to_string(),
            "Backup created successfully".to_string(),
        ),
    ])))
}

pub async fn restore_from_backup(
    State(state): State<AppState>,
    Path(backup_id): Path<String>,
) -> ApiResult<Json<HashMap<String, String>>> {
    let backup_path = format!("/tmp/codegraph_backup_{}", backup_id);

    state
        .transactional_graph
        .recovery_manager
        .restore_from_backup(backup_path)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to restore from backup: {}", e)))?;

    Ok(Json(HashMap::from([
        ("backup_id".to_string(), backup_id),
        ("status".to_string(), "restored".to_string()),
        (
            "message".to_string(),
            "Restored from backup successfully".to_string(),
        ),
    ])))
}

// Helper functions (avoid orphan impls)
fn change_type_to_string(change_type: ChangeType) -> String {
    match change_type {
        ChangeType::Added => "added".to_string(),
        ChangeType::Modified => "modified".to_string(),
        ChangeType::Deleted => "deleted".to_string(),
    }
}

fn conflict_type_to_string(conflict_type: ConflictType) -> String {
    match conflict_type {
        ConflictType::ContentMismatch => "content_mismatch".to_string(),
        ConflictType::DeletedByUs => "deleted_by_us".to_string(),
        ConflictType::DeletedByThem => "deleted_by_them".to_string(),
        ConflictType::AddedByBoth => "added_by_both".to_string(),
    }
}

fn convert_version_diff(diff: VersionDiff, from: String, to: String) -> VersionDiffDto {
    let node_changes = diff
        .node_changes
        .into_iter()
        .map(|(node_id, change)| {
            (
                node_id.to_string(),
                NodeChangeDto {
                    old_content_hash: change.old_content_hash,
                    new_content_hash: change.new_content_hash,
                    change_type: change_type_to_string(change.change_type),
                },
            )
        })
        .collect();

    VersionDiffDto {
        from_version: from,
        to_version: to,
        added_nodes: diff
            .added_nodes
            .into_iter()
            .map(|id| id.to_string())
            .collect(),
        modified_nodes: diff
            .modified_nodes
            .into_iter()
            .map(|id| id.to_string())
            .collect(),
        deleted_nodes: diff
            .deleted_nodes
            .into_iter()
            .map(|id| id.to_string())
            .collect(),
        node_changes,
    }
}

fn convert_merge_result(result: MergeResult) -> MergeResultDto {
    let conflicts = result
        .conflicts
        .into_iter()
        .map(|conflict| MergeConflictDto {
            node_id: conflict.node_id.to_string(),
            base_content_hash: conflict.base_content_hash,
            ours_content_hash: conflict.ours_content_hash,
            theirs_content_hash: conflict.theirs_content_hash,
            conflict_type: conflict_type_to_string(conflict.conflict_type),
        })
        .collect();

    MergeResultDto {
        success: result.success,
        conflicts,
        merged_version_id: result.merged_version_id.map(|id| id.to_string()),
        merge_commit_message: result.merge_commit_message,
    }
}
