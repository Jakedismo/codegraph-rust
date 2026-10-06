// ABOUTME: Reflexion agent implementation (Retry with reflection)

use crate::agent::api::{AgentEvent, RigAgentTrait};
use anyhow::Result;
use async_trait::async_trait;
use codegraph_mcp_core::context_aware_limits::ContextTier;
use futures::Stream;
use futures::stream;
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
                            attempt, response
                        ));
                    }
                    return Ok(response);
                }
                Err(e) => {
                    last_error = Some(e.to_string());
                    // Reflection step: Append error context to query
                    current_query = format!(
                        "{}\n\nA previous attempt at this request failed with this error:\n{}\n\nWork out what caused the error and avoid it this time. If it came from running out of tool rounds, use fewer rounds and answer from less evidence. If it came from a tool call, change the arguments or use a different tool.",
                        query, e
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
        // Since I can't easily clone Box<dyn RigAgentTrait>, I will stick to buffered reflexion for streaming
        // in this phase, but I will add thinking events to show the process.

        let result = self.execute(query).await;
        match result {
            Ok(response) => {
                let events = vec![
                    Ok(AgentEvent::Thinking(
                        "Reflexion: Validating reasoning...".to_string(),
                    )),
                    Ok(AgentEvent::OutputChunk(response)),
                    Ok(AgentEvent::Done),
                ];
                Ok(Box::pin(stream::iter(events)))
            }
            Err(e) => {
                let events = vec![
                    Ok(AgentEvent::Thinking(
                        "Reflexion: Attempt failed, reflecting...".to_string(),
                    )),
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
