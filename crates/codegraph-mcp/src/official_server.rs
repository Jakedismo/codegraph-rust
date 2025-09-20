/// Official MCP SDK Implementation for CodeGraph
///
/// This module provides full protocol compliance using the official rmcp SDK
/// while preserving all revolutionary CodeGraph functionality.

use rmcp::{
    handler::server::router::tool::ToolRouter,
    model::{CallToolResult, Content, ServerCapabilities, ServerInfo},
    tool, tool_handler, tool_router,
    ErrorData as McpError,
    ServerHandler,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::Mutex;

#[cfg(feature = "qwen-integration")]
use crate::qwen::{QwenClient, QwenConfig};
#[cfg(feature = "qwen-integration")]
use crate::cache::{CacheConfig, init_cache};

/// Official MCP-compliant CodeGraph server with revolutionary capabilities
#[derive(Clone)]
pub struct CodeGraphMCPServer {
    /// Graph state for semantic analysis
    graph: Arc<Mutex<codegraph_graph::CodeGraph>>,

    /// Revolutionary Qwen2.5-Coder-14B-128K integration
    #[cfg(feature = "qwen-integration")]
    qwen_client: Option<QwenClient>,

    /// Official MCP tool router
    tool_router: ToolRouter<Self>,
}

/// Parameters for enhanced search tool
#[derive(Debug, Deserialize, Serialize)]
pub struct EnhancedSearchParams {
    /// Natural language description of functionality to search for
    pub query: String,
    /// Include Qwen2.5-Coder AI analysis of results
    #[serde(default = "default_include_analysis")]
    pub include_analysis: bool,
    /// Maximum number of search results to return
    #[serde(default = "default_max_results")]
    pub max_results: usize,
}

fn default_include_analysis() -> bool { true }
fn default_max_results() -> usize { 10 }

/// Parameters for semantic intelligence tool
#[derive(Debug, Deserialize, Serialize)]
pub struct SemanticIntelligenceParams {
    /// Analysis focus area or specific question about the codebase
    pub query: String,
    /// Type of analysis to perform
    #[serde(default = "default_task_type")]
    pub task_type: String,
    /// Maximum context tokens to use (out of 128K available)
    #[serde(default = "default_max_context_tokens")]
    pub max_context_tokens: usize,
}

fn default_task_type() -> String { "comprehensive_analysis".to_string() }
fn default_max_context_tokens() -> usize { 80000 }

/// Parameters for impact analysis tool
#[derive(Debug, Deserialize, Serialize)]
pub struct ImpactAnalysisParams {
    /// Name of the function to analyze for impact
    pub target_function: String,
    /// Path to the file containing the target function
    pub file_path: String,
    /// Type of change being proposed
    #[serde(default = "default_change_type")]
    pub change_type: String,
}

fn default_change_type() -> String { "modify".to_string() }

/// Parameters for pattern detection tool
#[derive(Debug, Deserialize, Serialize)]
pub struct PatternDetectionParams {
    /// Scope of pattern detection analysis
    #[serde(default = "default_scope")]
    pub scope: String,
    /// Specific area to focus pattern detection on
    #[serde(default = "default_focus_area")]
    pub focus_area: String,
    /// Maximum code samples to analyze for patterns
    #[serde(default = "default_max_results")]
    pub max_results: usize,
}

fn default_scope() -> String { "project".to_string() }
fn default_focus_area() -> String { "all_patterns".to_string() }

/// Parameters for vector search tool
#[derive(Debug, Deserialize, Serialize)]
pub struct VectorSearchParams {
    /// Search query text
    pub query: String,
    /// Optional file paths to restrict search
    pub paths: Option<Vec<String>>,
    /// Optional programming languages to filter
    pub langs: Option<Vec<String>>,
    /// Maximum results to return
    #[serde(default = "default_max_results")]
    pub limit: usize,
}

impl CodeGraphMCPServer {
    pub fn new() -> Self {
        Self {
            graph: Arc::new(Mutex::new(
                codegraph_graph::CodeGraph::new().expect("Failed to initialize CodeGraph")
            )),
            #[cfg(feature = "qwen-integration")]
            qwen_client: None, // Will be initialized async
            tool_router: Self::tool_router(),
        }
    }

    /// Initialize the revolutionary Qwen2.5-Coder integration
    pub async fn initialize_qwen(&mut self) {
        #[cfg(feature = "qwen-integration")]
        {
            // Initialize intelligent cache
            let cache_config = CacheConfig::default();
            init_cache(cache_config);
            tracing::info!("✅ Intelligent response cache initialized");

            let config = QwenConfig::default();
            let client = QwenClient::new(config.clone());

            match client.check_availability().await {
                Ok(true) => {
                    tracing::info!("✅ Qwen2.5-Coder-14B-128K available for CodeGraph intelligence");
                    self.qwen_client = Some(client);
                }
                Ok(false) => {
                    tracing::warn!("⚠️ Qwen2.5-Coder model not found. Install with: ollama pull {}", config.model_name);
                }
                Err(e) => {
                    tracing::error!("❌ Failed to connect to Qwen2.5-Coder: {}", e);
                }
            }
        }
    }

    /// Revolutionary enhanced search with Qwen2.5-Coder intelligence
    #[tool(description = "Enhanced semantic search with Qwen2.5-Coder intelligence analysis")]
    pub async fn enhanced_search(
        &self,
        query: String,
        include_analysis: Option<bool>,
        max_results: Option<usize>,
    ) -> Result<CallToolResult, McpError> {
        let include_analysis = include_analysis.unwrap_or(true);
        let max_results = max_results.unwrap_or(10);
        let max_results = max_results.min(50); // Cap at 50

        // Use existing revolutionary search logic
        let search_results = match crate::server::bin_search_with_scores(
            query.clone(),
            None,
            None,
            max_results * 2
        ).await {
            Ok(results) => results,
            Err(e) => return Err(McpError {
                code: rmcp::model::ErrorCode(-32603),
                message: format!("Search failed: {}", e).into(),
                data: None,
            }),
        };

        #[cfg(feature = "qwen-integration")]
        if include_analysis && self.qwen_client.is_some() {
            // Use revolutionary Qwen analysis
            let search_context = crate::server::build_search_context(&search_results, &query);

            if let Some(qwen_client) = &self.qwen_client {
                match qwen_client.analyze_codebase(&query, &search_context).await {
                    Ok(qwen_result) => {
                        // Record performance metrics
                        crate::performance::record_qwen_operation(
                            "enhanced_search",
                            qwen_result.processing_time,
                            qwen_result.context_tokens,
                            qwen_result.completion_tokens,
                            qwen_result.confidence_score,
                        );

                        // Build enhanced response with Qwen intelligence
                        let enhanced_response = serde_json::json!({
                            "search_results": search_results["results"],
                            "ai_analysis": qwen_result.text,
                            "intelligence_metadata": {
                                "model_used": qwen_result.model_used,
                                "processing_time_ms": qwen_result.processing_time.as_millis(),
                                "context_tokens": qwen_result.context_tokens,
                                "completion_tokens": qwen_result.completion_tokens,
                                "confidence_score": qwen_result.confidence_score,
                                "context_window_used": qwen_client.config.context_window
                            },
                            "generation_guidance": crate::prompts::extract_enhanced_generation_guidance(&qwen_result.text),
                            "quality_assessment": crate::prompts::extract_enhanced_quality_assessment(&qwen_result.text)
                        });

                        // Cache the response
                        let _ = crate::cache::cache_response(
                            &query,
                            &search_context,
                            enhanced_response.clone(),
                            qwen_result.confidence_score,
                            qwen_result.processing_time,
                            qwen_result.context_tokens,
                            qwen_result.completion_tokens,
                        ).await;

                        return Ok(CallToolResult::success(vec![Content::text(
                            serde_json::to_string_pretty(&enhanced_response)
                                .unwrap_or_else(|_| "Error formatting response".to_string())
                        )]));
                    }
                    Err(e) => {
                        tracing::error!("Qwen analysis failed: {}", e);
                        // Fall back to basic search results
                    }
                }
            }
        }

        // Return basic search results
        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&search_results)
                .unwrap_or_else(|_| "Error formatting results".to_string())
        )]))
    }

    /// Revolutionary semantic intelligence with 128K context analysis
    #[tool(description = "Comprehensive codebase analysis using Qwen2.5-Coder's 128K context window")]
    pub async fn semantic_intelligence(
        &self,
        query: String,
        task_type: Option<String>,
        max_context_tokens: Option<usize>,
    ) -> Result<CallToolResult, McpError> {
        let _task_type = task_type.unwrap_or_else(|| "comprehensive_analysis".to_string());
        let max_context_tokens = max_context_tokens.unwrap_or(80000).min(120000); // Cap at 120K

        #[cfg(feature = "qwen-integration")]
        {
            if let Some(qwen_client) = &self.qwen_client {
                // Check cache first
                if let Some(cached_response) = crate::cache::get_cached_response(&query, "semantic_intelligence").await {
                    tracing::info!("🚀 Cache hit for semantic intelligence: {}", query);
                    return Ok(CallToolResult::success(vec![Content::text(
                        serde_json::to_string_pretty(&cached_response)
                            .unwrap_or_else(|_| "Error formatting cached response".to_string())
                    )]));
                }

            // Build comprehensive context using existing revolutionary logic
            // Create temporary ServerState for function call compatibility
            #[cfg(feature = "qwen-integration")]
            let temp_state = crate::server::ServerState {
                graph: self.graph.clone(),
                qwen_client: self.qwen_client.clone(),
            };
            #[cfg(not(feature = "qwen-integration"))]
            let temp_state = crate::server::ServerState {
                graph: self.graph.clone(),
            };

            let codebase_context = match crate::server::build_comprehensive_context(
                &temp_state,
                &query,
                max_context_tokens,
            ).await {
                Ok(context) => context,
                Err(e) => return Err(McpError {
                    code: rmcp::model::ErrorCode(-32603),
                    message: format!("Context building failed: {}", e).into(),
                    data: None,
                }),
            };

            // Use revolutionary Qwen2.5-Coder analysis
            match qwen_client.analyze_codebase(&query, &codebase_context).await {
                Ok(analysis_result) => {
                    // Record performance metrics
                    crate::performance::record_qwen_operation(
                        "semantic_intelligence",
                        analysis_result.processing_time,
                        analysis_result.context_tokens,
                        analysis_result.completion_tokens,
                        analysis_result.confidence_score,
                    );

                    // Build comprehensive response
                    let response = serde_json::json!({
                        "task_type": task_type,
                        "user_query": &query,
                        "comprehensive_analysis": analysis_result.text,
                        "codebase_context_summary": crate::server::build_context_summary(&codebase_context),
                        "model_performance": {
                            "model_used": analysis_result.model_used,
                            "processing_time_ms": analysis_result.processing_time.as_millis(),
                            "context_tokens_used": analysis_result.context_tokens,
                            "completion_tokens": analysis_result.completion_tokens,
                            "confidence_score": analysis_result.confidence_score,
                            "context_window_total": qwen_client.config.context_window
                        },
                        "generation_guidance": crate::prompts::extract_enhanced_generation_guidance(&analysis_result.text),
                        "structured_insights": crate::prompts::extract_enhanced_structured_insights(&analysis_result.text),
                        "mcp_metadata": {
                            "tool_version": "1.0.0",
                            "recommended_for": ["claude", "gpt-4", "custom-agents"],
                            "context_quality": analysis_result.confidence_score
                        }
                    });

                    // Cache the response
                    let _ = crate::cache::cache_response(
                        &query,
                        &codebase_context,
                        response.clone(),
                        analysis_result.confidence_score,
                        analysis_result.processing_time,
                        analysis_result.context_tokens,
                        analysis_result.completion_tokens,
                    ).await;

                    return Ok(CallToolResult::success(vec![Content::text(
                        serde_json::to_string_pretty(&response)
                            .unwrap_or_else(|_| "Error formatting response".to_string())
                    )]));
                }
                Err(e) => {
                    return Err(McpError {
                        code: rmcp::model::ErrorCode(-32603),
                        message: format!("Analysis failed: {}", e).into(),
                        data: None,
                    });
                }
            }
            } // Close qwen_client if statement

            // Fallback if qwen_client is None
            Err(McpError {
                code: rmcp::model::ErrorCode(-32601),
                message: "Qwen2.5-Coder not available".into(),
                data: None,
            })
        }

        #[cfg(not(feature = "qwen-integration"))]
        {
            Err(McpError {
                code: rmcp::model::ErrorCode(-32601),
                message: "Qwen integration not available".into(),
                data: None,
            })
        }
    }

    /// Revolutionary impact analysis - predict what breaks before changes
    #[tool(description = "Analyze impact of proposed code changes using dependency mapping and Qwen intelligence")]
    pub async fn impact_analysis(
        &self,
        target_function: String,
        file_path: String,
        change_type: Option<String>,
    ) -> Result<CallToolResult, McpError> {
        let change_type = change_type.unwrap_or_else(|| "modify".to_string());
        #[cfg(feature = "qwen-integration")]
        {
            let qwen_client = self.qwen_client.as_ref()
                .ok_or_else(|| McpError {
                    code: rmcp::model::ErrorCode(-32601),
                    message: "Qwen2.5-Coder not available. Please install: ollama pull qwen2.5-coder-14b-128k".into(),
                    data: None,
                })?;

            // Build dependency context using existing revolutionary logic
            #[cfg(feature = "qwen-integration")]
            let temp_state = crate::server::ServerState {
                graph: self.graph.clone(),
                qwen_client: self.qwen_client.clone(),
            };
            #[cfg(not(feature = "qwen-integration"))]
            let temp_state = crate::server::ServerState {
                graph: self.graph.clone(),
            };

            let dependency_context = match crate::server::build_dependency_context(
                &temp_state,
                &target_function,
                &file_path,
            ).await {
                Ok(context) => context,
                Err(e) => return Err(McpError {
                    code: rmcp::model::ErrorCode(-32603),
                    message: format!("Dependency analysis failed: {}", e).into(),
                    data: None,
                }),
            };

            // Use revolutionary Qwen2.5-Coder for impact analysis
            let impact_prompt = crate::prompts::build_impact_analysis_prompt(
                &target_function,
                &file_path,
                &dependency_context,
                &change_type
            );

            match qwen_client.analyze_codebase(&impact_prompt, "").await {
                Ok(analysis_result) => {
                    // Record performance metrics
                    crate::performance::record_qwen_operation(
                        "impact_analysis",
                        analysis_result.processing_time,
                        analysis_result.context_tokens,
                        analysis_result.completion_tokens,
                        analysis_result.confidence_score,
                    );

                    // Build comprehensive impact response
                    let response = serde_json::json!({
                        "target": {
                            "function": target_function,
                            "file_path": file_path,
                            "change_type": change_type
                        },
                        "comprehensive_impact_analysis": analysis_result.text,
                        "dependency_analysis": crate::server::parse_dependency_info(&dependency_context),
                        "risk_assessment": crate::server::extract_risk_level(&analysis_result.text),
                        "affected_components": crate::server::extract_affected_components(&analysis_result.text),
                        "testing_requirements": crate::server::extract_testing_requirements(&analysis_result.text),
                        "implementation_plan": crate::server::extract_implementation_plan(&analysis_result.text),
                        "model_performance": {
                            "model_used": analysis_result.model_used,
                            "processing_time_ms": analysis_result.processing_time.as_millis(),
                            "context_tokens": analysis_result.context_tokens,
                            "completion_tokens": analysis_result.completion_tokens,
                            "confidence_score": analysis_result.confidence_score
                        },
                        "safety_recommendations": crate::server::extract_safety_recommendations(&analysis_result.text),
                        "mcp_metadata": {
                            "tool_version": "1.0.0",
                            "analysis_type": "impact_assessment",
                            "recommended_for": ["claude", "gpt-4", "custom-agents"]
                        }
                    });

                    Ok(CallToolResult::success(vec![Content::text(
                        serde_json::to_string_pretty(&response)
                            .unwrap_or_else(|_| "Error formatting response".to_string())
                    )]))
                }
                Err(e) => Err(McpError {
                    code: rmcp::model::ErrorCode(-32603),
                    message: format!("Impact analysis failed: {}", e).into(),
                    data: None,
                })
            }
        }
        #[cfg(not(feature = "qwen-integration"))]
        {
            Err(McpError {
                code: rmcp::model::ErrorCode(-32601),
                message: "Qwen integration not available".to_string(),
                data: None,
            })
        }
    }

    /// Team intelligence and pattern detection
    #[tool(description = "Detect team patterns and conventions using existing semantic analysis")]
    pub async fn pattern_detection(
        &self,
        scope: Option<String>,
        focus_area: Option<String>,
        max_results: Option<usize>,
    ) -> Result<CallToolResult, McpError> {
        let scope = scope.unwrap_or_else(|| "project".to_string());
        let focus_area = focus_area.unwrap_or_else(|| "all_patterns".to_string());
        let max_results = max_results.unwrap_or(50);
        // Use existing revolutionary pattern detection logic
        let search_query = match focus_area.as_str() {
            "naming" => "function class variable naming",
            "error_handling" => "try catch throw error exception",
            "imports" => "import require use from",
            "architecture" => "service component module class",
            "testing" => "test spec assert expect describe",
            _ => "function class method", // General pattern detection
        };

        let search_results = match crate::server::bin_search_with_scores(
            search_query.to_string(),
            None,
            None,
            max_results
        ).await {
            Ok(results) => results,
            Err(e) => return Err(McpError {
                code: rmcp::model::ErrorCode(-32603),
                message: format!("Pattern search failed: {}", e).into(),
                data: None,
            }),
        };

        // Use existing pattern detection
        let pattern_detector = crate::pattern_detector::PatternDetector::new(
            crate::pattern_detector::PatternConfig::default()
        );

        let team_intelligence = match pattern_detector
            .detect_patterns_from_search(&search_results)
            .await
        {
            Ok(intelligence) => intelligence,
            Err(e) => return Err(McpError {
                code: rmcp::model::ErrorCode(-32603),
                message: format!("Pattern detection failed: {}", e).into(),
                data: None,
            }),
        };

        // Build response using existing logic
        let response = serde_json::json!({
            "scope": scope,
            "focus_area": focus_area,
            "team_intelligence": crate::pattern_detector::team_intelligence_to_json(&team_intelligence),
            "pattern_summary": {
                "total_patterns_detected": team_intelligence.patterns.len(),
                "high_confidence_patterns": team_intelligence.patterns.iter().filter(|p| p.confidence > 0.8).count(),
                "team_conventions_identified": team_intelligence.conventions.len(),
                "overall_quality_score": team_intelligence.quality_metrics.overall_score,
                "consistency_score": team_intelligence.quality_metrics.consistency_score
            },
            "actionable_insights": crate::server::generate_pattern_insights(&team_intelligence),
            "mcp_metadata": {
                "tool_version": "1.0.0",
                "analysis_method": "semantic_analysis_without_external_model",
                "uses_existing_codegraph_infrastructure": true,
                "recommended_for": ["claude", "gpt-4", "custom-agents"]
            }
        });

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&response)
                .unwrap_or_else(|_| "Error formatting response".to_string())
        )]))
    }

    /// High-performance vector search
    #[tool(description = "Basic vector similarity search using FAISS + 90K lines of analysis")]
    pub async fn vector_search(
        &self,
        query: String,
        paths: Option<Vec<String>>,
        langs: Option<Vec<String>>,
        limit: Option<usize>,
    ) -> Result<CallToolResult, McpError> {
        let limit = limit.unwrap_or(10);
        let res = match crate::server::bin_search_with_scores(
            query.clone(),
            paths.clone(),
            langs.clone(),
            limit
        ).await {
            Ok(results) => results,
            Err(e) => return Err(McpError {
                code: rmcp::model::ErrorCode(-32603),
                message: format!("Vector search failed: {}", e).into(),
                data: None,
            }),
        };

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&res)
                .unwrap_or_else(|_| "Error formatting results".to_string())
        )]))
    }

    /// Performance monitoring and metrics
    #[tool(description = "Get real-time performance metrics for Qwen2.5-Coder operations")]
    pub async fn performance_metrics(&self) -> Result<CallToolResult, McpError> {
        let mut metrics = crate::performance::get_performance_summary();

        // Add cache performance data
        if let Some(cache_stats) = crate::cache::get_cache_stats() {
            metrics["cache_performance"] = serde_json::json!(cache_stats);
        }

        // Add cache performance analysis
        if let Some(cache_analysis) = crate::cache::analyze_cache_performance() {
            metrics["cache_analysis"] = serde_json::json!(cache_analysis);
        }

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&metrics)
                .unwrap_or_else(|_| "Error formatting metrics".to_string())
        )]))
    }

    /// Intelligent cache statistics and optimization
    #[tool(description = "Get intelligent cache statistics and performance analysis")]
    pub async fn cache_stats(&self) -> Result<CallToolResult, McpError> {
        let cache_stats = crate::cache::get_cache_stats();
        let cache_analysis = crate::cache::analyze_cache_performance();

        let response = serde_json::json!({
            "cache_statistics": cache_stats,
            "performance_analysis": cache_analysis,
            "recommendations": crate::server::generate_cache_recommendations(&cache_stats, &cache_analysis),
            "cache_health": crate::server::assess_cache_health(&cache_stats)
        });

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&response)
                .unwrap_or_else(|_| "Error formatting cache stats".to_string())
        )]))
    }
}

/// Tool router implementation for CodeGraph MCP server
#[tool_router]
impl CodeGraphMCPServer {}

/// Official MCP ServerHandler implementation
#[tool_handler]
impl ServerHandler for CodeGraphMCPServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            instructions: Some(
                "CodeGraph provides revolutionary AI codebase intelligence through local SOTA models. \
                Features include:\n\
                • Enhanced semantic search with Qwen2.5-Coder-14B-128K analysis\n\
                • Revolutionary impact prediction before code changes\n\
                • Team intelligence and pattern detection\n\
                • 128K context window comprehensive analysis\n\
                • Complete local-first processing with zero external dependencies\n\
                • Performance optimized for high-memory systems\n\n\
                This is the world's most advanced local-first AI development platform.".into()
            ),
            capabilities: ServerCapabilities::builder()
                .enable_tools()
                .enable_logging()
                .build(),
            ..Default::default()
        }
    }
}