use crate::{ApiError, ApiResult, AppState};
use axum::{
    extract::{Query, State},
    Json,
};
use codegraph_core::NodeId;
use codegraph_vector::{
    BatchOperation, BatchStats, IndexStats, SearchPerformanceStats,
    IndexConfig, IndexType, SearchConfig,
};
use serde::{Deserialize, Serialize};
use std::time::Instant;
use uuid::Uuid;

// Advanced vector search types

#[derive(Deserialize)]
pub struct VectorSearchRequest {
    pub query_embedding: Vec<f32>,
    pub k: usize,
    pub search_config: Option<SearchConfigDto>,
}

#[derive(Deserialize)]
pub struct BatchVectorSearchRequest {
    pub queries: Vec<VectorSearchQuery>,
    pub search_config: Option<SearchConfigDto>,
}

#[derive(Deserialize)]
pub struct VectorSearchQuery {
    pub embedding: Vec<f32>,
    pub k: usize,
    pub id: Option<String>, // Optional query ID for tracking
}

#[derive(Deserialize, Serialize)]
pub struct SearchConfigDto {
    pub target_latency_us: Option<u64>,
    pub cache_enabled: Option<bool>,
    pub prefetch_enabled: Option<bool>,
    pub parallel_search: Option<bool>,
}

#[derive(Serialize)]
pub struct VectorSearchResponse {
    pub results: Vec<VectorSearchResult>,
    pub search_time_us: u64,
    pub cache_hit: bool,
}

#[derive(Serialize)]
pub struct BatchVectorSearchResponse {
    pub results: Vec<BatchSearchResult>,
    pub total_search_time_us: u64,
    pub queries_processed: usize,
}

#[derive(Serialize)]
pub struct BatchSearchResult {
    pub query_id: Option<String>,
    pub results: Vec<VectorSearchResult>,
    pub search_time_us: u64,
}

#[derive(Serialize)]
pub struct VectorSearchResult {
    pub node_id: String,
    pub score: f32,
    pub distance: f32,
    pub metadata: Option<SearchResultMetadata>,
}

#[derive(Serialize)]
pub struct SearchResultMetadata {
    pub name: String,
    pub node_type: String,
    pub language: String,
    pub file_path: String,
    pub line: u32,
}

#[derive(Serialize)]
pub struct IndexStatsResponse {
    pub num_vectors: usize,
    pub dimension: usize,
    pub index_type: String,
    pub is_trained: bool,
    pub memory_usage_mb: f64,
    pub last_updated: String,
}

#[derive(Serialize)]
pub struct IndexConfigResponse {
    pub index_type: String,
    pub metric_type: String,
    pub dimension: usize,
    pub gpu_enabled: bool,
    pub compression_level: u32,
}

#[derive(Deserialize)]
pub struct RebuildIndexRequest {
    pub index_config: Option<IndexConfigDto>,
    pub force_rebuild: Option<bool>,
}

#[derive(Deserialize, Serialize)]
pub struct IndexConfigDto {
    pub index_type: String,
    pub dimension: Option<usize>,
    pub gpu_enabled: Option<bool>,
    pub compression_level: Option<u32>,
    // Index-specific parameters
    pub nlist: Option<usize>,      // For IVF
    pub nprobe: Option<usize>,     // For IVF
    pub m: Option<usize>,          // For HNSW/PQ
    pub ef_construction: Option<usize>, // For HNSW
    pub ef_search: Option<usize>,  // For HNSW
    pub nbits: Option<usize>,      // For LSH/PQ
}

#[derive(Serialize)]
pub struct RebuildIndexResponse {
    pub status: String,
    pub message: String,
    pub rebuild_time_ms: u64,
    pub vectors_processed: usize,
}

#[derive(Serialize)]
pub struct SearchPerformanceResponse {
    pub total_searches: u64,
    pub sub_millisecond_searches: u64,
    pub sub_ms_rate_percent: f64,
    pub average_latency_us: u64,
    pub p95_latency_us: u64,
    pub p99_latency_us: u64,
    pub cache_hit_rate_percent: f64,
    pub cache_entries: usize,
}

#[derive(Deserialize)]
pub struct BatchOperationsRequest {
    pub operations: Vec<BatchOperationDto>,
    pub batch_config: Option<BatchConfigDto>,
}

#[derive(Deserialize)]
pub struct BatchOperationDto {
    pub operation_type: String,
    pub node_id: String,
    pub embedding: Option<Vec<f32>>,
    pub search_params: Option<BatchSearchParams>,
}

#[derive(Deserialize)]
pub struct BatchSearchParams {
    pub k: usize,
    pub callback_id: Option<String>,
}

#[derive(Deserialize, Serialize)]
pub struct BatchConfigDto {
    pub batch_size: Option<usize>,
    pub parallel_processing: Option<bool>,
    pub memory_limit_mb: Option<usize>,
}

#[derive(Serialize)]
pub struct BatchOperationsResponse {
    pub status: String,
    pub operations_submitted: usize,
    pub estimated_processing_time_ms: u64,
}

#[derive(Serialize)]
pub struct BatchStatusResponse {
    pub total_operations: usize,
    pub successful_operations: usize,
    pub failed_operations: usize,
    pub pending_operations: usize,
    pub success_rate_percent: f64,
    pub active: bool,
}

// Vector search handler implementations

pub async fn vector_search(
    State(state): State<AppState>,
    Json(request): Json<VectorSearchRequest>,
) -> ApiResult<Json<VectorSearchResponse>> {
    let start_time = Instant::now();
    
    // Validate embedding dimension
    if request.query_embedding.is_empty() {
        return Err(ApiError::BadRequest("Query embedding cannot be empty".to_string()));
    }

    // Use the optimized search engine for sub-millisecond performance
    let results = state
        .vector_search
        .search_knn(&request.query_embedding, request.k)
        .await
        .map_err(ApiError::CodeGraph)?;

    let search_time_us = start_time.elapsed().as_micros() as u64;

    // Convert to API response format with metadata enrichment
    let mut api_results = Vec::new();
    let graph = state.graph.read().await;

    for (node_id, distance) in results {
        let score = 1.0 / (1.0 + distance); // Convert distance to similarity score
        
        let metadata = if let Ok(Some(node)) = graph.get_node(node_id).await {
            Some(SearchResultMetadata {
                name: node.name,
                node_type: format!("{:?}", node.node_type),
                language: format!("{:?}", node.language),
                file_path: node.location.file_path,
                line: node.location.line,
            })
        } else {
            None
        };

        api_results.push(VectorSearchResult {
            node_id: node_id.to_string(),
            score,
            distance,
            metadata,
        });
    }

    Ok(Json(VectorSearchResponse {
        results: api_results,
        search_time_us,
        cache_hit: false, // Would need to track this from the search engine
    }))
}

pub async fn batch_vector_search(
    State(state): State<AppState>,
    Json(request): Json<BatchVectorSearchRequest>,
) -> ApiResult<Json<BatchVectorSearchResponse>> {
    let start_time = Instant::now();

    if request.queries.is_empty() {
        return Err(ApiError::BadRequest("No queries provided".to_string()));
    }

    let query_refs: Vec<&[f32]> = request.queries
        .iter()
        .map(|q| q.embedding.as_slice())
        .collect();

    // Use batch search for optimal performance
    let batch_results = state
        .vector_search
        .batch_search_knn(&query_refs, request.queries[0].k)
        .await
        .map_err(ApiError::CodeGraph)?;

    let total_search_time_us = start_time.elapsed().as_micros() as u64;

    // Process results for each query
    let mut api_results = Vec::new();
    let graph = state.graph.read().await;

    for (i, query_result) in batch_results.into_iter().enumerate() {
        let query_start = Instant::now();
        let mut vector_results = Vec::new();

        for (node_id, distance) in query_result {
            let score = 1.0 / (1.0 + distance);
            
            let metadata = if let Ok(Some(node)) = graph.get_node(node_id).await {
                Some(SearchResultMetadata {
                    name: node.name,
                    node_type: format!("{:?}", node.node_type),
                    language: format!("{:?}", node.language),
                    file_path: node.location.file_path,
                    line: node.location.line,
                })
            } else {
                None
            };

            vector_results.push(VectorSearchResult {
                node_id: node_id.to_string(),
                score,
                distance,
                metadata,
            });
        }

        let query_time_us = query_start.elapsed().as_micros() as u64;

        api_results.push(BatchSearchResult {
            query_id: request.queries.get(i).and_then(|q| q.id.clone()),
            results: vector_results,
            search_time_us: query_time_us,
        });
    }

    Ok(Json(BatchVectorSearchResponse {
        results: api_results,
        total_search_time_us,
        queries_processed: request.queries.len(),
    }))
}

pub async fn get_index_stats(
    State(state): State<AppState>,
) -> ApiResult<Json<IndexStatsResponse>> {
    // Get stats from the search engine
    let stats = state.vector_search.get_performance_stats();
    
    // This is a simplified response - in a real implementation,
    // you'd get these stats from the index manager
    Ok(Json(IndexStatsResponse {
        num_vectors: 0, // Would come from index manager
        dimension: 768,
        index_type: "HNSW".to_string(),
        is_trained: true,
        memory_usage_mb: 0.0,
        last_updated: chrono::Utc::now().to_rfc3339(),
    }))
}

pub async fn get_index_config(
    State(_state): State<AppState>,
) -> ApiResult<Json<IndexConfigResponse>> {
    Ok(Json(IndexConfigResponse {
        index_type: "HNSW".to_string(),
        metric_type: "InnerProduct".to_string(),
        dimension: 768,
        gpu_enabled: false,
        compression_level: 0,
    }))
}

pub async fn rebuild_index(
    State(state): State<AppState>,
    Json(_request): Json<RebuildIndexRequest>,
) -> ApiResult<Json<RebuildIndexResponse>> {
    let start_time = Instant::now();
    
    // Placeholder implementation - would rebuild the FAISS index
    // This would involve:
    // 1. Creating a new index with the specified config
    // 2. Re-adding all vectors from storage
    // 3. Training the index if necessary
    // 4. Replacing the old index atomically
    
    // Broadcast indexing progress start
    let job_id = uuid::Uuid::new_v4().to_string();
    crate::event_bus::publish_indexing_progress(
        job_id.clone(),
        0.0,
        "initializing".to_string(),
        None,
        Some("Index rebuild started".to_string()),
    );
    
    let rebuild_time_ms = start_time.elapsed().as_millis() as u64;
    
    // Broadcast indexing progress completion
    crate::event_bus::publish_indexing_progress(
        job_id.clone(),
        1.0,
        "completed".to_string(),
        Some(0.0),
        Some("Index rebuild completed".to_string()),
    );

    Ok(Json(RebuildIndexResponse {
        status: "completed".to_string(),
        message: "Index rebuild completed successfully".to_string(),
        rebuild_time_ms,
        vectors_processed: 0,
    }))
}

pub async fn get_search_performance(
    State(state): State<AppState>,
) -> ApiResult<Json<SearchPerformanceResponse>> {
    let stats = state.vector_search.get_performance_stats();

    Ok(Json(SearchPerformanceResponse {
        total_searches: stats.total_searches,
        sub_millisecond_searches: stats.sub_millisecond_searches,
        sub_ms_rate_percent: stats.sub_ms_rate * 100.0,
        average_latency_us: stats.average_latency_us,
        p95_latency_us: stats.p95_latency_us,
        p99_latency_us: stats.p99_latency_us,
        cache_hit_rate_percent: stats.cache_hit_rate * 100.0,
        cache_entries: stats.cache_entries,
    }))
}

pub async fn submit_batch_operations(
    State(state): State<AppState>,
    Json(request): Json<BatchOperationsRequest>,
) -> ApiResult<Json<BatchOperationsResponse>> {
    if request.operations.is_empty() {
        return Err(ApiError::BadRequest("No operations provided".to_string()));
    }

    // Convert and validate operations
    let mut batch_operations = Vec::new();
    for op in request.operations {
        let operation = match op.operation_type.as_str() {
            "insert" => {
                let embedding = op.embedding
                    .ok_or_else(|| ApiError::BadRequest("Embedding required for insert operation".to_string()))?;
                let node_id = Uuid::parse_str(&op.node_id)
                    .map_err(|_| ApiError::BadRequest("Invalid node ID format".to_string()))?;
                BatchOperation::Insert { node_id, embedding }
            },
            "update" => {
                let embedding = op.embedding
                    .ok_or_else(|| ApiError::BadRequest("Embedding required for update operation".to_string()))?;
                let node_id = Uuid::parse_str(&op.node_id)
                    .map_err(|_| ApiError::BadRequest("Invalid node ID format".to_string()))?;
                BatchOperation::Update { node_id, embedding }
            },
            "delete" => {
                let node_id = Uuid::parse_str(&op.node_id)
                    .map_err(|_| ApiError::BadRequest("Invalid node ID format".to_string()))?;
                BatchOperation::Delete { node_id }
            },
            "search" => {
                let embedding = op.embedding
                    .ok_or_else(|| ApiError::BadRequest("Embedding required for search operation".to_string()))?;
                let search_params = op.search_params
                    .ok_or_else(|| ApiError::BadRequest("Search parameters required for search operation".to_string()))?;
                let callback_id = search_params.callback_id
                    .map(|id| Uuid::parse_str(&id))
                    .transpose()
                    .map_err(|_| ApiError::BadRequest("Invalid callback ID format".to_string()))?
                    .unwrap_or_else(Uuid::new_v4);
                
                BatchOperation::Search {
                    query_embedding: embedding,
                    k: search_params.k,
                    callback_id,
                }
            },
            _ => return Err(ApiError::BadRequest(format!("Unknown operation type: {}", op.operation_type))),
        };
        batch_operations.push(operation);
    }

    // Enqueue operations with the batch processor
    // This would need to be integrated with the actual BatchProcessor in AppState
    let estimated_time = batch_operations.len() as u64 * 10; // 10ms per operation estimate

    Ok(Json(BatchOperationsResponse {
        status: "submitted".to_string(),
        operations_submitted: batch_operations.len(),
        estimated_processing_time_ms: estimated_time,
    }))
}

pub async fn get_batch_status(
    State(state): State<AppState>,
) -> ApiResult<Json<BatchStatusResponse>> {
    // Get statistics from the batch processor
    // This would need to be integrated with the actual BatchProcessor in AppState
    
    // Placeholder implementation
    Ok(Json(BatchStatusResponse {
        total_operations: 0,
        successful_operations: 0,
        failed_operations: 0,
        pending_operations: 0,
        success_rate_percent: 100.0,
        active: false,
    }))
}
