// ABOUTME: High-level executor wrapper for AutoAgents workflows
// ABOUTME: Orchestrates architecture detection, factory-based executor creation, and delegation

use crate::autoagents::codegraph_agent::CodeGraphAgentOutput;
use crate::autoagents::startup_context::{build_startup_context, StartupContextRender};
use codegraph_ai::llm_provider::LLMProvider;
use codegraph_graph::{GraphFunctions, HubNode};
use codegraph_mcp_core::analysis::AnalysisType;
use codegraph_mcp_tools::GraphToolExecutor;
use std::cmp::Reverse;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;

/// Default execution timeout in seconds (targeting long-running analyses)
const DEFAULT_TIMEOUT_SECS: u64 = 9000;

/// Patterns that indicate context window overflow from various LLM providers
const CONTEXT_OVERFLOW_PATTERNS: &[&str] = &[
    "context_length_exceeded",
    "maximum context length",
    "too many tokens",
    "context window",
    "token limit",
    "max_tokens",
    "context length",
    "exceeds the maximum",
];

/// Error type for executor operations
#[derive(Debug, Error)]
pub enum ExecutorError {
    #[error("Agent build failed: {0}")]
    BuildFailed(String),

    #[error("Agent execution failed: {0}")]
    ExecutionFailed(String),

    #[error("Tier detection failed: {0}")]
    TierDetectionFailed(String),

    #[error("Output conversion failed: {0}")]
    OutputConversionFailed(String),

    #[error("Agent execution timed out after {elapsed_secs} seconds")]
    Timeout {
        elapsed_secs: u64,
        partial_result: Option<String>,
        steps_completed: usize,
    },

    #[error("Context window limit reached: {0}")]
    ContextOverflow(String),
}

/// Check if an error message indicates context window overflow
pub fn is_context_overflow_error(error_message: &str) -> bool {
    let lower = error_message.to_lowercase();
    CONTEXT_OVERFLOW_PATTERNS
        .iter()
        .any(|pattern| lower.contains(pattern))
}

/// Transform an ExecutorError to ContextOverflow if it matches overflow patterns
pub fn transform_context_overflow(err: ExecutorError) -> ExecutorError {
    match &err {
        ExecutorError::ExecutionFailed(msg) if is_context_overflow_error(msg) => {
            // Log the overflow event for debugging
            if std::env::var("CODEGRAPH_DEBUG").is_ok() {
                tracing::warn!(
                    target: "context_overflow",
                    original_error = %msg,
                    "Context window overflow detected"
                );
            }
            ExecutorError::ContextOverflow(
                "Context window limit reached. Query too complex for model's capacity.".to_string(),
            )
        }
        _ => err,
    }
}

/// Read timeout configuration from environment variable
fn read_timeout_config() -> Duration {
    std::env::var("CODEGRAPH_AGENT_TIMEOUT_SECS")
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
        .map(|secs| {
            if secs == 0 {
                // Zero means unlimited - use a very large duration
                Duration::from_secs(u64::MAX / 2)
            } else {
                Duration::from_secs(secs)
            }
        })
        .unwrap_or(Duration::from_secs(DEFAULT_TIMEOUT_SECS))
}

/// High-level executor for AutoAgents-based code analysis
///
/// This orchestrates the complete workflow:
/// 1. Detect agent architecture from configuration
/// 2. Use factory to create architecture-specific executor
/// 3. Delegate execution to the executor with timeout enforcement
/// 4. Return structured output
pub struct CodeGraphExecutor {
    factory: crate::autoagents::executor_factory::AgentExecutorFactory,
    architecture: codegraph_mcp_core::agent_architecture::AgentArchitecture,
    timeout: Duration,
}

impl CodeGraphExecutor {
    pub fn new(
        llm_provider: Arc<dyn LLMProvider>,
        tool_executor: Arc<GraphToolExecutor>,
        config: Arc<codegraph_mcp_core::config_manager::CodeGraphConfig>,
    ) -> Self {
        use crate::autoagents::executor_factory::AgentExecutorFactory;

        // Create factory for architecture-specific executors
        let factory = AgentExecutorFactory::new(llm_provider, tool_executor, config.clone());

        // Detect architecture from environment or config
        let architecture = AgentExecutorFactory::detect_architecture();

        // Read timeout configuration
        let timeout = read_timeout_config();

        tracing::debug!(
            timeout_secs = timeout.as_secs(),
            architecture = %architecture,
            "CodeGraphExecutor initialized"
        );

        Self {
            factory,
            architecture,
            timeout,
        }
    }

    /// Execute agentic analysis with automatic architecture selection and timeout enforcement
    pub async fn execute(
        &self,
        query: String,
        analysis_type: AnalysisType,
    ) -> Result<CodeGraphAgentOutput, ExecutorError> {
        use std::time::Instant;

        // Create architecture-specific executor via factory
        let executor = self.factory.create(self.architecture)?;

        // Optional graph bootstrap for architecture/context-heavy analyses
        let graph_bootstrap = if std::env::var("CODEGRAPH_ARCH_BOOTSTRAP")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false)
        {
            let gf = self.factory.tool_executor().graph_functions();
            Some(Self::build_graph_bootstrap(gf).await)
        } else {
            None
        };

        let project_root = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let enriched_query = match build_startup_context(&project_root) {
            Ok(ctx) => ctx.render_with_query_and_bootstrap(&query, graph_bootstrap.as_deref()),
            Err(e) => {
                tracing::warn!(project = %project_root.display(), error = %e, "startup context unavailable");
                query.clone()
            }
        };

        let start_time = Instant::now();

        // Wrap execution in timeout
        let result = tokio::time::timeout(
            self.timeout,
            executor.execute(enriched_query, analysis_type),
        )
        .await;

        match result {
            Ok(inner_result) => {
                // Transform context overflow errors to user-friendly message
                inner_result.map_err(transform_context_overflow)
            }
            Err(_elapsed) => {
                let elapsed_secs = start_time.elapsed().as_secs();

                // Log timeout event for debugging
                if std::env::var("CODEGRAPH_DEBUG").is_ok() {
                    tracing::error!(
                        target: "agent_execution_timeout",
                        elapsed_secs = elapsed_secs,
                        query_preview = %query.chars().take(100).collect::<String>(),
                        analysis_type = ?analysis_type,
                        architecture = %self.architecture,
                        "Agent execution timed out"
                    );
                }

                // Still emit finish event even on timeout
                tracing::info!(
                    target: "agent_execution_finish",
                    status = "timeout",
                    elapsed_secs = elapsed_secs,
                    "Agent execution terminated due to timeout"
                );

                let partial_result = Some(format!(
                    "WARNING: Agent timed out after {} seconds. Output may be incomplete.",
                    elapsed_secs
                ));

                Err(ExecutorError::Timeout {
                    elapsed_secs,
                    partial_result,
                    steps_completed: 0, // Step tracking not yet available from executors
                })
            }
        }
    }

    async fn build_graph_bootstrap(graph_functions: Arc<GraphFunctions>) -> String {
        const HUB_LIMIT: usize = 5;
        const HOTSPOT_LIMIT: usize = 5;

        let mut parts: Vec<String> = Vec::new();

        if let Ok(count) = graph_functions.count_nodes_for_project().await {
            parts.push(format!("nodes indexed: {}", count));
        }

        if let Ok(dirs) = graph_functions.get_top_directories(5).await {
            if !dirs.is_empty() {
                let list: Vec<String> = dirs
                    .into_iter()
                    .map(|d| format!("dir: {} (files={})", d.full_path, d.file_count))
                    .collect();
                parts.push(format!("top directories: {}", list.join("; ")));
            }
        }

        if let Ok(mut hubs) = graph_functions.get_hub_nodes(5).await {
            hubs.sort_by_key(|h: &HubNode| Reverse(h.total_degree));
            let list: Vec<String> = hubs
                .into_iter()
                .take(HUB_LIMIT)
                .map(|h| {
                    let name = h.node.name;
                    let kind = h.node.kind.unwrap_or_else(|| "unknown".into());
                    format!(
                        "hub: {} ({} deg={} in/out={}/{})",
                        name, kind, h.total_degree, h.afferent_degree, h.efferent_degree
                    )
                })
                .collect();
            if !list.is_empty() {
                parts.push(format!("top hubs: {}", list.join("; ")));
            }
        }

        if let Ok(hotspots) = graph_functions
            .get_complexity_hotspots(10.0, HOTSPOT_LIMIT as i32)
            .await
        {
            let list: Vec<String> = hotspots
                .into_iter()
                .map(|c| {
                    let path = c.file_path.unwrap_or_else(|| "(unknown)".into());
                    format!(
                        "hotspot: {} [{}] cc={:.1} risk={:.1}",
                        c.name, path, c.complexity, c.risk_score
                    )
                })
                .collect();
            if !list.is_empty() {
                parts.push(format!("complexity hotspots: {}", list.join("; ")));
            }
        }

        parts.join("\n")
    }
}

/// Builder for CodeGraphExecutor with fluent API
pub struct CodeGraphExecutorBuilder {
    llm_provider: Option<Arc<dyn LLMProvider>>,
    tool_executor: Option<Arc<GraphToolExecutor>>,
    config: Option<Arc<codegraph_mcp_core::config_manager::CodeGraphConfig>>,
}

impl CodeGraphExecutorBuilder {
    pub fn new() -> Self {
        Self {
            llm_provider: None,
            tool_executor: None,
            config: None,
        }
    }

    pub fn llm_provider(mut self, provider: Arc<dyn LLMProvider>) -> Self {
        self.llm_provider = Some(provider);
        self
    }

    pub fn tool_executor(mut self, executor: Arc<GraphToolExecutor>) -> Self {
        self.tool_executor = Some(executor);
        self
    }

    pub fn config(
        mut self,
        config: Arc<codegraph_mcp_core::config_manager::CodeGraphConfig>,
    ) -> Self {
        self.config = Some(config);
        self
    }

    pub fn build(self) -> Result<CodeGraphExecutor, ExecutorError> {
        let llm_provider = self
            .llm_provider
            .ok_or_else(|| ExecutorError::BuildFailed("LLM provider required".to_string()))?;

        let tool_executor = self
            .tool_executor
            .ok_or_else(|| ExecutorError::BuildFailed("Tool executor required".to_string()))?;

        // Config is optional for backward compatibility
        // If not provided, create default config
        let config = self.config.unwrap_or_else(|| {
            Arc::new(codegraph_mcp_core::config_manager::CodeGraphConfig::default())
        });

        Ok(CodeGraphExecutor::new(llm_provider, tool_executor, config))
    }
}

impl Default for CodeGraphExecutorBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_executor_builder_pattern() {
        let builder = CodeGraphExecutorBuilder::new();

        // Builder should require both LLM provider and tool executor
        let result = builder.build();
        assert!(result.is_err());
    }

    #[test]
    fn test_executor_error_display() {
        let err = ExecutorError::BuildFailed("test".to_string());
        assert_eq!(err.to_string(), "Agent build failed: test");
    }

    #[test]
    fn test_timeout_error_display() {
        let err = ExecutorError::Timeout {
            elapsed_secs: 300,
            partial_result: Some("partial".to_string()),
            steps_completed: 5,
        };
        assert_eq!(
            err.to_string(),
            "Agent execution timed out after 300 seconds"
        );
    }

    #[test]
    fn test_context_overflow_error_display() {
        let err = ExecutorError::ContextOverflow("context_length_exceeded".to_string());
        assert_eq!(
            err.to_string(),
            "Context window limit reached: context_length_exceeded"
        );
    }

    #[test]
    fn test_read_timeout_config_default() {
        // Clear env var to test default
        std::env::remove_var("CODEGRAPH_AGENT_TIMEOUT_SECS");
        let timeout = read_timeout_config();
        assert_eq!(timeout.as_secs(), DEFAULT_TIMEOUT_SECS);
    }

    #[test]
    fn test_read_timeout_config_from_env() {
        std::env::set_var("CODEGRAPH_AGENT_TIMEOUT_SECS", "60");
        let timeout = read_timeout_config();
        assert_eq!(timeout.as_secs(), 60);
        std::env::remove_var("CODEGRAPH_AGENT_TIMEOUT_SECS");
    }

    #[test]
    fn test_read_timeout_config_zero_means_unlimited() {
        std::env::set_var("CODEGRAPH_AGENT_TIMEOUT_SECS", "0");
        let timeout = read_timeout_config();
        // Zero means unlimited - should be very large
        assert!(timeout.as_secs() > 1_000_000);
        std::env::remove_var("CODEGRAPH_AGENT_TIMEOUT_SECS");
    }

    #[test]
    fn test_read_timeout_config_invalid_fallback() {
        std::env::set_var("CODEGRAPH_AGENT_TIMEOUT_SECS", "not_a_number");
        let timeout = read_timeout_config();
        assert_eq!(timeout.as_secs(), DEFAULT_TIMEOUT_SECS);
        std::env::remove_var("CODEGRAPH_AGENT_TIMEOUT_SECS");
    }

    #[test]
    fn test_is_context_overflow_error_positive() {
        assert!(is_context_overflow_error("context_length_exceeded"));
        assert!(is_context_overflow_error("maximum context length exceeded"));
        assert!(is_context_overflow_error("too many tokens in request"));
        assert!(is_context_overflow_error("token limit reached"));
        assert!(is_context_overflow_error("exceeds the maximum allowed"));
    }

    #[test]
    fn test_is_context_overflow_error_case_insensitive() {
        assert!(is_context_overflow_error("CONTEXT_LENGTH_EXCEEDED"));
        assert!(is_context_overflow_error("Maximum Context Length"));
        assert!(is_context_overflow_error("TOO MANY TOKENS"));
    }

    #[test]
    fn test_is_context_overflow_error_negative() {
        assert!(!is_context_overflow_error("connection failed"));
        assert!(!is_context_overflow_error("rate limit exceeded"));
        assert!(!is_context_overflow_error("authentication error"));
        assert!(!is_context_overflow_error("server error 500"));
    }

    #[test]
    fn test_transform_context_overflow_matches() {
        let err =
            ExecutorError::ExecutionFailed("Request failed: context_length_exceeded".to_string());
        let transformed = transform_context_overflow(err);
        match transformed {
            ExecutorError::ContextOverflow(msg) => {
                assert!(msg.contains("Context window limit reached"));
            }
            _ => panic!("Expected ContextOverflow error"),
        }
    }

    #[test]
    fn test_transform_context_overflow_passthrough() {
        let err = ExecutorError::ExecutionFailed("connection timeout".to_string());
        let transformed = transform_context_overflow(err);
        match transformed {
            ExecutorError::ExecutionFailed(msg) => {
                assert_eq!(msg, "connection timeout");
            }
            _ => panic!("Expected ExecutionFailed error to pass through"),
        }
    }

    #[test]
    fn test_transform_other_error_types() {
        let err = ExecutorError::BuildFailed("test".to_string());
        let transformed = transform_context_overflow(err);
        match transformed {
            ExecutorError::BuildFailed(msg) => {
                assert_eq!(msg, "test");
            }
            _ => panic!("Expected BuildFailed error to pass through"),
        }
    }
}
