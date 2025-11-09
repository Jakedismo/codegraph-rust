// ABOUTME: Factory for creating AutoAgents with CodeGraph-specific configuration
// ABOUTME: Bridges codegraph_ai LLM providers to AutoAgents ChatProvider

use autoagents::llm::chat::{ChatMessage, ChatMessageBuilder, ChatRole, ChatResponse, Tool};
use autoagents::llm::chat::ChatProvider;
use autoagents::llm::completion::{CompletionProvider, CompletionRequest, CompletionResponse};
use autoagents::llm::embedding::EmbeddingProvider;
use autoagents::llm::models::{ModelListRequest, ModelListResponse, ModelsProvider};
use autoagents::llm::error::LLMError;
use autoagents::llm::ToolCall;
use codegraph_ai::llm_provider::{Message, MessageRole, LLMProvider as CodeGraphLLM};
use async_trait::async_trait;
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
        Err(LLMError::Generic(
            "Streaming not supported".to_string(),
        ))
    }

    async fn chat_stream_struct(
        &self,
        _messages: &[ChatMessage],
        _tools: Option<&[Tool]>,
        _json_schema: Option<autoagents::llm::chat::StructuredOutputFormat>,
    ) -> Result<
        std::pin::Pin<
            Box<dyn futures::Stream<Item = Result<autoagents::llm::chat::StreamResponse, LLMError>> + Send>,
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
        Err(LLMError::Generic(
            "Model listing not supported".to_string(),
        ))
    }
}

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

impl ChatResponse for CodeGraphChatResponse {
    fn text(&self) -> Option<String> {
        Some(self.content.clone())
    }

    fn tool_calls(&self) -> Option<Vec<ToolCall>> {
        None // CodeGraph doesn't use tool calls in responses
    }
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

        assert_eq!(response.text(), "Echo: Hello");
    }
}
