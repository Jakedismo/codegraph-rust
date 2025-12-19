// ABOUTME: ReAct agent implementations for different providers

use crate::agent::api::{AgentEvent, RigAgentTrait};
use crate::tools::GraphToolFactory;
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use codegraph_mcp_core::context_aware_limits::ContextTier;
use futures::stream;
use futures::Stream;
use std::pin::Pin;

/// OpenAI-based Rig agent
#[cfg(feature = "openai")]
pub struct OpenAIAgent {
    pub(crate) agent:
        rig::agent::Agent<rig::providers::openai::responses_api::ResponsesCompletionModel>,
    pub(crate) factory: GraphToolFactory,
    pub(crate) max_turns: usize,
    pub(crate) tier: ContextTier,
}

#[cfg(feature = "openai")]
#[async_trait]
impl RigAgentTrait for OpenAIAgent {
    async fn execute(&self, query: &str) -> Result<String> {
        use rig::agent::PromptRequest;

        let mut chat_history = vec![];
        let response = PromptRequest::new(&self.agent, query)
            .multi_turn(self.max_turns)
            .with_history(&mut chat_history)
            .await
            .map_err(|e| anyhow!("Agent execution failed: {}", e))?;

        Ok(response)
    }

    async fn execute_stream(
        &self,
        query: &str,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<AgentEvent>> + Send>>> {
        // Buffered implementation: wait for full response, then yield chunks
        let response = self.execute(query).await?;
        let events = vec![
            Ok(AgentEvent::OutputChunk(response)),
            Ok(AgentEvent::Done),
        ];
        Ok(Box::pin(stream::iter(events)))
    }

    fn tier(&self) -> ContextTier {
        self.tier
    }

    fn max_turns(&self) -> usize {
        self.max_turns
    }

    fn take_tool_call_count(&self) -> usize {
        self.factory.take_call_count()
    }

    fn take_tool_traces(&self) -> Vec<crate::tools::ToolTrace> {
        self.factory.take_traces()
    }
}

/// Anthropic-based Rig agent
#[cfg(feature = "anthropic")]
pub struct AnthropicAgent {
    pub(crate) agent: rig::agent::Agent<rig::providers::anthropic::completion::CompletionModel>,
    pub(crate) factory: GraphToolFactory,
    pub(crate) max_turns: usize,
    pub(crate) tier: ContextTier,
}

#[cfg(feature = "anthropic")]
#[async_trait]
impl RigAgentTrait for AnthropicAgent {
    async fn execute(&self, query: &str) -> Result<String> {
        use rig::agent::PromptRequest;

        let mut chat_history = vec![];
        let response = PromptRequest::new(&self.agent, query)
            .multi_turn(self.max_turns)
            .with_history(&mut chat_history)
            .await
            .map_err(|e| anyhow!("Agent execution failed: {}", e))?;

        Ok(response)
    }

    async fn execute_stream(
        &self,
        query: &str,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<AgentEvent>> + Send>>> {
        let response = self.execute(query).await?;
        let events = vec![
            Ok(AgentEvent::OutputChunk(response)),
            Ok(AgentEvent::Done),
        ];
        Ok(Box::pin(stream::iter(events)))
    }

    fn tier(&self) -> ContextTier {
        self.tier
    }

    fn max_turns(&self) -> usize {
        self.max_turns
    }

    fn take_tool_call_count(&self) -> usize {
        self.factory.take_call_count()
    }

    fn take_tool_traces(&self) -> Vec<crate::tools::ToolTrace> {
        self.factory.take_traces()
    }
}

/// Ollama-based Rig agent
#[cfg(feature = "ollama")]
pub struct OllamaAgent {
    pub(crate) agent: rig::agent::Agent<rig::providers::ollama::CompletionModel>,
    pub(crate) factory: GraphToolFactory,
    pub(crate) max_turns: usize,
    pub(crate) tier: ContextTier,
}

#[cfg(feature = "ollama")]
#[async_trait]
impl RigAgentTrait for OllamaAgent {
    async fn execute(&self, query: &str) -> Result<String> {
        use rig::agent::PromptRequest;

        let mut chat_history = vec![];
        let response = PromptRequest::new(&self.agent, query)
            .multi_turn(self.max_turns)
            .with_history(&mut chat_history)
            .await
            .map_err(|e| anyhow!("Agent execution failed: {}", e))?;

        Ok(response)
    }

    async fn execute_stream(
        &self,
        query: &str,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<AgentEvent>> + Send>>> {
        let response = self.execute(query).await?;
        let events = vec![
            Ok(AgentEvent::OutputChunk(response)),
            Ok(AgentEvent::Done),
        ];
        Ok(Box::pin(stream::iter(events)))
    }

    fn tier(&self) -> ContextTier {
        self.tier
    }

    fn max_turns(&self) -> usize {
        self.max_turns
    }

    fn take_tool_call_count(&self) -> usize {
        self.factory.take_call_count()
    }

    fn take_tool_traces(&self) -> Vec<crate::tools::ToolTrace> {
        self.factory.take_traces()
    }
}

/// xAI-based Rig agent (native rig provider)
#[cfg(feature = "xai")]
pub struct XAIAgent {
    pub(crate) agent: rig::agent::Agent<rig::providers::xai::completion::CompletionModel>,
    pub(crate) factory: GraphToolFactory,
    pub(crate) max_turns: usize,
    pub(crate) tier: ContextTier,
}

#[cfg(feature = "xai")]
#[async_trait]
impl RigAgentTrait for XAIAgent {
    async fn execute(&self, query: &str) -> Result<String> {
        use rig::agent::PromptRequest;

        let mut chat_history = vec![];
        let response = PromptRequest::new(&self.agent, query)
            .multi_turn(self.max_turns)
            .with_history(&mut chat_history)
            .await
            .map_err(|e| anyhow!("Agent execution failed: {}", e))?;

        Ok(response)
    }

    async fn execute_stream(
        &self,
        query: &str,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<AgentEvent>> + Send>>> {
        let response = self.execute(query).await?;
        let events = vec![
            Ok(AgentEvent::OutputChunk(response)),
            Ok(AgentEvent::Done),
        ];
        Ok(Box::pin(stream::iter(events)))
    }

    fn tier(&self) -> ContextTier {
        self.tier
    }

    fn max_turns(&self) -> usize {
        self.max_turns
    }

    fn take_tool_call_count(&self) -> usize {
        self.factory.take_call_count()
    }

    fn take_tool_traces(&self) -> Vec<crate::tools::ToolTrace> {
        self.factory.take_traces()
    }
}
