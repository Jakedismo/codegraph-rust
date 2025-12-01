// ABOUTME: Defines the AgentExecutorTrait for pluggable agent architectures
// ABOUTME: Enables runtime switching between ReAct, LATS, and future agent types

use crate::autoagents::codegraph_agent::CodeGraphAgentOutput;
use crate::autoagents::executor::ExecutorError;
use async_trait::async_trait;
use codegraph_mcp_core::agent_architecture::AgentArchitecture;
use codegraph_mcp_core::analysis::AnalysisType;
use codegraph_mcp_core::context_aware_limits::ContextTier;

/// Universal executor trait for all agent architectures
///
/// This trait enables runtime switching between different agent architectures
/// (ReAct, LATS, etc.) without changing the MCP server code.
#[async_trait]
pub trait AgentExecutorTrait: Send + Sync {
    /// Execute agentic analysis with the given query
    async fn execute(
        &self,
        query: String,
        analysis_type: AnalysisType,
    ) -> Result<CodeGraphAgentOutput, ExecutorError>;

    /// Get the architecture type this executor implements
    fn architecture(&self) -> AgentArchitecture;

    /// Get the context tier this executor is configured for
    fn tier(&self) -> ContextTier;
}
