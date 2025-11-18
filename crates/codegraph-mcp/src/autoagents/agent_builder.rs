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
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

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
    tier: ContextTier,
}

impl CodeGraphChatAdapter {
    pub fn new(provider: Arc<dyn CodeGraphLLM>, tier: ContextTier) -> Self {
        Self { provider, tier }
    }

    /// Get tier-aware max_tokens, respecting environment variable override
    fn get_max_tokens(&self) -> Option<usize> {
        // Check for environment variable override first
        if let Ok(val) = std::env::var("MCP_CODE_AGENT_MAX_OUTPUT_TOKENS") {
            if let Ok(tokens) = val.parse::<usize>() {
                tracing::info!("Using MCP_CODE_AGENT_MAX_OUTPUT_TOKENS={}", tokens);
                return Some(tokens);
            }
        }

        // Use tier-based defaults
        let tokens = match self.tier {
            ContextTier::Small => 2048,
            ContextTier::Medium => 4096,
            ContextTier::Large => 8192,
            ContextTier::Massive => 16384,
        };

        Some(tokens)
    }
}

#[async_trait]
impl ChatProvider for CodeGraphChatAdapter {
    async fn chat(
        &self,
        messages: &[ChatMessage],
        _tools: Option<&[Tool]>,
        json_schema: Option<autoagents::llm::chat::StructuredOutputFormat>,
    ) -> Result<Box<dyn ChatResponse>, LLMError> {
        // Log whether schema is being passed by AutoAgents
        if json_schema.is_some() {
            tracing::info!("✅ AutoAgents passed json_schema to chat()");
        } else {
            tracing::warn!("⚠️  AutoAgents did NOT pass json_schema - schema enforcement may be prompt-only!");
        }

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

        // Convert AutoAgents json_schema to CodeGraph ResponseFormat
        let response_format = json_schema.and_then(|schema| {
            schema.schema.map(|schema_value| {
                codegraph_ai::llm_provider::ResponseFormat::JsonSchema {
                    json_schema: codegraph_ai::llm_provider::JsonSchema {
                        name: schema.name,
                        schema: schema_value,
                        strict: schema.strict.unwrap_or(true),
                    },
                }
            })
        });

        // Call CodeGraph LLM provider with structured output support
        let config = codegraph_ai::llm_provider::GenerationConfig {
            temperature: 0.1,
            max_tokens: self.get_max_tokens(),
            response_format,
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
        tracing::info!(
            "tool_calls() called with content length: {}",
            self.content.len()
        );
        tracing::debug!(
            "Content preview: {}",
            &self.content.chars().take(200).collect::<String>()
        );

        // Try to parse the response as CodeGraph's JSON format
        match serde_json::from_str::<CodeGraphLLMResponse>(&self.content) {
            Ok(parsed) => {
                tracing::info!(
                    "Successfully parsed CodeGraphLLMResponse. has_tool_call={}, is_final={}",
                    parsed.tool_call.is_some(),
                    parsed.is_final
                );

                // If there's a tool_call and is_final is false, convert to AutoAgents format
                if let Some(tool_call) = parsed.tool_call {
                    if !parsed.is_final {
                        // Convert parameters to JSON string
                        let arguments = match serde_json::to_string(&tool_call.parameters) {
                            Ok(json) => json,
                            Err(e) => {
                                tracing::error!("Failed to serialize tool call parameters: {}", e);
                                return None;
                            }
                        };

                        // Generate unique ID using atomic counter
                        let call_id = TOOL_CALL_COUNTER.fetch_add(1, Ordering::SeqCst);

                        let autoagents_tool_call = ToolCall {
                            id: format!("call_{}", call_id),
                            call_type: "function".to_string(),
                            function: FunctionCall {
                                name: tool_call.tool_name.clone(),
                                arguments: arguments.clone(),
                            },
                        };

                        tracing::info!(
                            "Returning tool call: name='{}', args='{}', id='{}'",
                            tool_call.tool_name,
                            arguments,
                            autoagents_tool_call.id
                        );

                        return Some(vec![autoagents_tool_call]);
                    } else {
                        tracing::info!("is_final is true, not returning tool calls");
                    }
                } else {
                    tracing::info!("No tool_call field in parsed response");
                }
            }
            Err(e) => {
                // Not a JSON response or doesn't match our format - that's fine
                tracing::warn!(
                    "Response is not CodeGraph JSON format: {}. Content: {}",
                    e,
                    &self.content.chars().take(500).collect::<String>()
                );
            }
        }

        tracing::info!("tool_calls() returning None");
        None
    }
}

// ============================================================================
// Tier-Aware ReAct Agent Wrapper
// ============================================================================

/// Wrapper around ReActAgent that overrides max_turns configuration
/// This allows tier-aware max_turns without forking AutoAgents
#[derive(Debug)]
pub struct TierAwareReActAgent<T: AgentDeriveT> {
    inner: ReActAgent<T>,
    inner_derive: Arc<T>,
    max_turns: usize,
}

impl<T: AgentDeriveT + AgentHooks + Clone> TierAwareReActAgent<T> {
    pub fn new(agent: T, max_turns: usize) -> Self {
        let agent_arc = Arc::new(agent);
        Self {
            inner: ReActAgent::new((*agent_arc).clone()),
            inner_derive: agent_arc,
            max_turns,
        }
    }
}

impl<T: AgentDeriveT + AgentHooks + Clone> AgentDeriveT for TierAwareReActAgent<T> {
    type Output = T::Output;

    fn description(&self) -> &'static str {
        self.inner_derive.description()
    }

    fn name(&self) -> &'static str {
        self.inner_derive.name()
    }

    fn output_schema(&self) -> Option<serde_json::Value> {
        self.inner_derive.output_schema()
    }

    fn tools(&self) -> Vec<Box<dyn ToolT>> {
        self.inner_derive.tools()
    }
}

impl<T: AgentDeriveT + AgentHooks + Clone> AgentHooks for TierAwareReActAgent<T> {}

impl<T: AgentDeriveT + AgentHooks + Clone> Clone for TierAwareReActAgent<T> {
    fn clone(&self) -> Self {
        Self {
            inner: ReActAgent::new((*self.inner_derive).clone()),
            inner_derive: Arc::clone(&self.inner_derive),
            max_turns: self.max_turns,
        }
    }
}

#[async_trait]
impl<T: AgentDeriveT + AgentHooks + Clone> AgentExecutor for TierAwareReActAgent<T> {
    type Output = <ReActAgent<T> as AgentExecutor>::Output;
    type Error = <ReActAgent<T> as AgentExecutor>::Error;

    fn config(&self) -> ExecutorConfig {
        ExecutorConfig {
            max_turns: self.max_turns,
        }
    }

    async fn execute(
        &self,
        task: &autoagents::core::agent::task::Task,
        context: Arc<Context>,
    ) -> Result<Self::Output, Self::Error> {
        self.inner.execute(task, context).await
    }

    async fn execute_stream(
        &self,
        task: &autoagents::core::agent::task::Task,
        context: Arc<Context>,
    ) -> Result<
        std::pin::Pin<
            Box<
                dyn futures::Stream<Item = Result<Self::Output, Self::Error>> + Send,
            >,
        >,
        Self::Error,
    > {
        self.inner.execute_stream(task, context).await
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
use autoagents::core::agent::{AgentDeriveT, AgentExecutor, AgentHooks, AgentOutputT, Context, DirectAgentHandle, ExecutorConfig};
use autoagents::core::error::Error as AutoAgentsError;
use autoagents::core::tool::{shared_tools_to_boxes, ToolT};

/// Agent implementation for CodeGraph with manual tool registration
#[derive(Debug, Clone)]
pub struct CodeGraphReActAgent {
    tools: Vec<Arc<dyn ToolT>>,
    system_prompt: String,
    analysis_type: AnalysisType,
    max_iterations: usize,
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
        use codegraph_ai::agentic_schemas::*;
        use schemars::schema_for;

        // Return the appropriate schema based on analysis type
        let schema = match self.analysis_type {
            AnalysisType::CodeSearch => schema_for!(CodeSearchOutput),
            AnalysisType::DependencyAnalysis => schema_for!(DependencyAnalysisOutput),
            AnalysisType::CallChainAnalysis => schema_for!(CallChainOutput),
            AnalysisType::ArchitectureAnalysis => schema_for!(ArchitectureAnalysisOutput),
            AnalysisType::ApiSurfaceAnalysis => schema_for!(APISurfaceOutput),
            AnalysisType::ContextBuilder => schema_for!(ContextBuilderOutput),
            AnalysisType::SemanticQuestion => schema_for!(SemanticQuestionOutput),
        };

        serde_json::to_value(schema).ok()
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
            llm_adapter: Arc::new(CodeGraphChatAdapter::new(llm_provider, tier)),
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

        // Get tier-aware max iterations
        let max_iterations = tier_plugin.get_max_iterations();
        tracing::info!("Setting ReActAgent max_turns={} for tier={:?}", max_iterations, self.tier);

        // Create CodeGraph agent with tools and tier-aware system prompt
        let codegraph_agent = CodeGraphReActAgent {
            tools,
            system_prompt,
            analysis_type: self.analysis_type,
            max_iterations,
        };

        // Wrap in TierAwareReActAgent to override max_turns configuration
        let tier_aware_agent = TierAwareReActAgent::new(codegraph_agent, max_iterations);

        // Build full agent with configuration
        // System prompt injected via AgentDeriveT::description() using Box::leak pattern
        use autoagents::core::agent::DirectAgent;
        let agent = AgentBuilder::<_, DirectAgent>::new(tier_aware_agent)
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
    pub agent: DirectAgentHandle<TierAwareReActAgent<CodeGraphReActAgent>>,
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
        let adapter = CodeGraphChatAdapter::new(mock_llm, ContextTier::Medium);

        let messages = vec![ChatMessage::user().content("Hello").build()];
        let response = adapter.chat(&messages, None, None).await.unwrap();

        assert_eq!(response.text(), Some("Echo: Hello".to_string()));
    }

    #[test]
    fn test_tier_aware_max_turns_small() {
        // Test that Small tier gets 5 max_turns
        let agent = create_mock_codegraph_agent(ContextTier::Small);
        let wrapper = TierAwareReActAgent::new(agent, 5);

        let config = wrapper.config();
        assert_eq!(config.max_turns, 5, "Small tier should have 5 max_turns");
    }

    #[test]
    fn test_tier_aware_max_turns_medium() {
        // Test that Medium tier gets 10 max_turns
        let agent = create_mock_codegraph_agent(ContextTier::Medium);
        let wrapper = TierAwareReActAgent::new(agent, 10);

        let config = wrapper.config();
        assert_eq!(config.max_turns, 10, "Medium tier should have 10 max_turns");
    }

    #[test]
    fn test_tier_aware_max_turns_large() {
        // Test that Large tier gets 15 max_turns
        let agent = create_mock_codegraph_agent(ContextTier::Large);
        let wrapper = TierAwareReActAgent::new(agent, 15);

        let config = wrapper.config();
        assert_eq!(config.max_turns, 15, "Large tier should have 15 max_turns");
    }

    #[test]
    fn test_tier_aware_max_turns_massive() {
        // Test that Massive tier gets 20 max_turns
        let agent = create_mock_codegraph_agent(ContextTier::Massive);
        let wrapper = TierAwareReActAgent::new(agent, 20);

        let config = wrapper.config();
        assert_eq!(config.max_turns, 20, "Massive tier should have 20 max_turns");
    }

    // Helper function to create mock agent for testing
    fn create_mock_codegraph_agent(_tier: ContextTier) -> CodeGraphReActAgent {
        CodeGraphReActAgent {
            tools: vec![],
            system_prompt: "Test prompt".to_string(),
            analysis_type: AnalysisType::CodeSearch,
            max_iterations: 10,
        }
    }
}
