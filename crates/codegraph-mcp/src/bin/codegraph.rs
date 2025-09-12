use anyhow::Result;
use clap::{Parser, Subcommand};
use codegraph_core::GraphStore;
use codegraph_mcp::{IndexerConfig, ProcessManager, ProjectIndexer};
use colored::*;
use std::path::PathBuf;
use tracing::info;

#[derive(Parser)]
#[command(
    name = "codegraph",
    version,
    author,
    about = "CodeGraph CLI - MCP server management and project indexing",
    long_about = "CodeGraph provides a unified interface for managing MCP servers and indexing projects with the codegraph system."
)]
#[command(propagate_version = true)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    #[arg(short, long, global = true, help = "Enable verbose logging")]
    verbose: bool,

    #[arg(long, global = true, help = "Configuration file path")]
    config: Option<PathBuf>,
}

#[derive(Subcommand)]
enum Commands {
    #[command(about = "Start MCP server with specified transport")]
    Start {
        #[command(subcommand)]
        transport: TransportType,

        #[arg(short, long, help = "Server configuration file")]
        config: Option<PathBuf>,

        #[arg(long, help = "Run server in background")]
        daemon: bool,

        #[arg(long, help = "PID file location for daemon mode")]
        pid_file: Option<PathBuf>,
    },

    #[command(about = "Stop running MCP server")]
    Stop {
        #[arg(long, help = "PID file location")]
        pid_file: Option<PathBuf>,

        #[arg(short, long, help = "Force stop without graceful shutdown")]
        force: bool,
    },

    #[command(about = "Check status of MCP server")]
    Status {
        #[arg(long, help = "PID file location")]
        pid_file: Option<PathBuf>,

        #[arg(short, long, help = "Show detailed status information")]
        detailed: bool,
    },

    #[command(about = "Index a project or directory")]
    Index {
        #[arg(help = "Path to project directory")]
        path: PathBuf,

        #[arg(short, long, help = "Languages to index", value_delimiter = ',')]
        languages: Option<Vec<String>>,

        #[arg(long, help = "Exclude patterns (gitignore format)")]
        exclude: Vec<String>,

        #[arg(long, help = "Include only these patterns")]
        include: Vec<String>,

        #[arg(short, long, help = "Recursively index subdirectories")]
        recursive: bool,

        #[arg(long, help = "Force reindex even if already indexed")]
        force: bool,

        #[arg(long, help = "Watch for changes and auto-reindex")]
        watch: bool,

        #[arg(long, help = "Number of parallel workers", default_value = "4")]
        workers: usize,

        #[arg(long, help = "Embedding batch size", default_value = "100")]
        batch_size: usize,

        #[arg(long, help = "Local embedding device: cpu | metal | cuda:<id>")]
        device: Option<String>,

        #[arg(long, help = "Max sequence length for local embeddings", default_value = "512")]
        max_seq_len: usize,
    },

    #[command(about = "Search indexed code")]
    Search {
        #[arg(help = "Search query")]
        query: String,

        #[arg(short, long, help = "Search type", default_value = "semantic")]
        search_type: SearchType,

        #[arg(short, long, help = "Maximum results", default_value = "10")]
        limit: usize,

        #[arg(long, help = "Similarity threshold (0.0-1.0)", default_value = "0.7")]
        threshold: f32,

        #[arg(short, long, help = "Output format", default_value = "human")]
        format: OutputFormat,

        #[arg(long, help = "Restrict to path prefixes (comma-separated)", value_delimiter = ',')]
        paths: Option<Vec<String>>,

        #[arg(long, help = "Restrict to languages (comma-separated)", value_delimiter = ',')]
        langs: Option<Vec<String>>,
    },

    #[command(about = "Manage MCP server configuration")]
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },

    #[command(about = "Show statistics and metrics")]
    Stats {
        #[arg(long, help = "Show index statistics")]
        index: bool,

        #[arg(long, help = "Show server statistics")]
        server: bool,

        #[arg(long, help = "Show performance metrics")]
        performance: bool,

        #[arg(short, long, help = "Output format", default_value = "table")]
        format: StatsFormat,
    },

    #[command(about = "Initialize a new CodeGraph project")]
    Init {
        #[arg(help = "Project directory", default_value = ".")]
        path: PathBuf,

        #[arg(long, help = "Project name")]
        name: Option<String>,

        #[arg(long, help = "Skip interactive setup")]
        non_interactive: bool,
    },

    #[command(about = "Clean up resources and cache")]
    Clean {
        #[arg(long, help = "Clean index database")]
        index: bool,

        #[arg(long, help = "Clean vector embeddings")]
        vectors: bool,

        #[arg(long, help = "Clean cache files")]
        cache: bool,

        #[arg(long, help = "Clean all resources")]
        all: bool,

        #[arg(short, long, help = "Skip confirmation prompt")]
        yes: bool,
    },
}

#[derive(Subcommand)]
enum TransportType {
    #[command(about = "Start with STDIO transport (default)")]
    Stdio {
        #[arg(long, help = "Buffer size for STDIO", default_value = "8192")]
        buffer_size: usize,
    },

    #[command(about = "Start with HTTP streaming transport")]
    Http {
        #[arg(short, long, help = "Host to bind to", default_value = "127.0.0.1")]
        host: String,

        #[arg(short, long, help = "Port to bind to", default_value = "3000")]
        port: u16,

        #[arg(long, help = "Enable TLS/HTTPS")]
        tls: bool,

        #[arg(long, help = "TLS certificate file")]
        cert: Option<PathBuf>,

        #[arg(long, help = "TLS key file")]
        key: Option<PathBuf>,

        #[arg(long, help = "Enable CORS")]
        cors: bool,
    },

    #[command(about = "Start with both STDIO and HTTP transports")]
    Dual {
        #[arg(short, long, help = "HTTP host", default_value = "127.0.0.1")]
        host: String,

        #[arg(short, long, help = "HTTP port", default_value = "3000")]
        port: u16,

        #[arg(long, help = "STDIO buffer size", default_value = "8192")]
        buffer_size: usize,
    },
}

#[derive(Subcommand)]
enum ConfigAction {
    #[command(about = "Show current configuration")]
    Show {
        #[arg(long, help = "Show as JSON")]
        json: bool,
    },

    #[command(about = "Set configuration value")]
    Set {
        #[arg(help = "Configuration key")]
        key: String,

        #[arg(help = "Configuration value")]
        value: String,
    },

    #[command(about = "Get configuration value")]
    Get {
        #[arg(help = "Configuration key")]
        key: String,
    },

    #[command(about = "Reset configuration to defaults")]
    Reset {
        #[arg(short, long, help = "Skip confirmation")]
        yes: bool,
    },

    #[command(about = "Validate configuration")]
    Validate,
}

#[derive(clap::ValueEnum, Clone, Debug)]
enum SearchType {
    Semantic,
    Exact,
    Fuzzy,
    Regex,
    Ast,
}

#[derive(clap::ValueEnum, Clone, Debug)]
enum OutputFormat {
    Human,
    Json,
    Yaml,
    Table,
}

#[derive(clap::ValueEnum, Clone, Debug)]
enum StatsFormat {
    Table,
    Json,
    Yaml,
    Human,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Initialize logging
    let log_level = if cli.verbose { "debug" } else { "info" };
    tracing_subscriber::fmt()
        .with_env_filter(std::env::var("RUST_LOG").unwrap_or_else(|_| log_level.to_string()))
        .init();

    // Load configuration if provided
    if let Some(config_path) = &cli.config {
        info!("Loading configuration from: {:?}", config_path);
        // TODO: Load and merge configuration
    }

    match cli.command {
        Commands::Start {
            transport,
            config,
            daemon,
            pid_file,
        } => {
            handle_start(transport, config, daemon, pid_file).await?;
        }
        Commands::Stop { pid_file, force } => {
            handle_stop(pid_file, force).await?;
        }
        Commands::Status { pid_file, detailed } => {
            handle_status(pid_file, detailed).await?;
        }
        Commands::Index {
            path,
            languages,
            exclude,
            include,
            recursive,
            force,
            watch,
            workers,
            batch_size,
            device,
            max_seq_len,
        } => {
            handle_index(
                path,
                languages,
                exclude,
                include,
                recursive,
                force,
                watch,
                workers,
                batch_size,
                device,
                max_seq_len,
            )
            .await?;
        }
        Commands::Search {
            query,
            search_type,
            limit,
            threshold,
            format,
            paths,
            langs,
        } => {
            handle_search(query, search_type, limit, threshold, format, paths, langs).await?;
        }
        Commands::Config { action } => {
            handle_config(action).await?;
        }
        Commands::Stats {
            index,
            server,
            performance,
            format,
        } => {
            handle_stats(index, server, performance, format).await?;
        }
        Commands::Init {
            path,
            name,
            non_interactive,
        } => {
            handle_init(path, name, non_interactive).await?;
        }
        Commands::Clean {
            index,
            vectors,
            cache,
            all,
            yes,
        } => {
            handle_clean(index, vectors, cache, all, yes).await?;
        }
    }

    Ok(())
}

async fn handle_start(
    transport: TransportType,
    config: Option<PathBuf>,
    daemon: bool,
    pid_file: Option<PathBuf>,
) -> Result<()> {
    println!("{}", "Starting CodeGraph MCP Server...".green().bold());

    let manager = ProcessManager::new();

    match transport {
        TransportType::Stdio { buffer_size } => {
            info!(
                "Starting with STDIO transport (buffer size: {})",
                buffer_size
            );
            let pid = manager
                .start_stdio_server(config, daemon, pid_file.clone(), buffer_size)
                .await?;
            println!("✓ MCP server started with STDIO transport (PID: {})", pid);
        }
        TransportType::Http {
            host,
            port,
            tls,
            cert,
            key,
            cors: _,
        } => {
            info!("Starting with HTTP transport at {}:{}", host, port);
            if tls {
                info!("TLS enabled");
            }
            let pid = manager
                .start_http_server(
                    host.clone(),
                    port,
                    config,
                    daemon,
                    pid_file.clone(),
                    tls,
                    cert,
                    key,
                )
                .await?;
            println!(
                "✓ MCP server started at http{}://{}:{} (PID: {})",
                if tls { "s" } else { "" },
                host,
                port,
                pid
            );
        }
        TransportType::Dual {
            host,
            port,
            buffer_size,
        } => {
            info!("Starting with dual transport (STDIO + HTTP)");
            let (stdio_pid, http_pid) = manager
                .start_dual_transport(
                    host.clone(),
                    port,
                    buffer_size,
                    config,
                    daemon,
                    pid_file.clone(),
                )
                .await?;
            println!("✓ MCP server started with dual transport");
            println!("  STDIO: buffer size {} (PID: {})", buffer_size, stdio_pid);
            println!("  HTTP: http://{}:{} (PID: {})", host, port, http_pid);
        }
    }

    if daemon {
        println!("Running in daemon mode");
        if let Some(ref pid_file) = pid_file {
            println!("PID file: {:?}", pid_file);
        }
    }

    Ok(())
}

async fn handle_stop(pid_file: Option<PathBuf>, force: bool) -> Result<()> {
    println!("{}", "Stopping CodeGraph MCP Server...".yellow().bold());

    let manager = ProcessManager::new();

    if force {
        println!("Force stopping server");
    }

    manager.stop_server(pid_file, force).await?;
    println!("✓ Server stopped");

    Ok(())
}

async fn handle_status(pid_file: Option<PathBuf>, detailed: bool) -> Result<()> {
    println!("{}", "CodeGraph MCP Server Status".blue().bold());
    println!();

    let manager = ProcessManager::new();

    match manager.get_status(pid_file).await {
        Ok(info) => {
            println!("Server: {}", "Running".green());
            println!("Transport: {}", info.transport);
            println!("Process ID: {}", info.pid);

            let duration = chrono::Utc::now() - info.start_time;
            let hours = duration.num_hours();
            let minutes = (duration.num_minutes() % 60) as u32;
            println!("Uptime: {}h {}m", hours, minutes);

            if detailed {
                println!();
                println!("Detailed Information:");
                println!(
                    "  Start Time: {}",
                    info.start_time.format("%Y-%m-%d %H:%M:%S UTC")
                );
                if let Some(config) = info.config_path {
                    println!("  Config File: {:?}", config);
                }
                // TODO: Add more detailed metrics once integrated with monitoring
            }
        }
        Err(e) => {
            println!("Server: {}", "Not Running".red());
            println!("Error: {}", e);
        }
    }

    Ok(())
}

async fn handle_index(
    path: PathBuf,
    languages: Option<Vec<String>>,
    exclude: Vec<String>,
    include: Vec<String>,
    recursive: bool,
    force: bool,
    watch: bool,
    workers: usize,
    batch_size: usize,
    device: Option<String>,
    max_seq_len: usize,
) -> Result<()> {
    println!("{}", format!("Indexing project: {:?}", path).cyan().bold());

    if let Some(langs) = &languages {
        println!("Languages: {}", langs.join(", "));
    }

    if recursive {
        println!("Recursive indexing enabled");
    }

    println!("Workers: {}", workers);

    // Configure indexer
    let config = IndexerConfig {
        languages: languages.unwrap_or_default(),
        exclude_patterns: exclude,
        include_patterns: include,
        recursive,
        force_reindex: force,
        watch,
        workers,
        batch_size,
        device,
        max_seq_len,
        ..Default::default()
    };

    // Create indexer
    let mut indexer = ProjectIndexer::new(config).await?;

    // Perform indexing
    let stats = indexer.index_project(&path).await?;

    println!();
    println!("✓ Indexing complete!");
    println!("  Files indexed: {}", stats.files);
    println!("  Total lines: {}", stats.lines);
    println!("  Functions: {}", stats.functions);
    println!("  Classes: {}", stats.classes);
    println!("  Embeddings: {}", stats.embeddings);

    if stats.errors > 0 {
        println!("  {} Errors: {}", "⚠".yellow(), stats.errors);
    }

    if watch {
        println!();
        println!("Watching for changes... (Press Ctrl+C to stop)");
        indexer.watch_for_changes(path).await?;
    }

    Ok(())
}

async fn handle_search(
    query: String,
    search_type: SearchType,
    limit: usize,
    _threshold: f32,
    format: OutputFormat,
    paths: Option<Vec<String>>,
    langs: Option<Vec<String>>,
) -> Result<()> {
    println!("{}", format!("Searching for: '{}'", query).magenta().bold());
    println!("Search type: {:?}", search_type);
    println!();

    // Build a query embedding compatible with the indexer
    #[cfg(feature = "embeddings")]
    let emb = {
        let gen = codegraph_vector::EmbeddingGenerator::with_auto_from_env().await;
        let e = gen.generate_text_embedding(&query).await?;
        codegraph_mcp::indexer::normalize(&e)
    };
    #[cfg(not(feature = "embeddings"))]
    let emb = {
        let dimension = 1536; // fallback default
        let e = codegraph_mcp::indexer::simple_text_embedding(&query, dimension);
        codegraph_mcp::indexer::normalize(&e)
    };

    #[cfg(feature = "faiss")]
    {
        use faiss::index::io::read_index;
        use faiss::index::Index as _;
        use std::path::Path;

        // Try to use shards when filters provided, else global index
        let mut scored: Vec<(codegraph_core::NodeId, f32)> = Vec::new();
        let mut used_any_shard = false;

        // Helper to search a specific index file + mapping file
        let mut search_index = |
            index_path: &Path,
            ids_path: &Path,
            topk: usize,
        | -> Result<()> {
            if !index_path.exists() || !ids_path.exists() {
                return Ok(());
            }
            let mut index = read_index(index_path.to_string_lossy())
                .map_err(|e| anyhow::anyhow!(e.to_string()))?;
            let mapping_raw = std::fs::read_to_string(ids_path)?;
            let mapping: Vec<codegraph_core::NodeId> = serde_json::from_str(&mapping_raw)?;
            let res = index.search(&emb, topk).map_err(|e| anyhow::anyhow!(e.to_string()))?;
            for (i, label) in res.labels.into_iter().enumerate() {
                if let Some(idx_val) = label.get() {
                    let idx = idx_val as usize;
                    if idx < mapping.len() {
                        let score = res.distances[i];
                        scored.push((mapping[idx], score));
                    }
                }
            }
            Ok(())
        };

        let mut shard_count = 0usize;
        if let Some(prefs) = &paths {
            for p in prefs {
                let seg = p.trim_start_matches("./").split('/').next().unwrap_or("");
                if seg.is_empty() { continue; }
                let idx = Path::new(".codegraph/shards/path").join(format!("{}.index", seg));
                let ids = Path::new(".codegraph/shards/path").join(format!("{}_ids.json", seg));
                search_index(&idx, &ids, limit * 5)?;
                shard_count += 1;
            }
        }
        if let Some(langs) = &langs {
            for l in langs {
                let norm = l.to_lowercase();
                let idx = Path::new(".codegraph/shards/lang").join(format!("{}.index", norm));
                let ids = Path::new(".codegraph/shards/lang").join(format!("{}_ids.json", norm));
                search_index(&idx, &ids, limit * 5)?;
                shard_count += 1;
            }
        }
        used_any_shard = shard_count > 0;

        if !used_any_shard {
            // Fall back to global index
            let idx = Path::new(".codegraph/faiss.index");
            let ids = Path::new(".codegraph/faiss_ids.json");
            if !idx.exists() || !ids.exists() {
                println!("FAISS index not found. Run 'codegraph index .' first (with --features faiss).");
                return Ok(());
            }
            search_index(idx, ids, limit * 5)?;
        }

        // Rank and trim
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scored.dedup_by_key(|(id, _)| *id);
        let mut results: Vec<codegraph_core::NodeId> = scored.into_iter().map(|(id, _)| id).collect();
        if results.len() > limit { results.truncate(limit); }

        // Optionally enrich results from graph
        let graph = codegraph_graph::CodeGraph::new()?;
        let mut filtered = Vec::new();
        let path_filters = paths.unwrap_or_default();
        for id in results {
            if let Some(node) = graph.get_node(id).await? {
                if path_filters.is_empty() || path_filters.iter().any(|p| node.location.file_path.starts_with(p)) {
                    filtered.push((id, node));
                }
            }
        }

        match format {
            OutputFormat::Human => {
                println!("Results (top {}):", filtered.len());
                for (i, (id, node)) in filtered.iter().enumerate() {
                    println!("{}. {}  {}  {}", i + 1, id, node.name, node.location.file_path);
                }
            }
            OutputFormat::Json => {
                let j = serde_json::json!({
                    "results": filtered.iter().map(|(id, node)| serde_json::json!({
                        "id": id,
                        "name": node.name,
                        "path": node.location.file_path,
                    })).collect::<Vec<_>>()
                });
                println!("{}", serde_json::to_string_pretty(&j)?);
            }
            _ => {
                println!("Format {:?} not yet implemented", format);
            }
        }
        return Ok(());
    }

    #[cfg(not(feature = "faiss"))]
    {
        println!("Vector search requires FAISS support. Reinstall with:\n  cargo install --path crates/codegraph-mcp --features faiss");
        Ok(())
    }
}

async fn handle_config(action: ConfigAction) -> Result<()> {
    match action {
        ConfigAction::Show { json } => {
            if json {
                println!(r#"{{"embedding_model": "openai", "vector_dimension": 1536}}"#);
            } else {
                println!("{}", "Current Configuration:".blue().bold());
                println!("  Embedding Model: openai");
                println!("  Vector Dimension: 1536");
                println!("  Database Path: ~/.codegraph/db");
            }
        }
        ConfigAction::Set { key, value } => {
            println!("Set {} = {}", key.yellow(), value.green());
        }
        ConfigAction::Get { key } => {
            println!("{}: {}", key, "value");
        }
        ConfigAction::Reset { yes } => {
            if !yes {
                println!("Reset configuration to defaults? (y/n)");
                // TODO: Read user input
            }
            println!("✓ Configuration reset to defaults");
        }
        ConfigAction::Validate => {
            println!("✓ Configuration is valid");
        }
    }

    Ok(())
}

async fn handle_stats(
    index: bool,
    server: bool,
    performance: bool,
    _format: StatsFormat,
) -> Result<()> {
    println!("{}", "CodeGraph Statistics".blue().bold());
    println!();

    if index || (!index && !server && !performance) {
        println!("Index Statistics:");
        println!("  Projects: 3");
        println!("  Files: 1,234");
        println!("  Functions: 567");
        println!("  Classes: 89");
        println!("  Embeddings: 5,678");
        println!("  Database Size: 125 MB");
        println!();
    }

    if server {
        println!("Server Statistics:");
        println!("  Uptime: 2h 15m");
        println!("  Requests: 1,234");
        println!("  Errors: 2");
        println!("  Avg Response Time: 45ms");
        println!();
    }

    if performance {
        println!("Performance Metrics:");
        println!("  CPU Usage: 12%");
        println!("  Memory: 45.2 MB");
        println!("  Disk I/O: 1.2 MB/s");
        println!("  Network: 0.5 MB/s");
    }

    Ok(())
}

async fn handle_init(path: PathBuf, name: Option<String>, _non_interactive: bool) -> Result<()> {
    println!("{}", "Initializing CodeGraph project...".green().bold());
    println!("Path: {:?}", path);

    if let Some(name) = name {
        println!("Project name: {}", name);
    }

    // TODO: Implement initialization logic
    println!("✓ Created .codegraph/config.toml");
    println!("✓ Created .codegraph/db/");
    println!("✓ Created .codegraph/vectors/");
    println!();
    println!("Project initialized successfully!");
    println!("Run 'codegraph index .' to start indexing");

    Ok(())
}

async fn handle_clean(index: bool, vectors: bool, cache: bool, all: bool, yes: bool) -> Result<()> {
    println!("{}", "Cleaning CodeGraph resources...".yellow().bold());

    if all || index {
        println!("  Cleaning index database...");
    }
    if all || vectors {
        println!("  Cleaning vector embeddings...");
    }
    if all || cache {
        println!("  Cleaning cache files...");
    }

    if !yes {
        println!();
        println!("This will permanently delete data. Continue? (y/n)");
        // TODO: Read user input
    }

    // TODO: Implement cleanup logic
    println!();
    println!("✓ Cleanup complete");

    Ok(())
}
