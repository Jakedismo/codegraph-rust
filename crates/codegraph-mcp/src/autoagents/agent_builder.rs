// ABOUTME: Factory for creating AutoAgents with CodeGraph-specific configuration
// ABOUTME: Bridges codegraph_ai LLM providers to AutoAgents ChatProvider
// ABOUTME: Builder for tier-aware CodeGraph agents with graph analysis tools

use async_trait::async_trait;
use autoagents::llm::chat::ChatProvider;
use autoagents::llm::chat::{ChatMessage, ChatMessageBuilder, ChatResponse, ChatRole, Tool};
use autoagents::llm::completion::{CompletionProvider, CompletionRequest, CompletionResponse};
use autoagents::llm::embedding::EmbeddingProvider;
use autoagents::llm::error::LLMError;
use autoagents::llm::models::{ModelListRequest, ModelListResponse, ModelsProvider};
use autoagents::llm::{FunctionCall, ToolCall};
use codegraph_ai::llm_provider::{LLMProvider as CodeGraphLLM, Message, MessageRole};
use serde::Deserialize;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

/// Convert CodeGraph Message to AutoAgents ChatMessage
pub(crate) fn convert_to_chat_message(msg: &Message) -> ChatMessage {
    let builder = match msg.role {
        MessageRole::System => ChatMessageBuilder::new(ChatRole::System),
        MessageRole::User => ChatMessage::user(),
        MessageRole::Assistant => ChatMessage::assistant(),
    };

    builder.content(&msg.content).build()
}

/// Convert CodeGraph Messages to AutoAgents ChatMessages
pub(crate) fn convert_messages(messages: &[Message]) -> Vec<ChatMessage> {
    messages.iter().map(convert_to_chat_message).collect()
}

/// Adapter that bridges codegraph_ai::LLMProvider to AutoAgents ChatProvider
pub struct CodeGraphChatAdapter {
    provider: Arc<dyn CodeGraphLLM>,
}

impl CodeGraphChatAdapter {
    pub fn new(provider: Arc<dyn CodeGraphLLM>) -> Self {
        Self { provider }
    }
}

#[async_trait]
impl ChatProvider for CodeGraphChatAdapter {
    async fn chat(
        &self,
        messages: &[ChatMessage],
        _tools: Option<&[Tool]>,
        _json_schema: Option<autoagents::llm::chat::StructuredOutputFormat>,
    ) -> Result<Box<dyn ChatResponse>, LLMError> {
        // Convert AutoAgents messages to CodeGraph messages
        let cg_messages: Vec<Message> = messages
            .iter()
            .map(|msg| Message {
                role: match msg.role {
                    ChatRole::System => MessageRole::System,
                    ChatRole::User => MessageRole::User,
                    ChatRole::Assistant => MessageRole::Assistant,
                    ChatRole::Tool => MessageRole::User, // Fallback: treat tool as user
                },
                content: msg.content.clone(),
            })
            .collect();

        // Call CodeGraph LLM provider
        let config = codegraph_ai::llm_provider::GenerationConfig {
            temperature: 0.1,
            max_tokens: None,
            ..Default::default()
        };

        let response = self
            .provider
            .generate_chat(&cg_messages, &config)
            .await
            .map_err(|e| LLMError::Generic(e.to_string()))?;

        // Wrap response in AutoAgents ChatResponse
        Ok(Box::new(CodeGraphChatResponse {
            content: response.content,
            _total_tokens: response.total_tokens.unwrap_or(0),
        }))
    }

    async fn chat_stream(
        &self,
        _messages: &[ChatMessage],
        _tools: Option<&[Tool]>,
        _json_schema: Option<autoagents::llm::chat::StructuredOutputFormat>,
    ) -> Result<
        std::pin::Pin<Box<dyn futures::Stream<Item = Result<String, LLMError>> + Send>>,
        LLMError,
    > {
        Err(LLMError::Generic("Streaming not supported".to_string()))
    }

    async fn chat_stream_struct(
        &self,
        _messages: &[ChatMessage],
        _tools: Option<&[Tool]>,
        _json_schema: Option<autoagents::llm::chat::StructuredOutputFormat>,
    ) -> Result<
        std::pin::Pin<
            Box<
                dyn futures::Stream<Item = Result<autoagents::llm::chat::StreamResponse, LLMError>>
                    + Send,
            >,
        >,
        LLMError,
    > {
        Err(LLMError::Generic(
            "Structured streaming not supported".to_string(),
        ))
    }
}

#[async_trait]
impl CompletionProvider for CodeGraphChatAdapter {
    async fn complete(
        &self,
        _req: &CompletionRequest,
        _json_schema: Option<autoagents::llm::chat::StructuredOutputFormat>,
    ) -> Result<CompletionResponse, LLMError> {
        Err(LLMError::Generic(
            "Completion not supported - use ChatProvider instead".to_string(),
        ))
    }
}

#[async_trait]
impl EmbeddingProvider for CodeGraphChatAdapter {
    async fn embed(&self, _text: Vec<String>) -> Result<Vec<Vec<f32>>, LLMError> {
        Err(LLMError::Generic(
            "Embedding not supported - CodeGraph uses separate embedding providers".to_string(),
        ))
    }
}

#[async_trait]
impl ModelsProvider for CodeGraphChatAdapter {
    async fn list_models(
        &self,
        _request: Option<&ModelListRequest>,
    ) -> Result<Box<dyn ModelListResponse>, LLMError> {
        Err(LLMError::Generic("Model listing not supported".to_string()))
    }
}

// Implement the LLMProvider supertrait (combines all provider traits)
impl autoagents::llm::LLMProvider for CodeGraphChatAdapter {}

/// ChatResponse wrapper for CodeGraph LLM responses
#[derive(Debug)]
struct CodeGraphChatResponse {
    content: String,
    _total_tokens: usize,
}

impl std::fmt::Display for CodeGraphChatResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.content)
    }
}

/// Helper struct to parse CodeGraph LLM response format
#[derive(Debug, Deserialize)]
struct CodeGraphLLMResponse {
    #[serde(default)]
    reasoning: Option<String>,
    #[serde(default)]
    tool_call: Option<CodeGraphToolCall>,
    #[serde(default)]
    is_final: bool,
}

#[derive(Debug, Deserialize)]
struct CodeGraphToolCall {
    tool_name: String,
    parameters: serde_json::Value,
}

// Static counter for generating unique tool call IDs
static TOOL_CALL_COUNTER: AtomicU64 = AtomicU64::new(0);

impl ChatResponse for CodeGraphChatResponse {
    fn text(&self) -> Option<String> {
        Some(self.content.clone())
    }

    fn tool_calls(&self) -> Option<Vec<ToolCall>> {
        // Try to parse the response as CodeGraph's JSON format
        match serde_json::from_str::<CodeGraphLLMResponse>(&self.content) {
            Ok(parsed) => {
                // If there's a tool_call and is_final is false, convert to AutoAgents format
                if let Some(tool_call) = parsed.tool_call {
                    if !parsed.is_final {
                        // Convert parameters to JSON string
                        let arguments = match serde_json::to_string(&tool_call.parameters) {
                            Ok(json) => json,
                            Err(e) => {
                                tracing::warn!(
                                    "Failed to serialize tool call parameters: {}",
                                    e
                                );
                                return None;
                            }
                        };

                        // Generate unique ID using atomic counter
                        let call_id = TOOL_CALL_COUNTER.fetch_add(1, Ordering::SeqCst);

                        tracing::debug!(
                            "Parsed tool call: {} with args: {}",
                            tool_call.tool_name,
                            arguments
                        );

                        let autoagents_tool_call = ToolCall {
                            id: format!("call_{}", call_id),
                            call_type: "function".to_string(),
                            function: FunctionCall {
                                name: tool_call.tool_name,
                                arguments,
                            },
                        };

                        return Some(vec![autoagents_tool_call]);
                    } else {
                        tracing::debug!("is_final is true, not returning tool calls");
                    }
                } else {
                    tracing::debug!("No tool_call in parsed response");
                }
            }
            Err(e) => {
                // Not a JSON response or doesn't match our format - that's fine
                tracing::trace!("Response is not CodeGraph JSON format: {}", e);
            }
        }

        // No tool calls in response
        None
    }
}

// ============================================================================
// Agent Builder
// ============================================================================

use crate::autoagents::tier_plugin::TierAwarePromptPlugin;
use crate::autoagents::tools::graph_tools::*;
use crate::autoagents::tools::tool_executor_adapter::GraphToolFactory;
use crate::context_aware_limits::ContextTier;
use crate::{AnalysisType, GraphToolExecutor};

use crate::autoagents::codegraph_agent::CodeGraphAgentOutput;
use autoagents::core::agent::memory::SlidingWindowMemory;
use autoagents::core::agent::prebuilt::executor::ReActAgent;
use autoagents::core::agent::AgentBuilder;
use autoagents::core::agent::{AgentDeriveT, AgentHooks, AgentOutputT, DirectAgentHandle};
use autoagents::core::error::Error as AutoAgentsError;
use autoagents::core::tool::{shared_tools_to_boxes, ToolT};
use autoagents_derive::AgentHooks;

/// Agent implementation for CodeGraph with manual tool registration
#[derive(Debug)]
pub struct CodeGraphReActAgent {
    tools: Vec<Arc<dyn ToolT>>,
    system_prompt: String,
}

impl AgentDeriveT for CodeGraphReActAgent {
    type Output = CodeGraphAgentOutput;

    fn description(&self) -> &'static str {
        // Use Box::leak to convert runtime String to &'static str
        // This is the standard AutoAgents pattern for dynamic descriptions
        Box::leak(self.system_prompt.clone().into_boxed_str())
    }

    fn name(&self) -> &'static str {
        "codegraph_agent"
    }

    fn output_schema(&self) -> Option<serde_json::Value> {
        Some(CodeGraphAgentOutput::structured_output_format())
    }

    fn tools(&self) -> Vec<Box<dyn ToolT>> {
        shared_tools_to_boxes(&self.tools)
    }
}

impl AgentHooks for CodeGraphReActAgent {}

/// Builder for CodeGraph AutoAgents workflows
pub struct CodeGraphAgentBuilder {
    llm_adapter: Arc<CodeGraphChatAdapter>,
    tool_factory: GraphToolFactory,
    tier: ContextTier,
    analysis_type: AnalysisType,
}

impl CodeGraphAgentBuilder {
    pub fn new(
        llm_provider: Arc<dyn codegraph_ai::llm_provider::LLMProvider>,
        tool_executor: Arc<GraphToolExecutor>,
        tier: ContextTier,
        analysis_type: AnalysisType,
    ) -> Self {
        Self {
            llm_adapter: Arc::new(CodeGraphChatAdapter::new(llm_provider)),
            tool_factory: GraphToolFactory::new(tool_executor),
            tier,
            analysis_type,
        }
    }

    pub async fn build(self) -> Result<AgentHandle, AutoAgentsError> {
        // Get tier-aware configuration and system prompt
        let tier_plugin = TierAwarePromptPlugin::new(self.analysis_type, self.tier);
        let system_prompt = tier_plugin
            .get_system_prompt()
            .map_err(|e| AutoAgentsError::CustomError(e.to_string()))?;

        // Create memory (sliding window keeps last N messages)
        let memory_size = tier_plugin.get_max_iterations() * 2;
        let memory = Box::new(SlidingWindowMemory::new(memory_size));

        // Get executor adapter for tool construction
        let executor_adapter = self.tool_factory.adapter();

        // Manually construct all 6 tools with the executor (Arc-wrapped for sharing)
        let tools: Vec<Arc<dyn ToolT>> = vec![
            Arc::new(GetTransitiveDependencies::new(executor_adapter.clone())),
            Arc::new(GetReverseDependencies::new(executor_adapter.clone())),
            Arc::new(TraceCallChain::new(executor_adapter.clone())),
            Arc::new(DetectCycles::new(executor_adapter.clone())),
            Arc::new(CalculateCoupling::new(executor_adapter.clone())),
            Arc::new(GetHubNodes::new(executor_adapter.clone())),
        ];

        // Create CodeGraph agent with tools and tier-aware system prompt
        let codegraph_agent = CodeGraphReActAgent {
            tools,
            system_prompt,
        };

        // Build ReAct agent with our CodeGraph agent
        let react_agent = ReActAgent::new(codegraph_agent);

        // Build full agent with configuration
        // System prompt injected via AgentDeriveT::description() using Box::leak pattern
        use autoagents::core::agent::DirectAgent;
        let agent = AgentBuilder::<_, DirectAgent>::new(react_agent)
            .llm(self.llm_adapter)
            .memory(memory)
            .build()
            .await?;

        Ok(AgentHandle {
            agent,
            tier: self.tier,
            analysis_type: self.analysis_type,
        })
    }
}

/// Handle for executing CodeGraph agent
pub struct AgentHandle {
    pub agent: DirectAgentHandle<ReActAgent<CodeGraphReActAgent>>,
    pub tier: ContextTier,
    pub analysis_type: AnalysisType,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_conversion_user() {
        let cg_msg = Message {
            role: MessageRole::User,
            content: "Hello".to_string(),
        };

        let aa_msg = convert_to_chat_message(&cg_msg);

        assert_eq!(aa_msg.role, ChatRole::User);
        assert_eq!(aa_msg.content, "Hello");
    }

    #[test]
    fn test_message_conversion_system() {
        let cg_msg = Message {
            role: MessageRole::System,
            content: "You are helpful".to_string(),
        };

        let aa_msg = convert_to_chat_message(&cg_msg);

        assert_eq!(aa_msg.role, ChatRole::System);
    }

    #[test]
    fn test_message_conversion_assistant() {
        let cg_msg = Message {
            role: MessageRole::Assistant,
            content: "I can help".to_string(),
        };

        let aa_msg = convert_to_chat_message(&cg_msg);

        assert_eq!(aa_msg.role, ChatRole::Assistant);
    }

    #[test]
    fn test_convert_messages_batch() {
        let cg_messages = vec![
            Message {
                role: MessageRole::System,
                content: "System".to_string(),
            },
            Message {
                role: MessageRole::User,
                content: "User".to_string(),
            },
        ];

        let aa_messages = convert_messages(&cg_messages);

        assert_eq!(aa_messages.len(), 2);
        assert_eq!(aa_messages[0].role, ChatRole::System);
        assert_eq!(aa_messages[1].role, ChatRole::User);
    }

    // Integration test for ChatProvider
    struct MockCodeGraphLLM;

    #[async_trait]
    impl CodeGraphLLM for MockCodeGraphLLM {
        async fn generate_chat(
            &self,
            messages: &[Message],
            _config: &codegraph_ai::llm_provider::GenerationConfig,
        ) -> codegraph_ai::llm_provider::LLMResult<codegraph_ai::llm_provider::Response> {
            Ok(codegraph_ai::llm_provider::Response {
                content: format!("Echo: {}", messages.last().unwrap().content),
                total_tokens: 10,
            })
        }
    }

    #[tokio::test]
    async fn test_chat_adapter_integration() {
        let mock_llm = Arc::new(MockCodeGraphLLM);
        let adapter = CodeGraphChatAdapter::new(mock_llm);

        let messages = vec![ChatMessage::user().content("Hello").build()];
        let response = adapter.chat(&messages, None, None).await.unwrap();

        assert_eq!(response.text(), Some("Echo: Hello".to_string()));
    }
}
