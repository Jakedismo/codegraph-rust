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
