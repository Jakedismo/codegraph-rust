# AutoAgents API Research Findings

**Date:** 2025-11-09
**Status:** Plan revision required before continuing implementation

## Summary

Implementation of plan `2025-11-09-autoagents-integration.md` paused after Task 2 due to significant differences between plan assumptions and actual AutoAgents v0.2.4 API.

## What Was Completed

✅ **Task 1: Add AutoAgents Dependency**
- Added `autoagents` git dependency to `Cargo.toml`
- Added `autoagents-experimental` feature flag
- Verified dependency resolves (v0.2.4 from https://github.com/liquidos-ai/AutoAgents)
- Committed: `668522f`

✅ **Task 2: Create AutoAgents Module Structure**
- Created `crates/codegraph-mcp/src/autoagents/` module with 8 files
- All files compile with placeholder content
- Registered in `lib.rs` with feature flag
- Committed: `6bab700`

## API Differences Discovered

### 1. LLM Provider Architecture

**Plan Assumption:**
```rust
// Plan assumed simple trait
trait LLM {
    async fn generate(&self, messages: Vec<Message>) -> Result<String, LLMError>;
}
```

**Actual AutoAgents API:**
```rust
// Composite trait requiring 4 sub-trait implementations
pub trait LLMProvider:
    ChatProvider +
    CompletionProvider +
    EmbeddingProvider +
    ModelsProvider +
    Send + Sync + 'static
{}

#[async_trait]
pub trait ChatProvider: Sync + Send {
    async fn chat(
        &self,
        messages: &[ChatMessage],
        tools: Option<&[Tool]>,
        json_schema: Option<StructuredOutputFormat>,
    ) -> Result<Box<dyn ChatResponse>, LLMError>;

    async fn chat_stream(...) -> Result<Pin<Box<dyn Stream<...>>>, LLMError>;
    async fn chat_stream_struct(...) -> Result<Pin<Box<dyn Stream<...>>>, LLMError>;
}
```

**Impact:**
- Need to implement 4 traits instead of 1
- Need `ChatMessage` ↔ `codegraph_ai::Message` conversion
- Need `ChatResponse` ↔ `codegraph_ai::LLMResponse` conversion
- ~300-400 lines vs plan's ~100 lines for LLM adapter

### 2. Agent Builder Pattern

**Plan Assumption:**
```rust
let agent = AgentBuilder::new(llm_adapter)
    .with_tier_config(...)
    .build();
```

**Actual AutoAgents API:**
```rust
let agent_handle = AgentBuilder::<_, DirectAgent>::new(ReActAgent::new(MyAgent {}))
    .llm(llm)
    .memory(sliding_window_memory)
    .build()
    .await?;

let result = agent_handle.agent.run(Task::new("query")).await?;
```

**Impact:**
- Need to wrap `ReActAgent::new(our_tools)`
- Need to provide memory (e.g., `SlidingWindowMemory`)
- `build()` is async and returns `Result<AgentHandle, Error>`
- Execution via `agent_handle.agent.run(Task)` not `executor.execute()`

### 3. Tool Definition

**Plan Assumption:**
```rust
#[derive(Tool)]
#[tool(name = "...", description = "...")]
pub struct MyTool {
    #[tool(skip)]
    executor: Arc<GraphToolExecutor>,
}

impl ToolExecutor for MyTool {
    async fn execute(&self, params: Self::Input) -> Result<Self::Output, ToolError>;
}
```

**Actual AutoAgents API:**
```rust
#[derive(Serialize, Deserialize, ToolInput, Debug)]
pub struct MyToolArgs {
    #[input(description = "...")]
    field: String,
}

#[tool(
    name = "my_tool",
    description = "...",
    input = MyToolArgs,
)]
struct MyTool {}

impl ToolRuntime for MyTool {
    fn execute(&self, args: Value) -> Result<Value, ToolCallError> {
        let typed_args: MyToolArgs = serde_json::from_value(args)?;
        // ... implementation
        Ok(result.into())
    }
}
```

**Impact:**
- Separate `ToolInput` derive for parameters
- Use `#[input(...)]` not `#[tool(...)]` for fields
- Implement `ToolRuntime` not `ToolExecutor`
- Takes/returns `serde_json::Value` not typed params
- Synchronous `fn execute` not `async fn`

### 4. Agent Definition with Tools

**Plan Assumption:**
Tools registered via factory in AgentBuilder

**Actual AutoAgents API:**
```rust
#[agent(
    name = "my_agent",
    description = "You are a ...",
    tools = [Tool1, Tool2, Tool3],  // Listed in macro
    output = MyAgentOutput,
)]
#[derive(Default, Clone, AgentHooks)]
pub struct MyAgent {}

#[derive(Debug, Serialize, Deserialize, AgentOutput)]
pub struct MyAgentOutput {
    #[output(description = "...")]
    field: String,
}
```

**Impact:**
- Tools listed in `#[agent(...)]` macro, not registered dynamically
- Need `AgentOutput` type with `#[output(...)]` fields
- Agent must `impl From<ReActAgentOutput> for MyAgentOutput`
- Less flexible than plan's factory pattern

### 5. Imports Structure

**Plan Assumption:**
```rust
use autoagents::prelude::*;
```

**Actual Structure:**
```rust
use autoagents::core::agent::prebuilt::executor::{ReActAgent, ReActAgentOutput};
use autoagents::core::agent::{AgentBuilder, AgentDeriveT, AgentOutputT, DirectAgent};
use autoagents::core::agent::memory::SlidingWindowMemory;
use autoagents::core::agent::task::Task;
use autoagents::core::tool::{ToolCallError, ToolInputT, ToolRuntime, ToolT};
use autoagents::core::error::Error;
use autoagents::llm::LLMProvider;
use autoagents_derive::{agent, tool, AgentHooks, AgentOutput, ToolInput};
```

**Impact:**
- No `prelude` module available
- Need specific imports from `core` and `llm` crates
- Derive macros from `autoagents_derive` crate

## CodeGraph Integration Challenges

### Current CodeGraph LLM Provider

```rust
#[async_trait]
pub trait LLMProvider: Send + Sync {
    async fn generate_chat(
        &self,
        messages: &[Message],
        config: &GenerationConfig,
    ) -> LLMResult<LLMResponse>;
}

pub struct Message {
    pub role: MessageRole,  // System, User, Assistant
    pub content: String,
}
```

### AutoAgents ChatMessage

Needs investigation - likely different structure with tool calls support.

### Conversion Required

1. `codegraph_ai::Message` → `autoagents::ChatMessage`
2. `autoagents::ChatResponse` → `codegraph_ai::LLMResponse`
3. `codegraph_ai::GenerationConfig` → AutoAgents params (temperature, max_tokens, etc.)
4. Error types: `LLMError` (AutoAgents) ↔ `codegraph_ai::LLMError`

## Recommended Plan Revisions

### Phase 2: LLM Adapter (Revised Estimate: 4-6 hours)

**Task 3A: Implement ChatProvider Bridge**
- Create adapter implementing `autoagents::llm::ChatProvider`
- Message conversion: `Message` ↔ `ChatMessage`
- Response conversion: `ChatResponse` → `LLMResponse`
- Error mapping

**Task 3B: Implement Stub Providers (Optional)**
- Minimal `CompletionProvider` impl (delegates to chat)
- Minimal `EmbeddingProvider` impl (not used)
- Minimal `ModelsProvider` impl (returns hardcoded)

**Task 3C: Full LLMProvider Implementation**
- Composite struct satisfying all 4 traits
- Test compilation and basic functionality

### Phase 4: Graph Tools (Revised Estimate: 6-8 hours)

**Changes:**
- Tools must be synchronous `fn execute`, not async
- Need separate `ToolInput` structs with `#[input(...)]` attrs
- Tools take/return `serde_json::Value`
- Can't use Arc<GraphToolExecutor> directly - need wrapper

**New Approach:**
1. Create sync wrapper around async `GraphToolExecutor`
2. Use `tokio::runtime::Handle::current().block_on()` for async calls
3. Or restructure to make graph functions synchronous

### Phase 6: Agent Builder (Revised Estimate: 3-4 hours)

**Changes:**
- Define `CodeGraphAgent` struct with `#[agent(...)]` macro
- List all 6 tools in macro
- Define `CodeGraphAgentOutput` with `#[output(...)]` fields
- Implement `From<ReActAgentOutput>`
- Build with `AgentBuilder::<_, DirectAgent>::new(ReActAgent::new(CodeGraphAgent {}))`

## Effort Re-estimation

| Phase | Original Estimate | Revised Estimate | Reason |
|-------|------------------|------------------|--------|
| Phase 1 | 2h | ✅ 2h | Completed as planned |
| Phase 2 | 3h | **6h** | 4 traits vs 1, complex conversions |
| Phase 3 | 2h | ⚠️ **4h** | Different prompt injection approach |
| Phase 4 | 4h | **8h** | Sync tools, macro-based, wrapper needed |
| Phase 5 | 2h | **3h** | More complex observer API |
| Phase 6 | 2h | **4h** | Macro-based agent definition |
| Phase 7-10 | 3h | **5h** | Integration more complex |
| **Total** | **18h** | **32h** | +14 hours (78% increase) |

## Next Steps

1. **Update Implementation Plan**
   - Revise tasks 3-18 with correct API
   - Add sub-tasks for complex conversions
   - Update code examples with real AutoAgents syntax

2. **Create API Reference Document**
   - Document AutoAgents → CodeGraph mappings
   - Include conversion helpers
   - Example snippets for each pattern

3. **Decision Point: Proceed or Pivot?**
   - **Continue**: 32 hours implementation with revised plan
   - **Pivot**: Evaluate simpler agent frameworks
   - **Hybrid**: Use AutoAgents only for ReAct, keep custom orchestrator

## Files Modified

```
crates/codegraph-mcp/Cargo.toml        (+ autoagents dep, feature flag)
crates/codegraph-mcp/src/lib.rs        (+ autoagents module)
crates/codegraph-mcp/src/autoagents/   (new module, 8 files, placeholders)
```

## References

- AutoAgents Docs: Context7 `/liquidos-ai/autoagents` (54 snippets)
- DeepWiki: https://github.com/liquidos-ai/AutoAgents
- ChatProvider trait: `crates/llm/src/chat/mod.rs`
- Tool examples: README.md math agent
- Agent patterns: `examples/design_patterns/`
