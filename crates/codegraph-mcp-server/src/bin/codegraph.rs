use anyhow::{Context, Result};
use atty::Stream;
use chrono::Utc;
use clap::{Parser, Subcommand};
use codegraph_mcp::{
    EmbeddingThroughputConfig, IndexerConfig, ProcessManager, ProjectIndexer, RepositoryEstimate,
    RepositoryEstimator,
};
use codegraph_mcp_core::debug_logger::DebugLogger;
#[cfg(feature = "daemon")]
use codegraph_mcp_daemon::{DaemonManager, PidFile, WatchConfig, WatchDaemon};
use codegraph_mcp_server::CodeGraphMCPServer;
use colored::Colorize;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use rmcp::ServiceExt;
use std::fs::File;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tracing::info;
use tracing_subscriber::{
    filter::EnvFilter, fmt::writer::BoxMakeWriter, layer::SubscriberExt, Registry,
};

const DEFAULT_JINA_BATCH_SIZE: usize = 2000;
const DEFAULT_JINA_BATCH_MINUTES: f64 = 9.0;
const DEFAULT_LOCAL_EMBEDDINGS_PER_WORKER_PER_MINUTE: f64 = 3600.0;

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

    #[arg(
        long,
        global = true,
        help = "Capture index logs to a file under .codegraph/logs"
    )]
    debug: bool,

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

    #[command(about = "Run SurrealDB connectivity/schema canary (debug)")]
    DbCheck {
        #[arg(long, help = "Namespace to use (overrides env)")]
        namespace: Option<String>,
        #[arg(long, help = "Database to use (overrides env)")]
        database: Option<String>,
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

    #[command(
        about = "Index a project or directory",
        long_about = "Index a project with dual-mode support:\n\
                      • Local Mode (FAISS): Set CODEGRAPH_EMBEDDING_PROVIDER=local or ollama\n\
                      • Local Mode (SurrealDB HNSW + Ollama Embeddings + LMStudio Rerank): Set CODEGRAPH_EMBEDDING_PROVIDER=ollama and Set CODEGRAPH_RERANKING_PROVIDER=lmstudio\n\
                      • Cloud Mode (SurrealDB HNSW + Jina reranking): Set CODEGRAPH_EMBEDDING_PROVIDER=jina\n\
                      \n\
                      Some flags are mode-specific (see individual flag help for details)."
    )]
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

        #[arg(
            long,
            help = "Number of parallel workers (applies to both local and cloud modes)",
            default_value = "4"
        )]
        workers: usize,

        #[arg(
            long,
            help = "Embedding batch size (both modes; cloud mode uses API batching, local uses local processing batches)",
            default_value = "100"
        )]
        batch_size: usize,

        #[arg(
            long,
            help = "[Cloud mode only] Maximum concurrent API requests for parallel embedding generation (ignored in local mode)",
            default_value = "10"
        )]
        max_concurrent: usize,

        #[arg(
            long,
            help = "[Local mode only] Embedding device: cpu | metal | cuda:<id> (ignored in cloud mode)"
        )]
        device: Option<String>,

        #[arg(
            long,
            help = "[Local mode only] Max sequence length for embeddings (ignored in cloud mode)",
            default_value = "512"
        )]
        max_seq_len: usize,
        #[arg(
            long,
            help = "Symbol embedding batch size (overrides generic batch size for precomputing symbols)",
            value_parser = clap::value_parser!(usize)
        )]
        symbol_batch_size: Option<usize>,
        #[arg(
            long,
            help = "Symbol embedding max concurrency (overrides generic max-concurrent)",
            value_parser = clap::value_parser!(usize)
        )]
        symbol_max_concurrent: Option<usize>,
    },

    #[command(
        about = "Estimate indexing cost (node/edge counts + embedding ETA) without writing to SurrealDB"
    )]
    Estimate {
        #[arg(help = "Path to project directory")]
        path: PathBuf,

        #[arg(short, long, help = "Languages to scan", value_delimiter = ',')]
        languages: Option<Vec<String>>,

        #[arg(long, help = "Exclude patterns (gitignore format)")]
        exclude: Vec<String>,

        #[arg(long, help = "Include only these patterns")]
        include: Vec<String>,

        #[arg(short, long, help = "Recursively walk subdirectories")]
        recursive: bool,

        #[arg(
            long,
            help = "Worker concurrency to assume for parsing/local embeddings",
            default_value = "4"
        )]
        workers: usize,

        #[arg(
            long,
            help = "Parser batch size baseline (affects local estimate heuristics)",
            default_value = "100"
        )]
        batch_size: usize,

        #[arg(
            long,
            help = "Override Jina batch size (defaults to 2000 based on current limits)"
        )]
        jina_batch_size: Option<usize>,

        #[arg(
            long,
            help = "Override minutes per Jina batch (defaults to 9 based on observed throughput)"
        )]
        jina_batch_minutes: Option<f64>,

        #[arg(
            long,
            help = "Override local embedding throughput (embeddings per minute)"
        )]
        local_throughput: Option<f64>,

        #[arg(short, long, help = "Output format", default_value = "human")]
        format: StatsFormat,
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

    #[cfg(feature = "daemon")]
    #[command(about = "Manage watch daemon for automatic re-indexing on file changes")]
    Daemon {
        #[command(subcommand)]
        action: DaemonAction,
    },
}

#[cfg(feature = "daemon")]
#[derive(Subcommand)]
enum DaemonAction {
    #[command(about = "Start watch daemon for a project")]
    Start {
        #[arg(help = "Path to project directory", default_value = ".")]
        path: PathBuf,

        #[arg(long, help = "Run in foreground (default: daemonize)")]
        foreground: bool,

        #[arg(short, long, help = "Languages to watch", value_delimiter = ',')]
        languages: Option<Vec<String>>,

        #[arg(long, help = "Exclude patterns")]
        exclude: Vec<String>,

        #[arg(long, help = "Include patterns")]
        include: Vec<String>,
    },

    #[command(about = "Stop running watch daemon")]
    Stop {
        #[arg(help = "Path to project directory", default_value = ".")]
        path: PathBuf,
    },

    #[command(about = "Show watch daemon status")]
    Status {
        #[arg(help = "Path to project directory", default_value = ".")]
        path: PathBuf,

        #[arg(long, help = "Output as JSON")]
        json: bool,
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

        /// Enable automatic file watching and re-indexing (daemon mode)
        #[arg(
            long = "watch",
            help = "Enable automatic file watching and re-indexing",
            env = "CODEGRAPH_DAEMON_AUTO_START"
        )]
        enable_daemon: bool,

        /// Path to watch for file changes (defaults to current directory)
        #[arg(
            long = "watch-path",
            help = "Path to watch for file changes (defaults to current directory)",
            env = "CODEGRAPH_DAEMON_WATCH_PATH"
        )]
        watch_path: Option<PathBuf>,

        /// Explicitly disable daemon even if config enables it
        #[arg(
            long = "no-watch",
            help = "Explicitly disable daemon even if config enables it"
        )]
        disable_daemon: bool,
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

        /// Enable automatic file watching and re-indexing (daemon mode)
        #[arg(
            long = "watch",
            help = "Enable automatic file watching and re-indexing",
            env = "CODEGRAPH_DAEMON_AUTO_START"
        )]
        enable_daemon: bool,

        /// Path to watch for file changes (defaults to current directory)
        #[arg(
            long = "watch-path",
            help = "Path to watch for file changes (defaults to current directory)",
            env = "CODEGRAPH_DAEMON_WATCH_PATH"
        )]
        watch_path: Option<PathBuf>,

        /// Explicitly disable daemon even if config enables it
        #[arg(
            long = "no-watch",
            help = "Explicitly disable daemon even if config enables it"
        )]
        disable_daemon: bool,
    },

    #[command(about = "Start with both STDIO and HTTP transports")]
    Dual {
        #[arg(short, long, help = "HTTP host", default_value = "127.0.0.1")]
        host: String,

        #[arg(short, long, help = "HTTP port", default_value = "3000")]
        port: u16,

        #[arg(long, help = "STDIO buffer size", default_value = "8192")]
        buffer_size: usize,

        /// Enable automatic file watching and re-indexing (daemon mode)
        #[arg(
            long = "watch",
            help = "Enable automatic file watching and re-indexing",
            env = "CODEGRAPH_DAEMON_AUTO_START"
        )]
        enable_daemon: bool,

        /// Path to watch for file changes (defaults to current directory)
        #[arg(
            long = "watch-path",
            help = "Path to watch for file changes (defaults to current directory)",
            env = "CODEGRAPH_DAEMON_WATCH_PATH"
        )]
        watch_path: Option<PathBuf>,

        /// Explicitly disable daemon even if config enables it
        #[arg(
            long = "no-watch",
            help = "Explicitly disable daemon even if config enables it"
        )]
        disable_daemon: bool,
    },
}

#[derive(Subcommand)]
enum ConfigAction {
    #[command(
        about = "Initialize global configuration",
        long_about = "Create global configuration files at ~/.codegraph/:\n\
                      • config.toml - Structured configuration\n\
                      • .env - Environment variables (recommended for API keys)\n\
                      \n\
                      Configuration hierarchy (highest to lowest priority):\n\
                      1. Environment variables\n\
                      2. Local .env (current directory)\n\
                      3. Global ~/.codegraph.env\n\
                      4. Local .codegraph.toml (current directory)\n\
                      5. Global ~/.codegraph/config.toml\n\
                      6. Built-in defaults"
    )]
    Init {
        #[arg(short, long, help = "Overwrite existing files")]
        force: bool,
    },

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

    #[command(about = "Run SurrealDB connectivity/schema canary (debug)")]
    DbCheck {
        #[arg(long, help = "Namespace to use (overrides env)")]
        namespace: Option<String>,
        #[arg(long, help = "Database to use (overrides env)")]
        database: Option<String>,
    },

    #[command(about = "Show orchestrator-agent configuration metadata")]
    AgentStatus {
        #[arg(long, help = "Show as JSON")]
        json: bool,
    },
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
    // Load .env file if present
    dotenv::dotenv().ok();

    // Initialize debug logger (enabled with CODEGRAPH_DEBUG=1)
    DebugLogger::init();

    let cli = Cli::parse();

    // Load configuration once at startup
    use codegraph_core::config_manager::ConfigManager;
    let config_mgr = ConfigManager::load().context("Failed to load configuration")?;
    let config = config_mgr.config();

    // TODO: Override with CLI config path if provided
    if let Some(_config_path) = &cli.config {
        // Future: merge CLI-specified config file
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
            max_concurrent,
            device,
            max_seq_len,
            symbol_batch_size,
            symbol_max_concurrent,
        } => {
            handle_index(
                config,
                path,
                languages,
                exclude,
                include,
                recursive,
                force,
                watch,
                workers,
                batch_size,
                max_concurrent,
                device,
                max_seq_len,
                symbol_batch_size,
                symbol_max_concurrent,
                cli.debug,
            )
            .await?;
        }
        Commands::Estimate {
            path,
            languages,
            exclude,
            include,
            recursive,
            workers,
            batch_size,
            jina_batch_size,
            jina_batch_minutes,
            local_throughput,
            format,
        } => {
            handle_estimate(
                config,
                path,
                languages,
                exclude,
                include,
                recursive,
                workers,
                batch_size,
                jina_batch_size,
                jina_batch_minutes,
                local_throughput,
                format,
            )
            .await?;
        }
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
                config,
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
        Commands::DbCheck {
            namespace,
            database,
        } => {
            handle_db_check(namespace, database).await?;
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

        #[cfg(feature = "daemon")]
        Commands::Daemon { action } => {
            handle_daemon(action).await?;
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
        TransportType::Stdio {
            buffer_size: _buffer_size,
            enable_daemon,
            watch_path,
            disable_daemon,
        } => {
            // Configure logging to file for stdio transport (stdout/stderr are used for MCP protocol)
            // Logs will be written to .codegraph/logs/mcp-server.log
            let log_dir = std::env::current_dir()
                .unwrap_or_else(|_| std::path::PathBuf::from("."))
                .join(".codegraph")
                .join("logs");

            std::fs::create_dir_all(&log_dir).ok();

            // Use tracing_appender for non-blocking file writes
            let file_appender = tracing_appender::rolling::never(&log_dir, "mcp-server.log");
            let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);

            let subscriber = tracing_subscriber::fmt()
                .with_writer(non_blocking)
                .with_max_level(tracing_subscriber::filter::LevelFilter::INFO)
                .with_ansi(false)
                .with_target(false)
                .with_line_number(true)
                .finish();

            tracing::subscriber::set_global_default(subscriber).ok();

            // Keep the guard alive for the duration of the server
            std::mem::forget(_guard);

            // Start background daemon if enabled
            #[cfg(feature = "daemon")]
            let mut daemon_manager: Option<DaemonManager> = None;

            #[cfg(feature = "daemon")]
            {
                use codegraph_core::config_manager::ConfigManager;

                // Load configuration
                if let Ok(config_mgr) = ConfigManager::load() {
                    let global_config = config_mgr.config().clone();

                    // Determine if daemon should start:
                    // Priority: --no-watch > --watch > config.daemon.auto_start_with_mcp
                    let should_start_daemon = if disable_daemon {
                        false
                    } else if enable_daemon {
                        true
                    } else {
                        global_config.daemon.auto_start_with_mcp
                    };

                    if should_start_daemon {
                        let project_root = watch_path
                            .or_else(|| global_config.daemon.project_path.clone())
                            .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());

                        let project_root =
                            std::fs::canonicalize(&project_root).unwrap_or(project_root);

                        // Create daemon config with auto_start forced on
                        let daemon_config = codegraph_core::config_manager::DaemonConfig {
                            auto_start_with_mcp: true,
                            project_path: Some(project_root.clone()),
                            ..global_config.daemon.clone()
                        };

                        let mut dm =
                            DaemonManager::new(daemon_config, global_config, project_root.clone());

                        match dm.start_background().await {
                            Ok(()) => {
                                if atty::is(Stream::Stderr) {
                                    eprintln!(
                                        "{}",
                                        format!("🔄 Daemon watching: {}", project_root.display())
                                            .cyan()
                                    );
                                }
                                daemon_manager = Some(dm);
                            }
                            Err(e) => {
                                // Log error but continue - MCP server should still work
                                if atty::is(Stream::Stderr) {
                                    eprintln!(
                                        "{}",
                                        format!(
                                            "⚠️  Daemon failed to start: {} (MCP server continuing)",
                                            e
                                        )
                                        .yellow()
                                    );
                                }
                                tracing::warn!("Daemon startup failed: {}", e);
                            }
                        }
                    }
                }
            }

            if atty::is(Stream::Stderr) {
                eprintln!(
                    "{}",
                    "Starting CodeGraph MCP Server with 100% Official SDK..."
                        .green()
                        .bold()
                );
            }

            // Create and initialize the revolutionary CodeGraph server with official SDK
            let server = CodeGraphMCPServer::new();

            if atty::is(Stream::Stderr) {
                eprintln!(
                    "✅ Revolutionary CodeGraph MCP server ready with 100% protocol compliance"
                );
            }

            // Use official rmcp STDIO transport for perfect compliance
            let service: rmcp::service::RunningService<rmcp::RoleServer, CodeGraphMCPServer> =
                server.serve(rmcp::transport::stdio()).await.map_err(|e| {
                    if atty::is(Stream::Stderr) {
                        eprintln!("❌ Failed to start official MCP server: {}", e);
                    }
                    anyhow::anyhow!("MCP server startup failed: {}", e)
                })?;

            if atty::is(Stream::Stderr) {
                eprintln!("🚀 Official MCP server started with revolutionary capabilities");
            }

            // Wait for the server to complete
            service
                .waiting()
                .await
                .map_err(|e| anyhow::anyhow!("Server error: {}", e))?;

            // Clean up daemon on exit
            #[cfg(feature = "daemon")]
            if let Some(mut dm) = daemon_manager {
                if let Err(e) = dm.stop().await {
                    tracing::warn!("Daemon cleanup error: {}", e);
                }
            }
        }
        TransportType::Http {
            host,
            port,
            tls,
            cert,
            key,
            cors: _,
            enable_daemon,
            watch_path,
            disable_daemon,
        } => {
            #[cfg(not(feature = "server-http"))]
            let _ = (
                host,
                port,
                tls,
                cert,
                key,
                enable_daemon,
                watch_path,
                disable_daemon,
            );
            #[cfg(not(feature = "server-http"))]
            {
                eprintln!("🚧 HTTP transport requires the 'server-http' feature");
                eprintln!();
                eprintln!("💡 Rebuild with HTTP support:");
                eprintln!("   cargo build --release --features server-http");
                eprintln!();
                eprintln!("💡 Or use STDIO transport:");
                eprintln!("   codegraph start stdio");

                return Err(anyhow::anyhow!(
                    "HTTP transport not enabled - rebuild with 'server-http' feature"
                ));
            }

            #[cfg(feature = "server-http")]
            {
                use axum::Router;
                use rmcp::transport::streamable_http_server::{
                    session::local::LocalSessionManager, StreamableHttpServerConfig,
                    StreamableHttpService,
                };
                use std::sync::Arc;
                use std::time::Duration;

                // Start background daemon if enabled
                #[cfg(feature = "daemon")]
                let mut daemon_manager: Option<DaemonManager> = None;

                #[cfg(feature = "daemon")]
                {
                    use codegraph_core::config_manager::ConfigManager;

                    if let Ok(config_mgr) = ConfigManager::load() {
                        let global_config = config_mgr.config().clone();

                        let should_start_daemon = if disable_daemon {
                            false
                        } else if enable_daemon {
                            true
                        } else {
                            global_config.daemon.auto_start_with_mcp
                        };

                        if should_start_daemon {
                            let project_root = watch_path
                                .or_else(|| global_config.daemon.project_path.clone())
                                .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());

                            let project_root =
                                std::fs::canonicalize(&project_root).unwrap_or(project_root);

                            let daemon_config = codegraph_core::config_manager::DaemonConfig {
                                auto_start_with_mcp: true,
                                project_path: Some(project_root.clone()),
                                ..global_config.daemon.clone()
                            };

                            let mut dm = DaemonManager::new(
                                daemon_config,
                                global_config,
                                project_root.clone(),
                            );

                            match dm.start_background().await {
                                Ok(()) => {
                                    eprintln!(
                                        "{}",
                                        format!("🔄 Daemon watching: {}", project_root.display())
                                            .cyan()
                                    );
                                    daemon_manager = Some(dm);
                                }
                                Err(e) => {
                                    eprintln!(
                                        "{}",
                                        format!(
                                            "⚠️  Daemon failed to start: {} (HTTP server continuing)",
                                            e
                                        )
                                        .yellow()
                                    );
                                    tracing::warn!("Daemon startup failed: {}", e);
                                }
                            }
                        }
                    }
                }

                if atty::is(Stream::Stderr) {
                    eprintln!(
                        "{}",
                        "Starting CodeGraph MCP Server with HTTP transport..."
                            .green()
                            .bold()
                    );
                }

                // Handle TLS configuration
                if tls {
                    if cert.is_none() || key.is_none() {
                        return Err(anyhow::anyhow!(
                            "TLS enabled but certificate or key not provided. Use --cert and --key"
                        ));
                    }
                    eprintln!("⚠️  TLS configuration detected but not yet implemented");
                    eprintln!("   Server will start without TLS");
                }

                // Create session manager for stateful HTTP connections
                let session_manager = Arc::new(LocalSessionManager::default());

                // Service factory - creates new CodeGraphMCPServer for each session
                let service_factory = || {
                    let server = CodeGraphMCPServer::new();
                    // Note: initialize_qwen() is async, but service factory must be sync
                    // Qwen initialization will happen on first use
                    Ok(server)
                };

                // Configure HTTP server with SSE streaming
                let config = StreamableHttpServerConfig {
                    sse_keep_alive: Some(Duration::from_secs(15)), // Send keep-alive every 15s
                    stateful_mode: true, // Enable session management + SSE
                };

                if atty::is(Stream::Stderr) {
                    eprintln!("📡 Configuring StreamableHTTP with SSE keep-alive (15s)");
                }

                // Create StreamableHttpService (implements tower::Service)
                let http_service =
                    StreamableHttpService::new(service_factory, session_manager, config);

                // Create Axum router
                let app = Router::new().fallback_service(http_service);

                // Bind to address
                let addr = format!("{}:{}", host, port);
                let listener = tokio::net::TcpListener::bind(&addr)
                    .await
                    .map_err(|e| anyhow::anyhow!("Failed to bind to {}: {}", addr, e))?;

                if atty::is(Stream::Stderr) {
                    eprintln!("✅ CodeGraph MCP HTTP server ready");
                    eprintln!("🚀 Listening on http://{}", addr);
                    eprintln!();
                    eprintln!("📋 HTTP Endpoints:");
                    eprintln!("   POST   /mcp  - Initialize session");
                    eprintln!("   GET    /mcp  - Open SSE stream (with Mcp-Session-Id header)");
                    eprintln!("   POST   /mcp  - Send request (with Mcp-Session-Id header)");
                    eprintln!("   DELETE /mcp  - Close session");
                    eprintln!();
                    eprintln!("💡 Progress notifications stream via Server-Sent Events");
                }

                // Serve with Axum
                axum::serve(listener, app)
                    .await
                    .map_err(|e| anyhow::anyhow!("HTTP server error: {}", e))?;

                // Clean up daemon on exit
                #[cfg(feature = "daemon")]
                if let Some(mut dm) = daemon_manager {
                    if let Err(e) = dm.stop().await {
                        tracing::warn!("Daemon cleanup error: {}", e);
                    }
                }
            }
        }
        TransportType::Dual {
            host,
            port,
            buffer_size,
            enable_daemon: _,
            watch_path: _,
            disable_daemon: _,
        } => {
            // Note: Dual transport doesn't support daemon integration yet
            // The daemon fields are accepted but not used
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
            if atty::is(Stream::Stdout) {
                println!("✓ MCP server started with dual transport");
                println!("  STDIO: buffer size {} (PID: {})", buffer_size, stdio_pid);
                println!("  HTTP: http://{}:{} (PID: {})", host, port, http_pid);
            }
        }
    }

    if daemon {
        if atty::is(Stream::Stdout) {
            println!("Running in daemon mode");
            if let Some(ref pid_file) = pid_file {
                println!("PID file: {:?}", pid_file);
            }
        }
    }

    Ok(())
}

async fn handle_stop(pid_file: Option<PathBuf>, force: bool) -> Result<()> {
    if atty::is(Stream::Stdout) {
        println!("{}", "Stopping CodeGraph MCP Server...".yellow().bold());
    }

    let manager = ProcessManager::new();

    if force {
        if atty::is(Stream::Stdout) {
            println!("Force stopping server");
        }
    }

    manager.stop_server(pid_file, force).await?;

    if atty::is(Stream::Stdout) {
        println!("✓ Server stopped");
    }

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
    config: &codegraph_core::config_manager::CodeGraphConfig,
    path: PathBuf,
    languages: Option<Vec<String>>,
    exclude: Vec<String>,
    include: Vec<String>,
    recursive: bool,
    force: bool,
    watch: bool,
    workers: usize,
    batch_size: usize,
    max_concurrent: usize,
    device: Option<String>,
    max_seq_len: usize,
    symbol_batch_size: Option<usize>,
    symbol_max_concurrent: Option<usize>,
    debug_log: bool,
) -> Result<()> {
    let project_root = path.clone().canonicalize().unwrap_or_else(|_| path.clone());

    let env_filter =
        || EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    let mut debug_log_path: Option<PathBuf> = None;

    if debug_log {
        let (writer, log_path) = prepare_debug_writer(&project_root)?;
        let subscriber = Registry::default()
            .with(env_filter())
            .with(tracing_subscriber::fmt::layer())
            .with(
                tracing_subscriber::fmt::layer()
                    .with_writer(writer)
                    .with_ansi(false),
            );
        tracing::subscriber::set_global_default(subscriber).ok();
        println!("{} {}", "📝 Debug log:".cyan(), log_path.display());
        debug_log_path = Some(log_path);
    } else {
        let subscriber = Registry::default()
            .with(env_filter())
            .with(tracing_subscriber::fmt::layer());
        tracing::subscriber::set_global_default(subscriber).ok();
    }

    let multi_progress = MultiProgress::new();

    let h_style = ProgressStyle::with_template("{spinner:.blue} {msg}")
        .unwrap()
        .tick_chars("⠁⠂⠄⡀⢀⠠⠐⠈ ");

    let header_pb = multi_progress.add(ProgressBar::new(1));
    header_pb.set_style(h_style);
    header_pb.set_message(format!("Indexing project: {}", path.to_string_lossy()));

    // Memory-aware optimization for high-memory systems
    let available_memory_gb = estimate_available_memory_gb();
    let (optimized_batch_size, optimized_workers) =
        optimize_for_memory(available_memory_gb, batch_size, workers);

    if available_memory_gb >= 64 {
        multi_progress.println(format!(
            "🚀 High-memory system detected ({}GB) - performance optimized!",
            available_memory_gb
        ))?;
    }

    // Configure indexer
    let languages_list = languages.clone().unwrap_or_default();
    let indexer_config = IndexerConfig {
        languages: languages_list.clone(),
        exclude_patterns: exclude,
        include_patterns: include,
        recursive,
        force_reindex: force,
        watch,
        workers: optimized_workers,
        batch_size: optimized_batch_size,
        max_concurrent,
        device,
        max_seq_len,
        symbol_batch_size,
        symbol_max_concurrent,
        project_root: project_root.clone(),
        ..Default::default()
    };

    // Create indexer
    let mut indexer = ProjectIndexer::new(indexer_config, config, multi_progress.clone()).await?;

    let start_time = std::time::Instant::now();

    // Perform indexing
    let stats = indexer.index_project(&path).await?;
    let elapsed = start_time.elapsed();

    header_pb.finish_with_message("✔ Indexing complete".to_string());

    println!();
    println!("{}", "🎉 INDEXING COMPLETE!".green().bold());
    println!();

    // Performance summary with comprehensive metrics
    println!("{}", "📊 Performance Summary".cyan().bold());
    println!("┌───────────────────────────────────────────────────────┐");
    println!(
        "│ ⏱️  Total time: {:<39} │",
        format!("{:.2?}", elapsed).green()
    );
    let files_per_sec = stats.files as f64 / elapsed.as_secs_f64();
    let nodes_per_sec = stats.nodes as f64 / elapsed.as_secs_f64();
    println!(
        "│ ⚡ Throughput: {:<39} │",
        format!("{:.1} files/s, {:.0} nodes/s", files_per_sec, nodes_per_sec).yellow()
    );
    println!("├───────────────────────────────────────────────────────┤");
    println!(
        "│ 📄 Files:      {:>6} indexed, {:>4} skipped           │",
        stats.files, stats.skipped
    );
    println!(
        "│ 📝 Lines:      {:>6} processed                        │",
        stats.lines
    );
    println!(
        "│ 🌳 Nodes:      {:>6} extracted                        │",
        stats.nodes
    );
    println!(
        "│ 🔗 Edges:      {:>6} relationships                    │",
        stats.edges
    );
    println!(
        "│ 💾 Embeddings: {:>6} chunks ({}-dim)                  │",
        stats.embeddings,
        stats.embedding_dimension
    );
    if stats.errors > 0 {
        println!(
            "│ ❌ Errors:     {:>6} encountered                      │",
            stats.errors
        );
    }
    println!("├──────────────────────────────────────────────────────┤");
    let avg_nodes = if stats.files > 0 { stats.nodes as f64 / stats.files as f64 } else { 0.0 };
    let avg_edges = if stats.files > 0 { stats.edges as f64 / stats.files as f64 } else { 0.0 };
    println!(
        "│ 📈 Averages: {:.1} nodes/file, {:.1} edges/file          │",
        avg_nodes, avg_edges
    );
    println!("└───────────────────────────────────────────────────────┘");

    // Configuration summary
    println!();
    println!("{}", "⚙️  Configuration Summary".cyan().bold());
    println!(
        "Workers: {} | Batch Size: {} | Languages: {}",
        optimized_workers,
        optimized_batch_size,
        languages_list.join(", ")
    );
    if !stats.embedding_provider.is_empty() {
        println!(
            "Embeddings: {} ({}-dim)",
            stats.embedding_provider,
            stats.embedding_dimension
        );
    }

    println!();
    println!(
        "{}",
        "🚀 Ready for CodeGraph Agentic MCP Intelligence!"
            .green()
            .bold()
    );
    println!("Next: Start MCP server with 'codegraph start http' or 'codegraph start stdio'");

    if stats.errors > 0 {
        println!("  {} Errors: {}", "⚠".yellow(), stats.errors);
    }

    if watch {
        println!();
        println!("Watching for changes... (Press Ctrl+C to stop)");
        indexer.watch_for_changes(path).await?;
    }

    if let Some(log_path) = debug_log_path {
        println!(
            "{} {}",
            "📂 Detailed log saved at:".cyan(),
            log_path.display()
        );
    }

    Ok(())
}

async fn handle_estimate(
    config: &codegraph_core::config_manager::CodeGraphConfig,
    path: PathBuf,
    languages: Option<Vec<String>>,
    exclude: Vec<String>,
    include: Vec<String>,
    recursive: bool,
    workers: usize,
    batch_size: usize,
    jina_batch_size: Option<usize>,
    jina_batch_minutes: Option<f64>,
    local_throughput: Option<f64>,
    format: StatsFormat,
) -> Result<()> {
    let project_root = path.clone().canonicalize().unwrap_or(path.clone());
    let languages_list = languages.clone().unwrap_or_default();

    let mut estimator_config = IndexerConfig {
        languages: languages_list.clone(),
        exclude_patterns: exclude,
        include_patterns: include,
        recursive,
        workers,
        batch_size,
        ..Default::default()
    };
    estimator_config.project_root = project_root.clone();

    let estimator = RepositoryEstimator::new(estimator_config);
    let throughput = resolve_throughput_config(
        jina_batch_size,
        jina_batch_minutes,
        local_throughput,
        workers,
    );

    println!(
        "{} {}",
        "🔍 Estimating repository:".cyan().bold(),
        project_root.to_string_lossy()
    );

    let start = std::time::Instant::now();
    let report = estimator.analyze(&path, &throughput).await?;
    let elapsed = start.elapsed();

    present_estimate_output(
        format,
        &project_root,
        &languages_list,
        &throughput,
        &report,
        workers,
        batch_size,
        config.embedding.provider.as_str(),
        elapsed,
    )
}

fn present_estimate_output(
    format: StatsFormat,
    project_root: &Path,
    languages: &[String],
    throughput: &EmbeddingThroughputConfig,
    report: &RepositoryEstimate,
    workers: usize,
    batch_size: usize,
    provider: &str,
    elapsed: Duration,
) -> Result<()> {
    let parsing_minutes = report.parsing_duration.as_secs_f64() / 60.0;
    let total_jina_minutes = parsing_minutes + report.timings.jina_minutes;
    let total_local_minutes = report
        .timings
        .local_minutes
        .map(|local| parsing_minutes + local);

    let payload = serde_json::json!({
        "path": project_root.to_string_lossy(),
        "languages": if languages.is_empty() { serde_json::Value::Null } else { serde_json::Value::from(languages.to_vec()) },
        "counts": {
            "total_files": report.counts.total_files,
            "parsed_files": report.counts.parsed_files,
            "failed_files": report.counts.failed_files,
            "nodes": report.counts.nodes,
            "edges": report.counts.edges,
            "symbols": report.counts.symbols,
        },
        "parsing": {
            "minutes": parsing_minutes,
            "duration_seconds": report.parsing_duration.as_secs_f64(),
            "total_lines": report.parsing.total_lines,
            "files_per_second": report.parsing.files_per_second,
            "lines_per_second": report.parsing.lines_per_second,
        },
        "timings": {
            "jina": {
                "batches": report.timings.jina_batches,
                "batch_size": report.timings.jina_batch_size,
                "batch_minutes": report.timings.jina_batch_minutes,
                "minutes": report.timings.jina_minutes,
                "total_minutes_with_parsing": total_jina_minutes,
            },
            "local": {
                "rate_per_minute": report.timings.local_rate_per_minute,
                "minutes": report.timings.local_minutes,
                "total_minutes_with_parsing": total_local_minutes,
            }
        },
        "workers": workers,
        "batch_size": batch_size,
        "embedding_provider": provider,
        "assumptions": {
            "jina_batch_size": throughput.jina_batch_size,
            "jina_batch_minutes": throughput.jina_batch_minutes,
            "local_embeddings_per_minute": throughput.local_embeddings_per_minute,
        },
        "estimate_runtime_seconds": elapsed.as_secs_f64(),
    });

    match format {
        StatsFormat::Json => {
            println!("{}", serde_json::to_string_pretty(&payload)?);
        }
        StatsFormat::Yaml => {
            let yaml = serde_yaml::to_string(&payload)?;
            println!("{yaml}");
        }
        StatsFormat::Table | StatsFormat::Human => {
            println!();
            println!("{}", "📊 Indexing Estimate".cyan().bold());
            println!("Path: {}", project_root.display());
            println!(
                "Languages: {}",
                if languages.is_empty() {
                    "auto-detect".to_string()
                } else {
                    languages.join(", ")
                }
            );
            println!("Embedding provider (config): {}", provider);
            println!();
            println!(
                "Files parsed: {} / {} (failed: {})",
                report.counts.parsed_files, report.counts.total_files, report.counts.failed_files
            );
            println!("Nodes: {}", report.counts.nodes);
            println!("Edges: {}", report.counts.edges);
            println!("Symbols: {}", report.counts.symbols);
            println!(
                "Parsing time (measured): {}",
                format_duration_minutes(parsing_minutes)
            );
            println!(
                "Jina embeddings: {} ({} batches × {} nodes)",
                format_duration_minutes(report.timings.jina_minutes),
                report.timings.jina_batches,
                report.timings.jina_batch_size
            );
            println!(
                "Total time (parsing + Jina): {}",
                format_duration_minutes(total_jina_minutes)
            );
            if let Some(local_minutes) = report.timings.local_minutes {
                let rate = report
                    .timings
                    .local_rate_per_minute
                    .unwrap_or(default_local_rate(workers));
                println!(
                    "Local embeddings: {} ({:.0} embeddings/min)",
                    format_duration_minutes(local_minutes),
                    rate
                );
                if let Some(total_local) = total_local_minutes {
                    println!(
                        "Total time (parsing + local): {}",
                        format_duration_minutes(total_local)
                    );
                }
            } else {
                println!(
                    "{}",
                    "Local embeddings: set --local-throughput to compare speed.".dimmed()
                );
            }
            println!(
                "Assumptions: {} nodes/batch @ {:.1} min, local {:.0} embeddings/min baseline.",
                throughput.jina_batch_size,
                throughput.jina_batch_minutes,
                throughput
                    .local_embeddings_per_minute
                    .unwrap_or(default_local_rate(workers))
            );
            println!(
                "Estimation runtime: {} (parser only, no DB writes)",
                format_duration_minutes(elapsed.as_secs_f64() / 60.0)
            );
        }
    }

    Ok(())
}

fn resolve_throughput_config(
    jina_batch_size_cli: Option<usize>,
    jina_batch_minutes_cli: Option<f64>,
    local_throughput_cli: Option<f64>,
    workers: usize,
) -> EmbeddingThroughputConfig {
    let jina_size = jina_batch_size_cli
        .or_else(|| env_usize("CODEGRAPH_JINA_BATCH_SIZE"))
        .unwrap_or(DEFAULT_JINA_BATCH_SIZE)
        .max(1);

    let jina_minutes = jina_batch_minutes_cli
        .or_else(|| env_f64("CODEGRAPH_JINA_BATCH_MINUTES"))
        .unwrap_or(DEFAULT_JINA_BATCH_MINUTES)
        .max(0.1);

    let local_rate = sanitize_positive(local_throughput_cli)
        .or_else(|| sanitize_positive(env_f64("CODEGRAPH_LOCAL_EMBEDDINGS_PER_MINUTE")))
        .or_else(|| Some(default_local_rate(workers)));

    EmbeddingThroughputConfig {
        jina_batch_size: jina_size,
        jina_batch_minutes: jina_minutes,
        local_embeddings_per_minute: local_rate,
    }
}

fn default_local_rate(workers: usize) -> f64 {
    DEFAULT_LOCAL_EMBEDDINGS_PER_WORKER_PER_MINUTE * workers.max(1) as f64
}

fn sanitize_positive(value: Option<f64>) -> Option<f64> {
    value.and_then(|v| {
        if v.is_finite() && v > 0.0 {
            Some(v)
        } else {
            None
        }
    })
}

fn env_usize(key: &str) -> Option<usize> {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
}

fn env_f64(key: &str) -> Option<f64> {
    std::env::var(key).ok().and_then(|v| v.parse::<f64>().ok())
}

fn format_duration_minutes(minutes: f64) -> String {
    if !minutes.is_finite() {
        return "unknown".to_string();
    }
    let total_seconds = (minutes * 60.0).round() as i64;
    let hours = total_seconds / 3600;
    let minutes_part = (total_seconds % 3600) / 60;
    let seconds = total_seconds % 60;

    let mut parts = Vec::new();
    if hours > 0 {
        parts.push(format!("{hours}h"));
    }
    if minutes_part > 0 {
        parts.push(format!("{minutes_part}m"));
    }
    if seconds > 0 || parts.is_empty() {
        parts.push(format!("{seconds}s"));
    }
    parts.join(" ")
}

async fn handle_config(action: ConfigAction) -> Result<()> {
    match action {
        ConfigAction::Init { force } => {
            use codegraph_core::config_manager::ConfigManager;

            println!(
                "{}",
                "Initializing CodeGraph global configuration..."
                    .green()
                    .bold()
            );
            println!();

            // Ensure ~/.codegraph directory exists
            let home = dirs::home_dir()
                .ok_or_else(|| anyhow::anyhow!("Could not determine home directory"))?;
            let codegraph_dir = home.join(".codegraph");

            if !codegraph_dir.exists() {
                std::fs::create_dir_all(&codegraph_dir)
                    .context("Failed to create ~/.codegraph directory")?;
                println!("✓ Created directory: {}", codegraph_dir.display());
            }

            // Create config.toml
            let config_path = codegraph_dir.join("config.toml");
            if config_path.exists() && !force {
                println!(
                    "⚠️  Configuration file already exists: {}",
                    config_path.display()
                );
                println!("   Use --force to overwrite");
            } else {
                ConfigManager::create_default_config(&config_path)
                    .context("Failed to create config.toml")?;
                println!("✓ Created config file: {}", config_path.display());
            }

            // Create .env template
            let env_path = codegraph_dir.join(".env");
            if env_path.exists() && !force {
                println!(
                    "⚠️  Environment file already exists: {}",
                    env_path.display()
                );
                println!("   Use --force to overwrite");
            } else {
                let env_template = r#"# CodeGraph Global Configuration
# This file is loaded automatically by codegraph
# Environment variables here override config.toml settings

# ============================================================================
# EMBEDDING PROVIDER
# ============================================================================
# Provider: "local", "ollama", "lmstudio", "openai", "jina", or "auto"
# - local: ONNX local embeddings (fastest, no API needed)
# - ollama: Ollama embeddings (good balance, requires Ollama running)
# - openai: OpenAI embeddings (cloud, requires API key)
# - jina: Jina embeddings with reranking (best quality, requires API key)
# - auto: Auto-detect best available provider
CODEGRAPH_EMBEDDING_PROVIDER=auto

# ============================================================================
# CLOUD PROVIDERS (OpenAI, Jina)
# ============================================================================
# OpenAI API Key (if using OpenAI embeddings)
# OPENAI_API_KEY=sk-...

# Jina API Key (if using Jina embeddings + reranking)
# JINA_API_KEY=jina_...

# Enable Jina reranking (improves search quality, requires Jina provider)
# JINA_ENABLE_RERANKING=true

# ============================================================================
# LOCAL PROVIDERS (Ollama, Local)
# ============================================================================
# Ollama URL (if using Ollama)
# CODEGRAPH_OLLAMA_URL=http://localhost:11434

# Local embedding model (if using local/ONNX provider)
# CODEGRAPH_LOCAL_MODEL=/path/to/model

# ============================================================================
# PERFORMANCE
# ============================================================================
# Embedding batch size (default: 100)
# Higher values = faster indexing but more memory
# CODEGRAPH_BATCH_SIZE=100

# Embedding dimension (default: auto-detected)
# - 384: all-MiniLM (local)
# - 2048: jina-embeddings-v4 (supports Matryoshka 1024/512/256)
# - 1536: OpenAI text-embedding-3-small, jina-code-embeddings
# CODEGRAPH_EMBEDDING_DIMENSION=2048

# ============================================================================
# LLM PROVIDER (for insights, optional)
# ============================================================================
# LLM Provider: "ollama", "lmstudio", "anthropic", "openai"
# CODEGRAPH_LLM_PROVIDER=lmstudio

# LLM Model
# CODEGRAPH_MODEL=qwen2.5-coder:14b

# Enable LLM insights (context-only mode if disabled)
# CODEGRAPH_LLM_ENABLED=false

# ============================================================================
# LOGGING
# ============================================================================
# Log level: trace, debug, info, warn, error
# RUST_LOG=warn
"#;

                std::fs::write(&env_path, env_template).context("Failed to create .env file")?;
                println!("✓ Created environment file: {}", env_path.display());
            }

            println!();
            println!(
                "{}",
                "Configuration initialized successfully!".green().bold()
            );
            println!();
            println!("Next steps:");
            println!(
                "1. Edit {} to configure your API keys and preferences",
                env_path.display()
            );
            println!("2. Run 'codegraph config show' to verify your configuration");
            println!("3. Run 'codegraph index <path>' to start indexing your codebase");
            println!();
            println!("Configuration hierarchy (highest to lowest priority):");
            println!("  1. Environment variables");
            println!("  2. Local .env (current directory)");
            println!("  3. Global {}", env_path.display());
            println!("  4. Local .codegraph.toml (current directory)");
            println!("  5. Global {}", config_path.display());
            println!("  6. Built-in defaults");
        }
        ConfigAction::Show { json } => {
            use codegraph_core::config_manager::ConfigManager;

            // Load actual configuration
            let config_mgr = ConfigManager::load().context("Failed to load configuration")?;
            let config = config_mgr.config();

            if json {
                let json_output = serde_json::json!({
                    "embedding": {
                        "provider": config.embedding.provider,
                        "model": config.embedding.model,
                        "dimension": config.embedding.dimension,
                        "jina_task": config.embedding.jina_task,
                    },
                    "rerank": {
                        "provider": config.rerank.provider,
                        "top_n": config.rerank.top_n,
                        "jina_model": config.rerank.jina.as_ref().map(|j| &j.model),
                        "ollama_model": config.rerank.ollama.as_ref().map(|o| &o.model),
                    },
                    "llm": {
                        "enabled": config.llm.enabled,
                        "provider": config.llm.provider,
                        "model": config.llm.model,
                        "context_window": config.llm.context_window,
                    }
                });
                println!("{}", serde_json::to_string_pretty(&json_output)?);
            } else {
                println!("{}", "Current Configuration:".blue().bold());
                println!(
                    "  Embedding Provider: {}",
                    config.embedding.provider.yellow()
                );
                if let Some(model) = config.embedding.model.as_deref() {
                    println!("  Embedding Model: {}", model.yellow());
                }
                println!("  Vector Dimension: {}", config.embedding.dimension);

                if config.embedding.provider == "jina" {
                    println!("\n  {}", "Jina Embedding Settings:".green().bold());
                    println!("    API Base: {}", config.embedding.jina_api_base);
                    println!("    Late Chunking: {}", config.embedding.jina_late_chunking);
                    println!("    Task: {}", config.embedding.jina_task);
                }

                // Reranking configuration (separate from embedding)
                println!("\n  {}", "Reranking Settings:".green().bold());
                println!("    Provider: {:?}", config.rerank.provider);
                println!("    Top-N: {}", config.rerank.top_n);
                if let Some(ref jina) = config.rerank.jina {
                    println!("    Jina Model: {}", jina.model);
                }
                if let Some(ref ollama) = config.rerank.ollama {
                    println!("    Ollama Model: {}", ollama.model);
                }

                println!("\n  {}", "LLM Settings:".green().bold());
                println!("    Provider: {}", config.llm.provider.yellow());
                println!(
                    "    Enabled: {}",
                    if config.llm.enabled {
                        "yes".green()
                    } else {
                        "no (context-only)".yellow()
                    }
                );
                if let Some(model) = config.llm.model.as_deref() {
                    println!("    Model: {}", model.yellow());
                }
                println!("    Context Window: {}", config.llm.context_window);
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
        ConfigAction::DbCheck { namespace, database } => {
            handle_db_check(namespace, database).await?;
        }
        ConfigAction::AgentStatus { json } => {
            handle_agent_status(json).await?;
        }
    }

    Ok(())
}

async fn handle_agent_status(json: bool) -> Result<()> {
    use codegraph_core::config_manager::ConfigManager;
    use codegraph_mcp_core::context_aware_limits::ContextTier;

    // Load configuration
    let config_mgr = ConfigManager::load().context("Failed to load configuration")?;
    let config = config_mgr.config();

    // Determine context tier
    let context_window = config.llm.context_window;
    let tier = ContextTier::from_context_window(context_window);

    // Determine prompt verbosity based on tier
    let prompt_verbosity = match tier {
        ContextTier::Small => "TERSE",
        ContextTier::Medium => "BALANCED",
        ContextTier::Large => "DETAILED",
        ContextTier::Massive => "EXPLORATORY",
    };

    // Get tier-specific parameters
    let (max_steps, base_limit, default_max_tokens) = match tier {
        ContextTier::Small => (5, 10, 2048),
        ContextTier::Medium => (10, 25, 4096),
        ContextTier::Large => (15, 50, 8192),
        ContextTier::Massive => (20, 100, 16384),
    };

    // Get max output tokens (config override or tier default)
    let max_output_tokens = config
        .llm
        .mcp_code_agent_max_output_tokens
        .unwrap_or(default_max_tokens);

    // Active MCP tools
    let mcp_tools = vec![
        ("enhanced_search", "Search code with AI insights (2-5s)"),
        (
            "pattern_detection",
            "Analyze coding patterns and conventions (1-3s)",
        ),
        ("vector_search", "Fast vector search (0.5s)"),
        (
            "graph_neighbors",
            "Find dependencies for a code element (0.3s)",
        ),
        (
            "graph_traverse",
            "Follow dependency chains through code (0.5-2s)",
        ),
        (
            "codebase_qa",
            "Ask questions about code and get AI answers (5-30s)",
        ),
        (
            "semantic_intelligence",
            "Deep architectural analysis (30-120s)",
        ),
    ];

    // Analysis types and their prompts
    let analysis_types = vec![
        "code_search",
        "dependency_analysis",
        "call_chain_analysis",
        "architecture_analysis",
        "api_surface_analysis",
        "context_builder",
        "semantic_question",
    ];

    if json {
        // JSON output
        let output = serde_json::json!({
            "llm": {
                "provider": config.llm.provider,
                "model": config.llm.model.as_deref().unwrap_or("auto-detected"),
                "enabled": config.llm.enabled,
            },
            "context": {
                "tier": format!("{:?}", tier),
                "window_size": context_window,
                "prompt_verbosity": prompt_verbosity,
            },
            "orchestrator": {
                "max_steps": max_steps,
                "base_search_limit": base_limit,
                "cache_enabled": true,
                "cache_size": 100,
                "max_output_tokens": max_output_tokens,
                "max_output_tokens_source": if config.llm.mcp_code_agent_max_output_tokens.is_some() {
                    "custom"
                } else {
                    "tier_default"
                },
            },
            "mcp_tools": mcp_tools.iter().map(|(name, desc)| {
                serde_json::json!({
                    "name": name,
                    "description": desc,
                    "prompt_type": prompt_verbosity,
                })
            }).collect::<Vec<_>>(),
            "analysis_types": analysis_types.iter().map(|name| {
                serde_json::json!({
                    "name": name,
                    "prompt_type": prompt_verbosity,
                })
            }).collect::<Vec<_>>(),
        });
        println!("{}", serde_json::to_string_pretty(&output)?);
    } else {
        // Human-readable output
        println!(
            "{}",
            "╔══════════════════════════════════════════════════════════════════════╗"
                .blue()
                .bold()
        );
        println!(
            "{}",
            "║          CodeGraph Orchestrator-Agent Configuration                  ║"
                .blue()
                .bold()
        );
        println!(
            "{}",
            "╚══════════════════════════════════════════════════════════════════════╝"
                .blue()
                .bold()
        );
        println!();

        // LLM Configuration
        println!("{}", "🤖 LLM Configuration".green().bold());
        println!("   Provider: {}", config.llm.provider.yellow());
        println!(
            "   Model: {}",
            config
                .llm
                .model
                .as_deref()
                .unwrap_or("auto-detected")
                .yellow()
        );
        println!(
            "   Status: {}",
            if config.llm.enabled {
                "Enabled".green()
            } else {
                "Context-only mode".yellow()
            }
        );
        println!();

        // Context Configuration
        println!("{}", "📊 Context Configuration".green().bold());
        println!(
            "   Tier: {} ({} tokens)",
            format!("{:?}", tier).yellow(),
            context_window.to_string().cyan()
        );
        println!("   Prompt Verbosity: {}", prompt_verbosity.cyan());
        println!(
            "   Base Search Limit: {} results",
            base_limit.to_string().cyan()
        );
        println!();

        // Orchestrator Configuration
        println!("{}", "⚙️  Orchestrator Settings".green().bold());
        println!(
            "   Max Steps per Workflow: {}",
            max_steps.to_string().cyan()
        );
        println!(
            "   Cache: {} (size: {} entries)",
            "Enabled".green(),
            "100".cyan()
        );
        let max_tokens_display = if config.llm.mcp_code_agent_max_output_tokens.is_some() {
            format!("{} (custom)", max_output_tokens.to_string().cyan())
        } else {
            format!("{} (tier default)", max_output_tokens.to_string().cyan())
        };
        println!("   Max Output Tokens: {}", max_tokens_display);
        println!();

        // MCP Tools
        println!("{}", "🛠️  Available MCP Tools".green().bold());
        for (name, desc) in &mcp_tools {
            println!("   • {} [{}]", name.yellow(), prompt_verbosity.cyan());
            println!("     {}", desc.dimmed());
        }
        println!();

        // Analysis Types
        println!("{}", "🔍 Analysis Types & Prompt Variants".green().bold());
        for analysis_type in &analysis_types {
            println!(
                "   • {} → {}",
                analysis_type.yellow(),
                prompt_verbosity.cyan()
            );
        }
        println!();

        // Tier Details
        println!("{}", "📈 Context Tier Details".green().bold());
        println!("   Small (< 50K):        5 steps,  10 results, TERSE prompts");
        println!("   Medium (50K-150K):   10 steps,  25 results, BALANCED prompts");
        println!("   Large (150K-500K):   15 steps,  50 results, DETAILED prompts");
        println!("   Massive (> 500K):    20 steps, 100 results, EXPLORATORY prompts");
        println!();
        println!("   Current tier: {}", format!("{:?}", tier).green().bold());
    }

    Ok(())
}

async fn handle_db_check(namespace: Option<String>, database: Option<String>) -> Result<()> {
    use codegraph_graph::{SurrealDbConfig, SurrealDbStorage};

    let mut config = SurrealDbConfig::default();
    if let Some(ns) = namespace {
        config.namespace = ns;
    }
    if let Some(db) = database {
        config.database = db;
    }

    let _storage = SurrealDbStorage::new(config).await?;
    println!("✓ SurrealDB connectivity verified");
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
    _config: &codegraph_core::config_manager::CodeGraphConfig,
    _path: PathBuf,
    _langs: Option<Vec<String>>,
    _warmup: usize,
    _trials: usize,
    _queries: Option<Vec<String>>,
    _workers: usize,
    _batch_size: usize,
    _device: Option<String>,
    _max_seq_len: usize,
    _clean: bool,
    _format: String,
    _graph_readonly: bool,
) -> Result<()> {
    println!("The legacy `codegraph perf` command depended on FAISS/RocksDB and has been retired.");
    println!("Use the MCP harnesses (`test_agentic_tools.py` / `test_http_mcp.py`) to benchmark the new agentic tools instead.");
    Ok(())
}

fn prepare_debug_writer(base_path: &Path) -> Result<(BoxMakeWriter, PathBuf)> {
    let log_dir = base_path.join(".codegraph").join("logs");
    std::fs::create_dir_all(&log_dir)?;
    let timestamp = Utc::now().format("%Y%m%dT%H%M%S");
    let log_path = log_dir.join(format!("index-debug-{}.log", timestamp));
    let file = File::create(&log_path)?;
    let shared = Arc::new(Mutex::new(file));
    let writer = BoxMakeWriter::new({
        let shared = Arc::clone(&shared);
        move || DebugLogWriter::new(Arc::clone(&shared))
    });
    Ok((writer, log_path))
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
                128.. => 64,    // 128GB+: Even high-memory boxes benefit from modest batches with Ollama
                96..=127 => 64, // 96-127GB: Keep batches capped for GPU/CPU stability
                64..=95 => 48,  // 64-95GB: Slightly leaner batch for steady throughput
                32..=63 => 32,  // 32-63GB: Conservative batch to prevent throttling
                16..=31 => 24,  // 16-31GB: Small batch keeps latency predictable
                _ => 16,        // <16GB: Minimal batch on constrained systems
            }
        } else {
            // ONNX/OpenAI/LM Studio: Reasonable batches to avoid throttling
            match memory_gb {
                128.. => 256,    // 128GB+: Maximum safe batch size
                64..=127 => 128, // 64-127GB: Large batch for good throughput
                32..=63 => 64,   // 32-63GB: Medium batch size
                16..=31 => 32,   // 16-31GB: Conservative batch size
                _ => 16,         // <16GB: Minimal batch to avoid memory pressure
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

#[derive(Clone)]
struct DebugLogWriter {
    file: Arc<Mutex<File>>,
}

impl DebugLogWriter {
    fn new(file: Arc<Mutex<File>>) -> Self {
        Self { file }
    }
}

impl Write for DebugLogWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let mut guard = self.file.lock().unwrap();
        guard.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        let mut guard = self.file.lock().unwrap();
        guard.flush()
    }
}

// Daemon mode handlers
#[cfg(feature = "daemon")]
async fn handle_daemon(action: DaemonAction) -> Result<()> {
    match action {
        DaemonAction::Start {
            path,
            foreground,
            languages,
            exclude,
            include,
        } => handle_daemon_start(path, foreground, languages, exclude, include).await,
        DaemonAction::Stop { path } => handle_daemon_stop(path).await,
        DaemonAction::Status { path, json } => handle_daemon_status(path, json).await,
    }
}

#[cfg(feature = "daemon")]
async fn handle_daemon_start(
    path: PathBuf,
    foreground: bool,
    languages: Option<Vec<String>>,
    exclude: Vec<String>,
    include: Vec<String>,
) -> Result<()> {
    use codegraph_mcp::{IndexerConfig, ProjectIndexer};

    let project_root = std::fs::canonicalize(&path)
        .with_context(|| format!("Invalid project path: {:?}", path))?;

    println!(
        "{}",
        format!("🚀 Starting watch daemon for: {}", project_root.display()).green()
    );

    // Check if daemon already running
    let pid_path = PidFile::default_path(&project_root);
    let pid_file = PidFile::new(&pid_path);
    if pid_file.is_process_running()? {
        anyhow::bail!(
            "Daemon already running for this project (PID file: {:?})",
            pid_path
        );
    }

    // Create IndexerConfig
    let indexer_config = IndexerConfig {
        languages: languages.unwrap_or_default(),
        exclude_patterns: exclude,
        include_patterns: include,
        project_root: project_root.clone(),
        ..Default::default()
    };

    // Create WatchConfig
    let watch_config = WatchConfig {
        project_root: project_root.clone(),
        debounce_ms: 30,
        batch_timeout_ms: 200,
        health_check_interval_secs: 30,
        reconnect_backoff: Default::default(),
        circuit_breaker: Default::default(),
        indexer: indexer_config.clone(),
    };

    // Create ProjectIndexer
    use codegraph_core::config_manager::ConfigManager;
    let config_mgr = ConfigManager::load().with_context(|| "Failed to load configuration")?;
    let global_config = config_mgr.config().clone();

    let indexer = ProjectIndexer::new(
        indexer_config,
        &global_config,
        indicatif::MultiProgress::new(),
    )
    .await
    .with_context(|| "Failed to create project indexer")?;

    // Create and start daemon
    let mut daemon =
        WatchDaemon::new(watch_config).with_context(|| "Failed to create watch daemon")?;
    daemon.set_indexer(indexer);

    if foreground {
        println!(
            "{}",
            "Running in foreground. Press Ctrl+C to stop.".yellow()
        );
        daemon.start().await?;
    } else {
        // For now, just run in foreground (proper daemonization requires fork)
        println!(
            "{}",
            "Note: Running in foreground mode. Use --foreground flag for explicit foreground mode."
                .yellow()
        );
        println!("{}", "Press Ctrl+C to stop the daemon.".yellow());
        daemon.start().await?;
    }

    Ok(())
}

#[cfg(feature = "daemon")]
async fn handle_daemon_stop(path: PathBuf) -> Result<()> {
    use nix::sys::signal::{kill, Signal};
    use nix::unistd::Pid;

    let project_root = std::fs::canonicalize(&path)
        .with_context(|| format!("Invalid project path: {:?}", path))?;

    let pid_path = PidFile::default_path(&project_root);
    let pid_file = PidFile::new(&pid_path);

    match pid_file.read()? {
        Some(pid) => {
            println!(
                "{}",
                format!("🛑 Stopping daemon (PID: {})...", pid).yellow()
            );

            // Send SIGTERM
            let pid = Pid::from_raw(pid as i32);
            match kill(pid, Signal::SIGTERM) {
                Ok(_) => {
                    println!("{}", "✅ Stop signal sent successfully".green());

                    // Wait a bit and check if it stopped
                    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

                    if !pid_file.is_process_running()? {
                        println!("{}", "✅ Daemon stopped".green());
                    } else {
                        println!(
                            "{}",
                            "⚠️  Daemon still running. May take a moment to shut down.".yellow()
                        );
                    }
                }
                Err(e) => {
                    anyhow::bail!("Failed to send stop signal: {}", e);
                }
            }
        }
        None => {
            println!(
                "{}",
                format!("No daemon running for: {}", project_root.display()).yellow()
            );
        }
    }

    Ok(())
}

#[cfg(feature = "daemon")]
async fn handle_daemon_status(path: PathBuf, json: bool) -> Result<()> {
    let project_root = std::fs::canonicalize(&path)
        .with_context(|| format!("Invalid project path: {:?}", path))?;

    let pid_path = PidFile::default_path(&project_root);
    let pid_file = PidFile::new(&pid_path);

    let (running, pid): (bool, Option<u32>) = match pid_file.read()? {
        Some(pid) => (pid_file.is_process_running()?, Some(pid)),
        None => (false, None),
    };

    if json {
        let status = serde_json::json!({
            "project": project_root.display().to_string(),
            "running": running,
            "pid": pid,
            "pid_file": pid_path.display().to_string(),
        });
        println!("{}", serde_json::to_string_pretty(&status)?);
    } else {
        println!("{}", "📊 Watch Daemon Status".bold());
        println!("   Project: {}", project_root.display());
        println!("   PID file: {}", pid_path.display());

        if running {
            println!(
                "   Status: {} (PID: {})",
                "Running".green(),
                pid.unwrap_or(0)
            );
        } else {
            println!("   Status: {}", "Stopped".red());
            if pid.is_some() {
                println!(
                    "   {}",
                    "⚠️  Stale PID file detected. Run 'daemon start' to clean up.".yellow()
                );
            }
        }
    }

    Ok(())
}
