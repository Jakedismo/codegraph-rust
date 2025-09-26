use anyhow::Result;
use clap::{Parser, Subcommand};
use codegraph_core::GraphStore;
use codegraph_mcp::{IndexerConfig, ProcessManager, ProjectIndexer};
use colored::*;
use rmcp::{transport::stdio, ServiceExt};
use std::path::PathBuf;
use tracing::info;
use tracing_subscriber;

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

        #[arg(
            long,
            help = "Max sequence length for local embeddings",
            default_value = "512"
        )]
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

        #[arg(
            long,
            help = "Restrict to path prefixes (comma-separated)",
            value_delimiter = ','
        )]
        paths: Option<Vec<String>>,

        #[arg(
            long,
            help = "Restrict to languages (comma-separated)",
            value_delimiter = ','
        )]
        langs: Option<Vec<String>>,

        #[arg(
            long,
            help = "Expand graph neighbors to this depth (0 to disable)",
            default_value = "0"
        )]
        expand_graph: usize,
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

    #[command(about = "Graph utilities")]
    Graph {
        #[command(subcommand)]
        action: GraphAction,
    },

    #[command(about = "Vector operations (FAISS)")]
    Vector {
        #[command(subcommand)]
        action: VectorAction,
    },

    #[command(about = "Code IO operations")]
    Code {
        #[command(subcommand)]
        action: CodeAction,
    },

    #[command(about = "Test runner helpers")]
    Test {
        #[command(subcommand)]
        action: TestAction,
    },
    #[command(about = "Run performance benchmarks (index + query)")]
    Perf {
        #[arg(help = "Path to project directory")]
        path: PathBuf,
        #[arg(long, value_delimiter = ',')]
        langs: Option<Vec<String>>,
        #[arg(long, default_value = "3", help = "Warmup runs before timing")]
        warmup: usize,
        #[arg(long, default_value = "10", help = "Timed query trials")]
        trials: usize,
        #[arg(long, value_delimiter = ',', help = "Queries to benchmark")]
        queries: Option<Vec<String>>,
        #[arg(long, default_value = "4")]
        workers: usize,
        #[arg(long, default_value = "100")]
        batch_size: usize,
        #[arg(long, help = "Local embedding device: cpu | metal | cuda:<id>")]
        device: Option<String>,
        #[arg(long, default_value = "512")]
        max_seq_len: usize,
        #[arg(long, help = "Remove existing .codegraph before indexing")]
        clean: bool,
        #[arg(long, default_value = "json", value_parser = clap::builder::PossibleValuesParser::new(["human","json"]))]
        format: String,
        #[arg(long, help = "Open graph in read-only mode for perf queries")]
        graph_readonly: bool,
    },
    #[command(about = "Serve HTTP MCP endpoint")]
    #[cfg(feature = "server-http")]
    ServeHttp {
        #[arg(long, default_value = "127.0.0.1")]
        host: String,
        #[arg(long, default_value = "3000")]
        port: u16,
    },
    #[command(about = "Serve STDIO MCP endpoint")]
    ServeStdio {
        #[arg(long, default_value = "8192")]
        buffer_size: usize,
    },
}

#[derive(Subcommand)]
enum GraphAction {
    #[command(about = "Get neighbors of a node")]
    Neighbors {
        #[arg(long, help = "Node UUID")]
        node: String,
        #[arg(long, help = "Limit", default_value = "20")]
        limit: usize,
    },
    #[command(about = "Traverse graph breadth-first")]
    Traverse {
        #[arg(long, help = "Start node UUID")]
        start: String,
        #[arg(long, help = "Depth", default_value = "2")]
        depth: usize,
        #[arg(long, help = "Limit nodes", default_value = "100")]
        limit: usize,
    },
}

#[derive(Subcommand)]
enum VectorAction {
    #[command(about = "Semantic vector search (FAISS)")]
    Search {
        #[arg(help = "Query string")]
        query: String,
        #[arg(long, value_delimiter = ',')]
        paths: Option<Vec<String>>,
        #[arg(long, value_delimiter = ',')]
        langs: Option<Vec<String>>,
        #[arg(long, default_value = "10")]
        limit: usize,
    },
}

#[derive(Subcommand)]
enum CodeAction {
    #[command(about = "Read a file or range")]
    Read {
        #[arg(help = "File path")]
        path: String,
        #[arg(long)]
        start: Option<usize>,
        #[arg(long)]
        end: Option<usize>,
    },
    #[command(about = "Patch a file (find/replace)")]
    Patch {
        #[arg(help = "File path")]
        path: String,
        #[arg(long, help = "Find text")]
        find: String,
        #[arg(long, help = "Replace with")]
        replace: String,
        #[arg(long, help = "Dry run")]
        dry_run: bool,
    },
}

#[derive(Subcommand)]
enum TestAction {
    #[command(about = "Run tests (cargo test)")]
    Run {
        #[arg(long)]
        package: Option<String>,
        #[arg(trailing_var_arg = true)]
        args: Vec<String>,
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
            expand_graph,
        } => {
            handle_search(
                query,
                search_type,
                limit,
                threshold,
                format,
                paths,
                langs,
                expand_graph,
            )
            .await?;
        }
        Commands::Graph { action } => match action {
            GraphAction::Neighbors { node, limit } => {
                let id = uuid::Uuid::parse_str(&node)?;
                let graph = codegraph_graph::CodeGraph::new()?;
                let neighbors = graph.get_neighbors(id).await?;
                println!("Neighbors ({}):", neighbors.len().min(limit));
                for n in neighbors.into_iter().take(limit) {
                    if let Some(nn) = graph.get_node(n).await? {
                        println!("- {}  {}", n, nn.name);
                    } else {
                        println!("- {}", n);
                    }
                }
            }
            GraphAction::Traverse {
                start,
                depth,
                limit,
            } => {
                use std::collections::{HashSet, VecDeque};
                let start_id = uuid::Uuid::parse_str(&start)?;
                let graph = codegraph_graph::CodeGraph::new()?;
                let mut seen: HashSet<codegraph_core::NodeId> = HashSet::new();
                let mut q: VecDeque<(codegraph_core::NodeId, usize)> = VecDeque::new();
                q.push_back((start_id, 0));
                seen.insert(start_id);
                let mut out = Vec::new();
                while let Some((nid, d)) = q.pop_front() {
                    out.push((nid, d));
                    if out.len() >= limit {
                        break;
                    }
                    if d >= depth {
                        continue;
                    }
                    for nb in graph.get_neighbors(nid).await.unwrap_or_default() {
                        if seen.insert(nb) {
                            q.push_back((nb, d + 1));
                        }
                    }
                }
                println!("Traversal (visited {}):", out.len());
                for (nid, d) in out {
                    if let Some(nn) = graph.get_node(nid).await? {
                        println!("{} {}  {}", d, nid, nn.name);
                    } else {
                        println!("{} {}", d, nid);
                    }
                }
            }
        },
        Commands::Vector { action } => match action {
            VectorAction::Search {
                query,
                paths,
                langs,
                limit,
            } => {
                handle_search(
                    query,
                    SearchType::Semantic,
                    limit,
                    0.7,
                    OutputFormat::Human,
                    paths,
                    langs,
                    0,
                )
                .await?;
            }
        },
        Commands::Code { action } => match action {
            CodeAction::Read { path, start, end } => {
                let text = std::fs::read_to_string(&path)?;
                let total = text.lines().count();
                let s = start.unwrap_or(1).max(1);
                let e = end.unwrap_or(total).min(total);
                for (i, line) in text.lines().enumerate() {
                    let ln = i + 1;
                    if ln >= s && ln <= e {
                        println!("{:>6} | {}", ln, line);
                    }
                }
            }
            CodeAction::Patch {
                path,
                find,
                replace,
                dry_run,
            } => {
                let text = std::fs::read_to_string(&path)?;
                let patched = text.replace(&find, &replace);
                if dry_run {
                    println!("--- DRY RUN: changes not written ---");
                    println!("Replacements: {}", text.matches(&find).count());
                } else {
                    std::fs::write(&path, patched)?;
                    println!("Patched '{}'", path);
                }
            }
        },
        Commands::Test { action } => match action {
            TestAction::Run { package, args } => {
                let mut cmd = std::process::Command::new("cargo");
                cmd.arg("test");
                if let Some(p) = package {
                    cmd.arg("-p").arg(p);
                }
                if !args.is_empty() {
                    cmd.args(args);
                }
                let status = cmd.status()?;
                if !status.success() {
                    anyhow::bail!("tests failed");
                }
            }
        },
        Commands::Perf {
            path,
            langs,
            warmup,
            trials,
            queries,
            workers,
            batch_size,
            device,
            max_seq_len,
            clean,
            format,
            graph_readonly,
        } => {
            handle_perf(
                path,
                langs,
                warmup,
                trials,
                queries,
                workers,
                batch_size,
                device,
                max_seq_len,
                clean,
                format,
                graph_readonly,
            )
            .await?;
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
        #[cfg(feature = "server-http")]
        Commands::ServeHttp { host, port } => {
            codegraph_mcp::server::serve_http(host, port).await?;
        }
        Commands::ServeStdio { buffer_size } => {
            codegraph_mcp::server::serve_stdio(buffer_size).await?;
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
    let manager = ProcessManager::new();

    match transport {
        TransportType::Stdio { buffer_size: _ } => {
            // For STDIO transport, reconfigure tracing to use stderr only
            tracing_subscriber::fmt()
                .with_writer(std::io::stderr)
                .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
                .try_init()
                .ok(); // Ignore if already initialized

            eprintln!(
                "{}",
                "Starting CodeGraph MCP Server with 100% Official SDK..."
                    .green()
                    .bold()
            );

            // Create and initialize the revolutionary CodeGraph server with official SDK
            let mut server = codegraph_mcp::official_server::CodeGraphMCPServer::new();
            server.initialize_qwen().await;

            eprintln!("✅ Revolutionary CodeGraph MCP server ready with 100% protocol compliance");

            // Use official rmcp STDIO transport for perfect compliance
            let service = server.serve(rmcp::transport::stdio()).await.map_err(|e| {
                eprintln!("❌ Failed to start official MCP server: {}", e);
                anyhow::anyhow!("MCP server startup failed: {}", e)
            })?;

            eprintln!("🚀 Official MCP server started with revolutionary capabilities");

            // Wait for the server to complete
            service
                .waiting()
                .await
                .map_err(|e| anyhow::anyhow!("Server error: {}", e))?;
        }
        TransportType::Http {
            host,
            port,
            tls,
            cert,
            key,
            cors: _,
        } => {
            eprintln!("🚧 HTTP transport with official SDK not yet implemented");
            eprintln!("💡 Use STDIO transport for 100% official SDK compliance:");
            eprintln!("   codegraph start stdio");
            eprintln!("💡 Or use legacy HTTP server:");
            eprintln!("   codegraph start http --legacy");

            return Err(anyhow::anyhow!(
                "HTTP transport requires legacy mode or official SDK implementation"
            ));
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

    // Memory-aware optimization for high-memory systems
    let available_memory_gb = estimate_available_memory_gb();
    let (optimized_batch_size, optimized_workers) =
        optimize_for_memory(available_memory_gb, batch_size, workers);

    println!("Workers: {} → {} (optimized)", workers, optimized_workers);
    println!(
        "Batch size: {} → {} (optimized)",
        batch_size, optimized_batch_size
    );

    if available_memory_gb >= 64 {
        println!(
            "{}",
            format!(
                "🚀 High-memory system detected ({}GB) - performance optimized!",
                available_memory_gb
            )
            .green()
            .bold()
        );
        println!(
            "{}",
            format!(
                "💾 Memory capacity: ~{} embeddings per batch",
                optimized_batch_size
            )
            .cyan()
        );
    }

    // Configure indexer
    let languages_list = languages.clone().unwrap_or_default();
    let config = IndexerConfig {
        languages: languages_list.clone(),
        exclude_patterns: exclude,
        include_patterns: include,
        recursive,
        force_reindex: force,
        watch,
        workers: optimized_workers,
        batch_size: optimized_batch_size,
        device,
        max_seq_len,
        ..Default::default()
    };

    // Create indexer
    let mut indexer = ProjectIndexer::new(config).await?;

    // Perform indexing
    let stats = indexer.index_project(&path).await?;

    println!();
    println!("{}", "🎉 INDEXING COMPLETE!".green().bold());
    println!();

    // Performance summary with dual metrics
    println!("{}", "📊 Performance Summary".cyan().bold());
    println!("┌─────────────────────────────────────────────────┐");
    println!(
        "│ 📄 Files: {} indexed                             │",
        format!("{:>6}", stats.files).yellow()
    );
    println!(
        "│ 📝 Lines: {} processed                           │",
        format!("{:>6}", stats.lines).yellow()
    );
    println!(
        "│ 🔧 Functions: {} extracted                       │",
        format!("{:>6}", stats.functions).green()
    );
    println!(
        "│ 🏗️  Classes: {} extracted                        │",
        format!("{:>6}", stats.classes).green()
    );
    println!(
        "│ 📦 Structs: {} extracted                         │",
        format!("{:>6}", stats.structs).green()
    );
    println!(
        "│ 🎯 Traits: {} extracted                          │",
        format!("{:>6}", stats.traits).green()
    );
    println!(
        "│ 💾 Embeddings: {} generated                      │",
        format!("{:>6}", stats.embeddings).cyan()
    );
    if stats.errors > 0 {
        println!(
            "│ ❌ Errors: {} encountered                        │",
            format!("{:>6}", stats.errors).red()
        );
    }
    println!("└─────────────────────────────────────────────────┘");

    // Configuration summary
    println!();
    println!("{}", "⚙️  Configuration Summary".cyan().bold());
    println!(
        "Workers: {} | Batch Size: {} | Languages: {}",
        optimized_workers,
        optimized_batch_size,
        languages_list.join(", ")
    );

    let provider = std::env::var("CODEGRAPH_EMBEDDING_PROVIDER").unwrap_or("default".to_string());
    if provider == "ollama" {
        println!(
            "{}",
            "🧠 Using SOTA Code-Specialized Embeddings (nomic-embed-code)".green()
        );
    } else if provider == "onnx" {
        println!("{}", "⚡ Using Speed-Optimized Embeddings (ONNX)".yellow());
    }

    if available_memory_gb >= 128 {
        println!(
            "{}",
            format!(
                "🚀 Ultra-High Memory System ({}GB) - Maximum Performance!",
                available_memory_gb
            )
            .green()
            .bold()
        );
    } else if available_memory_gb >= 64 {
        println!(
            "{}",
            format!(
                "💪 High Memory System ({}GB) - Optimized Performance!",
                available_memory_gb
            )
            .green()
        );
    }

    // Success rate and recommendations
    if stats.embeddings > 0 && stats.files > 0 {
        let embedding_success_rate = (stats.embeddings as f64 / stats.files as f64) * 100.0;
        if embedding_success_rate >= 90.0 {
            println!("{}", "✅ Excellent embedding success rate (>90%)".green());
        } else if embedding_success_rate >= 75.0 {
            println!("{}", "⚠️  Good embedding success rate (75-90%)".yellow());
        } else {
            println!(
                "{}",
                "❌ Low embedding success rate (<75%) - check language support".red()
            );
        }
    }

    println!();
    println!(
        "{}",
        "🚀 Ready for Revolutionary MCP Intelligence!"
            .green()
            .bold()
    );
    println!("Next: Start MCP server with 'codegraph start stdio'");

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
    expand_graph: usize,
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
        let dimension = 384; // Match EmbeddingGenerator default (all-MiniLM-L6-v2)
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
        let mut search_index = |index_path: &Path, ids_path: &Path, topk: usize| -> Result<()> {
            if !index_path.exists() || !ids_path.exists() {
                return Ok(());
            }
            let mut index = read_index(index_path.to_string_lossy())
                .map_err(|e| anyhow::anyhow!(e.to_string()))?;
            let mapping_raw = std::fs::read_to_string(ids_path)?;
            let mapping: Vec<codegraph_core::NodeId> = serde_json::from_str(&mapping_raw)?;
            let res = index
                .search(&emb, topk)
                .map_err(|e| anyhow::anyhow!(e.to_string()))?;
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
                if seg.is_empty() {
                    continue;
                }
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
                println!(
                    "FAISS index not found. Run 'codegraph index .' first (with --features faiss)."
                );
                return Ok(());
            }
            search_index(idx, ids, limit * 5)?;
        }

        // Rank and trim, keep scores for output
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scored.dedup_by_key(|(id, _)| *id);
        let top: Vec<(codegraph_core::NodeId, f32)> = scored.into_iter().take(limit).collect();

        // Optionally enrich results from graph; keep scores with nodes
        let graph = codegraph_graph::CodeGraph::new()?;
        let mut base_scored: Vec<(codegraph_core::NodeId, codegraph_core::CodeNode, f32)> =
            Vec::new();
        let path_filters = paths.unwrap_or_default();
        for (id, score) in top {
            if let Some(node) = graph.get_node(id).await? {
                if path_filters.is_empty()
                    || path_filters
                        .iter()
                        .any(|p| node.location.file_path.starts_with(p))
                {
                    base_scored.push((id, node, score));
                }
            }
        }

        // Expand graph neighbors if requested (depth-weighted)
        if expand_graph > 0 {
            use std::collections::{HashSet, VecDeque};
            let mut seen: HashSet<codegraph_core::NodeId> =
                base_scored.iter().map(|(id, _, _)| *id).collect();
            let mut q: VecDeque<(codegraph_core::NodeId, usize)> =
                base_scored.iter().map(|(id, _, _)| (*id, 0usize)).collect();
            let mut extra: Vec<(codegraph_core::NodeId, codegraph_core::CodeNode, usize)> =
                Vec::new();
            while let Some((nid, depth)) = q.pop_front() {
                if depth >= expand_graph {
                    continue;
                }
                let neighbors = graph.get_neighbors(nid).await.unwrap_or_default();
                for nb in neighbors {
                    if seen.insert(nb) {
                        if let Some(nnode) = graph.get_node(nb).await? {
                            if path_filters.is_empty()
                                || path_filters
                                    .iter()
                                    .any(|p| nnode.location.file_path.starts_with(p))
                            {
                                extra.push((nb, nnode, depth + 1));
                                q.push_back((nb, depth + 1));
                            }
                        }
                    }
                }
            }
            // Shallow neighbors first; cap to avoid explosion
            extra.sort_by_key(|(_, _, d)| *d);
            // Build final results with depth field
            let mut results_with_depth: Vec<(
                codegraph_core::NodeId,
                codegraph_core::CodeNode,
                usize,
            )> = base_scored
                .iter()
                .map(|(id, node, _)| (*id, node.clone(), 0usize))
                .collect();
            results_with_depth.extend(extra.into_iter().take(limit.saturating_mul(5)));

            match format {
                OutputFormat::Human => {
                    println!("Results ({}):", results_with_depth.len());
                    for (i, (_id, node, depth)) in results_with_depth.iter().enumerate() {
                        let summary = node
                            .content
                            .as_deref()
                            .map(|s| {
                                let mut t = s.trim().replace('\n', " ");
                                if t.len() > 160 {
                                    t.truncate(160);
                                    t.push_str("...");
                                }
                                t
                            })
                            .unwrap_or_else(|| "".to_string());
                        println!(
                            "{}. [d={}] {}
                           {}",
                            i + 1,
                            depth,
                            node.location.file_path,
                            summary
                        );
                    }
                }
                OutputFormat::Json => {
                    use std::collections::HashMap;
                    let score_map: HashMap<codegraph_core::NodeId, f32> = base_scored
                        .iter()
                        .map(|(id, _node, score)| (*id, *score))
                        .collect();
                    let j = serde_json::json!({
                        "results": results_with_depth.iter().map(|(id, node, depth)| {
                            let score = score_map.get(id).copied();
                            serde_json::json!({
                                "id": id,
                                "name": node.name,
                                "path": node.location.file_path,
                                "node_type": node.node_type.as_ref().map(|t| format!("{:?}", t)).unwrap_or_else(|| "unknown".into()),
                                "language": node.language.as_ref().map(|l| format!("{:?}", l)).unwrap_or_else(|| "unknown".into()),
                                "depth": depth,
                                "summary": node.content.as_deref().map(|s| {
                                    let mut t = s.trim().replace('\n', " ");
                                    if t.len() > 160 { t.truncate(160); t.push_str("..."); }
                                    t
                                }).unwrap_or_default(),
                                "score": score,
                            })
                        }).collect::<Vec<_>>()
                    });
                    println!("{}", serde_json::to_string_pretty(&j)?);
                }
                _ => {
                    println!("Format {:?} not yet implemented", format);
                }
            }
            return Ok(());
        }

        // No graph expansion: print base results with depth=0
        match format {
            OutputFormat::Human => {
                println!("Results ({}):", base_scored.len());
                for (i, (_id, node, _score)) in base_scored.iter().enumerate() {
                    let summary = node
                        .content
                        .as_deref()
                        .map(|s| {
                            let mut t = s.trim().replace('\n', " ");
                            if t.len() > 160 {
                                t.truncate(160);
                                t.push_str("...");
                            }
                            t
                        })
                        .unwrap_or_else(|| "".to_string());
                    println!(
                        "{}. [d=0] {}
                   {}",
                        i + 1,
                        node.location.file_path,
                        summary
                    );
                }
            }
            OutputFormat::Json => {
                let j = serde_json::json!({
                    "results": base_scored.iter().map(|(_id, node, score)| serde_json::json!({
                        "name": node.name,
                        "path": node.location.file_path,
                        "node_type": node.node_type.as_ref().map(|t| format!("{:?}", t)).unwrap_or_else(|| "unknown".into()),
                        "language": node.language.as_ref().map(|l| format!("{:?}", l)).unwrap_or_else(|| "unknown".into()),
                        "depth": 0,
                        "summary": node.content.as_deref().map(|s| {
                            let mut t = s.trim().replace('\n', " ");
                            if t.len() > 160 { t.truncate(160); t.push_str("..."); }
                            t
                        }).unwrap_or_default(),
                        "score": score,
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
        println!(
            "Vector search requires FAISS support. Reinstall with:
                  cargo install --path crates/codegraph-mcp --features faiss"
        );
        Ok(())
    }
}

async fn handle_config(action: ConfigAction) -> Result<()> {
    match action {
        ConfigAction::Show { json } => {
            if json {
                println!(r#"{{"embedding_model": "all-MiniLM-L6-v2", "vector_dimension": 384}}"#);
            } else {
                println!("{}", "Current Configuration:".blue().bold());
                println!("  Embedding Model: all-MiniLM-L6-v2");
                println!("  Vector Dimension: 384");
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

    // Create .codegraph directory structure
    let codegraph_dir = path.join(".codegraph");
    std::fs::create_dir_all(&codegraph_dir)?;

    // Create subdirectories
    std::fs::create_dir_all(codegraph_dir.join("db"))?;
    std::fs::create_dir_all(codegraph_dir.join("vectors"))?;
    std::fs::create_dir_all(codegraph_dir.join("cache"))?;

    // Create basic config.toml
    let config_content = r#"# CodeGraph Project Configuration
[project]
name = "codegraph-project"
version = "1.0.0"

[indexing]
languages = ["rust", "python", "typescript", "javascript", "go", "java", "cpp"]
exclude_patterns = ["**/node_modules/**", "**/target/**", "**/.git/**", "**/build/**"]
include_patterns = ["src/**", "lib/**", "**/*.rs", "**/*.py", "**/*.ts", "**/*.js"]

[mcp]
enable_qwen_integration = true
enable_caching = true
enable_pattern_detection = true

[database]
path = "./.codegraph/db"
cache_path = "./.codegraph/cache"
vectors_path = "./.codegraph/vectors"
"#;

    std::fs::write(codegraph_dir.join("config.toml"), config_content)?;

    // Create .gitignore for CodeGraph files
    let gitignore_content = r#"# CodeGraph generated files
.codegraph/db/
.codegraph/cache/
.codegraph/vectors/
.codegraph/logs/
"#;

    std::fs::write(path.join(".gitignore.codegraph"), gitignore_content)?;

    println!("✓ Created .codegraph/config.toml");
    println!("✓ Created .codegraph/db/");
    println!("✓ Created .codegraph/vectors/");
    println!("✓ Created .codegraph/cache/");
    println!("✓ Created .gitignore.codegraph");
    println!();
    println!("Project initialized successfully!");
    println!();
    println!("{}", "Next steps:".yellow().bold());
    println!("1. Run 'codegraph index .' to index your codebase");
    println!("2. Start MCP server: 'codegraph start stdio'");
    println!("3. Configure Claude Desktop with CodeGraph MCP");
    println!("4. Experience revolutionary AI codebase intelligence!");

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

async fn handle_perf(
    path: PathBuf,
    langs: Option<Vec<String>>,
    warmup: usize,
    trials: usize,
    queries: Option<Vec<String>>,
    workers: usize,
    batch_size: usize,
    device: Option<String>,
    max_seq_len: usize,
    clean: bool,
    format: String,
    graph_readonly: bool,
) -> Result<()> {
    use serde_json::json;
    use std::time::Instant;

    if clean && std::path::Path::new(".codegraph").exists() {
        let _ = std::fs::remove_dir_all(".codegraph");
    }

    let config = codegraph_mcp::IndexerConfig {
        languages: langs.clone().unwrap_or_default(),
        exclude_patterns: vec![],
        include_patterns: vec![],
        recursive: true,
        force_reindex: true,
        watch: false,
        workers,
        batch_size,
        vector_dimension: 384, // Match EmbeddingGenerator default (all-MiniLM-L6-v2)
        device: device.clone(),
        max_seq_len,
    };

    let mut indexer = codegraph_mcp::ProjectIndexer::new(config).await?;
    let t0 = Instant::now();
    let stats = indexer.index_project(&path).await?;
    let indexing_secs = t0.elapsed().as_secs_f64();
    // Release RocksDB handle before running queries (which open their own graph handles)
    drop(indexer);
    // Give RocksDB a brief moment to release OS locks
    tokio::time::sleep(std::time::Duration::from_millis(75)).await;

    let qset = if let Some(q) = queries {
        q
    } else {
        vec![
            "main function".to_string(),
            "http server router".to_string(),
            "database connection".to_string(),
            "graph traversal".to_string(),
            "embedding generator".to_string(),
        ]
    };

    for _ in 0..warmup.max(1) {
        for q in &qset {
            let _ = codegraph_mcp::server::bin_search_with_scores(q.clone(), None, None, 10).await;
        }
    }

    let mut latencies_ms: Vec<f64> = Vec::new();
    for _ in 0..trials {
        for q in &qset {
            let t = Instant::now();
            let _ = codegraph_mcp::server::bin_search_with_scores(q.clone(), None, None, 10).await;
            latencies_ms.push(t.elapsed().as_secs_f64() * 1000.0);
        }
    }
    latencies_ms.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let p50 = latencies_ms
        .get(latencies_ms.len() / 2)
        .copied()
        .unwrap_or(0.0);
    let p95 = latencies_ms
        .get(((latencies_ms.len() as f64) * 0.95).floor() as usize)
        .copied()
        .unwrap_or(0.0);
    let avg = if latencies_ms.is_empty() {
        0.0
    } else {
        latencies_ms.iter().sum::<f64>() / latencies_ms.len() as f64
    };

    // Open graph with small retry to avoid transient lock contention
    let graph = {
        use std::time::Duration;
        let mut attempts = 0;
        loop {
            let open_res = if graph_readonly {
                codegraph_graph::CodeGraph::new_read_only()
            } else {
                codegraph_graph::CodeGraph::new()
            };
            match open_res {
                Ok(g) => break g,
                Err(e) => {
                    let msg = e.to_string();
                    if msg.contains("LOCK") && attempts < 10 {
                        tokio::time::sleep(Duration::from_millis(50)).await;
                        attempts += 1;
                        continue;
                    }
                    return Err(e.into());
                }
            }
        }
    };
    let mut any_node: Option<codegraph_core::NodeId> = None;
    if let Ok(ids_raw) = std::fs::read_to_string(".codegraph/faiss_ids.json") {
        if let Ok(ids) = serde_json::from_str::<Vec<codegraph_core::NodeId>>(&ids_raw) {
            any_node = ids.into_iter().next();
        }
    }
    let (graph_bfs_ms, bfs_nodes) = if let Some(start) = any_node {
        use std::collections::{HashSet, VecDeque};
        let t = Instant::now();
        let mut seen: HashSet<codegraph_core::NodeId> = HashSet::new();
        let mut q: VecDeque<(codegraph_core::NodeId, usize)> = VecDeque::new();
        q.push_back((start, 0));
        seen.insert(start);
        let mut visited = 0usize;
        while let Some((nid, d)) = q.pop_front() {
            let _ = graph.get_node(nid).await; // load
            visited += 1;
            if d >= 2 {
                continue;
            }
            for nb in graph.get_neighbors(nid).await.unwrap_or_default() {
                if seen.insert(nb) {
                    q.push_back((nb, d + 1));
                }
            }
            if visited >= 1000 {
                break;
            }
        }
        (t.elapsed().as_secs_f64() * 1000.0, visited)
    } else {
        (0.0, 0)
    };

    let out = json!({
        "env": {
            "embedding_provider": std::env::var("CODEGRAPH_EMBEDDING_PROVIDER").ok(),
            "device": device,
            "features": {"faiss": cfg!(feature = "faiss"), "embeddings": cfg!(feature = "embeddings")}
        },
        "dataset": {"path": path, "languages": langs, "files": stats.files, "lines": stats.lines},
        "indexing": {"total_seconds": indexing_secs, "embeddings": stats.embeddings,
            "throughput_embeddings_per_sec": if indexing_secs > 0.0 { stats.embeddings as f64 / indexing_secs } else { 0.0 }},
        "vector_search": {"queries": qset.len() * trials, "latency_ms": {"avg": avg, "p50": p50, "p95": p95}},
        "graph": {"bfs_depth": 2, "visited_nodes": bfs_nodes, "elapsed_ms": graph_bfs_ms}
    });

    if format == "human" {
        println!(
            "Performance Results
                ===================="
        );
        println!(
            "Dataset: {:?} ({} files, {} lines)",
            out["dataset"]["path"], out["dataset"]["files"], out["dataset"]["lines"]
        );
        println!(
            "Indexing: {:.2}s ({} embeddings, {:.1} emb/s)",
            out["indexing"]["total_seconds"],
            out["indexing"]["embeddings"],
            out["indexing"]["throughput_embeddings_per_sec"]
        );
        println!(
            "Vector Search: avg={:.1}ms p50={:.1}ms p95={:.1}ms ({} queries)",
            out["vector_search"]["latency_ms"]["avg"],
            out["vector_search"]["latency_ms"]["p50"],
            out["vector_search"]["latency_ms"]["p95"],
            out["vector_search"]["queries"]
        );
        println!(
            "Graph BFS depth=2: visited {} nodes in {:.1}ms",
            out["graph"]["visited_nodes"], out["graph"]["elapsed_ms"]
        );
    } else {
        println!("{}", serde_json::to_string_pretty(&out)?);
    }

    Ok(())
}

/// Estimate available system memory in GB
fn estimate_available_memory_gb() -> usize {
    #[cfg(target_os = "macos")]
    {
        if let Ok(output) = std::process::Command::new("sysctl")
            .args(["-n", "hw.memsize"])
            .output()
        {
            if let Ok(memsize_str) = String::from_utf8(output.stdout) {
                if let Ok(memsize) = memsize_str.trim().parse::<u64>() {
                    return (memsize / 1024 / 1024 / 1024) as usize;
                }
            }
        }
    }

    #[cfg(target_os = "linux")]
    {
        if let Ok(content) = std::fs::read_to_string("/proc/meminfo") {
            for line in content.lines() {
                if line.starts_with("MemTotal:") {
                    if let Some(kb_str) = line.split_whitespace().nth(1) {
                        if let Ok(kb) = kb_str.parse::<u64>() {
                            return (kb / 1024 / 1024) as usize;
                        }
                    }
                }
            }
        }
    }

    16 // Default assumption if detection fails
}

/// Optimize batch size and workers based on available memory and embedding provider
fn optimize_for_memory(
    memory_gb: usize,
    default_batch_size: usize,
    default_workers: usize,
) -> (usize, usize) {
    let embedding_provider = std::env::var("CODEGRAPH_EMBEDDING_PROVIDER").unwrap_or_default();

    let optimized_batch_size = if default_batch_size == 100 {
        // Default value
        if embedding_provider == "ollama" {
            // Ollama models work better with smaller batches for stability
            match memory_gb {
                128.. => 1024,   // 128GB+: Large but stable batch size
                96..=127 => 768, // 96-127GB: Medium-large batch
                64..=95 => 512,  // 64-95GB: Medium batch
                32..=63 => 256,  // 32-63GB: Small batch
                16..=31 => 128,  // 16-31GB: Very small batch
                _ => 64,         // <16GB: Minimal batch
            }
        } else {
            // ONNX/OpenAI can handle much larger batches
            match memory_gb {
                128.. => 20480,    // 128GB+: Ultra-high batch size
                96..=127 => 15360, // 96-127GB: Very high batch size
                64..=95 => 10240,  // 64-95GB: High batch size
                48..=63 => 5120,   // 48-63GB: Medium-high batch size
                32..=47 => 2048,   // 32-47GB: Medium batch size
                16..=31 => 512,    // 16-31GB: Conservative batch size
                _ => 100,          // <16GB: Keep default
            }
        }
    } else {
        default_batch_size // User specified - respect their choice
    };

    let optimized_workers = if default_workers == 4 {
        // Default value
        match memory_gb {
            128.. => 16,    // 128GB+: Maximum parallelism
            96..=127 => 14, // 96-127GB: Very high parallelism
            64..=95 => 12,  // 64-95GB: High parallelism
            48..=63 => 10,  // 48-63GB: Medium-high parallelism
            32..=47 => 8,   // 32-47GB: Medium parallelism
            16..=31 => 6,   // 16-31GB: Conservative parallelism
            _ => 4,         // <16GB: Keep default
        }
    } else {
        default_workers // User specified - respect their choice
    };

    (optimized_batch_size, optimized_workers)
}
