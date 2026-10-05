// ABOUTME: ReAct agent implementations for different providers

#[cfg(any(
    feature = "openai",
    feature = "anthropic",
    feature = "ollama",
    feature = "xai"
))]
use crate::agent::api::{AgentEvent, RigAgentTrait};
#[cfg(any(
    feature = "openai",
    feature = "anthropic",
    feature = "ollama",
    feature = "xai"
))]
use crate::tools::GraphToolFactory;
#[cfg(any(
    feature = "openai",
    feature = "anthropic",
    feature = "ollama",
    feature = "xai"
))]
use anyhow::{Result, anyhow};
#[cfg(any(
    feature = "openai",
    feature = "anthropic",
    feature = "ollama",
    feature = "xai"
))]
use async_trait::async_trait;
#[cfg(any(
    feature = "openai",
    feature = "anthropic",
    feature = "ollama",
    feature = "xai"
))]
use codegraph_mcp_core::context_aware_limits::ContextTier;
#[cfg(any(
    feature = "openai",
    feature = "anthropic",
    feature = "ollama",
    feature = "xai"
))]
use futures::{Stream, stream};
#[cfg(any(
    feature = "openai",
    feature = "anthropic",
    feature = "ollama",
    feature = "xai"
))]
use std::pin::Pin;

/// OpenAI-based Rig agent
#[cfg(feature = "openai")]
pub struct OpenAIAgent {
    pub(crate) agent: rig_agent::Agent,
    pub(crate) factory: GraphToolFactory,
    pub(crate) max_turns: usize,
    pub(crate) tier: ContextTier,
}

#[cfg(feature = "openai")]
#[async_trait]
impl RigAgentTrait for OpenAIAgent {
    async fn execute(&self, query: &str) -> Result<String> {
        let response = self
            .agent
            .prompt(query)
            .max_turns(self.max_turns)
            .await
            .map_err(|e| anyhow!("Agent execution failed: {}", e))?;

        Ok(response.output)
    }

    async fn execute_stream(
        &self,
        query: &str,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<AgentEvent>> + Send>>> {
        // Retain buffered events while the shared executor consumes complete answers.
        let response = self.execute(query).await?;
        let events = vec![
            Ok(AgentEvent::Thinking("Agent processing...".to_string())),
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
    pub(crate) agent: rig_agent::Agent,
    pub(crate) factory: GraphToolFactory,
    pub(crate) max_turns: usize,
    pub(crate) tier: ContextTier,
}

#[cfg(feature = "anthropic")]
#[async_trait]
impl RigAgentTrait for AnthropicAgent {
    async fn execute(&self, query: &str) -> Result<String> {
        let response = self
            .agent
            .prompt(query)
            .max_turns(self.max_turns)
            .await
            .map_err(|e| anyhow!("Agent execution failed: {}", e))?;

        Ok(response.output)
    }

    async fn execute_stream(
        &self,
        query: &str,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<AgentEvent>> + Send>>> {
        let response = self.execute(query).await?;
        let events = vec![
            Ok(AgentEvent::Thinking("Agent processing...".to_string())),
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
    pub(crate) agent: rig_agent::Agent,
    pub(crate) factory: GraphToolFactory,
    pub(crate) max_turns: usize,
    pub(crate) tier: ContextTier,
}

#[cfg(feature = "ollama")]
#[async_trait]
impl RigAgentTrait for OllamaAgent {
    async fn execute(&self, query: &str) -> Result<String> {
        let response = self
            .agent
            .prompt(query)
            .max_turns(self.max_turns)
            .await
            .map_err(|e| anyhow!("Agent execution failed: {}", e))?;

        Ok(response.output)
    }

    async fn execute_stream(
        &self,
        query: &str,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<AgentEvent>> + Send>>> {
        let response = self.execute(query).await?;
        let events = vec![
            Ok(AgentEvent::Thinking("Agent processing...".to_string())),
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
    pub(crate) agent: rig_agent::Agent,
    pub(crate) factory: GraphToolFactory,
    pub(crate) max_turns: usize,
    pub(crate) tier: ContextTier,
}

#[cfg(feature = "xai")]
#[async_trait]
impl RigAgentTrait for XAIAgent {
    async fn execute(&self, query: &str) -> Result<String> {
        let response = self
            .agent
            .prompt(query)
            .max_turns(self.max_turns)
            .await
            .map_err(|e| anyhow!("Agent execution failed: {}", e))?;

        Ok(response.output)
    }

    async fn execute_stream(
        &self,
        query: &str,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<AgentEvent>> + Send>>> {
        let response = self.execute(query).await?;
        let events = vec![
            Ok(AgentEvent::Thinking("Agent processing...".to_string())),
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

#[cfg(all(test, feature = "ollama"))]
mod tests {
    use super::*;
    use axum::{Json, Router, routing::post};
    use codegraph_core::CodeGraphConfig;
    use codegraph_graph::{GraphFunctions, SurrealDbConfig, SurrealDbStorage};
    use codegraph_mcp_tools::GraphToolExecutor;
    use codegraph_vector::EmbeddingGenerator;
    use serde_json::{Value, json};
    use std::sync::{Arc, Mutex};

    #[tokio::test]
    async fn ollama_agent_runs_graph_tool_and_preserves_trace_across_turns() {
        let storage = SurrealDbStorage::new(SurrealDbConfig {
            connection: "mem://".to_string(),
            username: None,
            password: None,
            ..Default::default()
        })
        .await
        .unwrap();
        storage
            .db()
            .query(
                "DEFINE FUNCTION fn::get_hub_nodes($project: string, $degree: int) { RETURN []; };",
            )
            .await
            .unwrap()
            .check()
            .unwrap();
        let functions = Arc::new(GraphFunctions::new_with_project_id(
            storage.db(),
            "rig-test",
        ));
        let executor = Arc::new(GraphToolExecutor::new(
            functions,
            Arc::new(CodeGraphConfig::default()),
            Arc::new(EmbeddingGenerator::default()),
        ));
        let factory = GraphToolFactory::new(executor);
        let requests = Arc::new(Mutex::new(Vec::<Value>::new()));
        let recorded = requests.clone();
        let app = Router::new().route("/api/chat", post(move |Json(request): Json<Value>| {
            let mut requests = recorded.lock().unwrap();
            let first = requests.is_empty();
            requests.push(request);
            async move {
                let message = if first {
                    json!({"role": "assistant", "content": "", "tool_calls": [{"function": {"name": "get_hub_nodes", "arguments": {"min_degree": 1}}}]})
                } else {
                    json!({"role": "assistant", "content": "The graph has no hub nodes."})
                };
                Json(json!({"model": "mock-model", "created_at": "2026-01-01T00:00:00Z", "message": message, "done": true}))
            }
        }));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        let client = rig::providers::ollama::OllamaConfig::new()
            .with_base_url(format!("http://{address}"))
            .client();
        let agent = OllamaAgent {
            agent: rig_agent::AgentBuilder::new(client.completion("mock-model"))
                .tool(factory.hub_nodes())
                .build(),
            factory,
            max_turns: 3,
            tier: ContextTier::from_context_window(128_000),
        };
        let answer = tokio::time::timeout(
            std::time::Duration::from_secs(10),
            agent.execute("Find hub nodes"),
        )
        .await
        .unwrap()
        .unwrap();
        server.abort();
        assert_eq!(answer, "The graph has no hub nodes.");
        assert_eq!(agent.take_tool_call_count(), 1);
        let traces = agent.take_tool_traces();
        assert_eq!(traces.len(), 1);
        assert_eq!(traces[0].tool_name, "get_hub_nodes");
        assert_eq!(traces[0].parameters["min_degree"], 1);
        assert!(traces[0].result.is_some());
        assert!(traces[0].error.is_none());
        let requests = requests.lock().unwrap();
        assert_eq!(requests.len(), 2);
        assert!(
            requests[0]["tools"]
                .as_array()
                .unwrap()
                .iter()
                .any(|tool| tool["function"]["name"] == "get_hub_nodes")
        );
        assert!(
            requests[1]["messages"]
                .as_array()
                .unwrap()
                .iter()
                .any(|message| message["role"] == "tool")
        );
    }
}
