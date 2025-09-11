use core_rag_mcp_server::{CoreRagMcpServer, CoreRagServerConfig};
use rmcp::{transport::stdio, ServiceExt};
use std::env;
use tracing::{error, info};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(env::var("RUST_LOG").unwrap_or_else(|_| "info".to_string()))
        .init();

    info!("Starting Core RAG MCP Server (STDIO)...");

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

    // Create server instance
    let server = match CoreRagMcpServer::new(config) {
        Ok(server) => server,
        Err(e) => {
            error!("Failed to create server: {}", e);
            std::process::exit(1);
        }
    };

    info!("Server instance created successfully");

    // Start server with STDIO transport
    let service = server.serve(stdio()).await.inspect_err(|e| {
        error!("Error starting server: {}", e);
    })?;

    info!("Core RAG MCP Server started on STDIO transport");
    info!("Available tools:");
    info!("  - search_code: Search for code patterns using vector similarity");
    info!("  - get_code_details: Get detailed information about a code node");
    info!("  - analyze_relationships: Analyze code relationships and dependencies");
    info!("  - get_repo_stats: Get repository statistics and overview");
    info!("  - semantic_search: Semantic search using natural language queries");

    // Wait for shutdown
    let result = service.waiting().await;

    match result {
        Ok(_) => info!("Server shut down gracefully"),
        Err(e) => error!("Server error: {}", e),
    }

    Ok(())
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
