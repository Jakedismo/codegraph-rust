use futures::future::BoxFuture;
use futures::FutureExt;
use rmcp::{
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::*,
    tool, tool_handler, tool_router, ErrorData as McpError, ServerHandler,
};
use std::future::Future;

pub mod config;
pub mod error;
pub mod rag_tools;

#[cfg(test)]
mod tests;

pub use config::*;
pub use error::{CoreRagError, CoreRagResult};
pub use rag_tools::*;

/// Core RAG MCP Server providing CodeGraph functionality through MCP protocol
#[derive(Clone)]
pub struct CoreRagMcpServer {
    /// Server configuration
    _config: CoreRagServerConfig,
    /// CodeGraph tools for RAG functionality
    rag_tools: RagTools,
    /// Tool router for handling tool calls
    tool_router: ToolRouter<Self>,
}

#[tool_router]
impl CoreRagMcpServer {
    /// Create a new Core RAG MCP Server instance
    pub fn new(config: CoreRagServerConfig) -> CoreRagResult<Self> {
        let rag_tools = RagTools::new(config.clone())?;

        Ok(Self {
            _config: config,
            rag_tools,
            tool_router: Self::tool_router(),
        })
    }

    /// Search for code in the CodeGraph vector database
    #[tool(
        description = "Search for code patterns, functions, and concepts in the CodeGraph database using vector similarity"
    )]
    pub fn search_code(
        &self,
        params: Parameters<SearchCodeParams>,
    ) -> BoxFuture<'_, std::result::Result<String, McpError>> {
        let p = params.0;
        async move {
            let limit = p.limit.unwrap_or(10).min(100);
            let threshold = p.threshold.unwrap_or(0.7).clamp(0.0, 1.0);
            match self.rag_tools.search_code(&p.query, limit, threshold).await {
                Ok(results) => {
                    let content =
                        format!(
                            "Found {} code matches for query: '{}'\n\n{}",
                            results.len(),
                            p.query,
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
                                    + if result.content.len() > 200 { "..." } else { "" }
                            ))
                                .collect::<Vec<_>>()
                                .join("\n"),
                        );
                    Ok(content)
                }
                Err(e) => Err(McpError::internal_error(
                    format!("Search failed: {}", e),
                    None,
                )),
            }
        }
        .boxed()
    }

    /// Get detailed information about a specific code node
    #[tool(description = "Get detailed information about a specific code node by its ID")]
    pub fn get_code_details(
        &self,
        params: Parameters<GetCodeDetailsParams>,
    ) -> BoxFuture<'_, std::result::Result<String, McpError>> {
        let p = params.0;
        async move {
            match self.rag_tools.get_code_details(&p.node_id).await {
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
                        p.node_id,
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
                    Ok(content)
                }
                Ok(None) => Err(McpError::internal_error(
                    format!("Node not found: {}", p.node_id),
                    None,
                )),
                Err(e) => Err(McpError::internal_error(
                    format!("Failed to get details: {}", e),
                    None,
                )),
            }
        }
        .boxed()
    }

    /// Analyze code relationships and dependencies
    #[tool(description = "Analyze relationships and dependencies for a given code node")]
    pub fn analyze_relationships(
        &self,
        params: Parameters<AnalyzeRelationshipsParams>,
    ) -> BoxFuture<'_, std::result::Result<String, McpError>> {
        let p = params.0;
        async move {
            let depth = p.depth.unwrap_or(2).min(5);
            match self
                .rag_tools
                .analyze_relationships(&p.node_id, depth)
                .await
            {
                Ok(analysis) => {
                    let content = format!(
                        "Relationship Analysis for Node: {}\n\n\
                     Direct Dependencies ({}):\n{}\n\n\
                     Dependents ({}):\n{}\n\n\
                     Related Nodes ({}):\n{}",
                        p.node_id,
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
                    Ok(content)
                }
                Err(e) => Err(McpError::internal_error(
                    format!("Analysis failed: {}", e),
                    None,
                )),
            }
        }
        .boxed()
    }

    /// Get repository statistics and overview
    #[tool(description = "Get statistics and overview of the CodeGraph repository")]
    pub fn get_repo_stats(
        &self,
        _params: Parameters<rmcp::model::EmptyObject>,
    ) -> BoxFuture<'_, std::result::Result<String, McpError>> {
        async move {
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

                    Ok(content)
                }
                Err(e) => Err(McpError::internal_error(
                    format!("Failed to get stats: {}", e),
                    None,
                )),
            }
        }
        .boxed()
    }

    /// Semantic code search using natural language queries
    #[tool(
        description = "Perform semantic search using natural language queries to find relevant code"
    )]
    pub fn semantic_search(
        &self,
        params: Parameters<SemanticSearchParams>,
    ) -> BoxFuture<'_, std::result::Result<String, McpError>> {
        let p = params.0;
        async move {
            let limit = p.limit.unwrap_or(10).min(50);
            match self.rag_tools.semantic_search(&p.query, limit).await {
                Ok(results) => {
                    let content = format!(
                        "Semantic Search Results for: '{}'\n\n{}",
                        p.query,
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
                            .join("\n"),
                    );

                    Ok(content)
                }
                Err(e) => Err(McpError::internal_error(
                    format!("Semantic search failed: {}", e),
                    None,
                )),
            }
        }
        .boxed()
    }
}

/// Implement the MCP ServerHandler trait
#[tool_handler]
impl ServerHandler for CoreRagMcpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: ProtocolVersion::default(),
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            server_info: Implementation {
                name: "core-rag-mcp-server".into(),
                version: env!("CARGO_PKG_VERSION").into(),
            },
            instructions: Some(
                "CodeGraph RAG MCP Server providing semantic code search, analysis, and repository insights. \
                Use the available tools to search for code, analyze relationships, and get repository statistics."
                    .into(),
            ),
        }
    }
}

impl CoreRagMcpServer {
    /// Create a service factory for the MCP server
    pub fn service_factory(
        config: CoreRagServerConfig,
    ) -> impl Fn() -> std::result::Result<Self, std::io::Error> + Send + Sync + 'static {
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

// Tool parameter structs for rmcp Parameters wrapper
// Use schemars re-exported by rmcp to avoid version mismatches
use rmcp::schemars::JsonSchema;
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[schemars(crate = "rmcp::schemars")]
struct SearchCodeParams {
    query: String,
    #[serde(default)]
    limit: Option<u32>,
    #[serde(default)]
    threshold: Option<f32>,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[schemars(crate = "rmcp::schemars")]
struct GetCodeDetailsParams {
    node_id: String,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[schemars(crate = "rmcp::schemars")]
struct AnalyzeRelationshipsParams {
    node_id: String,
    #[serde(default)]
    depth: Option<u32>,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[schemars(crate = "rmcp::schemars")]
struct SemanticSearchParams {
    query: String,
    #[serde(default)]
    limit: Option<u32>,
}
