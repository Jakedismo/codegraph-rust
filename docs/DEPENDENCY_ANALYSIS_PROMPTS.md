# Dependency Analysis Prompts for Agentic MCP System

Created 4 tier-aware system prompts for the "dependency_analysis" analysis type in CodeGraph's agentic MCP system.

## File Location

`crates/codegraph-mcp/src/dependency_analysis_prompts.rs`

## Overview

These prompts guide LLMs to analyze dependency relationships using SurrealDB graph tools with structured JSON responses. Each tier is optimized for different context window sizes and analysis depths.

## The 4 Tier-Aware Prompts

### 1. DEPENDENCY_ANALYSIS_TERSE (Small Tier)
- **Context Window**: < 50K tokens
- **Target Models**: qwen3:8b, smaller models
- **Max Steps**: ~5
- **Strategy**: Minimal tool calls, immediate dependencies only
- **Use Case**: Quick impact checks, direct dependency analysis
- **Depth**: 1-2 levels maximum
- **Example**: "What directly depends on this function?" → get_reverse_dependencies(depth=1) → report

### 2. DEPENDENCY_ANALYSIS_BALANCED (Medium Tier)
- **Context Window**: 50K-200K tokens
- **Target Models**: Claude Sonnet, GPT-4, standard production models
- **Max Steps**: ~10
- **Strategy**: Systematic multi-tool analysis with clear chains
- **Use Case**: Production dependency analysis, impact assessment
- **Depth**: 2-3 levels
- **Example**: Full impact analysis with coupling metrics and circular dependency checks

### 3. DEPENDENCY_ANALYSIS_DETAILED (Large Tier)
- **Context Window**: 200K-500K tokens
- **Target Models**: GPT-4, Kimi-k2-thinking, large context models
- **Max Steps**: ~15
- **Strategy**: Comprehensive multi-level dependency mapping
- **Use Case**: Architectural analysis, refactoring planning
- **Depth**: 3-5 levels
- **Example**: Complete dependency tree with coupling analysis, circular detection, and refactoring roadmap

### 4. DEPENDENCY_ANALYSIS_EXPLORATORY (Massive Tier)
- **Context Window**: > 500K tokens
- **Target Models**: Claude 1M, Grok-4-fast, massive context models
- **Max Steps**: ~20
- **Strategy**: Exhaustive multi-dimensional exploration
- **Use Case**: Codebase-wide architectural assessment
- **Depth**: 5-10 levels
- **Example**: Complete dependency topology with statistical analysis, pattern detection, and comprehensive refactoring roadmap

## Available Tools

All prompts use these 6 SurrealDB graph tools:

1. **get_transitive_dependencies(node_id, edge_type, depth)**
   - Get all dependencies of a node up to specified depth
   - Edge types: Calls, Imports, Uses, Extends, Implements, References, Contains, Defines

2. **detect_circular_dependencies(edge_type)**
   - Find circular dependency cycles
   - Returns bidirectional dependency pairs

3. **trace_call_chain(from_node, max_depth)**
   - Trace execution call sequences
   - Maps which functions are invoked

4. **calculate_coupling_metrics(node_id)**
   - Returns Ca (afferent), Ce (efferent), I (instability)
   - Instability = Ce/(Ce+Ca), where 0=stable, 1=unstable

5. **get_hub_nodes(min_degree)**
   - Find highly connected nodes
   - Identifies architectural hotspots

6. **get_reverse_dependencies(node_id, edge_type, depth)**
   - Find what depends ON this node
   - Critical for change impact analysis

## JSON Response Format

All prompts enforce this exact JSON structure:

### Intermediate Step (Tool Call)
```json
{
  "reasoning": "Analysis of current findings and next action",
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

### Final Step (Complete Answer)
```json
{
  "reasoning": "Comprehensive dependency analysis:\n1. DEPENDENCY SUMMARY: ...\n2. IMPACT ASSESSMENT: ...\n3. COUPLING ANALYSIS: ...\n4. RECOMMENDATIONS: ...",
  "tool_call": null,
  "is_final": true
}
```

## Critical Requirements

### ZERO HEURISTICS
- Only report verified graph data from tools
- Never assume or infer relationships
- All claims must be backed by tool results

### Structured Tool Outputs Only
- Extract exact node IDs from tool results (e.g., "nodes:123")
- Use structured data: counts, metrics, node IDs, paths
- No vague terms like "many" - use actual numbers

### JSON Response Format
- Every response must be valid JSON
- Must include "reasoning", "tool_call", "is_final" fields
- Tool parameters must match schema exactly

### Dependency Analysis Focus
- **Impact**: What depends on this? (reverse_dependencies)
- **Coupling**: How coupled is this? (coupling_metrics)
- **Circular Dependencies**: Any cycles? (detect_circular_dependencies)
- **Transitive Relationships**: Full dependency tree (transitive_dependencies)

## Integration Example

To use these prompts in the agentic orchestrator:

```rust
use crate::dependency_analysis_prompts::*;
use crate::context_aware_limits::ContextTier;

fn select_dependency_prompt(tier: ContextTier) -> &'static str {
    match tier {
        ContextTier::Small => DEPENDENCY_ANALYSIS_TERSE,
        ContextTier::Medium => DEPENDENCY_ANALYSIS_BALANCED,
        ContextTier::Large => DEPENDENCY_ANALYSIS_DETAILED,
        ContextTier::Massive => DEPENDENCY_ANALYSIS_EXPLORATORY,
    }
}
```

Then register with PromptSelector:

```rust
use crate::prompt_selector::{PromptSelector, AnalysisType, PromptVerbosity};

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
```

## Example Workflows

### TERSE Example
```
User: "What depends on the login function?"
Step 1: get_reverse_dependencies("login_fn", "Calls", depth=1)
        → Returns 8 callers
Final: "Login function has 8 direct callers: [list].
        Impact: LOW - only direct callers affected."
```

### BALANCED Example
```
User: "Analyze impact of changing payment_service"
Step 1: get_reverse_dependencies("payment_service", "Calls", depth=2)
        → Returns 25 dependents
Step 2: calculate_coupling_metrics("payment_service")
        → Ca=25, Ce=8, I=0.24 (stable)
Step 3: detect_circular_dependencies("Calls")
        → No cycles found
Final: "DEPENDENCY ANALYSIS:
        1. SUMMARY: 25 dependents, coupling=0.24 (stable)
        2. IMPACT: HIGH - 25 direct/transitive dependents
        3. COUPLING: Stable (I=0.24), good architecture
        4. CIRCULAR: Clean - no cycles
        5. RECOMMENDATIONS: Safe to modify with coordination"
```

### DETAILED Example
```
User: "Complete dependency analysis of auth module"
Steps 1-3: Hub analysis → identify auth components
Steps 4-6: Multi-depth transitive dependencies (depth=3,5)
Steps 7-9: Reverse dependencies at multiple depths
Steps 10-12: Coupling metrics for all auth nodes
Steps 13-14: Circular dependency detection
Step 15: Final comprehensive report with:
         - Dependency tree (5 levels)
         - Impact analysis (125 dependents)
         - Coupling distribution
         - Circular dependencies: 2 found
         - Refactoring recommendations
```

### EXPLORATORY Example
```
User: "Exhaustive codebase dependency health analysis"
Steps 1-5: Multi-threshold hub analysis (degree=3,5,10,20)
Steps 6-12: Complete dependency mapping (all edge types, depth=8)
Steps 13-17: Exhaustive reverse dependency analysis
Steps 18-20: Complete call chain tracing
Final: 50-page comprehensive report with:
       - Complete dependency topology
       - Statistical analysis (distributions, correlations)
       - All circular dependencies mapped
       - Coupling health score: 73/100
       - Architectural pattern detection
       - Prioritized refactoring roadmap (20 items)
```

## Metrics Interpretation

### Coupling Metrics
- **Ca (Afferent Coupling)**: Incoming dependencies
  - High Ca = Stable, many depend on it, risky to change
- **Ce (Efferent Coupling)**: Outgoing dependencies
  - High Ce = Unstable, depends on many, complex
- **Instability (I = Ce/(Ce+Ca))**:
  - I < 0.3: Stable (infrastructure, interfaces)
  - 0.3 ≤ I ≤ 0.7: Balanced (normal components)
  - I > 0.7: Unstable (UI/clients, orchestrators)

### Dependency Depth
- Depth 1: Immediate dependencies only
- Depth 2-3: Standard transitive analysis (BALANCED/DETAILED)
- Depth 5-8: Deep dependency trees (DETAILED/EXPLORATORY)
- Depth 10: Complete transitive closure (EXPLORATORY only)

### Hub Classification
- degree ≥ 20: Major hub (architectural center)
- 10 ≤ degree < 20: Secondary hub
- 5 ≤ degree < 10: Minor hub
- degree < 5: Regular node

## Testing the Prompts

### Unit Test Example
```rust
#[tokio::test]
async fn test_terse_dependency_analysis() {
    let orchestrator = AgenticOrchestrator::new(
        llm_provider,
        tool_executor,
        ContextTier::Small,
    );

    let result = orchestrator.execute(
        "What depends on the login function?",
        ""
    ).await.unwrap();

    // Should complete in 3-5 steps
    assert!(result.total_steps <= 5);
    assert!(result.completed_successfully);

    // Should use reverse_dependencies tool
    assert!(result.steps.iter().any(|s|
        s.tool_name.as_ref().map(|n| n == "get_reverse_dependencies").unwrap_or(false)
    ));
}
```

## Next Steps

1. **Integration**: Update `prompt_selector.rs` to use these prompts for `DependencyAnalysis`
2. **Testing**: Add integration tests for each tier
3. **Validation**: Run against real codebases to validate effectiveness
4. **Iteration**: Refine based on LLM performance (hallucinations, missed patterns)
5. **Other Analysis Types**: Create similar tier-aware prompts for:
   - CodeSearch
   - CallChainAnalysis
   - ArchitectureAnalysis
   - ApiSurfaceAnalysis
   - ContextBuilder
   - SemanticQuestion

## Design Philosophy

These prompts follow these principles:

1. **Zero Heuristics**: Only structured tool outputs, no assumptions
2. **Tier-Appropriate Depth**: Small models get simple tasks, large models get complex analysis
3. **JSON Strictness**: Enforce exact format for reliable parsing
4. **Strategic Tool Usage**: Guide LLM to use tools efficiently
5. **Actionable Output**: Every analysis includes specific recommendations
6. **Quantitative Focus**: Use metrics, not vague descriptions
7. **Architectural Lens**: Interpret through SOLID principles and patterns

## Troubleshooting

### LLM Not Following JSON Format
- Check that the model supports structured output
- Reduce temperature (already set to 0.1 in AgenticConfig)
- Add examples in prompt if needed

### Too Many Tool Calls
- Verify tier is appropriate for context window
- Check max_steps configuration matches tier
- Review reasoning - LLM should build on previous results

### Missing Critical Analysis
- Ensure prompt emphasizes the missing dimension
- Check if tools provide necessary data
- May need to add additional tools or prompts

### Hallucinations (Claims Without Tool Evidence)
- Strengthen "ZERO HEURISTICS" requirement in prompt
- Add validation: check that all claims cite tool results
- Lower temperature further (< 0.1)

## References

- Main implementation: `crates/codegraph-mcp/src/agentic_orchestrator.rs`
- Tool schemas: `crates/codegraph-mcp/src/graph_tool_schemas.rs`
- Context tiers: `crates/codegraph-mcp/src/context_aware_limits.rs`
- Prompt selector: `crates/codegraph-mcp/src/prompt_selector.rs`
