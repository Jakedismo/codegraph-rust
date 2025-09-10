use crate::{ApiError, ApiResult, AppState};
use axum::{
    extract::{Path, Query, State},
    response::IntoResponse,
};
use axum_streams::{StreamBodyAs, StreamBodyAsOptions};
use codegraph_core::{GraphStore, NodeId};
use futures::{stream, Stream, StreamExt, TryStreamExt};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, pin::Pin, time::Duration};
use tokio::time::sleep;
use tracing::{debug, warn};
use uuid::Uuid;

#[derive(Deserialize)]
pub struct StreamQuery {
    pub query: String,
    pub limit: Option<usize>,
    pub batch_size: Option<usize>,
    pub throttle_ms: Option<u64>,
}

#[derive(Serialize, Clone)]
pub struct StreamingSearchResult {
    pub node_id: String,
    pub score: f32,
    pub name: String,
    pub node_type: String,
    pub language: String,
    pub file_path: String,
    pub batch_id: usize,
    pub total_processed: usize,
}

#[derive(Serialize)]
pub struct StreamingMetadata {
    pub total_results: usize,
    pub batch_size: usize,
    pub estimated_batches: usize,
    pub stream_id: String,
}

pub async fn stream_search_results(
    State(state): State<AppState>,
    Query(params): Query<StreamQuery>,
) -> ApiResult<impl IntoResponse> {
    let limit = params.limit.unwrap_or(1000);
    let batch_size = params.batch_size.unwrap_or(50);
    let throttle_duration = Duration::from_millis(params.throttle_ms.unwrap_or(10));
    
    debug!(
        "Starting streaming search: query='{}', limit={}, batch_size={}, throttle_ms={}",
        params.query, limit, batch_size, params.throttle_ms.unwrap_or(10)
    );

    let results = state
        .semantic_search
        .search_by_text(&params.query, limit)
        .await
        .map_err(ApiError::CodeGraph)?;

    let graph = state.graph.read().await;
    let stream_id = Uuid::new_v4().to_string();
    
    let search_stream = create_backpressure_stream(
        results,
        graph,
        batch_size,
        throttle_duration,
        stream_id,
    );

    Ok(StreamBodyAsOptions::new()
        .buffering_ready_items(batch_size)
        .json_array(search_stream))
}

fn create_backpressure_stream(
    results: Vec<codegraph_vector::SearchResult>,
    graph: tokio::sync::RwLockReadGuard<'_, Box<dyn GraphStore + Send + Sync>>,
    batch_size: usize,
    throttle_duration: Duration,
    stream_id: String,
) -> Pin<Box<dyn Stream<Item = StreamingSearchResult> + Send + 'static>> {
    let total_results = results.len();
    
    // Convert results to owned data to avoid lifetime issues
    let owned_results: Vec<_> = results.into_iter().collect();
    
    let stream = stream::iter(owned_results.into_iter().enumerate())
        .chunks(batch_size)
        .enumerate()
        .then(move |(batch_idx, batch)| {
            let stream_id = stream_id.clone();
            async move {
                if batch_idx > 0 {
                    sleep(throttle_duration).await;
                }
                
                let mut batch_results = Vec::new();
                for (global_idx, search_result) in batch {
                    // Note: In a real implementation, we would need to handle the graph access differently
                    // since we can't hold the read guard across await points. This is a simplified version.
                    let dummy_result = StreamingSearchResult {
                        node_id: search_result.node_id.to_string(),
                        score: search_result.score,
                        name: format!("Node_{}", global_idx),
                        node_type: "Unknown".to_string(),
                        language: "Unknown".to_string(),
                        file_path: format!("/unknown/path_{}.rs", global_idx),
                        batch_id: batch_idx,
                        total_processed: global_idx + 1,
                    };
                    batch_results.push(dummy_result);
                }
                
                debug!(
                    "Processed batch {} with {} items for stream {}",
                    batch_idx,
                    batch_results.len(),
                    stream_id
                );
                
                stream::iter(batch_results)
            }
        })
        .flatten();

    Box::pin(stream)
}

pub async fn stream_large_dataset(
    State(state): State<AppState>,
    Query(params): Query<HashMap<String, String>>,
) -> ApiResult<impl IntoResponse> {
    let batch_size: usize = params
        .get("batch_size")
        .and_then(|s| s.parse().ok())
        .unwrap_or(100);
    
    let throttle_ms: u64 = params
        .get("throttle_ms")
        .and_then(|s| s.parse().ok())
        .unwrap_or(5);

    debug!("Streaming large dataset with batch_size={}, throttle_ms={}", batch_size, throttle_ms);

    let dataset_stream = create_large_dataset_stream(batch_size, throttle_ms);

    Ok(StreamBodyAsOptions::new()
        .buffering_ready_items(batch_size)
        .json_array(dataset_stream))
}

fn create_large_dataset_stream(
    batch_size: usize,
    throttle_ms: u64,
) -> impl Stream<Item = StreamingSearchResult> + Send + 'static {
    let total_items = 10000; // Simulate large dataset
    let throttle_duration = Duration::from_millis(throttle_ms);
    
    stream::iter(0..total_items)
        .chunks(batch_size)
        .enumerate()
        .then(move |(batch_idx, batch)| async move {
            if batch_idx > 0 {
                sleep(throttle_duration).await;
            }
            
            let batch_results: Vec<_> = batch
                .into_iter()
                .map(|i| StreamingSearchResult {
                    node_id: Uuid::new_v4().to_string(),
                    score: 1.0 - (i as f32 / total_items as f32),
                    name: format!("LargeDataItem_{}", i),
                    node_type: "DataNode".to_string(),
                    language: "Rust".to_string(),
                    file_path: format!("/data/large_item_{}.rs", i),
                    batch_id: batch_idx,
                    total_processed: i + 1,
                })
                .collect();
            
            debug!("Generated batch {} with {} items", batch_idx, batch_results.len());
            
            stream::iter(batch_results)
        })
        .flatten()
}

pub async fn stream_csv_results(
    State(state): State<AppState>,
    Query(params): Query<StreamQuery>,
) -> ApiResult<impl IntoResponse> {
    let limit = params.limit.unwrap_or(1000);
    let batch_size = params.batch_size.unwrap_or(50);
    
    debug!("Streaming CSV results: limit={}, batch_size={}", limit, batch_size);

    let results = state
        .semantic_search
        .search_by_text(&params.query, limit)
        .await
        .map_err(ApiError::CodeGraph)?;

    let csv_stream = create_csv_stream(results, batch_size);

    Ok(StreamBodyAsOptions::new()
        .buffering_ready_items(batch_size)
        .csv(csv_stream))
}

fn create_csv_stream(
    results: Vec<codegraph_vector::SearchResult>,
    batch_size: usize,
) -> impl Stream<Item = StreamingSearchResult> + Send + 'static {
    stream::iter(results.into_iter().enumerate())
        .chunks(batch_size)
        .enumerate()
        .then(move |(batch_idx, batch)| async move {
            if batch_idx > 0 {
                sleep(Duration::from_millis(10)).await;
            }
            
            let batch_results: Vec<_> = batch
                .into_iter()
                .map(|(idx, search_result)| StreamingSearchResult {
                    node_id: search_result.node_id.to_string(),
                    score: search_result.score,
                    name: format!("CsvNode_{}", idx),
                    node_type: "CsvData".to_string(),
                    language: "Data".to_string(),
                    file_path: format!("/csv/data_{}.csv", idx),
                    batch_id: batch_idx,
                    total_processed: idx + 1,
                })
                .collect();
            
            stream::iter(batch_results)
        })
        .flatten()
}

pub async fn get_stream_metadata(
    State(state): State<AppState>,
    Path(stream_id): Path<String>,
) -> ApiResult<axum::Json<StreamingMetadata>> {
    // In a real implementation, this would look up actual stream metadata
    let metadata = StreamingMetadata {
        total_results: 1000,
        batch_size: 50,
        estimated_batches: 20,
        stream_id,
    };
    
    Ok(axum::Json(metadata))
}

#[derive(Serialize)]
pub struct FlowControlStats {
    pub active_streams: usize,
    pub total_bytes_streamed: u64,
    pub average_batch_time_ms: f64,
    pub backpressure_events: u64,
}

pub async fn get_flow_control_stats(
    State(state): State<AppState>,
) -> ApiResult<axum::Json<FlowControlStats>> {
    // In a real implementation, these would be tracked by a flow control manager
    let stats = FlowControlStats {
        active_streams: 3,
        total_bytes_streamed: 1048576,
        average_batch_time_ms: 15.5,
        backpressure_events: 2,
    };
    
    Ok(axum::Json(stats))
}

#[cfg(test)]
mod tests;

// Helper function for creating better backpressure streams
pub fn create_optimized_stream<T>(
    items: Vec<T>,
    batch_size: usize,
    throttle_duration: Duration,
    max_concurrent: usize,
) -> Pin<Box<dyn Stream<Item = T> + Send + 'static>>
where
    T: Send + 'static,
{
    use futures::stream::FuturesUnordered;
    
    let stream = stream::iter(items.into_iter())
        .chunks(batch_size)
        .enumerate()
        .then(move |(batch_idx, batch)| async move {
            if batch_idx > 0 {
                sleep(throttle_duration).await;
            }
            stream::iter(batch)
        })
        .flatten()
        .buffer_unordered(max_concurrent);

    Box::pin(stream)
}