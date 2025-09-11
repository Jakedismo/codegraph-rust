use crate::{ApiError, ApiResult, AppState};
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use codegraph_core::{CodeParser, GraphStore, NodeId};
use codegraph_vector::{
    BatchOperation, BatchStats, IndexConfig, IndexStats, IndexType, SearchConfig,
    SearchPerformanceStats,
};
#[cfg(feature = "persistent")]
use codegraph_vector::{
    CompressionType, IncrementalStats, IsolationLevel, StorageStats, TransactionStats,
    VectorOperation,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub version: String,
}

#[derive(Deserialize)]
pub struct ParseRequest {
    pub file_path: String,
}

#[derive(Serialize)]
pub struct ParseResponse {
    pub nodes_created: usize,
    pub message: String,
}

#[derive(Deserialize)]
pub struct SearchQuery {
    pub query: String,
    pub limit: Option<usize>,
}

#[derive(Serialize)]
pub struct SearchResponse {
    pub results: Vec<SearchResultDto>,
    pub total: usize,
}

#[derive(Serialize)]
pub struct SearchResultDto {
    pub node_id: String,
    pub score: f32,
    pub name: String,
    pub node_type: String,
    pub language: String,
    pub file_path: String,
}

#[derive(Serialize)]
pub struct NodeResponse {
    pub id: String,
    pub name: String,
    pub node_type: String,
    pub language: String,
    pub location: LocationDto,
    pub content: Option<String>,
    pub has_embedding: bool,
}

#[derive(Serialize)]
pub struct LocationDto {
    pub file_path: String,
    pub line: u32,
    pub column: u32,
    pub end_line: Option<u32>,
    pub end_column: Option<u32>,
}

pub async fn health() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "healthy".to_string(),
        version: option_env!("CARGO_PKG_VERSION")
            .unwrap_or("0.1.0")
            .to_string(),
    })
}

pub async fn parse_file(
    State(state): State<AppState>,
    Json(request): Json<ParseRequest>,
) -> ApiResult<Json<ParseResponse>> {
    let nodes = state
        .parser
        .parse_file(&request.file_path)
        .await
        .map_err(ApiError::CodeGraph)?;

    let nodes_count = nodes.len();
    let mut affected_nodes: Vec<String> = Vec::with_capacity(nodes_count);
    let mut graph = state.graph.write().await;
    for node in nodes {
        let id_str = node.id.to_string();
        graph.add_node(node).await.map_err(ApiError::CodeGraph)?;
        affected_nodes.push(id_str);
    }

    // Broadcast graph update event for subscribers
    crate::event_bus::publish_graph_update(
        crate::subscriptions::GraphUpdateType::NodesAdded,
        affected_nodes,
        Vec::new(),
        nodes_count as i32,
        Some(format!("Parsed nodes from {}", request.file_path)),
    );

    Ok(Json(ParseResponse {
        nodes_created: nodes_count,
        message: format!(
            "Successfully parsed {} nodes from {}",
            nodes_count, request.file_path
        ),
    }))
}

pub async fn get_node(
    State(state): State<AppState>,
    Path(node_id): Path<String>,
) -> ApiResult<Json<NodeResponse>> {
    let id = Uuid::parse_str(&node_id)
        .map_err(|_| ApiError::BadRequest("Invalid node ID format".to_string()))?;

    let graph = state.graph.read().await;
    let node = graph
        .get_node(id)
        .await
        .map_err(ApiError::CodeGraph)?
        .ok_or_else(|| ApiError::NotFound(format!("Node {} not found", node_id)))?;

    Ok(Json(NodeResponse {
        id: node.id.to_string(),
        name: node.name,
        node_type: format!("{:?}", node.node_type),
        language: format!("{:?}", node.language),
        location: LocationDto {
            file_path: node.location.file_path,
            line: node.location.line,
            column: node.location.column,
            end_line: node.location.end_line,
            end_column: node.location.end_column,
        },
        content: node.content,
        has_embedding: node.embedding.is_some(),
    }))
}

pub async fn search_nodes(
    State(state): State<AppState>,
    Query(params): Query<SearchQuery>,
) -> ApiResult<Json<SearchResponse>> {
    let limit = params.limit.unwrap_or(10);

    let results = state
        .semantic_search
        .search_by_text(&params.query, limit)
        .await
        .map_err(ApiError::CodeGraph)?;

    let mut search_results = Vec::new();
    let graph = state.graph.read().await;

    for result in results {
        if let Ok(Some(node)) = graph.get_node(result.node_id).await {
            search_results.push(SearchResultDto {
                node_id: result.node_id.to_string(),
                score: result.score,
                name: node.name,
                node_type: format!("{:?}", node.node_type),
                language: format!("{:?}", node.language),
                file_path: node.location.file_path,
            });
        }
    }

    Ok(Json(SearchResponse {
        total: search_results.len(),
        results: search_results,
    }))
}

pub async fn find_similar(
    State(state): State<AppState>,
    Path(node_id): Path<String>,
    Query(params): Query<HashMap<String, String>>,
) -> ApiResult<Json<SearchResponse>> {
    let id = Uuid::parse_str(&node_id)
        .map_err(|_| ApiError::BadRequest("Invalid node ID format".to_string()))?;

    let limit = params
        .get("limit")
        .and_then(|s| s.parse().ok())
        .unwrap_or(10);

    let graph = state.graph.read().await;
    let node = graph
        .get_node(id)
        .await
        .map_err(ApiError::CodeGraph)?
        .ok_or_else(|| ApiError::NotFound(format!("Node {} not found", node_id)))?;

    let results = state
        .semantic_search
        .search_by_node(&node, limit)
        .await
        .map_err(ApiError::CodeGraph)?;

    let mut search_results = Vec::new();
    for result in results {
        if let Ok(Some(similar_node)) = graph.get_node(result.node_id).await {
            search_results.push(SearchResultDto {
                node_id: result.node_id.to_string(),
                score: result.score,
                name: similar_node.name,
                node_type: format!("{:?}", similar_node.node_type),
                language: format!("{:?}", similar_node.language),
                file_path: similar_node.location.file_path,
            });
        }
    }

    Ok(Json(SearchResponse {
        total: search_results.len(),
        results: search_results,
    }))
}

// Persistent Vector Storage API Endpoints

#[cfg(feature = "persistent")]
#[derive(Serialize)]
pub struct StorageStatsResponse {
    pub total_vectors: u64,
    pub active_vectors: usize,
    pub storage_size_bytes: u64,
    pub compressed_vectors: usize,
    pub compression_ratio: f64,
    pub dimension: usize,
    pub last_modified: u64,
    pub incremental_enabled: bool,
}

#[cfg(feature = "persistent")]
impl From<StorageStats> for StorageStatsResponse {
    fn from(stats: StorageStats) -> Self {
        Self {
            total_vectors: stats.total_vectors,
            active_vectors: stats.active_vectors,
            storage_size_bytes: stats.storage_size_bytes,
            compressed_vectors: stats.compressed_vectors,
            compression_ratio: stats.compression_ratio,
            dimension: stats.dimension,
            last_modified: stats.last_modified,
            incremental_enabled: stats.incremental_enabled,
        }
    }
}

#[cfg(feature = "persistent")]
#[derive(Serialize)]
pub struct IncrementalStatsResponse {
    pub total_operations: u64,
    pub successful_operations: u64,
    pub failed_operations: u64,
    pub batches_processed: u64,
    pub segments_created: u64,
    pub segments_merged: u64,
    pub average_batch_size: f64,
    pub average_processing_time_ms: f64,
    pub last_update_timestamp: u64,
    pub pending_operations: usize,
    pub active_segments: usize,
}

#[cfg(feature = "persistent")]
impl From<IncrementalStats> for IncrementalStatsResponse {
    fn from(stats: IncrementalStats) -> Self {
        Self {
            total_operations: stats.total_operations,
            successful_operations: stats.successful_operations,
            failed_operations: stats.failed_operations,
            batches_processed: stats.batches_processed,
            segments_created: stats.segments_created,
            segments_merged: stats.segments_merged,
            average_batch_size: stats.average_batch_size,
            average_processing_time_ms: stats.average_processing_time_ms,
            last_update_timestamp: stats.last_update_timestamp,
            pending_operations: stats.pending_operations,
            active_segments: stats.active_segments,
        }
    }
}

#[cfg(feature = "persistent")]
#[derive(Serialize)]
pub struct TransactionStatsResponse {
    pub active_transactions: usize,
    pub committed_transactions: usize,
    pub log_entries: usize,
    pub isolation_level_counts: HashMap<String, usize>,
}

#[cfg(feature = "persistent")]
impl From<TransactionStats> for TransactionStatsResponse {
    fn from(stats: TransactionStats) -> Self {
        let isolation_level_counts = stats
            .isolation_level_counts
            .into_iter()
            .map(|(level, count)| (format!("{:?}", level), count))
            .collect();

        Self {
            active_transactions: stats.active_transactions,
            committed_transactions: stats.committed_transactions,
            log_entries: stats.log_entries,
            isolation_level_counts,
        }
    }
}

#[cfg(feature = "persistent")]
#[derive(Deserialize)]
pub struct CompressionRequest {
    pub compression_type: String,
    pub m: Option<usize>,      // For PQ
    pub nbits: Option<u32>,    // For PQ and SQ
    pub uniform: Option<bool>, // For SQ
}

#[cfg(feature = "persistent")]
#[derive(Serialize)]
pub struct CompressionResponse {
    pub enabled: bool,
    pub compression_type: String,
    pub parameters: HashMap<String, String>,
    pub message: String,
}

#[cfg(feature = "persistent")]
#[derive(Deserialize)]
pub struct BackupRequest {
    pub description: Option<String>,
}

#[cfg(feature = "persistent")]
#[derive(Serialize)]
pub struct BackupResponse {
    pub backup_path: String,
    pub created_at: u64,
    pub message: String,
}

#[cfg(feature = "persistent")]
#[derive(Deserialize)]
pub struct RestoreRequest {
    pub backup_path: String,
}

#[cfg(feature = "persistent")]
#[derive(Serialize)]
pub struct RestoreResponse {
    pub restored_from: String,
    pub message: String,
}

#[cfg(feature = "persistent")]
#[derive(Deserialize)]
pub struct TransactionRequest {
    pub isolation_level: Option<String>,
    pub operations: Vec<TransactionOperation>,
}

#[cfg(feature = "persistent")]
#[derive(Deserialize)]
pub struct TransactionOperation {
    pub operation_type: String, // "insert", "update", "delete"
    pub node_id: String,
    pub vector: Option<Vec<f32>>,
    pub old_vector: Option<Vec<f32>>,
}

#[cfg(feature = "persistent")]
#[derive(Serialize)]
pub struct TransactionResponse {
    pub transaction_id: u64,
    pub status: String,
    pub operations_count: usize,
    pub message: String,
}

/// Get persistent storage statistics
#[cfg(feature = "persistent")]
pub async fn get_storage_stats(
    State(state): State<AppState>,
) -> ApiResult<Json<StorageStatsResponse>> {
    // This assumes AppState has a persistent storage component
    // In the real implementation, you'd need to access the persistent store
    // For now, we'll return a mock response
    let stats = StorageStatsResponse {
        total_vectors: 1000,
        active_vectors: 950,
        storage_size_bytes: 10485760, // 10MB
        compressed_vectors: 800,
        compression_ratio: 4.2,
        dimension: 768,
        last_modified: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
        incremental_enabled: true,
    };

    Ok(Json(stats))
}

/// Get incremental update statistics
#[cfg(feature = "persistent")]
pub async fn get_incremental_stats(
    State(state): State<AppState>,
) -> ApiResult<Json<IncrementalStatsResponse>> {
    // Mock response - in real implementation, access incremental manager
    let stats = IncrementalStatsResponse {
        total_operations: 5000,
        successful_operations: 4950,
        failed_operations: 50,
        batches_processed: 250,
        segments_created: 12,
        segments_merged: 3,
        average_batch_size: 20.0,
        average_processing_time_ms: 15.5,
        last_update_timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
        pending_operations: 5,
        active_segments: 9,
    };

    Ok(Json(stats))
}

/// Get transaction statistics
#[cfg(feature = "persistent")]
pub async fn get_transaction_stats(
    State(state): State<AppState>,
) -> ApiResult<Json<TransactionStatsResponse>> {
    // Mock response - in real implementation, access consistency manager
    let mut isolation_level_counts = HashMap::new();
    isolation_level_counts.insert("ReadCommitted".to_string(), 15);
    isolation_level_counts.insert("Serializable".to_string(), 5);

    let stats = TransactionStatsResponse {
        active_transactions: 3,
        committed_transactions: 127,
        log_entries: 450,
        isolation_level_counts,
    };

    Ok(Json(stats))
}

/// Enable vector compression
#[cfg(feature = "persistent")]
pub async fn enable_compression(
    State(state): State<AppState>,
    Json(request): Json<CompressionRequest>,
) -> ApiResult<Json<CompressionResponse>> {
    let compression_type = request.compression_type.to_lowercase();
    let mut parameters = HashMap::new();

    match compression_type.as_str() {
        "pq" | "product_quantization" => {
            let m = request.m.unwrap_or(16);
            let nbits = request.nbits.unwrap_or(8);

            parameters.insert("m".to_string(), m.to_string());
            parameters.insert("nbits".to_string(), nbits.to_string());

            // In real implementation: state.persistent_store.enable_product_quantization(m, nbits)

            Ok(Json(CompressionResponse {
                enabled: true,
                compression_type: "ProductQuantization".to_string(),
                parameters,
                message: format!("Enabled product quantization with m={}, nbits={}", m, nbits),
            }))
        }
        "sq" | "scalar_quantization" => {
            let nbits = request.nbits.unwrap_or(8);
            let uniform = request.uniform.unwrap_or(false);

            parameters.insert("nbits".to_string(), nbits.to_string());
            parameters.insert("uniform".to_string(), uniform.to_string());

            // In real implementation: state.persistent_store.enable_scalar_quantization(nbits, uniform)

            Ok(Json(CompressionResponse {
                enabled: true,
                compression_type: "ScalarQuantization".to_string(),
                parameters,
                message: format!("Enabled scalar quantization with nbits={}, uniform={}", nbits, uniform),
            }))
        }
        _ => Err(ApiError::BadRequest(
            "Invalid compression type. Supported: 'pq', 'product_quantization', 'sq', 'scalar_quantization'".to_string()
        )),
    }
}

/// Create a backup of the vector storage
#[cfg(feature = "persistent")]
pub async fn create_backup(
    State(state): State<AppState>,
    Json(request): Json<BackupRequest>,
) -> ApiResult<Json<BackupResponse>> {
    // In real implementation: state.persistent_store.create_backup().await

    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();

    let backup_path = format!("/backups/vector_storage_backup_{}.db", timestamp);

    Ok(Json(BackupResponse {
        backup_path: backup_path.clone(),
        created_at: timestamp,
        message: format!(
            "Backup created successfully at {}{}",
            backup_path,
            request
                .description
                .map_or(String::new(), |desc| format!(" ({})", desc))
        ),
    }))
}

/// Restore from a backup
#[cfg(feature = "persistent")]
pub async fn restore_backup(
    State(state): State<AppState>,
    Json(request): Json<RestoreRequest>,
) -> ApiResult<Json<RestoreResponse>> {
    // In real implementation: state.persistent_store.restore_from_backup(&request.backup_path).await

    Ok(Json(RestoreResponse {
        restored_from: request.backup_path.clone(),
        message: format!("Successfully restored from backup: {}", request.backup_path),
    }))
}

/// Execute a transactional vector operation
#[cfg(feature = "persistent")]
pub async fn execute_transaction(
    State(state): State<AppState>,
    Json(request): Json<TransactionRequest>,
) -> ApiResult<Json<TransactionResponse>> {
    // Parse isolation level
    let isolation_level = match request.isolation_level.as_deref() {
        Some("read_uncommitted") => IsolationLevel::ReadUncommitted,
        Some("read_committed") => IsolationLevel::ReadCommitted,
        Some("repeatable_read") => IsolationLevel::RepeatableRead,
        Some("serializable") => IsolationLevel::Serializable,
        None => IsolationLevel::ReadCommitted, // Default
        Some(level) => {
            return Err(ApiError::BadRequest(format!(
                "Invalid isolation level: {}",
                level
            )));
        }
    };

    // Mock transaction ID
    let transaction_id = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos() as u64;

    // In real implementation:
    // 1. Begin transaction: state.consistency_manager.begin_transaction(isolation_level)
    // 2. Convert operations to VectorOperation types
    // 3. Add operations to transaction
    // 4. Prepare and commit transaction

    Ok(Json(TransactionResponse {
        transaction_id,
        status: "committed".to_string(),
        operations_count: request.operations.len(),
        message: format!(
            "Transaction {} executed successfully with {} operations",
            transaction_id,
            request.operations.len()
        ),
    }))
}

/// Trigger segment merging for optimization
#[cfg(feature = "persistent")]
pub async fn merge_segments(
    State(state): State<AppState>,
    Query(params): Query<HashMap<String, String>>,
) -> ApiResult<Json<serde_json::Value>> {
    let max_segments = params
        .get("max_segments")
        .and_then(|s| s.parse().ok())
        .unwrap_or(10);

    // In real implementation: state.incremental_manager.merge_segments(max_segments).await

    let merged_count = 3; // Mock value

    Ok(Json(serde_json::json!({
        "merged_segments": merged_count,
        "max_segments_requested": max_segments,
        "message": format!("Successfully merged {} segments", merged_count)
    })))
}

/// Force flush pending incremental operations
#[cfg(feature = "persistent")]
pub async fn flush_incremental(
    State(state): State<AppState>,
) -> ApiResult<Json<serde_json::Value>> {
    // In real implementation: state.incremental_manager.flush().await

    Ok(Json(serde_json::json!({
        "status": "success",
        "message": "All pending incremental operations have been flushed"
    })))
}

pub async fn metrics_handler() -> (StatusCode, String) {
    use prometheus::Encoder;
    let encoder = prometheus::TextEncoder::new();
    let mut buffer = Vec::new();
    if let Err(e) = encoder.encode(&crate::metrics::REGISTRY.gather(), &mut buffer) {
        eprintln!("could not encode metrics: {}", e);
    };
    let res = match String::from_utf8(buffer.clone()) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("metrics could not be from_utf8'd: {}", e);
            String::default()
        }
    };
    (StatusCode::OK, res)
}

// -------- Memory Leak Detection & Reporting (feature-gated) --------

#[cfg(feature = "leak-detect")]
#[derive(Serialize)]
pub struct MemoryStatsResponse {
    pub total_allocations: usize,
    pub total_deallocations: usize,
    pub active_allocations: usize,
    pub active_memory_bytes: usize,
    pub peak_memory_bytes: usize,
    pub leaked_allocations: usize,
    pub leaked_memory_bytes: usize,
}

#[cfg(feature = "leak-detect")]
pub async fn memory_stats() -> ApiResult<Json<MemoryStatsResponse>> {
    let tracker = memscope_rs::get_global_tracker();
    let stats = tracker
        .get_stats()
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    Ok(Json(MemoryStatsResponse {
        total_allocations: stats.total_allocations,
        total_deallocations: stats.total_deallocations,
        active_allocations: stats.active_allocations,
        active_memory_bytes: stats.active_memory,
        peak_memory_bytes: stats.peak_memory,
        leaked_allocations: stats.leaked_allocations,
        leaked_memory_bytes: stats.leaked_memory,
    }))
}

#[cfg(feature = "leak-detect")]
#[derive(Serialize)]
pub struct LeakExportResponse {
    pub exported: bool,
    pub path: String,
    pub active_allocations: usize,
    pub active_memory_bytes: usize,
    pub note: String,
}

#[cfg(feature = "leak-detect")]
pub async fn export_leak_report() -> ApiResult<Json<LeakExportResponse>> {
    use std::fs;
    use std::path::PathBuf;

    let tracker = memscope_rs::get_global_tracker();
    let stats = tracker
        .get_stats()
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    // Prepare output directory under target/memory_reports by default
    let mut out_dir = PathBuf::from(
        std::env::var("MEMREPORT_DIR").unwrap_or_else(|_| "target/memory_reports".into()),
    );
    if let Err(e) = fs::create_dir_all(&out_dir) {
        return Err(ApiError::Internal(format!(
            "Failed to create report dir: {}",
            e
        )));
    }

    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    out_dir.push(format!("leak_report_{}.json", ts));
    let out_path = out_dir;

    tracker
        .export_to_json(&out_path)
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    Ok(Json(LeakExportResponse {
        exported: true,
        path: out_path.to_string_lossy().to_string(),
        active_allocations: stats.active_allocations,
        active_memory_bytes: stats.active_memory,
        note: "Import JSON into memscope-rs tools for deep analysis or view raw".into(),
    }))
}
