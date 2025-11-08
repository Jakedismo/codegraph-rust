# API Surface Analysis: Tier-Aware Agentic Prompts

## Overview

This document describes the 4 tier-aware system prompts created for the `api_surface_analysis` analysis type in CodeGraph's agentic MCP system.

## Implementation Files

- **Prompts Module**: `/crates/codegraph-mcp/src/agentic_api_surface_prompts.rs`
- **Integration**: `/crates/codegraph-mcp/src/prompt_selector.rs`
- **Module Declaration**: `/crates/codegraph-mcp/src/lib.rs`

## Prompt Design Philosophy

### Zero Heuristics Principle

All prompts strictly adhere to the **ZERO HEURISTICS** rule:
- NO assumptions about what makes a "good" or "bad" API
- Only report factual, measurable graph data from tool outputs
- No qualitative assessments or interpretations
- Focus on structured tool outputs and exact metric values

### Response Format

All prompts enforce strict JSON format:

```json
{
  "reasoning": "Explain which tool to call next and why",
  "tool_call": {
    "tool_name": "name_of_tool",
    "parameters": { "param": "value" }
  },
  "is_final": false
}
```

Final response:
```json
{
  "reasoning": "FINAL API SURFACE ANALYSIS:\n\n[Complete factual report]",
  "tool_call": null,
  "is_final": true
}
```

## Available Tools

Each prompt has access to 6 SurrealDB graph analysis tools:

1. **get_transitive_dependencies(node_id, edge_type, depth)**
   - Follow dependency edges recursively
   - Max depth: 10 levels

2. **detect_circular_dependencies(edge_type)**
   - Find bidirectional dependency cycles
   - Critical for API contract validation

3. **trace_call_chain(from_node, max_depth)**
   - Trace function call sequences
   - Map API execution flows

4. **calculate_coupling_metrics(node_id)**
   - Ca (afferent coupling): incoming dependencies
   - Ce (efferent coupling): outgoing dependencies
   - I (instability): I = Ce/(Ce+Ca), where 0=stable, 1=unstable

5. **get_hub_nodes(min_degree)**
   - Find highly connected nodes
   - Identifies widely-used API points

6. **get_reverse_dependencies(node_id, edge_type, depth)**
   - Find what depends ON this node
   - Critical for breaking change impact analysis

## The Four Tiers

### 1. TERSE (Small Context Window)

**Constant**: `API_SURFACE_TERSE`

**Target**: Small tier (ContextTier::Small)

**Constraints**:
- Maximum 5 tool calls
- Shallow depth exploration (1-2 levels)
- Focus on high-level overview

**Workflow**:
1. Identify public API nodes via `get_hub_nodes(min_degree=3)`
2. Assess stability via `calculate_coupling_metrics()` for hub nodes
3. If specific API given: `get_reverse_dependencies(depth=1)` for impact

**Output Structure**:
- Public API Nodes: count and node IDs
- Stability Metrics: afferent coupling and instability scores
- Breaking Change Impact: dependent count for critical nodes

### 2. BALANCED (Medium Context Window)

**Constant**: `API_SURFACE_BALANCED`

**Target**: Medium tier (ContextTier::Medium)

**Constraints**:
- Maximum 10 tool calls
- Moderate depth (2-3 levels)
- Balance breadth vs. depth

**Workflow**:
1. `get_hub_nodes(min_degree=5)` for major API points
2. `calculate_coupling_metrics()` for top 3-5 hub nodes (Ca, Ce, I)
3. `get_reverse_dependencies(depth=2)` for impact radius
4. `detect_circular_dependencies()` for Calls and Implements
5. `trace_call_chain(max_depth=3)` for top API nodes

**Output Structure**:
- Public API Nodes: list with degrees
- Stability Metrics: Ca, Ce, I values per API
- Breaking Change Impact: dependent counts at depth 1 and 2
- API Contract Issues: circular dependencies
- API Call Flows: key call chain mappings

### 3. DETAILED (Large Context Window)

**Constant**: `API_SURFACE_DETAILED`

**Target**: Large tier (ContextTier::Large)

**Constraints**:
- Maximum 15 tool calls
- Deep exploration (depth 3-5)
- Comprehensive coverage

**Workflow**:
1. Multi-level hub analysis (min_degree=5 and min_degree=10)
2. Complete stability analysis for ALL hub nodes
3. Deep impact analysis with `get_reverse_dependencies(depth=3)` for Calls and Implements
4. Dependency mapping with `get_transitive_dependencies(depth=4)`
5. Multi-edge-type circular dependency detection (Calls, Implements, Extends)
6. Deep call flow analysis with `trace_call_chain(max_depth=5)`
7. Interface implementation analysis with depth-2 exploration

**Output Structure**:
1. Public API Surface Inventory (categorized by degree)
2. API Stability Distribution (categorized by I values: <0.3, 0.3-0.7, ≥0.7)
3. Breaking Change Impact Radius (depth 1, 2, 3 + total)
4. API Dependency Chains (transitive deps at depth 4, max depth, external count)
5. API Contract Issues (cycles by edge type with hub node involvement)
6. API Execution Flows (complete paths from critical APIs)
7. Interface Implementation Mapping (implementer counts and depth)

### 4. EXPLORATORY (Massive Context Window)

**Constant**: `API_SURFACE_EXPLORATORY`

**Target**: Massive tier (ContextTier::Massive)

**Constraints**:
- Maximum 20 tool calls
- Maximum exploration depth (5-8 levels)
- Complete ecosystem coverage

**Workflow**:
1. Exhaustive API discovery (min_degree=3, 7, and 15)
2. Complete stability characterization for ALL hub nodes with ecosystem statistics
3. Maximum-depth impact analysis (depth=5 for Calls, Implements, Uses)
4. Comprehensive dependency mapping (depth=6 for Calls, Imports, Uses)
5. Ecosystem-wide contract integrity (5 edge types: Calls, Implements, Extends, Uses, Imports)
6. Deep call flow tracing (depth=8 for top 10 APIs)
7. Complete interface/trait analysis (depth=4)
8. API boundary analysis (module crossing detection)

**Output Structure**:
1. Complete Public API Inventory (degree distribution histogram)
2. Comprehensive Stability Characterization (7 bands from I=0 to I=1, ecosystem stats)
3. Maximum-Depth Breaking Change Impact Models (5-depth breakdown per edge type)
4. Complete API Dependency Graphs (6-depth analysis, shared dependency overlap)
5. Ecosystem-Wide Contract Integrity Report (5 edge types, cycle clusters)
6. Deep Execution Flow Maps (depth-8 complete path trees)
7. Complete Interface/Trait Ecosystem (4-depth transitive analysis)
8. API Boundary Crossing Analysis (module containment, external-facing identification)

## Focus Areas

All prompts focus on these four key areas:

1. **API Boundaries**: Identifying public vs. internal API surfaces
2. **Public Contracts**: Interface/trait definitions and implementations
3. **Breaking Change Impact**: Reverse dependency analysis and impact radius
4. **API Stability**: Coupling metrics (Ca, Ce, I) and dependency characteristics

## Metrics Reported (No Interpretation)

### Coupling Metrics
- **Ca (Afferent Coupling)**: Number of incoming dependencies
- **Ce (Efferent Coupling)**: Number of outgoing dependencies
- **I (Instability)**: I = Ce/(Ce+Ca)
  - I = 0: Maximally stable (pure dependency, no dependents)
  - I = 1: Maximally unstable (pure dependent, no dependencies)

### Impact Metrics
- Direct dependent count (depth 1)
- Near-impact dependents (depth 2-3)
- Far-impact dependents (depth 4-5)
- Total impact radius (all reachable dependents)
- Cascade chain length (longest path to leaf)

### Graph Metrics
- Hub node degree (total connections)
- Circular dependency counts
- Dependency depth (longest chain)
- Implementation counts
- Boundary crossing counts

## Integration Example

The prompts are integrated into `PromptSelector` via:

```rust
use crate::agentic_api_surface_prompts::{
    API_SURFACE_BALANCED, API_SURFACE_DETAILED,
    API_SURFACE_EXPLORATORY, API_SURFACE_TERSE,
};

// In generate_default_prompt():
if analysis_type == AnalysisType::ApiSurfaceAnalysis {
    return match verbosity {
        PromptVerbosity::Terse => API_SURFACE_TERSE.to_string(),
        PromptVerbosity::Balanced => API_SURFACE_BALANCED.to_string(),
        PromptVerbosity::Detailed => API_SURFACE_DETAILED.to_string(),
        PromptVerbosity::Exploratory => API_SURFACE_EXPLORATORY.to_string(),
    };
}
```

## Usage Pattern

When a user requests API surface analysis, the system:

1. Determines context tier based on LLM capabilities (Small/Medium/Large/Massive)
2. Maps tier to verbosity (Terse/Balanced/Detailed/Exploratory)
3. Selects appropriate prompt via `PromptSelector::select_prompt()`
4. Provides prompt to agentic orchestrator
5. LLM makes tool calls following the workflow
6. System executes graph queries via `GraphToolExecutor`
7. Results accumulate through iterative reasoning
8. Final factual analysis returned in structured format

## Key Design Decisions

1. **No Heuristics**: Prompts never judge "good" vs "bad" - only report facts
2. **Structured Outputs**: All analyses follow consistent section structure
3. **Tier-Specific Depth**: Deeper analysis at higher tiers without changing focus
4. **Tool Call Budgets**: Clear max_steps constraints per tier (5/10/15/20)
5. **Multi-Dimensional**: Higher tiers explore more edge types and depths
6. **Ecosystem Statistics**: EXPLORATORY tier adds aggregate metrics (mean, median, distribution)
7. **Cross-Referencing**: Higher tiers correlate multiple analyses (e.g., hub nodes in cycles)
8. **Exact Metrics**: Always report precise values (e.g., "Ca=15" not "high coupling")

## Testing Compilation

Verified with:
```bash
cargo check --package codegraph-mcp --features ai-enhanced
```

Status: ✅ Compiles successfully with zero errors
