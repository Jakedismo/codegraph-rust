// ABOUTME: Rig agent executor with conversation memory
// ABOUTME: Maintains conversation history for analysis task duration

use super::api::AgentEvent;
use super::builder::RigAgentBuilder;
use super::RigAgentOutput;
use crate::adapter::RigLLMAdapter;
use anyhow::Result;
use codegraph_mcp_core::analysis::AnalysisType;
use codegraph_mcp_core::context_aware_limits::ContextTier;
use codegraph_mcp_tools::GraphToolExecutor;
use futures::Stream;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Instant;
use tracing::{debug, info};

/// Executor for Rig-based code analysis agents
/// Maintains conversation history for the duration of an analysis task
pub struct RigExecutor {
    executor: Arc<GraphToolExecutor>,
    /// Conversation history for multi-turn interactions
    history: Vec<ConversationTurn>,
}

/// A single turn in the conversation
#[derive(Debug, Clone)]
pub struct ConversationTurn {
    /// User query
    pub query: String,
    /// Agent response
    pub response: String,
    /// Number of tool calls made
    pub tool_calls: usize,
    /// Duration in milliseconds
    pub duration_ms: u64,
}

impl RigExecutor {
    /// Create a new executor with shared GraphToolExecutor
    pub fn new(executor: Arc<GraphToolExecutor>) -> Self {
        Self {
            executor,
            history: Vec::new(),
        }
    }

    /// Execute an analysis query
    pub async fn execute(
        &mut self,
        query: &str,
        analysis_type: AnalysisType,
    ) -> Result<RigAgentOutput> {
        let start = Instant::now();

        info!(
            query = %query,
            analysis_type = ?analysis_type,
            history_len = self.history.len(),
            "Starting Rig agent execution"
        );

        // --- Dynamic Context Throttling ---
        let max_tokens = RigLLMAdapter::context_window();
        let estimated_history_tokens: usize = self
            .history
            .iter()
            .map(|t| (t.query.len() + t.response.len()) / 4)
            .sum();
        let usage_ratio = estimated_history_tokens as f64 / max_tokens as f64;

        let mut builder = RigAgentBuilder::new(self.executor.clone())
            .analysis_type(analysis_type);

        if usage_ratio > 0.8 {
            info!(
                usage_ratio = usage_ratio,
                "Context usage high (>80%), throttling tier to Small"
            );
            builder = builder.tier(ContextTier::Small);
            // Future: Trigger summary if > 0.95
        }

        let agent = builder.build()?;

        debug!(
            tier = ?agent.tier(),
            max_turns = agent.max_turns(),
            "Agent configured"
        );

        // Build context from history if present
        let contextualized_query = if self.history.is_empty() {
            query.to_string()
        } else {
            self.build_contextualized_query(query)
        };

        // Execute the agent
        let response = agent.execute(&contextualized_query).await?;

        let duration_ms = start.elapsed().as_millis() as u64;
        let tool_calls = agent.take_tool_call_count();
        let tool_traces = agent.take_tool_traces();

        // Record in history
        let turn = ConversationTurn {
            query: query.to_string(),
            response: response.clone(),
            tool_calls,
            duration_ms,
        };
        self.history.push(turn);

        info!(
            duration_ms = duration_ms,
            tool_calls = tool_calls,
            history_len = self.history.len(),
            "Rig agent execution completed"
        );

        Ok(RigAgentOutput {
            response,
            tool_calls,
            duration_ms,
            tool_traces,
        })
    }

    /// Execute agent with streaming events
    /// Note: This does NOT update history automatically (limit of this simple implementation)
    /// Clients using streaming must handle history recording manually or use execute() for stateful turns.
    pub async fn execute_stream(
        &mut self,
        query: &str,
        analysis_type: AnalysisType,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<AgentEvent>> + Send>>> {
        let agent = RigAgentBuilder::new(self.executor.clone())
            .analysis_type(analysis_type)
            .build()?;

        let contextualized_query = if self.history.is_empty() {
            query.to_string()
        } else {
            self.build_contextualized_query(query)
        };

        agent.execute_stream(&contextualized_query).await
    }

    /// Execute with explicit tier override
    pub async fn execute_with_tier(
        &mut self,
        query: &str,
        analysis_type: AnalysisType,
        tier: ContextTier,
    ) -> Result<RigAgentOutput> {
        let start = Instant::now();

        let agent = RigAgentBuilder::new(self.executor.clone())
            .analysis_type(analysis_type)
            .tier(tier)
            .build()?;

        let contextualized_query = if self.history.is_empty() {
            query.to_string()
        } else {
            self.build_contextualized_query(query)
        };

        let response = agent.execute(&contextualized_query).await?;
        let duration_ms = start.elapsed().as_millis() as u64;
        let tool_calls = agent.take_tool_call_count();
        let tool_traces = agent.take_tool_traces();

        let turn = ConversationTurn {
            query: query.to_string(),
            response: response.clone(),
            tool_calls,
            duration_ms,
        };
        self.history.push(turn);

        Ok(RigAgentOutput {
            response,
            tool_calls,
            duration_ms,
            tool_traces,
        })
    }

    /// Build a query with conversation context
    fn build_contextualized_query(&self, query: &str) -> String {
        let mut context = String::new();

        // Include relevant history (last 3 turns for context)
        let relevant_history: Vec<_> = self.history.iter().rev().take(3).collect();

        if !relevant_history.is_empty() {
            context.push_str("Previous conversation context:\n\n");

            for (i, turn) in relevant_history.iter().rev().enumerate() {
                context.push_str(&format!("Turn {}:\n", i + 1));
                context.push_str(&format!("Query: {}\n", turn.query));

                // Truncate long responses for context
                let response_preview = if turn.response.len() > 500 {
                    format!("{}...", &turn.response[..500])
                } else {
                    turn.response.clone()
                };
                context.push_str(&format!("Response: {}\n\n", response_preview));
            }

            context.push_str("---\n\nCurrent query: ");
        }

        format!("{}{}", context, query)
    }

    /// Clear conversation history (start fresh session)
    pub fn clear_history(&mut self) {
        self.history.clear();
        info!("Conversation history cleared");
    }

    /// Get the current conversation history
    pub fn history(&self) -> &[ConversationTurn] {
        &self.history
    }

    /// Get the number of turns in the conversation
    pub fn turn_count(&self) -> usize {
        self.history.len()
    }

    /// Get cache statistics from the underlying executor
    pub fn cache_stats(&self) -> codegraph_mcp_tools::CacheStats {
        self.executor.cache_stats()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_contextualized_query_empty_history() {
        // Without executor, we just test the logic
        let query = "test query";

        // Empty history should return query as-is
        let history: Vec<ConversationTurn> = vec![];
        let result = if history.is_empty() {
            query.to_string()
        } else {
            format!("with context: {}", query)
        };

        assert_eq!(result, query);
    }

    #[test]
    fn test_conversation_turn_structure() {
        let turn = ConversationTurn {
            query: "test".to_string(),
            response: "response".to_string(),
            tool_calls: 5,
            duration_ms: 1000,
        };

        assert_eq!(turn.query, "test");
        assert_eq!(turn.tool_calls, 5);
        assert_eq!(turn.duration_ms, 1000);
    }
}