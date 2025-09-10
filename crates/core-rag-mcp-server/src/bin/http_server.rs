use core_rag_mcp_server::{CoreRagMcpServer, CoreRagServerConfig};
use rmcp::{
    transport::{
        streamable_http_server::{StreamableHttpServerConfig, StreamableHttpService},
        stdio,
    },
    ServiceExt,
};
use std::{env, net::SocketAddr, sync::Arc};
use tracing::{info, error, warn};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            env::var("RUST_LOG").unwrap_or_else(|_| "info".to_string())
        )
        .init();

    info!("Starting Core RAG MCP Server (HTTP)...");

    // Load configuration
    let config = load_config().unwrap_or_else(|e| {
        error!("Failed to load config, using defaults: {}", e);
        CoreRagServerConfig::default()
    });

    // Validate configuration
    if let Err(e) = config.validate() {
        error!("Invalid configuration: {}", e);
        std::process::exit(1);
    }

    info!("Configuration loaded and validated");
    info!("Database path: {:?}", config.database_path);
    info!("Vector dimension: {}", config.vector_config.dimension);
    info!("Worker threads: {}", config.performance.worker_threads);

    // Parse server address
    let addr = parse_server_address();
    info!("Server will bind to: {}", addr);

    // Create HTTP server configuration
    let http_config = StreamableHttpServerConfig {
        stateful_mode: true,
        ..Default::default()
    };

    // Create service factory
    let service_factory = CoreRagMcpServer::service_factory(config);
    
    // Create session manager (using default local session manager)
    let session_manager = Arc::new(
        rmcp::transport::streamable_http_server::session::local::LocalSessionManager::new()
    );

    // Create HTTP service
    let http_service = StreamableHttpService::new(
        service_factory,
        session_manager,
        http_config,
    );

    info!("HTTP service created successfully");
    info!("Available endpoints:");
    info!("  - GET  /     - Health check and server info");
    info!("  - POST /mcp  - MCP JSON-RPC requests");
    info!("  - GET  /mcp  - MCP session management (stateful mode)");
    info!("  - DELETE /mcp - Close MCP session (stateful mode)");
    
    info!("Available tools:");
    info!("  - search_code: Search for code patterns using vector similarity");
    info!("  - get_code_details: Get detailed information about a code node");
    info!("  - analyze_relationships: Analyze code relationships and dependencies");
    info!("  - get_repo_stats: Get repository statistics and overview");
    info!("  - semantic_search: Semantic search using natural language queries");

    // Create Axum app using Tower service integration
    let app = axum::Router::new()
        .route("/", axum::routing::get(health_check))
        .route("/health", axum::routing::get(health_check))
        .route("/status", axum::routing::get(server_status))
        .fallback_service(tower::service_fn(move |req| {
            let service = http_service.clone();
            async move {
                service.handle(req).await
            }
        }));

    // Create server
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    info!("Core RAG MCP Server listening on http://{}", addr);
    info!("Try: curl http://{}/health", addr);

    // Start server
    axum::serve(listener, app).await?;

    Ok(())
}

/// Health check endpoint
async fn health_check() -> axum::Json<serde_json::Value> {
    axum::Json(serde_json::json!({
        "status": "healthy",
        "service": "core-rag-mcp-server",
        "version": env!("CARGO_PKG_VERSION"),
        "transport": "http",
        "endpoints": {
            "mcp": "/mcp",
            "health": "/health",
            "status": "/status"
        },
        "capabilities": {
            "tools": true,
            "resources": false,
            "prompts": false,
            "stateful": true
        }
    }))
}

/// Server status endpoint
async fn server_status() -> axum::Json<serde_json::Value> {
    let memory_info = get_memory_info();
    
    axum::Json(serde_json::json!({
        "service": "core-rag-mcp-server",
        "version": env!("CARGO_PKG_VERSION"),
        "uptime": get_uptime(),
        "memory": memory_info,
        "features": {
            "vector_search": true,
            "semantic_search": true,
            "relationship_analysis": true,
            "repository_stats": true,
            "code_details": true
        },
        "config": {
            "transport": "http_streaming",
            "stateful_mode": true,
            "max_results": 100,
            "default_threshold": 0.7
        }
    }))
}

/// Get memory information
fn get_memory_info() -> serde_json::Value {
    // Basic memory info - in production, you might use a proper memory profiler
    serde_json::json!({
        "allocated_mb": "unknown",
        "heap_size_mb": "unknown",
        "status": "monitoring_not_implemented"
    })
}

/// Get server uptime
fn get_uptime() -> String {
    // Simple uptime - in production, you'd track actual start time
    "unknown".to_string()
}

/// Parse server address from environment or use default
fn parse_server_address() -> SocketAddr {
    let host = env::var("CORE_RAG_HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
    let port = env::var("CORE_RAG_PORT")
        .unwrap_or_else(|_| "8080".to_string())
        .parse::<u16>()
        .unwrap_or_else(|_| {
            warn!("Invalid port specified, using default 8080");
            8080
        });

    format!("{}:{}", host, port)
        .parse()
        .unwrap_or_else(|_| {
            warn!("Invalid address format, using 127.0.0.1:8080");
            "127.0.0.1:8080".parse().unwrap()
        })
}

/// Load configuration from file or environment
fn load_config() -> Result<CoreRagServerConfig, Box<dyn std::error::Error>> {
    // Try to load from config file if specified
    if let Ok(config_path) = env::var("CORE_RAG_CONFIG") {
        info!("Loading config from: {}", config_path);
        return Ok(CoreRagServerConfig::from_file(&config_path)?);
    }

    // Try to load from default config file
    let default_config_path = "core-rag-config.json";
    if std::path::Path::new(default_config_path).exists() {
        info!("Loading config from: {}", default_config_path);
        return Ok(CoreRagServerConfig::from_file(default_config_path)?);
    }

    // Check for environment variable overrides
    let mut config = CoreRagServerConfig::default();

    if let Ok(db_path) = env::var("CORE_RAG_DB_PATH") {
        config.database_path = db_path.into();
    }

    if let Ok(max_results) = env::var("CORE_RAG_MAX_RESULTS") {
        if let Ok(val) = max_results.parse::<u32>() {
            config.vector_config.max_results = val;
        }
    }

    if let Ok(threshold) = env::var("CORE_RAG_THRESHOLD") {
        if let Ok(val) = threshold.parse::<f32>() {
            config.vector_config.default_threshold = val;
        }
    }

    if let Ok(cache_size) = env::var("CORE_RAG_CACHE_SIZE_MB") {
        if let Ok(val) = cache_size.parse::<usize>() {
            config.cache_config.cache_size_mb = val;
        }
    }

    if let Ok(workers) = env::var("CORE_RAG_WORKERS") {
        if let Ok(val) = workers.parse::<usize>() {
            config.performance.worker_threads = val;
        }
    }

    info!("Using default configuration with environment overrides");
    Ok(config)
}