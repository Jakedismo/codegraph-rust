// ABOUTME: Core traits and types for Rig-based agents

use anyhow::Result;
use async_trait::async_trait;
use codegraph_mcp_core::context_aware_limits::ContextTier;
use futures::Stream;
use std::pin::Pin;

/// Event emitted during agent streaming execution
#[derive(Debug, Clone)]
pub enum AgentEvent {
    /// Agent is thinking / generating reasoning
    Thinking(String),
    /// Agent is calling a tool
    ToolCall {
        tool_name: String,
        arguments: String,
    },
    /// Tool execution completed
    ToolResult {
        tool_name: String,
        result: String,
    },
    /// Chunk of the final answer
    OutputChunk(String),
    /// Execution error
    Error(String),
    /// Execution finished
    Done,
}

/// Trait for unified agent interface
#[async_trait]
pub trait RigAgentTrait: Send + Sync {
    /// Execute the agent with the given query (blocking/buffered)
    async fn execute(&self, query: &str) -> Result<String>;

    /// Execute the agent with streaming events
    async fn execute_stream(
        &self,
        query: &str,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<AgentEvent>> + Send>>>;

    /// Get the configured tier
    fn tier(&self) -> ContextTier;

    /// Get max turns
    fn max_turns(&self) -> usize;

    /// Get and reset the tool call count since last query
    fn take_tool_call_count(&self) -> usize;

    /// Get and reset tool traces since last query
    fn take_tool_traces(&self) -> Vec<crate::tools::ToolTrace>;
}
