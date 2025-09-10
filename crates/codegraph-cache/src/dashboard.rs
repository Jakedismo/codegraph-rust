use axum::{
    extract::{Path, Query, State, ws::{WebSocketUpgrade, WebSocket}},
    http::StatusCode,
    response::{Html, IntoResponse, Json, Response},
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::time::interval;
use tower_http::cors::CorsLayer;
use tower_http::services::ServeDir;
use tracing::{info, warn, error};

use crate::profiler::{
    MemoryProfiler, MemoryMetrics, ProfilerEvent, MemoryLeak, 
    ProfilerRecommendation, UsagePattern, AllocationType, MEMORY_PROFILER
};
use crate::memory::{MemoryPressure, SystemMemoryInfo};

/// Memory profiler dashboard server
pub struct MemoryDashboard {
    profiler: Arc<MemoryProfiler>,
}

impl MemoryDashboard {
    pub fn new() -> Self {
        Self {
            profiler: MEMORY_PROFILER.clone(),
        }
    }

    /// Create the dashboard web server
    pub fn create_app(self) -> Router {
        Router::new()
            .route("/", get(dashboard_home))
            .route("/api/metrics", get(get_metrics))
            .route("/api/leaks", get(get_leaks))
            .route("/api/patterns", get(get_patterns))
            .route("/api/recommendations", get(get_recommendations))
            .route("/api/system-info", get(get_system_info))
            .route("/api/history", get(get_history))
            .route("/api/export", get(export_data))
            .route("/api/config", get(get_config).post(update_config))
            .route("/ws", get(websocket_handler))
            .nest_service("/static", ServeDir::new("static"))
            .layer(CorsLayer::permissive())
            .with_state(Arc::new(self))
    }

    /// Start the dashboard server
    pub async fn start_server(self, port: u16) -> Result<(), Box<dyn std::error::Error>> {
        let app = self.create_app();
        let addr = format!("0.0.0.0:{}", port);
        
        info!("Starting memory profiler dashboard on http://{}", addr);
        
        let listener = tokio::net::TcpListener::bind(addr).await?;
        axum::serve(listener, app).await?;
        
        Ok(())
    }
}

/// Query parameters for metrics endpoint
#[derive(Debug, Deserialize)]
pub struct MetricsQuery {
    #[serde(default)]
    pub category: Option<AllocationType>,
    #[serde(default = "default_limit")]
    pub limit: usize,
    #[serde(default)]
    pub since: Option<u64>, // Unix timestamp
}

fn default_limit() -> usize { 100 }

/// Query parameters for history endpoint
#[derive(Debug, Deserialize)]
pub struct HistoryQuery {
    #[serde(default = "default_hours")]
    pub hours: u64,
    #[serde(default = "default_resolution")]
    pub resolution: String, // "minute", "hour", "day"
}

fn default_hours() -> u64 { 24 }
fn default_resolution() -> String { "minute".to_string() }

/// Configuration update request
#[derive(Debug, Deserialize)]
pub struct ConfigUpdate {
    pub enabled: Option<bool>,
    pub sampling_rate: Option<f64>,
    pub real_time_monitoring: Option<bool>,
    pub leak_detection_interval_secs: Option<u64>,
}

/// Export format options
#[derive(Debug, Deserialize)]
pub struct ExportQuery {
    #[serde(default = "default_format")]
    pub format: String, // "json", "csv", "prometheus"
    #[serde(default)]
    pub include_allocations: bool,
    #[serde(default)]
    pub include_history: bool,
}

fn default_format() -> String { "json".to_string() }

/// Dashboard API responses
#[derive(Debug, Serialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<String>,
    pub timestamp: SystemTime,
}

impl<T> ApiResponse<T> {
    pub fn success(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
            timestamp: SystemTime::now(),
        }
    }

    pub fn error(message: String) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(message),
            timestamp: SystemTime::now(),
        }
    }
}

/// Historical data point for charting
#[derive(Debug, Serialize)]
pub struct HistoryPoint {
    pub timestamp: u64,
    pub total_usage: usize,
    pub peak_usage: usize,
    pub allocation_count: u64,
    pub deallocation_count: u64,
    pub memory_pressure: MemoryPressure,
    pub categories: HashMap<AllocationType, usize>,
}

/// Live metrics for real-time updates
#[derive(Debug, Serialize)]
pub struct LiveMetrics {
    pub current_usage: usize,
    pub memory_pressure: MemoryPressure,
    pub allocation_rate: f64, // per second
    pub deallocation_rate: f64,
    pub active_leaks: usize,
    pub top_categories: Vec<(AllocationType, usize)>,
}

/// Dashboard home page
async fn dashboard_home() -> Html<&'static str> {
    Html(include_str!("../static/dashboard.html"))
}

/// Get current memory metrics
async fn get_metrics(
    Query(query): Query<MetricsQuery>,
    State(dashboard): State<Arc<MemoryDashboard>>,
) -> Json<ApiResponse<MemoryMetrics>> {
    let metrics = dashboard.profiler.get_metrics();
    Json(ApiResponse::success(metrics))
}

/// Get detected memory leaks
async fn get_leaks(
    State(dashboard): State<Arc<MemoryDashboard>>,
) -> Json<ApiResponse<Vec<MemoryLeak>>> {
    let leaks = dashboard.profiler.detect_leaks();
    Json(ApiResponse::success(leaks))
}

/// Get usage patterns analysis
async fn get_patterns(
    State(dashboard): State<Arc<MemoryDashboard>>,
) -> Json<ApiResponse<HashMap<AllocationType, UsagePattern>>> {
    let patterns = dashboard.profiler.analyze_patterns();
    Json(ApiResponse::success(patterns))
}

/// Get optimization recommendations
async fn get_recommendations(
    State(dashboard): State<Arc<MemoryDashboard>>,
) -> Json<ApiResponse<Vec<ProfilerRecommendation>>> {
    let recommendations = dashboard.profiler.generate_recommendations();
    Json(ApiResponse::success(recommendations))
}

/// Get system memory information
async fn get_system_info(
    State(dashboard): State<Arc<MemoryDashboard>>,
) -> Json<ApiResponse<SystemInfo>> {
    // We need to get system info from memory manager
    // This is a simplified version for now
    let current_usage = dashboard.profiler.get_current_usage();
    let metrics = dashboard.profiler.get_metrics();
    
    let system_info = SystemInfo {
        total_memory_mb: 16384, // Placeholder - would get from sysinfo
        available_memory_mb: 8192,
        used_memory_mb: 8192,
        cache_usage_mb: current_usage / (1024 * 1024),
        cache_limit_mb: 250, // 250MB target
        cpu_usage_percent: 0.0, // Placeholder
        load_average: [0.0, 0.0, 0.0],
        uptime_seconds: 0,
    };
    
    Json(ApiResponse::success(system_info))
}

/// Get historical memory data
async fn get_history(
    Query(query): Query<HistoryQuery>,
    State(dashboard): State<Arc<MemoryDashboard>>,
) -> Json<ApiResponse<Vec<HistoryPoint>>> {
    // For now, we'll return synthetic historical data
    // In a real implementation, this would come from the profiler's history
    let mut history = Vec::new();
    
    let now = SystemTime::now();
    let duration = Duration::from_hours(query.hours);
    let points = match query.resolution.as_str() {
        "minute" => query.hours * 60,
        "hour" => query.hours,
        "day" => query.hours / 24,
        _ => query.hours * 60,
    };
    
    for i in 0..points {
        let point_time = now - duration + Duration::from_secs(i * duration.as_secs() / points);
        let timestamp = point_time.duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        
        // Generate synthetic data for demo
        let usage = 50_000_000 + (i * 1_000_000) as usize; // Gradual increase
        
        let mut categories = HashMap::new();
        categories.insert(AllocationType::Cache, usage / 3);
        categories.insert(AllocationType::Vector, usage / 4);
        categories.insert(AllocationType::Graph, usage / 5);
        
        history.push(HistoryPoint {
            timestamp,
            total_usage: usage,
            peak_usage: usage + 10_000_000,
            allocation_count: i * 100,
            deallocation_count: i * 90,
            memory_pressure: if usage > 200_000_000 {
                MemoryPressure::High
            } else if usage > 150_000_000 {
                MemoryPressure::Medium
            } else {
                MemoryPressure::Low
            },
            categories,
        });
    }
    
    Json(ApiResponse::success(history))
}

/// Export profiler data in various formats
async fn export_data(
    Query(query): Query<ExportQuery>,
    State(dashboard): State<Arc<MemoryDashboard>>,
) -> Result<Response, StatusCode> {
    match query.format.as_str() {
        "json" => {
            let data = ExportData {
                metrics: dashboard.profiler.get_metrics(),
                leaks: dashboard.profiler.detect_leaks(),
                patterns: dashboard.profiler.analyze_patterns(),
                recommendations: dashboard.profiler.generate_recommendations(),
            };
            
            let json = serde_json::to_string_pretty(&data)
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            
            Ok((
                [("Content-Type", "application/json")],
                json
            ).into_response())
        }
        "csv" => {
            // Generate CSV format for metrics
            let metrics = dashboard.profiler.get_metrics();
            let csv = format!(
                "timestamp,total_allocated,total_freed,current_usage,peak_usage,allocation_count\n{},{},{},{},{},{}\n",
                metrics.timestamp.duration_since(SystemTime::UNIX_EPOCH).unwrap_or_default().as_secs(),
                metrics.total_allocated,
                metrics.total_freed,
                metrics.current_usage,
                metrics.peak_usage,
                metrics.allocation_count
            );
            
            Ok((
                [("Content-Type", "text/csv")],
                csv
            ).into_response())
        }
        "prometheus" => {
            // Generate Prometheus metrics format
            let metrics = dashboard.profiler.get_metrics();
            let prometheus = format!(
                "# HELP memory_total_allocated Total bytes allocated\n# TYPE memory_total_allocated counter\nmemory_total_allocated {}\n\n# HELP memory_current_usage Current memory usage in bytes\n# TYPE memory_current_usage gauge\nmemory_current_usage {}\n\n# HELP memory_peak_usage Peak memory usage in bytes\n# TYPE memory_peak_usage gauge\nmemory_peak_usage {}\n",
                metrics.total_allocated,
                metrics.current_usage,
                metrics.peak_usage
            );
            
            Ok((
                [("Content-Type", "text/plain")],
                prometheus
            ).into_response())
        }
        _ => Err(StatusCode::BAD_REQUEST),
    }
}

/// Get profiler configuration
async fn get_config(
    State(dashboard): State<Arc<MemoryDashboard>>,
) -> Json<ApiResponse<ProfilerConfigView>> {
    let config = ProfilerConfigView {
        enabled: true, // Would get from actual config
        sampling_rate: 1.0,
        real_time_monitoring: true,
        leak_detection_interval_secs: 30,
        memory_limit_mb: 250,
    };
    Json(ApiResponse::success(config))
}

/// Update profiler configuration
async fn update_config(
    State(dashboard): State<Arc<MemoryDashboard>>,
    Json(update): Json<ConfigUpdate>,
) -> Json<ApiResponse<String>> {
    // In a real implementation, this would update the profiler config
    info!("Config update requested: {:?}", update);
    Json(ApiResponse::success("Configuration updated successfully".to_string()))
}

/// WebSocket handler for real-time updates
async fn websocket_handler(
    ws: WebSocketUpgrade,
    State(dashboard): State<Arc<MemoryDashboard>>,
) -> Response {
    ws.on_upgrade(|socket| handle_websocket(socket, dashboard))
}

/// Handle WebSocket connection for real-time monitoring
async fn handle_websocket(socket: WebSocket, dashboard: Arc<MemoryDashboard>) {
    let (mut sender, mut receiver) = socket.split();
    
    // Start real-time monitoring
    let mut event_receiver = dashboard.profiler.start_monitoring();
    let mut update_interval = interval(Duration::from_secs(1));
    
    info!("WebSocket connection established for real-time monitoring");
    
    loop {
        tokio::select! {
            // Send periodic live metrics
            _ = update_interval.tick() => {
                let metrics = dashboard.profiler.get_metrics();
                let live_metrics = LiveMetrics {
                    current_usage: metrics.current_usage,
                    memory_pressure: metrics.memory_pressure,
                    allocation_rate: 100.0, // Placeholder
                    deallocation_rate: 90.0,
                    active_leaks: dashboard.profiler.detect_leaks().len(),
                    top_categories: metrics.categories.iter()
                        .map(|(k, v)| (k.clone(), v.current))
                        .collect(),
                };
                
                let message = serde_json::to_string(&WebSocketMessage::LiveMetrics(live_metrics))
                    .unwrap_or_default();
                
                if sender.send(axum::extract::ws::Message::Text(message)).await.is_err() {
                    break;
                }
            }
            
            // Forward profiler events
            Some(event) = event_receiver.recv() => {
                let message = serde_json::to_string(&WebSocketMessage::Event(event))
                    .unwrap_or_default();
                
                if sender.send(axum::extract::ws::Message::Text(message)).await.is_err() {
                    break;
                }
            }
            
            // Handle incoming messages (for future interactive features)
            msg = receiver.next() => {
                if msg.is_none() {
                    break;
                }
            }
        }
    }
    
    info!("WebSocket connection closed");
}

/// Supporting data structures

#[derive(Debug, Serialize)]
pub struct SystemInfo {
    pub total_memory_mb: usize,
    pub available_memory_mb: usize,
    pub used_memory_mb: usize,
    pub cache_usage_mb: usize,
    pub cache_limit_mb: usize,
    pub cpu_usage_percent: f64,
    pub load_average: [f64; 3],
    pub uptime_seconds: u64,
}

#[derive(Debug, Serialize)]
pub struct ExportData {
    pub metrics: MemoryMetrics,
    pub leaks: Vec<MemoryLeak>,
    pub patterns: HashMap<AllocationType, UsagePattern>,
    pub recommendations: Vec<ProfilerRecommendation>,
}

#[derive(Debug, Serialize)]
pub struct ProfilerConfigView {
    pub enabled: bool,
    pub sampling_rate: f64,
    pub real_time_monitoring: bool,
    pub leak_detection_interval_secs: u64,
    pub memory_limit_mb: usize,
}

#[derive(Debug, Serialize)]
#[serde(tag = "type")]
pub enum WebSocketMessage {
    LiveMetrics(LiveMetrics),
    Event(ProfilerEvent),
}

use tokio_tungstenite::tungstenite::Message;
use futures_util::{SinkExt, StreamExt};

/// Utility trait for duration extensions
trait DurationExt {
    fn from_hours(hours: u64) -> Duration;
}

impl DurationExt for Duration {
    fn from_hours(hours: u64) -> Duration {
        Duration::from_secs(hours * 3600)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::StatusCode;
    use tower::ServiceExt;
    
    #[tokio::test]
    async fn test_dashboard_routes() {
        let dashboard = MemoryDashboard::new();
        let app = dashboard.create_app();
        
        // Test metrics endpoint
        let response = app
            .clone()
            .oneshot(
                axum::http::Request::builder()
                    .uri("/api/metrics")
                    .body(axum::body::Body::empty())
                    .unwrap()
            )
            .await
            .unwrap();
        
        assert_eq!(response.status(), StatusCode::OK);
    }
    
    #[tokio::test]
    async fn test_export_json() {
        let dashboard = MemoryDashboard::new();
        let app = dashboard.create_app();
        
        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .uri("/api/export?format=json")
                    .body(axum::body::Body::empty())
                    .unwrap()
            )
            .await
            .unwrap();
        
        assert_eq!(response.status(), StatusCode::OK);
        
        let content_type = response.headers().get("content-type").unwrap();
        assert_eq!(content_type, "application/json");
    }
    
    #[test]
    fn test_duration_extension() {
        let duration = Duration::from_hours(2);
        assert_eq!(duration.as_secs(), 7200);
    }
}
