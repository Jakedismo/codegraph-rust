# CodeGraph Initial Instructions - Usage Guide

## Overview

The CodeGraph Initial Instructions system provides AI agents with comprehensive guidance on effective tool usage without overriding their autonomy.

## Access Methods

### Method 1: MCP Prompt (Recommended)

```json
{
  "method": "prompts/get",
  "params": {
    "name": "codegraph_initial_instructions"
  }
}
```

**Benefits:**
- Native MCP protocol integration
- Can be cached by MCP clients
- Includes proper role structure

### Method 2: Tool Call

```json
{
  "method": "tools/call",
  "params": {
    "name": "read_initial_instructions"
  }
}
```

**Benefits:**
- Simple tool call interface
- Returns formatted markdown
- Easy to request in natural language

## Instruction Structure

### 1. Tool Selection Framework
- Decision gates for choosing appropriate tools
- Scope clarity, appropriateness, evidence requirements
- Safety checks for destructive operations

### 2. Tool Categories
- **Discovery & Search**: enhanced_search, semantic_intelligence
- **Impact & Dependency**: impact_analysis, graph_neighbors, graph_traverse
- **Code Intelligence**: pattern_detection, vector_search
- **Performance**: performance_metrics

### 3. Metacognitive Patterns
- Progressive refinement (broad → narrow)
- Evidence-driven claims (cite tool outputs)
- Impact-first changes (analyze before acting)
- Context layering (project → component → implementation)

### 4. Safety Requirements
**Hard gates:**
- Impact analysis before refactoring (non-negotiable)
- Evidence-based reasoning (must cite tools)
- Explain before execute (reasoning first)

**Soft suggestions:**
- Start with search, end with graph
- Layer your analysis
- Iterate on results

### 5. Common Workflows
- Implementing new features
- Debugging issues
- Learning unfamiliar code

## Design Philosophy

### Non-Invasive Guidance

**What we DON'T do (unlike Serena):**
- ❌ Override agent's system prompt
- ❌ Mandate specific response formats
- ❌ Require strict tool call sequences
- ❌ Enforce rigid workflows

**What we DO:**
- ✅ Provide decision frameworks
- ✅ Suggest metacognitive gates
- ✅ Offer workflow patterns
- ✅ Require safety checks (impact analysis)
- ✅ Emphasize evidence-based reasoning

### Hard vs Soft Requirements

**Hard (non-negotiable):**
1. Run `impact_analysis` before refactoring shared code
2. Cite tool outputs when making claims
3. Explain reasoning before tool calls

**Soft (recommendations):**
1. Use progressive refinement (search → understand → analyze)
2. Layer understanding (project → module → component)
3. Validate assumptions with cross-references

## Integration Examples

### Claude Desktop Configuration

```json
{
  "mcpServers": {
    "codegraph": {
      "command": "codegraph",
      "args": ["start", "stdio"],
      "env": {
        "CODEGRAPH_ENABLE_INITIAL_INSTRUCTIONS": "true"
      }
    }
  }
}
```

### First-Time Usage Pattern

```
Agent: "I need to refactor the authentication system"

Step 1: Read initial instructions
Agent calls: read_initial_instructions()
Agent reviews: Tool selection framework, safety requirements

Step 2: Apply decision gates
Gate 1 (Scope): "I need to understand current auth before changing it"
Gate 2 (Tool): enhanced_search is appropriate for overview
Gate 3 (Evidence): I'll cite tool outputs in my analysis
Gate 4 (Safety): I'll run impact_analysis before changes

Step 3: Follow workflow
Discovery → Design → Impact → Implementation → Validation

Result: Systematic, evidence-based refactoring with safety checks
```

## Maintenance & Updates

### Updating Instructions

1. Edit `crates/codegraph-mcp/src/prompts/initial_instructions.md`
2. Update version number in markdown
3. Test with `./tests/test_initial_instructions.sh`
4. Commit changes
5. Rebuild server

### Adding New Sections

Follow this structure:
```markdown
## Section Name

**When to use:** [Trigger conditions]

**Key concepts:**
- Concept 1
- Concept 2

**Workflow Pattern:**
```
[Step-by-step pattern]
```

**Decision Gates:**
- ❓ Question 1
- ❓ Question 2
- ✅ Proceed when...
```

## Troubleshooting

### Prompt Not Found

**Symptom:** MCP client can't find `codegraph_initial_instructions`

**Solution:**
1. Verify server built with latest code: `cargo build -p codegraph-mcp`
2. Check prompts list: `tools/call` → `tools/list` includes prompts
3. Restart MCP server

### Tool Not Registered

**Symptom:** `read_initial_instructions` tool not available

**Solution:**
1. Check tool is annotated with `#[tool]` macro
2. Verify compilation: `cargo build -p codegraph-mcp --lib`
3. Check tool list after server restart

### Content Not Loading

**Symptom:** Prompt returns empty content

**Solution:**
1. Verify file exists: `crates/codegraph-mcp/src/prompts/initial_instructions.md`
2. Check `include_str!` path in `mod.rs`
3. Rebuild to embed updated content

## References

- [MCP Prompts Specification](https://spec.modelcontextprotocol.io/specification/2024-11-05/server/prompts/)
- [rmcp SDK Documentation](https://docs.rs/rmcp/)
- [CodeGraph Tools Reference](./CODEGRAPH-MCP-TOOLS-GUIDE.md)
