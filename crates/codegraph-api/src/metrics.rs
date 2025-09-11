use lazy_static::lazy_static;
use prometheus::{
    Counter, CounterVec, Gauge, GaugeVec, Histogram, HistogramOpts, HistogramVec, IntGauge, Opts,
    Registry,
};

lazy_static! {
    pub static ref REGISTRY: Registry = Registry::new();

    // Sync operations metrics
    pub static ref SYNC_OPERATIONS_TOTAL: Counter =
        Counter::with_opts(Opts::new("sync_operations_total", "Total number of sync operations"))
            .unwrap();

    pub static ref SYNC_OPERATION_DURATION_SECONDS: Histogram =
        Histogram::with_opts(Opts::new("sync_operation_duration_seconds", "Duration of sync operations in seconds"))
            .unwrap();

    // HTTP metrics
    pub static ref HTTP_REQUESTS_TOTAL: CounterVec = CounterVec::new(
        Opts::new("http_requests_total", "Total number of HTTP requests"),
        &["method", "endpoint", "status"]
    ).unwrap();

    pub static ref HTTP_REQUEST_DURATION_SECONDS: HistogramVec = HistogramVec::new(
        HistogramOpts::new("http_request_duration_seconds", "Duration of HTTP requests in seconds")
            .buckets(vec![0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0]),
        &["method", "endpoint"]
    ).unwrap();

    pub static ref HTTP_REQUESTS_IN_FLIGHT: IntGauge = IntGauge::with_opts(
        Opts::new("http_requests_in_flight", "Current number of HTTP requests being processed")
    ).unwrap();

    // Application-specific metrics
    pub static ref GRAPH_NODES_TOTAL: IntGauge = IntGauge::with_opts(
        Opts::new("graph_nodes_total", "Total number of nodes in the graph")
    ).unwrap();

    pub static ref GRAPH_EDGES_TOTAL: IntGauge = IntGauge::with_opts(
        Opts::new("graph_edges_total", "Total number of edges in the graph")
    ).unwrap();

    pub static ref VECTOR_INDEX_SIZE: IntGauge = IntGauge::with_opts(
        Opts::new("vector_index_size", "Number of vectors in the search index")
    ).unwrap();

    pub static ref VECTOR_SEARCH_DURATION_SECONDS: Histogram = Histogram::with_opts(
        HistogramOpts::new("vector_search_duration_seconds", "Duration of vector search operations in seconds")
            .buckets(vec![0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0])
    ).unwrap();

    pub static ref PARSE_OPERATIONS_TOTAL: CounterVec = CounterVec::new(
        Opts::new("parse_operations_total", "Total number of parse operations"),
        &["language", "status"]
    ).unwrap();

    pub static ref PARSE_DURATION_SECONDS: HistogramVec = HistogramVec::new(
        HistogramOpts::new("parse_duration_seconds", "Duration of parse operations in seconds")
            .buckets(vec![0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0]),
        &["language"]
    ).unwrap();

    // System metrics
    pub static ref SYSTEM_CPU_USAGE_PERCENT: Gauge = Gauge::with_opts(
        Opts::new("system_cpu_usage_percent", "CPU usage percentage")
    ).unwrap();

    pub static ref SYSTEM_MEMORY_USAGE_BYTES: Gauge = Gauge::with_opts(
        Opts::new("system_memory_usage_bytes", "Memory usage in bytes")
    ).unwrap();

    pub static ref SYSTEM_MEMORY_AVAILABLE_BYTES: Gauge = Gauge::with_opts(
        Opts::new("system_memory_available_bytes", "Available memory in bytes")
    ).unwrap();

    pub static ref SYSTEM_DISK_USAGE_BYTES: GaugeVec = GaugeVec::new(
        Opts::new("system_disk_usage_bytes", "Disk usage in bytes"),
        &["mount_point"]
    ).unwrap();

    pub static ref CONNECTION_POOL_ACTIVE: IntGauge = IntGauge::with_opts(
        Opts::new("connection_pool_active", "Active connections in the pool")
    ).unwrap();

    pub static ref CONNECTION_POOL_IDLE: IntGauge = IntGauge::with_opts(
        Opts::new("connection_pool_idle", "Idle connections in the pool")
    ).unwrap();

    // Health check metrics
    pub static ref HEALTH_CHECK_DURATION_SECONDS: HistogramVec = HistogramVec::new(
        HistogramOpts::new("health_check_duration_seconds", "Duration of health checks in seconds")
            .buckets(vec![0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0]),
        &["component"]
    ).unwrap();

    pub static ref HEALTH_CHECK_STATUS: GaugeVec = GaugeVec::new(
        Opts::new("health_check_status", "Health check status (1=healthy, 0=unhealthy)"),
        &["component"]
    ).unwrap();

    // Runtime memory/leak monitoring gauges (populated when feature `leak-detect` is enabled)
    pub static ref MEM_ACTIVE_BYTES: Gauge = Gauge::with_opts(
        Opts::new("memscope_active_memory_bytes", "Currently active allocated heap bytes tracked by memscope")
    ).unwrap();

    pub static ref MEM_ACTIVE_ALLOCATIONS: IntGauge = IntGauge::with_opts(
        Opts::new("memscope_active_allocations", "Number of active heap allocations tracked by memscope")
    ).unwrap();

    pub static ref MEM_LEAKED_BYTES: Gauge = Gauge::with_opts(
        Opts::new("memscope_leaked_memory_bytes", "Total bytes in allocations considered leaked by memscope")
    ).unwrap();

    pub static ref MEM_LEAKED_ALLOCATIONS: IntGauge = IntGauge::with_opts(
        Opts::new("memscope_leaked_allocations", "Number of allocations considered leaked by memscope")
    ).unwrap();

    // Application uptime
    pub static ref APPLICATION_UPTIME_SECONDS: Gauge = Gauge::with_opts(
        Opts::new("application_uptime_seconds", "Application uptime in seconds")
    ).unwrap();

    // Build information
    pub static ref BUILD_INFO: GaugeVec = GaugeVec::new(
        Opts::new("build_info", "Build information"),
        &["version", "git_commit", "build_date", "rust_version"]
    ).unwrap();
}

pub fn register_metrics() {
    // Sync operations
    REGISTRY
        .register(Box::new(SYNC_OPERATIONS_TOTAL.clone()))
        .unwrap();
    REGISTRY
        .register(Box::new(SYNC_OPERATION_DURATION_SECONDS.clone()))
        .unwrap();

    // HTTP metrics
    REGISTRY
        .register(Box::new(HTTP_REQUESTS_TOTAL.clone()))
        .unwrap();
    REGISTRY
        .register(Box::new(HTTP_REQUEST_DURATION_SECONDS.clone()))
        .unwrap();
    REGISTRY
        .register(Box::new(HTTP_REQUESTS_IN_FLIGHT.clone()))
        .unwrap();

    // Application-specific metrics
    REGISTRY
        .register(Box::new(GRAPH_NODES_TOTAL.clone()))
        .unwrap();
    REGISTRY
        .register(Box::new(GRAPH_EDGES_TOTAL.clone()))
        .unwrap();
    REGISTRY
        .register(Box::new(VECTOR_INDEX_SIZE.clone()))
        .unwrap();
    REGISTRY
        .register(Box::new(VECTOR_SEARCH_DURATION_SECONDS.clone()))
        .unwrap();
    REGISTRY
        .register(Box::new(PARSE_OPERATIONS_TOTAL.clone()))
        .unwrap();
    REGISTRY
        .register(Box::new(PARSE_DURATION_SECONDS.clone()))
        .unwrap();

    // System metrics
    REGISTRY
        .register(Box::new(SYSTEM_CPU_USAGE_PERCENT.clone()))
        .unwrap();
    REGISTRY
        .register(Box::new(SYSTEM_MEMORY_USAGE_BYTES.clone()))
        .unwrap();
    REGISTRY
        .register(Box::new(SYSTEM_MEMORY_AVAILABLE_BYTES.clone()))
        .unwrap();
    REGISTRY
        .register(Box::new(SYSTEM_DISK_USAGE_BYTES.clone()))
        .unwrap();
    REGISTRY
        .register(Box::new(CONNECTION_POOL_ACTIVE.clone()))
        .unwrap();
    REGISTRY
        .register(Box::new(CONNECTION_POOL_IDLE.clone()))
        .unwrap();

    // Health check metrics
    REGISTRY
        .register(Box::new(HEALTH_CHECK_DURATION_SECONDS.clone()))
        .unwrap();
    REGISTRY
        .register(Box::new(HEALTH_CHECK_STATUS.clone()))
        .unwrap();

    // Application metadata
    REGISTRY
        .register(Box::new(APPLICATION_UPTIME_SECONDS.clone()))
        .unwrap();
    REGISTRY.register(Box::new(BUILD_INFO.clone())).unwrap();

    // Register memory metrics as well; values will be zero unless updated
    // by the leak-detection task.
    REGISTRY
        .register(Box::new(MEM_ACTIVE_BYTES.clone()))
        .unwrap();
    REGISTRY
        .register(Box::new(MEM_ACTIVE_ALLOCATIONS.clone()))
        .unwrap();
    REGISTRY
        .register(Box::new(MEM_LEAKED_BYTES.clone()))
        .unwrap();
    REGISTRY
        .register(Box::new(MEM_LEAKED_ALLOCATIONS.clone()))
        .unwrap();

    // Initialize build info
    initialize_build_info();
}

/// Update memory metrics from the memscope tracker (if enabled).
#[cfg(feature = "leak-detect")]
pub fn update_memory_metrics() {
    let tracker = memscope_rs::get_global_tracker();
    if let Ok(stats) = tracker.get_stats() {
        MEM_ACTIVE_BYTES.set(stats.active_memory as f64);
        MEM_ACTIVE_ALLOCATIONS.set(stats.active_allocations as i64);
        MEM_LEAKED_BYTES.set(stats.leaked_memory as f64);
        MEM_LEAKED_ALLOCATIONS.set(stats.leaked_allocations as i64);
    }
}

/// No-op when leak detection is disabled.
#[cfg(not(feature = "leak-detect"))]
pub fn update_memory_metrics() {
    // leave gauges at their default values
}

/// Initialize build information metrics
fn initialize_build_info() {
    let version = option_env!("CARGO_PKG_VERSION").unwrap_or("0.1.0");
    let git_commit = option_env!("GIT_COMMIT").unwrap_or("unknown");
    let build_date = option_env!("BUILD_DATE").unwrap_or("unknown");
    let rust_version = option_env!("RUSTC_VERSION").unwrap_or("unknown");

    BUILD_INFO
        .with_label_values(&[version, git_commit, build_date, rust_version])
        .set(1.0);
}

/// Update system metrics
pub fn update_system_metrics() {
    use sysinfo::{ProcessExt, System, SystemExt};

    let mut sys = System::new_all();
    sys.refresh_all();

    // Update system-level metrics
    SYSTEM_CPU_USAGE_PERCENT.set(sys.global_cpu_info().cpu_usage() as f64);
    SYSTEM_MEMORY_USAGE_BYTES.set(sys.used_memory() as f64);
    SYSTEM_MEMORY_AVAILABLE_BYTES.set(sys.available_memory() as f64);

    // Update disk usage for root mount point
    if let Ok(disk_usage) = std::fs::metadata("/") {
        // This is a simplified approach - in production you'd want proper disk space calculation
        SYSTEM_DISK_USAGE_BYTES.with_label_values(&["/"]).set(0.0); // Would need proper disk space calculation
    }

    // Get current process info
    if let Ok(pid) = sysinfo::get_current_pid() {
        if let Some(process) = sys.process(pid) {
            SYSTEM_CPU_USAGE_PERCENT.set(process.cpu_usage() as f64);
        }
    }
}

/// Update application uptime
pub fn update_uptime() {
    lazy_static::lazy_static! {
        static ref START_TIME: std::time::SystemTime = std::time::SystemTime::now();
    }

    if let Ok(uptime) = START_TIME.elapsed() {
        APPLICATION_UPTIME_SECONDS.set(uptime.as_secs_f64());
    }
}

/// Record HTTP request metrics
pub fn record_http_request(method: &str, endpoint: &str, status: u16, duration: f64) {
    HTTP_REQUESTS_TOTAL
        .with_label_values(&[method, endpoint, &status.to_string()])
        .inc();

    HTTP_REQUEST_DURATION_SECONDS
        .with_label_values(&[method, endpoint])
        .observe(duration);
}

/// Record vector search metrics
pub fn record_vector_search(duration: f64) {
    VECTOR_SEARCH_DURATION_SECONDS.observe(duration);
}

/// Record parse operation metrics
pub fn record_parse_operation(language: &str, status: &str, duration: f64) {
    PARSE_OPERATIONS_TOTAL
        .with_label_values(&[language, status])
        .inc();

    PARSE_DURATION_SECONDS
        .with_label_values(&[language])
        .observe(duration);
}

/// Update graph statistics
pub fn update_graph_stats(nodes: i64, edges: i64) {
    GRAPH_NODES_TOTAL.set(nodes);
    GRAPH_EDGES_TOTAL.set(edges);
}

/// Update vector index size
pub fn update_vector_index_size(size: i64) {
    VECTOR_INDEX_SIZE.set(size);
}

/// Record health check metrics
pub fn record_health_check(component: &str, healthy: bool, duration: f64) {
    HEALTH_CHECK_DURATION_SECONDS
        .with_label_values(&[component])
        .observe(duration);

    HEALTH_CHECK_STATUS
        .with_label_values(&[component])
        .set(if healthy { 1.0 } else { 0.0 });
}

/// Update connection pool metrics
pub fn update_connection_pool_stats(active: i64, idle: i64) {
    CONNECTION_POOL_ACTIVE.set(active);
    CONNECTION_POOL_IDLE.set(idle);
}

/// Middleware for recording HTTP request metrics
pub async fn http_metrics_middleware<B>(
    req: axum::extract::Request<B>,
    next: axum::middleware::Next<B>,
) -> axum::response::Response {
    let start = std::time::Instant::now();
    let method = req.method().to_string();
    let path = req.uri().path().to_string();

    HTTP_REQUESTS_IN_FLIGHT.inc();

    let response = next.run(req).await;

    HTTP_REQUESTS_IN_FLIGHT.dec();

    let duration = start.elapsed().as_secs_f64();
    let status = response.status().as_u16();

    record_http_request(&method, &path, status, duration);

    response
}
