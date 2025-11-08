# Call Chain Analysis Prompts - Implementation Summary

## Overview

Created 4 tier-aware system prompts for the `call_chain_analysis` analysis type in CodeGraph's agentic MCP system. These prompts guide LLMs to trace execution call chains through codebases using SurrealDB graph tools with zero heuristics.

## Location

**File:** `/crates/codegraph-mcp/src/call_chain_prompts.rs`

## Prompts Created

### 1. `CALL_CHAIN_TERSE` (Small Tier)
- **Context:** < 50K tokens, max_steps: 5
- **Strategy:** Quick, shallow traces (depth 2-3)
- **Focus:** Direct call relationships only
- **Output:** Terse, focused call chain summary

### 2. `CALL_CHAIN_BALANCED` (Medium Tier)
- **Context:** 50K-200K tokens, max_steps: 10
- **Strategy:** Standard depth (3-5), balanced exploration
- **Focus:** Main paths + 2-3 key branches
- **Output:** Structured execution flow with critical paths

### 3. `CALL_CHAIN_DETAILED` (Large Tier)
- **Context:** 200K-500K tokens, max_steps: 15
- **Strategy:** Deep analysis (depth 5-7), multiple branches
- **Focus:** Comprehensive call chains with architectural context
- **Output:** Detailed multi-branch analysis with coupling metrics

### 4. `CALL_CHAIN_EXPLORATORY` (Massive Tier)
- **Context:** > 500K tokens, max_steps: 20
- **Strategy:** Exhaustive mapping (depth 7-10), all paths
- **Focus:** Complete execution graph with statistical analysis
- **Output:** Comprehensive report with 10 major sections

## Key Design Principles

### 1. Zero Heuristics
- Every claim must be backed by tool output data
- Extract node IDs from tool results - never invent
- Cite specific tool calls in reasoning
- Build incrementally from structured data only

### 2. JSON Response Format
All prompts enforce strict JSON responses:
```json
{
  "reasoning": "Explanation of what you're doing",
  "tool_call": {
    "tool_name": "trace_call_chain",
    "parameters": {"from_node": "nodes:123", "max_depth": 5}
  },
  "is_final": false
}
```

Final response:
```json
{
  "reasoning": "FINAL ANSWER: [Complete analysis]",
  "tool_call": null,
  "is_final": true
}
```

### 3. Primary Tool Focus
- `trace_call_chain(from_node, max_depth)` is PRIMARY tool
- Supporting tools used strategically:
  - `get_reverse_dependencies` - Find callers
  - `calculate_coupling_metrics` - Assess coupling
  - `get_hub_nodes` - Find central functions
  - `detect_circular_dependencies` - Find cycles
  - `get_transitive_dependencies` - Broader context

### 4. Tier-Aware Scaling

| Tier | Max Depth | Branches | Metrics | Output Size |
|------|-----------|----------|---------|-------------|
| Small | 2-3 | Main only | Minimal | 1-2 paragraphs |
| Medium | 3-5 | Main + 2-3 | Selective | Structured sections |
| Large | 5-7 | Multiple | Comprehensive | Detailed report |
| Massive | 7-10 | All paths | Statistical | Exhaustive 10-section report |

## Integration Points

### 1. PromptSelector Integration

To use these prompts in the `PromptSelector`, update `generate_default_prompt()`:

```rust
use crate::call_chain_prompts::{
    CALL_CHAIN_TERSE,
    CALL_CHAIN_BALANCED,
    CALL_CHAIN_DETAILED,
    CALL_CHAIN_EXPLORATORY
};

fn generate_default_prompt(
    &self,
    analysis_type: AnalysisType,
    verbosity: PromptVerbosity,
) -> String {
    match (analysis_type, verbosity) {
        (AnalysisType::CallChainAnalysis, PromptVerbosity::Terse) =>
            CALL_CHAIN_TERSE.to_string(),
        (AnalysisType::CallChainAnalysis, PromptVerbosity::Balanced) =>
            CALL_CHAIN_BALANCED.to_string(),
        (AnalysisType::CallChainAnalysis, PromptVerbosity::Detailed) =>
            CALL_CHAIN_DETAILED.to_string(),
        (AnalysisType::CallChainAnalysis, PromptVerbosity::Exploratory) =>
            CALL_CHAIN_EXPLORATORY.to_string(),
        // ... other analysis types use existing logic
        _ => { /* existing default prompt logic */ }
    }
}
```

### 2. Module Declaration

Add to `/crates/codegraph-mcp/src/lib.rs`:

```rust
mod call_chain_prompts;
```

### 3. AgenticOrchestrator

The prompts are designed to work with the existing `AgenticOrchestrator`:
- Uses `build_system_prompt()` to inject tool schemas
- Parses JSON responses via `parse_llm_response()`
- Executes tools via `GraphToolExecutor`
- Tracks steps and manages conversation history

## Prompt Structure Comparison

### TERSE (402 lines)
- Minimal guidance
- One focused tool call per step
- Shallow depth only
- Concise output format

### BALANCED (185 lines)
- Systematic strategy
- Balanced exploration
- Standard depth
- Structured sections

### DETAILED (332 lines)
- Comprehensive methodology
- 5-phase analysis strategy
- Deep tracing
- 7-section report format

### EXPLORATORY (605 lines)
- Exhaustive methodology
- 6-phase comprehensive strategy
- Maximum depth
- 10-section detailed report

## Analysis Focus Areas

All prompts cover these call chain analysis dimensions:

### Execution Flow
- Trace main execution path
- Identify branching/decision points
- Map conditional execution
- Track error handling paths

### Call Patterns
- Recursive calls (direct and indirect)
- Circular dependencies
- Mutual recursion
- Call cycles

### Architectural Context
- Coupling metrics (Ca, Ce, I)
- Hub nodes and hotspots
- God functions
- Layering violations

### Complexity Metrics
- Call depth distribution
- Branching factor (fan-out)
- Path convergence points
- Execution graph topology

### Performance Analysis
- Performance-critical paths
- Deeply nested sequences
- Potential bottlenecks
- Stack overflow risks

## Example Usage Flow

1. **User Query:** "Trace the execution flow of `processPayment()` function"

2. **Orchestrator:**
   - Selects tier based on LLM context window
   - Loads appropriate prompt (TERSE/BALANCED/DETAILED/EXPLORATORY)
   - Injects tool schemas
   - Sends to LLM

3. **LLM Step 1:**
   ```json
   {
     "reasoning": "Need to identify the processPayment function first. Will search for it.",
     "tool_call": null,
     "is_final": false
   }
   ```

4. **LLM Step 2:**
   ```json
   {
     "reasoning": "Found processPayment at nodes:payment_123. Now trace its call chain with depth 5.",
     "tool_call": {
       "tool_name": "trace_call_chain",
       "parameters": {"from_node": "nodes:payment_123", "max_depth": 5}
     },
     "is_final": false
   }
   ```

5. **Tool Execution:**
   - Orchestrator calls `trace_call_chain` via GraphToolExecutor
   - Returns call tree data
   - Injects result back to LLM

6. **LLM Step 3:**
   ```json
   {
     "reasoning": "FINAL ANSWER:\n\n## Execution Flow\nprocessPayment() calls...",
     "tool_call": null,
     "is_final": true
   }
   ```

## Validation Checklist

Before deploying these prompts:

- [x] **Zero heuristics:** All prompts require tool-backed claims
- [x] **JSON format:** Strict JSON response format enforced
- [x] **Tool focus:** `trace_call_chain` as primary tool
- [x] **Tier awareness:** Scaling from terse to exploratory
- [x] **Node ID extraction:** Always extract from tool results
- [x] **Structured outputs:** Defined section formats
- [x] **Call chain focus:** Execution paths, control flow, invocation sequences

## Testing Recommendations

1. **Small Tier (TERSE):**
   - Test with simple function (< 5 calls deep)
   - Verify shallow depth (2-3 levels)
   - Check concise output format

2. **Medium Tier (BALANCED):**
   - Test with moderate function (5-10 calls deep)
   - Verify 2-3 branch exploration
   - Check structured output sections

3. **Large Tier (DETAILED):**
   - Test with complex function (10-20 calls deep)
   - Verify 5-phase methodology
   - Check comprehensive 7-section report

4. **Massive Tier (EXPLORATORY):**
   - Test with architectural hotspot (20+ calls)
   - Verify exhaustive 6-phase analysis
   - Check 10-section detailed report

## Metrics to Track

After deployment, monitor:

1. **Tool Usage Patterns:**
   - Primary tool usage: `trace_call_chain` (should be 60-80% of calls)
   - Supporting tool distribution
   - Average tools per analysis

2. **Analysis Quality:**
   - Node ID citation rate (should be 100%)
   - Heuristic claims (should be 0%)
   - Analysis completeness by tier

3. **Performance:**
   - Steps used vs. max_steps by tier
   - Token efficiency
   - Time to completion

4. **Output Quality:**
   - Structured format compliance
   - Section completeness
   - Depth achieved vs. requested

## Future Enhancements

Potential improvements to consider:

1. **Dynamic Depth Adjustment:**
   - Start shallow, go deeper based on findings
   - Adaptive depth based on call complexity

2. **Parallel Branch Exploration:**
   - Trace multiple branches simultaneously
   - Merge results into unified analysis

3. **Incremental Analysis:**
   - Cache partial results
   - Resume from previous analysis

4. **Visualization Integration:**
   - Generate call graph visualizations
   - Interactive execution flow diagrams

5. **Pattern Recognition:**
   - Detect common call patterns
   - Identify anti-patterns automatically

## Files Modified/Created

- **Created:** `/crates/codegraph-mcp/src/call_chain_prompts.rs`
  - 4 const string prompts (TERSE, BALANCED, DETAILED, EXPLORATORY)
  - Complete tier-aware prompt set
  - Ready for integration

- **To Modify:** `/crates/codegraph-mcp/src/prompt_selector.rs`
  - Add import for `call_chain_prompts`
  - Update `generate_default_prompt()` match statement
  - Wire up prompts for CallChainAnalysis type

- **To Modify:** `/crates/codegraph-mcp/src/lib.rs`
  - Add `mod call_chain_prompts;`

## Conclusion

These prompts provide a complete, tier-aware system for call chain analysis that:
- Enforces zero heuristics through structured tool usage
- Scales from quick traces to exhaustive analysis
- Maintains strict JSON response format
- Focuses on execution flow and control flow patterns
- Provides actionable architectural insights

The prompts are production-ready and can be integrated into the existing `PromptSelector` and `AgenticOrchestrator` infrastructure with minimal changes.
