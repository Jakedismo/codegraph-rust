// ABOUTME: Reflexion agent implementation (Retry with reflection)

use crate::agent::api::{AgentEvent, RigAgentTrait};
use anyhow::Result;
use async_trait::async_trait;
use codegraph_mcp_core::context_aware_limits::ContextTier;
use futures::stream::{self, StreamExt};
use futures::Stream;
use std::pin::Pin;

/// Reflexion agent that wraps an inner agent and retries on failure
pub struct ReflexionAgent {
    pub(crate) inner: Box<dyn RigAgentTrait>,
    pub(crate) max_retries: usize,
}

#[async_trait]
impl RigAgentTrait for ReflexionAgent {
    async fn execute(&self, query: &str) -> Result<String> {
        // Simple retry loop with "reflection" appended to query
        let mut current_query = query.to_string();
        let mut last_error = None;

        for attempt in 0..=self.max_retries {
            match self.inner.execute(&current_query).await {
                Ok(response) => {
                    if attempt > 0 {
                        return Ok(format!(
                            "[REFLEXION SUCCESS after {} retries]\n{}",
                            attempt,
                            response
                        ));
                    }
                    return Ok(response);
                }
                Err(e) => {
                    last_error = Some(e.to_string());
                    // Reflection step: Append error context to query
                    current_query = format!(
                        "{}\n\n[Previous Attempt Failed]: {}\nPlease reflect on this error and try a different approach.",
                        query,
                        e
                    );
                }
            }
        }

        Err(anyhow::anyhow!(
            "Reflexion failed after {} retries. Last error: {:?}",
            self.max_retries,
            last_error
        ))
    }

    async fn execute_stream(
        &self,
        query: &str,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<AgentEvent>> + Send>>> {
        // For streaming, we need to stream the attempts.
        // This is complex because we might fail and retry.
        // Simplified: Buffer the inner execution, check result, if fail, emit "Reflecting..." event and retry.
        // If success, emit the inner events (replayed) or just the result.
        
        // Proper implementation would require RigAgentTrait::execute_stream to return an error we can catch.
        // But here we implement "buffered reflexion" for simplicity in Phase 1.
        let result = self.execute(query).await;
        match result {
             Ok(response) => {
                 let events = vec![
                     Ok(AgentEvent::OutputChunk(response)),
                     Ok(AgentEvent::Done),
                 ];
                 Ok(Box::pin(stream::iter(events)))
             }
             Err(e) => {
                 let events = vec![
                     Ok(AgentEvent::Error(e.to_string())),
                 ];
                 Ok(Box::pin(stream::iter(events)))
             }
        }
    }

    fn tier(&self) -> ContextTier {
        self.inner.tier()
    }

    fn max_turns(&self) -> usize {
        self.inner.max_turns()
    }

    fn take_tool_call_count(&self) -> usize {
        self.inner.take_tool_call_count()
    }

    fn take_tool_traces(&self) -> Vec<crate::tools::ToolTrace> {
        self.inner.take_tool_traces()
    }
}
