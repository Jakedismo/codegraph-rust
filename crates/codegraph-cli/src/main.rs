use anyhow::{Context, Result};
use clap::{Parser, Subcommand, ValueEnum};
use codegraph_api::state::AppState;
use codegraph_core::{ConfigManager, IsolationLevel};
use colored::Colorize;
use serde::Serialize;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Parser)]
#[command(name = "codegraph")]
#[command(about = "CodeGraph CLI - Code versioning and dependency analysis", long_about = None)]
#[command(version)]
struct Cli {
    /// Output format (json, pretty, table)
    #[arg(short, long, global = true, default_value = "pretty")]
    output: OutputFormat,

    /// Storage path for CodeGraph data
    #[arg(long, global = true, env = "CODEGRAPH_STORAGE")]
    storage: Option<String>,

    /// Verbose output
    #[arg(short, long, global = true)]
    verbose: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Clone, ValueEnum)]
enum OutputFormat {
    Json,
    Pretty,
    Table,
}

#[derive(Subcommand)]
enum Commands {
    /// Transaction management
    #[command(subcommand)]
    Transaction(TransactionCommands),

    /// Version management
    #[command(subcommand)]
    Version(VersionCommands),

    /// Branch management
    #[command(subcommand)]
    Branch(BranchCommands),

    /// Search and query
    #[command(subcommand)]
    Search(SearchCommands),

    /// System information and status
    Status,
}

#[derive(Subcommand)]
enum TransactionCommands {
    /// Begin a new transaction
    Begin {
        /// Isolation level
        #[arg(short, long, value_enum, default_value = "read-committed")]
        isolation: IsolationLevelArg,
    },

    /// Commit a transaction
    Commit {
        /// Transaction ID
        transaction_id: String,
    },

    /// Rollback a transaction
    Rollback {
        /// Transaction ID
        transaction_id: String,
    },

    /// Get transaction statistics
    Stats,
}

#[derive(Clone, ValueEnum)]
enum IsolationLevelArg {
    ReadUncommitted,
    ReadCommitted,
    RepeatableRead,
    Serializable,
}

impl From<IsolationLevelArg> for IsolationLevel {
    fn from(arg: IsolationLevelArg) -> Self {
        match arg {
            IsolationLevelArg::ReadUncommitted => IsolationLevel::ReadUncommitted,
            IsolationLevelArg::ReadCommitted => IsolationLevel::ReadCommitted,
            IsolationLevelArg::RepeatableRead => IsolationLevel::RepeatableRead,
            IsolationLevelArg::Serializable => IsolationLevel::Serializable,
        }
    }
}

#[derive(Subcommand)]
enum VersionCommands {
    /// Create a new version
    Create {
        /// Version name
        #[arg(short, long)]
        name: String,

        /// Version description
        #[arg(short, long)]
        description: String,

        /// Author
        #[arg(short, long)]
        author: String,

        /// Parent version IDs (comma-separated)
        #[arg(short, long, value_delimiter = ',')]
        parents: Vec<String>,
    },

    /// List versions
    List {
        /// Maximum number of versions to list
        #[arg(short, long, default_value = "50")]
        limit: u32,
    },

    /// Get version details
    Get {
        /// Version ID
        version_id: String,
    },

    /// Tag a version
    Tag {
        /// Version ID
        version_id: String,

        /// Tag name
        #[arg(short, long)]
        tag: String,

        /// Tag message
        #[arg(short, long)]
        message: Option<String>,

        /// Author
        #[arg(short, long)]
        author: String,
    },

    /// Compare two versions
    Compare {
        /// From version ID
        from: String,

        /// To version ID
        to: String,
    },
}

#[derive(Subcommand)]
enum BranchCommands {
    /// Create a new branch
    Create {
        /// Branch name
        #[arg(short, long)]
        name: String,

        /// Version to branch from
        #[arg(short, long)]
        from: String,

        /// Author
        #[arg(short, long)]
        author: String,

        /// Description
        #[arg(short, long)]
        description: Option<String>,
    },

    /// List branches
    List,

    /// Get branch details
    Get {
        /// Branch name
        name: String,
    },

    /// Delete a branch
    Delete {
        /// Branch name
        name: String,
    },

    /// Merge branches
    Merge {
        /// Source branch
        #[arg(short, long)]
        source: String,

        /// Target branch
        #[arg(short, long)]
        target: String,

        /// Author
        #[arg(short, long)]
        author: String,

        /// Merge message
        #[arg(short, long)]
        message: Option<String>,
    },
}

#[derive(Subcommand)]
enum SearchCommands {
    /// Search for nodes
    Query {
        /// Search query
        query: String,
    },
}

// Output structures
#[derive(Serialize)]
struct TransactionResult {
    transaction_id: String,
    isolation_level: String,
    status: String,
}

#[derive(Serialize)]
struct VersionResult {
    version_id: String,
    name: String,
    description: String,
    author: String,
    created_at: String,
}

#[derive(Serialize)]
struct BranchResult {
    name: String,
    head: String,
    created_at: String,
    created_by: String,
}

#[derive(Serialize)]
struct StatusResult {
    storage_path: String,
    status: String,
    message: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Initialize configuration
    let config = ConfigManager::new(None)
        .context("Failed to load configuration")?;

    // Initialize app state (for now, we'll create a temporary one)
    // In production, this would connect to the actual storage
    let state = AppState::new(Arc::new(config))
        .await
        .context("Failed to initialize application state")?;

    // Execute command
    match execute_command(&cli, state).await {
        Ok(output) => {
            print_output(&cli.output, &output)?;
            Ok(())
        }
        Err(e) => {
            eprintln!("{} {}", "Error:".red().bold(), e);
            std::process::exit(1);
        }
    }
}

async fn execute_command(cli: &Cli, state: AppState) -> Result<serde_json::Value> {
    match &cli.command {
        Commands::Transaction(cmd) => execute_transaction_command(cmd, state).await,
        Commands::Version(cmd) => execute_version_command(cmd, state).await,
        Commands::Branch(cmd) => execute_branch_command(cmd, state).await,
        Commands::Search(cmd) => execute_search_command(cmd, state).await,
        Commands::Status => execute_status_command(state).await,
    }
}

async fn execute_transaction_command(
    cmd: &TransactionCommands,
    state: AppState,
) -> Result<serde_json::Value> {
    match cmd {
        TransactionCommands::Begin { isolation } => {
            let isolation_level: IsolationLevel = isolation.clone().into();
            let tx_id = state
                .transactional_graph
                .transaction_manager
                .begin_transaction(isolation_level)
                .await
                .context("Failed to begin transaction")?;

            let result = TransactionResult {
                transaction_id: tx_id.to_string(),
                isolation_level: format!("{:?}", isolation_level),
                status: "active".to_string(),
            };

            Ok(serde_json::to_value(result)?)
        }

        TransactionCommands::Commit { transaction_id } => {
            let tx_id = Uuid::parse_str(transaction_id)
                .context("Invalid transaction ID format")?;

            state
                .transactional_graph
                .transaction_manager
                .commit_transaction(tx_id)
                .await
                .context("Failed to commit transaction")?;

            let result = TransactionResult {
                transaction_id: transaction_id.clone(),
                isolation_level: "N/A".to_string(),
                status: "committed".to_string(),
            };

            Ok(serde_json::to_value(result)?)
        }

        TransactionCommands::Rollback { transaction_id } => {
            let tx_id = Uuid::parse_str(transaction_id)
                .context("Invalid transaction ID format")?;

            state
                .transactional_graph
                .transaction_manager
                .rollback_transaction(tx_id)
                .await
                .context("Failed to rollback transaction")?;

            let result = TransactionResult {
                transaction_id: transaction_id.clone(),
                isolation_level: "N/A".to_string(),
                status: "rolled_back".to_string(),
            };

            Ok(serde_json::to_value(result)?)
        }

        TransactionCommands::Stats => {
            let stats = state
                .transactional_graph
                .transaction_manager
                .get_transaction_stats()
                .await
                .context("Failed to get transaction statistics")?;

            Ok(serde_json::json!({
                "active_transactions": stats.active_transactions,
                "committed_transactions": stats.committed_transactions,
                "aborted_transactions": stats.aborted_transactions,
                "average_commit_time_ms": stats.average_commit_time_ms,
            }))
        }
    }
}

async fn execute_version_command(
    cmd: &VersionCommands,
    state: AppState,
) -> Result<serde_json::Value> {
    match cmd {
        VersionCommands::Create {
            name,
            description,
            author,
            parents,
        } => {
            let parent_ids: Result<Vec<Uuid>> = parents
                .iter()
                .map(|id| Uuid::parse_str(id).context("Invalid parent version ID"))
                .collect();

            let parent_ids = parent_ids?;

            let version_id = state
                .transactional_graph
                .version_manager
                .create_version(
                    name.clone(),
                    description.clone(),
                    author.clone(),
                    parent_ids,
                )
                .await
                .context("Failed to create version")?;

            let result = VersionResult {
                version_id: version_id.to_string(),
                name: name.clone(),
                description: description.clone(),
                author: author.clone(),
                created_at: chrono::Utc::now().to_rfc3339(),
            };

            Ok(serde_json::to_value(result)?)
        }

        VersionCommands::List { limit } => {
            let versions = state
                .transactional_graph
                .version_manager
                .list_versions()
                .await
                .context("Failed to list versions")?;

            let results: Vec<_> = versions
                .into_iter()
                .take(*limit as usize)
                .map(|v| VersionResult {
                    version_id: v.id.to_string(),
                    name: v.name,
                    description: v.description,
                    author: v.author,
                    created_at: v.timestamp.to_rfc3339(),
                })
                .collect();

            Ok(serde_json::to_value(results)?)
        }

        VersionCommands::Get { version_id } => {
            let id = Uuid::parse_str(version_id).context("Invalid version ID format")?;

            let version = state
                .transactional_graph
                .version_manager
                .get_version(id)
                .await
                .context("Failed to get version")?
                .ok_or_else(|| anyhow::anyhow!("Version not found"))?;

            let result = VersionResult {
                version_id: version.id.to_string(),
                name: version.name,
                description: version.description,
                author: version.author,
                created_at: version.timestamp.to_rfc3339(),
            };

            Ok(serde_json::to_value(result)?)
        }

        VersionCommands::Tag {
            version_id,
            tag,
            message,
            author,
        } => {
            let id = Uuid::parse_str(version_id).context("Invalid version ID format")?;

            state
                .transactional_graph
                .version_manager
                .tag_version(id, tag.clone())
                .await
                .context("Failed to tag version")?;

            Ok(serde_json::json!({
                "version_id": version_id,
                "tag": tag,
                "message": message,
                "author": author,
                "status": "tagged"
            }))
        }

        VersionCommands::Compare { from, to } => {
            let from_id = Uuid::parse_str(from).context("Invalid from version ID")?;
            let to_id = Uuid::parse_str(to).context("Invalid to version ID")?;

            let diff = state
                .transactional_graph
                .version_manager
                .compare_versions(from_id, to_id)
                .await
                .context("Failed to compare versions")?;

            Ok(serde_json::json!({
                "from_version": from,
                "to_version": to,
                "added_nodes": diff.added_nodes.len(),
                "modified_nodes": diff.modified_nodes.len(),
                "deleted_nodes": diff.deleted_nodes.len(),
            }))
        }
    }
}

async fn execute_branch_command(
    cmd: &BranchCommands,
    state: AppState,
) -> Result<serde_json::Value> {
    match cmd {
        BranchCommands::Create {
            name,
            from,
            author,
            description,
        } => {
            let from_id = Uuid::parse_str(from).context("Invalid version ID")?;

            state
                .transactional_graph
                .version_manager
                .create_branch(name.clone(), from_id)
                .await
                .context("Failed to create branch")?;

            let result = BranchResult {
                name: name.clone(),
                head: from.clone(),
                created_at: chrono::Utc::now().to_rfc3339(),
                created_by: author.clone(),
            };

            Ok(serde_json::to_value(result)?)
        }

        BranchCommands::List => {
            let branches = state
                .transactional_graph
                .version_manager
                .list_branches()
                .await
                .context("Failed to list branches")?;

            let results: Vec<_> = branches
                .into_iter()
                .map(|b| BranchResult {
                    name: b.name,
                    head: b.head.to_string(),
                    created_at: b.created_at.to_rfc3339(),
                    created_by: b.created_by,
                })
                .collect();

            Ok(serde_json::to_value(results)?)
        }

        BranchCommands::Get { name } => {
            let branch = state
                .transactional_graph
                .version_manager
                .get_branch(name.clone())
                .await
                .context("Failed to get branch")?
                .ok_or_else(|| anyhow::anyhow!("Branch not found"))?;

            let result = BranchResult {
                name: branch.name,
                head: branch.head.to_string(),
                created_at: branch.created_at.to_rfc3339(),
                created_by: branch.created_by,
            };

            Ok(serde_json::to_value(result)?)
        }

        BranchCommands::Delete { name } => {
            state
                .transactional_graph
                .version_manager
                .delete_branch(name.clone())
                .await
                .context("Failed to delete branch")?;

            Ok(serde_json::json!({
                "name": name,
                "status": "deleted"
            }))
        }

        BranchCommands::Merge {
            source,
            target,
            author,
            message,
        } => {
            let result = state
                .transactional_graph
                .version_manager
                .merge_branches(source.clone(), target.clone())
                .await
                .context("Failed to merge branches")?;

            Ok(serde_json::json!({
                "source": source,
                "target": target,
                "author": author,
                "message": message,
                "success": result.success,
                "conflicts": result.conflicts.len(),
                "merged_version_id": result.merged_version_id,
            }))
        }
    }
}

async fn execute_search_command(
    _cmd: &SearchCommands,
    _state: AppState,
) -> Result<serde_json::Value> {
    // Placeholder for search functionality
    Ok(serde_json::json!({
        "message": "Search functionality coming soon"
    }))
}

async fn execute_status_command(state: AppState) -> Result<serde_json::Value> {
    let result = StatusResult {
        storage_path: "~/.codegraph".to_string(),
        status: "ok".to_string(),
        message: "CodeGraph is operational".to_string(),
    };

    Ok(serde_json::to_value(result)?)
}

fn print_output(format: &OutputFormat, value: &serde_json::Value) -> Result<()> {
    match format {
        OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(value)?);
        }
        OutputFormat::Pretty => {
            print_pretty(value)?;
        }
        OutputFormat::Table => {
            print_table(value)?;
        }
    }
    Ok(())
}

fn print_pretty(value: &serde_json::Value) -> Result<()> {
    match value {
        serde_json::Value::Object(map) => {
            for (key, val) in map {
                let key_colored = key.cyan().bold();
                match val {
                    serde_json::Value::String(s) => {
                        println!("{}: {}", key_colored, s.green());
                    }
                    serde_json::Value::Number(n) => {
                        println!("{}: {}", key_colored, n.to_string().yellow());
                    }
                    serde_json::Value::Bool(b) => {
                        let val_colored = if *b {
                            "true".green()
                        } else {
                            "false".red()
                        };
                        println!("{}: {}", key_colored, val_colored);
                    }
                    _ => {
                        println!("{}: {}", key_colored, val);
                    }
                }
            }
        }
        serde_json::Value::Array(arr) => {
            for (i, item) in arr.iter().enumerate() {
                println!("\n{}{}:", "Item ".cyan(), (i + 1).to_string().yellow());
                print_pretty(item)?;
            }
        }
        _ => {
            println!("{}", serde_json::to_string_pretty(value)?);
        }
    }
    Ok(())
}

fn print_table(value: &serde_json::Value) -> Result<()> {
    // For simple implementation, fallback to pretty print
    // In production, you'd use the tabled crate for nice tables
    print_pretty(value)
}
