# AutoAgents Integration Implementation Plan (REVISED)

> **Status:** Active - Based on real AutoAgents v0.2.4 API
> **Supersedes:** `2025-11-09-autoagents-integration.md` (paused after Task 2)
> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Replace CodeGraph's custom agentic orchestrator with AutoAgents framework using the actual v0.2.4 API discovered through research.

**Key Changes from Original Plan:**
- LLM adapter now implements 4 traits instead of 1
- Tools are synchronous with `serde_json::Value` params
- Agent definition uses `#[agent(...)]` macro with compile-time tool list
- Revised effort: ~32 hours (vs original 12-16 hours)

---

## Progress Summary

**Completed from Original Plan:**
- ✅ Task 1: Add AutoAgents dependency to Cargo.toml (commit: 668522f)
- ✅ Task 2: Create AutoAgents module structure (commit: 6bab700)
- ✅ Research: AutoAgents API investigation (commit: 0ceab3f)

**Starting Point:** Task 3 with corrected API understanding

---

## Phase 1: Foundation (COMPLETED)

### ✅ Task 1: Add AutoAgents Dependency
Already completed - see commit 668522f

### ✅ Task 2: Create AutoAgents Module Structure
Already completed - see commit 6bab700

---

## Phase 2: LLM Provider Bridge (6 hours)

### Task 3: Implement ChatProvider Adapter

**Files:**
- Modify: `crates/codegraph-mcp/src/autoagents/agent_builder.rs`

**Step 1: Write test for ChatMessage conversion**

```rust
// ABOUTME: Factory for creating AutoAgents with CodeGraph-specific configuration
// ABOUTME: Bridges codegraph_ai LLM providers to AutoAgents ChatProvider

use autoagents::llm::{ChatProvider, LLMProvider, LLMError};
use autoagents::llm::chat::{ChatMessage, ChatResponse};
use autoagents::llm::completion::CompletionProvider;
use autoagents::llm::embedding::EmbeddingProvider;
use autoagents::llm::models::ModelsProvider;
use codegraph_ai::llm_provider::{LLMProvider as CodeGraphLLM, Message, MessageRole};
use async_trait::async_trait;
use std::sync::Arc;

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

        assert_eq!(aa_msg.role(), "user");
        assert_eq!(aa_msg.content(), "Hello");
    }

    #[test]
    fn test_message_conversion_system() {
        let cg_msg = Message {
            role: MessageRole::System,
            content: "You are helpful".to_string(),
        };

        let aa_msg = convert_to_chat_message(&cg_msg);

        assert_eq!(aa_msg.role(), "system");
    }
}

fn convert_to_chat_message(msg: &Message) -> ChatMessage {
    // Implementation in Step 3
    todo!()
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p codegraph-mcp test_message_conversion --features autoagents-experimental`
Expected: FAIL - convert_to_chat_message not implemented

**Step 3: Implement message conversion helpers**

```rust
use autoagents::llm::chat::ChatMessage;

/// Convert CodeGraph Message to AutoAgents ChatMessage
fn convert_to_chat_message(msg: &Message) -> ChatMessage {
    let role = match msg.role {
        MessageRole::System => "system",
        MessageRole::User => "user",
        MessageRole::Assistant => "assistant",
    };

    ChatMessage::new(role, &msg.content)
}

/// Convert CodeGraph Messages to AutoAgents ChatMessages
fn convert_messages(messages: &[Message]) -> Vec<ChatMessage> {
    messages.iter().map(convert_to_chat_message).collect()
}
```

**Step 4: Run test to verify it passes**

Run: `cargo test -p codegraph-mcp test_message_conversion --features autoagents-experimental`
Expected: PASS

**Step 5: Implement ChatProvider adapter**

```rust
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
        _tools: Option<&[autoagents::core::tool::Tool]>,
        _json_schema: Option<autoagents::llm::chat::StructuredOutputFormat>,
    ) -> Result<Box<dyn ChatResponse>, LLMError> {
        // Convert AutoAgents messages to CodeGraph messages
        let cg_messages: Vec<Message> = messages
            .iter()
            .map(|msg| Message {
                role: match msg.role() {
                    "system" => MessageRole::System,
                    "user" => MessageRole::User,
                    "assistant" => MessageRole::Assistant,
                    _ => MessageRole::User, // fallback
                },
                content: msg.content().to_string(),
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
            total_tokens: response.total_tokens,
        }))
    }

    async fn chat_stream(
        &self,
        _messages: &[ChatMessage],
        _tools: Option<&[autoagents::core::tool::Tool]>,
        _json_schema: Option<autoagents::llm::chat::StructuredOutputFormat>,
    ) -> Result<std::pin::Pin<Box<dyn futures::Stream<Item = Result<String, LLMError>> + Send>>, LLMError> {
        Err(LLMError::Generic("Streaming not supported".to_string()))
    }

    async fn chat_stream_struct(
        &self,
        _messages: &[ChatMessage],
        _tools: Option<&[autoagents::core::tool::Tool]>,
        _json_schema: Option<autoagents::llm::chat::StructuredOutputFormat>,
    ) -> Result<
        std::pin::Pin<Box<dyn futures::Stream<Item = Result<autoagents::llm::chat::StreamResponse, LLMError>> + Send>>,
        LLMError,
    > {
        Err(LLMError::Generic("Structured streaming not supported".to_string()))
    }
}

/// ChatResponse wrapper for CodeGraph LLM responses
struct CodeGraphChatResponse {
    content: String,
    total_tokens: usize,
}

impl ChatResponse for CodeGraphChatResponse {
    fn text(&self) -> &str {
        &self.content
    }

    fn tool_calls(&self) -> Vec<autoagents::llm::chat::ToolCall> {
        vec![] // CodeGraph doesn't use tool calls in responses
    }
}
```

**Step 6: Verify compilation**

Run: `cargo check -p codegraph-mcp --features "ai-enhanced,autoagents-experimental"`
Expected: Compiles successfully

**Step 7: Commit ChatProvider adapter**

```bash
git add crates/codegraph-mcp/src/autoagents/agent_builder.rs
git commit -m "feat: implement ChatProvider adapter for AutoAgents"
```

---

### Task 4: Implement Minimal Stub Providers

**Files:**
- Modify: `crates/codegraph-mcp/src/autoagents/agent_builder.rs`

**Step 1: Implement CompletionProvider (delegates to chat)**

```rust
use autoagents::llm::completion::{CompletionProvider, CompletionResponse};

#[async_trait]
impl CompletionProvider for CodeGraphChatAdapter {
    async fn complete(
        &self,
        prompt: &str,
        _json_schema: Option<autoagents::llm::chat::StructuredOutputFormat>,
    ) -> Result<Box<dyn CompletionResponse>, LLMError> {
        // Convert to chat message and delegate
        let messages = vec![ChatMessage::new("user", prompt)];
        let chat_response = self.chat(&messages, None, None).await?;

        Ok(Box::new(CodeGraphCompletionResponse {
            text: chat_response.text().to_string(),
        }))
    }

    async fn complete_stream(
        &self,
        _prompt: &str,
        _json_schema: Option<autoagents::llm::chat::StructuredOutputFormat>,
    ) -> Result<std::pin::Pin<Box<dyn futures::Stream<Item = Result<String, LLMError>> + Send>>, LLMError> {
        Err(LLMError::Generic("Streaming not supported".to_string()))
    }
}

struct CodeGraphCompletionResponse {
    text: String,
}

impl CompletionResponse for CodeGraphCompletionResponse {
    fn text(&self) -> &str {
        &self.text
    }
}
```

**Step 2: Implement EmbeddingProvider (stub - not used)**

```rust
use autoagents::llm::embedding::EmbeddingProvider;

#[async_trait]
impl EmbeddingProvider for CodeGraphChatAdapter {
    async fn embed(&self, _texts: &[&str]) -> Result<Vec<Vec<f32>>, LLMError> {
        // Not used by ReAct agents, return empty
        Err(LLMError::Generic("Embeddings not supported in CodeGraph adapter".to_string()))
    }
}
```

**Step 3: Implement ModelsProvider (returns hardcoded)**

```rust
use autoagents::llm::models::ModelsProvider;

#[async_trait]
impl ModelsProvider for CodeGraphChatAdapter {
    async fn list_models(&self) -> Result<Vec<String>, LLMError> {
        // Return placeholder - not critical for our use case
        Ok(vec!["codegraph-llm".to_string()])
    }
}
```

**Step 4: Implement LLMProvider composite trait**

```rust
// Automatically satisfied since we implemented all 4 sub-traits
impl LLMProvider for CodeGraphChatAdapter {}
```

**Step 5: Verify compilation**

Run: `cargo check -p codegraph-mcp --features "ai-enhanced,autoagents-experimental"`
Expected: Compiles successfully

**Step 6: Write integration test**

```rust
#[cfg(test)]
mod integration_tests {
    use super::*;
    use codegraph_ai::llm_provider::{LLMProvider as _, Response};

    struct MockCodeGraphLLM;

    #[async_trait::async_trait]
    impl CodeGraphLLM for MockCodeGraphLLM {
        async fn generate_chat(
            &self,
            messages: &[Message],
            _config: &codegraph_ai::llm_provider::GenerationConfig,
        ) -> codegraph_ai::llm_provider::LLMResult<Response> {
            Ok(Response {
                content: format!("Echo: {}", messages.last().unwrap().content),
                total_tokens: 10,
            })
        }
    }

    #[tokio::test]
    async fn test_chat_adapter_integration() {
        let mock_llm = Arc::new(MockCodeGraphLLM);
        let adapter = CodeGraphChatAdapter::new(mock_llm);

        let messages = vec![ChatMessage::new("user", "Hello")];
        let response = adapter.chat(&messages, None, None).await.unwrap();

        assert_eq!(response.text(), "Echo: Hello");
    }
}
```

**Step 7: Run integration test**

Run: `cargo test -p codegraph-mcp test_chat_adapter_integration --features autoagents-experimental`
Expected: PASS

**Step 8: Commit complete LLM adapter**

```bash
git add crates/codegraph-mcp/src/autoagents/agent_builder.rs
git commit -m "feat: implement complete LLMProvider adapter with all 4 sub-traits"
```

---

## Phase 3: Tier-Aware Configuration (4 hours)

### Task 5: Implement Tier-Aware Prompt Plugin

**Files:**
- Modify: `crates/codegraph-mcp/src/autoagents/tier_plugin.rs`

**Step 1: Update tier_plugin.rs with real implementation**

```rust
// ABOUTME: AutoAgents plugin for tier-aware prompt injection
// ABOUTME: Selects prompts based on LLM context window and analysis type

use crate::{PromptSelector, AnalysisType};
use crate::context_aware_limits::ContextTier;
use crate::McpError;

/// Plugin that provides tier-aware system prompts and limits
pub struct TierAwarePromptPlugin {
    prompt_selector: PromptSelector,
    analysis_type: AnalysisType,
    tier: ContextTier,
}

impl TierAwarePromptPlugin {
    pub fn new(analysis_type: AnalysisType, tier: ContextTier) -> Self {
        Self {
            prompt_selector: PromptSelector::new(),
            analysis_type,
            tier,
        }
    }

    /// Get tier-appropriate system prompt
    pub fn get_system_prompt(&self) -> Result<String, McpError> {
        self.prompt_selector
            .select_prompt(self.analysis_type, self.tier)
            .map(|s| s.to_string())
    }

    /// Get tier-appropriate max_iterations (max_steps in original plan)
    pub fn get_max_iterations(&self) -> usize {
        self.prompt_selector
            .recommended_max_steps(self.tier, self.analysis_type)
    }

    /// Get tier-appropriate max_tokens for LLM responses
    pub fn get_max_tokens(&self) -> usize {
        match self.tier {
            ContextTier::Small => 2048,
            ContextTier::Medium => 4096,
            ContextTier::Large => 8192,
            ContextTier::Massive => 16384,
        }
    }

    /// Get temperature setting (consistent across tiers)
    pub fn get_temperature(&self) -> f32 {
        0.1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tier_plugin_max_iterations() {
        let small = TierAwarePromptPlugin::new(AnalysisType::CodeSearch, ContextTier::Small);
        let massive = TierAwarePromptPlugin::new(AnalysisType::CodeSearch, ContextTier::Massive);

        assert_eq!(small.get_max_iterations(), 5);
        assert_eq!(massive.get_max_iterations(), 20);
    }

    #[test]
    fn test_tier_plugin_max_tokens() {
        let plugin = TierAwarePromptPlugin::new(AnalysisType::CodeSearch, ContextTier::Medium);
        assert_eq!(plugin.get_max_tokens(), 4096);
    }
}
```

**Step 2: Run tests**

Run: `cargo test -p codegraph-mcp test_tier_plugin --features "ai-enhanced,autoagents-experimental"`
Expected: PASS

**Step 3: Commit tier plugin**

```bash
git add crates/codegraph-mcp/src/autoagents/tier_plugin.rs
git commit -m "feat: implement tier-aware prompt plugin with max iterations and tokens"
```

---

## Phase 4: Synchronous Tool Wrappers (8 hours)

### Task 6: Create Async-to-Sync GraphToolExecutor Wrapper

**Files:**
- Modify: `crates/codegraph-mcp/src/autoagents/tools/tool_executor_adapter.rs`

**Step 1: Implement synchronous wrapper**

```rust
// ABOUTME: Adapter for GraphToolExecutor integration with AutoAgents
// ABOUTME: Synchronous wrapper around async GraphToolExecutor for AutoAgents tools

use crate::graph_tool_executor::GraphToolExecutor;
use std::sync::Arc;
use serde_json::Value;

/// Synchronous wrapper around async GraphToolExecutor
///
/// AutoAgents tools must be synchronous, but GraphToolExecutor is async.
/// This wrapper uses tokio::runtime::Handle to bridge the gap.
pub struct GraphToolExecutorAdapter {
    executor: Arc<GraphToolExecutor>,
    runtime_handle: tokio::runtime::Handle,
}

impl GraphToolExecutorAdapter {
    pub fn new(executor: Arc<GraphToolExecutor>) -> Self {
        Self {
            executor,
            runtime_handle: tokio::runtime::Handle::current(),
        }
    }

    /// Execute a graph tool synchronously (blocks on async call)
    pub fn execute_sync(&self, function_name: &str, params: Value) -> Result<Value, String> {
        self.runtime_handle
            .block_on(self.executor.execute(function_name, params))
            .map_err(|e| e.to_string())
    }
}

/// Factory for creating AutoAgents tools with shared executor
pub struct GraphToolFactory {
    adapter: Arc<GraphToolExecutorAdapter>,
}

impl GraphToolFactory {
    pub fn new(executor: Arc<GraphToolExecutor>) -> Self {
        Self {
            adapter: Arc::new(GraphToolExecutorAdapter::new(executor)),
        }
    }

    pub fn adapter(&self) -> Arc<GraphToolExecutorAdapter> {
        self.adapter.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_adapter_struct_exists() {
        let _ = std::mem::size_of::<GraphToolExecutorAdapter>();
        let _ = std::mem::size_of::<GraphToolFactory>();
    }
}
```

**Step 2: Verify compilation**

Run: `cargo check -p codegraph-mcp --features "ai-enhanced,autoagents-experimental"`
Expected: Compiles successfully

**Step 3: Commit wrapper**

```bash
git add crates/codegraph-mcp/src/autoagents/tools/tool_executor_adapter.rs
git commit -m "feat: implement synchronous wrapper for GraphToolExecutor"
```

---

### Task 7: Implement First Tool (GetTransitiveDependencies)

**Files:**
- Modify: `crates/codegraph-mcp/src/autoagents/tools/graph_tools.rs`

**Step 1: Write tool with proper AutoAgents syntax**

```rust
// ABOUTME: AutoAgents tool definitions for SurrealDB graph analysis
// ABOUTME: Type-safe wrappers using AutoAgents derive macros

use autoagents::core::tool::{ToolCallError, ToolRuntime};
use autoagents_derive::{tool, ToolInput};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;
use crate::autoagents::tools::tool_executor_adapter::GraphToolExecutorAdapter;

/// Parameters for get_transitive_dependencies
#[derive(Serialize, Deserialize, ToolInput, Debug)]
pub struct GetTransitiveDependenciesArgs {
    #[input(description = "The ID of the code node to analyze (e.g., 'nodes:123')")]
    node_id: String,
    #[input(description = "Type of dependency relationship to follow (default: 'Calls')")]
    #[serde(default = "default_edge_type")]
    edge_type: String,
    #[input(description = "Maximum traversal depth (1-10, default: 3)")]
    #[serde(default = "default_depth")]
    depth: usize,
}

fn default_edge_type() -> String {
    "Calls".to_string()
}

fn default_depth() -> usize {
    3
}

/// Get transitive dependencies of a code node
#[tool(
    name = "get_transitive_dependencies",
    description = "Get all transitive dependencies of a code node up to specified depth. \
                   Follows dependency edges recursively to find all nodes this node depends on.",
    input = GetTransitiveDependenciesArgs,
)]
pub struct GetTransitiveDependencies {
    executor: Arc<GraphToolExecutorAdapter>,
}

impl GetTransitiveDependencies {
    pub fn new(executor: Arc<GraphToolExecutorAdapter>) -> Self {
        Self { executor }
    }
}

impl ToolRuntime for GetTransitiveDependencies {
    fn execute(&self, args: Value) -> Result<Value, ToolCallError> {
        let typed_args: GetTransitiveDependenciesArgs = serde_json::from_value(args)
            .map_err(|e| ToolCallError::InvalidInput(e.to_string()))?;

        let params = serde_json::json!({
            "node_id": typed_args.node_id,
            "edge_type": typed_args.edge_type,
            "depth": typed_args.depth,
        });

        self.executor
            .execute_sync("get_transitive_dependencies", params)
            .map_err(|e| ToolCallError::ExecutionError(e))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_args_deserialization() {
        let json = serde_json::json!({
            "node_id": "nodes:123",
            "edge_type": "Imports",
            "depth": 5
        });

        let args: GetTransitiveDependenciesArgs = serde_json::from_value(json).unwrap();
        assert_eq!(args.node_id, "nodes:123");
        assert_eq!(args.edge_type, "Imports");
        assert_eq!(args.depth, 5);
    }

    #[test]
    fn test_args_defaults() {
        let json = serde_json::json!({
            "node_id": "nodes:456"
        });

        let args: GetTransitiveDependenciesArgs = serde_json::from_value(json).unwrap();
        assert_eq!(args.edge_type, "Calls");
        assert_eq!(args.depth, 3);
    }
}
```

**Step 2: Run tests**

Run: `cargo test -p codegraph-mcp test_args --features "ai-enhanced,autoagents-experimental"`
Expected: PASS

**Step 3: Commit first tool**

```bash
git add crates/codegraph-mcp/src/autoagents/tools/graph_tools.rs
git commit -m "feat: implement GetTransitiveDependencies AutoAgents tool"
```

---

### Task 8: Implement Remaining 5 Graph Tools

**Files:**
- Modify: `crates/codegraph-mcp/src/autoagents/tools/graph_tools.rs`

**Step 1-5: Implement each tool following the same pattern**

Add these tools using the same structure as GetTransitiveDependencies:

1. **GetReverseDependencies** - Args: `node_id`, `edge_type`, `depth`
2. **TraceCallChain** - Args: `start_node_id`, `max_depth`, `include_indirect`
3. **DetectCircularDependencies** - Args: `edge_type`, `max_cycle_length`
4. **CalculateCouplingMetrics** - Args: `node_id`, `edge_type`
5. **GetHubNodes** - Args: `edge_type`, `min_connections`, `limit`

(Full code omitted for brevity - follow GetTransitiveDependencies pattern)

**Step 6: Verify all tools compile**

Run: `cargo check -p codegraph-mcp --features "ai-enhanced,autoagents-experimental"`
Expected: Compiles successfully

**Step 7: Commit all tools**

```bash
git add crates/codegraph-mcp/src/autoagents/tools/graph_tools.rs
git commit -m "feat: implement remaining 5 AutoAgents graph analysis tools"
```

---

## Phase 5: Agent Definition (4 hours)

### Task 9: Define CodeGraphAgent with Macro

**Files:**
- Create: `crates/codegraph-mcp/src/autoagents/codegraph_agent.rs`
- Modify: `crates/codegraph-mcp/src/autoagents/mod.rs`

**Step 1: Create agent definition file**

```rust
// ABOUTME: CodeGraph agent definition for AutoAgents ReAct workflow
// ABOUTME: Defines tools, output format, and behavior for graph analysis

use autoagents::core::agent::prebuilt::executor::ReActAgentOutput;
use autoagents_derive::{agent, AgentHooks, AgentOutput};
use serde::{Deserialize, Serialize};

use crate::autoagents::tools::graph_tools::*;

/// CodeGraph agent output format
#[derive(Debug, Serialize, Deserialize, AgentOutput)]
pub struct CodeGraphAgentOutput {
    #[output(description = "Final answer to the query")]
    answer: String,

    #[output(description = "Key findings from graph analysis")]
    findings: Vec<String>,

    #[output(description = "Number of analysis steps performed")]
    steps_taken: usize,
}

/// CodeGraph agent for code analysis via graph traversal
#[agent(
    name = "codegraph_agent",
    description = "You are a code analysis agent with access to graph database tools. \
                   Analyze code dependencies, call chains, and architectural patterns.",
    tools = [
        GetTransitiveDependencies,
        GetReverseDependencies,
        TraceCallChain,
        DetectCircularDependencies,
        CalculateCouplingMetrics,
        GetHubNodes
    ],
    output = CodeGraphAgentOutput,
)]
#[derive(Default, Clone, AgentHooks)]
pub struct CodeGraphAgent {}

impl From<ReActAgentOutput> for CodeGraphAgentOutput {
    fn from(output: ReActAgentOutput) -> Self {
        let resp = output.response;

        if output.done && !resp.trim().is_empty() {
            // Try to parse as structured JSON
            if let Ok(value) = serde_json::from_str::<CodeGraphAgentOutput>(&resp) {
                return value;
            }
        }

        // Fallback: create output from raw response
        CodeGraphAgentOutput {
            answer: resp,
            findings: vec![],
            steps_taken: 0,
        }
    }
}
```

**Step 2: Register in mod.rs**

```rust
// In crates/codegraph-mcp/src/autoagents/mod.rs
#[cfg(feature = "autoagents-experimental")]
pub mod codegraph_agent;

#[cfg(feature = "autoagents-experimental")]
pub use codegraph_agent::{CodeGraphAgent, CodeGraphAgentOutput};
```

**Step 3: Verify compilation**

Run: `cargo check -p codegraph-mcp --features "ai-enhanced,autoagents-experimental"`
Expected: Compiles successfully

**Step 4: Commit agent definition**

```bash
git add crates/codegraph-mcp/src/autoagents/codegraph_agent.rs crates/codegraph-mcp/src/autoagents/mod.rs
git commit -m "feat: define CodeGraphAgent with 6 graph analysis tools"
```

---

### Task 10: Implement Agent Builder

**Files:**
- Modify: `crates/codegraph-mcp/src/autoagents/agent_builder.rs`

**Step 1: Add builder implementation**

```rust
use crate::autoagents::codegraph_agent::CodeGraphAgent;
use crate::autoagents::tier_plugin::TierAwarePromptPlugin;
use crate::autoagents::tools::GraphToolFactory;
use crate::{AnalysisType, GraphToolExecutor};
use crate::context_aware_limits::ContextTier;

use autoagents::core::agent::{AgentBuilder, DirectAgent};
use autoagents::core::agent::prebuilt::executor::ReActAgent;
use autoagents::core::agent::memory::SlidingWindowMemory;
use autoagents::core::error::Error as AutoAgentsError;
use std::sync::Arc;

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
        // Get tier-aware configuration
        let tier_plugin = TierAwarePromptPlugin::new(self.analysis_type, self.tier);
        let system_prompt = tier_plugin
            .get_system_prompt()
            .map_err(|e| AutoAgentsError::Generic(e.to_string()))?;

        // Create memory (sliding window keeps last N messages)
        let memory_size = tier_plugin.get_max_iterations() * 2;
        let memory = Box::new(SlidingWindowMemory::new(memory_size));

        // Build agent
        let agent_handle = AgentBuilder::<_, DirectAgent>::new(
            ReActAgent::new(CodeGraphAgent::default())
        )
        .llm(self.llm_adapter)
        .memory(memory)
        .system_prompt(&system_prompt)
        .max_iterations(tier_plugin.get_max_iterations())
        .build()
        .await?;

        Ok(AgentHandle {
            agent: agent_handle.agent,
            tier: self.tier,
            analysis_type: self.analysis_type,
        })
    }
}

/// Handle for executing CodeGraph agent
pub struct AgentHandle {
    pub agent: DirectAgent<ReActAgent<CodeGraphAgent>>,
    pub tier: ContextTier,
    pub analysis_type: AnalysisType,
}
```

**Step 2: Verify compilation**

Run: `cargo check -p codegraph-mcp --features "ai-enhanced,autoagents-experimental"`
Expected: Compiles successfully

**Step 3: Commit agent builder**

```bash
git add crates/codegraph-mcp/src/autoagents/agent_builder.rs
git commit -m "feat: implement CodeGraphAgentBuilder with tier-aware configuration"
```

---

## Phase 6: Executor Wrapper (3 hours)

### Task 11: Implement Executor Wrapper

**Files:**
- Modify: `crates/codegraph-mcp/src/autoagents/executor.rs`

**Step 1: Implement executor**

```rust
// ABOUTME: AutoAgents executor wrapper for CodeGraph workflows
// ABOUTME: Converts AutoAgents results to CodeGraph AgenticResult format

use crate::{AgenticResult, ReasoningStep, AnalysisType};
use crate::context_aware_limits::ContextTier;
use crate::autoagents::agent_builder::AgentHandle;
use crate::autoagents::codegraph_agent::CodeGraphAgentOutput;
use crate::McpError;

use autoagents::core::agent::task::Task;
use std::time::Instant;

pub struct CodeGraphAgenticExecutor {
    agent_handle: AgentHandle,
}

impl CodeGraphAgenticExecutor {
    pub fn new(agent_handle: AgentHandle) -> Self {
        Self { agent_handle }
    }

    pub async fn execute(&self, query: &str) -> Result<AgenticResult, McpError> {
        let start = Instant::now();

        let task = Task::new(query);
        let output = self
            .agent_handle
            .agent
            .run(task)
            .await
            .map_err(|e| McpError::Protocol(format!("AutoAgents execution failed: {}", e)))?;

        // Convert to CodeGraphAgentOutput
        let typed_output: CodeGraphAgentOutput = output.into();

        // Build result (simplified - no step-by-step trace from ReAct)
        let result = AgenticResult {
            final_answer: typed_output.answer,
            steps: vec![], // TODO: Extract from ReAct trace if available
            total_steps: typed_output.steps_taken,
            duration_ms: start.elapsed().as_millis() as u64,
            total_tokens: 0, // TODO: Extract from LLM calls
            completed_successfully: true,
            termination_reason: "success".to_string(),
        };

        Ok(result)
    }
}
```

**Step 2: Verify compilation**

Run: `cargo check -p codegraph-mcp --features "ai-enhanced,autoagents-experimental"`
Expected: Compiles successfully

**Step 3: Commit executor**

```bash
git add crates/codegraph-mcp/src/autoagents/executor.rs
git commit -m "feat: implement AutoAgents executor wrapper with result conversion"
```

---

## Phase 7: MCP Server Integration (3 hours)

### Task 12: Add Feature Flag Toggle

**Files:**
- Modify: `crates/codegraph-mcp/src/official_server.rs`

**Step 1: Add AutoAgents execution function**

Add after existing `execute_agentic_workflow`:

```rust
#[cfg(feature = "autoagents-experimental")]
async fn execute_agentic_workflow_autoagents(
    &self,
    analysis_type: crate::AnalysisType,
    query: &str,
    _peer: Peer<RoleServer>,
    _meta: Meta,
) -> Result<CallToolResult, McpError> {
    use crate::autoagents::{CodeGraphAgentBuilder, CodeGraphAgenticExecutor};
    use codegraph_ai::llm_factory::LLMProviderFactory;

    // Detect tier
    let tier = Self::detect_context_tier();

    eprintln!("🤖 AutoAgents {} (tier={:?})", analysis_type.as_str(), tier);

    // Load config and create LLM provider
    let config_manager = codegraph_core::config_manager::ConfigManager::load()
        .map_err(|e| McpError::Protocol(format!("Config load failed: {}", e)))?;
    let config = config_manager.config();
    let llm_provider = LLMProviderFactory::create_from_config(&config.llm)
        .map_err(|e| McpError::Protocol(format!("LLM provider creation failed: {}", e)))?;

    // Get GraphFunctions
    let graph_functions = self.graph_functions.as_ref()
        .ok_or_else(|| McpError::Protocol("GraphFunctions not initialized".to_string()))?;

    let tool_executor = Arc::new(crate::GraphToolExecutor::new(graph_functions.clone()));

    // Build and execute agent
    let agent_handle = CodeGraphAgentBuilder::new(
        llm_provider,
        tool_executor,
        tier,
        analysis_type,
    )
    .build()
    .await
    .map_err(|e| McpError::Protocol(format!("Agent builder failed: {}", e)))?;

    let executor = CodeGraphAgenticExecutor::new(agent_handle);
    let result = executor
        .execute(query)
        .await?;

    // Format result
    let response_json = serde_json::json!({
        "analysis_type": analysis_type.as_str(),
        "tier": format!("{:?}", tier),
        "final_answer": result.final_answer,
        "total_steps": result.total_steps,
        "duration_ms": result.duration_ms,
        "completed_successfully": result.completed_successfully,
    });

    Ok(CallToolResult::success(vec![Content::text(
        serde_json::to_string_pretty(&response_json)
            .unwrap_or_else(|_| "Error formatting result".to_string()),
    )]))
}
```

**Step 2: Add runtime toggle**

Modify existing `execute_agentic_workflow`:

```rust
async fn execute_agentic_workflow(
    &self,
    analysis_type: crate::AnalysisType,
    query: &str,
    peer: Peer<RoleServer>,
    meta: Meta,
) -> Result<CallToolResult, McpError> {
    // Runtime toggle via environment variable
    #[cfg(feature = "autoagents-experimental")]
    if std::env::var("USE_AUTOAGENTS").is_ok() {
        return self.execute_agentic_workflow_autoagents(
            analysis_type,
            query,
            peer,
            meta,
        ).await;
    }

    // Legacy implementation continues...
    use crate::agentic_orchestrator::AgenticOrchestrator;
    // ... existing code ...
}
```

**Step 3: Verify compilation**

Run: `cargo check -p codegraph-mcp --features "ai-enhanced,autoagents-experimental"`
Expected: Compiles successfully

**Step 4: Commit MCP integration**

```bash
git add crates/codegraph-mcp/src/official_server.rs
git commit -m "feat: integrate AutoAgents with MCP server via USE_AUTOAGENTS flag"
```

---

## Phase 8: Testing & Documentation (5 hours)

### Task 13-18: Testing and Documentation

Follow original plan tasks 13-18 with no changes needed.

---

## Completion Checklist

- [ ] All 18 tasks completed
- [ ] ~18 commits made
- [ ] `cargo check --workspace --features "ai-enhanced,autoagents-experimental"` passes
- [ ] `cargo test --workspace --features "ai-enhanced,autoagents-experimental"` passes
- [ ] Legacy still works: `cargo test --workspace --features ai-enhanced`
- [ ] Documentation complete
- [ ] Feature flag working: `USE_AUTOAGENTS=1` selects AutoAgents
- [ ] All 7 agentic MCP tools working

## Effort Summary

| Phase | Tasks | Estimated Hours | Notes |
|-------|-------|----------------|-------|
| Foundation | 1-2 | ✅ Complete | Done in original plan |
| LLM Bridge | 3-4 | 6h | 4 traits + conversions |
| Configuration | 5 | 4h | Tier-aware prompts |
| Tools | 6-8 | 8h | Sync wrappers + 6 tools |
| Agent | 9-10 | 4h | Macro-based definition |
| Executor | 11 | 3h | Result conversion |
| Integration | 12 | 3h | MCP server toggle |
| Testing/Docs | 13-18 | 4h | Same as original |
| **Total** | | **32h** | Revised estimate |

---

**Notes:**
- This plan reflects the **real AutoAgents v0.2.4 API** discovered through research
- Code examples are accurate to actual AutoAgents syntax
- Effort estimates are realistic based on API complexity
- Each task follows TDD: Test → Fail → Implement → Pass → Commit
