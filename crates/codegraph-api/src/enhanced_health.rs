use crate::{ApiError, ApiResult, AppState};
use crate::parser_ext::TreeSitterParserExt;
use crate::semantic_search_ext::SemanticSearchExt;
use axum::{extract::State, Json};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::time::{timeout, Duration};

#[derive(Serialize, Debug)]
pub struct EnhancedHealthResponse {
    pub status: String,
    pub version: String,
    pub timestamp: u64,
    pub uptime_seconds: u64,
    pub components: ComponentsHealth,
    pub metrics: SystemMetrics,
    pub performance: PerformanceMetrics,
    pub alerts: Vec<AlertInfo>,
}

#[derive(Serialize, Debug)]
pub struct PerformanceMetrics {
    pub avg_response_time_ms: f64,
    pub p95_response_time_ms: f64,
    pub p99_response_time_ms: f64,
    pub throughput_rps: f64,
    pub error_rate_percent: f64,
    pub memory_leak_detected: bool,
    pub active_connections: u64,
    pub queue_depth: u64,
}

#[derive(Serialize, Debug)]
pub struct AlertInfo {
    pub severity: String,
    pub component: String,
    pub message: String,
    pub threshold: Option<f64>,
    pub current_value: Option<f64>,
    pub first_seen: u64,
}

#[derive(Serialize, Debug)]
pub struct ComponentsHealth {
    pub database: ComponentStatus,
    pub vector_search: ComponentStatus,
    pub parser: ComponentStatus,
    pub memory: ComponentStatus,
    pub storage: ComponentStatus,
    pub connection_pool: ComponentStatus,
    pub cache: ComponentStatus,
}

#[derive(Serialize, Debug)]
pub struct ComponentStatus {
    pub status: String,
    pub last_check: u64,
    pub response_time_ms: Option<u64>,
    pub details: Option<HashMap<String, String>>,
    pub error: Option<String>,
    pub health_score: Option<f64>, // 0.0 to 1.0
}

#[derive(Serialize, Debug)]
pub struct SystemMetrics {
    pub memory_usage_bytes: u64,
    pub memory_available_bytes: u64,
    pub memory_usage_percent: f64,
    pub cpu_usage_percent: f64,
    pub disk_usage_percent: f64,
    pub active_connections: u64,
    pub total_requests: u64,
    pub requests_per_second: f64,
    pub error_rate_percent: f64,
    pub goroutines_count: Option<u64>,
}

lazy_static::lazy_static! {
    static ref START_TIME: SystemTime = SystemTime::now();
    static ref HEALTH_ALERTS: std::sync::Arc<tokio::sync::Mutex<Vec<AlertInfo>>> =
        std::sync::Arc::new(tokio::sync::Mutex::new(Vec::new()));
}

/// Enhanced health check that includes performance metrics and alerting
pub async fn enhanced_health_check(
    State(state): State<AppState>,
) -> ApiResult<Json<EnhancedHealthResponse>> {
    let timestamp = current_timestamp();
    let uptime = START_TIME.elapsed().unwrap_or_default().as_secs();

    // Perform comprehensive health checks
    let components = ComponentsHealth {
        database: check_database_health_enhanced(&state).await,
        vector_search: check_vector_search_health_enhanced(&state).await,
        parser: check_parser_health_enhanced(&state).await,
        memory: check_memory_health_enhanced().await,
        storage: check_storage_health_enhanced(&state).await,
        connection_pool: check_connection_pool_health(&state).await,
        cache: check_cache_health(&state).await,
    };

    // Collect enhanced system metrics
    let metrics = collect_enhanced_system_metrics(&state).await;

    // Collect performance metrics
    let performance = collect_performance_metrics(&state).await;

    // Update Prometheus metrics
    update_prometheus_health_metrics(&components, &metrics, &performance).await;

    // Check for alerts
    let alerts = check_and_update_alerts(&components, &metrics, &performance).await;

    let overall_status = determine_overall_status(&components, &performance);

    let health_response = EnhancedHealthResponse {
        status: overall_status,
        version: option_env!("CARGO_PKG_VERSION")
            .unwrap_or("0.1.0")
            .to_string(),
        timestamp,
        uptime_seconds: uptime,
        components,
        metrics,
        performance,
        alerts,
    };

    Ok(Json(health_response))
}

async fn check_database_health_enhanced(state: &AppState) -> ComponentStatus {
    let start = SystemTime::now();

    let health_check = timeout(Duration::from_millis(2000), async {
        let graph = state.graph.read().await;
        let stats = graph.get_stats().await?;

        // Additional checks
        let connection_test = graph.test_connection().await;
        Ok::<_, codegraph_core::CodeGraphError>((stats, connection_test))
    })
    .await;

    let response_time = start.elapsed().unwrap_or_default().as_millis() as u64;

    match health_check {
        Ok(Ok((stats, connection_ok))) => {
            let mut details = HashMap::new();
            details.insert("total_nodes".to_string(), stats.total_nodes.to_string());
            details.insert("total_edges".to_string(), stats.total_edges.to_string());
            details.insert("connection_test".to_string(), connection_ok.unwrap_or(false).to_string());

            // Calculate health score based on response time and stats
            let health_score = calculate_health_score(response_time, Some(&details));

            ComponentStatus {
                status: if health_score > 0.8 {
                    "healthy".to_string()
                } else if health_score > 0.5 {
                    "degraded".to_string()
                } else {
                    "unhealthy".to_string()
                },
                last_check: current_timestamp(),
                response_time_ms: Some(response_time),
                details: Some(details),
                error: None,
                health_score: Some(health_score),
            }
        }
        Ok(Err(e)) => ComponentStatus {
            status: "unhealthy".to_string(),
            last_check: current_timestamp(),
            response_time_ms: Some(response_time),
            details: None,
            error: Some(format!("Database error: {}", e)),
            health_score: Some(0.0),
        },
        Err(_) => ComponentStatus {
            status: "unhealthy".to_string(),
            last_check: current_timestamp(),
            response_time_ms: Some(response_time),
            details: None,
            error: Some("Database timeout".to_string()),
            health_score: Some(0.0),
        },
    }
}

async fn check_vector_search_health_enhanced(state: &AppState) -> ComponentStatus {
    let start = SystemTime::now();

    let health_check = timeout(Duration::from_millis(2000), async {
        let stats = state.semantic_search.get_index_stats().await?;

        // Test with a small search
        let test_result = state.semantic_search.test_search().await;
        Ok::<_, codegraph_core::CodeGraphError>((stats, test_result))
    })
    .await;

    let response_time = start.elapsed().unwrap_or_default().as_millis() as u64;

    match health_check {
        Ok(Ok((stats, test_ok))) => {
            let mut details = HashMap::new();
            details.insert(
                "indexed_vectors".to_string(),
                stats.total_vectors.to_string(),
            );
            details.insert("index_type".to_string(), format!("{:?}", stats.index_type));
            details.insert("dimension".to_string(), stats.dimension.to_string());
            details.insert("search_test".to_string(), test_ok.unwrap_or(false).to_string());

            let health_score = calculate_health_score(response_time, Some(&details));

            ComponentStatus {
                status: if health_score > 0.8 {
                    "healthy".to_string()
                } else if health_score > 0.5 {
                    "degraded".to_string()
                } else {
                    "unhealthy".to_string()
                },
                last_check: current_timestamp(),
                response_time_ms: Some(response_time),
                details: Some(details),
                error: None,
                health_score: Some(health_score),
            }
        }
        Ok(Err(e)) => ComponentStatus {
            status: "unhealthy".to_string(),
            last_check: current_timestamp(),
            response_time_ms: Some(response_time),
            details: None,
            error: Some(format!("Vector search error: {}", e)),
            health_score: Some(0.0),
        },
        Err(_) => ComponentStatus {
            status: "unhealthy".to_string(),
            last_check: current_timestamp(),
            response_time_ms: Some(response_time),
            details: None,
            error: Some("Vector search timeout".to_string()),
            health_score: Some(0.0),
        },
    }
}

async fn check_parser_health_enhanced(state: &AppState) -> ComponentStatus {
    let start = SystemTime::now();

    // Test parsing multiple languages
    let test_snippets = vec![
        ("rust", "fn test() { println!(\"hello\"); }"),
        ("python", "def test(): print(\"hello\")"),
        ("javascript", "function test() { console.log(\"hello\"); }"),
    ];

    let mut all_passed = true;
    let mut details = HashMap::new();

    for (lang, code) in test_snippets {
        let health_check = timeout(Duration::from_millis(1000), async {
            state.parser.parse_snippet(code, lang).await
        })
        .await;

        let passed = health_check.is_ok() && health_check.unwrap().is_ok();
        all_passed &= passed;
        details.insert(format!("{}_parse_test", lang), passed.to_string());
    }

    let response_time = start.elapsed().unwrap_or_default().as_millis() as u64;
    let health_score = if all_passed { 1.0 } else { 0.5 };

    ComponentStatus {
        status: if all_passed {
            "healthy".to_string()
        } else {
            "degraded".to_string()
        },
        last_check: current_timestamp(),
        response_time_ms: Some(response_time),
        details: Some(details),
        error: if all_passed {
            None
        } else {
            Some("Some parser tests failed".to_string())
        },
        health_score: Some(health_score),
    }
}

async fn check_memory_health_enhanced() -> ComponentStatus {
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

                // Enhanced health scoring
                let leak_ratio = if stats.active_memory > 0 {
                    stats.leaked_memory as f64 / stats.active_memory as f64
                } else {
                    0.0
                };

                let health_score = if leak_ratio < 0.01 {
                    1.0
                } else if leak_ratio < 0.05 {
                    0.8
                } else if leak_ratio < 0.1 {
                    0.6
                } else {
                    0.2
                };

                let status = if stats.leaked_memory > 50 * 1024 * 1024 {
                    // 50MB
                    "unhealthy".to_string()
                } else if stats.leaked_memory > 10 * 1024 * 1024 {
                    // 10MB
                    "degraded".to_string()
                } else {
                    "healthy".to_string()
                };

                ComponentStatus {
                    status,
                    last_check: current_timestamp(),
                    response_time_ms: Some(0),
                    details: Some(details),
                    error: if stats.leaked_memory > 10 * 1024 * 1024 {
                        Some(format!(
                            "Memory leaks detected: {}MB",
                            stats.leaked_memory / 1024 / 1024
                        ))
                    } else {
                        None
                    },
                    health_score: Some(health_score),
                }
            }
            Err(e) => ComponentStatus {
                status: "unhealthy".to_string(),
                last_check: current_timestamp(),
                response_time_ms: Some(0),
                details: None,
                error: Some(format!("Memory tracker error: {}", e)),
                health_score: Some(0.0),
            },
        }
    }

    #[cfg(not(feature = "leak-detect"))]
    {
        use sysinfo::System;
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

        let memory_usage_percent = (sys.used_memory() as f64 / sys.total_memory() as f64) * 100.0;
        let health_score = if memory_usage_percent < 80.0 {
            1.0
        } else if memory_usage_percent < 90.0 {
            0.7
        } else if memory_usage_percent < 95.0 {
            0.4
        } else {
            0.1
        };

        ComponentStatus {
            status: if memory_usage_percent > 95.0 {
                "unhealthy".to_string()
            } else if memory_usage_percent > 85.0 {
                "degraded".to_string()
            } else {
                "healthy".to_string()
            },
            last_check: current_timestamp(),
            response_time_ms: Some(0),
            details: Some(details),
            error: if memory_usage_percent > 90.0 {
                Some(format!("High memory usage: {:.1}%", memory_usage_percent))
            } else {
                None
            },
            health_score: Some(health_score),
        }
    }
}

async fn check_storage_health_enhanced(state: &AppState) -> ComponentStatus {
    use std::fs;

    // Multiple storage checks
    let temp_file = "/tmp/codegraph_health_check";
    let write_test = fs::write(temp_file, "health_check").is_ok();
    let read_test = write_test && fs::read_to_string(temp_file).is_ok();
    let cleanup = fs::remove_file(temp_file).is_ok();

    let mut details = HashMap::new();
    details.insert("write_test".to_string(), write_test.to_string());
    details.insert("read_test".to_string(), read_test.to_string());
    details.insert("cleanup_test".to_string(), cleanup.to_string());

    // Check disk space
    let disk_usage = get_disk_usage_percent();
    details.insert("disk_usage_percent".to_string(), disk_usage.to_string());

    let health_score = if write_test && read_test && disk_usage < 90.0 {
        1.0
    } else if write_test && read_test && disk_usage < 95.0 {
        0.7
    } else if write_test && read_test {
        0.5
    } else {
        0.0
    };

    ComponentStatus {
        status: if health_score > 0.8 {
            "healthy".to_string()
        } else if health_score > 0.4 {
            "degraded".to_string()
        } else {
            "unhealthy".to_string()
        },
        last_check: current_timestamp(),
        response_time_ms: Some(0),
        details: Some(details),
        error: if health_score < 0.5 {
            Some("Storage operations failing".to_string())
        } else {
            None
        },
        health_score: Some(health_score),
    }
}

async fn check_connection_pool_health(state: &AppState) -> ComponentStatus {
    let mut details = HashMap::new();

    // Get connection pool metrics from Prometheus
    let active = crate::metrics::CONNECTION_POOL_ACTIVE.get() as u64;
    let idle = crate::metrics::CONNECTION_POOL_IDLE.get() as u64;
    let total = active + idle;

    details.insert("active_connections".to_string(), active.to_string());
    details.insert("idle_connections".to_string(), idle.to_string());
    details.insert("total_connections".to_string(), total.to_string());

    let utilization = if total > 0 {
        active as f64 / total as f64
    } else {
        0.0
    };
    details.insert(
        "utilization_percent".to_string(),
        format!("{:.1}", utilization * 100.0),
    );

    let health_score = if utilization < 0.8 {
        1.0
    } else if utilization < 0.9 {
        0.7
    } else if utilization < 0.95 {
        0.4
    } else {
        0.1
    };

    ComponentStatus {
        status: if health_score > 0.8 {
            "healthy".to_string()
        } else if health_score > 0.4 {
            "degraded".to_string()
        } else {
            "unhealthy".to_string()
        },
        last_check: current_timestamp(),
        response_time_ms: Some(0),
        details: Some(details),
        error: if health_score < 0.5 {
            Some(format!(
                "High connection pool utilization: {:.1}%",
                utilization * 100.0
            ))
        } else {
            None
        },
        health_score: Some(health_score),
    }
}

async fn check_cache_health(state: &AppState) -> ComponentStatus {
    // Placeholder for cache health check
    // In a real implementation, you'd check cache hit rates, memory usage, etc.
    let mut details = HashMap::new();
    details.insert("cache_type".to_string(), "in-memory".to_string());

    ComponentStatus {
        status: "healthy".to_string(),
        last_check: current_timestamp(),
        response_time_ms: Some(0),
        details: Some(details),
        error: None,
        health_score: Some(1.0),
    }
}

async fn collect_enhanced_system_metrics(state: &AppState) -> SystemMetrics {
    use sysinfo::System;

    let mut sys = System::new_all();
    sys.refresh_all();

    // Per-process metrics optional in this simplified build
    let memory_usage = 0u64;
    let cpu_usage = 0.0f64;

    let total_memory = sys.total_memory();
    let available_memory = sys.available_memory();
    let memory_usage_percent = if total_memory > 0 {
        ((total_memory - available_memory) as f64 / total_memory as f64) * 100.0
    } else {
        0.0
    };

    let active_connections = crate::metrics::CONNECTION_POOL_ACTIVE.get() as u64;
    let total_requests = get_total_requests();
    let rps = calculate_enhanced_rps();
    let error_rate = calculate_enhanced_error_rate();
    let disk_usage = get_disk_usage_percent();

    SystemMetrics {
        memory_usage_bytes: memory_usage,
        memory_available_bytes: available_memory,
        memory_usage_percent,
        cpu_usage_percent: cpu_usage,
        disk_usage_percent: disk_usage,
        active_connections,
        total_requests,
        requests_per_second: rps,
        error_rate_percent: error_rate,
        goroutines_count: None,
    }
}

async fn collect_performance_metrics(state: &AppState) -> PerformanceMetrics {
    // Get metrics from Prometheus
    let avg_response_time = get_avg_response_time();
    let p95_response_time = get_p95_response_time();
    let p99_response_time = get_p99_response_time();
    let throughput = calculate_enhanced_rps();
    let error_rate = calculate_enhanced_error_rate();
    let memory_leak_detected = detect_memory_leaks();
    let active_connections = crate::metrics::CONNECTION_POOL_ACTIVE.get() as u64;
    let queue_depth = crate::metrics::HTTP_REQUESTS_IN_FLIGHT.get() as u64;

    PerformanceMetrics {
        avg_response_time_ms: avg_response_time,
        p95_response_time_ms: p95_response_time,
        p99_response_time_ms: p99_response_time,
        throughput_rps: throughput,
        error_rate_percent: error_rate,
        memory_leak_detected,
        active_connections,
        queue_depth,
    }
}

async fn update_prometheus_health_metrics(
    components: &ComponentsHealth,
    metrics: &SystemMetrics,
    performance: &PerformanceMetrics,
) {
    use crate::metrics::*;

    // Update health check status metrics
    record_health_check(
        "database",
        components.database.status == "healthy",
        components.database.response_time_ms.unwrap_or(0) as f64 / 1000.0,
    );
    record_health_check(
        "vector_search",
        components.vector_search.status == "healthy",
        components.vector_search.response_time_ms.unwrap_or(0) as f64 / 1000.0,
    );
    record_health_check(
        "parser",
        components.parser.status == "healthy",
        components.parser.response_time_ms.unwrap_or(0) as f64 / 1000.0,
    );
    record_health_check("memory", components.memory.status == "healthy", 0.0);
    record_health_check(
        "storage",
        components.storage.status == "healthy",
        components.storage.response_time_ms.unwrap_or(0) as f64 / 1000.0,
    );
    record_health_check(
        "connection_pool",
        components.connection_pool.status == "healthy",
        0.0,
    );
    record_health_check("cache", components.cache.status == "healthy", 0.0);

    // Update system metrics
    SYSTEM_CPU_USAGE_PERCENT.set(metrics.cpu_usage_percent);
    SYSTEM_MEMORY_USAGE_BYTES.set(metrics.memory_usage_bytes as f64);
    SYSTEM_MEMORY_AVAILABLE_BYTES.set(metrics.memory_available_bytes as f64);

    // Update application metrics
    update_uptime();
    update_memory_metrics();
    update_connection_pool_stats(
        performance.active_connections as i64,
        (performance.active_connections as i64).saturating_sub(performance.queue_depth as i64),
    );
}

async fn check_and_update_alerts(
    components: &ComponentsHealth,
    metrics: &SystemMetrics,
    performance: &PerformanceMetrics,
) -> Vec<AlertInfo> {
    let mut alerts = Vec::new();
    let current_time = current_timestamp();

    // Check component health alerts
    for (component, status) in [
        ("database", &components.database),
        ("vector_search", &components.vector_search),
        ("parser", &components.parser),
        ("memory", &components.memory),
        ("storage", &components.storage),
        ("connection_pool", &components.connection_pool),
        ("cache", &components.cache),
    ] {
        if status.status == "unhealthy" {
            alerts.push(AlertInfo {
                severity: "critical".to_string(),
                component: component.to_string(),
                message: status
                    .error
                    .clone()
                    .unwrap_or_else(|| "Component unhealthy".to_string()),
                threshold: None,
                current_value: status.health_score,
                first_seen: current_time,
            });
        } else if status.status == "degraded" {
            alerts.push(AlertInfo {
                severity: "warning".to_string(),
                component: component.to_string(),
                message: status
                    .error
                    .clone()
                    .unwrap_or_else(|| "Component degraded".to_string()),
                threshold: None,
                current_value: status.health_score,
                first_seen: current_time,
            });
        }
    }

    // Check performance alerts
    if performance.error_rate_percent > 5.0 {
        alerts.push(AlertInfo {
            severity: "critical".to_string(),
            component: "api".to_string(),
            message: "High error rate detected".to_string(),
            threshold: Some(5.0),
            current_value: Some(performance.error_rate_percent),
            first_seen: current_time,
        });
    }

    if performance.p95_response_time_ms > 1000.0 {
        alerts.push(AlertInfo {
            severity: "warning".to_string(),
            component: "api".to_string(),
            message: "High response time detected".to_string(),
            threshold: Some(1000.0),
            current_value: Some(performance.p95_response_time_ms),
            first_seen: current_time,
        });
    }

    if metrics.memory_usage_percent > 90.0 {
        alerts.push(AlertInfo {
            severity: "critical".to_string(),
            component: "system".to_string(),
            message: "High memory usage".to_string(),
            threshold: Some(90.0),
            current_value: Some(metrics.memory_usage_percent),
            first_seen: current_time,
        });
    }

    if metrics.cpu_usage_percent > 80.0 {
        alerts.push(AlertInfo {
            severity: "warning".to_string(),
            component: "system".to_string(),
            message: "High CPU usage".to_string(),
            threshold: Some(80.0),
            current_value: Some(metrics.cpu_usage_percent),
            first_seen: current_time,
        });
    }

    alerts
}

fn determine_overall_status(
    components: &ComponentsHealth,
    performance: &PerformanceMetrics,
) -> String {
    let critical_components = [&components.database, &components.vector_search];
    let all_components = [
        &components.database,
        &components.vector_search,
        &components.parser,
        &components.memory,
        &components.storage,
        &components.connection_pool,
        &components.cache,
    ];

    // If any critical component is unhealthy, service is unhealthy
    if critical_components.iter().any(|c| c.status == "unhealthy") {
        return "unhealthy".to_string();
    }

    // If error rate is too high, service is unhealthy
    if performance.error_rate_percent > 10.0 {
        return "unhealthy".to_string();
    }

    // If any component is degraded, service is degraded
    if all_components.iter().any(|c| c.status == "degraded")
        || performance.error_rate_percent > 5.0
        || performance.p95_response_time_ms > 2000.0
    {
        return "degraded".to_string();
    }

    "healthy".to_string()
}

// Helper functions
fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn calculate_health_score(
    response_time_ms: u64,
    _details: Option<&HashMap<String, String>>,
) -> f64 {
    // Simple health score based on response time
    if response_time_ms < 100 {
        1.0
    } else if response_time_ms < 500 {
        0.8
    } else if response_time_ms < 1000 {
        0.6
    } else if response_time_ms < 2000 {
        0.4
    } else {
        0.2
    }
}

fn get_disk_usage_percent() -> f64 {
    // Simplified disk usage check - in production you'd use proper filesystem APIs
    85.0 // Placeholder
}

fn get_total_requests() -> u64 {
    // Sum all HTTP requests from metrics
    // This would need proper implementation with metric collection
    1000 // Placeholder
}

fn calculate_enhanced_rps() -> f64 {
    // Calculate from metrics over time window
    50.0 // Placeholder
}

fn calculate_enhanced_error_rate() -> f64 {
    // Calculate from error metrics
    1.2 // Placeholder
}

fn get_avg_response_time() -> f64 {
    250.0 // Placeholder
}

fn get_p95_response_time() -> f64 {
    500.0 // Placeholder
}

fn get_p99_response_time() -> f64 {
    800.0 // Placeholder
}

fn detect_memory_leaks() -> bool {
    #[cfg(feature = "leak-detect")]
    {
        if let Ok(stats) = memscope_rs::get_global_tracker().get_stats() {
            stats.leaked_memory > 10 * 1024 * 1024 // 10MB threshold
        } else {
            false
        }
    }
    #[cfg(not(feature = "leak-detect"))]
    {
        false
    }
}
