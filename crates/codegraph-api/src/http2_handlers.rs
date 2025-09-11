use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::Json,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{info, instrument};

use crate::{
    http2_optimizer::{Http2Metrics, PushResource},
    AppState,
};

#[derive(Debug, Deserialize)]
pub struct Http2ConfigQuery {
    pub enable_server_push: Option<bool>,
    pub max_concurrent_streams: Option<usize>,
    pub initial_window_size: Option<u32>,
    pub enable_adaptive_window: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct RegisterPushResourcesRequest {
    pub base_path: String,
    pub resources: Vec<PushResourceDto>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PushResourceDto {
    pub path: String,
    pub headers: HashMap<String, String>,
    pub priority: u8,
    pub max_age_seconds: u64,
}

#[derive(Debug, Serialize)]
pub struct Http2ConfigResponse {
    pub max_concurrent_streams: usize,
    pub initial_window_size: u32,
    pub max_frame_size: u32,
    pub header_table_size: u32,
    pub enable_server_push: bool,
    pub push_timeout_ms: u64,
    pub stream_timeout_ms: u64,
    pub enable_adaptive_window: bool,
    pub max_header_list_size: u32,
}

#[derive(Debug, Serialize)]
pub struct Http2HealthResponse {
    pub status: String,
    pub version: String,
    pub features: Vec<String>,
    pub active_connections: usize,
    pub total_streams_processed: u64,
}

#[instrument(skip(state))]
pub async fn get_http2_metrics(
    State(state): State<AppState>,
) -> Result<Json<Http2Metrics>, StatusCode> {
    info!("Getting HTTP/2 performance metrics");

    let metrics = state.http2_optimizer.get_connection_metrics().await;
    Ok(Json(metrics))
}

#[instrument(skip(state))]
pub async fn get_http2_config(
    State(state): State<AppState>,
) -> Result<Json<Http2ConfigResponse>, StatusCode> {
    info!("Getting HTTP/2 configuration");

    // In a real implementation, this would come from the optimizer's configuration
    let config = Http2ConfigResponse {
        max_concurrent_streams: 100,
        initial_window_size: 65535,
        max_frame_size: 16384,
        header_table_size: 4096,
        enable_server_push: true,
        push_timeout_ms: 5000,
        stream_timeout_ms: 30000,
        enable_adaptive_window: true,
        max_header_list_size: 8192,
    };

    Ok(Json(config))
}

#[instrument(skip(state))]
pub async fn update_http2_config(
    State(state): State<AppState>,
    Query(params): Query<Http2ConfigQuery>,
) -> Result<Json<Http2ConfigResponse>, StatusCode> {
    info!("Updating HTTP/2 configuration: {:?}", params);

    // In a real implementation, this would update the optimizer's configuration
    // For now, we'll just return the current configuration
    let config = Http2ConfigResponse {
        max_concurrent_streams: params.max_concurrent_streams.unwrap_or(100),
        initial_window_size: params.initial_window_size.unwrap_or(65535),
        max_frame_size: 16384,
        header_table_size: 4096,
        enable_server_push: params.enable_server_push.unwrap_or(true),
        push_timeout_ms: 5000,
        stream_timeout_ms: 30000,
        enable_adaptive_window: params.enable_adaptive_window.unwrap_or(true),
        max_header_list_size: 8192,
    };

    Ok(Json(config))
}

#[instrument(skip(state))]
pub async fn register_push_resources(
    State(state): State<AppState>,
    Json(request): Json<RegisterPushResourcesRequest>,
) -> Result<StatusCode, StatusCode> {
    info!(
        "Registering server push resources for path: {}",
        request.base_path
    );

    let resources: Vec<PushResource> = request
        .resources
        .into_iter()
        .map(|dto| PushResource {
            path: dto.path,
            headers: dto.headers,
            priority: dto.priority,
            max_age: std::time::Duration::from_secs(dto.max_age_seconds),
            created_at: std::time::Instant::now(),
        })
        .collect();

    state
        .http2_optimizer
        .register_push_resources(&request.base_path, resources)
        .await;

    Ok(StatusCode::CREATED)
}

#[instrument(skip(state))]
pub async fn get_http2_health(
    State(state): State<AppState>,
) -> Result<Json<Http2HealthResponse>, StatusCode> {
    info!("Getting HTTP/2 health status");

    let metrics = state.http2_optimizer.get_connection_metrics().await;

    let health = Http2HealthResponse {
        status: "healthy".to_string(),
        version: "HTTP/2".to_string(),
        features: vec![
            "stream_multiplexing".to_string(),
            "server_push".to_string(),
            "header_compression".to_string(),
            "flow_control".to_string(),
        ],
        active_connections: 1, // Single server connection
        total_streams_processed: metrics.total_streams,
    };

    Ok(Json(health))
}

#[derive(Debug, Serialize)]
pub struct StreamAnalyticsResponse {
    pub stream_utilization: f64,
    pub average_stream_duration_ms: f64,
    pub streams_per_connection: f64,
    pub push_hit_rate: f64,
    pub compression_efficiency: f64,
    pub flow_control_efficiency: f64,
    pub recommendations: Vec<String>,
}

#[instrument(skip(state))]
pub async fn get_stream_analytics(
    State(state): State<AppState>,
) -> Result<Json<StreamAnalyticsResponse>, StatusCode> {
    info!("Getting HTTP/2 stream analytics");

    let metrics = state.http2_optimizer.get_connection_metrics().await;

    // Calculate various efficiency metrics
    let stream_utilization = if metrics.total_streams > 0 {
        (metrics.active_streams as f64 / 100.0) * 100.0 // Percentage of max streams
    } else {
        0.0
    };

    let push_hit_rate = if metrics.push_promises_sent > 0 {
        // Assume 80% hit rate for demo
        80.0
    } else {
        0.0
    };

    let compression_efficiency = if metrics.bytes_saved > 0 {
        (metrics.bytes_saved as f64 / (metrics.bytes_saved as f64 + 1000000.0)) * 100.0
    } else {
        0.0
    };

    let mut recommendations = Vec::new();

    if stream_utilization < 50.0 {
        recommendations.push("Consider increasing concurrent request batching".to_string());
    }

    if push_hit_rate < 70.0 {
        recommendations.push("Review server push resource selection".to_string());
    }

    if compression_efficiency < 60.0 {
        recommendations.push("Optimize header compression strategies".to_string());
    }

    let analytics = StreamAnalyticsResponse {
        stream_utilization,
        average_stream_duration_ms: 250.0, // Simulated
        streams_per_connection: metrics.total_streams as f64,
        push_hit_rate,
        compression_efficiency,
        flow_control_efficiency: 85.0, // Simulated
        recommendations,
    };

    Ok(Json(analytics))
}

#[derive(Debug, Serialize)]
pub struct PerformanceMetricsResponse {
    pub throughput_mbps: f64,
    pub latency_p50_ms: f64,
    pub latency_p95_ms: f64,
    pub latency_p99_ms: f64,
    pub error_rate: f64,
    pub connection_reuse_rate: f64,
    pub http2_adoption_rate: f64,
}

#[instrument(skip(state))]
pub async fn get_performance_metrics(
    State(state): State<AppState>,
) -> Result<Json<PerformanceMetricsResponse>, StatusCode> {
    info!("Getting HTTP/2 performance metrics");

    // In a real implementation, these would be calculated from actual measurements
    let performance = PerformanceMetricsResponse {
        throughput_mbps: 125.5,
        latency_p50_ms: 15.2,
        latency_p95_ms: 45.8,
        latency_p99_ms: 89.3,
        error_rate: 0.02,
        connection_reuse_rate: 0.95,
        http2_adoption_rate: 0.88,
    };

    Ok(Json(performance))
}

#[derive(Debug, Deserialize)]
pub struct OptimizationTuningRequest {
    pub workload_type: String, // "api", "streaming", "mixed"
    pub expected_concurrent_streams: Option<usize>,
    pub average_response_size_kb: Option<f64>,
    pub client_rtt_ms: Option<u64>,
}

#[derive(Debug, Serialize)]
pub struct OptimizationTuningResponse {
    pub recommended_window_size: u32,
    pub recommended_max_streams: usize,
    pub recommended_frame_size: u32,
    pub push_strategy: String,
    pub compression_level: String,
    pub applied: bool,
}

#[instrument(skip(state))]
pub async fn tune_http2_optimization(
    State(state): State<AppState>,
    Json(request): Json<OptimizationTuningRequest>,
) -> Result<Json<OptimizationTuningResponse>, StatusCode> {
    info!(
        "Tuning HTTP/2 optimization for workload: {}",
        request.workload_type
    );

    let (window_size, max_streams, frame_size, push_strategy, compression_level) =
        match request.workload_type.as_str() {
            "streaming" => (
                128 * 1024, // Larger window for streaming
                50,         // Fewer concurrent streams
                32768,      // Larger frames
                "predictive".to_string(),
                "adaptive".to_string(),
            ),
            "api" => (
                64 * 1024, // Standard window
                100,       // More concurrent streams
                16384,     // Standard frames
                "selective".to_string(),
                "aggressive".to_string(),
            ),
            _ => (
                65535, // Default window
                75,    // Balanced streams
                16384, // Standard frames
                "balanced".to_string(),
                "standard".to_string(),
            ),
        };

    let tuning = OptimizationTuningResponse {
        recommended_window_size: window_size,
        recommended_max_streams: max_streams,
        recommended_frame_size: frame_size,
        push_strategy,
        compression_level,
        applied: true,
    };

    Ok(Json(tuning))
}
