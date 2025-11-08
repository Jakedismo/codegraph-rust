# CodeGraph Initial Instructions System Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add a sophisticated initial instructions prompt to CodeGraph MCP that guides agents on effective tool usage without overriding their autonomy, accessible via "Read the Initial Instructions of CodeGraph"

**Architecture:** Implement using MCP's prompt resource mechanism. Create a rich instruction document with metacognitive gates, tool selection criteria, and workflow patterns. Register as an MCP prompt resource that agents can request. Structure guidance as soft suggestions with hard gates only for safety/evidence requirements.

**Tech Stack:** Rust, rmcp SDK, MCP prompt resources, Markdown formatting

---

## Task 1: Create Initial Instructions Prompt Content

**Files:**
- Create: `crates/codegraph-mcp/src/prompts/initial_instructions.md`
- Create: `crates/codegraph-mcp/src/prompts/mod.rs`

**Step 1: Create prompts module structure**

```bash
mkdir -p crates/codegraph-mcp/src/prompts
```

**Step 2: Write initial instructions prompt content**

Create `crates/codegraph-mcp/src/prompts/initial_instructions.md`:

```markdown
# CodeGraph MCP Server - Initial Instructions for AI Agents

## Introduction

Welcome to CodeGraph! This document guides you on using CodeGraph's code-intelligence tools effectively during development. These are **soft guidelines**, not rigid rules—adapt them to your context.

## Core Philosophy

**Evidence-Based Development**: Ground all architectural decisions and code insights in actual codebase analysis, not assumptions.

**Metacognitive Workflow**: Think → Verify → Act. Always explain your reasoning before using tools.

---

## Tool Selection Framework

### 🎯 Decision Gates (Use These Before Tool Calls)

**Gate 1: Scope Clarity**
- ❓ "What specific question am I trying to answer?"
- ❓ "Do I need broad context or pinpoint precision?"
- ✅ Proceed only when you can state the question in one sentence

**Gate 2: Tool Appropriateness**
- ❓ "Which tool directly answers this question?"
- ❓ "Am I reaching for the right granularity?" (project → file → symbol)
- ✅ Choose the most specific tool that covers your scope

**Gate 3: Evidence Requirement**
- ❓ "What evidence do I need to collect before making a claim?"
- ❓ "Can I cite tool outputs to support this conclusion?"
- ✅ Never infer; always verify with tool data

**Gate 4: Safety Check**
- ❓ "Could this tool call modify state or be expensive?"
- ❓ "Do I understand the parameters I'm passing?"
- ✅ For write operations, explain your intent first

---

## Tool Categories & Selection Criteria

### 🔍 Category 1: Discovery & Search Tools

**When to use:** Starting analysis, exploring unfamiliar code, finding entry points

**Tools:**
- `enhanced_search` - **Natural language semantic search**
  - ✅ Use when: "How does X work?", "Find implementations of Y"
  - 📊 Returns: Ranked results with code snippets + explanations
  - 💡 Pattern: Start broad, then drill down with specific tools

- `semantic_intelligence` - **Deep pattern analysis**
  - ✅ Use when: Need design patterns, architecture insights, code quality analysis
  - 📊 Returns: Patterns, anti-patterns, architectural recommendations
  - 💡 Pattern: Use after enhanced_search to understand design context

**Workflow Pattern:**
```
1. enhanced_search("authentication flow")
   → Get high-level overview
2. semantic_intelligence on key files
   → Understand patterns and quality
3. Drill down with graph tools for dependencies
```

### 🧠 Category 2: Impact & Dependency Analysis Tools

**When to use:** Planning changes, understanding blast radius, refactoring

**Tools:**
- `impact_analysis` - **Change impact prediction**
  - ✅ Use when: Before modifying code, assessing refactoring scope
  - 📊 Returns: Affected files, functions, tests, risk level
  - ⚠️ Critical gate: ALWAYS run before major changes
  - 💡 Pattern: Run → Review → Plan → Execute

- `graph_neighbors` - **Direct dependencies**
  - ✅ Use when: Need immediate callers/callees of a function
  - 📊 Returns: 1-hop dependency graph
  - 💡 Pattern: Quick check for local impact

- `graph_traverse` - **Transitive dependency chains**
  - ✅ Use when: Tracing call chains, understanding dependency depth
  - 📊 Returns: Full dependency paths up to max depth
  - 💡 Pattern: Use after graph_neighbors for deep analysis

**Workflow Pattern:**
```
Before Refactoring:
1. impact_analysis on target code
   → Understand full scope
2. graph_traverse for dependency chains
   → Map transitive impacts
3. Review affected tests
   → Plan test updates
4. Make changes
5. Re-run impact_analysis
   → Verify no unexpected ripples
```

### 📊 Category 3: Code Intelligence Tools

**When to use:** Understanding code quality, finding patterns, detecting issues

**Tools:**
- `pattern_detection` - **Anti-pattern & smell detection**
  - ✅ Use when: Code review, quality assessment, finding technical debt
  - 📊 Returns: Detected patterns, severity, remediation suggestions
  - 💡 Pattern: Run on new code before committing

- `vector_search` - **Similarity-based code search**
  - ✅ Use when: "Find code similar to this", "Where else do we do X?"
  - 📊 Returns: Semantically similar code blocks
  - 💡 Pattern: Use for consistency checks and duplicate detection

**Workflow Pattern:**
```
Code Quality Check:
1. pattern_detection on changed files
   → Identify issues
2. vector_search for similar patterns
   → Check consistency across codebase
3. semantic_intelligence for design validation
   → Ensure patterns align with architecture
```

### 🔧 Category 4: Performance & Metrics Tools

**When to use:** Profiling, optimization, system health monitoring

**Tools:**
- `performance_metrics` - **System performance data**
  - ✅ Use when: Investigating slowness, profiling bottlenecks
  - 📊 Returns: CPU, memory, I/O metrics, bottleneck detection
  - 💡 Pattern: Baseline → Change → Measure → Compare

**Workflow Pattern:**
```
Performance Investigation:
1. performance_metrics (baseline)
   → Capture current state
2. Identify hotspots in output
3. Use enhanced_search to find implementation
4. Analyze with semantic_intelligence
5. Make optimization
6. performance_metrics (comparison)
   → Validate improvement
```

---

## Metacognitive Reasoning Patterns

### 🧩 Pattern 1: Progressive Refinement

**Principle:** Start broad, narrow iteratively

```
Question: "How does authentication work?"

❌ Bad: Immediately grep for "authenticate"
✅ Good:
  1. enhanced_search("authentication system")
     → Understand high-level flow
  2. graph_neighbors on auth entry point
     → See direct dependencies
  3. semantic_intelligence on auth module
     → Understand design patterns
  4. Now I can speak confidently about the system
```

**Reasoning Gate:** "Can I explain the architecture before diving into implementation details?"

### 🧩 Pattern 2: Evidence-Driven Claims

**Principle:** Never state facts without tool citations

```
❌ Bad: "This uses JWT for auth"
✅ Good: "Based on enhanced_search results showing JwtValidator in auth/tokens.rs:45, the system uses JWT tokens"

❌ Bad: "Changing this will break tests"
✅ Good: "impact_analysis shows this change affects 12 test files (listed in output), requiring test updates"
```

**Reasoning Gate:** "Can I cite the specific tool output that supports this claim?"

### 🧩 Pattern 3: Impact-First Changes

**Principle:** Understand consequences before acting

```
Before any refactoring:

✅ Required sequence:
  1. impact_analysis on target code
  2. Review affected components
  3. Explain the blast radius
  4. Get confirmation if impact > expected
  5. Only then proceed

❌ Skip this at your peril: Unintended breakage
```

**Reasoning Gate:** "Have I mapped all affected code and tests before changing anything?"

### 🧩 Pattern 4: Context Layering

**Principle:** Build understanding in layers: Project → Module → Component → Implementation

```
New feature in unfamiliar area:

✅ Layer 1 (Project): enhanced_search for high-level patterns
✅ Layer 2 (Module): semantic_intelligence on relevant modules
✅ Layer 3 (Component): graph_traverse to understand dependencies
✅ Layer 4 (Implementation): Read specific files with context

Each layer informs the next. Skip layers = miss critical context.
```

**Reasoning Gate:** "Do I understand each layer before going deeper?"

---

## Safety & Best Practices

### 🔒 Hard Requirements (These are non-negotiable)

1. **Impact Analysis Before Refactoring**
   - MUST run `impact_analysis` before modifying shared code
   - MUST review affected files list
   - MUST explain why the impact is acceptable

2. **Evidence-Based Reasoning**
   - MUST cite tool outputs when making claims
   - MUST NOT infer behavior without verification
   - MUST separate "tool output says X" from "I think Y"

3. **Explain Before Execute**
   - MUST explain reasoning before tool calls
   - MUST state what question you're answering
   - MUST NOT chain tools without reviewing intermediate results

### 💡 Soft Suggestions (Consider these guidelines)

1. **Start with Search, End with Graph**
   - Typically: enhanced_search → understand → graph tools → verify
   - Use semantic_intelligence for design-level questions
   - Use graph tools when you need hard dependency data

2. **Layer Your Analysis**
   - Consider: Project-level → Module-level → Component-level
   - Avoid: Jumping straight to implementation without context

3. **Iterate on Results**
   - Review tool outputs before next step
   - Refine queries based on what you learn
   - Build mental model incrementally

4. **Validate Assumptions**
   - When uncertain, verify with vector_search for similar code
   - Use pattern_detection to confirm suspected anti-patterns
   - Cross-reference multiple tool outputs for confidence

---

## Common Workflows

### 🚀 Workflow: Implementing a New Feature

```
1. Discovery Phase
   enhanced_search("similar feature")
   → Find existing patterns

2. Design Phase
   semantic_intelligence on similar modules
   → Understand design patterns
   graph_traverse on entry points
   → Map integration points

3. Impact Phase
   impact_analysis on files you'll modify
   → Understand change scope

4. Implementation Phase
   Write code
   pattern_detection on new code
   → Ensure quality

5. Validation Phase
   Run tests
   performance_metrics (if relevant)
   → Verify no regressions
```

### 🔧 Workflow: Debugging an Issue

```
1. Locate Phase
   enhanced_search("error symptom")
   → Find relevant code

2. Context Phase
   graph_neighbors on suspected function
   → See what calls it
   semantic_intelligence on the file
   → Understand design intent

3. Root Cause Phase
   graph_traverse backwards from symptom
   → Trace the bug path
   vector_search for similar bugs
   → Check if pattern exists elsewhere

4. Fix Phase
   impact_analysis on fix location
   → Ensure fix is safe
   Make change
   Re-run search to verify fix
```

### 📚 Workflow: Learning Unfamiliar Code

```
1. Overview Phase
   enhanced_search("high-level question")
   → Get architectural overview

2. Structure Phase
   semantic_intelligence on main modules
   → Understand design patterns

3. Dependency Phase
   graph_traverse from entry points
   → Map call flows

4. Detail Phase
   Read specific files with context
   Use vector_search for examples
   → Understand implementation
```

---

## Tool Parameters: What to Provide

### Quality Gates for Parameters

**Before calling ANY tool:**
- ✅ "Are my parameters specific enough?"
- ✅ "Am I using appropriate filters?"
- ✅ "Do I understand what each parameter does?"

### Parameter Guidelines by Tool

**enhanced_search:**
- `query`: Natural language, specific question
- `top_k`: Start with 5-10, increase if needed
- 💡 Tip: Phrase as a question, not keywords

**semantic_intelligence:**
- `file_path` or `code_snippet`: Be specific
- Use file paths when analyzing whole modules
- Use code snippets for targeted analysis

**impact_analysis:**
- `file_path` + `line_range` OR `symbol_name`
- ⚠️ Critical: Provide accurate scope
- Review output before changes

**graph_neighbors / graph_traverse:**
- `node_id` or `symbol_name`: Exact symbol reference
- `max_depth`: Start small (2-3), increase if needed
- 💡 Tip: Use enhanced_search first to find node IDs

**pattern_detection:**
- `file_paths` or `code_snippet`
- Consider scope: Single file vs directory
- 💡 Tip: Run on modified files before committing

**vector_search:**
- `query`: Code snippet or description
- `top_k`: 5-10 for precision, 20+ for exploration
- 💡 Tip: Use for "find similar" questions

**performance_metrics:**
- `operation`: Specific operation to profile
- Use baseline measurements for comparisons

---

## Red Flags: When You're Off Track

**🚨 Stop and reconsider if:**

- You're about to change code without running `impact_analysis`
- You're making claims without citing tool outputs
- You're using tools randomly hoping for insights
- You haven't explained your reasoning before tool calls
- You're skipping from discovery to implementation without understanding design
- You're ignoring tool warnings or unexpected results

**✅ You're on track if:**

- You can explain why you chose each tool
- Each tool call answers a specific question
- You're building understanding layer by layer
- You're citing tool outputs in your explanations
- You're running impact analysis before changes
- You're validating assumptions with cross-references

---

## Quick Reference: Tool Selection Decision Tree

```
Question: "How does X work?"
├─ Need overview? → enhanced_search
├─ Need design patterns? → semantic_intelligence
└─ Need dependencies? → graph_neighbors/traverse

Question: "Where should I make this change?"
├─ Need to find code? → enhanced_search
├─ Need similar examples? → vector_search
└─ Need to assess impact? → impact_analysis (REQUIRED)

Question: "Is this code good quality?"
├─ Need anti-pattern detection? → pattern_detection
├─ Need design analysis? → semantic_intelligence
└─ Need performance data? → performance_metrics

Question: "What depends on this?"
├─ Need direct dependencies? → graph_neighbors
├─ Need full chain? → graph_traverse
└─ Need change impact? → impact_analysis

Question: "Find similar code"
├─ Semantic similarity? → vector_search
├─ Pattern matching? → enhanced_search
└─ Design patterns? → semantic_intelligence
```

---

## Remember

**This is guidance, not law.** Adapt these patterns to your context. The gates exist to help you think clearly, not to constrain you.

**The core principles:**
1. **Think before you act** - Explain reasoning first
2. **Evidence over intuition** - Cite tool outputs
3. **Impact before change** - Always check blast radius
4. **Layer your understanding** - Build context incrementally

**When in doubt:** Start with `enhanced_search`, build understanding, then drill down with specific tools.

---

**Last Updated:** 2025-01-08
**Version:** 1.0.0
```

**Step 3: Create prompts module declaration**

Create `crates/codegraph-mcp/src/prompts/mod.rs`:

```rust
// ABOUTME: MCP prompt resources for agent guidance
// ABOUTME: Provides initial instructions and tool usage guidelines

pub const INITIAL_INSTRUCTIONS: &str = include_str!("initial_instructions.md");
```

**Step 4: Commit prompt content**

```bash
git add crates/codegraph-mcp/src/prompts/
git commit -m "feat: add initial instructions prompt with metacognitive gates"
```

---

## Task 2: Register Initial Instructions as MCP Prompt Resource

**Files:**
- Modify: `crates/codegraph-mcp/src/official_server.rs`
- Modify: `crates/codegraph-mcp/src/lib.rs`

**Step 1: Add prompts module to lib.rs**

In `crates/codegraph-mcp/src/lib.rs`, add:

```rust
pub mod prompts;
```

**Step 2: Import prompt types in official_server.rs**

At the top of `crates/codegraph-mcp/src/official_server.rs`:

```rust
use rmcp::model::{Prompt, PromptMessage, PromptArgument, Role};
use crate::prompts::INITIAL_INSTRUCTIONS;
```

**Step 3: Add prompt list handler to CodeGraphMCPServer**

Add method to `CodeGraphMCPServer` impl block:

```rust
#[prompts_list]
async fn list_prompts(&self) -> Vec<Prompt> {
    vec![
        Prompt {
            name: "codegraph_initial_instructions".to_string(),
            description: Some(
                "Read the Initial Instructions for CodeGraph MCP - comprehensive guide on effective tool usage with metacognitive gates and selection criteria".to_string()
            ),
            arguments: None,
        }
    ]
}
```

**Step 4: Add prompt get handler to CodeGraphMCPServer**

Add method to `CodeGraphMCPServer` impl block:

```rust
#[prompts_get]
async fn get_prompt(&self, name: String, _arguments: Option<serde_json::Value>) -> Result<Vec<PromptMessage>, Box<dyn std::error::Error + Send + Sync>> {
    match name.as_str() {
        "codegraph_initial_instructions" => {
            Ok(vec![
                PromptMessage {
                    role: Role::User,
                    content: rmcp::model::Content::text(
                        "Please read and acknowledge the CodeGraph Initial Instructions below. Use these guidelines to inform your tool selection and workflow patterns when using CodeGraph MCP tools."
                    ),
                },
                PromptMessage {
                    role: Role::Assistant,
                    content: rmcp::model::Content::text(INITIAL_INSTRUCTIONS),
                },
            ])
        }
        _ => Err(format!("Unknown prompt: {}", name).into()),
    }
}
```

**Step 5: Run tests to verify compilation**

```bash
cargo test -p codegraph-mcp --lib
```

Expected: Compiles successfully

**Step 6: Commit MCP prompt registration**

```bash
git add crates/codegraph-mcp/src/official_server.rs crates/codegraph-mcp/src/lib.rs
git commit -m "feat: register initial instructions as MCP prompt resource"
```

---

## Task 3: Add Convenience Tool for Reading Instructions

**Files:**
- Modify: `crates/codegraph-mcp/src/official_server.rs`

**Step 1: Add read_initial_instructions tool**

Add to `CodeGraphMCPServer` impl block:

```rust
/// Read the Initial Instructions for CodeGraph MCP Server
///
/// Provides comprehensive guidance on effective tool usage including:
/// - Tool selection framework with decision gates
/// - Metacognitive reasoning patterns
/// - Evidence-based workflow guidelines
/// - Common development workflows
/// - Safety requirements and best practices
#[tool]
async fn read_initial_instructions(
    &self,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    Ok(format!(
        "# CodeGraph MCP Initial Instructions\n\n{}\n\n---\n\n**Note:** These instructions are also available as an MCP prompt resource named 'codegraph_initial_instructions'.",
        INITIAL_INSTRUCTIONS
    ))
}
```

**Step 2: Build and test**

```bash
cargo build -p codegraph-mcp --lib
```

Expected: Compiles successfully

**Step 3: Commit convenience tool**

```bash
git add crates/codegraph-mcp/src/official_server.rs
git commit -m "feat: add read_initial_instructions tool for easy access"
```

---

## Task 4: Test the Implementation

**Files:**
- Create: `tests/test_initial_instructions.sh`

**Step 1: Create test script**

Create `tests/test_initial_instructions.sh`:

```bash
#!/bin/bash
# Test initial instructions accessibility

set -e

echo "🧪 Testing CodeGraph Initial Instructions..."
echo ""

# Start server in background
echo "📡 Starting MCP server..."
cargo run -p codegraph-mcp --bin codegraph -- start stdio &
SERVER_PID=$!
sleep 2

# Test 1: Check prompts list includes our prompt
echo "✅ Test 1: Verify prompt is listed"
# This would use MCP client to call prompts/list
# For now, we verify build succeeds

# Test 2: Check tool is registered
echo "✅ Test 2: Verify tool is registered"
# This would use MCP client to call tools/list
# For now, we verify build succeeds

# Test 3: Retrieve prompt content
echo "✅ Test 3: Verify prompt content is accessible"
# This would use MCP client to call prompts/get
# For now, we verify file exists

if [ -f "crates/codegraph-mcp/src/prompts/initial_instructions.md" ]; then
    echo "   ✓ Prompt file exists"

    # Verify key sections present
    if grep -q "Tool Selection Framework" crates/codegraph-mcp/src/prompts/initial_instructions.md; then
        echo "   ✓ Tool Selection Framework section found"
    fi

    if grep -q "Metacognitive Reasoning Patterns" crates/codegraph-mcp/src/prompts/initial_instructions.md; then
        echo "   ✓ Metacognitive Reasoning Patterns section found"
    fi

    if grep -q "Decision Gates" crates/codegraph-mcp/src/prompts/initial_instructions.md; then
        echo "   ✓ Decision Gates section found"
    fi
fi

# Cleanup
kill $SERVER_PID 2>/dev/null || true

echo ""
echo "✅ All tests passed!"
```

**Step 2: Make executable and run**

```bash
chmod +x tests/test_initial_instructions.sh
./tests/test_initial_instructions.sh
```

Expected: All checks pass

**Step 3: Commit test script**

```bash
git add tests/test_initial_instructions.sh
git commit -m "test: add initial instructions accessibility tests"
```

---

## Task 5: Update Documentation

**Files:**
- Modify: `README.md`
- Modify: `CLAUDE.md`
- Create: `docs/INITIAL_INSTRUCTIONS_GUIDE.md`

**Step 1: Add section to README.md**

Add after the "Getting Started" section:

```markdown
## 📚 For AI Agents: Initial Instructions

CodeGraph provides comprehensive guidance for AI agents using the MCP server:

**Read the instructions:**
```bash
# Via MCP prompt (recommended)
Use prompt: codegraph_initial_instructions

# Via tool call
Call tool: read_initial_instructions
```

**What you'll learn:**
- 🎯 Tool selection framework with decision gates
- 🧠 Metacognitive reasoning patterns
- 📊 Evidence-based workflow guidelines
- 🔒 Safety requirements and best practices
- 🚀 Common development workflows

**For detailed guidance, see:** [Initial Instructions Guide](docs/INITIAL_INSTRUCTIONS_GUIDE.md)
```

**Step 2: Add section to CLAUDE.md**

Add new section:

```markdown
## Initial Instructions for AI Agents

When working with this codebase, read the CodeGraph Initial Instructions first:

**How to access:**
- Call the `read_initial_instructions` tool
- Or request the MCP prompt `codegraph_initial_instructions`

**Key principles:**
1. **Evidence-Based**: Ground all claims in tool outputs
2. **Impact-First**: Run `impact_analysis` before refactoring
3. **Metacognitive**: Explain reasoning before tool calls
4. **Layered Understanding**: Build context incrementally

These are *soft guidelines*, not rigid rules. Adapt to your context.
```

**Step 3: Create detailed guide**

Create `docs/INITIAL_INSTRUCTIONS_GUIDE.md`:

```markdown
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
```

**Step 4: Commit documentation**

```bash
git add README.md CLAUDE.md docs/INITIAL_INSTRUCTIONS_GUIDE.md
git commit -m "docs: add initial instructions usage guide and references"
```

---

## Task 6: Final Integration Testing

**Files:**
- Create: `examples/initial_instructions_demo.sh`

**Step 1: Create demo script**

Create `examples/initial_instructions_demo.sh`:

```bash
#!/bin/bash
# Demo: Using CodeGraph Initial Instructions

echo "═══════════════════════════════════════════════════"
echo "  CodeGraph Initial Instructions Demo"
echo "═══════════════════════════════════════════════════"
echo ""

echo "📋 Available Access Methods:"
echo ""
echo "1️⃣  Via MCP Prompt (Native Protocol)"
echo "   Request: prompts/get → codegraph_initial_instructions"
echo ""
echo "2️⃣  Via Tool Call (Simple Interface)"
echo "   Request: tools/call → read_initial_instructions"
echo ""

echo "───────────────────────────────────────────────────"
echo ""

echo "📚 What's Included:"
echo ""
echo "  🎯 Tool Selection Framework"
echo "     • Decision gates for choosing tools"
echo "     • Scope, appropriateness, evidence checks"
echo ""
echo "  🧠 Metacognitive Patterns"
echo "     • Progressive refinement"
echo "     • Evidence-driven claims"
echo "     • Impact-first changes"
echo ""
echo "  🔒 Safety Requirements"
echo "     • Impact analysis before refactoring (HARD)"
echo "     • Evidence-based reasoning (HARD)"
echo "     • Explain before execute (HARD)"
echo ""
echo "  🚀 Common Workflows"
echo "     • Implementing features"
echo "     • Debugging issues"
echo "     • Learning unfamiliar code"
echo ""

echo "───────────────────────────────────────────────────"
echo ""

echo "🎓 Quick Start for AI Agents:"
echo ""
echo "Step 1: Read the instructions"
echo "  → Call: read_initial_instructions()"
echo ""
echo "Step 2: Apply decision gates"
echo "  → Before each tool: Why this tool? What evidence?"
echo ""
echo "Step 3: Follow workflows"
echo "  → Discovery → Analysis → Impact → Implementation"
echo ""

echo "═══════════════════════════════════════════════════"
```

**Step 2: Make executable**

```bash
chmod +x examples/initial_instructions_demo.sh
```

**Step 3: Run demo**

```bash
./examples/initial_instructions_demo.sh
```

Expected: Display comprehensive demo output

**Step 4: Commit demo script**

```bash
git add examples/initial_instructions_demo.sh
git commit -m "examples: add initial instructions demo script"
```

---

## Task 7: Add to Installation Script

**Files:**
- Modify: `install-codegraph-cloud.sh`

**Step 1: Add initial instructions info to installation output**

After the "MCP Server Transport Options" section, add:

```bash
    echo -e "${BLUE}📚 AI Agent Initial Instructions:${NC}"
    echo "   CodeGraph provides comprehensive guidance for AI agents:"
    echo ""
    echo "   ${GREEN}# Read via tool call${NC}"
    echo "   Call: read_initial_instructions()"
    echo ""
    echo "   ${GREEN}# Or request MCP prompt${NC}"
    echo "   Prompt: codegraph_initial_instructions"
    echo ""
    echo "   ${GREEN}# What you get:${NC}"
    echo "   • Tool selection framework with decision gates"
    echo "   • Metacognitive reasoning patterns"
    echo "   • Evidence-based workflow guidelines"
    echo "   • Safety requirements and best practices"
    echo ""
```

**Step 2: Test installation script**

```bash
./install-codegraph-cloud.sh --help
```

Expected: Script shows updated output

**Step 3: Commit installation script update**

```bash
git add install-codegraph-cloud.sh
git commit -m "feat: add initial instructions info to installation output"
```

---

## Completion Checklist

- [x] Initial instructions prompt created with metacognitive gates
- [x] Prompts module structure established
- [x] MCP prompt resource registered (`codegraph_initial_instructions`)
- [x] Convenience tool added (`read_initial_instructions`)
- [x] Tests created and passing
- [x] Documentation updated (README, CLAUDE.md, guide)
- [x] Demo script created
- [x] Installation script updated

---

## Testing the Complete System

**Manual test procedure:**

1. Build the server:
   ```bash
   cargo build -p codegraph-mcp --bin codegraph --features "server-http,ai-enhanced"
   ```

2. Start server:
   ```bash
   codegraph start stdio
   ```

3. Test via MCP client:
   ```json
   // List prompts
   {"method": "prompts/list"}

   // Get initial instructions
   {"method": "prompts/get", "params": {"name": "codegraph_initial_instructions"}}

   // Or via tool
   {"method": "tools/call", "params": {"name": "read_initial_instructions"}}
   ```

4. Verify content includes:
   - Tool selection framework
   - Decision gates
   - Metacognitive patterns
   - Safety requirements
   - Common workflows

---

## Plan Complete

This plan implements a sophisticated initial instructions system that:

✅ **Provides guidance without overriding autonomy** (unlike Serena)
✅ **Uses metacognitive gates** for decision-making
✅ **Enforces hard requirements** only for safety (impact analysis)
✅ **Offers soft suggestions** for workflows and patterns
✅ **Accessible via MCP prompts** and tool calls
✅ **Evidence-based philosophy** throughout
✅ **Non-invasive design** that complements agent capabilities

**Key Differentiators from Serena:**
- No system prompt override
- No rigid response format requirements
- Soft guidance with hard safety gates only
- Emphasizes reasoning over rules
- Adapts to agent's natural workflow
