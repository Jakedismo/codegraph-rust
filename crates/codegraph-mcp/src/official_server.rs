/// Clean Official MCP SDK Implementation for CodeGraph
/// Following exact Counter pattern from rmcp SDK documentation

use rmcp::{
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{CallToolResult, Content, ServerCapabilities, ServerInfo},
    tool, tool_handler, tool_router,
    ErrorData as McpError,
    ServerHandler,
};
use schemars::JsonSchema;
use serde::Deserialize;
use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;

#[cfg(feature = "qwen-integration")]
use crate::qwen::{QwenClient, QwenConfig};
#[cfg(feature = "qwen-integration")]
use crate::cache::{CacheConfig, init_cache};

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

fn default_limit() -> usize { 10 }

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

fn default_neighbor_limit() -> usize { 20 }

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

fn default_depth() -> usize { 2 }
fn default_traverse_limit() -> usize { 100 }

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
    /// Maximum context tokens to use from 128K available (default: 80000)
    #[serde(default = "default_max_context_tokens")]
    max_context_tokens: usize,
}

fn default_task_type() -> String { "semantic_search".to_string() }
fn default_max_context_tokens() -> usize { 80000 }

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

fn default_change_type() -> String { "modify".to_string() }

#[derive(Deserialize, JsonSchema)]
struct EmptyRequest {
    /// No parameters required
    #[serde(default)]
    _unused: Option<String>,
}

/// REVOLUTIONARY: Request for intelligent codebase Q&A using RAG
#[cfg(all(feature = "ai-enhanced", feature = "qwen-integration"))]
#[derive(Deserialize, JsonSchema)]
struct CodebaseQaRequest {
    /// Natural language question about the codebase
    question: String,
    /// Maximum number of results to consider (default: 10)
    #[serde(default)]
    max_results: Option<usize>,
    /// Enable streaming response (default: false for MCP compatibility)
    #[serde(default)]
    streaming: Option<bool>,
}

/// REVOLUTIONARY: Request for intelligent code documentation generation
#[cfg(all(feature = "ai-enhanced", feature = "qwen-integration"))]
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

#[cfg(all(feature = "ai-enhanced", feature = "qwen-integration"))]
fn default_doc_style() -> String { "comprehensive".to_string() }

/// Clean CodeGraph MCP server following official Counter pattern
#[derive(Clone)]
pub struct CodeGraphMCPServer {
    /// Graph database for revolutionary AI tools
    graph: Arc<tokio::sync::Mutex<codegraph_graph::CodeGraph>>,
    /// Simple counter for demonstration
    counter: Arc<Mutex<i32>>,
    /// Official MCP tool router (required by macros)
    tool_router: ToolRouter<Self>,
}

#[tool_router]
impl CodeGraphMCPServer {
    pub fn new() -> Self {
        // Create read-only database connection for concurrent multi-agent access
        let current_dir = std::env::current_dir()
            .unwrap_or_else(|_| std::path::PathBuf::from("."));
        let db_path = current_dir.join(".codegraph/db");

        // CRITICAL FIX: Use same database constructor as CLI (not read-only)
        // The CLI uses CodeGraph::new() which works, read-only connections had access issues
        let graph = codegraph_graph::CodeGraph::new()
            .unwrap_or_else(|_| panic!("Failed to initialize CodeGraph database"));

        Self {
            graph: Arc::new(tokio::sync::Mutex::new(graph)),
            counter: Arc::new(Mutex::new(0)),
            tool_router: Self::tool_router(),
        }
    }

    /// Initialize with existing graph database from working directory
    pub async fn new_with_graph() -> Result<Self, String> {
        // Try to open existing database first, create new if needed
        let graph = codegraph_graph::CodeGraph::new()
            .map_err(|e| format!("Failed to initialize CodeGraph database: {}", e))?;

        Ok(Self {
            graph: Arc::new(tokio::sync::Mutex::new(graph)),
            counter: Arc::new(Mutex::new(0)),
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
    #[tool(description = "Search your codebase with AI analysis. Finds code patterns, architectural insights, and team conventions. Use when you need intelligent analysis of search results. Required: query (what to search for). Optional: limit (max results, default 10).")]
    async fn enhanced_search(&self, params: Parameters<SearchRequest>) -> Result<CallToolResult, McpError> {
        let request = params.0; // Extract the inner value

        #[cfg(feature = "qwen-integration")]
        {
            // 1. Perform vector search first using shared database connection
            let graph = self.graph.lock().await;
            match crate::server::bin_search_with_scores_shared(
                request.query.clone(),
                None, // No path filtering for enhanced search
                None, // No language filtering for enhanced search
                request.limit * 2, // Get more results for AI analysis
                &graph
            ).await {
                Ok(search_results) => {
                    // 2. Use the existing graph database from the server
                    let state = crate::server::ServerState {
                        graph: self.graph.clone(),
                        qwen_client: crate::server::init_qwen_client().await,
                    };

                    // 3. Call enhanced search with Qwen analysis
                    match crate::server::enhanced_search(&state, serde_json::json!({
                        "query": request.query,
                        "limit": request.limit
                    })).await {
                        Ok(enhanced_results) => Ok(CallToolResult::success(vec![Content::text(
                            serde_json::to_string_pretty(&enhanced_results)
                                .unwrap_or_else(|_| "Error formatting enhanced search results".to_string())
                        )])),
                        Err(e) => {
                            // Fallback to basic search results if AI fails
                            let fallback = serde_json::json!({
                                "search_results": search_results["results"],
                                "ai_analysis": format!("AI analysis failed: {}", e),
                                "query": request.query,
                                "fallback_mode": true
                            });
                            Ok(CallToolResult::success(vec![Content::text(
                                serde_json::to_string_pretty(&fallback)
                                    .unwrap_or_else(|_| "Error formatting fallback results".to_string())
                            )]))
                        }
                    }
                },
                Err(e) => Err(McpError {
                    code: rmcp::model::ErrorCode(-32603),
                    message: format!("Vector search failed: {}. Ensure codebase is indexed.", e).into(),
                    data: None,
                })
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
    #[tool(description = "Analyze your team's coding patterns and conventions. Detects naming conventions, code organization patterns, error handling styles, and quality metrics. Use to understand team standards or onboard new developers. No parameters required.")]
    async fn pattern_detection(&self, params: Parameters<EmptyRequest>) -> Result<CallToolResult, McpError> {
        let _request = params.0; // Extract the inner value (unused)

        // Use the existing graph database from the server
        let state = crate::server::ServerState {
            graph: self.graph.clone(),
            #[cfg(feature = "qwen-integration")]
            qwen_client: crate::server::init_qwen_client().await,
        };

        // Call pattern detection with team intelligence analysis
        match crate::server::pattern_detection(&state, serde_json::json!({})).await {
            Ok(pattern_results) => Ok(CallToolResult::success(vec![Content::text(
                serde_json::to_string_pretty(&pattern_results)
                    .unwrap_or_else(|_| "Error formatting pattern detection results".to_string())
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
                        .unwrap_or_else(|_| "Error formatting fallback results".to_string())
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
    #[tool(description = "Fast vector similarity search to find code similar to your query. Returns raw search results without AI analysis (faster than enhanced_search). Use for quick code discovery. Required: query (what to find). Optional: paths (filter by directories), langs (filter by languages), limit (max results, default 10).")]
    async fn vector_search(&self, params: Parameters<VectorSearchRequest>) -> Result<CallToolResult, McpError> {
        let request = params.0;
        let graph = self.graph.lock().await;
        match crate::server::bin_search_with_scores_shared(
            request.query.clone(),
            request.paths,
            request.langs,
            request.limit,
            &graph
        ).await {
            Ok(results) => Ok(CallToolResult::success(vec![Content::text(
                serde_json::to_string_pretty(&results)
                    .unwrap_or_else(|_| "Error formatting search results".to_string())
            )])),
            Err(e) => Err(McpError {
                code: rmcp::model::ErrorCode(-32603),
                message: format!("Vector search failed: {}. Ensure codebase is indexed with 'codegraph index .'", e).into(),
                data: None,
            })
        }
    }

    /// Find code dependencies and relationships for a specific code element (function, class, etc)
    #[tool(description = "Find all code that depends on or is used by a specific code element. Shows dependencies, imports, and relationships. Use to understand code impact before refactoring. Required: node (UUID from search results). Optional: limit (max results, default 20). Note: Get node UUIDs from vector_search or enhanced_search results.")]
    async fn graph_neighbors(&self, params: Parameters<GraphNeighborsRequest>) -> Result<CallToolResult, McpError> {
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
            qwen_client: crate::server::init_qwen_client().await,
        };

        // Call graph neighbors with dependency analysis
        match crate::server::graph_neighbors(&state, serde_json::json!({
            "node": request.node,
            "limit": request.limit
        })).await {
            Ok(neighbors_results) => Ok(CallToolResult::success(vec![Content::text(
                serde_json::to_string_pretty(&neighbors_results)
                    .unwrap_or_else(|_| "Error formatting graph neighbors results".to_string())
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
                        .unwrap_or_else(|_| "Error formatting fallback results".to_string())
                )]))
            }
        }
    }

    /// Explore code architecture by following dependency chains from a starting point
    #[tool(description = "Follow dependency chains through your codebase to understand architectural flow and code relationships. Use to trace execution paths or understand system architecture. Required: start (UUID from search results). Optional: depth (how far to traverse, default 2), limit (max results, default 100). Note: Get start UUIDs from vector_search or enhanced_search results.")]
    async fn graph_traverse(&self, params: Parameters<GraphTraverseRequest>) -> Result<CallToolResult, McpError> {
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
            qwen_client: crate::server::init_qwen_client().await,
        };

        // Call graph traverse with architectural flow analysis
        match crate::server::graph_traverse(&state, serde_json::json!({
            "start": request.start,
            "depth": request.depth,
            "limit": request.limit
        })).await {
            Ok(traverse_results) => Ok(CallToolResult::success(vec![Content::text(
                serde_json::to_string_pretty(&traverse_results)
                    .unwrap_or_else(|_| "Error formatting graph traverse results".to_string())
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
                        .unwrap_or_else(|_| "Error formatting fallback results".to_string())
                )]))
            }
        }
    }

    /// REVOLUTIONARY: Intelligent codebase Q&A using RAG (Retrieval-Augmented Generation)
    #[cfg(all(feature = "ai-enhanced", feature = "qwen-integration"))]
    #[tool(description = "Ask natural language questions about the codebase and get intelligent, cited responses. Uses hybrid retrieval (vector search + graph traversal + keyword matching) with AI generation. Provides streaming responses with source citations and confidence scoring. Examples: 'How does authentication work?', 'Explain the data flow', 'What would break if I change this function?'. Required: question (natural language query). Optional: max_results (default 10), streaming (default false).")]
    async fn codebase_qa(&self, params: Parameters<CodebaseQaRequest>) -> Result<CallToolResult, McpError> {
        let request = params.0;

        // Use the existing graph database from the server
        let config = codegraph_ai::rag::engine::RAGEngineConfig {
            max_results: request.max_results.unwrap_or(10),
            graph_neighbor_expansion: true,
            neighbor_hops: 2,
            streaming_chunk_chars: 64,
            streaming_min_delay_ms: 10,
        };

        // Use the shared graph instance from the server (fixes lock conflict)
        let graph_instance = self.graph.clone();

        let rag_engine = codegraph_ai::rag::engine::RAGEngine::new(
            graph_instance,
            config
        );

        // Execute intelligent Q&A
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
                        .unwrap_or_else(|_| "Error formatting RAG response".to_string())
                )]))
            },
            Err(e) => {
                let fallback = serde_json::json!({
                    "question": request.question,
                    "error": format!("RAG processing failed: {}", e),
                    "fallback_mode": true,
                    "note": "Ensure codebase is indexed with edge processing enabled"
                });
                Ok(CallToolResult::success(vec![Content::text(
                    serde_json::to_string_pretty(&fallback)
                        .unwrap_or_else(|_| "Error formatting fallback response".to_string())
                )]))
            }
        }
    }

    /// REVOLUTIONARY: AI-powered code documentation generation with graph context
    #[cfg(all(feature = "ai-enhanced", feature = "qwen-integration"))]
    #[tool(description = "Generate comprehensive documentation for functions, classes, or modules using AI analysis with graph context. Analyzes dependencies, usage patterns, and architectural relationships to create intelligent documentation with source citations. Required: target_name (function/class/module name). Optional: file_path (focus scope), style (comprehensive/concise/tutorial).")]
    async fn code_documentation(&self, params: Parameters<CodeDocumentationRequest>) -> Result<CallToolResult, McpError> {
        let request = params.0;

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

        let rag_engine = codegraph_ai::rag::engine::RAGEngine::new(
            graph_instance,
            config
        );

        // Craft documentation query based on target and style
        let doc_query = format!(
            "Generate {} documentation for '{}' including its purpose, parameters, return values, usage examples, dependencies, and architectural context",
            request.style,
            request.target_name
        );

        // Execute intelligent documentation generation
        match rag_engine.answer(&doc_query).await {
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
                        .unwrap_or_else(|_| "Error formatting documentation response".to_string())
                )]))
            },
            Err(e) => {
                let fallback = serde_json::json!({
                    "target_name": request.target_name,
                    "error": format!("Documentation generation failed: {}", e),
                    "fallback_mode": true,
                    "note": "Ensure target exists in codebase and is properly indexed"
                });
                Ok(CallToolResult::success(vec![Content::text(
                    serde_json::to_string_pretty(&fallback)
                        .unwrap_or_else(|_| "Error formatting fallback response".to_string())
                )]))
            }
        }
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
    #[tool(description = "Perform deep architectural analysis of your entire codebase using AI. Explains system design, component relationships, and overall architecture. Use for understanding large codebases or documenting architecture. Required: query (analysis focus). Optional: task_type (analysis type, default 'semantic_search'), max_context_tokens (AI context limit, default 80000).")]
    async fn semantic_intelligence(&self, params: Parameters<SemanticIntelligenceRequest>) -> Result<CallToolResult, McpError> {
        let request = params.0;

        #[cfg(feature = "qwen-integration")]
        {
            // Use the existing graph database from the server
            let state = crate::server::ServerState {
                graph: self.graph.clone(),
                qwen_client: crate::server::init_qwen_client().await,
            };

            // Call semantic intelligence with Qwen analysis
            match crate::server::semantic_intelligence(&state, serde_json::json!({
                "query": request.query,
                "task_type": request.task_type,
                "max_context_tokens": request.max_context_tokens
            })).await {
                Ok(intelligence_results) => Ok(CallToolResult::success(vec![Content::text(
                    serde_json::to_string_pretty(&intelligence_results)
                        .unwrap_or_else(|_| "Error formatting semantic intelligence results".to_string())
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
                            .unwrap_or_else(|_| "Error formatting fallback results".to_string())
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
    #[tool(description = "Predict the impact of modifying a specific function or class. Shows what code depends on it and might break. Use before refactoring to avoid breaking changes. Required: target_function (function/class name), file_path (path to file containing it). Optional: change_type (type of change, default 'modify').")]
    async fn impact_analysis(&self, params: Parameters<ImpactAnalysisRequest>) -> Result<CallToolResult, McpError> {
        let request = params.0;

        #[cfg(feature = "qwen-integration")]
        {
            // Use the existing graph database from the server
            let state = crate::server::ServerState {
                graph: self.graph.clone(),
                qwen_client: crate::server::init_qwen_client().await,
            };

            // Call impact analysis with Qwen-powered dependency analysis
            match crate::server::impact_analysis(&state, serde_json::json!({
                "target_function": request.target_function,
                "file_path": request.file_path,
                "change_type": request.change_type
            })).await {
                Ok(impact_results) => Ok(CallToolResult::success(vec![Content::text(
                    serde_json::to_string_pretty(&impact_results)
                        .unwrap_or_else(|_| "Error formatting impact analysis results".to_string())
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
                            .unwrap_or_else(|_| "Error formatting fallback results".to_string())
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

    // /// Analyze CodeGraph's cache performance and get optimization recommendations (DISABLED - not useful for coding agents)
    // #[tool(description = "Analyze CodeGraph's caching system performance and get optimization recommendations. Shows cache hit/miss ratios, memory usage, and performance improvements. Use to optimize system performance or diagnose caching issues. No parameters required.")]
    // async fn cache_stats(&self, params: Parameters<EmptyRequest>) -> Result<CallToolResult, McpError> {
    //     let _request = params.0;

    //     #[cfg(feature = "qwen-integration")]
    //     {
    //         // This would use the cache analysis from the original server
    //         Ok(CallToolResult::success(vec![Content::text(
    //             "Intelligent Cache Performance Analysis\n\
    //             📈 Revolutionary Cache Intelligence:\n\
    //             • Semantic similarity matching effectiveness\n\
    //             • Response time improvements from caching\n\
    //             • Memory usage and optimization suggestions\n\
    //             • Performance trend analysis\n\
    //             🚀 Features:\n\
    //             • Hit/miss ratio optimization\n\
    //             • Cache health assessment\n\
    //             • Intelligent cache recommendations\n\
    //             💡 Status: Cache analytics ready!\n\
    //             💡 Note: Detailed statistics available with active cache usage".to_string()
    //         )]))
    //     }
    //     #[cfg(not(feature = "qwen-integration"))]
    //     {
    //         Ok(CallToolResult::success(vec![Content::text(
    //             "Cache Statistics\n\
    //             📈 Basic cache information available\n\
    //             💡 Note: Enable qwen-integration for advanced analytics".to_string()
    //         )]))
    //     }
    // }
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
                }
                Ok(false) => {
                    eprintln!("⚠️ Qwen2.5-Coder model not found. Install with: ollama pull {}", config.model_name);
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