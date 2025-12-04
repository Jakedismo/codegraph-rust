// ABOUTME: MCP server implementation for CodeGraph code intelligence tools
// ABOUTME: Provides semantic search, graph analysis, and agentic orchestration via MCP protocol
#![allow(dead_code, unused_variables, unused_imports)]

use futures::future::BoxFuture;
/// Clean Official MCP SDK Implementation for CodeGraph
/// Following exact Counter pattern from rmcp SDK documentation
use rmcp::{
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{
        CallToolResult, Content, GetPromptRequestParam, GetPromptResult, ListPromptsResult, Meta,
        NumberOrString, PaginatedRequestParam, ProgressNotification, ProgressNotificationParam,
        ProgressToken, Prompt, PromptMessage, PromptMessageContent, PromptMessageRole,
        ServerCapabilities, ServerInfo, ServerNotification,
    },
    service::RequestContext,
    tool, tool_handler, tool_router, ErrorData as McpError, Peer, RoleServer, ServerHandler,
};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{json, Value};
use std::future::Future;
use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::prompt_selector::AnalysisType;
use crate::prompts::INITIAL_INSTRUCTIONS;
use codegraph_ai::agentic_schemas::AgenticOutput;
use codegraph_mcp_autoagents::{CodeGraphAgentOutput, CodeGraphExecutor, CodeGraphExecutorBuilder};
use codegraph_mcp_core::context_aware_limits::ContextTier;
use codegraph_mcp_core::debug_logger::DebugLogger;
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
            "{}\n\n---\n\n**Tip:** These instructions are also available as the MCP prompt 'codegraph_initial_instructions'.",
            INITIAL_INSTRUCTIONS
        );

        Ok(CallToolResult::success(vec![Content::text(content)]))
    }

    // === AGENTIC MCP TOOLS ===
    // These tools use AgenticOrchestrator for multi-step graph analysis workflows
    // with automatic tier detection based on CODEGRAPH_CONTEXT_WINDOW or config

    /// Agentic code search with multi-step graph exploration
    #[tool(
        description = "Find code by semantic meaning. Returns: code snippets with file paths, line numbers, and explanations of relevance. Use the results to navigate to implementations or understand how features work. Required: query."
    )]
    async fn agentic_code_search(
        &self,
        peer: Peer<RoleServer>,
        meta: Meta,
        params: Parameters<SearchRequest>,
    ) -> Result<CallToolResult, McpError> {
        let request = params.0;
        self.execute_agentic_workflow(AnalysisType::CodeSearch, &request.query, peer, meta)
            .await
    }

    /// Agentic dependency analysis with multi-step exploration
    #[tool(
        description = "Understand what code depends on what. Returns: dependency chains showing which components use which others, with impact analysis if components change. Use the results to plan refactoring, assess risk of changes, or understand coupling. Required: query."
    )]
    async fn agentic_dependency_analysis(
        &self,
        peer: Peer<RoleServer>,
        meta: Meta,
        params: Parameters<SearchRequest>,
    ) -> Result<CallToolResult, McpError> {
        let request = params.0;
        self.execute_agentic_workflow(AnalysisType::DependencyAnalysis, &request.query, peer, meta)
            .await
    }

    /// Agentic call chain analysis with multi-step tracing
    #[tool(
        description = "Trace how code executes from start to finish. Returns: execution paths showing which functions call which others, with file paths and line numbers. Use the results to debug issues, understand data flow, or trace request handling. Required: query."
    )]
    async fn agentic_call_chain_analysis(
        &self,
        peer: Peer<RoleServer>,
        meta: Meta,
        params: Parameters<SearchRequest>,
    ) -> Result<CallToolResult, McpError> {
        let request = params.0;
        self.execute_agentic_workflow(AnalysisType::CallChainAnalysis, &request.query, peer, meta)
            .await
    }

    /// Agentic architecture analysis with multi-step system exploration
    #[tool(
        description = "Understand system structure and design patterns. Returns: module organization, architectural layers, design patterns used, and component relationships. Use the results to onboard to a codebase, plan architectural changes, or evaluate design decisions. Required: query."
    )]
    async fn agentic_architecture_analysis(
        &self,
        peer: Peer<RoleServer>,
        meta: Meta,
        params: Parameters<SearchRequest>,
    ) -> Result<CallToolResult, McpError> {
        let request = params.0;
        self.execute_agentic_workflow(
            AnalysisType::ArchitectureAnalysis,
            &request.query,
            peer,
            meta,
        )
        .await
    }

    /// Agentic API surface analysis with multi-step exploration
    #[tool(
        description = "Discover public interfaces and contracts. Returns: public functions, types, and interfaces with their signatures and usage patterns. Use the results to understand integration points, plan API changes, or document interfaces. Required: query."
    )]
    async fn agentic_api_surface_analysis(
        &self,
        peer: Peer<RoleServer>,
        meta: Meta,
        params: Parameters<SearchRequest>,
    ) -> Result<CallToolResult, McpError> {
        let request = params.0;
        self.execute_agentic_workflow(AnalysisType::ApiSurfaceAnalysis, &request.query, peer, meta)
            .await
    }

    /// Agentic context builder with multi-step comprehensive context gathering
    #[tool(
        description = "Gather comprehensive context for implementing changes. Returns: relevant code snippets, dependencies, patterns, and related implementations needed to understand a feature area. Use the results to prepare for coding tasks or understand how to extend functionality. Required: query."
    )]
    async fn agentic_context_builder(
        &self,
        peer: Peer<RoleServer>,
        meta: Meta,
        params: Parameters<SearchRequest>,
    ) -> Result<CallToolResult, McpError> {
        let request = params.0;
        self.execute_agentic_workflow(AnalysisType::ContextBuilder, &request.query, peer, meta)
            .await
    }

    /// Agentic semantic question answering with multi-step exploration
    #[tool(
        description = "Answer complex questions about the codebase. Returns: detailed explanations with supporting code evidence and reasoning. Use for questions that span multiple files or require understanding across the system. Required: query."
    )]
    async fn agentic_semantic_question(
        &self,
        peer: Peer<RoleServer>,
        meta: Meta,
        params: Parameters<SearchRequest>,
    ) -> Result<CallToolResult, McpError> {
        let request = params.0;
        self.execute_agentic_workflow(AnalysisType::SemanticQuestion, &request.query, peer, meta)
            .await
    }
}

impl CodeGraphMCPServer {
    /// Auto-detect context tier from environment or config
    #[cfg(feature = "ai-enhanced")]
    fn detect_context_tier() -> ContextTier {
        // Try CODEGRAPH_CONTEXT_WINDOW env var first
        if let Ok(context_window_str) = std::env::var("CODEGRAPH_CONTEXT_WINDOW") {
            if let Ok(context_window) = context_window_str.parse::<usize>() {
                return ContextTier::from_context_window(context_window);
            }
        }

        // Fall back to config
        match codegraph_core::config_manager::ConfigManager::load() {
            Ok(config_manager) => {
                let config = config_manager.config();
                ContextTier::from_context_window(config.llm.context_window)
            }
            Err(_) => {
                // Default to Medium tier if config can't be loaded
                tracing::warn!("Failed to load config, defaulting to Medium context tier");
                ContextTier::Medium
            }
        }
    }

    /// Creates a progress notification callback that sends MCP protocol notifications
    /// with message support for 3-stage progress updates
    #[cfg(feature = "ai-enhanced")]
    fn create_progress_callback_with_message(
        peer: Peer<RoleServer>,
        progress_token: ProgressToken,
    ) -> codegraph_mcp_autoagents::ProgressCallback {
        Arc::new(move |progress, message| {
            let peer = peer.clone();
            let progress_token = progress_token.clone();

            Box::pin(async move {
                let notification = ProgressNotification {
                    method: Default::default(),
                    params: ProgressNotificationParam {
                        progress_token: progress_token.clone(),
                        progress,
                        total: Some(1.0), // Total is always 1.0 for 3-stage progress
                        message,
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

    /// Execute agentic workflow using AutoAgents framework
    #[cfg(feature = "ai-enhanced")]
    async fn execute_agentic_workflow(
        &self,
        analysis_type: AnalysisType,
        query: &str,
        peer: Peer<RoleServer>,
        meta: Meta,
    ) -> Result<CallToolResult, McpError> {
        use codegraph_ai::llm_factory::LLMProviderFactory;
        use codegraph_graph::GraphFunctions;
        use codegraph_mcp_autoagents::{CodeGraphExecutor, CodeGraphExecutorBuilder, ProgressNotifier};
        use std::sync::Arc;

        // Auto-detect context tier
        let tier = Self::detect_context_tier();

        tracing::info!("AutoAgents {} (tier={:?})", analysis_type.as_str(), tier);

        DebugLogger::log_agent_start(query, analysis_type.as_str(), &format!("{:?}", tier));

        // Create progress notifier for 3-stage notifications
        let progress_notifier = if let Some(progress_token) = meta.get_progress_token() {
            let callback = Self::create_progress_callback_with_message(peer.clone(), progress_token);
            ProgressNotifier::new(callback, analysis_type.as_str())
        } else {
            ProgressNotifier::noop()
        };

        // Stage 1: Agent started (progress: 0.0)
        progress_notifier.notify_started().await;

        // Load config for LLM provider
        let config_manager = codegraph_core::config_manager::ConfigManager::load()
            .map_err(|e| {
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

        // Create LLM provider
        let llm_provider = LLMProviderFactory::create_from_config(&config.llm)
            .map_err(|e| {
                let error_msg = format!("Failed to create LLM provider: {}", e);
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

        // Create GraphFunctions with SurrealDB connection
        let graph_functions = {
            use codegraph_graph::SurrealDbStorage;

            // Use CODEGRAPH_* env if present; fall back to SURREALDB_*; else defaults
            let connection = std::env::var("CODEGRAPH_SURREALDB_URL")
                .or_else(|_| std::env::var("SURREALDB_URL"))
                .unwrap_or_else(|_| "ws://localhost:3004".to_string());
            let namespace = std::env::var("CODEGRAPH_SURREALDB_NAMESPACE")
                .or_else(|_| std::env::var("SURREALDB_NAMESPACE"))
                .unwrap_or_else(|_| "ouroboros".to_string());
            let use_graph_db = std::env::var("CODEGRAPH_USE_GRAPH_SCHEMA")
                .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
                .unwrap_or(false);
            let graph_db = std::env::var("CODEGRAPH_GRAPH_DB_DATABASE")
                .unwrap_or_else(|_| "codegraph_graph".to_string());

            let database = if use_graph_db {
                graph_db
            } else {
                std::env::var("CODEGRAPH_SURREALDB_DATABASE")
                    .or_else(|_| std::env::var("SURREALDB_DATABASE"))
                    .unwrap_or_else(|_| "codegraph".to_string())
            };
            let username = std::env::var("CODEGRAPH_SURREALDB_USERNAME")
                .or_else(|_| std::env::var("SURREALDB_USERNAME"))
                .ok();
            let password = std::env::var("CODEGRAPH_SURREALDB_PASSWORD")
                .or_else(|_| std::env::var("SURREALDB_PASSWORD"))
                .ok();

            let surrealdb_config = codegraph_graph::SurrealDbConfig {
                connection,
                namespace,
                database,
                username,
                password,
                strict_mode: false,
                auto_migrate: false,
                cache_enabled: false,
            };

            let storage = SurrealDbStorage::new(surrealdb_config)
                .await
                .map_err(|e| {
                    let error_msg = format!("Failed to create SurrealDB storage: {}. Ensure SurrealDB is running on ws://localhost:3004", e);
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

            Arc::new(GraphFunctions::new(storage.db()))
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
        let embedding_generator: Arc<EmbeddingGenerator> =
            Arc::new(EmbeddingGenerator::with_config(&config).await);
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

        // Build CodeGraphExecutor
        let executor = CodeGraphExecutorBuilder::new()
            .llm_provider(llm_provider)
            .tool_executor(tool_executor)
            .build()
            .map_err(|e| {
                let error_msg = format!("Failed to build AutoAgents executor: {}", e);
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

        // Stage 2: Agent analyzing with tools (progress: 0.5)
        // Sent after all setup is complete, before actual agent execution
        progress_notifier.notify_analyzing().await;

        // Execute agentic workflow
        let result: CodeGraphAgentOutput = match executor
            .execute(query.to_string(), analysis_type)
            .await
        {
            Ok(output) => output,
            Err(e) => {
                let error_msg = format!("AutoAgents workflow failed: {}", e);
                // Stage 3: Error notification (progress: 1.0)
                progress_notifier.notify_error(&error_msg).await;
                DebugLogger::log_agent_finish(false, None, Some(&error_msg));
                return Err(McpError {
                    code: rmcp::model::ErrorCode(-32603),
                    message: error_msg.into(),
                    data: None,
                });
            }
        };

        // Parse structured output from answer field (contains JSON schema)
        use codegraph_ai::agentic_schemas::*;

        // Try to parse the answer as structured output first
        tracing::debug!(
            "Attempting to parse structured output for {:?}",
            analysis_type
        );
        tracing::debug!(
            "Answer length: {}, first 200 chars: {}",
            result.answer.len(),
            result.answer.chars().take(200).collect::<String>()
        );

        let structured_output = match analysis_type {
            AnalysisType::CodeSearch => {
                match serde_json::from_str::<CodeSearchOutput>(&result.answer) {
                    Ok(o) => {
                        tracing::info!("✅ Successfully parsed CodeSearchOutput");
                        serde_json::to_value(AgenticOutput::CodeSearch(o)).ok()
                    }
                    Err(e) => {
                        tracing::warn!("Failed to parse CodeSearchOutput: {}", e);
                        None
                    }
                }
            }
            AnalysisType::DependencyAnalysis => {
                match serde_json::from_str::<DependencyAnalysisOutput>(&result.answer) {
                    Ok(o) => {
                        tracing::info!("✅ Successfully parsed DependencyAnalysisOutput");
                        serde_json::to_value(AgenticOutput::DependencyAnalysis(o)).ok()
                    }
                    Err(e) => {
                        tracing::warn!("Failed to parse DependencyAnalysisOutput: {}", e);
                        None
                    }
                }
            }
            AnalysisType::CallChainAnalysis => {
                match serde_json::from_str::<CallChainOutput>(&result.answer) {
                    Ok(o) => {
                        tracing::info!("✅ Successfully parsed CallChainOutput");
                        serde_json::to_value(AgenticOutput::CallChain(o)).ok()
                    }
                    Err(e) => {
                        tracing::warn!("Failed to parse CallChainOutput: {}", e);
                        None
                    }
                }
            }
            AnalysisType::ArchitectureAnalysis => {
                match serde_json::from_str::<ArchitectureAnalysisOutput>(&result.answer) {
                    Ok(o) => {
                        tracing::info!("✅ Successfully parsed ArchitectureAnalysisOutput");
                        serde_json::to_value(AgenticOutput::ArchitectureAnalysis(o)).ok()
                    }
                    Err(e) => {
                        tracing::warn!("Failed to parse ArchitectureAnalysisOutput: {}", e);
                        None
                    }
                }
            }
            AnalysisType::ApiSurfaceAnalysis => {
                match serde_json::from_str::<APISurfaceOutput>(&result.answer) {
                    Ok(o) => {
                        tracing::info!("✅ Successfully parsed APISurfaceOutput");
                        serde_json::to_value(AgenticOutput::APISurface(o)).ok()
                    }
                    Err(e) => {
                        tracing::warn!("Failed to parse APISurfaceOutput: {}", e);
                        None
                    }
                }
            }
            AnalysisType::ContextBuilder => {
                match serde_json::from_str::<ContextBuilderOutput>(&result.answer) {
                    Ok(o) => {
                        tracing::info!("✅ Successfully parsed ContextBuilderOutput");
                        serde_json::to_value(AgenticOutput::ContextBuilder(o)).ok()
                    }
                    Err(e) => {
                        tracing::warn!("Failed to parse ContextBuilderOutput: {}", e);
                        None
                    }
                }
            }
            AnalysisType::SemanticQuestion => {
                match serde_json::from_str::<SemanticQuestionOutput>(&result.answer) {
                    Ok(o) => {
                        tracing::info!("✅ Successfully parsed SemanticQuestionOutput");
                        serde_json::to_value(AgenticOutput::SemanticQuestion(o)).ok()
                    }
                    Err(e) => {
                        tracing::warn!("Failed to parse SemanticQuestionOutput: {}", e);
                        None
                    }
                }
            }
        };

        // Format result as JSON with structured output if available
        let response_json = if let Some(structured) = structured_output {
            serde_json::json!({
                "analysis_type": analysis_type.as_str(),
                "tier": format!("{:?}", tier),
                "query": query,
                "structured_output": structured,
                "steps_taken": result.steps_taken,
                "framework": "AutoAgents",
            })
        } else {
            // Fallback to original format if parsing failed
            serde_json::json!({
                "analysis_type": analysis_type.as_str(),
                "tier": format!("{:?}", tier),
                "query": query,
                "answer": result.answer,
                "findings": result.findings,
                "steps_taken": result.steps_taken,
                "framework": "AutoAgents",
            })
        };

        DebugLogger::log_agent_finish(true, Some(&response_json), None);

        // Stage 3: Agent complete (progress: 1.0)
        progress_notifier.notify_complete().await;

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&response_json)
                .unwrap_or_else(|_| "Error formatting AutoAgents result".to_string()),
        )]))
    }

    /// Stub when ai-enhanced feature is disabled
    #[cfg(not(feature = "ai-enhanced"))]
    async fn execute_agentic_workflow(
        &self,
        analysis_type: AnalysisType,
        query: &str,
        _peer: Peer<RoleServer>,
        _meta: Meta,
    ) -> Result<CallToolResult, McpError> {
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
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            // Use the aggressive MANDATORY instructions for automatic delivery
            // This is sent automatically in the initialize response
            // Also available via MCP prompt 'codegraph_initial_instructions'
            instructions: Some(INITIAL_INSTRUCTIONS.into()),
            capabilities: ServerCapabilities::builder()
                .enable_tools()
                .enable_prompts()
                .enable_logging()
                .build(),
            ..Default::default()
        }
    }

    fn list_prompts(
        &self,
        _request: Option<PaginatedRequestParam>,
        _context: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<ListPromptsResult, McpError>> + Send + '_ {
        async move {
            Ok(ListPromptsResult {
                prompts: vec![Prompt {
                    name: "codegraph_initial_instructions".to_string(),
                    title: Some("MANDATORY: CodeGraph Usage Protocol for AI Agents".to_string()),
                    description: Some(
                        "REQUIRED reading before using CodeGraph tools. Enforces context-efficient tool usage patterns. You MUST use CodeGraph agentic tools BEFORE grep/read/find. Includes tool selection decision tree, anti-patterns, and compliance checklist.".to_string()
                    ),
                    arguments: None,
                    icons: None,
                }],
                next_cursor: None,
            })
        }
    }

    fn get_prompt(
        &self,
        request: GetPromptRequestParam,
        _context: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<GetPromptResult, McpError>> + Send + '_ {
        let name = request.name.clone();
        async move {
            match name.as_str() {
                "codegraph_initial_instructions" => Ok(GetPromptResult {
                    description: Some(
                        "MANDATORY: CodeGraph Usage Protocol - You MUST read and follow these instructions before using any CodeGraph tools".to_string()
                    ),
                    messages: vec![
                        PromptMessage {
                            role: PromptMessageRole::User,
                            content: PromptMessageContent::text(
                                "Please read the CodeGraph Initial Instructions below. These guidelines will help you use CodeGraph tools efficiently and avoid wasting context by reading unnecessary files."
                            ),
                        },
                        PromptMessage {
                            role: PromptMessageRole::Assistant,
                            content: PromptMessageContent::text(INITIAL_INSTRUCTIONS),
                        },
                    ],
                }),
                _ => Err(McpError::invalid_params(
                    format!("Unknown prompt: {}", name),
                    None
                )),
            }
        }
    }
}
