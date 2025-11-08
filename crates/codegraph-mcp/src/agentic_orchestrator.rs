// ABOUTME: Agentic LLM orchestrator for multi-step graph analysis workflows
// ABOUTME: Coordinates LLM reasoning with SurrealDB graph tool calls for comprehensive code intelligence

use crate::context_aware_limits::ContextTier;
use crate::error::McpError;
use crate::graph_tool_executor::GraphToolExecutor;
use crate::graph_tool_schemas::{GraphToolSchemas, ToolSchema};
use crate::Result;
use codegraph_ai::llm_provider::{GenerationConfig, LLMProvider, Message, MessageRole};
use futures::future::BoxFuture;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value as JsonValue};
use std::sync::Arc;
use std::time::Instant;
use tracing::{debug, info, instrument, warn};

/// Configuration for agentic workflow behavior
#[derive(Debug, Clone)]
pub struct AgenticConfig {
    /// Maximum number of reasoning steps (tier-aware)
    pub max_steps: usize,
    /// Maximum time allowed for entire workflow in seconds
    pub max_duration_secs: u64,
    /// Temperature for LLM tool calling decisions
    pub temperature: f32,
    /// Maximum tokens for each LLM response
    pub max_tokens: usize,
    /// Enable LRU caching for SurrealDB tool results
    pub enable_cache: bool,
    /// Maximum number of cache entries (LRU eviction)
    pub cache_size: usize,
}

impl AgenticConfig {
    /// Create tier-aware configuration
    pub fn from_tier(tier: ContextTier) -> Self {
        Self::from_tier_with_override(tier, None)
    }

    /// Create tier-aware configuration with optional max_tokens override
    pub fn from_tier_with_override(tier: ContextTier, max_tokens_override: Option<usize>) -> Self {
        let (max_steps, default_max_tokens) = match tier {
            ContextTier::Small => (5, 2048),     // Conservative for small models
            ContextTier::Medium => (10, 4096),   // Moderate for medium models
            ContextTier::Large => (15, 8192),    // Generous for large models
            ContextTier::Massive => (20, 16384), // Very generous for massive models
        };

        let max_tokens = max_tokens_override.unwrap_or(default_max_tokens);

        Self {
            max_steps,
            max_duration_secs: 300, // 5 minutes max
            temperature: 0.1,       // Low temperature for focused tool calling
            max_tokens,
            enable_cache: true, // Enable caching by default
            cache_size: 100,    // 100 entries (~1MB memory with typical JSON results)
        }
    }
}

/// A single step in the agentic workflow
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReasoningStep {
    /// Step number (1-indexed)
    pub step_number: usize,
    /// LLM's reasoning about what to do
    pub reasoning: String,
    /// Tool name to execute (if any)
    pub tool_name: Option<String>,
    /// Tool parameters (if tool is being called)
    pub tool_params: Option<JsonValue>,
    /// Tool execution result (if tool was called)
    pub tool_result: Option<JsonValue>,
    /// Whether this is the final step
    pub is_final: bool,
}

/// Result of the agentic workflow
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgenticResult {
    /// Final analysis/answer from the LLM
    pub final_answer: String,
    /// All reasoning steps taken
    pub steps: Vec<ReasoningStep>,
    /// Total number of steps executed
    pub total_steps: usize,
    /// Total time taken for the workflow
    pub duration_ms: u64,
    /// Total tokens used across all LLM calls
    pub total_tokens: usize,
    /// Whether the workflow reached a natural conclusion
    pub completed_successfully: bool,
    /// Termination reason (e.g., "max_steps", "timeout", "success")
    pub termination_reason: String,
}

impl AgenticResult {
    /// Extract a formatted reasoning trace for logging/analysis
    pub fn reasoning_trace(&self) -> String {
        let mut trace = String::new();
        trace.push_str(&format!(
            "=== Agentic Workflow Trace ({} steps, {}ms, {} tokens) ===\n\n",
            self.total_steps, self.duration_ms, self.total_tokens
        ));

        for (idx, step) in self.steps.iter().enumerate() {
            trace.push_str(&format!("--- Step {} ---\n", idx + 1));
            trace.push_str(&format!("Reasoning: {}\n", step.reasoning));

            if let Some(tool_name) = &step.tool_name {
                trace.push_str(&format!("Tool: {}\n", tool_name));
                if let Some(params) = &step.tool_params {
                    trace.push_str(&format!(
                        "Parameters: {}\n",
                        serde_json::to_string_pretty(params).unwrap_or_else(|_| params.to_string())
                    ));
                }
                if let Some(result) = &step.tool_result {
                    trace.push_str(&format!(
                        "Result: {}\n",
                        serde_json::to_string_pretty(result).unwrap_or_else(|_| result.to_string())
                    ));
                }
            }

            if step.is_final {
                trace.push_str("[FINAL STEP]\n");
            }
            trace.push_str("\n");
        }

        trace.push_str(&format!(
            "=== Workflow {} ({}) ===\n",
            if self.completed_successfully {
                "COMPLETED"
            } else {
                "INCOMPLETE"
            },
            self.termination_reason
        ));

        trace
    }

    /// Get all tool calls made during the workflow
    pub fn tool_calls(&self) -> Vec<(&str, &JsonValue)> {
        self.steps
            .iter()
            .filter_map(|step| step.tool_name.as_deref().zip(step.tool_params.as_ref()))
            .collect()
    }

    /// Get tool call statistics
    pub fn tool_call_stats(&self) -> ToolCallStats {
        let total_tool_calls = self
            .steps
            .iter()
            .filter(|step| step.tool_name.is_some())
            .count();

        let mut tool_usage: std::collections::HashMap<String, usize> =
            std::collections::HashMap::new();
        for step in &self.steps {
            if let Some(tool_name) = &step.tool_name {
                *tool_usage.entry(tool_name.clone()).or_insert(0) += 1;
            }
        }

        ToolCallStats {
            total_tool_calls,
            unique_tools_used: tool_usage.len(),
            tool_usage,
            avg_tokens_per_step: if self.total_steps > 0 {
                self.total_tokens / self.total_steps
            } else {
                0
            },
        }
    }
}

/// Statistics about tool calls in an agentic workflow
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallStats {
    /// Total number of tool calls made
    pub total_tool_calls: usize,
    /// Number of unique tools used
    pub unique_tools_used: usize,
    /// Usage count per tool
    pub tool_usage: std::collections::HashMap<String, usize>,
    /// Average tokens used per step
    pub avg_tokens_per_step: usize,
}

/// Callback for sending progress notifications during workflow execution
/// Takes (progress, total) and returns a future that sends the notification
pub type ProgressCallback = Arc<dyn Fn(f64, Option<f64>) -> BoxFuture<'static, ()> + Send + Sync>;

/// Agentic orchestrator that coordinates LLM reasoning with tool execution
pub struct AgenticOrchestrator {
    /// LLM provider for reasoning and tool calling decisions
    llm_provider: Arc<dyn LLMProvider>,
    /// Tool executor for SurrealDB graph functions
    tool_executor: Arc<GraphToolExecutor>,
    /// Configuration for agentic behavior
    config: AgenticConfig,
    /// Context tier for this orchestrator
    tier: ContextTier,
    /// Optional callback for sending progress notifications
    progress_callback: Option<ProgressCallback>,
}

impl AgenticOrchestrator {
    /// Create a new agentic orchestrator
    pub fn new(
        llm_provider: Arc<dyn LLMProvider>,
        tool_executor: Arc<GraphToolExecutor>,
        tier: ContextTier,
    ) -> Self {
        Self::new_with_override(llm_provider, tool_executor, tier, None, None)
    }

    /// Create a new agentic orchestrator with optional max_tokens override and progress callback
    pub fn new_with_override(
        llm_provider: Arc<dyn LLMProvider>,
        tool_executor: Arc<GraphToolExecutor>,
        tier: ContextTier,
        max_tokens_override: Option<usize>,
        progress_callback: Option<ProgressCallback>,
    ) -> Self {
        let config = AgenticConfig::from_tier_with_override(tier, max_tokens_override);
        Self {
            llm_provider,
            tool_executor,
            config,
            tier,
            progress_callback,
        }
    }

    /// Create with custom configuration and optional progress callback
    pub fn with_config(
        llm_provider: Arc<dyn LLMProvider>,
        tool_executor: Arc<GraphToolExecutor>,
        tier: ContextTier,
        config: AgenticConfig,
        progress_callback: Option<ProgressCallback>,
    ) -> Self {
        Self {
            llm_provider,
            tool_executor,
            config,
            tier,
            progress_callback,
        }
    }

    /// Execute agentic workflow for a user query
    ///
    /// The LLM will analyze the query, decide which tools to call, execute them,
    /// and iteratively refine its analysis until it has a complete answer.
    pub async fn execute(&self, user_query: &str, context: &str) -> Result<AgenticResult> {
        let start_time = Instant::now();
        info!(
            "🤖 Starting agentic workflow: tier={:?}, max_steps={}",
            self.tier, self.config.max_steps
        );

        let mut steps = Vec::new();
        let mut conversation_history = self.build_initial_messages(user_query, context);
        let mut total_tokens = 0;

        let mut termination_reason = "unknown".to_string();
        let mut completed_successfully = false;

        for step_number in 1..=self.config.max_steps {
            // Check timeout
            if start_time.elapsed().as_secs() > self.config.max_duration_secs {
                warn!("⏱️ Agentic workflow exceeded timeout");
                termination_reason = "timeout".to_string();
                break;
            }

            debug!("📍 Agentic step {}/{}", step_number, self.config.max_steps);

            // Send progress notification at step start
            if let Some(ref callback) = self.progress_callback {
                let progress = step_number as f64;
                let total = Some(self.config.max_steps as f64);
                callback(progress, total).await;
            }

            // Get LLM decision
            let gen_config = GenerationConfig {
                temperature: self.config.temperature,
                max_tokens: Some(self.config.max_tokens),
                ..Default::default()
            };

            let llm_response = self
                .llm_provider
                .generate_chat(&conversation_history, &gen_config)
                .await
                .map_err(|e| McpError::Protocol(format!("LLM generation failed: {}", e)))?;

            total_tokens += llm_response.total_tokens.unwrap_or(0);

            // Parse LLM response to extract reasoning and tool call
            let step = self.parse_llm_response(step_number, &llm_response.content)?;

            // Add assistant's reasoning to conversation
            conversation_history.push(Message {
                role: MessageRole::Assistant,
                content: llm_response.content.clone(),
            });

            // Execute tool if requested
            let mut executed_step = step.clone();
            if let (Some(tool_name), Some(tool_params)) = (&step.tool_name, &step.tool_params) {
                let tool_start = Instant::now();
                info!(
                    tool = %tool_name,
                    params = %serde_json::to_string(tool_params).unwrap_or_else(|_| "{}".to_string()),
                    "🔧 Executing tool"
                );

                let tool_result = self
                    .tool_executor
                    .execute(tool_name, tool_params.clone())
                    .await?;

                let tool_duration = tool_start.elapsed();
                info!(
                    tool = %tool_name,
                    duration_ms = tool_duration.as_millis(),
                    result_size = tool_result.to_string().len(),
                    "✓ Tool execution completed"
                );

                executed_step.tool_result = Some(tool_result.clone());

                // Send progress notification after tool completion
                if let Some(ref callback) = self.progress_callback {
                    let progress = step_number as f64 + 0.5; // Half-step increment for tool completion
                    let total = Some(self.config.max_steps as f64);
                    callback(progress, total).await;
                }

                // Add tool result to conversation
                conversation_history.push(Message {
                    role: MessageRole::User,
                    content: format!(
                        "Tool execution completed:\n\n{}",
                        serde_json::to_string_pretty(&tool_result)
                            .unwrap_or_else(|_| tool_result.to_string())
                    ),
                });
            }

            let is_final = executed_step.is_final;
            steps.push(executed_step);

            // Check if workflow is complete
            if is_final {
                info!("✅ Agentic workflow completed successfully");
                termination_reason = "success".to_string();
                completed_successfully = true;
                break;
            }
        }

        // If we exhausted max_steps without completion
        if !completed_successfully && termination_reason == "unknown" {
            warn!("⚠️ Agentic workflow reached max_steps limit");
            termination_reason = "max_steps".to_string();
        }

        // Extract final answer from last step
        let final_answer = steps
            .last()
            .map(|step| step.reasoning.clone())
            .unwrap_or_else(|| "No answer generated".to_string());

        let duration_ms = start_time.elapsed().as_millis() as u64;
        let total_steps = steps.len();

        info!(
            "🏁 Agentic workflow finished: steps={}, duration={}ms, tokens={}, success={}",
            total_steps, duration_ms, total_tokens, completed_successfully
        );

        Ok(AgenticResult {
            final_answer,
            steps,
            total_steps,
            duration_ms,
            total_tokens,
            completed_successfully,
            termination_reason,
        })
    }

    /// Build initial conversation messages with system prompt and user query
    fn build_initial_messages(&self, user_query: &str, context: &str) -> Vec<Message> {
        let system_prompt = self.build_system_prompt();
        let user_message = self.build_user_message(user_query, context);

        vec![
            Message {
                role: MessageRole::System,
                content: system_prompt,
            },
            Message {
                role: MessageRole::User,
                content: user_message,
            },
        ]
    }

    /// Build tier-aware system prompt with tool descriptions
    fn build_system_prompt(&self) -> String {
        let available_tools = GraphToolSchemas::all();
        let tools_json =
            serde_json::to_string_pretty(&available_tools).unwrap_or_else(|_| "[]".to_string());

        let tier_guidance = match self.tier {
            ContextTier::Small => {
                "You have a limited context window. Be concise and focused. \
                 Prefer single targeted tool calls over multiple exploratory calls."
            }
            ContextTier::Medium => {
                "You have a moderate context window. Balance thoroughness with efficiency. \
                 You can make several tool calls but should avoid redundancy."
            }
            ContextTier::Large => {
                "You have a large context window. Be thorough and comprehensive. \
                 You can make multiple tool calls to build complete understanding."
            }
            ContextTier::Massive => {
                "You have a massive context window. Be extremely thorough and exploratory. \
                 You can make many tool calls to achieve deep comprehensive analysis."
            }
        };

        format!(
            "You are an expert code analysis agent with access to powerful graph analysis tools.\n\n\
            YOUR CAPABILITIES:\n\
            You can analyze codebases using SurrealDB-powered graph functions. \
            These tools give you deep insights into code structure, dependencies, and architecture.\n\n\
            AVAILABLE TOOLS:\n\
            {}\n\n\
            RESPONSE FORMAT:\n\
            For each step, respond in this exact JSON format:\n\
            {{\n  \
              \"reasoning\": \"Your analysis of what to do next\",\n  \
              \"tool_call\": {{\n    \
                \"tool_name\": \"name_of_tool_to_call\",\n    \
                \"parameters\": {{ tool parameters }}\n  \
              }},\n  \
              \"is_final\": false\n\
            }}\n\n\
            When you have enough information to answer the user's question completely, \
            respond with:\n\
            {{\n  \
              \"reasoning\": \"Your final comprehensive answer\",\n  \
              \"tool_call\": null,\n  \
              \"is_final\": true\n\
            }}\n\n\
            CONTEXT TIER GUIDANCE:\n\
            {}\n\n\
            IMPORTANT:\n\
            - Always explain your reasoning before calling tools\n\
            - Use structured tool calls - extract node IDs from previous results\n\
            - Build on previous tool results to avoid redundant calls\n\
            - Be strategic - plan your tool usage to minimize steps\n\
            - When you have sufficient information, provide a final comprehensive answer",
            tools_json, tier_guidance
        )
    }

    /// Build user message with query and context
    fn build_user_message(&self, user_query: &str, context: &str) -> String {
        if context.is_empty() {
            user_query.to_string()
        } else {
            format!(
                "USER QUERY:\n{}\n\nCODEBASE CONTEXT:\n{}",
                user_query, context
            )
        }
    }

    /// Parse LLM response to extract reasoning step
    fn parse_llm_response(&self, step_number: usize, response: &str) -> Result<ReasoningStep> {
        // Try to parse as JSON first
        if let Ok(parsed) = serde_json::from_str::<JsonValue>(response) {
            let reasoning = parsed["reasoning"]
                .as_str()
                .ok_or_else(|| McpError::Protocol("Missing 'reasoning' field".to_string()))?
                .to_string();

            let is_final = parsed["is_final"].as_bool().unwrap_or(false);

            let (tool_name, tool_params) = if let Some(tool_call) = parsed["tool_call"].as_object()
            {
                let name = tool_call.get("tool_name").and_then(|v| v.as_str());
                let params = tool_call.get("parameters").cloned();
                (name.map(|s| s.to_string()), params)
            } else {
                (None, None)
            };

            return Ok(ReasoningStep {
                step_number,
                reasoning,
                tool_name,
                tool_params,
                tool_result: None,
                is_final,
            });
        }

        // Fallback: treat entire response as reasoning, no tool call
        warn!("⚠️ LLM response not in expected JSON format, treating as final answer");
        Ok(ReasoningStep {
            step_number,
            reasoning: response.to_string(),
            tool_name: None,
            tool_params: None,
            tool_result: None,
            is_final: true, // Assume completion if format is wrong
        })
    }

    /// Get current configuration
    pub fn config(&self) -> &AgenticConfig {
        &self.config
    }

    /// Get context tier
    pub fn tier(&self) -> ContextTier {
        self.tier
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agentic_config_from_tier() {
        let small_config = AgenticConfig::from_tier(ContextTier::Small);
        assert_eq!(small_config.max_steps, 5);
        assert_eq!(small_config.max_tokens, 2048);

        let massive_config = AgenticConfig::from_tier(ContextTier::Massive);
        assert_eq!(massive_config.max_steps, 20);
        assert_eq!(massive_config.max_tokens, 16384);
    }

    #[test]
    fn test_parse_valid_json_response() {
        let orchestrator = create_test_orchestrator();

        let json_response = r#"{
            "reasoning": "I need to analyze dependencies",
            "tool_call": {
                "tool_name": "get_transitive_dependencies",
                "parameters": {
                    "node_id": "nodes:123",
                    "edge_type": "Calls",
                    "depth": 3
                }
            },
            "is_final": false
        }"#;

        let step = orchestrator
            .parse_llm_response(1, json_response)
            .expect("Should parse valid JSON");

        assert_eq!(step.step_number, 1);
        assert_eq!(step.reasoning, "I need to analyze dependencies");
        assert_eq!(
            step.tool_name,
            Some("get_transitive_dependencies".to_string())
        );
        assert!(!step.is_final);
    }

    #[test]
    fn test_parse_final_response() {
        let orchestrator = create_test_orchestrator();

        let json_response = r#"{
            "reasoning": "Based on the analysis, the answer is...",
            "tool_call": null,
            "is_final": true
        }"#;

        let step = orchestrator
            .parse_llm_response(1, json_response)
            .expect("Should parse final response");

        assert_eq!(step.step_number, 1);
        assert!(step.is_final);
        assert!(step.tool_name.is_none());
    }

    // Helper to create test orchestrator
    fn create_test_orchestrator() -> AgenticOrchestrator {
        use codegraph_graph::GraphFunctions;
        use std::sync::Arc;

        // This is a minimal test setup - in real tests, we'd use mock objects
        let graph_functions =
            Arc::new(GraphFunctions::new_memory().expect("Failed to create test graph"));
        let tool_executor = Arc::new(GraphToolExecutor::new(graph_functions));

        // For testing parse logic, we don't need a real LLM provider
        // In real tests, we'd use a mock LLM provider
        let llm_provider: Arc<dyn LLMProvider> = unimplemented!("Use mock in real tests");

        AgenticOrchestrator::new(llm_provider, tool_executor, ContextTier::Medium)
    }
}
