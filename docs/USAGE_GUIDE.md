# CodeGraph Usage Guide

How to get the most out of CodeGraph when working with AI coding assistants.

---

## Getting Started with Your AI Assistant

### Step 1: Initialize the Session

When you start a new conversation with your AI assistant (Claude Code, Cursor, etc.), the first thing to do is have it read CodeGraph's instructions:

```
Call the read_initial_instructions tool from codegraph
```

Or simply ask:

```
"Read the codegraph instructions so you know how to use it"
```

This loads guidance on when and how to use each agentic tool. **Do this once per session.**

### Step 2: Verify Connection

Ask your assistant to check that CodeGraph is working:

```
"Can you verify codegraph is connected and the index is available?"
```

The assistant should be able to call a simple tool like `agentic_code_search` to confirm.

---

## The Agentic Tools

CodeGraph provides 7 specialized tools. Here's when to ask for each:

### `agentic_code_search`
**Use when:** Finding code, exploring unfamiliar areas, discovering patterns

**Good prompts:**
- "Find where user authentication is handled"
- "Show me how error handling works in this codebase"
- "Find all API endpoints"
- "Where is the database connection configured?"

**What it does:** Multi-step semantic search with AI synthesis. Returns answers with code references, not just file lists.

---

### `agentic_dependency_analysis`
**Use when:** Before refactoring, understanding impact, checking coupling

**Good prompts:**
- "What depends on the UserService class?"
- "What would break if I change the auth module?"
- "Show me the dependency tree for the payment system"
- "How coupled is the API layer to the database?"

**What it does:** Maps transitive dependencies, calculates coupling metrics, identifies hub nodes.

---

### `agentic_call_chain_analysis`
**Use when:** Tracing execution flow, debugging, understanding data paths

**Good prompts:**
- "Trace the execution from HTTP request to database in the order flow"
- "How does data flow from the API to the cache?"
- "What's the call chain when a user logs in?"
- "Follow the error handling path from controller to response"

**What it does:** Traces call chains through your codebase, showing the execution path with context.

---

### `agentic_architecture_analysis`
**Use when:** Onboarding, architecture reviews, understanding the big picture

**Good prompts:**
- "Give me an overview of this project's architecture"
- "What are the main components and how do they interact?"
- "Explain the layer structure of this application"
- "What design patterns are used in this codebase?"

**What it does:** Analyzes the overall structure, identifies patterns, maps component relationships.

---

### `agentic_api_surface_analysis`
**Use when:** API design review, breaking change detection, documentation

**Good prompts:**
- "What public APIs does the auth module expose?"
- "List all exported functions from the utils package"
- "What interfaces does this service implement?"
- "Would changing this function signature break anything?"

**What it does:** Analyzes public interfaces, exports, and their consumers.

---

### `agentic_context_builder`
**Use when:** Before implementing a feature, gathering all relevant context

**Good prompts:**
- "I need to add rate limiting to the API. Gather all relevant context."
- "Collect everything I need to know to add a new payment provider"
- "What context do I need to implement user notifications?"
- "Prepare context for adding WebSocket support"

**What it does:** Comprehensively gathers related code, patterns, dependencies, and conventions.

---

### `agentic_semantic_question`
**Use when:** Complex questions that span multiple areas

**Good prompts:**
- "How does error handling work across all layers of the application?"
- "What's the testing strategy in this codebase?"
- "How are environment variables managed throughout the project?"
- "What conventions does this project use for async operations?"

**What it does:** Deep semantic reasoning across the entire indexed codebase.

---

## Prompt Patterns for Maximum Efficiency

### Be Specific About What You Need

**Less effective:**
```
"Tell me about the auth system"
```

**More effective:**
```
"I need to add OAuth support. Use codegraph to:
1. Find how authentication currently works
2. Identify what I'd need to modify
3. Check for any existing OAuth-related code"
```

### Chain Tools for Complex Tasks

For complex work, guide your assistant to use multiple tools:

```
"I'm going to refactor the UserService. Please:
1. First, use agentic_dependency_analysis to see what depends on it
2. Then use agentic_call_chain_analysis to understand how it's used
3. Finally, use agentic_context_builder to gather everything I need"
```

### Ask for the Right Tool

If your assistant is grepping files manually, redirect it:

```
"Instead of reading files one by one, use codegraph's agentic_code_search
to find what you're looking for"
```

Or:

```
"Use codegraph's dependency analysis tool instead of manually tracing imports"
```

### Reference Previous Results

CodeGraph results can inform follow-up questions:

```
"Based on that dependency analysis, which of those dependent components
would be most affected by changing the interface?"
```

---

## Working Patterns

### Pattern 1: Exploration First

When starting work on an unfamiliar area:

1. **Architecture overview:** "Use codegraph to explain the architecture of the payments module"
2. **Find entry points:** "Where does payment processing start?"
3. **Trace the flow:** "Trace the execution path for a successful payment"
4. **Check dependencies:** "What does the payment module depend on?"

### Pattern 2: Pre-Refactoring

Before making changes:

1. **Impact analysis:** "What would be affected if I change the OrderProcessor?"
2. **Dependency map:** "Show me everything that depends on this class"
3. **Find consumers:** "Who calls processOrder()?"
4. **Gather context:** "Collect all context I need for this refactoring"

### Pattern 3: Feature Implementation

When adding new features:

1. **Find patterns:** "How are similar features implemented in this codebase?"
2. **Gather context:** "Use agentic_context_builder for adding [feature]"
3. **Check conventions:** "What conventions should I follow based on existing code?"
4. **Verify integration:** "Where should this new feature integrate?"

### Pattern 4: Debugging

When tracking down issues:

1. **Find the code:** "Where is [symptom] likely originating?"
2. **Trace execution:** "Trace the call chain that leads to this error"
3. **Check dependencies:** "What could be affecting this behavior?"
4. **Find similar patterns:** "Are there similar patterns elsewhere that work correctly?"

---

## Tips for Best Results

### 1. Let CodeGraph Do the Heavy Lifting

Don't let your AI assistant manually grep through files when CodeGraph can do semantic search with context. If you see it reading files one by one, say:

```
"Use codegraph for this instead of reading files manually"
```

### 2. Trust the Synthesis

CodeGraph's agentic tools return synthesized answers, not raw search results. The answer includes reasoning based on graph relationships. Trust it.

### 3. Use Natural Language

You don't need to know the exact tool names. These all work:

- "Find where X is implemented" → `agentic_code_search`
- "What depends on X?" → `agentic_dependency_analysis`
- "How does X flow through the system?" → `agentic_call_chain_analysis`
- "Give me an overview of X" → `agentic_architecture_analysis`

### 4. Combine with Your AI's Reasoning

CodeGraph provides context; your AI provides reasoning. A good workflow:

1. Gather context with CodeGraph
2. Let your AI analyze and propose solutions
3. Verify proposals against CodeGraph's dependency analysis
4. Implement with confidence

### 5. Keep the Index Fresh

If you're making changes, ensure CodeGraph is running with `--watch`:

```bash
codegraph start stdio --watch
```

This keeps the index current as you edit files.

---

## Troubleshooting

### "CodeGraph isn't finding my recent changes"

The index might be stale. Either:
- Restart with `--watch` flag
- Manually re-index: `codegraph index . -r -l <languages>`

### "The AI keeps reading files manually instead of using CodeGraph"

Explicitly redirect:
```
"Stop reading files manually. Use codegraph's agentic_code_search tool instead."
```

### "Results seem incomplete"

Try being more specific, or use a different tool:
- For broad questions → `agentic_semantic_question`
- For specific code → `agentic_code_search`
- For relationships → `agentic_dependency_analysis`

### "The AI doesn't know about CodeGraph tools"

Have it read the instructions:
```
"Call read_initial_instructions from codegraph"
```

---

## Quick Reference

| I want to... | Ask for... |
|--------------|------------|
| Find code | `agentic_code_search` |
| See what depends on X | `agentic_dependency_analysis` |
| Trace execution flow | `agentic_call_chain_analysis` |
| Understand architecture | `agentic_architecture_analysis` |
| See public interfaces | `agentic_api_surface_analysis` |
| Gather context for a task | `agentic_context_builder` |
| Answer complex questions | `agentic_semantic_question` |
| Load tool instructions | `read_initial_instructions` |
