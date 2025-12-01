// ABOUTME: High-level executor wrapper for AutoAgents workflows
// ABOUTME: Orchestrates architecture detection, factory-based executor creation, and delegation

use crate::autoagents::codegraph_agent::CodeGraphAgentOutput;
use codegraph_mcp_core::analysis::AnalysisType;
use codegraph_mcp_tools::GraphToolExecutor;
use codegraph_ai::llm_provider::LLMProvider;
use std::sync::Arc;
use thiserror::Error;

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
}

/// High-level executor for AutoAgents-based code analysis
///
/// This orchestrates the complete workflow:
/// 1. Detect agent architecture from configuration
/// 2. Use factory to create architecture-specific executor
/// 3. Delegate execution to the executor
/// 4. Return structured output
pub struct CodeGraphExecutor {
    factory: crate::autoagents::executor_factory::AgentExecutorFactory,
    architecture: codegraph_mcp_core::agent_architecture::AgentArchitecture,
}

impl CodeGraphExecutor {
    pub fn new(
        llm_provider: Arc<dyn LLMProvider>,
        tool_executor: Arc<GraphToolExecutor>,
        config: Arc<codegraph_mcp_core::config_manager::CodeGraphConfig>,
    ) -> Self {
        use crate::autoagents::executor_factory::AgentExecutorFactory;

        // Create factory for architecture-specific executors
        let factory = AgentExecutorFactory::new(
            llm_provider,
            tool_executor,
            config.clone(),
        );

        // Detect architecture from environment or config
        let architecture = AgentExecutorFactory::detect_architecture();

        Self { factory, architecture }
    }

    /// Execute agentic analysis with automatic architecture selection
    pub async fn execute(
        &self,
        query: String,
        analysis_type: AnalysisType,
    ) -> Result<CodeGraphAgentOutput, ExecutorError> {
        // Create architecture-specific executor via factory
        let executor = self.factory.create(self.architecture)?;

        // Delegate execution to the architecture-specific executor
        executor.execute(query, analysis_type).await
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

    pub fn config(mut self, config: Arc<codegraph_mcp_core::config_manager::CodeGraphConfig>) -> Self {
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
}
