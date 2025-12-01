// ABOUTME: ReAct executor implementing AgentExecutorTrait
// ABOUTME: Self-contained ReAct implementation using CodeGraphAgentBuilder

use async_trait::async_trait;
use codegraph_mcp_core::agent_architecture::AgentArchitecture;
use codegraph_mcp_core::context_aware_limits::ContextTier;
use codegraph_mcp_core::analysis::AnalysisType;
use codegraph_mcp_tools::GraphToolExecutor;
use codegraph_ai::llm_provider::LLMProvider;
use crate::autoagents::codegraph_agent::CodeGraphAgentOutput;
use crate::autoagents::executor_trait::AgentExecutorTrait;
use crate::autoagents::executor::ExecutorError;
use crate::autoagents::agent_builder::{AgentHandle, CodeGraphAgentBuilder};
use std::sync::Arc;

/// ReAct executor implementing AgentExecutorTrait
///
/// This is a self-contained ReAct implementation that builds and executes
/// ReAct agents using CodeGraphAgentBuilder. It can be used by the
/// AgentExecutorFactory to create ReAct-based executors.
pub struct ReActExecutor {
    llm_provider: Arc<dyn LLMProvider>,
    tool_executor: Arc<GraphToolExecutor>,
    tier: ContextTier,
}

impl ReActExecutor {
    pub fn new(
        llm_provider: Arc<dyn LLMProvider>,
        tool_executor: Arc<GraphToolExecutor>,
        tier: ContextTier,
    ) -> Self {
        Self {
            llm_provider,
            tool_executor,
            tier,
        }
    }

    /// Build CodeGraph agent with specified tier and analysis type
    async fn build_agent(
        &self,
        analysis_type: AnalysisType,
    ) -> Result<AgentHandle, ExecutorError> {
        let builder = CodeGraphAgentBuilder::new(
            self.llm_provider.clone(),
            self.tool_executor.clone(),
            self.tier,
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
        agent_handle: AgentHandle,
        query: String,
    ) -> Result<CodeGraphAgentOutput, ExecutorError> {
        use autoagents::core::agent::task::Task;

        // Execute the agent with the query wrapped in a Task
        let react_output = agent_handle
            .agent
            .agent
            .run(Task::new(&query))
            .await
            .map_err(|e| ExecutorError::ExecutionFailed(e.to_string()))?;

        // Convert ReActAgentOutput to CodeGraphAgentOutput
        Ok(react_output.into())
    }
}

#[async_trait]
impl AgentExecutorTrait for ReActExecutor {
    async fn execute(
        &self,
        query: String,
        analysis_type: AnalysisType,
    ) -> Result<CodeGraphAgentOutput, ExecutorError> {
        // Step 1: Build tier-aware agent
        let agent_handle = self.build_agent(analysis_type).await?;

        // Step 2: Execute agent with query
        let output = self.run_agent(agent_handle, query).await?;

        Ok(output)
    }

    fn architecture(&self) -> AgentArchitecture {
        AgentArchitecture::ReAct
    }

    fn tier(&self) -> ContextTier {
        self.tier
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_react_executor_architecture() {
        // We can't easily test the full executor without mocks,
        // but we can verify the architecture type
        let arch = AgentArchitecture::ReAct;
        assert_eq!(format!("{}", arch), "react");
    }
}
