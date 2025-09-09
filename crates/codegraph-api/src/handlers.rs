use crate::{ApiError, ApiResult, AppState};
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use codegraph_core::{CodeParser, GraphStore, NodeId};
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
        version: env!("CARGO_PKG_VERSION").to_string(),
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

    let mut graph = state.graph.write().await;
    for node in nodes {
        graph.add_node(node).await.map_err(ApiError::CodeGraph)?;
    }

    Ok(Json(ParseResponse {
        nodes_created: nodes_count,
        message: format!("Successfully parsed {} nodes from {}", nodes_count, request.file_path),
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