/// Official MCP-compliant CodeGraph server using rmcp SDK
///
/// This binary provides full MCP protocol compliance while preserving
/// all revolutionary CodeGraph functionality including:
/// - Qwen2.5-Coder-14B-128K integration
/// - nomic-embed-code embeddings
/// - Revolutionary impact analysis
/// - Team intelligence and pattern detection

use clap::{Parser, Subcommand};
use rmcp::{transport::stdio, ServiceExt};
use std::path::PathBuf;
use tracing::info;

/// Official MCP-compliant CodeGraph CLI with revolutionary AI capabilities
#[derive(Parser, Debug)]
#[command(
    name = "codegraph-official",
    about = "Revolutionary AI codebase intelligence with full MCP protocol compliance",
    long_about = "CodeGraph transforms any MCP-compatible LLM into a codebase expert through \
                  semantic intelligence powered by Qwen2.5-Coder-14B-128K and nomic-embed-code. \
                  This version provides full compliance with MCP protocol 2025-06-18."
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Start official MCP server with full protocol compliance
    Serve {
        /// Transport type to use
        #[arg(long, default_value = "stdio")]
        transport: String,

        /// Port for HTTP transport
        #[arg(long, default_value = "3000")]
        port: u16,

        /// Host for HTTP transport
        #[arg(long, default_value = "127.0.0.1")]
        host: String,

        /// Configuration file path
        #[arg(long)]
        config: Option<PathBuf>,
    },

    /// Initialize a new project with CodeGraph
    Init {
        /// Project path
        path: PathBuf,

        /// Project name
        #[arg(long)]
        name: Option<String>,
    },

    /// Index a codebase with revolutionary optimizations
    Index {
        /// Path to index
        path: PathBuf,

        /// Languages to index
        #[arg(long, value_delimiter = ',')]
        languages: Option<Vec<String>>,

        /// Force reindex
        #[arg(long)]
        force: bool,

        /// Recursive indexing
        #[arg(long)]
        recursive: bool,

        /// Batch size for embeddings (auto-optimized for your system)
        #[arg(long, default_value = "100")]
        batch_size: usize,

        /// Number of workers (auto-optimized for your system)
        #[arg(long, default_value = "4")]
        workers: usize,
    },
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing for stderr (STDIO-safe)
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Serve { transport, port, host, config } => {
            info!("🚀 Starting CodeGraph MCP server with official SDK");

            // Create and initialize the revolutionary CodeGraph server
            let mut server = codegraph_mcp::official_server::CodeGraphMCPServer::new();
            server.initialize_qwen().await;

            match transport.as_str() {
                "stdio" => {
                    info!("📡 Using STDIO transport (official MCP protocol)");

                    // Use official rmcp STDIO transport
                    let service = server.serve(stdio()).await.map_err(|e| {
                        eprintln!("❌ Failed to start MCP server: {}", e);
                        e
                    })?;

                    info!("✅ CodeGraph MCP server ready with revolutionary capabilities");

                    // Wait for the server to complete
                    service.waiting().await?;
                }
                "http" => {
                    info!("🌐 HTTP transport not yet implemented in official SDK migration");
                    info!("💡 Use STDIO transport for Claude Desktop integration");
                    return Err("HTTP transport not implemented".into());
                }
                _ => {
                    return Err(format!("Unknown transport: {}", transport).into());
                }
            }
        }

        Commands::Init { path, name } => {
            info!("🔧 Initializing CodeGraph project");

            // Use existing initialization logic
            codegraph_mcp::indexer::ProjectIndexer::init_project(&path, name.as_deref()).await?;

            println!("✅ Project initialized successfully!");
            println!("💡 Next: codegraph-official index {} --recursive", path.display());
        }

        Commands::Index {
            path,
            languages,
            force,
            recursive,
            batch_size,
            workers
        } => {
            info!("📊 Starting revolutionary indexing with official MCP backend");

            // Use existing revolutionary indexing logic with optimizations
            let available_memory_gb = estimate_available_memory_gb();
            let (optimized_batch_size, optimized_workers) = optimize_for_memory(
                available_memory_gb,
                batch_size,
                workers
            );

            println!("🚀 Revolutionary CodeGraph Indexing");
            println!("Memory: {}GB detected", available_memory_gb);
            println!("Workers: {} → {} (optimized)", workers, optimized_workers);
            println!("Batch size: {} → {} (optimized)", batch_size, optimized_batch_size);

            if available_memory_gb >= 64 {
                println!("🚀 High-memory system detected - performance optimized!");
            }

            // Use existing indexer with optimizations
            let config = codegraph_mcp::IndexerConfig {
                languages: languages.unwrap_or_default(),
                exclude_patterns: vec![],
                include_patterns: vec![],
                recursive,
                force_reindex: force,
                watch: false,
                workers: optimized_workers,
                batch_size: optimized_batch_size,
                device: "auto".to_string(),
                max_seq_len: 512,
                ..Default::default()
            };

            let mut indexer = codegraph_mcp::ProjectIndexer::new(config);
            let stats = indexer.index_project(&path).await?;

            // Display beautiful results
            println!();
            println!("🎉 INDEXING COMPLETE!");
            println!();
            println!("📊 Performance Summary");
            println!("┌─────────────────────────────────────────────────┐");
            println!("│ 📄 Files: {} indexed                             │", format!("{:>6}", stats.files));
            println!("│ 📝 Lines: {} processed                           │", format!("{:>6}", stats.lines));
            println!("│ 🔧 Functions: {} extracted                       │", format!("{:>6}", stats.functions));
            println!("│ 🏗️  Classes: {} extracted                        │", format!("{:>6}", stats.classes));
            println!("│ 💾 Embeddings: {} generated                      │", format!("{:>6}", stats.embeddings));
            println!("└─────────────────────────────────────────────────┘");

            let provider = std::env::var("CODEGRAPH_EMBEDDING_PROVIDER").unwrap_or("default".to_string());
            if provider == "ollama" {
                println!("🧠 Using SOTA Code-Specialized Embeddings (nomic-embed-code)");
            } else if provider == "onnx" {
                println!("⚡ Using Speed-Optimized Embeddings (ONNX)");
            }

            println!();
            println!("🚀 Ready for Revolutionary MCP Intelligence!");
            println!("Next: codegraph-official serve --transport stdio");
        }
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
fn optimize_for_memory(memory_gb: usize, default_batch_size: usize, default_workers: usize) -> (usize, usize) {
    let embedding_provider = std::env::var("CODEGRAPH_EMBEDDING_PROVIDER").unwrap_or_default();

    let optimized_batch_size = if default_batch_size == 100 { // Default value
        if embedding_provider == "ollama" {
            // Ollama models work better with smaller batches for stability
            match memory_gb {
                128.. => 1024,     // 128GB+: Large but stable batch size
                96..=127 => 768,   // 96-127GB: Medium-large batch
                64..=95 => 512,    // 64-95GB: Medium batch
                32..=63 => 256,    // 32-63GB: Small batch
                16..=31 => 128,    // 16-31GB: Very small batch
                _ => 64,           // <16GB: Minimal batch
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

    let optimized_workers = if default_workers == 4 { // Default value
        match memory_gb {
            128.. => 16,       // 128GB+: Maximum parallelism
            96..=127 => 14,    // 96-127GB: Very high parallelism
            64..=95 => 12,     // 64-95GB: High parallelism
            48..=63 => 10,     // 48-63GB: Medium-high parallelism
            32..=47 => 8,      // 32-47GB: Medium parallelism
            16..=31 => 6,      // 16-31GB: Conservative parallelism
            _ => 4,            // <16GB: Keep default
        }
    } else {
        default_workers // User specified - respect their choice
    };

    (optimized_batch_size, optimized_workers)
}