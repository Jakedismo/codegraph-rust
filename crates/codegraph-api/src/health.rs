use crate::{ApiError, ApiResult, AppState};
use axum::{extract::State, Json};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::time::{timeout, Duration};

#[derive(Serialize, Debug)]
pub struct HealthResponse {
    pub status: String,
    pub version: String,
    pub timestamp: u64,
    pub uptime_seconds: u64,
    pub components: ComponentsHealth,
    pub metrics: SystemMetrics,
}

#[derive(Serialize, Debug)]
pub struct ComponentsHealth {
    pub database: ComponentStatus,
    pub vector_search: ComponentStatus,
    pub parser: ComponentStatus,
    pub memory: ComponentStatus,
    pub storage: ComponentStatus,
}

#[derive(Serialize, Debug)]
pub struct ComponentStatus {
    pub status: String,
    pub last_check: u64,
    pub response_time_ms: Option<u64>,
    pub details: Option<HashMap<String, String>>,
    pub error: Option<String>,
}

#[derive(Serialize, Debug)]
pub struct SystemMetrics {
    pub memory_usage_bytes: u64,
    pub cpu_usage_percent: f64,
    pub goroutines_count: Option<u64>,
    pub active_connections: u64,
    pub requests_per_second: f64,
    pub error_rate_percent: f64,
}

lazy_static::lazy_static! {
    static ref START_TIME: SystemTime = SystemTime::now();
}

impl HealthResponse {
    pub fn is_healthy(&self) -> bool {
        [
            &self.components.database.status,
            &self.components.vector_search.status,
            &self.components.parser.status,
            &self.components.memory.status,
            &self.components.storage.status,
        ]
        .iter()
        .all(|status| *status == "healthy")
    }
}

impl ComponentStatus {
    pub fn healthy() -> Self {
        Self {
            status: "healthy".to_string(),
            last_check: current_timestamp(),
            response_time_ms: None,
            details: None,
            error: None,
        }
    }

    pub fn unhealthy(error: String) -> Self {
        Self {
            status: "unhealthy".to_string(),
            last_check: current_timestamp(),
            response_time_ms: None,
            details: None,
            error: Some(error),
        }
    }

    pub fn degraded(error: String, details: HashMap<String, String>) -> Self {
        Self {
            status: "degraded".to_string(),
            last_check: current_timestamp(),
            response_time_ms: None,
            details: Some(details),
            error: Some(error),
        }
    }

    pub fn with_response_time(mut self, response_time_ms: u64) -> Self {
        self.response_time_ms = Some(response_time_ms);
        self
    }

    pub fn with_details(mut self, details: HashMap<String, String>) -> Self {
        self.details = Some(details);
        self
    }
}

pub async fn comprehensive_health_check(
    State(state): State<AppState>,
) -> ApiResult<Json<HealthResponse>> {
    let timestamp = current_timestamp();
    let uptime = START_TIME.elapsed().unwrap_or_default().as_secs();

    // Perform health checks for all components
    let components = ComponentsHealth {
        database: check_database_health(&state).await,
        vector_search: check_vector_search_health(&state).await,
        parser: check_parser_health(&state).await,
        memory: check_memory_health().await,
        storage: check_storage_health(&state).await,
    };

    // Collect system metrics
    let metrics = collect_system_metrics(&state).await;

    let health_response = HealthResponse {
        status: if components_healthy(&components) {
            "healthy".to_string()
        } else {
            "unhealthy".to_string()
        },
        version: option_env!("CARGO_PKG_VERSION")
            .unwrap_or("0.1.0")
            .to_string(),
        timestamp,
        uptime_seconds: uptime,
        components,
        metrics,
    };

    Ok(Json(health_response))
}

async fn check_database_health(state: &AppState) -> ComponentStatus {
    let start = SystemTime::now();

    // Try to read from the graph store with timeout
    let health_check = timeout(Duration::from_millis(1000), async {
        let graph = state.graph.read().await;
        // Try a simple operation to verify database connectivity
        graph.get_stats().await
    })
    .await;

    let response_time = start.elapsed().unwrap_or_default().as_millis() as u64;

    match health_check {
        Ok(Ok(stats)) => {
            let mut details = HashMap::new();
            details.insert("total_nodes".to_string(), stats.total_nodes.to_string());
            details.insert("total_edges".to_string(), stats.total_edges.to_string());

            ComponentStatus::healthy()
                .with_response_time(response_time)
                .with_details(details)
        }
        Ok(Err(e)) => ComponentStatus::unhealthy(format!("Database error: {}", e))
            .with_response_time(response_time),
        Err(_) => ComponentStatus::unhealthy("Database timeout".to_string())
            .with_response_time(response_time),
    }
}

async fn check_vector_search_health(state: &AppState) -> ComponentStatus {
    let start = SystemTime::now();

    let health_check = timeout(Duration::from_millis(1000), async {
        // Try a simple vector search operation
        state.semantic_search.get_index_stats().await
    })
    .await;

    let response_time = start.elapsed().unwrap_or_default().as_millis() as u64;

    match health_check {
        Ok(Ok(stats)) => {
            let mut details = HashMap::new();
            details.insert(
                "indexed_vectors".to_string(),
                stats.total_vectors.to_string(),
            );
            details.insert("index_type".to_string(), format!("{:?}", stats.index_type));
            details.insert("dimension".to_string(), stats.dimension.to_string());

            ComponentStatus::healthy()
                .with_response_time(response_time)
                .with_details(details)
        }
        Ok(Err(e)) => ComponentStatus::unhealthy(format!("Vector search error: {}", e))
            .with_response_time(response_time),
        Err(_) => ComponentStatus::unhealthy("Vector search timeout".to_string())
            .with_response_time(response_time),
    }
}

async fn check_parser_health(state: &AppState) -> ComponentStatus {
    let start = SystemTime::now();

    // Try to parse a simple code snippet to verify parser health
    let health_check = timeout(Duration::from_millis(500), async {
        state.parser.parse_snippet("fn test() {}", "rust").await
    })
    .await;

    let response_time = start.elapsed().unwrap_or_default().as_millis() as u64;

    match health_check {
        Ok(Ok(nodes)) => {
            let mut details = HashMap::new();
            details.insert("test_parse_success".to_string(), "true".to_string());
            details.insert("parsed_nodes".to_string(), nodes.len().to_string());

            ComponentStatus::healthy()
                .with_response_time(response_time)
                .with_details(details)
        }
        Ok(Err(e)) => ComponentStatus::unhealthy(format!("Parser error: {}", e))
            .with_response_time(response_time),
        Err(_) => ComponentStatus::unhealthy("Parser timeout".to_string())
            .with_response_time(response_time),
    }
}

async fn check_memory_health() -> ComponentStatus {
    #[cfg(feature = "leak-detect")]
    {
        let tracker = memscope_rs::get_global_tracker();
        match tracker.get_stats() {
            Ok(stats) => {
                let mut details = HashMap::new();
                details.insert(
                    "active_allocations".to_string(),
                    stats.active_allocations.to_string(),
                );
                details.insert(
                    "active_memory_mb".to_string(),
                    (stats.active_memory / 1024 / 1024).to_string(),
                );
                details.insert(
                    "leaked_allocations".to_string(),
                    stats.leaked_allocations.to_string(),
                );
                details.insert(
                    "leaked_memory_mb".to_string(),
                    (stats.leaked_memory / 1024 / 1024).to_string(),
                );

                // Consider memory degraded if we have significant leaks
                if stats.leaked_memory > 10 * 1024 * 1024 {
                    // 10MB
                    ComponentStatus::degraded("Memory leaks detected".to_string(), details)
                } else {
                    ComponentStatus::healthy().with_details(details)
                }
            }
            Err(e) => ComponentStatus::unhealthy(format!("Memory tracker error: {}", e)),
        }
    }

    #[cfg(not(feature = "leak-detect"))]
    {
        // Get basic memory info from system
        use sysinfo::{System, SystemExt};
        let mut sys = System::new_all();
        sys.refresh_memory();

        let mut details = HashMap::new();
        details.insert(
            "available_memory_gb".to_string(),
            (sys.available_memory() / 1024 / 1024 / 1024).to_string(),
        );
        details.insert(
            "used_memory_gb".to_string(),
            (sys.used_memory() / 1024 / 1024 / 1024).to_string(),
        );
        details.insert(
            "total_memory_gb".to_string(),
            (sys.total_memory() / 1024 / 1024 / 1024).to_string(),
        );

        // Consider memory degraded if usage is very high
        let memory_usage_percent = (sys.used_memory() as f64 / sys.total_memory() as f64) * 100.0;
        if memory_usage_percent > 90.0 {
            ComponentStatus::degraded(
                format!("High memory usage: {:.1}%", memory_usage_percent),
                details,
            )
        } else {
            ComponentStatus::healthy().with_details(details)
        }
    }
}

async fn check_storage_health(state: &AppState) -> ComponentStatus {
    use std::fs;

    // Check if we can write to temp directory
    let temp_file = "/tmp/codegraph_health_check";
    match fs::write(temp_file, "health_check") {
        Ok(()) => {
            let _ = fs::remove_file(temp_file);

            let mut details = HashMap::new();
            details.insert("write_test".to_string(), "passed".to_string());

            // Check available disk space
            if let Ok(metadata) = fs::metadata("/tmp") {
                details.insert("temp_dir_accessible".to_string(), "true".to_string());
            }

            ComponentStatus::healthy().with_details(details)
        }
        Err(e) => ComponentStatus::unhealthy(format!("Storage write test failed: {}", e)),
    }
}

async fn collect_system_metrics(state: &AppState) -> SystemMetrics {
    use sysinfo::{ProcessExt, System, SystemExt};

    let mut sys = System::new_all();
    sys.refresh_all();

    // Get current process info
    let pid = sysinfo::get_current_pid().unwrap_or(sysinfo::Pid::from(0));
    let process = sys.process(pid);

    let memory_usage = process.map(|p| p.memory() * 1024).unwrap_or(0); // Convert KB to bytes
    let cpu_usage = process.map(|p| p.cpu_usage() as f64).unwrap_or(0.0);

    // Get metrics from Prometheus registry
    let active_connections = crate::metrics::MEM_ACTIVE_ALLOCATIONS.get() as u64;

    SystemMetrics {
        memory_usage_bytes: memory_usage,
        cpu_usage_percent: cpu_usage,
        goroutines_count: None, // Not applicable for Rust
        active_connections,
        requests_per_second: calculate_rps(), // Would need actual implementation
        error_rate_percent: calculate_error_rate(), // Would need actual implementation
    }
}

fn components_healthy(components: &ComponentsHealth) -> bool {
    [
        &components.database.status,
        &components.vector_search.status,
        &components.parser.status,
        &components.memory.status,
        &components.storage.status,
    ]
    .iter()
    .all(|status| *status == "healthy")
}

fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn calculate_rps() -> f64 {
    // This would need to be implemented with actual request tracking
    // For now, return a placeholder
    0.0
}

fn calculate_error_rate() -> f64 {
    // This would need to be implemented with actual error tracking
    // For now, return a placeholder
    0.0
}

// Readiness probe - checks if service is ready to receive traffic
pub async fn readiness_check(State(state): State<AppState>) -> ApiResult<Json<HealthResponse>> {
    let health_response = comprehensive_health_check(State(state)).await?;

    // Service is ready if core components are healthy
    let core_components_healthy = [
        &health_response.components.database.status,
        &health_response.components.vector_search.status,
    ]
    .iter()
    .all(|status| *status == "healthy");

    if core_components_healthy {
        Ok(health_response)
    } else {
        Err(ApiError::ServiceUnavailable(
            "Service not ready".to_string(),
        ))
    }
}

// Liveness probe - checks if service is alive and should not be restarted
pub async fn liveness_check() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "status": "alive",
        "timestamp": current_timestamp(),
        "uptime_seconds": START_TIME.elapsed().unwrap_or_default().as_secs()
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_component_status_creation() {
        let healthy = ComponentStatus::healthy();
        assert_eq!(healthy.status, "healthy");
        assert!(healthy.error.is_none());

        let unhealthy = ComponentStatus::unhealthy("test error".to_string());
        assert_eq!(unhealthy.status, "unhealthy");
        assert_eq!(unhealthy.error, Some("test error".to_string()));
    }

    #[test]
    fn test_health_response_status() {
        let mut components = ComponentsHealth {
            database: ComponentStatus::healthy(),
            vector_search: ComponentStatus::healthy(),
            parser: ComponentStatus::healthy(),
            memory: ComponentStatus::healthy(),
            storage: ComponentStatus::healthy(),
        };

        let response = HealthResponse {
            status: "healthy".to_string(),
            version: "1.0.0".to_string(),
            timestamp: current_timestamp(),
            uptime_seconds: 100,
            components,
            metrics: SystemMetrics {
                memory_usage_bytes: 1024,
                cpu_usage_percent: 10.0,
                goroutines_count: None,
                active_connections: 5,
                requests_per_second: 100.0,
                error_rate_percent: 0.1,
            },
        };

        assert!(response.is_healthy());
    }
}
