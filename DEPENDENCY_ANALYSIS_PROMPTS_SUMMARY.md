# Dependency Analysis Prompts - Implementation Summary

Created 4 tier-aware system prompts for dependency analysis in CodeGraph's agentic MCP system.

## What Was Created

### 1. Core Prompts File
**Location**: `crates/codegraph-mcp/src/dependency_analysis_prompts.rs`

Four Rust string constants, each optimized for a different context tier:

- **DEPENDENCY_ANALYSIS_TERSE** (Small tier, < 50K tokens)
  - ~5 steps max, depth 1-2
  - Minimal tool calls, immediate dependencies only
  - For: qwen3:8b, smaller models

- **DEPENDENCY_ANALYSIS_BALANCED** (Medium tier, 50K-200K tokens)
  - ~10 steps max, depth 2-3
  - Systematic multi-tool analysis
  - For: Claude Sonnet, GPT-4, production models

- **DEPENDENCY_ANALYSIS_DETAILED** (Large tier, 200K-500K tokens)
  - ~15 steps max, depth 3-5
  - Comprehensive multi-level mapping
  - For: GPT-4, Kimi-k2, large context models

- **DEPENDENCY_ANALYSIS_EXPLORATORY** (Massive tier, > 500K tokens)
  - ~20 steps max, depth 5-10
  - Exhaustive multi-dimensional exploration
  - For: Claude 1M, Grok-4, massive context models

### 2. Documentation
**Location**: `docs/DEPENDENCY_ANALYSIS_PROMPTS.md`

Comprehensive guide covering:
- Prompt descriptions and use cases
- Available tools and their parameters
- JSON response format specification
- Integration examples
- Metrics interpretation
- Testing guidelines
- Troubleshooting

### 3. Integration Example
**Location**: `crates/codegraph-mcp/src/dependency_analysis_prompts_integration_example.rs`

Complete integration code with:
- Helper functions to register prompts
- Tier selection logic
- Comprehensive unit tests
- Integration examples for each tier

### 4. Module Export
**Location**: `crates/codegraph-mcp/src/lib.rs`

Added module declaration:
```rust
#[cfg(feature = "ai-enhanced")]
pub mod dependency_analysis_prompts;
```

## How the Prompts Work

### JSON Response Format
Each prompt guides the LLM to respond in this exact format:

**Intermediate steps (with tool call):**
```json
{
  "reasoning": "Analysis and next action",
  "tool_call": {
    "tool_name": "get_reverse_dependencies",
    "parameters": {
      "node_id": "nodes:123",
      "edge_type": "Calls",
      "depth": 3
    }
  },
  "is_final": false
}
```

**Final step (with answer):**
```json
{
  "reasoning": "Complete dependency analysis with summary, impact, coupling, recommendations",
  "tool_call": null,
  "is_final": true
}
```

### Available Tools
All prompts use these 6 SurrealDB graph tools:

1. `get_transitive_dependencies(node_id, edge_type, depth)` - Get dependencies
2. `detect_circular_dependencies(edge_type)` - Find cycles
3. `trace_call_chain(from_node, max_depth)` - Trace execution paths
4. `calculate_coupling_metrics(node_id)` - Get Ca, Ce, Instability
5. `get_hub_nodes(min_degree)` - Find highly connected nodes
6. `get_reverse_dependencies(node_id, edge_type, depth)` - Find dependents

## Key Features

### ZERO HEURISTICS
All prompts enforce this strictly:
- Only report verified graph data from tools
- Never assume or infer relationships
- All claims must be backed by tool results
- Extract exact node IDs from tool outputs

### Tier-Appropriate Strategies
Each tier has optimized guidance:
- **TERSE**: "Limit tool calls to 3-5 total"
- **BALANCED**: "Use 5-10 tool calls for thorough but focused analysis"
- **DETAILED**: "Use 10-15 tool calls for comprehensive analysis"
- **EXPLORATORY**: "Use 15-20+ tool calls for exhaustive exploration"

### Structured Output
Each tier produces progressively more detailed analysis:
- **TERSE**: Direct impact summary
- **BALANCED**: 5-section analysis (Summary, Impact, Coupling, Circular, Recommendations)
- **DETAILED**: 10-section report with metrics and refactoring roadmap
- **EXPLORATORY**: 12-section comprehensive report with statistical analysis

## Integration

### Option 1: Use PromptSelector (Recommended)

```rust
use codegraph_mcp::dependency_analysis_prompts::*;
use codegraph_mcp::{PromptSelector, AnalysisType, PromptVerbosity};

let mut selector = PromptSelector::new();

// Register all 4 tiers
selector.register_prompt(
    AnalysisType::DependencyAnalysis,
    PromptVerbosity::Terse,
    DEPENDENCY_ANALYSIS_TERSE.to_string(),
);

selector.register_prompt(
    AnalysisType::DependencyAnalysis,
    PromptVerbosity::Balanced,
    DEPENDENCY_ANALYSIS_BALANCED.to_string(),
);

selector.register_prompt(
    AnalysisType::DependencyAnalysis,
    PromptVerbosity::Detailed,
    DEPENDENCY_ANALYSIS_DETAILED.to_string(),
);

selector.register_prompt(
    AnalysisType::DependencyAnalysis,
    PromptVerbosity::Exploratory,
    DEPENDENCY_ANALYSIS_EXPLORATORY.to_string(),
);

// Use it
let prompt = selector.select_prompt(
    AnalysisType::DependencyAnalysis,
    ContextTier::Medium
)?;
```

### Option 2: Direct Helper Function

```rust
use codegraph_mcp::dependency_analysis_prompts::get_dependency_analysis_prompt;
use codegraph_mcp::ContextTier;

let prompt = get_dependency_analysis_prompt(ContextTier::Medium);
```

## Example Workflows

### TERSE (Small Tier)
```
User: "What depends on the login function?"

Step 1: get_reverse_dependencies("login_fn", "Calls", depth=1)
        Returns: 8 callers

Final: {
  "reasoning": "Login function has 8 direct callers:\n
                - UserController.login() (users/controller.rs:45)\n
                - AdminController.adminLogin() (admin/controller.rs:23)\n
                ...\n
                IMPACT: LOW - Only 8 direct callers affected.",
  "tool_call": null,
  "is_final": true
}
```

### BALANCED (Medium Tier)
```
User: "Analyze impact of changing payment_service"

Step 1: get_reverse_dependencies("payment_service", "Calls", depth=2)
        Returns: 25 dependents

Step 2: calculate_coupling_metrics("payment_service")
        Returns: Ca=25, Ce=8, I=0.24

Step 3: detect_circular_dependencies("Calls")
        Returns: No cycles

Final: {
  "reasoning": "COMPREHENSIVE DEPENDENCY ANALYSIS:\n\n
                1. DEPENDENCY SUMMARY: 25 dependents, coupling=0.24 (stable)\n
                2. IMPACT ASSESSMENT: HIGH - 25 direct/transitive dependents\n
                3. COUPLING ANALYSIS: Stable (I=0.24), good architecture\n
                4. CIRCULAR DEPENDENCIES: Clean - no cycles detected\n
                5. RECOMMENDATIONS: Safe to modify with proper coordination",
  "tool_call": null,
  "is_final": true
}
```

### DETAILED (Large Tier)
15-step analysis with:
- Hub identification
- Multi-depth transitive dependencies
- Reverse dependency mapping
- Coupling metrics for all key nodes
- Circular dependency detection
- Comprehensive refactoring roadmap

### EXPLORATORY (Massive Tier)
20-step exhaustive analysis with:
- Multi-threshold hub analysis
- Complete dependency mapping (all edge types, depth 8)
- Statistical analysis (distributions, correlations)
- Pattern detection
- Architectural quality assessment
- Prioritized refactoring roadmap (20+ items)

## Testing

Compilation verified:
```bash
cargo check --package codegraph-mcp --features ai-enhanced
# ✅ Finished successfully
```

Run the integration tests:
```rust
cargo test --package codegraph-mcp --features ai-enhanced dependency_analysis
```

## Next Steps

1. **Update PromptSelector Initialization**
   - Add call to `register_dependency_analysis_prompts()` in initialization code
   - Replace placeholder prompts with optimized prompts

2. **Test with Real LLMs**
   - Small tier: Test with qwen3:8b
   - Medium tier: Test with Claude Sonnet / GPT-4
   - Large tier: Test with GPT-4 / Kimi-k2
   - Massive tier: Test with Claude 1M

3. **Validate Effectiveness**
   - Check JSON format adherence
   - Verify zero heuristics compliance
   - Measure step counts vs. tier limits
   - Assess quality of dependency analysis

4. **Iterate Based on Results**
   - Adjust depth limits if needed
   - Refine tool call strategies
   - Add examples if LLM struggles
   - Fine-tune final report structures

5. **Create Prompts for Other Analysis Types**
   - CodeSearch (7 types total in AnalysisType enum)
   - CallChainAnalysis
   - ArchitectureAnalysis
   - ApiSurfaceAnalysis
   - ContextBuilder
   - SemanticQuestion

## Design Principles

These prompts follow strict principles:

1. **ZERO HEURISTICS**: Only structured tool outputs
2. **Tier-Appropriate**: Small models get simple tasks, large models get complex analysis
3. **JSON Strictness**: Exact format enforcement for reliable parsing
4. **Strategic Tools**: Efficient tool usage guidance
5. **Actionable Output**: Every analysis includes specific recommendations
6. **Quantitative**: Metrics, not vague descriptions
7. **Architectural**: Interpret through SOLID principles

## Files Modified

✅ Created:
- `crates/codegraph-mcp/src/dependency_analysis_prompts.rs` (core prompts)
- `crates/codegraph-mcp/src/dependency_analysis_prompts_integration_example.rs` (integration code)
- `docs/DEPENDENCY_ANALYSIS_PROMPTS.md` (documentation)
- `DEPENDENCY_ANALYSIS_PROMPTS_SUMMARY.md` (this file)

✅ Modified:
- `crates/codegraph-mcp/src/lib.rs` (added module export)

## Verification

The code compiles successfully with the `ai-enhanced` feature:
```bash
✓ cargo check --package codegraph-mcp --features ai-enhanced
  Finished `dev` profile [optimized + debuginfo] target(s) in 1.20s
```

All prompts:
- ✅ Are valid Rust string constants
- ✅ Contain all 6 dependency analysis tools
- ✅ Enforce JSON response format
- ✅ Include ZERO HEURISTICS requirement
- ✅ Have tier-appropriate depth and step limits
- ✅ Provide structured output templates

## Usage Example

```rust
use codegraph_mcp::{
    AgenticOrchestrator,
    ContextTier,
    dependency_analysis_prompts::get_dependency_analysis_prompt,
};

// Create orchestrator with dependency analysis prompt
let tier = ContextTier::Medium;
let system_prompt = get_dependency_analysis_prompt(tier);

let orchestrator = AgenticOrchestrator::new(
    llm_provider,
    tool_executor,
    tier,
);

// Execute analysis
let result = orchestrator.execute(
    "Analyze the impact of changing the authentication module",
    "" // optional context
).await?;

// Result includes:
// - result.final_answer: Complete dependency analysis
// - result.steps: All reasoning steps with tool calls
// - result.total_steps: Number of steps taken
// - result.completed_successfully: Whether it finished
```

## Questions?

For implementation details, see:
- Documentation: `docs/DEPENDENCY_ANALYSIS_PROMPTS.md`
- Integration examples: `crates/codegraph-mcp/src/dependency_analysis_prompts_integration_example.rs`
- Agentic orchestrator: `crates/codegraph-mcp/src/agentic_orchestrator.rs`
- Tool schemas: `crates/codegraph-mcp/src/graph_tool_schemas.rs`
