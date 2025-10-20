/// Official MCP-compliant CodeGraph server using rmcp SDK
use clap::{Parser, Subcommand};
use indicatif::MultiProgress;
use rmcp::{transport::stdio, ServiceExt};
use std::path::PathBuf;
use tracing::info;

#[derive(Parser, Debug)]
#[command(
    name = "codegraph-official",
    about = "Revolutionary AI codebase intelligence with full MCP protocol compliance",
    long_about = "CodeGraph transforms any MCP-compatible LLM into a codebase expert"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    Serve {
        #[arg(long, default_value = "stdio")]
        transport: String,
        #[arg(long, default_value = "3000")]
        port: u16,
        #[arg(long, default_value = "127.0.0.1")]
        host: String,
        #[arg(long)]
        config: Option<PathBuf>,
    },
    Init {
        path: PathBuf,
        #[arg(long)]
        name: Option<String>,
    },
    Index {
        path: PathBuf,
        #[arg(long, value_delimiter = ',')]
        languages: Option<Vec<String>>,
        #[arg(long)]
        force: bool,
        #[arg(long)]
        recursive: bool,
        #[arg(long, default_value = "100")]
        batch_size: usize,
        #[arg(long, default_value = "4")]
        workers: usize,
    },
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Serve {
            transport,
            port: _port,
            host: _host,
            config: _config,
        } => {
            info!("Starting CodeGraph MCP server with official SDK");

            let mut server = codegraph_mcp::official_server::CodeGraphMCPServer::new_with_graph()
                .await
                .map_err(|e| anyhow::anyhow!("Failed to initialize server with graph: {}", e))?;
            server.initialize_qwen().await;

            match transport.as_str() {
                "stdio" => {
                    info!("Using STDIO transport (official MCP protocol)");

                    let service = server.serve(stdio()).await.map_err(|e| {
                        eprintln!("Failed to start MCP server: {}", e);
                        e
                    })?;

                    info!("CodeGraph MCP server ready with revolutionary capabilities");
                    service.waiting().await?;
                }
                "http" => {
                    info!("HTTP transport not yet implemented in official SDK migration");
                    return Err("HTTP transport not implemented".into());
                }
                _ => {
                    return Err(format!("Unknown transport: {}", transport).into());
                }
            }
        }

        Commands::Init { path, name } => {
            info!("Initializing CodeGraph project");

            let codegraph_dir = path.join(".codegraph");
            std::fs::create_dir_all(&codegraph_dir)?;
            std::fs::create_dir_all(codegraph_dir.join("db"))?;
            std::fs::create_dir_all(codegraph_dir.join("vectors"))?;
            std::fs::create_dir_all(codegraph_dir.join("cache"))?;

            if let Some(name) = name {
                println!("Project name: {}", name);
            }

            println!("Project initialized successfully!");
        }

        Commands::Index {
            path,
            languages,
            force,
            recursive,
            batch_size,
            workers,
        } => {
            info!("Starting revolutionary indexing with official MCP backend");

            let config = codegraph_mcp::IndexerConfig {
                languages: languages.unwrap_or_default(),
                exclude_patterns: vec![],
                include_patterns: vec![],
                recursive,
                force_reindex: force,
                watch: false,
                workers,
                batch_size,
                device: Some("auto".to_string()),
                max_seq_len: 512,
                ..Default::default()
            };

            let multi_progress = MultiProgress::new();
            let mut indexer =
                codegraph_mcp::ProjectIndexer::new(config, multi_progress).await?;
            let stats = indexer.index_project(&path).await?;

            println!("INDEXING COMPLETE!");
            println!("Files: {} indexed", stats.files);
            println!("Embeddings: {} generated", stats.embeddings);
        }
    }

    Ok(())
}
