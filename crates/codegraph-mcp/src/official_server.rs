#![allow(dead_code, unused_variables, unused_imports)]

use futures::future::BoxFuture;
/// Clean Official MCP SDK Implementation for CodeGraph
/// Following exact Counter pattern from rmcp SDK documentation
use rmcp::{
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{
        CallToolResult, Content, Meta, NumberOrString, ProgressNotification,
        ProgressNotificationParam, ProgressToken, ServerCapabilities, ServerInfo,
        ServerNotification,
    },
    tool, tool_handler, tool_router, ErrorData as McpError, Peer, RoleServer, ServerHandler,
};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;

#[cfg(feature = "qwen-integration")]
use crate::cache::{init_cache, CacheConfig};
#[cfg(feature = "qwen-integration")]
use crate::qwen::{QwenClient, QwenConfig};

/// Parameter structs following official rmcp SDK pattern
// #[derive(Deserialize, JsonSchema)]
// struct IncrementRequest {
//     /// Optional amount to increment (defaults to 1)
//     #[serde(default = "default_increment")]
//     amount: i32,
// }

// fn default_increment() -> i32 { 1 }

#[derive(Deserialize, JsonSchema)]
struct SearchRequest {
    /// The search query for semantic analysis
    query: String,
    /// Maximum number of results to return
    #[serde(default = "default_limit")]
    limit: usize,
}

fn default_limit() -> usize {
    5 // Reduced from 10 for faster agent responses
}

#[derive(Deserialize, JsonSchema)]
struct VectorSearchRequest {
    /// Search query text for vector similarity matching
    query: String,
    /// Optional file paths to restrict search (e.g., ["src/", "lib/"])
    #[serde(default)]
    paths: Option<Vec<String>>,
    /// Optional programming languages to filter (e.g., ["rust", "typescript"])
    #[serde(default)]
    langs: Option<Vec<String>>,
    /// Maximum number of results to return
    #[serde(default = "default_limit")]
    limit: usize,
}

#[derive(Deserialize, JsonSchema)]
struct GraphNeighborsRequest {
    /// Node UUID to find neighbors for
    node: String,
    /// Maximum number of neighbors to return
    #[serde(default = "default_neighbor_limit")]
    limit: usize,
}

fn default_neighbor_limit() -> usize {
    20
}

#[derive(Deserialize, JsonSchema)]
struct GraphTraverseRequest {
    /// Starting node UUID for traversal
    start: String,
    /// Maximum depth to traverse (default: 2)
    #[serde(default = "default_depth")]
    depth: usize,
    /// Maximum number of nodes to return
    #[serde(default = "default_traverse_limit")]
    limit: usize,
}

fn default_depth() -> usize {
    2
}
fn default_traverse_limit() -> usize {
    20 // Reduced from 100 to prevent overwhelming agent responses
}

// #[derive(Deserialize, JsonSchema)]
// struct CodeReadRequest {
//     /// File path to read
//     path: String,
//     /// Starting line number (default: 1)
//     #[serde(default = "default_start_line")]
//     start: usize,
//     /// Optional ending line number (default: end of file)
//     #[serde(default)]
//     end: Option<usize>,
// }

// fn default_start_line() -> usize { 1 }

// #[derive(Deserialize, JsonSchema)]
// struct CodePatchRequest {
//     /// File path to modify
//     path: String,
//     /// Text to find and replace
//     find: String,
//     /// Replacement text
//     replace: String,
//     /// Perform dry run without making changes (default: false)
//     #[serde(default)]
//     dry_run: bool,
// }

// #[derive(Deserialize, JsonSchema)]
// struct TestRunRequest {
//     /// Optional package name to test (e.g., "codegraph-core")
//     #[serde(default)]
//     package: Option<String>,
//     /// Additional cargo test arguments
//     #[serde(default)]
//     args: Option<Vec<String>>,
// }

#[derive(Deserialize, JsonSchema)]
struct SemanticIntelligenceRequest {
    /// Analysis query or focus area for comprehensive codebase analysis
    query: String,
    /// Type of analysis to perform (default: "semantic_search")
    #[serde(default = "default_task_type")]
    task_type: String,
    /// Maximum context tokens to use from 128K available (default: 20000 for faster responses)
    #[serde(default = "default_max_context_tokens")]
    max_context_tokens: usize,
}

fn default_task_type() -> String {
    "semantic_search".to_string()
}
fn default_max_context_tokens() -> usize {
    20000 // Reduced from 80000 for faster responses (30-60s instead of 60-120s)
}

#[derive(Deserialize, JsonSchema)]
struct ImpactAnalysisRequest {
    /// Name of the function to analyze for impact
    target_function: String,
    /// Path to the file containing the target function
    file_path: String,
    /// Type of change being proposed (default: "modify")
    #[serde(default = "default_change_type")]
    change_type: String,
}

fn default_change_type() -> String {
    "modify".to_string()
}

#[derive(Deserialize, JsonSchema)]
struct EmptyRequest {
    /// No parameters required
    #[serde(default)]
    _unused: Option<String>,
}

/// REVOLUTIONARY: Request for intelligent codebase Q&A using RAG
#[derive(Deserialize, JsonSchema)]
struct CodebaseQaRequest {
    /// Natural language question about the codebase
    question: String,
    /// Maximum number of results to consider (default: 5 for faster responses)
    #[serde(default)]
    max_results: Option<usize>,
    /// Enable streaming response (default: false for MCP compatibility)
    #[serde(default)]
    streaming: Option<bool>,
}

/// REVOLUTIONARY: Request for intelligent code documentation generation
#[derive(Deserialize, JsonSchema)]
struct CodeDocumentationRequest {
    /// Function, class, or module name to document
    target_name: String,
    /// Optional file path to focus documentation scope
    #[serde(default)]
    file_path: Option<String>,
    /// Documentation style (default: "comprehensive")
    #[serde(default = "default_doc_style")]
    style: String,
}

fn default_doc_style() -> String {
    "comprehensive".to_string()
}

/// Clean CodeGraph MCP server following official Counter pattern
#[derive(Clone)]
pub struct CodeGraphMCPServer {
    /// Graph database for revolutionary AI tools
    graph: Arc<tokio::sync::Mutex<codegraph_graph::CodeGraph>>,
    /// Simple counter for demonstration
    counter: Arc<Mutex<i32>>,
    /// Cached Qwen client for AI-enhanced features
    #[cfg(feature = "qwen-integration")]
    qwen_client: Arc<Mutex<Option<QwenClient>>>,
    /// Official MCP tool router (required by macros)
    tool_router: ToolRouter<Self>,
}

#[tool_router]
impl CodeGraphMCPServer {
    pub fn new() -> Self {
        // Create read-only database connection for concurrent multi-agent access
        let current_dir = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
        let db_root = current_dir.join(".codegraph");
        let db_path = db_root.join("db");

        if let Err(err) = std::fs::create_dir_all(&db_root) {
            eprintln!(
                "⚠️ Unable to create .codegraph directory at {:?}: {}",
                db_root, err
            );
        }
        if let Err(err) = std::fs::create_dir_all(&db_path) {
            eprintln!(
                "⚠️ Unable to create RocksDB directory at {:?}: {}",
                db_path, err
            );
        }

        // Attempt a full read/write graph first, then gracefully fall back to read-only mode
        let graph = match codegraph_graph::CodeGraph::new() {
            Ok(graph) => graph,
            Err(err) => {
                eprintln!(
                    "⚠️ Primary CodeGraph open failed ({}). Falling back to read-only mode.",
                    err
                );
                match codegraph_graph::CodeGraph::new_read_only() {
                    Ok(read_only_graph) => read_only_graph,
                    Err(ro_err) => panic!(
                        "Failed to initialize CodeGraph database. rw={} ro={}",
                        err, ro_err
                    ),
                }
            }
        };

        Self {
            graph: Arc::new(tokio::sync::Mutex::new(graph)),
            counter: Arc::new(Mutex::new(0)),
            #[cfg(feature = "qwen-integration")]
            qwen_client: Arc::new(Mutex::new(None)),
            tool_router: Self::tool_router(),
        }
    }

    /// Initialize with existing graph database from working directory
    pub async fn new_with_graph() -> Result<Self, String> {
        // Ensure database directories exist before opening
        let current_dir = std::env::current_dir().map_err(|e| e.to_string())?;
        let db_root = current_dir.join(".codegraph");
        let db_path = db_root.join("db");

        std::fs::create_dir_all(&db_root)
            .map_err(|e| format!("Failed to create .codegraph directory: {}", e))?;
        std::fs::create_dir_all(&db_path)
            .map_err(|e| format!("Failed to create database directory: {}", e))?;

        // Try to open existing database first, create new if needed
        let graph = codegraph_graph::CodeGraph::new().or_else(|err| {
            eprintln!(
                "⚠️ CodeGraph open failed in writable mode ({}). Falling back to read-only mode.",
                err
            );
            codegraph_graph::CodeGraph::new_read_only().map_err(|ro_err| {
                format!(
                    "Failed to initialize CodeGraph database: {} | {}",
                    err, ro_err
                )
            })
        })?;

        Ok(Self {
            graph: Arc::new(tokio::sync::Mutex::new(graph)),
            counter: Arc::new(Mutex::new(0)),
            #[cfg(feature = "qwen-integration")]
            qwen_client: Arc::new(Mutex::new(None)),
            tool_router: Self::tool_router(),
        })
    }

    // /// Increment counter with proper parameter schema (DISABLED - redundant for development)
    // #[tool(description = "Increment the counter by a specified amount")]
    // async fn increment(&self, params: Parameters<IncrementRequest>) -> Result<CallToolResult, McpError> {
    //     let request = params.0; // Extract the inner value
    //     let mut counter = self.counter.lock().await;
    //     *counter += request.amount;
    //     Ok(CallToolResult::success(vec![Content::text(format!(
    //         "Counter incremented by {} to: {}",
    //         request.amount,
    //         *counter
    //     ))]))
    // }

    /// Enhanced semantic search with AI-powered analysis for finding code patterns and architectural insights
    #[tool(
        description = "Search code with AI insights (2-5s). Returns relevant code + analysis of patterns and architecture. Use for: understanding code behavior, finding related functionality, discovering patterns. Fast alternative: vector_search. Required: query. Optional: limit (default 5)."
    )]
    async fn enhanced_search(
        &self,
        params: Parameters<SearchRequest>,
    ) -> Result<CallToolResult, McpError> {
        let request = params.0; // Extract the inner value

        #[cfg(feature = "qwen-integration")]
        {
            let SearchRequest { query, limit } = request;

            // Pre-compute search results for fallback handling without holding the lock
            let search_results = {
                let graph = self.graph.lock().await;
                crate::server::bin_search_with_scores_shared(
                    query.clone(),
                    None,      // No path filtering for enhanced search
                    None,      // No language filtering for enhanced search
                    limit * 2, // Get more results for AI analysis
                    &graph,
                )
                .await
            };

            let search_results = match search_results {
                Ok(results) => results,
                Err(e) => {
                    return Err(McpError {
                        code: rmcp::model::ErrorCode(-32603),
                        message: format!(
                            "Vector search failed: {}. Ensure codebase is indexed.",
                            e
                        )
                        .into(),
                        data: None,
                    })
                }
            };

            // Use the existing graph database from the server
            let state = crate::server::ServerState {
                graph: self.graph.clone(),
                qwen_client: self.get_qwen_client().await,
            };

            // Call enhanced search with Qwen analysis
            match crate::server::enhanced_search(
                &state,
                serde_json::json!({
                    "query": query.clone(),
                    "max_results": limit,
                    "include_analysis": true
                }),
            )
            .await
            {
                Ok(enhanced_results) => Ok(CallToolResult::success(vec![Content::text(
                    serde_json::to_string_pretty(&enhanced_results)
                        .unwrap_or_else(|_| "Error formatting enhanced search results".to_string()),
                )])),
                Err(e) => {
                    // Fallback to basic search results if AI fails
                    let fallback_results = search_results
                        .get("results")
                        .cloned()
                        .unwrap_or_else(|| serde_json::Value::Array(Vec::new()));

                    let fallback = serde_json::json!({
                        "search_results": fallback_results,
                        "ai_analysis": format!("AI analysis failed: {}", e),
                        "query": query,
                        "fallback_mode": true
                    });
                    Ok(CallToolResult::success(vec![Content::text(
                        serde_json::to_string_pretty(&fallback)
                            .unwrap_or_else(|_| "Error formatting fallback results".to_string()),
                    )]))
                }
            }
        }
        #[cfg(not(feature = "qwen-integration"))]
        {
            Ok(CallToolResult::success(vec![Content::text(format!(
                "Basic Enhanced Search for: '{}'
    \n\
                📊 Results Limit: {}
    \
                💡 Note: Enable qwen-integration for revolutionary AI analysis",
                request.query, request.limit
            ))]))
        }
    }

    /// Analyze coding patterns, conventions, and team standards across your codebase
    #[tool(
        description = "Analyze coding patterns and conventions (1-3s). Detects naming styles, organization patterns, error handling, quality metrics. Use for: understanding team standards, onboarding, code review guidelines. No parameters required."
    )]
    async fn pattern_detection(
        &self,
        params: Parameters<EmptyRequest>,
    ) -> Result<CallToolResult, McpError> {
        let _request = params.0; // Extract the inner value (unused)

        // Use the existing graph database from the server
        let state = crate::server::ServerState {
            graph: self.graph.clone(),
            #[cfg(feature = "qwen-integration")]
            qwen_client: self.get_qwen_client().await,
        };

        // Call pattern detection with team intelligence analysis
        match crate::server::pattern_detection(&state, serde_json::json!({})).await {
            Ok(pattern_results) => Ok(CallToolResult::success(vec![Content::text(
                serde_json::to_string_pretty(&pattern_results)
                    .unwrap_or_else(|_| "Error formatting pattern detection results".to_string()),
            )])),
            Err(e) => {
                // Fallback if pattern analysis fails
                let fallback = serde_json::json!({
                    "error": format!("Pattern detection failed: {}", e),
                    "fallback_mode": true,
                    "note": "Enhanced pattern analysis requires indexed codebase"
                });
                Ok(CallToolResult::success(vec![Content::text(
                    serde_json::to_string_pretty(&fallback)
                        .unwrap_or_else(|_| "Error formatting fallback results".to_string()),
                )]))
            }
        }
    }

    // /// Monitor CodeGraph system performance, cache efficiency, and AI model usage statistics
    // #[tool(description = "Get CodeGraph system performance metrics including cache hit rates, search performance, and AI model usage stats. Use to monitor system health or troubleshoot performance issues. No parameters required.")]
    // async fn performance_metrics(&self, params: Parameters<EmptyRequest>) -> Result<CallToolResult, McpError> {
    //     let _request = params.0; // Extract the inner value (unused)
    //     let counter_val = *self.counter.lock().await;
    //     Ok(CallToolResult::success(vec![Content::text(format!(
    //         "CodeGraph Performance Metrics\n\
    //         📈 Revolutionary System Status:\n\
    //         • Qwen2.5-Coder-14B-128K: Available\n\
    //         • nomic-embed-code embeddings: Available\n\
    //         • FAISS vector indexing: Ready\n\
    //         • 128K context window: Ready\n\
    //         • Complete local stack: Operational\n\
    //         • Tool calls: {}\n\
    //         🚀 Status: World's most advanced AI development platform ready!",
    //         counter_val
    //     ))]))
    // }

    /// Fast similarity search for finding code that matches your query without AI analysis
    #[tool(
        description = "Fast vector search (0.5s). Returns matching code with similarity scores. Use for: quick code lookups, finding similar implementations. For deeper insights use enhanced_search. Required: query. Optional: paths, langs, limit (default 5)."
    )]
    async fn vector_search(
        &self,
        params: Parameters<VectorSearchRequest>,
    ) -> Result<CallToolResult, McpError> {
        let request = params.0;
        let graph = self.graph.lock().await;
        match crate::server::bin_search_with_scores_shared(
            request.query.clone(),
            request.paths,
            request.langs,
            request.limit,
            &graph,
        )
        .await
        {
            Ok(results) => Ok(CallToolResult::success(vec![Content::text(
                serde_json::to_string_pretty(&results)
                    .unwrap_or_else(|_| "Error formatting search results".to_string()),
            )])),
            Err(e) => Err(McpError {
                code: rmcp::model::ErrorCode(-32603),
                message: format!(
                    "Vector search failed: {}. Ensure codebase is indexed with 'codegraph index .'",
                    e
                )
                .into(),
                data: None,
            }),
        }
    }

    /// Find code dependencies and relationships for a specific code element (function, class, etc)
    #[tool(
        description = "Find dependencies for a code element (0.3s). Shows what imports/calls this code and what it depends on. Use for: impact analysis, understanding relationships. Required: node (UUID from search). Optional: limit (default 20). Get UUIDs from vector_search or enhanced_search results."
    )]
    async fn graph_neighbors(
        &self,
        params: Parameters<GraphNeighborsRequest>,
    ) -> Result<CallToolResult, McpError> {
        let request = params.0;
        let id = uuid::Uuid::parse_str(&request.node).map_err(|e| McpError {
            code: rmcp::model::ErrorCode(-32602),
            message: format!("Invalid node UUID: {}", e).into(),
            data: None,
        })?;

        // Use the existing graph database from the server
        let state = crate::server::ServerState {
            graph: self.graph.clone(),
            #[cfg(feature = "qwen-integration")]
            qwen_client: self.get_qwen_client().await,
        };

        // Call graph neighbors with dependency analysis
        match crate::server::graph_neighbors(
            &state,
            serde_json::json!({
                "node": request.node,
                "limit": request.limit
            }),
        )
        .await
        {
            Ok(neighbors_results) => Ok(CallToolResult::success(vec![Content::text(
                serde_json::to_string_pretty(&neighbors_results)
                    .unwrap_or_else(|_| "Error formatting graph neighbors results".to_string()),
            )])),
            Err(e) => {
                // Fallback if graph analysis fails
                let fallback = serde_json::json!({
                    "node": request.node,
                    "limit": request.limit,
                    "error": format!("Graph neighbors analysis failed: {}", e),
                    "fallback_mode": true,
                    "note": "Ensure codebase is indexed and node UUID is valid"
                });
                Ok(CallToolResult::success(vec![Content::text(
                    serde_json::to_string_pretty(&fallback)
                        .unwrap_or_else(|_| "Error formatting fallback results".to_string()),
                )]))
            }
        }
    }

    /// Explore code architecture by following dependency chains from a starting point
    #[tool(
        description = "Follow dependency chains through code (0.5-2s). Traces execution paths and architectural flow. Use for: understanding call chains, mapping data flow. Required: start (UUID from search). Optional: depth (default 2), limit (default 20). Get UUIDs from vector_search or enhanced_search results."
    )]
    async fn graph_traverse(
        &self,
        params: Parameters<GraphTraverseRequest>,
    ) -> Result<CallToolResult, McpError> {
        let request = params.0;
        let _start = uuid::Uuid::parse_str(&request.start).map_err(|e| McpError {
            code: rmcp::model::ErrorCode(-32602),
            message: format!("Invalid start node UUID: {}", e).into(),
            data: None,
        })?;

        // Use the existing graph database from the server
        let state = crate::server::ServerState {
            graph: self.graph.clone(),
            #[cfg(feature = "qwen-integration")]
            qwen_client: self.get_qwen_client().await,
        };

        // Call graph traverse with architectural flow analysis
        match crate::server::graph_traverse(
            &state,
            serde_json::json!({
                "start": request.start,
                "depth": request.depth,
                "limit": request.limit
            }),
        )
        .await
        {
            Ok(traverse_results) => Ok(CallToolResult::success(vec![Content::text(
                serde_json::to_string_pretty(&traverse_results)
                    .unwrap_or_else(|_| "Error formatting graph traverse results".to_string()),
            )])),
            Err(e) => {
                // Fallback if graph traversal fails
                let fallback = serde_json::json!({
                    "start": request.start,
                    "depth": request.depth,
                    "limit": request.limit,
                    "error": format!("Graph traversal failed: {}", e),
                    "fallback_mode": true,
                    "note": "Ensure codebase is indexed and start node UUID is valid"
                });
                Ok(CallToolResult::success(vec![Content::text(
                    serde_json::to_string_pretty(&fallback)
                        .unwrap_or_else(|_| "Error formatting fallback results".to_string()),
                )]))
            }
        }
    }

    /// REVOLUTIONARY: Intelligent codebase Q&A using RAG (Retrieval-Augmented Generation)
    #[cfg(all(feature = "ai-enhanced", feature = "qwen-integration"))]
    #[tool(
        description = "Ask questions about code and get AI answers with citations (5-30s). Examples: 'How does auth work?', 'Explain data flow'. Use for: complex questions requiring context. SLOW - use enhanced_search for simpler queries. Required: question. Optional: max_results (default 5), streaming (default false)."
    )]
    async fn codebase_qa(
        &self,
        params: Parameters<CodebaseQaRequest>,
    ) -> Result<CallToolResult, McpError> {
        let request = params.0;

        let config = codegraph_ai::rag::engine::RAGEngineConfig {
            max_results: request.max_results.unwrap_or(5), // Reduced from 10 for faster responses
            graph_neighbor_expansion: true,
            neighbor_hops: 2,
            streaming_chunk_chars: 64,
            streaming_min_delay_ms: 10,
        };

        let graph_instance = self.graph.clone();

        let rag_engine = codegraph_ai::rag::engine::RAGEngine::new(graph_instance, config);

        match rag_engine.answer(&request.question).await {
            Ok(answer) => {
                let response = serde_json::json!({
                    "query_id": answer.query_id,
                    "question": request.question,
                    "answer": answer.answer,
                    "confidence": answer.confidence,
                    "citations": answer.citations.iter().map(|c| serde_json::json!({
                        "node_id": c.node_id,
                        "name": c.name,
                        "file_path": c.file_path,
                        "line": c.line,
                        "end_line": c.end_line,
                        "relevance": c.relevance
                    })).collect::<Vec<_>>(),
                    "processing_time_ms": answer.processing_time_ms,
                    "rag_method": "hybrid_retrieval_with_graph_expansion",
                    "intelligence_level": "conversational_ai",
                    "tool_type": "revolutionary_rag"
                });

                Ok(CallToolResult::success(vec![Content::text(
                    serde_json::to_string_pretty(&response)
                        .unwrap_or_else(|_| "Error formatting RAG response".to_string()),
                )]))
            }
            Err(e) => {
                let fallback = serde_json::json!({
                    "question": request.question,
                    "error": format!("RAG processing failed: {}", e),
                    "fallback_mode": true,
                    "note": "Ensure codebase is indexed with edge processing enabled"
                });
                Ok(CallToolResult::success(vec![Content::text(
                    serde_json::to_string_pretty(&fallback)
                        .unwrap_or_else(|_| "Error formatting fallback response".to_string()),
                )]))
            }
        }
    }

    #[cfg(not(all(feature = "ai-enhanced", feature = "qwen-integration")))]
    #[tool(
        description = "codebase_qa (disabled – enable `ai-enhanced` and `qwen-integration` features to activate this tool)."
    )]
    async fn codebase_qa(
        &self,
        params: Parameters<CodebaseQaRequest>,
    ) -> Result<CallToolResult, McpError> {
        let _ = params;
        Err(McpError::invalid_request(
            "codebase_qa tool requires the `ai-enhanced` and `qwen-integration` features to be enabled",
            None,
        ))
    }

    /// REVOLUTIONARY: AI-powered code documentation generation with graph context
    #[cfg(all(feature = "ai-enhanced", feature = "qwen-integration"))]
    #[tool(
        description = "Generate AI documentation for functions/classes (10-45s). Includes dependencies, usage patterns, examples. Use for: creating comprehensive docs. VERY SLOW - consider manual docs for simple cases. Required: target_name. Optional: file_path, style (comprehensive/concise/tutorial, default comprehensive)."
    )]
    async fn code_documentation(
        &self,
        params: Parameters<CodeDocumentationRequest>,
    ) -> Result<CallToolResult, McpError> {
        let request = params.0;

        eprintln!(
            "📘 Generating documentation for '{}' (style: {})",
            request.target_name, request.style
        );

        // Use the existing graph database from the server
        let config = codegraph_ai::rag::engine::RAGEngineConfig {
            max_results: 15, // More context for comprehensive documentation
            graph_neighbor_expansion: true,
            neighbor_hops: 3, // Deeper context for documentation
            streaming_chunk_chars: 128,
            streaming_min_delay_ms: 5,
        };

        // Use the shared graph instance from the server (fixes lock conflict)
        let graph_instance = self.graph.clone();

        let rag_engine = codegraph_ai::rag::engine::RAGEngine::new(graph_instance, config);

        // Craft documentation query based on target and style
        let doc_query = format!(
            "Generate {} documentation for '{}' including its purpose, parameters, return values, usage examples, dependencies, and architectural context",
            request.style,
            request.target_name
        );

        // Prepare shared server state (graph + optional Qwen client)
        let server_state = crate::server::ServerState {
            graph: self.graph.clone(),
            qwen_client: self.get_qwen_client().await,
        };

        // First, gather RAG insights so we always have citations/fallback ready
        let rag_result = rag_engine.answer(&doc_query).await;
        if rag_result.is_ok() {
            eprintln!("🗂 RAG context assembled for '{}'", request.target_name);
        }
        let citations: Vec<serde_json::Value> = rag_result
            .as_ref()
            .ok()
            .map(|answer| {
                answer
                    .citations
                    .iter()
                    .map(|c| {
                        serde_json::json!({
                            "file": c.file_path,
                            "line": c.line,
                            "relevance": c.relevance,
                            "context": c.name
                        })
                    })
                    .collect()
            })
            .unwrap_or_else(Vec::new);

        // If Qwen is available, use it for richer documentation synthesis
        if let Some(ref qwen_client) = server_state.qwen_client {
            let raw_limit = ((qwen_client.config.context_window as f32) * 0.5) as usize;
            let context_limit = raw_limit.clamp(2048, 65536);
            if let Ok(context) = crate::server::build_comprehensive_context(
                &server_state,
                &doc_query,
                context_limit.max(1024),
            )
            .await
            {
                eprintln!(
                    "🤖 Qwen synthesis in progress for '{}' (context ~{} chars)",
                    request.target_name,
                    context.len()
                );
                match qwen_client.analyze_codebase(&doc_query, &context).await {
                    Ok(doc_result) => {
                        let response = serde_json::json!({
                            "target_name": request.target_name,
                            "documentation": doc_result.text,
                            "confidence": doc_result.confidence_score,
                            "style": request.style,
                            "sources": citations,
                            "processing_time_ms": doc_result.processing_time.as_millis() as u64,
                            "generation_method": "qwen_documentation",
                            "graph_context_used": true,
                            "tool_type": "revolutionary_documentation",
                            "model_performance": {
                                "model_used": doc_result.model_used,
                                "context_tokens": doc_result.context_tokens,
                                "completion_tokens": doc_result.completion_tokens,
                                "processing_time_ms": doc_result.processing_time.as_millis(),
                            }
                        });

                        eprintln!(
                            "✅ Qwen documentation ready for '{}' in {}ms",
                            request.target_name,
                            doc_result.processing_time.as_millis()
                        );

                        return Ok(CallToolResult::success(vec![Content::text(
                            serde_json::to_string_pretty(&response).unwrap_or_else(|_| {
                                "Error formatting documentation response".to_string()
                            }),
                        )]));
                    }
                    Err(e) => {
                        eprintln!("⚠️ Qwen documentation generation failed: {}", e);
                    }
                }
            }
        }

        // Fallback to heuristic RAG output if Qwen is unavailable or failed
        match rag_result {
            Ok(answer) => {
                let response = serde_json::json!({
                    "target_name": request.target_name,
                    "documentation": answer.answer,
                    "confidence": answer.confidence,
                    "style": request.style,
                    "sources": answer.citations.iter().map(|c| serde_json::json!({
                        "file": c.file_path,
                        "line": c.line,
                        "relevance": c.relevance,
                        "context": c.name
                    })).collect::<Vec<_>>(),
                    "processing_time_ms": answer.processing_time_ms,
                    "generation_method": "ai_powered_rag_documentation",
                    "graph_context_used": true,
                    "tool_type": "revolutionary_documentation"
                });

                Ok(CallToolResult::success(vec![Content::text(
                    serde_json::to_string_pretty(&response)
                        .unwrap_or_else(|_| "Error formatting documentation response".to_string()),
                )]))
            }
            Err(e) => {
                let fallback = serde_json::json!({
                    "target_name": request.target_name,
                    "error": format!("Documentation generation failed: {}", e),
                    "fallback_mode": true,
                    "note": "Ensure target exists in codebase and is properly indexed"
                });
                Ok(CallToolResult::success(vec![Content::text(
                    serde_json::to_string_pretty(&fallback)
                        .unwrap_or_else(|_| "Error formatting fallback response".to_string()),
                )]))
            }
        }
    }

    #[cfg(not(all(feature = "ai-enhanced", feature = "qwen-integration")))]
    #[tool(
        description = "code_documentation (disabled – enable `ai-enhanced` and `qwen-integration` features to activate this tool)."
    )]
    async fn code_documentation(
        &self,
        params: Parameters<CodeDocumentationRequest>,
    ) -> Result<CallToolResult, McpError> {
        let _ = params;
        Err(McpError::invalid_request(
            "code_documentation tool requires the `ai-enhanced` and `qwen-integration` features to be enabled",
            None,
        ))
    }

    // /// Read file contents with optional line range for precise code analysis (DISABLED - overlaps with client tools)
    // #[tool(description = "Read file contents with optional line range specification")]
    // async fn code_read(&self, params: Parameters<CodeReadRequest>) -> Result<CallToolResult, McpError> {
    //     let request = params.0;
    //     match std::fs::read_to_string(&request.path) {
    //         Ok(text) => {
    //             let total_lines = text.lines().count();
    //             let end_line = request.end.unwrap_or(total_lines);
    //             let lines: Vec<_> = text
    //                 .lines()
    //                 .enumerate()
    //                 .skip(request.start.saturating_sub(1))
    //                 .take(end_line.saturating_sub(request.start.saturating_sub(1)))
    //                 .map(|(i, line)| serde_json::json!({
    //                     "line": i + 1,
    //                     "text": line
    //                 }))
    //                 .collect();

    //             let result = serde_json::json!({
    //                 "path": request.path,
    //                 "start": request.start,
    //                 "end": end_line,
    //                 "total_lines": total_lines,
    //                 "lines": lines
    //             });

    //             Ok(CallToolResult::success(vec![Content::text(
    //                 serde_json::to_string_pretty(&result)
    //                     .unwrap_or_else(|_| "Error formatting file content".to_string())
    //             )]))
    //         }
    //         Err(e) => Err(McpError {
    //             code: rmcp::model::ErrorCode(-32603),
    //             message: format!("Failed to read file {}: {}", request.path, e).into(),
    //             data: None,
    //         })
    //     }
    // }

    // /// Intelligent find-and-replace with dry-run support for safe code modifications (DISABLED - overlaps with client tools)
    // #[tool(description = "Find and replace text in files with dry-run support for safe modifications")]
    // async fn code_patch(&self, params: Parameters<CodePatchRequest>) -> Result<CallToolResult, McpError> {
    //     let request = params.0;
    //     match std::fs::read_to_string(&request.path) {
    //         Ok(text) => {
    //             let replacements = text.matches(&request.find).count();

    //             if request.dry_run {
    //                 let result = serde_json::json!({
    //                     "path": request.path,
    //                     "find": request.find,
    //                     "replace": request.replace,
    //                     "replacements": replacements,
    //                     "dry_run": true,
    //                     "success": true
    //                 });

    //                 Ok(CallToolResult::success(vec![Content::text(
    //                     serde_json::to_string_pretty(&result)
    //                         .unwrap_or_else(|_| "Error formatting patch result".to_string())
    //                 )]))
    //             } else {
    //                 let new_text = text.replace(&request.find, &request.replace);
    //                 match std::fs::write(&request.path, new_text) {
    //                     Ok(_) => {
    //                         let result = serde_json::json!({
    //                             "path": request.path,
    //                             "find": request.find,
    //                             "replace": request.replace,
    //                             "replacements": replacements,
    //                             "dry_run": false,
    //                             "success": true
    //                         });

    //                         Ok(CallToolResult::success(vec![Content::text(
    //                             serde_json::to_string_pretty(&result)
    //                                 .unwrap_or_else(|_| "Error formatting patch result".to_string())
    //                         )]))
    //                     }
    //                     Err(e) => Err(McpError {
    //                         code: rmcp::model::ErrorCode(-32603),
    //                         message: format!("Failed to write to file {}: {}", request.path, e).into(),
    //                         data: None,
    //                     })
    //                 }
    //             }
    //         }
    //         Err(e) => Err(McpError {
    //             code: rmcp::model::ErrorCode(-32603),
    //             message: format!("Failed to read file {}: {}", request.path, e).into(),
    //             data: None,
    //         })
    //     }
    // }

    // /// Execute cargo tests with package filtering and argument support (DISABLED - redundant for development)
    // #[tool(description = "Run cargo tests with optional package selection and custom arguments")]
    // async fn test_run(&self, params: Parameters<TestRunRequest>) -> Result<CallToolResult, McpError> {
    //     let request = params.0;
    //     use tokio::process::Command;

    //     let mut args = vec!["test".to_string()];
    //     if let Some(pkg) = &request.package {
    //         args.push("-p".to_string());
    //         args.push(pkg.clone());
    //     }
    //     if let Some(extra_args) = &request.args {
    //         args.extend(extra_args.clone());
    //     }

    //     let output = Command::new("cargo")
    //         .args(&args)
    //         .output()
    //         .await
    //         .map_err(|e| McpError {
    //             code: rmcp::model::ErrorCode(-32603),
    //             message: format!("Failed to execute cargo test: {}", e).into(),
    //             data: None,
    //         })?;

    //     let status = output.status.code().unwrap_or(-1);
    //     let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    //     let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    //     let result = serde_json::json!({
    //         "command": format!("cargo {}", args.join(" ")),
    //         "status": status,
    //         "success": status == 0,
    //         "stdout": stdout,
    //         "stderr": stderr
    //     });

    //     Ok(CallToolResult::success(vec![Content::text(
    //         serde_json::to_string_pretty(&result)
    //             .unwrap_or_else(|_| "Error formatting test result".to_string())
    //     )]))
    // }

    /// Deep AI-powered analysis of your entire codebase architecture and system design
    #[tool(
        description = "⚠️ VERY SLOW (30-120s): Deep architectural analysis of entire codebase. Explains system design, components, architecture. Use ONLY for: major architectural questions, system-wide analysis. For specific code use enhanced_search. Required: query. Optional: max_context_tokens (default 20000, max 80000)."
    )]
    async fn semantic_intelligence(
        &self,
        params: Parameters<SemanticIntelligenceRequest>,
    ) -> Result<CallToolResult, McpError> {
        let request = params.0;

        #[cfg(feature = "qwen-integration")]
        {
            // Use the existing graph database from the server
            let state = crate::server::ServerState {
                graph: self.graph.clone(),
                qwen_client: self.get_qwen_client().await,
            };

            // Call semantic intelligence with Qwen analysis
            match crate::server::semantic_intelligence(
                &state,
                serde_json::json!({
                    "query": request.query,
                    "task_type": request.task_type,
                    "max_context_tokens": request.max_context_tokens
                }),
            )
            .await
            {
                Ok(intelligence_results) => Ok(CallToolResult::success(vec![Content::text(
                    serde_json::to_string_pretty(&intelligence_results).unwrap_or_else(|_| {
                        "Error formatting semantic intelligence results".to_string()
                    }),
                )])),
                Err(e) => {
                    // Fallback if AI analysis fails
                    let fallback = serde_json::json!({
                        "query": request.query,
                        "task_type": request.task_type,
                        "max_context_tokens": request.max_context_tokens,
                        "error": format!("Semantic intelligence failed: {}", e),
                        "fallback_mode": true,
                        "note": "Ensure codebase is indexed and Qwen2.5-Coder is available"
                    });
                    Ok(CallToolResult::success(vec![Content::text(
                        serde_json::to_string_pretty(&fallback)
                            .unwrap_or_else(|_| "Error formatting fallback results".to_string()),
                    )]))
                }
            }
        }
        #[cfg(not(feature = "qwen-integration"))]
        {
            Err(McpError {
                code: rmcp::model::ErrorCode(-32601),
                message: "Semantic intelligence requires qwen-integration feature".into(),
                data: None,
            })
        }
    }

    /// Predict what code will break before you modify a function or class
    #[tool(
        description = "Predict refactoring impact with AI (3-15s). Shows dependent code and breakage risks. Use for: pre-refactoring safety checks, understanding blast radius. Required: target_function, file_path. Optional: change_type (modify/delete/rename, default modify)."
    )]
    async fn impact_analysis(
        &self,
        params: Parameters<ImpactAnalysisRequest>,
    ) -> Result<CallToolResult, McpError> {
        let request = params.0;

        #[cfg(feature = "qwen-integration")]
        {
            // Use the existing graph database from the server
            let state = crate::server::ServerState {
                graph: self.graph.clone(),
                qwen_client: self.get_qwen_client().await,
            };

            // Call impact analysis with Qwen-powered dependency analysis
            match crate::server::impact_analysis(
                &state,
                serde_json::json!({
                    "target_function": request.target_function,
                    "file_path": request.file_path,
                    "change_type": request.change_type
                }),
            )
            .await
            {
                Ok(impact_results) => Ok(CallToolResult::success(vec![Content::text(
                    serde_json::to_string_pretty(&impact_results)
                        .unwrap_or_else(|_| "Error formatting impact analysis results".to_string()),
                )])),
                Err(e) => {
                    // Fallback if impact analysis fails
                    let fallback = serde_json::json!({
                        "target_function": request.target_function,
                        "file_path": request.file_path,
                        "change_type": request.change_type,
                        "error": format!("Impact analysis failed: {}", e),
                        "fallback_mode": true,
                        "note": "Ensure codebase is indexed and target function exists"
                    });
                    Ok(CallToolResult::success(vec![Content::text(
                        serde_json::to_string_pretty(&fallback)
                            .unwrap_or_else(|_| "Error formatting fallback results".to_string()),
                    )]))
                }
            }
        }
        #[cfg(not(feature = "qwen-integration"))]
        {
            Err(McpError {
                code: rmcp::model::ErrorCode(-32601),
                message: "Impact analysis requires qwen-integration feature".into(),
                data: None,
            })
        }
    }

    // === AGENTIC MCP TOOLS ===
    // These tools use AgenticOrchestrator for multi-step graph analysis workflows
    // with automatic tier detection based on CODEGRAPH_CONTEXT_WINDOW or config

    /// Agentic code search with multi-step graph exploration
    #[cfg(feature = "ai-enhanced")]
    #[tool(
        description = "Multi-step code search using agentic graph exploration. The LLM autonomously decides which graph analysis tools to call based on your query. Use for: finding code patterns, exploring unfamiliar codebases, discovering relationships. Required: query. Note: Uses automatic tier detection based on LLM context window."
    )]
    async fn agentic_code_search(
        &self,
        peer: Peer<RoleServer>,
        meta: Meta,
        params: Parameters<SearchRequest>,
    ) -> Result<CallToolResult, McpError> {
        #[cfg(feature = "ai-enhanced")]
        {
            let request = params.0;
            self.execute_agentic_workflow(
                crate::AnalysisType::CodeSearch,
                &request.query,
                peer,
                meta,
            )
            .await
        }
    }

    /// Agentic dependency analysis with multi-step exploration
    #[cfg(feature = "ai-enhanced")]
    #[tool(
        description = "Multi-step dependency analysis using agentic graph exploration. The LLM autonomously explores dependency chains and impact. Use for: understanding dependency relationships, impact analysis. Required: query."
    )]
    async fn agentic_dependency_analysis(
        &self,
        peer: Peer<RoleServer>,
        meta: Meta,
        params: Parameters<SearchRequest>,
    ) -> Result<CallToolResult, McpError> {
        let request = params.0;
        self.execute_agentic_workflow(
            crate::AnalysisType::DependencyAnalysis,
            &request.query,
            peer,
            meta,
        )
        .await
    }

    /// Agentic call chain analysis with multi-step tracing
    #[cfg(feature = "ai-enhanced")]
    #[tool(
        description = "Multi-step call chain analysis using agentic graph exploration. The LLM autonomously traces execution paths and call sequences. Use for: understanding execution flow, debugging call chains. Required: query."
    )]
    async fn agentic_call_chain_analysis(
        &self,
        peer: Peer<RoleServer>,
        meta: Meta,
        params: Parameters<SearchRequest>,
    ) -> Result<CallToolResult, McpError> {
        let request = params.0;
        self.execute_agentic_workflow(
            crate::AnalysisType::CallChainAnalysis,
            &request.query,
            peer,
            meta,
        )
        .await
    }

    /// Agentic architecture analysis with multi-step system exploration
    #[cfg(feature = "ai-enhanced")]
    #[tool(
        description = "Multi-step architecture analysis using agentic graph exploration. The LLM autonomously analyzes architectural patterns and system design. Use for: understanding system architecture, design patterns. Required: query."
    )]
    async fn agentic_architecture_analysis(
        &self,
        peer: Peer<RoleServer>,
        meta: Meta,
        params: Parameters<SearchRequest>,
    ) -> Result<CallToolResult, McpError> {
        let request = params.0;
        self.execute_agentic_workflow(
            crate::AnalysisType::ArchitectureAnalysis,
            &request.query,
            peer,
            meta,
        )
        .await
    }

    /// Agentic API surface analysis with multi-step exploration
    #[cfg(feature = "ai-enhanced")]
    #[tool(
        description = "Multi-step API surface analysis using agentic graph exploration. The LLM autonomously analyzes public interfaces and contracts. Use for: understanding API design, public interfaces. Required: query."
    )]
    async fn agentic_api_surface_analysis(
        &self,
        peer: Peer<RoleServer>,
        meta: Meta,
        params: Parameters<SearchRequest>,
    ) -> Result<CallToolResult, McpError> {
        let request = params.0;
        self.execute_agentic_workflow(
            crate::AnalysisType::ApiSurfaceAnalysis,
            &request.query,
            peer,
            meta,
        )
        .await
    }

    /// Agentic context builder with multi-step comprehensive context gathering
    #[cfg(feature = "ai-enhanced")]
    #[tool(
        description = "Multi-step context building using agentic graph exploration. The LLM autonomously gathers comprehensive context for code generation. Use for: preparing context for code generation, understanding code context. Required: query."
    )]
    async fn agentic_context_builder(
        &self,
        peer: Peer<RoleServer>,
        meta: Meta,
        params: Parameters<SearchRequest>,
    ) -> Result<CallToolResult, McpError> {
        let request = params.0;
        self.execute_agentic_workflow(
            crate::AnalysisType::ContextBuilder,
            &request.query,
            peer,
            meta,
        )
        .await
    }

    /// Agentic semantic question answering with multi-step exploration
    #[cfg(feature = "ai-enhanced")]
    #[tool(
        description = "Multi-step semantic question answering using agentic graph exploration. The LLM autonomously explores the codebase to answer complex questions. Use for: answering complex codebase questions, semantic analysis. Required: query."
    )]
    async fn agentic_semantic_question(
        &self,
        peer: Peer<RoleServer>,
        meta: Meta,
        params: Parameters<SearchRequest>,
    ) -> Result<CallToolResult, McpError> {
        let request = params.0;
        self.execute_agentic_workflow(
            crate::AnalysisType::SemanticQuestion,
            &request.query,
            peer,
            meta,
        )
        .await
    }
}

impl CodeGraphMCPServer {
    /// Initialize Qwen integration (separate from tool router)
    pub async fn initialize_qwen(&self) {
        #[cfg(feature = "qwen-integration")]
        {
            // Initialize intelligent cache
            let cache_config = CacheConfig::default();
            init_cache(cache_config);

            let config = QwenConfig::default();
            let client = QwenClient::new(config.clone());

            match client.check_availability().await {
                Ok(true) => {
                    eprintln!("✅ Qwen2.5-Coder-14B-128K available for CodeGraph intelligence");
                    let mut qwen_lock = self.qwen_client.lock().await;
                    *qwen_lock = Some(client);
                }
                Ok(false) => {
                    eprintln!(
                        "⚠️ Qwen2.5-Coder model not found. Install with: ollama pull {}",
                        config.model_name
                    );
                }
                Err(e) => {
                    eprintln!("❌ Failed to connect to Qwen2.5-Coder: {}", e);
                }
            }
        }
        #[cfg(not(feature = "qwen-integration"))]
        {
            eprintln!("💡 Qwen integration not enabled in this build");
        }
    }

    /// Get cached Qwen client if available
    #[cfg(feature = "qwen-integration")]
    pub async fn get_qwen_client(&self) -> Option<QwenClient> {
        let qwen_lock = self.qwen_client.lock().await;
        qwen_lock.clone()
    }

    /// Auto-detect context tier from environment or config
    #[cfg(feature = "ai-enhanced")]
    fn detect_context_tier() -> crate::ContextTier {
        // Try CODEGRAPH_CONTEXT_WINDOW env var first
        if let Ok(context_window_str) = std::env::var("CODEGRAPH_CONTEXT_WINDOW") {
            if let Ok(context_window) = context_window_str.parse::<usize>() {
                return crate::ContextTier::from_context_window(context_window);
            }
        }

        // Fall back to config
        match codegraph_core::config_manager::ConfigManager::load() {
            Ok(config_manager) => {
                let config = config_manager.config();
                crate::ContextTier::from_context_window(config.llm.context_window)
            }
            Err(_) => {
                // Default to Medium tier if config can't be loaded
                eprintln!("⚠️ Failed to load config, defaulting to Medium context tier");
                crate::ContextTier::Medium
            }
        }
    }

    /// Creates a progress notification callback that sends MCP protocol notifications
    #[cfg(feature = "ai-enhanced")]
    fn create_progress_callback(
        peer: Peer<RoleServer>,
        progress_token: ProgressToken,
    ) -> Arc<dyn Fn(f64, Option<f64>) -> BoxFuture<'static, ()> + Send + Sync> {
        Arc::new(move |progress, total| {
            let peer = peer.clone();
            let progress_token = progress_token.clone();

            Box::pin(async move {
                let notification = ProgressNotification {
                    method: Default::default(),
                    params: ProgressNotificationParam {
                        progress_token: progress_token.clone(),
                        progress,
                        total,
                        message: None, // Optional progress message
                    },
                    extensions: Default::default(),
                };

                // Ignore notification errors (non-blocking)
                let _ = peer
                    .send_notification(ServerNotification::ProgressNotification(notification))
                    .await;
            })
        })
    }

    /// Execute agentic workflow with automatic tier detection and prompt selection
    #[cfg(feature = "ai-enhanced")]
    async fn execute_agentic_workflow(
        &self,
        analysis_type: crate::AnalysisType,
        query: &str,
        peer: Peer<RoleServer>,
        meta: Meta,
    ) -> Result<CallToolResult, McpError> {
        use crate::{AgenticOrchestrator, PromptSelector};
        use codegraph_ai::llm_factory::LLMProviderFactory;
        use codegraph_graph::GraphFunctions;
        use std::sync::Arc;

        // Auto-detect context tier
        let tier = Self::detect_context_tier();

        eprintln!("🎯 Agentic {} (tier={:?})", analysis_type.as_str(), tier);

        // Extract progress token from meta or generate one
        let progress_token = meta.get_progress_token().unwrap_or_else(|| {
            ProgressToken(NumberOrString::String(
                format!("agentic-{}", Uuid::new_v4()).into(),
            ))
        });

        // Create progress callback
        let progress_callback = Some(Self::create_progress_callback(peer, progress_token));

        // Load config for LLM provider
        let config_manager =
            codegraph_core::config_manager::ConfigManager::load().map_err(|e| McpError {
                code: rmcp::model::ErrorCode(-32603),
                message: format!("Failed to load config: {}", e).into(),
                data: None,
            })?;
        let config = config_manager.config();

        // Create LLM provider
        let llm_provider =
            LLMProviderFactory::create_from_config(&config.llm).map_err(|e| McpError {
                code: rmcp::model::ErrorCode(-32603),
                message: format!("Failed to create LLM provider: {}", e).into(),
                data: None,
            })?;

        // Create GraphFunctions with SurrealDB connection
        // We'll use the SurrealDbStorage to create the connection
        let graph_functions = {
            use codegraph_graph::SurrealDbStorage;

            let surrealdb_config = codegraph_graph::SurrealDbConfig {
                connection: std::env::var("SURREALDB_URL")
                    .unwrap_or_else(|_| "ws://localhost:3004".to_string()),
                namespace: std::env::var("SURREALDB_NAMESPACE")
                    .unwrap_or_else(|_| "codegraph".to_string()),
                database: std::env::var("SURREALDB_DATABASE")
                    .unwrap_or_else(|_| "main".to_string()),
                username: std::env::var("SURREALDB_USERNAME").ok(),
                password: std::env::var("SURREALDB_PASSWORD").ok(),
                strict_mode: false,
                auto_migrate: false, // Don't auto-migrate for agentic tools
                cache_enabled: false,
            };

            // Create SurrealDbStorage which handles connection setup
            let storage = SurrealDbStorage::new(surrealdb_config)
                .await
                .map_err(|e| McpError {
                    code: rmcp::model::ErrorCode(-32603),
                    message: format!("Failed to create SurrealDB storage: {}. Ensure SurrealDB is running on ws://localhost:3004", e).into(),
                    data: None,
                })?;

            // Get the database connection from storage and create GraphFunctions
            Arc::new(GraphFunctions::new(storage.db()))
        };

        // Create GraphToolExecutor
        let tool_executor = Arc::new(crate::GraphToolExecutor::new(graph_functions));

        // Get max_tokens override from config if set
        let max_tokens_override = config.llm.mcp_code_agent_max_output_tokens;

        // Create AgenticOrchestrator with config override and progress callback
        let orchestrator = AgenticOrchestrator::new_with_override(
            llm_provider,
            tool_executor,
            tier,
            max_tokens_override,
            progress_callback,
        );

        // Get tier-appropriate prompt from PromptSelector
        let prompt_selector = PromptSelector::new();
        let system_prompt = prompt_selector
            .select_prompt(analysis_type, tier)
            .map_err(|e| McpError {
                code: rmcp::model::ErrorCode(-32603),
                message: format!("Failed to select prompt: {}", e).into(),
                data: None,
            })?;

        // Execute agentic workflow
        let result = orchestrator
            .execute(query, system_prompt)
            .await
            .map_err(|e| McpError {
                code: rmcp::model::ErrorCode(-32603),
                message: format!("Agentic workflow failed: {}", e).into(),
                data: None,
            })?;

        // Format result as JSON
        let response_json = serde_json::json!({
            "analysis_type": analysis_type.as_str(),
            "tier": format!("{:?}", tier),
            "query": query,
            "final_answer": result.final_answer,
            "total_steps": result.total_steps,
            "duration_ms": result.duration_ms,
            "total_tokens": result.total_tokens,
            "completed_successfully": result.completed_successfully,
            "termination_reason": result.termination_reason,
            "steps": result.steps,
            "tool_call_stats": result.tool_call_stats(),
        });

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&response_json)
                .unwrap_or_else(|_| "Error formatting agentic result".to_string()),
        )]))
    }
}

/// Official MCP ServerHandler implementation (following Counter pattern)
#[tool_handler]
impl ServerHandler for CodeGraphMCPServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            instructions: Some(
                "CodeGraph provides revolutionary AI codebase intelligence through local SOTA models.
    \n\
                🚀 Revolutionary Features:
    \
                • Enhanced semantic search with Qwen2.5-Coder-14B-128K analysis
    \
                • Revolutionary impact prediction before code changes
    \
                • Team intelligence and pattern detection
    \
                • 128K context window comprehensive analysis
    \
                • Complete local-first processing with zero external dependencies
    \
                • Performance optimized for high-memory systems
    \n\
                This is the world's most advanced local-first AI development platform.
    \n\
                💡 Getting Started:
    \
                1. Navigate to your project directory
    \
                2. Run: codegraph init .
    \
                3. Run: codegraph index . --recursive
    \
                4. Experience revolutionary AI codebase intelligence!".into()
            ),
            capabilities: ServerCapabilities::builder()
                .enable_tools()
                .enable_logging()
                .build(),
            ..Default::default()
        }
    }
}
