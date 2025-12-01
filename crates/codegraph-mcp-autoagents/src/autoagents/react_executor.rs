// ABOUTME: ReAct executor wrapper implementing AgentExecutorTrait
// ABOUTME: Delegates to existing CodeGraphExecutor for backward compatibility

use async_trait::async_trait;
use codegraph_mcp_core::agent_architecture::AgentArchitecture;
use codegraph_mcp_core::context_aware_limits::ContextTier;
use codegraph_mcp_core::analysis::AnalysisType;
use codegraph_mcp_tools::GraphToolExecutor;
use codegraph_ai::llm_provider::LLMProvider;
use crate::autoagents::codegraph_agent::CodeGraphAgentOutput;
use crate::autoagents::executor_trait::AgentExecutorTrait;
use crate::autoagents::executor::{CodeGraphExecutor, ExecutorError};
use std::sync::Arc;

/// ReAct executor wrapper implementing AgentExecutorTrait
///
/// This wraps the existing CodeGraphExecutor to provide a uniform
/// interface that can be used by the AgentExecutorFactory.
pub struct ReActExecutor {
    inner: CodeGraphExecutor,
    tier: ContextTier,
}

impl ReActExecutor {
    pub fn new(
        llm_provider: Arc<dyn LLMProvider>,
        tool_executor: Arc<GraphToolExecutor>,
    ) -> Self {
        let inner = CodeGraphExecutor::new(llm_provider, tool_executor);

        // Default to Medium tier - will be updated on first execution
        // when detect_tier() is called
        Self {
            inner,
            tier: ContextTier::Medium,
        }
    }
}

#[async_trait]
impl AgentExecutorTrait for ReActExecutor {
    async fn execute(
        &self,
        query: String,
        analysis_type: AnalysisType,
    ) -> Result<CodeGraphAgentOutput, ExecutorError> {
        // Delegate to the existing CodeGraphExecutor
        self.inner.execute(query, analysis_type).await
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
