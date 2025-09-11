use crate::{ApiError, ApiResult, AppState};
use axum::{
    extract::{Path, Query, State},
    http::{
        header::{CACHE_CONTROL, ETAG},
        HeaderMap, HeaderValue, StatusCode,
    },
    Json,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

#[derive(Serialize, ToSchema)]
pub struct LocationDto {
    pub file_path: String,
    pub line: u32,
    pub column: u32,
    pub end_line: Option<u32>,
    pub end_column: Option<u32>,
}

// -------- Index --------

#[derive(Deserialize, ToSchema)]
pub struct IndexRequest {
    /// Directory path to index (repository/workspace root)
    pub path: String,
    /// Use parallel parsing (default: true)
    pub parallel: Option<bool>,
}

#[derive(Serialize, ToSchema)]
pub struct IndexResponse {
    pub nodes_indexed: usize,
    pub files_parsed: usize,
    pub total_files: usize,
    pub total_lines: usize,
    pub duration_ms: u64,
    pub message: String,
}

/// Index a source tree (repository/workspace directory)
#[utoipa::path(
    post,
    path = "/v1/index",
    tag = "v1",
    request_body = IndexRequest,
    responses(
        (status = 200, description = "Indexing completed", body = IndexResponse),
        (status = 400, description = "Invalid input"),
        (status = 500, description = "Internal error")
    )
)]
pub async fn post_index(
    State(state): State<AppState>,
    Json(req): Json<IndexRequest>,
) -> ApiResult<Json<IndexResponse>> {
    if req.path.trim().is_empty() {
        return Err(ApiError::Validation("path must not be empty".into()));
    }

    // Validate directory exists
    let meta = tokio::fs::metadata(&req.path)
        .await
        .map_err(|_| ApiError::Validation(format!("path does not exist: {}", req.path)))?;
    if !meta.is_dir() {
        return Err(ApiError::Validation(format!(
            "path is not a directory: {}",
            req.path
        )));
    }

    let parallel = req.parallel.unwrap_or(true);
    let (nodes, stats) = if parallel {
        state
            .parser
            .parse_directory_parallel(&req.path)
            .await
            .map_err(ApiError::CodeGraph)?
    } else {
        // Fallback to parallel API if non-parallel is not implemented
        state
            .parser
            .parse_directory_parallel(&req.path)
            .await
            .map_err(ApiError::CodeGraph)?
    };

    let nodes_count = nodes.len();
    let mut graph = state.graph.write().await;
    for node in nodes {
        graph.add_node(node).await.map_err(ApiError::CodeGraph)?;
    }

    let duration_ms = stats.parsing_duration.as_millis() as u64;
    Ok(Json(IndexResponse {
        nodes_indexed: nodes_count,
        files_parsed: stats.parsed_files,
        total_files: stats.total_files,
        total_lines: stats.total_lines,
        duration_ms,
        message: format!(
            "Indexed {} nodes from {} files",
            nodes_count, stats.parsed_files
        ),
    }))
}

// -------- Search --------

#[derive(Deserialize, IntoParams, ToSchema)]
pub struct SearchRequest {
    /// Free-text query string
    pub q: String,
    /// Maximum results to return (1..=100)
    pub limit: Option<usize>,
}

#[derive(Serialize, ToSchema)]
pub struct SearchItem {
    pub node_id: String,
    pub score: f32,
    pub name: String,
    pub node_type: String,
    pub language: String,
    pub file_path: String,
}

#[derive(Serialize, ToSchema)]
pub struct SearchResponse {
    pub results: Vec<SearchItem>,
    pub total: usize,
}

/// Search code nodes using semantic search
#[utoipa::path(
    get,
    path = "/v1/search",
    tag = "v1",
    params(SearchRequest),
    responses(
        (status = 200, description = "Search results", body = SearchResponse),
        (status = 400, description = "Invalid input"),
        (status = 500, description = "Internal error")
    )
)]
pub async fn get_search(
    State(state): State<AppState>,
    Query(params): Query<SearchRequest>,
) -> ApiResult<(HeaderMap, Json<SearchResponse>)> {
    let q = params.q.trim();
    if q.is_empty() {
        return Err(ApiError::Validation("q must not be empty".into()));
    }
    let mut limit = params.limit.unwrap_or(10);
    if limit == 0 || limit > 100 {
        limit = 10;
    }

    let results = state
        .semantic_search
        .search_by_text(q, limit)
        .await
        .map_err(ApiError::CodeGraph)?;

    let mut items = Vec::with_capacity(results.len());
    let graph = state.graph.read().await;
    for r in results {
        if let Ok(Some(node)) = graph.get_node(r.node_id).await {
            items.push(SearchItem {
                node_id: r.node_id.to_string(),
                score: r.score,
                name: node.name.to_string(),
                node_type: format!("{:?}", node.node_type),
                language: format!("{:?}", node.language),
                file_path: node.location.file_path,
            });
        }
    }

    let body = SearchResponse {
        total: items.len(),
        results: items,
    };
    Ok((cache_headers(&body), Json(body)))
}

// -------- Get Node --------

#[derive(Serialize, ToSchema)]
pub struct NodeResponse {
    pub id: String,
    pub name: String,
    pub node_type: String,
    pub language: String,
    pub location: LocationDto,
    pub content: Option<String>,
    pub has_embedding: bool,
}

/// Get a code node by ID
#[utoipa::path(
    get,
    path = "/v1/node/{id}",
    tag = "v1",
    params(
        ("id" = String, Path, description = "Node UUID")
    ),
    responses(
        (status = 200, description = "Node details", body = NodeResponse),
        (status = 404, description = "Node not found"),
        (status = 400, description = "Invalid ID format")
    )
)]
pub async fn get_node(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> ApiResult<(HeaderMap, Json<NodeResponse>)> {
    let uuid = Uuid::parse_str(&id)
        .map_err(|_| ApiError::BadRequest("Invalid node ID format".to_string()))?;

    let graph = state.graph.read().await;
    let node = graph
        .get_node(uuid)
        .await
        .map_err(ApiError::CodeGraph)?
        .ok_or_else(|| ApiError::NotFound(format!("Node {} not found", id)))?;

    let body = NodeResponse {
        id: node.id.to_string(),
        name: node.name.to_string(),
        node_type: format!("{:?}", node.node_type),
        language: format!("{:?}", node.language),
        location: LocationDto {
            file_path: node.location.file_path,
            line: node.location.line,
            column: node.location.column,
            end_line: node.location.end_line,
            end_column: node.location.end_column,
        },
        content: node.content.map(|s| s.to_string()),
        has_embedding: node.embedding.is_some(),
    };

    Ok((cache_headers(&body), Json(body)))
}

// -------- Neighbors --------

#[derive(Deserialize, IntoParams, ToSchema)]
pub struct NeighborsRequest {
    /// Center node UUID
    pub id: String,
    /// Max neighbors to return (1..=500)
    pub limit: Option<usize>,
}

#[derive(Serialize, ToSchema)]
pub struct NeighborItem {
    pub id: String,
    pub name: String,
    pub node_type: String,
    pub language: String,
}

#[derive(Serialize, ToSchema)]
pub struct NeighborsResponse {
    pub center: String,
    pub total: usize,
    pub neighbors: Vec<NeighborItem>,
}

/// Get outgoing neighbors for a node
#[utoipa::path(
    get,
    path = "/v1/graph/neighbors",
    tag = "v1",
    params(NeighborsRequest),
    responses(
        (status = 200, description = "Neighbor list", body = NeighborsResponse),
        (status = 400, description = "Invalid input"),
        (status = 404, description = "Node not found")
    )
)]
pub async fn get_neighbors(
    State(state): State<AppState>,
    Query(params): Query<NeighborsRequest>,
) -> ApiResult<(HeaderMap, Json<NeighborsResponse>)> {
    let uuid = Uuid::parse_str(params.id.trim())
        .map_err(|_| ApiError::BadRequest("Invalid node ID format".to_string()))?;
    let mut limit = params.limit.unwrap_or(50);
    if limit == 0 || limit > 500 {
        limit = 50;
    }

    let graph = state.graph.read().await;
    // Validate node exists
    let exists = graph
        .get_node(uuid)
        .await
        .map_err(ApiError::CodeGraph)?
        .is_some();
    if !exists {
        return Err(ApiError::NotFound(format!("Node {} not found", params.id)));
    }

    let neighbors = graph
        .get_neighbors(uuid)
        .await
        .map_err(ApiError::CodeGraph)?;
    let mut out = Vec::new();
    for nid in neighbors.into_iter().take(limit) {
        if let Ok(Some(n)) = graph.get_node(nid).await {
            out.push(NeighborItem {
                id: n.id.to_string(),
                name: n.name,
                node_type: format!("{:?}", n.node_type),
                language: format!("{:?}", n.language),
            });
        }
    }

    let body = NeighborsResponse {
        center: params.id,
        total: out.len(),
        neighbors: out,
    };
    Ok((cache_headers(&body), Json(body)))
}

fn cache_headers<T: serde::Serialize>(value: &T) -> HeaderMap {
    let bytes = serde_json::to_vec(value).unwrap_or_default();
    let hash = Sha256::digest(&bytes);
    let etag = format!("\"{:x}\"", hash);
    let mut headers = HeaderMap::new();
    headers.insert(
        CACHE_CONTROL,
        HeaderValue::from_static("public, max-age=60"),
    );
    if let Ok(val) = HeaderValue::from_str(&etag) {
        headers.insert(ETAG, val);
    }
    headers
}
