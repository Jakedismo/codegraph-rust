// ABOUTME: LATS (Language Agent Tree Search) implementation using Rig

use crate::agent::api::{AgentEvent, RigAgentTrait};
use crate::tools::GraphToolFactory;
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use codegraph_mcp_core::context_aware_limits::ContextTier;
use futures::stream;
use futures::Stream;
use rig::completion::CompletionModel;
use rig::OneOrMany;
use std::pin::Pin;

/// LATS agent that explores multiple reasoning paths
pub struct LatsAgent<M: CompletionModel + Send + Sync> {
    pub(crate) model: M,
    pub(crate) factory: GraphToolFactory,
    pub(crate) max_turns: usize,
    pub(crate) tier: ContextTier,
}

#[async_trait]
impl<M: CompletionModel + Send + Sync> RigAgentTrait for LatsAgent<M> {
    async fn execute(&self, query: &str) -> Result<String> {
        // Placeholder LATS implementation
        // Real implementation would:
        // 1. Generate N initial thoughts (parallel)
        // 2. Score them
        // 3. Select best, expand
        // 4. Repeat

        // For this proof-of-concept, we'll do a simple "Best of 3" generation for the first step
        // to demonstrate the "Parallel Expansion" capability from the spec.

        let n_candidates = 3;
        let prompt = format!(
            "Query: {}\n\nGenerate a concise plan to answer this query. Do not execute tools yet, just plan.",
            query
        );

        // Parallel generation
        // Placeholder for future parallel expansion logic
        // let mut tasks = vec![];
        for _ in 0..n_candidates {
            // We need to clone the model, but CompletionModel might not be Clone?
            // Usually models are stateless clients or Arc-wrapped.
            // Assuming M is Clone or we use &M. Rig models usually take &self.
            // But to use in tokio::spawn, we need 'static. M might not be.
            // We'll run sequentially for safety if M !: Clone, or use join_all if we can.
            // Rig models are often just clients.
        }

        // Simpler: Just use the model to answer directly for now, marking it as LATS
        // to verify the architecture switch.
        // Constructing a "user" message for the chat history
        let chat_history = vec![
            rig::completion::Message::User {
                content: OneOrMany::one(rig::message::UserContent::Text(prompt.clone().into())),
            }
        ];

        let req = rig::completion::CompletionRequest {
            chat_history: OneOrMany::many(chat_history).expect("Chat history cannot be empty"),
            preamble: Some("You are a LATS-powered agent. Think step-by-step.".to_string()),
            documents: vec![],
            tools: vec![],
            temperature: Some(0.1),
            max_tokens: Some(1024),
            additional_params: None,
            tool_choice: None,
        };

        let response = self
            .model
            .completion(req)
            .await
            .map_err(|e| anyhow!("LATS model execution failed: {}", e))?;

        // response.choice is AssistantContent (not a Vec)
        let text = format!("{:?}", response.choice);

        Ok(format!(
            "[LATS ARCHITECTURE ACTIVE - Step 1/1]\nPlan generated: {}\n(Full tree search pending implementation)",
            text
        ))
    }

    async fn execute_stream(
        &self,
        query: &str,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<AgentEvent>> + Send>>> {
        let response = self.execute(query).await?;
        let events = vec![
            Ok(AgentEvent::Thinking("LATS: Generating candidate plans...".to_string())),
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
