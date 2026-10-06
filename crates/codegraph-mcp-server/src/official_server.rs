// ABOUTME: MCP server implementation for CodeGraph code intelligence tools
// ABOUTME: Provides semantic search, graph analysis, and agentic orchestration via MCP protocol
#![allow(dead_code, unused_variables, unused_imports)]

use futures::future::BoxFuture;
/// Clean Official MCP SDK Implementation for CodeGraph
/// Following exact Counter pattern from rmcp SDK documentation
use rmcp::{
    ErrorData as McpError, Peer, RoleServer, ServerHandler,
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{
        CallToolResult, ContentBlock as Content, GetPromptRequestParams, GetPromptResponse,
        GetPromptResult, ListPromptsResult, NumberOrString, PaginatedRequestParams,
        ProgressNotification, ProgressNotificationParam, ProgressToken, Prompt, PromptMessage,
        RequestMetaObject as Meta, Role as PromptMessageRole, ServerCapabilities, ServerConfig,
        ServerNotification,
    },
    service::RequestContext,
    tool, tool_handler, tool_router,
};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};
use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;

#[cfg(feature = "ai-enhanced")]
use crate::agentic_schemas::AgenticOutput;
use crate::agentic_tools::AgenticTool;
use crate::prompts::{INITIAL_INSTRUCTIONS, INITIAL_INSTRUCTIONS_PROMPT_NAME};
use codegraph_mcp_core::analysis::AnalysisType;
use codegraph_mcp_core::context_aware_limits::ContextTier;
use codegraph_mcp_core::debug_logger::DebugLogger;
#[cfg(feature = "ai-enhanced")]
use codegraph_mcp_rig::{RigAgentOutput, RigExecutor};
use codegraph_mcp_tools::GraphToolExecutor;
use codegraph_vector::EmbeddingGenerator;

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

/// Request for consolidated agentic tools with optional focus parameter
#[derive(Deserialize, JsonSchema)]
struct ConsolidatedSearchRequest {
    /// The search query for semantic analysis
    query: String,
    /// Maximum number of results to return
    #[serde(default = "default_limit")]
    limit: usize,
    /// Optional focus to narrow analysis scope. When omitted, agent auto-selects.
    /// Valid values depend on the tool:
    /// - agentic_context: "search", "builder", "question"
    /// - agentic_impact: "dependencies", "call_chain"
    /// - agentic_architecture: "structure", "api_surface"
    /// - agentic_quality: "complexity", "coupling", "hotspots"
    #[serde(default)]
    focus: Option<String>,
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
    /// Simple counter for demonstration
    counter: Arc<Mutex<i32>>,
    /// Official MCP tool router (required by macros)
    tool_router: ToolRouter<Self>,
}

#[tool_router]
impl CodeGraphMCPServer {
    pub fn new() -> Self {
        Self {
            counter: Arc::new(Mutex::new(0)),
            tool_router: Self::tool_router(),
        }
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
    /// DISABLED - Use agentic_code_search instead for multi-step reasoning
    // #[tool(
    //     description = "Search code with AI insights (2-5s). Returns relevant code + analysis of patterns and architecture. Use for: understanding code behavior, finding related functionality, discovering patterns. Fast alternative: vector_search. Required: query. Optional: limit (default 5)."
    // )]
    async fn read_initial_instructions(
        &self,
        _params: Parameters<EmptyRequest>,
    ) -> Result<CallToolResult, McpError> {
        let content = format!(
            "{}\n\n---\n\n**Tip:** These instructions are also available as the MCP prompt '{}'.",
            INITIAL_INSTRUCTIONS, INITIAL_INSTRUCTIONS_PROMPT_NAME
        );

        Ok(CallToolResult::success(vec![Content::text(content)]))
    }

    // === CONSOLIDATED AGENTIC MCP TOOLS ===
    // These 4 tools replace the previous 8 specialized tools for reduced cognitive load
    // Legacy tools available via "legacy-agentic-tools" feature flag

    /// Gather context for a query - default entrypoint for code discovery
    #[tool(
        description = "Gather client-readable context for a query. Returns JSON with: summary, analysis (how this answers the query), highlights (with file:line and snippets), related_locations, risks, next_steps (read/run), and confidence. Required: query."
    )]
    async fn agentic_context(
        &self,
        peer: Peer<RoleServer>,
        meta: Meta,
        params: Parameters<ConsolidatedSearchRequest>,
    ) -> Result<CallToolResult, McpError> {
        let request = params.0;
        self.execute_mcp_agentic_tool(AgenticTool::Context, request, peer, meta)
            .await
    }

    /// Assess change impact for a query
    #[tool(
        description = "Assess change impact for a query. Returns client-readable JSON with: summary, analysis (how this answers the query), impact highlights, affected file:line locations, risks, next_steps (read/run), and confidence. Required: query."
    )]
    async fn agentic_impact(
        &self,
        peer: Peer<RoleServer>,
        meta: Meta,
        params: Parameters<ConsolidatedSearchRequest>,
    ) -> Result<CallToolResult, McpError> {
        let request = params.0;
        self.execute_mcp_agentic_tool(AgenticTool::Impact, request, peer, meta)
            .await
    }

    /// Summarize system structure relevant to a query
    #[tool(
        description = "Summarize system structure relevant to a query. Returns client-readable JSON with: summary, analysis (how this answers the query), highlights, related_locations, risks, next_steps, and confidence (with file:line and snippets when available). Required: query."
    )]
    async fn agentic_architecture(
        &self,
        peer: Peer<RoleServer>,
        meta: Meta,
        params: Parameters<ConsolidatedSearchRequest>,
    ) -> Result<CallToolResult, McpError> {
        let request = params.0;
        self.execute_mcp_agentic_tool(AgenticTool::Architecture, request, peer, meta)
            .await
    }

    /// Highlight quality risks related to a query
    #[tool(
        description = "Highlight quality risks related to a query. Returns client-readable JSON with: summary, analysis (how this answers the query), hotspot highlights, risk notes, next_steps (read/run), and confidence (with file:line and snippets when available). Required: query."
    )]
    async fn agentic_quality(
        &self,
        peer: Peer<RoleServer>,
        meta: Meta,
        params: Parameters<ConsolidatedSearchRequest>,
    ) -> Result<CallToolResult, McpError> {
        let request = params.0;
        self.execute_mcp_agentic_tool(AgenticTool::Quality, request, peer, meta)
            .await
    }

    async fn execute_mcp_agentic_tool(
        &self,
        tool: AgenticTool,
        request: ConsolidatedSearchRequest,
        peer: Peer<RoleServer>,
        meta: Meta,
    ) -> Result<CallToolResult, McpError> {
        let response = self
            .execute_agentic_workflow(
                tool.analysis_type(request.focus.as_deref()),
                &request.query,
                Some(peer),
                meta,
            )
            .await?;
        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&response)
                .unwrap_or_else(|_| "Error formatting agent result".to_string()),
        )]))
    }

    /// Execute the same agentic workflow without starting an MCP transport.
    pub async fn execute_agentic_tool(
        &self,
        tool: AgenticTool,
        query: &str,
        focus: Option<&str>,
    ) -> Result<Value, McpError> {
        self.execute_agentic_workflow(tool.analysis_type(focus), query, None, Meta::default())
            .await
    }
}

// NOTE: Legacy agentic tools (agentic_code_search, agentic_dependency_analysis, etc.)
// have been removed in favor of the 4 consolidated tools above.
// The rmcp SDK doesn't support multiple #[tool_router] blocks, so feature-flag based
// toggling is not possible. Use the consolidated tools with the focus parameter instead:
// - agentic_context (focus: search, builder, question)
// - agentic_impact (focus: dependencies, call_chain)
// - agentic_architecture (focus: structure, api_surface)
// - agentic_quality (focus: complexity, coupling, hotspots)

impl CodeGraphMCPServer {
    #[cfg(feature = "ai-enhanced")]
    fn synthesize_structured_output_from_traces(
        analysis_type: AnalysisType,
        analysis_text: &str,
        traces: &[codegraph_mcp_rig::ToolTrace],
    ) -> Option<serde_json::Value> {
        let mut highlights: Vec<serde_json::Value> = Vec::new();

        for trace in traces {
            let Some(result) = trace.result.as_ref() else {
                continue;
            };

            let candidates = result
                .get("result")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();

            for item in candidates {
                let (file_path, line_number, snippet) = Self::extract_pinpoint(&item);
                let Some(file_path) = file_path else {
                    continue;
                };

                let name = item
                    .get("name")
                    .and_then(|v| v.as_str())
                    .or_else(|| item.get("function_name").and_then(|v| v.as_str()))
                    .unwrap_or(trace.tool_name.as_str())
                    .to_string();

                highlights.push(serde_json::json!({
                    "name": name,
                    "file_path": file_path,
                    "line_number": line_number,
                    "snippet": snippet,
                    "source_tool": trace.tool_name,
                }));

                if highlights.len() >= 25 {
                    break;
                }
            }

            if highlights.len() >= 25 {
                break;
            }
        }

        if highlights.is_empty() {
            return None;
        }

        let summary = analysis_text
            .lines()
            .find(|l| !l.trim().is_empty())
            .map(|l| l.trim().to_string())
            .unwrap_or_else(|| analysis_text.chars().take(160).collect());

        Some(serde_json::json!({
            "analysis_type": analysis_type.as_str(),
            "summary": summary,
            "analysis": analysis_text,
            "highlights": highlights,
        }))
    }

    #[cfg(feature = "ai-enhanced")]
    fn extract_pinpoint(
        item: &serde_json::Value,
    ) -> (Option<String>, Option<usize>, Option<String>) {
        let file_path = item
            .get("file_path")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .or_else(|| {
                item.get("location")
                    .and_then(|loc| loc.get("file_path"))
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
            })
            .or_else(|| {
                item.get("node")
                    .and_then(|n| n.get("location"))
                    .and_then(|loc| loc.get("file_path"))
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
            });

        let line_number = item
            .get("line_number")
            .and_then(|v| v.as_u64())
            .map(|n| n as usize)
            .or_else(|| {
                item.get("start_line")
                    .and_then(|v| v.as_u64())
                    .map(|n| n as usize)
            })
            .or_else(|| {
                item.get("location")
                    .and_then(|loc| loc.get("start_line"))
                    .and_then(|v| v.as_u64())
                    .map(|n| n as usize)
            })
            .or_else(|| {
                item.get("node")
                    .and_then(|n| n.get("location"))
                    .and_then(|loc| loc.get("start_line"))
                    .and_then(|v| v.as_u64())
                    .map(|n| n as usize)
            });

        let raw_snippet = item
            .get("content")
            .and_then(|v| v.as_str())
            .or_else(|| item.get("text").and_then(|v| v.as_str()));

        let snippet = raw_snippet.map(|s| {
            let trimmed = s.trim();
            if trimmed.len() <= 240 {
                trimmed.to_string()
            } else {
                format!("{}…", trimmed.chars().take(240).collect::<String>())
            }
        });

        (file_path, line_number, snippet)
    }

    /// Context tier for the agent: the same context window the Rig backend resolves
    /// (environment, then `[llm] context_window`, then the default).
    #[cfg(feature = "ai-enhanced")]
    fn detect_context_tier() -> ContextTier {
        ContextTier::from_context_window(
            codegraph_core::config_manager::ConfigManager::agent_context_window(),
        )
    }

    /// Creates a progress notification callback that sends MCP protocol notifications
    /// with message support for 3-stage progress updates
    #[cfg(feature = "ai-enhanced")]
    fn create_progress_callback_with_message(
        peer: Peer<RoleServer>,
        progress_token: ProgressToken,
    ) -> codegraph_mcp_core::ProgressCallback {
        Arc::new(move |progress, message| {
            let peer = peer.clone();
            let progress_token = progress_token.clone();

            Box::pin(async move {
                let mut params = ProgressNotificationParam::new(progress_token.clone(), progress);
                params.total = Some(1.0);
                params.message = message;
                let notification = ProgressNotification::new(params);

                // Ignore notification errors (non-blocking)
                let _ = peer
                    .send_notification(ServerNotification::ProgressNotification(notification))
                    .await;
            })
        })
    }

    /// Execute an agentic workflow with the Rig agent backend
    #[cfg(feature = "ai-enhanced")]
    async fn execute_agentic_workflow(
        &self,
        analysis_type: AnalysisType,
        query: &str,
        peer: Option<Peer<RoleServer>>,
        meta: Meta,
    ) -> Result<Value, McpError> {
        use codegraph_graph::GraphFunctions;
        use codegraph_mcp_core::ProgressNotifier;
        use std::sync::Arc;

        // Auto-detect context tier
        let tier = Self::detect_context_tier();

        tracing::info!("Agentic {} (tier={:?})", analysis_type.as_str(), tier);

        DebugLogger::log_agent_start(query, analysis_type.as_str(), &format!("{:?}", tier));

        // Create progress notifier for 3-stage notifications
        let progress_notifier = if let (Some(peer), Some(progress_token)) =
            (peer.as_ref(), meta.get_progress_token())
        {
            let callback =
                Self::create_progress_callback_with_message(peer.clone(), progress_token);
            ProgressNotifier::new(callback, analysis_type.as_str())
        } else {
            ProgressNotifier::noop()
        };

        // Stage 1: Agent started (progress: 0.0)
        progress_notifier.notify_started().await;

        // Load config for LLM provider
        let config_manager =
            codegraph_core::config_manager::ConfigManager::load().map_err(|e| {
                let error_msg = format!("Failed to load config: {}", e);
                let notifier = progress_notifier.clone();
                let error_for_spawn = error_msg.clone();
                tokio::spawn(async move {
                    notifier.notify_error(&error_for_spawn).await;
                });
                DebugLogger::log_agent_finish(false, None, Some(&error_msg));
                McpError {
                    code: rmcp::model::ErrorCode(-32603),
                    message: error_msg.into(),
                    data: None,
                }
            })?;
        let config = config_manager.config();

        // Create GraphFunctions with SurrealDB connection
        let graph_functions = {
            use codegraph_graph::SurrealDbStorage;

            // Embedded project store under the working directory unless
            // CODEGRAPH_SURREALDB_URL points at a server.
            let project_root = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
            let surrealdb_config = codegraph_graph::SurrealDbConfig::for_project(&project_root);

            let storage = SurrealDbStorage::new(surrealdb_config).await.map_err(|e| {
                let error_msg = format!("Failed to open SurrealDB storage: {}", e);
                let notifier = progress_notifier.clone();
                let error_for_spawn = error_msg.clone();
                tokio::spawn(async move {
                    notifier.notify_error(&error_for_spawn).await;
                });
                DebugLogger::log_agent_finish(false, None, Some(&error_msg));
                McpError {
                    code: rmcp::model::ErrorCode(-32603),
                    message: error_msg.into(),
                    data: None,
                }
            })?;

            // Derive project_id from env or canonical working directory for consistent DB selection
            let env_project = std::env::var("CODEGRAPH_PROJECT_ID")
                .ok()
                .filter(|v| !v.trim().is_empty());
            let cwd_fallback = std::env::current_dir()
                .ok()
                .map(|p| p.display().to_string());
            let raw_project = env_project
                .clone()
                .or(cwd_fallback)
                .unwrap_or_else(|| "default-project".to_string());

            let canonical_project = Path::new(&raw_project)
                .canonicalize()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|_| raw_project.clone());

            Arc::new(GraphFunctions::new_with_project_id(
                storage.db(),
                canonical_project,
            ))
        };

        // Health check: ensure the active project has indexed nodes
        match graph_functions.count_nodes_for_project().await {
            Ok(0) => tracing::warn!(
                "Project '{}' has zero nodes indexed. Ensure CODEGRAPH_PROJECT_ID matches the indexed project and rerun `codegraph index`.",
                graph_functions.project_id()
            ),
            Ok(count) => tracing::info!(
                "Project '{}' has {} indexed nodes available for analysis",
                graph_functions.project_id(),
                count
            ),
            Err(e) => tracing::warn!(
                "Could not verify project data presence: {}. Continuing without blocking.",
                e
            ),
        }

        // Create shared EmbeddingGenerator (once for entire server lifecycle)
        let embedding_generator: Arc<EmbeddingGenerator> = Arc::new(
            EmbeddingGenerator::with_config(&config)
                .await
                .map_err(|error| {
                    rmcp::ErrorData::internal_error(
                        format!("Embedding input policy initialization failed: {error}"),
                        None,
                    )
                })?,
        );
        tracing::info!(
            "✅ Shared EmbeddingGenerator initialized (dimension: {}, provider: {})",
            embedding_generator.dimension(),
            config.embedding.provider
        );

        // Create GraphToolExecutor with shared embedding generator
        let tool_executor = Arc::new(GraphToolExecutor::new(
            graph_functions,
            Arc::new(config.clone()),
            embedding_generator,
        ));

        // Stage 2: Agent analyzing with tools (progress: 0.5)
        // Sent after all setup is complete, before actual agent execution
        progress_notifier.notify_analyzing().await;

        // The Rig backend picks its agent (ReAct, LATS, Reflexion) from CODEGRAPH_AGENT_ARCHITECTURE
        let mut rig_executor = RigExecutor::new(tool_executor.clone());
        let rig_output = match rig_executor.execute(query, analysis_type).await {
            Ok(output) => output,
            Err(e) => {
                let error_msg = format!("Rig workflow failed: {}", e);
                progress_notifier.notify_error(&error_msg).await;
                DebugLogger::log_agent_finish(false, None, Some(&error_msg));
                return Err(McpError {
                    code: rmcp::model::ErrorCode(-32603),
                    message: error_msg.into(),
                    data: None,
                });
            }
        };
        let RigAgentOutput {
            response: answer,
            tool_calls: tool_use_count,
            duration_ms,
            tool_traces,
        } = rig_output;
        let findings = format!(
            "Completed in {}ms with {} tool calls",
            duration_ms, tool_use_count
        );
        let steps_taken = tool_use_count.to_string();
        let framework_name = "Rig";

        // Parse structured output from answer field (contains JSON schema)
        use crate::agentic_schemas::*;

        // Try to parse the answer as structured output first
        tracing::debug!(
            "Attempting to parse structured output for {:?}",
            analysis_type
        );
        tracing::debug!(
            "Answer length: {}, first 200 chars: {}",
            answer.len(),
            answer.chars().take(200).collect::<String>()
        );

        let structured_output = match analysis_type {
            AnalysisType::CodeSearch => match serde_json::from_str::<CodeSearchOutput>(&answer) {
                Ok(o) => {
                    tracing::info!("✅ Successfully parsed CodeSearchOutput");
                    serde_json::to_value(AgenticOutput::CodeSearch(o)).ok()
                }
                Err(e) => {
                    tracing::debug!(
                        "Answer is not typed CodeSearchOutput JSON ({}); using tool traces",
                        e
                    );
                    None
                }
            },
            AnalysisType::DependencyAnalysis => {
                match serde_json::from_str::<DependencyAnalysisOutput>(&answer) {
                    Ok(o) => {
                        tracing::info!("✅ Successfully parsed DependencyAnalysisOutput");
                        serde_json::to_value(AgenticOutput::DependencyAnalysis(o)).ok()
                    }
                    Err(e) => {
                        tracing::debug!(
                            "Answer is not typed DependencyAnalysisOutput JSON ({}); using tool traces",
                            e
                        );
                        None
                    }
                }
            }
            AnalysisType::CallChainAnalysis => {
                match serde_json::from_str::<CallChainOutput>(&answer) {
                    Ok(o) => {
                        tracing::info!("✅ Successfully parsed CallChainOutput");
                        serde_json::to_value(AgenticOutput::CallChain(o)).ok()
                    }
                    Err(e) => {
                        tracing::debug!(
                            "Answer is not typed CallChainOutput JSON ({}); using tool traces",
                            e
                        );
                        None
                    }
                }
            }
            AnalysisType::ArchitectureAnalysis => {
                match serde_json::from_str::<ArchitectureAnalysisOutput>(&answer) {
                    Ok(o) => {
                        tracing::info!("✅ Successfully parsed ArchitectureAnalysisOutput");
                        serde_json::to_value(AgenticOutput::ArchitectureAnalysis(o)).ok()
                    }
                    Err(e) => {
                        tracing::debug!(
                            "Answer is not typed ArchitectureAnalysisOutput JSON ({}); using tool traces",
                            e
                        );
                        None
                    }
                }
            }
            AnalysisType::ApiSurfaceAnalysis => {
                match serde_json::from_str::<APISurfaceOutput>(&answer) {
                    Ok(o) => {
                        tracing::info!("✅ Successfully parsed APISurfaceOutput");
                        serde_json::to_value(AgenticOutput::APISurface(o)).ok()
                    }
                    Err(e) => {
                        tracing::debug!(
                            "Answer is not typed APISurfaceOutput JSON ({}); using tool traces",
                            e
                        );
                        None
                    }
                }
            }
            AnalysisType::ContextBuilder => {
                match serde_json::from_str::<ContextBuilderOutput>(&answer) {
                    Ok(o) => {
                        tracing::info!("✅ Successfully parsed ContextBuilderOutput");
                        serde_json::to_value(AgenticOutput::ContextBuilder(o)).ok()
                    }
                    Err(e) => {
                        tracing::debug!(
                            "Answer is not typed ContextBuilderOutput JSON ({}); using tool traces",
                            e
                        );
                        None
                    }
                }
            }
            AnalysisType::SemanticQuestion => {
                match serde_json::from_str::<SemanticQuestionOutput>(&answer) {
                    Ok(o) => {
                        tracing::info!("✅ Successfully parsed SemanticQuestionOutput");
                        serde_json::to_value(AgenticOutput::SemanticQuestion(o)).ok()
                    }
                    Err(e) => {
                        tracing::debug!(
                            "Answer is not typed SemanticQuestionOutput JSON ({}); using tool traces",
                            e
                        );
                        None
                    }
                }
            }
            AnalysisType::ComplexityAnalysis => {
                match serde_json::from_str::<ComplexityAnalysisOutput>(&answer) {
                    Ok(o) => {
                        tracing::info!("✅ Successfully parsed ComplexityAnalysisOutput");
                        serde_json::to_value(AgenticOutput::ComplexityAnalysis(o)).ok()
                    }
                    Err(e) => {
                        tracing::debug!(
                            "Answer is not typed ComplexityAnalysisOutput JSON ({}); using tool traces",
                            e
                        );
                        None
                    }
                }
            }
        };

        let synthesized = structured_output.or_else(|| {
            Self::synthesize_structured_output_from_traces(analysis_type, &answer, &tool_traces)
        });

        // Format result as JSON with structured output if available
        let response_json = if let Some(structured) = synthesized {
            serde_json::json!({
                "analysis_type": analysis_type.as_str(),
                "tier": format!("{:?}", tier),
                "query": query,
                "structured_output": structured,
                "steps_taken": steps_taken,
                "tool_use_count": tool_use_count,
                "framework": framework_name,
                "answer": answer,
                "findings": findings,
            })
        } else {
            // Fallback to original format if parsing failed
            serde_json::json!({
                "analysis_type": analysis_type.as_str(),
                "tier": format!("{:?}", tier),
                "query": query,
                "answer": answer,
                "findings": findings,
                "steps_taken": steps_taken,
                "tool_use_count": tool_use_count,
                "framework": framework_name,
            })
        };

        DebugLogger::log_agent_finish(true, Some(&response_json), None);

        // Stage 3: Agent complete (progress: 1.0)
        progress_notifier.notify_complete().await;

        Ok(response_json)
    }

    /// Stub when ai-enhanced feature is disabled
    #[cfg(not(feature = "ai-enhanced"))]
    async fn execute_agentic_workflow(
        &self,
        analysis_type: AnalysisType,
        query: &str,
        _peer: Option<Peer<RoleServer>>,
        _meta: Meta,
    ) -> Result<Value, McpError> {
        let _ = (analysis_type, query);
        Err(McpError::invalid_request(
            "Agentic tools require the `ai-enhanced` feature to be enabled",
            None,
        ))
    }
}

/// Official MCP ServerHandler implementation (following Counter pattern)
#[tool_handler]
impl ServerHandler for CodeGraphMCPServer {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(
            ServerCapabilities::builder()
                .enable_tools()
                .enable_prompts()
                .build(),
        )
        .with_instructions(INITIAL_INSTRUCTIONS)
    }

    fn list_prompts(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<ListPromptsResult, McpError>> + Send + '_ {
        async move {
            Ok(ListPromptsResult {
                prompts: vec![initial_instructions_prompt()],
                next_cursor: None,
                ..Default::default()
            })
        }
    }

    fn get_prompt(
        &self,
        request: GetPromptRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<GetPromptResponse, McpError>> + Send + '_ {
        let name = request.name.clone();
        async move {
            match name.as_str() {
                INITIAL_INSTRUCTIONS_PROMPT_NAME => Ok(GetPromptResult::new(vec![
                    PromptMessage::new_text(PromptMessageRole::User,
                        "Please read the CodeGraph Initial Instructions below. These guidelines will help you use CodeGraph tools efficiently and avoid wasting context by reading unnecessary files."),
                    PromptMessage::new_text(PromptMessageRole::Assistant, INITIAL_INSTRUCTIONS),
                ]).with_description(
                    "MANDATORY: CodeGraph Usage Protocol - You MUST read and follow these instructions before using any CodeGraph tools"
                ).into()),
                _ => Err(McpError::invalid_params(
                    format!("Unknown prompt: {}", name),
                    None
                )),
            }
        }
    }
}

fn initial_instructions_prompt() -> Prompt {
    Prompt::new(
        INITIAL_INSTRUCTIONS_PROMPT_NAME,
        Some("MANDATORY: CodeGraph Usage Protocol - Read before using any CodeGraph tools"),
        None,
    )
}

#[cfg(all(test, feature = "ai-enhanced"))]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn synthesize_structured_output_includes_highlights_from_trace() {
        let traces = vec![codegraph_mcp_rig::ToolTrace {
            tool_name: "semantic_code_search".to_string(),
            parameters: json!({"query": "config loading"}),
            result: Some(json!({
                "tool": "semantic_code_search",
                "result": [
                    {
                        "name": "load_config",
                        "file_path": "crates/codegraph-core/src/config_manager.rs",
                        "start_line": 123,
                        "content": "fn load_config() { /* ... */ }"
                    }
                ]
            })),
            error: None,
        }];

        let synthesized = CodeGraphMCPServer::synthesize_structured_output_from_traces(
            AnalysisType::ContextBuilder,
            "analysis text",
            &traces,
        )
        .expect("expected synthesized output");

        let highlights = synthesized
            .get("highlights")
            .and_then(|v| v.as_array())
            .expect("expected highlights array");
        assert!(!highlights.is_empty(), "expected at least one highlight");
        assert_eq!(
            highlights[0].get("file_path").and_then(|v| v.as_str()),
            Some("crates/codegraph-core/src/config_manager.rs")
        );
    }
}

#[cfg(test)]
mod prompt_tests {
    use super::*;

    #[test]
    fn initial_instructions_prompt_name_is_mcp_compatible() {
        assert_eq!(
            initial_instructions_prompt().name,
            "codegraph:initial_instructions"
        );
    }
}
