//! Example of using the CodeGraph API to parse a source code file
//!
//! This example demonstrates how to:
//! - Initialize the API client components
//! - Parse a source code file
//! - Extract nodes and relationships
//! - Query the resulting graph
//!
//! Run with: `cargo run --example parse_file -- <path_to_file>`

use codegraph_api::AppState;
use codegraph_core::{ConfigManager, Node};
use codegraph_parser::TreeSitterParser;
use std::env;
use std::path::Path;
use std::sync::Arc;
use tracing::{info, Level};
use tracing_subscriber;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt().with_max_level(Level::INFO).init();

    // Get file path from command line arguments
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <path_to_file>", args[0]);
        eprintln!("Example: {} src/main.rs", args[0]);
        std::process::exit(1);
    }

    let file_path = &args[1];
    let path = Path::new(file_path);

    if !path.exists() {
        eprintln!("Error: File '{}' does not exist", file_path);
        std::process::exit(1);
    }

    info!("Parsing file: {}", file_path);

    // Create configuration and app state
    let config = Arc::new(ConfigManager::new()?);
    let state = AppState::new(config).await?;

    // Create parser
    let parser = TreeSitterParser::new();

    // Read file content
    let content = std::fs::read_to_string(path)?;
    info!("File read successfully, {} bytes", content.len());

    // Determine language from file extension
    let extension = path.extension().and_then(|ext| ext.to_str()).unwrap_or("");

    let language = match extension {
        "rs" => "rust",
        "py" => "python",
        "js" | "jsx" => "javascript",
        "ts" | "tsx" => "typescript",
        "go" => "go",
        _ => {
            eprintln!("Unsupported file extension: {}", extension);
            std::process::exit(1);
        }
    };

    info!("Detected language: {}", language);

    // Parse the file
    match parser.parse(&content, language, file_path) {
        Ok(nodes) => {
            info!("Parsing successful!");
            info!("Found {} nodes", nodes.len());

            // Display node summary
            let mut node_types = std::collections::HashMap::new();
            for node in &nodes {
                *node_types.entry(node.node_type.clone()).or_insert(0) += 1;
            }

            info!("\nNode type summary:");
            for (node_type, count) in node_types.iter() {
                info!("  {}: {}", node_type, count);
            }

            // Show first few nodes as examples
            info!("\nFirst 5 nodes:");
            for (i, node) in nodes.iter().take(5).enumerate() {
                info!(
                    "  {}. {} '{}' at {}:{}",
                    i + 1,
                    node.node_type,
                    node.name,
                    node.location.line,
                    node.location.column
                );
            }

            // Store nodes in the graph (optional)
            let graph = state.graph.write().await;
            let nodes_added = nodes.len();

            // Note: Actual storage would happen here
            // for node in nodes {
            //     graph.add_node(node).await?;
            // }

            info!("\n{} nodes ready to be added to graph", nodes_added);
        }
        Err(e) => {
            eprintln!("Error parsing file: {}", e);
            std::process::exit(1);
        }
    }

    Ok(())
}
