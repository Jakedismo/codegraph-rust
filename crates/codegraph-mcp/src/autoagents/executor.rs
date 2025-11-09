// ABOUTME: High-level executor wrapper for AutoAgents workflows
// ABOUTME: Orchestrates tier detection, agent building, and execution

use crate::autoagents::agent_builder::{AgentHandle, CodeGraphAgentBuilder};
use crate::autoagents::codegraph_agent::CodeGraphAgentOutput;
use crate::context_aware_limits::ContextTier;
use crate::{AnalysisType, GraphToolExecutor};
use codegraph_ai::llm_provider::LLMProvider;
use std::sync::Arc;

/// Error type for executor operations
#[derive(Debug, thiserror::Error)]
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
/// 1. Detect ContextTier from LLM configuration
/// 2. Build tier-aware agent with CodeGraphAgentBuilder
/// 3. Execute agent with user query
/// 4. Return structured output
pub struct CodeGraphExecutor {
    llm_provider: Arc<dyn LLMProvider>,
    tool_executor: Arc<GraphToolExecutor>,
}

impl CodeGraphExecutor {
    pub fn new(
        llm_provider: Arc<dyn LLMProvider>,
        tool_executor: Arc<GraphToolExecutor>,
    ) -> Self {
        Self {
            llm_provider,
            tool_executor,
        }
    }

    /// Execute agentic analysis with automatic tier detection
    pub async fn execute(
        &self,
        query: String,
        analysis_type: AnalysisType,
    ) -> Result<CodeGraphAgentOutput, ExecutorError> {
        // Step 1: Detect context tier from LLM
        let tier = self.detect_tier().await?;

        // Step 2: Build tier-aware agent
        let agent_handle = self.build_agent(tier, analysis_type).await?;

        // Step 3: Execute agent with query
        let output = self.run_agent(agent_handle, query).await?;

        Ok(output)
    }

    /// Detect ContextTier from LLM provider configuration
    async fn detect_tier(&self) -> Result<ContextTier, ExecutorError> {
        // Use the LLM provider's context window size to determine tier
        // This matches the existing tier detection logic in codegraph-ai

        // Get model info from provider (implementation depends on LLMProvider interface)
        // For now, default to Medium tier - this will be refined based on actual LLM config

        // TODO: Implement actual tier detection by:
        // 1. Getting model name from LLMProvider
        // 2. Looking up context window size in model registry
        // 3. Mapping context window to ContextTier

        Ok(ContextTier::Medium)
    }

    /// Build CodeGraph agent with specified tier and analysis type
    async fn build_agent(
        &self,
        tier: ContextTier,
        analysis_type: AnalysisType,
    ) -> Result<AgentHandle, ExecutorError> {
        let builder = CodeGraphAgentBuilder::new(
            self.llm_provider.clone(),
            self.tool_executor.clone(),
            tier,
            analysis_type,
        );

        builder
            .build()
            .await
            .map_err(|e| ExecutorError::BuildFailed(e.to_string()))
    }

    /// Execute agent and convert output
    async fn run_agent(
        &self,
        mut agent_handle: AgentHandle,
        query: String,
    ) -> Result<CodeGraphAgentOutput, ExecutorError> {
        use autoagents::core::agent::Agent;

        // Execute the agent with the query
        let react_output = agent_handle
            .agent
            .run(&query)
            .await
            .map_err(|e| ExecutorError::ExecutionFailed(e.to_string()))?;

        // Convert ReActAgentOutput to CodeGraphAgentOutput
        // The From impl handles this conversion
        Ok(react_output.into())
    }
}

/// Builder for CodeGraphExecutor with fluent API
pub struct CodeGraphExecutorBuilder {
    llm_provider: Option<Arc<dyn LLMProvider>>,
    tool_executor: Option<Arc<GraphToolExecutor>>,
}

impl CodeGraphExecutorBuilder {
    pub fn new() -> Self {
        Self {
            llm_provider: None,
            tool_executor: None,
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

    pub fn build(self) -> Result<CodeGraphExecutor, ExecutorError> {
        let llm_provider = self
            .llm_provider
            .ok_or_else(|| ExecutorError::BuildFailed("LLM provider required".to_string()))?;

        let tool_executor = self
            .tool_executor
            .ok_or_else(|| ExecutorError::BuildFailed("Tool executor required".to_string()))?;

        Ok(CodeGraphExecutor::new(llm_provider, tool_executor))
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
