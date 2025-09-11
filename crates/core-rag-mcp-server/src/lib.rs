use rmcp::{
    handler::server::router::tool::ToolRouter, model::*, tool, tool_handler, tool_router,
    ErrorData as McpError, ServerHandler, ServiceExt,
};
use std::future::Future;
use std::sync::Arc;
use tokio::sync::RwLock;

pub mod config;
pub mod error;
pub mod rag_tools;

#[cfg(test)]
mod tests;

pub use config::*;
pub use error::*;
pub use rag_tools::*;

/// Core RAG MCP Server providing CodeGraph functionality through MCP protocol
#[derive(Clone)]
pub struct CoreRagMcpServer {
    /// Server configuration
    config: CoreRagServerConfig,
    /// CodeGraph tools for RAG functionality
    rag_tools: RagTools,
    /// Tool router for handling tool calls
    tool_router: ToolRouter<Self>,
}

#[tool_router]
impl CoreRagMcpServer {
    /// Create a new Core RAG MCP Server instance
    pub fn new(config: CoreRagServerConfig) -> Result<Self> {
        let rag_tools = RagTools::new(config.clone())?;

        Ok(Self {
            config,
            rag_tools,
            tool_router: Self::tool_router(),
        })
    }

    /// Search for code in the CodeGraph vector database
    #[tool(
        description = "Search for code patterns, functions, and concepts in the CodeGraph database using vector similarity"
    )]
    async fn search_code(
        &self,
        query: String,
        limit: Option<u32>,
        threshold: Option<f32>,
    ) -> Result<CallToolResult, McpError> {
        let limit = limit.unwrap_or(10).min(100);
        let threshold = threshold.unwrap_or(0.7).clamp(0.0, 1.0);

        match self.rag_tools.search_code(&query, limit, threshold).await {
            Ok(results) => {
                let content = format!(
                    "Found {} code matches for query: '{}'\n\n{}",
                    results.len(),
                    query,
                    results
                        .iter()
                        .enumerate()
                        .map(|(i, result)| format!(
                            "{}. {} (score: {:.3})\n   Path: {}\n   Type: {}\n   Content: {}\n",
                            i + 1,
                            result.name,
                            result.score,
                            result.path,
                            result.node_type,
                            result.content.chars().take(200).collect::<String>()
                                + if result.content.len() > 200 {
                                    "..."
                                } else {
                                    ""
                                }
                        ))
                        .collect::<Vec<_>>()
                        .join("\n")
                );

                Ok(CallToolResult::success(vec![Content::text(content)]))
            }
            Err(e) => Err(McpError::internal_error(format!("Search failed: {}", e))),
        }
    }

    /// Get detailed information about a specific code node
    #[tool(description = "Get detailed information about a specific code node by its ID")]
    async fn get_code_details(&self, node_id: String) -> Result<CallToolResult, McpError> {
        match self.rag_tools.get_code_details(&node_id).await {
            Ok(Some(details)) => {
                let content = format!(
                    "Code Details for ID: {}\n\n\
                     Name: {}\n\
                     Type: {}\n\
                     Path: {}\n\
                     Language: {:?}\n\
                     Line Range: {} - {}\n\
                     Dependencies: {}\n\
                     Metadata: {:?}\n\n\
                     Content:\n{}",
                    node_id,
                    details.name,
                    details.node_type,
                    details.path,
                    details.language,
                    details.start_line,
                    details.end_line,
                    details.dependencies.join(", "),
                    details.metadata,
                    details.content
                );

                Ok(CallToolResult::success(vec![Content::text(content)]))
            }
            Ok(None) => Err(McpError::internal_error(format!(
                "Node not found: {}",
                node_id
            ))),
            Err(e) => Err(McpError::internal_error(format!(
                "Failed to get details: {}",
                e
            ))),
        }
    }

    /// Analyze code relationships and dependencies
    #[tool(description = "Analyze relationships and dependencies for a given code node")]
    async fn analyze_relationships(
        &self,
        node_id: String,
        depth: Option<u32>,
    ) -> Result<CallToolResult, McpError> {
        let depth = depth.unwrap_or(2).min(5);

        match self.rag_tools.analyze_relationships(&node_id, depth).await {
            Ok(analysis) => {
                let content = format!(
                    "Relationship Analysis for Node: {}\n\n\
                     Direct Dependencies ({}):\n{}\n\n\
                     Dependents ({}):\n{}\n\n\
                     Related Nodes ({}):\n{}",
                    node_id,
                    analysis.dependencies.len(),
                    analysis
                        .dependencies
                        .iter()
                        .map(|dep| format!("  - {} ({})", dep.name, dep.relationship_type))
                        .collect::<Vec<_>>()
                        .join("\n"),
                    analysis.dependents.len(),
                    analysis
                        .dependents
                        .iter()
                        .map(|dep| format!("  - {} ({})", dep.name, dep.relationship_type))
                        .collect::<Vec<_>>()
                        .join("\n"),
                    analysis.related.len(),
                    analysis
                        .related
                        .iter()
                        .map(|rel| format!("  - {} (similarity: {:.3})", rel.name, rel.score))
                        .collect::<Vec<_>>()
                        .join("\n")
                );

                Ok(CallToolResult::success(vec![Content::text(content)]))
            }
            Err(e) => Err(McpError::internal_error(format!("Analysis failed: {}", e))),
        }
    }

    /// Get repository statistics and overview
    #[tool(description = "Get statistics and overview of the CodeGraph repository")]
    async fn get_repo_stats(&self) -> Result<CallToolResult, McpError> {
        match self.rag_tools.get_repo_stats().await {
            Ok(stats) => {
                let content = format!(
                    "CodeGraph Repository Statistics\n\n\
                     Total Nodes: {}\n\
                     Languages: {}\n\
                     Files: {}\n\
                     Functions: {}\n\
                     Classes: {}\n\
                     Modules: {}\n\
                     Test Files: {}\n\n\
                     Language Breakdown:\n{}\n\n\
                     Recent Activity:\n\
                     - Last Updated: {}\n\
                     - Recent Changes: {}",
                    stats.total_nodes,
                    stats.languages.len(),
                    stats.file_count,
                    stats.function_count,
                    stats.class_count,
                    stats.module_count,
                    stats.test_file_count,
                    stats
                        .languages
                        .iter()
                        .map(|(lang, count)| format!("  - {}: {} files", lang, count))
                        .collect::<Vec<_>>()
                        .join("\n"),
                    stats.last_updated.format("%Y-%m-%d %H:%M:%S"),
                    stats.recent_changes
                );

                Ok(CallToolResult::success(vec![Content::text(content)]))
            }
            Err(e) => Err(McpError::internal_error(format!(
                "Failed to get stats: {}",
                e
            ))),
        }
    }

    /// Semantic code search using natural language queries
    #[tool(
        description = "Perform semantic search using natural language queries to find relevant code"
    )]
    async fn semantic_search(
        &self,
        query: String,
        limit: Option<u32>,
    ) -> Result<CallToolResult, McpError> {
        let limit = limit.unwrap_or(10).min(50);

        match self.rag_tools.semantic_search(&query, limit).await {
            Ok(results) => {
                let content = format!(
                    "Semantic Search Results for: '{}'\n\n{}",
                    query,
                    results
                        .iter()
                        .enumerate()
                        .map(|(i, result)| format!(
                            "{}. {}\n   Path: {}\n   Relevance: {:.3}\n   Context: {}\n",
                            i + 1,
                            result.title,
                            result.path,
                            result.relevance_score,
                            result.context.chars().take(300).collect::<String>()
                                + if result.context.len() > 300 {
                                    "..."
                                } else {
                                    ""
                                }
                        ))
                        .collect::<Vec<_>>()
                        .join("\n")
                );

                Ok(CallToolResult::success(vec![Content::text(content)]))
            }
            Err(e) => Err(McpError::internal_error(format!(
                "Semantic search failed: {}",
                e
            ))),
        }
    }
}

/// Implement the MCP ServerHandler trait
#[tool_handler]
impl ServerHandler for CoreRagMcpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            name: "core-rag-mcp-server".into(),
            version: env!("CARGO_PKG_VERSION").into(),
            instructions: Some(
                "CodeGraph RAG MCP Server providing semantic code search, analysis, and repository insights. \
                Use the available tools to search for code, analyze relationships, and get repository statistics."
                .into()
            ),
            capabilities: ServerCapabilities::builder()
                .enable_tools()
                .build(),
            ..Default::default()
        }
    }
}

impl CoreRagMcpServer {
    /// Create a service factory for the MCP server
    pub fn service_factory(
        config: CoreRagServerConfig,
    ) -> impl Fn() -> Result<Self, std::io::Error> + Send + Sync + 'static {
        move || {
            Self::new(config.clone()).map_err(|e| {
                std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!("Failed to create server: {}", e),
                )
            })
        }
    }
}
