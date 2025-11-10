# Rust Agent Framework Analysis for CodeGraph

## Executive Summary

CodeGraph currently has a **custom-built agentic orchestrator** with ~627 lines of sophisticated reasoning and tool-calling logic. This analysis evaluates 8+ production-ready Rust frameworks that could replace or significantly simplify this implementation while maintaining or improving capabilities.

**Key Finding**: Multiple mature frameworks exist (Rig, Kowalski, AutoAgents, axiom-ai-agents), but the choice depends on your priorities: ecosystem maturity (Rig), comprehensive feature set (AutoAgents), modularity (Kowalski), or production-ready completeness (axiom-ai-agents).

---

## Part 1: Current Implementation Complexity Analysis

### Agentic Orchestrator Architecture (627 lines)

**Current File**: `crates/codegraph-mcp/src/agentic_orchestrator.rs`

#### What's Implemented

1. **Tier-Aware Configuration** (60 lines)
   - 4-tier context window detection (Small/Medium/Large/Massive)
   - Automatic max_steps adjustment (5-20 steps)
   - Dynamic token budgeting per tier

2. **Conversation State Management** (200+ lines)
   - Message history tracking with Message/MessageRole types
   - Tool call logging and tracing
   - Multi-step reasoning capture in ReasoningStep structs

3. **LLM Integration & Tool Calling** (250+ lines)
   - Generic LLMProvider trait abstraction supporting multiple providers
   - JSON-based tool call parsing from LLM responses
   - Tool result injection back into conversation
   - Error handling for malformed responses

4. **Workflow Control** (150+ lines)
   - Step-by-step execution loop with max iterations
   - Timeout enforcement (300 second max)
   - Progress callbacks for streaming/notification
   - Termination reason tracking (success, max_steps, timeout)

5. **System Prompts** (358 lines in agentic_api_surface_prompts.rs)
   - Tier-specific prompts (TERSE, BALANCED, DETAILED, EXPLORATORY)
   - Tool schema injection
   - Context-aware guidance

#### Supporting Infrastructure

- **Tool Execution**: GraphToolExecutor with LRU caching (100 lines)
- **Tool Schemas**: JSON schema definitions for 6 graph analysis tools
- **Error Handling**: McpError custom error types
- **Metrics**: Tool call statistics, token tracking

#### Complexity Hotspots

1. **Prompt Management**: 7 different agentic workflows × 4 tier variants = 28 prompt versions manually maintained
2. **State Machine**: Manual step-by-step iteration with tool result injection
3. **Tool Call Parsing**: Hand-rolled JSON parsing for tool calls
4. **Tier Detection**: Hardcoded tier boundaries and token limits
5. **Progress Notifications**: Manual progress tracking for async operations

**Total Custom Agentic Code**: ~1,200 lines (orchestrator + prompts + supporting code)

---

## Part 2: Viable Rust Agent Frameworks

### 1. **Rig** ⭐ Most Mature
**GitHub**: https://github.com/0xPlaygrounds/rig  
**Crates**: rig-core, rig-bedrock, rig-postgres, rig-s3vectors, etc.  
**Current Version**: 0.23.1  
**Downloads**: 137,882+ all-time  
**Last Update**: Active 2024-2025

#### Features
- **20+ LLM providers** (OpenAI, Anthropic, Ollama, Bedrock, etc.)
- **10+ vector store integrations** (MongoDB, LanceDB, Neo4j, Qdrant, SQLite)
- **Tool calling** via Tool trait (requires definition() + call() methods)
- **Multi-turn conversation** support
- **Streaming responses** with OpenTelemetry integration
- **WASM compatible** core library

#### Strengths
- Most documented and battle-tested framework
- Active community and regular updates
- Comprehensive provider ecosystem
- Used in production (St Jude, Coral Protocol, Nethermind)
- Excellent ergonomics with builder pattern

#### Limitations
- Tool calling mechanics less explicit than some alternatives
- Breaking changes expected as library evolves
- Documentation for advanced agentic patterns could be deeper

#### Fit for CodeGraph
**Good**: Provider flexibility, chat history management  
**Gap**: Less explicit agent reasoning loop, smaller agentic reasoning examples

---

### 2. **AutoAgents** ⭐ Most Feature-Rich
**GitHub**: https://github.com/liquidos-ai/AutoAgents  
**Current Version**: Active development  
**Use Cases**: Multi-agent orchestration, cloud-to-edge deployment

#### Features
- **ReAct Executors** (Reasoning + Acting) - perfect for multi-step analysis
- **Streaming support** with type-safe JSON schema validation
- **Custom tool derivation** via macros (auto-generates JSON schemas)
- **WASM sandboxing** for safe tool execution
- **Ractor-based** async architecture
- **Configurable memory** backends (sliding window, persistent storage)
- **Multi-agent orchestration** with pub/sub communication

#### Strengths
- Purpose-built for agentic reasoning (ReAct pattern native)
- Explicit tool calling with macro-generated schemas
- Type-safe throughout (compile-time guarantees)
- Sandbox execution for untrusted tools
- Best for multi-agent coordination

#### Limitations
- Newer framework (less battle-tested than Rig)
- Documentation still evolving
- Ractor dependency (opinionated architecture)

#### Fit for CodeGraph
**Excellent**: ReAct pattern matches your step-by-step reasoning  
**Perfect**: Multi-tool orchestration with safety guarantees  
**Good**: Automatic schema generation for graph analysis tools

---

### 3. **Kowalski** ⭐ Most Modular
**GitHub**: https://github.com/yarenty/kowalski  
**Crates**: kowalski-core, kowalski-tools, kowalski-federation, +5 specialized agents  
**Current Version**: 0.5.0 (2 months ago)  
**Downloads**: kowalski-code-agent: 373+

#### Features
- **Modular workspace** - use only what you need
- **Domain-specific agents** (academic, code, data, web)
- **Pluggable tool architecture** (CSV, code analysis, PDF, web)
- **LLM provider flexibility** (Ollama, OpenAI, etc.)
- **Federation layer** for multi-agent coordination
- **Local-first** with zero Python dependencies

#### Strengths
- Excellent modularity - pick and compose what you need
- Specialized tools for different domains
- Clean separation of concerns
- Recent active development with v0.5.0 refactoring
- Good for domain-specific agents

#### Limitations
- Newer ecosystem (smaller community than Rig)
- Documentation spread across multiple crates
- Still evolving API

#### Fit for CodeGraph
**Good**: Modular approach aligns with your architecture  
**Good**: Code analysis agent could be extended  
**Gap**: Heavy focus on specific domains vs. general agentic reasoning

---

### 4. **axiom-ai-agents** ⭐ Most Production-Ready
**Crates**: axiom-core, axiom-llm, axiom-agents, axiom-rag, axiom-wasm  
**Ecosystem**: Complete framework with built-in tools

#### Features
- **Streaming-first architecture** from the ground up
- **Complete agent system** with tools, memory, execution planning
- **WASM sandboxing** for safe execution
- **Production monitoring** (retries, safety guards, observability)
- **High-performance RAG** with vector stores
- **Built-in tools** (calculator, weather, search, etc.)
- **Vendor-agnostic** LLM interface

#### Strengths
- Marketed as "production-ready LangChain alternative"
- Comprehensive feature set (no gaps)
- WASM sandboxing for tool safety
- Built from ground up for streaming
- Zero-cost abstractions (Rust performance)

#### Limitations
- Newest framework (least battle-tested)
- Smaller community
- Documentation still being built out

#### Fit for CodeGraph
**Excellent**: Complete feature set with no gaps  
**Good**: Tool sandboxing could protect against malicious analyses  
**Gap**: Less specific examples for agentic code analysis

---

### 5. **reagent-rs**
**GitHub**: https://github.com/VakeDomen/Reagent  
**Status**: Experimental/Emerging

#### Features
- **Builder pattern** for agent configuration
- **Provider abstraction** (Ollama, OpenRouter)
- **Structured output** via JSON Schema
- **Tool integration** with MCP support
- **Event streaming** for agent events

#### Limitations
- Experimental stage
- Minimal documentation
- Smaller ecosystem

---

### 6. **llm-chain**
**GitHub**: https://github.com/sobelio/llm-chain  
**Status**: Mature but slower updates

#### Features
- **Prompt templating** system
- **Chain composition** for multi-step workflows
- **Multiple provider support**
- **Vector store integration**
- **Tool/bash/python execution**

#### Limitations
- Updates have slowed
- Less focused on agents specifically
- More for prompt chaining than agentic reasoning

---

## Part 3: Feature Comparison Matrix

| Feature | CodeGraph Current | Rig | AutoAgents | Kowalski | axiom-ai-agents | reagent-rs |
|---------|------------------|-----|-----------|----------|-----------------|-----------|
| **Multi-step reasoning** | ✅ Custom | ✅ Via loops | ✅✅ ReAct native | ✅ Supported | ✅ Complete | ✅ Supported |
| **Tool calling** | ✅ JSON parsing | ✅ Tool trait | ✅✅ Macro-driven | ✅ Plugin arch | ✅ Complete | ✅ MCP+custom |
| **Conversation history** | ✅ Manual | ✅ Built-in | ✅ Built-in | ✅ Built-in | ✅ Built-in | ✅ Built-in |
| **Tier-aware prompting** | ✅ 4 tiers | ⚠️ Limited | ⚠️ Limited | ⚠️ Limited | ⚠️ Limited | ⚠️ Limited |
| **LLM provider flexibility** | ✅ 5 providers | ✅✅ 20+ | ✅ Multiple | ✅ Multiple | ✅ Multiple | ✅ 2+ |
| **Streaming support** | ⚠️ Manual | ✅ Built-in | ✅ Built-in | ✅ Built-in | ✅✅ Stream-first | ✅ Built-in |
| **Progress notifications** | ✅ Custom callback | ⚠️ Not explicit | ⚠️ Not explicit | ⚠️ Not explicit | ⚠️ Not explicit | ⚠️ Not explicit |
| **Token tracking** | ✅ Full | ✅ Via response | ✅ Via response | ✅ Via response | ✅ Via response | ✅ Via response |
| **Tool result caching** | ✅ LRU cache | ⚠️ Not built-in | ⚠️ Not built-in | ⚠️ Not built-in | ⚠️ Not built-in | ⚠️ Not built-in |
| **Vector store integration** | ⚠️ Separate | ✅✅ 10+ | ⚠️ Basic | ⚠️ Limited | ✅✅ Full RAG | ⚠️ Limited |
| **Multi-agent orchestration** | ❌ N/A | ⚠️ Basic | ✅✅ Complete | ✅✅ Complete | ✅ Complete | ⚠️ Limited |
| **WASM support** | ❌ N/A | ✅ Core only | ✅ Full | ⚠️ Basic | ✅✅ Full + sandbox | ⚠️ Limited |
| **Memory backends** | ❌ N/A | ⚠️ Via vectors | ✅ Configurable | ✅ Built-in | ✅ Complete | ⚠️ Limited |
| **Production maturity** | ✅ Proven | ✅✅ Highest | ✅ High | ✅ Medium-High | ✅ Medium | ⚠️ Experimental |
| **Community size** | 🔵 Internal | 🟢 Large | 🟡 Growing | 🟡 Growing | 🟡 Growing | 🔴 Small |
| **Documentation** | ✅ Internal | ✅✅ Excellent | ✅ Good | ⚠️ Scattered | ⚠️ Evolving | ⚠️ Minimal |
| **Code maturity** | ✅ 627 lines | ✅✅ Thousands | ✅ Active | ✅ Active | ✅ Complete | ⚠️ WIP |

---

## Part 4: Recommendation & Implementation Path

### Recommended Primary Choice: **AutoAgents**

**Why AutoAgents Wins for CodeGraph:**

1. **ReAct Pattern Native**: Your exact workflow is the canonical use case
   - Step 1: Reason ("I need to analyze dependencies")
   - Step 2: Act (call tool)
   - Step 3: Observe (get result)
   - Repeat until answer found

2. **Type-Safe Tool Definition**: Macro-based automatic schema generation
   - Replace 252 lines of manual JSON schema code
   - Compile-time verification of tool contracts
   - Zero runtime schema mismatch bugs

3. **Explicit Agentic Reasoning**: Built-in abstractions
   - Eliminates 627 lines of custom orchestration
   - Step tracking and termination logic
   - Progress tracking native to framework

4. **Multi-Agent Orchestration Ready**: Future-proof for federation
   - pub/sub communication for agent coordination
   - Type-safe message passing
   - Ractor for async safety

5. **Sandbox Execution**: Tool safety without external process overhead
   - WASM-based sandboxing for untrusted analyses
   - Resource limits built-in
   - Better than GraphToolExecutor's simple execution

### Secondary Recommendation: **Rig** (if you prefer established ecosystem)

**Why Rig is the safe choice:**
- Highest production maturity (St Jude, major companies)
- Most documentation and examples
- Largest provider ecosystem
- Less risk of breaking changes

**But requires:**
- Manual step-by-step loop implementation (AutoAgents does this for you)
- Manual tool calling orchestration
- More integration code

### Architecture Migration Path

#### Phase 1: Evaluate (1 week)
1. Clone AutoAgents repo, study ReAct executor example
2. Create minimal proof-of-concept for agentic_code_search tool
3. Compare code complexity vs current implementation
4. Test with real CodeGraph queries

#### Phase 2: Prototype (2-3 weeks)
1. Create new feature branch `feature/autoagents-migration`
2. Add AutoAgents to Cargo.toml alongside current code
3. Implement 1-2 agentic tools using AutoAgents
4. Run comparison tests (correctness, performance)
5. Measure metrics: code lines reduced, latency, token usage

#### Phase 3: Gradual Migration (3-4 weeks)
1. Migrate tools one-by-one with feature flags
2. Keep current implementation as fallback during transition
3. Run parallel tests in agentic_tools.py
4. Collect performance data

#### Phase 4: Consolidation (1-2 weeks)
1. Remove old code after all tools migrated
2. Update documentation and examples
3. Optimize memory usage with AutoAgents patterns
4. Deploy and monitor

### Estimated Simplifications

| Component | Current LOC | With Framework | Reduction |
|-----------|-----------|-----------------|-----------|
| Orchestration loop | 150 | 0 | -150 |
| Tool schema definitions | 252 | ~20 (macros) | -232 |
| Message handling | 100 | 0 | -100 |
| Tier configuration | 60 | ~30 (simpler) | -30 |
| Error handling | 65 | 10 | -55 |
| **Total** | **627+** | **~60** | **-567 lines** |

**Plus eliminate**: agentic_api_surface_prompts.rs (358 lines)  
**Total reduction**: ~925 lines of maintenance-heavy code

---

## Part 5: Risk Analysis & Mitigation

### AutoAgents Specific Risks

| Risk | Likelihood | Mitigation |
|------|-----------|-----------|
| Framework changes (newer) | Medium | Keep custom fallback during transition, pin versions |
| Ractor learning curve | Low | Good examples available, well-designed API |
| Smaller ecosystem | Low | Framework is feature-complete for your use case |
| WASM sandboxing overhead | Low | Benchmark before full adoption, keep direct mode option |

### Rig Specific Risks

| Risk | Likelihood | Mitigation |
|------|-----------|-----------|
| Manual orchestration overhead | Medium | Accept more code, but more battle-tested |
| Tool calling less explicit | Medium | Well-documented patterns available |
| Streaming callback changes | Low | Good examples in their docs |

---

## Part 6: Quick Start Code Comparison

### Current CodeGraph (627 lines)

```rust
// Manual state management
let mut steps = Vec::new();
let mut conversation_history = build_initial_messages();
for step_number in 1..=max_steps {
    let llm_response = llm.generate_chat(&conversation_history).await?;
    let step = parse_llm_response(&llm_response)?;
    conversation_history.push(assistant_message);
    if let Some(tool_name) = step.tool_name {
        let result = tool_executor.execute(tool_name, params).await?;
        conversation_history.push(tool_result_message);
    }
    steps.push(step);
    if step.is_final { break; }
}
```

### With AutoAgents (~50 lines)

```rust
// AutoAgents handles all state, step tracking, tool orchestration
let agent = AgentBuilder::new(llm)
    .add_tool(CodeSearchTool)
    .add_tool(DependencyAnalysisTool)
    .build();

let executor = ReActExecutor::new(agent);
let result = executor.execute(user_query, max_steps).await?;

// Result includes step trace, tool calls, final answer
println!("{:?}", result.steps);  // Full reasoning trace
println!("{:?}", result.final_answer);
```

---

## Conclusion

**Do Not** build more custom agentic code. Replace the 627-line orchestrator with an established framework.

**Recommended Path**:
1. **First choice**: Adopt AutoAgents (best fit for ReAct/multi-step reasoning)
2. **Alternative**: Use Rig (if you want more proven ecosystem)
3. **Not recommended**: Continue custom implementation (high maintenance, proven frameworks exist)

**Expected Outcome**:
- 900+ fewer lines of code
- Better tool safety guarantees (WASM sandboxing)
- Faster feature development (less boilerplate)
- More robust error handling (battle-tested)
- Production-ready agentic reasoning (no custom bugs)

The Rust AI/LLM agent ecosystem has matured enough that building custom orchestrators is no longer justified—even for specialized use cases like CodeGraph.

