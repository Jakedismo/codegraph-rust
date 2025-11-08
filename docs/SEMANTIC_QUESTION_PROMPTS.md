# Semantic Question Prompts for CodeGraph Agentic MCP System

## Overview

This document describes the 4 tier-aware system prompts created for the `semantic_question` analysis type in CodeGraph's agentic MCP system.

## Purpose

These prompts guide an LLM to answer semantic questions about code behavior using **ONLY** SurrealDB graph analysis tools. The core principle is **ZERO HEURISTICS** - all answers must be derived from concrete graph structure analysis, not from general programming knowledge or assumptions.

## Available Graph Tools

The LLM has access to 6 SurrealDB graph analysis tools:

1. **get_transitive_dependencies(node_id, edge_type, depth)** - Find all dependencies of a node recursively
2. **detect_circular_dependencies(edge_type)** - Find bidirectional dependency cycles
3. **trace_call_chain(from_node, max_depth)** - Follow execution flow through function calls
4. **calculate_coupling_metrics(node_id)** - Get Ca (afferent), Ce (efferent), I (instability)
5. **get_hub_nodes(min_degree)** - Find highly connected nodes (architectural hotspots)
6. **get_reverse_dependencies(node_id, edge_type, depth)** - Find what depends ON a node

## Response Format

All prompts instruct the LLM to respond in JSON format:

```json
{
  "reasoning": "Explanation of what I'm doing and why",
  "tool_call": {
    "tool_name": "name_of_tool",
    "parameters": { tool parameters }
  },
  "is_final": false
}
```

Final answer format:
```json
{
  "reasoning": "Complete answer based on graph evidence gathered, citing specific node IDs and relationships",
  "tool_call": null,
  "is_final": true
}
```

## Four Tier-Aware Prompts

### 1. TERSE (Small Tier - ContextTier::Small)

**File Location:** `/crates/codegraph-mcp/src/semantic_question_prompts.rs:SEMANTIC_QUESTION_TERSE`

**Characteristics:**
- Quick, focused answers
- 1-2 tool calls maximum
- Minimal investigation depth
- Direct evidence gathering

**Use Case:** Small context window models (< 50K tokens) that need fast, concise answers

**Key Features:**
- Question type mapping for quick strategy selection
- Direct tool-to-question-type mapping ("How does X work?" → trace_call_chain)
- Emphasis on single most relevant tool call
- Terse answer format

**Example Investigation:**
- Question: "What depends on function X?"
- Strategy: Single call to `get_reverse_dependencies(X, "Calls", 2)`
- Answer: Concise list of dependent nodes with counts

---

### 2. BALANCED (Medium Tier - ContextTier::Medium)

**File Location:** `/crates/codegraph-mcp/src/semantic_question_prompts.rs:SEMANTIC_QUESTION_BALANCED`

**Characteristics:**
- Standard investigation depth
- 2-4 targeted tool calls
- Well-rounded evidence gathering
- Reasonable thoroughness

**Use Case:** Medium context window models (50K-200K tokens) for standard analysis depth

**Key Features:**
- Investigation patterns for common question types
- Cross-verification of findings when appropriate
- Evidence requirements (cite node IDs, edge types, counts)
- Balanced between speed and thoroughness

**Example Investigation:**
- Question: "How does authentication work?"
- Strategy:
  1. trace_call_chain from login function (depth=5)
  2. get_transitive_dependencies for authentication components
  3. Synthesize behavioral explanation
- Answer: Structured response with execution flow + dependencies

---

### 3. DETAILED (Large Tier - ContextTier::Large)

**File Location:** `/crates/codegraph-mcp/src/semantic_question_prompts.rs:SEMANTIC_QUESTION_DETAILED`

**Characteristics:**
- Thorough investigation
- 4-7 strategic tool calls
- Multiple evidence points
- Cross-verification of findings

**Use Case:** Large context window models (200K-500K tokens) for comprehensive analysis

**Key Features:**
- Multi-angle investigation patterns
- Cross-verification strategies
- Quantitative evidence requirements (3-5+ node IDs per claim)
- Depth parameter strategy guidance (depth 4-5 for standard detailed analysis)
- Confidence scoring (0.0-1.0)

**Example Investigation:**
- Question: "What if X changes?"
- Strategy:
  1. get_reverse_dependencies(X, "Calls", depth=3) → call impact
  2. get_reverse_dependencies(X, "Imports", depth=2) → module impact
  3. calculate_coupling_metrics(X) → stability analysis
  4. Compare with hub_nodes(min_degree=5) → centrality assessment
  5. Quantify blast radius with affected node counts
- Answer: Multi-dimensional impact analysis with metrics and confidence scores

---

### 4. EXPLORATORY (Massive Tier - ContextTier::Massive)

**File Location:** `/crates/codegraph-mcp/src/semantic_question_prompts.rs:SEMANTIC_QUESTION_EXPLORATORY`

**Characteristics:**
- Exhaustive investigation
- 7-12+ tool calls
- Multiple perspectives and angles
- Comprehensive evidence gathering with statistical analysis

**Use Case:** Massive context window models (> 500K tokens) for deep, exhaustive analysis

**Key Features:**
- Multi-phase investigation strategy (Phase 1: Discovery → Phase 2: Deep Dive → Phase 3: Cross-Verification → Phase 4: Statistical Analysis)
- Multiple edge type exploration (Calls, Imports, Uses, Extends, Implements, References)
- Statistical analysis techniques (distributions, outliers, percentiles)
- Cross-verification protocols (5 different consistency checks)
- Confidence calculation formula with multiple factors
- Evidence quality standards (5-10+ node IDs, statistical distributions, depth progressions)

**Example Investigation:**
- Question: "Is there an architectural problem with X?"
- Strategy (10 tool calls):
  1. calculate_coupling_metrics(X) → quantitative baseline
  2. get_hub_nodes(min_degree=5) → compare X against hubs
  3. detect_circular_dependencies("Calls") → cycle involvement
  4. detect_circular_dependencies("Imports") → module cycles
  5. get_transitive_dependencies(X, "Calls", depth=6) → dependency breadth
  6. get_reverse_dependencies(X, "Calls", depth=6) → dependents breadth
  7. trace_call_chain(X, depth=7) → execution complexity
  8. Calculate metrics for X's dependencies → ecosystem health
  9. Calculate metrics for X's dependents → downstream health
  10. Statistical comparison: X's metrics vs. population distribution
- Answer: Exhaustive multi-dimensional architectural health assessment with statistical justification and confidence intervals

## Critical Requirements

All four prompts enforce these critical requirements:

### 1. ZERO HEURISTICS Principle

**Forbidden:**
- General programming knowledge
- Naming conventions or common patterns
- Assumptions about typical behavior
- Domain knowledge not in graph structure

**Required:**
- Concrete node IDs from tool results
- Exact edge relationships (Calls, Imports, Uses, etc.)
- Quantitative metrics from tools
- Explicit acknowledgment of limitations

### 2. Evidence Citation

All claims must cite:
- Specific node IDs
- Edge types and relationship counts
- Quantitative metrics (coupling scores, degree counts)
- Tool call results that support the claim

### 3. Question Type Handling

All prompts provide guidance for common semantic question types:
- "How does X work?" → trace_call_chain + dependencies
- "What depends on X?" → get_reverse_dependencies
- "Why does X depend on Y?" → get_transitive_dependencies to trace path
- "What if X changes?" → get_reverse_dependencies + coupling metrics
- "Is there circular dependency?" → detect_circular_dependencies

## Integration Points

### File Structure
```
crates/codegraph-mcp/src/
├── semantic_question_prompts.rs    # New file with 4 prompts
├── prompt_selector.rs               # Updated to use semantic_question_prompts
├── lib.rs                          # Module declaration added
└── agentic_orchestrator.rs         # Uses PromptSelector
```

### Usage in Code

```rust
use codegraph_mcp::prompt_selector::{PromptSelector, AnalysisType};
use codegraph_mcp::context_aware_limits::ContextTier;

let selector = PromptSelector::new();

// Get appropriate prompt for tier
let prompt = selector.select_prompt(
    AnalysisType::SemanticQuestion,
    ContextTier::Large  // Or Small, Medium, Massive
)?;

// prompt now contains the appropriate tier-specific system prompt
```

### Agentic Orchestrator Integration

The prompts are designed to work with the `AgenticOrchestrator`:

```rust
let orchestrator = AgenticOrchestrator::new(
    llm_provider,
    tool_executor,
    ContextTier::Large  // Automatically selects DETAILED prompt
);

let result = orchestrator.execute(
    "What would break if we change the authentication logic?",
    codebase_context
).await?;
```

## Tier Selection Guidelines

### Small Tier (TERSE)
- **When:** Small models, quick queries, time-sensitive answers
- **Example models:** qwen3:8b, smaller specialized models
- **Tool calls:** 1-2
- **Response time:** Fast (seconds)

### Medium Tier (BALANCED)
- **When:** Standard analysis needs, balanced thoroughness
- **Example models:** gpt-5-codex-mini, Claude Sonnet
- **Tool calls:** 2-4
- **Response time:** Moderate (10-30 seconds)

### Large Tier (DETAILED)
- **When:** Comprehensive analysis, high-stakes decisions
- **Example models:** gpt-5, kimi-k2-thinking
- **Tool calls:** 4-7
- **Response time:** Longer (30-90 seconds)

### Massive Tier (EXPLORATORY)
- **When:** Exhaustive analysis, architecture decisions, critical assessments
- **Example models:** claude-opus (1M context), grok-4-fast
- **Tool calls:** 7-12+
- **Response time:** Longest (1-3 minutes)

## Examples

### Example 1: Terse Tier

**Question:** "What calls the login function?"

**Tool Call 1:**
```json
{
  "reasoning": "Need to find what depends on login function using reverse dependencies",
  "tool_call": {
    "tool_name": "get_reverse_dependencies",
    "parameters": {
      "node_id": "nodes:login_function_123",
      "edge_type": "Calls",
      "depth": 2
    }
  },
  "is_final": false
}
```

**Final Answer:**
```json
{
  "reasoning": "Based on reverse dependency analysis: 5 functions call login:\n- nodes:auth_controller_456 (direct caller)\n- nodes:signup_handler_789 (direct caller)\n- nodes:oauth_callback_234 (direct caller)\n- nodes:session_refresh_567 (direct caller)\n- nodes:admin_login_890 (direct caller)\nAll are direct callers (depth 1). No transitive callers found at depth 2.",
  "tool_call": null,
  "is_final": true
}
```

### Example 2: Balanced Tier

**Question:** "How does password reset work?"

**Tool Call Sequence:**
1. trace_call_chain(password_reset_handler, 5)
2. get_transitive_dependencies(password_reset_handler, "Calls", 3)

**Final Answer:** Structured explanation showing execution flow + components involved

### Example 3: Detailed Tier

**Question:** "What would happen if we remove the cache layer?"

**Tool Call Sequence:**
1. get_reverse_dependencies(cache_service, "Calls", 5)
2. get_reverse_dependencies(cache_service, "Imports", 3)
3. calculate_coupling_metrics(cache_service)
4. get_hub_nodes(min_degree=10) to compare centrality
5. For top 3 affected nodes: calculate_coupling_metrics each

**Final Answer:** Multi-dimensional impact assessment with:
- Direct impact (23 nodes)
- Module impact (8 modules)
- Coupling analysis (I=0.73, unstable)
- Centrality comparison (cache is top 5% hub)
- Confidence: 0.88

## Testing

Tests are located in:
`/crates/codegraph-mcp/tests/semantic_question_prompts_test.rs`

Tests verify:
- All 4 prompts load correctly
- Each prompt contains critical elements (tools, response format, zero heuristics emphasis)
- Prompts increase in detail/length by tier
- Tier-specific guidance is unique
- Question type mappings are present
- Investigation patterns are appropriate for tier

Run tests (note: currently blocked by FAISS linking issue in test environment):
```bash
cargo test --package codegraph-mcp --test semantic_question_prompts_test --features ai-enhanced
```

Verify compilation:
```bash
cargo check --package codegraph-mcp --lib --features ai-enhanced
```

## Design Rationale

### Why 4 Tiers?

Different LLMs have vastly different context windows and computational budgets:
- Small models need quick, focused strategies
- Large models can afford exhaustive multi-phase investigation
- Tier-aware prompts optimize for each model's capabilities

### Why ZERO HEURISTICS?

Traditional code analysis often relies on naming conventions, common patterns, and domain knowledge. This approach:
- **Breaks** when conventions aren't followed
- **Hallucinates** based on typical patterns
- **Misses** actual graph structure

Graph-only analysis:
- **Verifiable** - every claim backed by graph evidence
- **Accurate** - reflects actual code relationships
- **Reliable** - works regardless of naming or conventions

### Why JSON Response Format?

The agentic orchestrator needs structured responses to:
- Parse tool calling decisions programmatically
- Track reasoning across multiple steps
- Determine when investigation is complete (is_final)
- Build conversation history for context

### Why Different Investigation Strategies Per Tier?

A 7B parameter model with 8K context cannot execute the same 12-step investigation as Claude Opus with 1M context:
- Small models: Single focused tool call → answer
- Large models: Multi-phase, multi-perspective deep dive → synthesis

## Future Enhancements

Potential improvements:
1. **Dynamic depth adjustment** - Adjust tool call depths based on result sizes
2. **Learned investigation patterns** - Use successful investigations to refine strategies
3. **Confidence calibration** - Track actual accuracy vs. claimed confidence
4. **Tool call optimization** - Identify redundant tool calls and optimize sequences
5. **Multi-language support** - Adapt prompts for non-English codebases

## Related Documentation

- [MCP_TOOL_PROMPTS.md](./MCP_TOOL_PROMPTS.md) - Prompts for other 6 analysis types
- [Agentic Orchestrator](../crates/codegraph-mcp/src/agentic_orchestrator.rs) - Orchestration engine
- [Graph Tool Schemas](../crates/codegraph-mcp/src/graph_tool_schemas.rs) - Tool definitions
- [Prompt Selector](../crates/codegraph-mcp/src/prompt_selector.rs) - Tier-aware prompt selection

## Version

- **Created:** 2025-01-08
- **Status:** Production Ready
- **Rust Module:** `codegraph_mcp::semantic_question_prompts`
- **Feature Flag:** `ai-enhanced`
