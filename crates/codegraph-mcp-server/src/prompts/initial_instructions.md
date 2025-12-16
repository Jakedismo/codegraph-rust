# CodeGraph Usage Guide

## When to Use CodeGraph Tools

Use CodeGraph tools as your **first choice** for understanding code. They return synthesized, relevant information instead of raw file contents, saving context for actual work.

### Tool Selection

| Question | Tool | What You Get |
|----------|------|--------------|
| "What do I need to proceed?" | `agentic_context` | Client-readable context bundle: summary, analysis, highlights (with file:line + snippets), related locations, risks, next steps, confidence |
| "If I change X, what breaks?" | `agentic_impact` | Client-readable impact bundle: summary, analysis, impact highlights, affected locations, risks, next steps, confidence |
| "How is this area structured?" | `agentic_architecture` | Client-readable architecture bundle: summary, analysis, structural highlights, related locations, risks, next steps, confidence |
| "Where are the risks?" | `agentic_quality` | Client-readable quality bundle: summary, analysis, hotspot highlights, risk notes, related locations, next steps, confidence |

### Example Queries

```json
{"query": "gather context for adding rate limiting"}
{"query": "if I change the indexing pipeline, what breaks?"}
{"query": "how is the MCP server structured?"}
{"query": "hotspots and coupling risks in the parser"}
```

---

## Focus Parameter for Precision

Each tool accepts an optional `focus` parameter to narrow analysis to a specific mode:

| Tool | Focus Values | Default Behavior |
|------|-------------|-----------------|
| `agentic_context` | `"search"`, `"builder"`, `"question"` | Auto-selects based on query |
| `agentic_impact` | `"dependencies"`, `"call_chain"` | Analyzes both dependency chains and call flows |
| `agentic_architecture` | `"structure"`, `"api_surface"` | Provides both structural and interface analysis |
| `agentic_quality` | `"complexity"`, `"coupling"`, `"hotspots"` | Comprehensive risk assessment |

Example with focus:
```json
{"query": "trace login flow", "focus": "call_chain"}
{"query": "what public interfaces exist", "focus": "api_surface"}
```

---

## When to Fall Back to File Reading

CodeGraph tools work best for **understanding and exploration**. Use regular file tools when:

### 1. You Need Exact File Contents for Editing

After CodeGraph tells you which file to modify, read that specific file to make edits:

```
1. agentic_context({"query": "where is the login handler?"})
   → Returns: src/auth/handler.rs:45-67
2. Read src/auth/handler.rs to make your edit
```

### 2. CodeGraph Results Are Insufficient

If a CodeGraph tool returns:
- "No relevant code found"
- Results that don't answer your question
- Errors or empty responses

Then fall back to targeted file reading based on what you know.

### 3. You Need Very Recent Changes

If the codebase was modified after the last changes, CodeGraph may not reflect those changes instantly. Use file tools for:
- Files you just created
- Code you modified during your last phase
- Uncommitted changes

### 4. Simple, Known Locations

For quick lookups where you already know the exact file:
- Reading a config file you know exists
- Checking a specific line number from an error
- Viewing a file path from a stack trace

---

## How to Use CodeGraph Results

### Results Include Locations

Every result includes file paths and line numbers. Use these to:
- Navigate to the code
- Read specific files for editing
- Verify the analysis matches current code

### Results Are Synthesized

Results explain relationships and patterns, not just raw code. Use this to:
- Understand how components connect
- Plan changes with full context
- Identify all affected areas before refactoring

### Verify Before Major Changes

For significant modifications:
1. Use CodeGraph to understand scope
2. Read the actual files to confirm
3. Make your changes
4. Use `agentic_impact` to check impact

---

## Recommended Workflows

### Implementing a Feature

```
1. agentic_architecture({"query": "how is [similar feature] structured?"})
2. agentic_context({"query": "context for implementing [feature]"})
3. Read specific files identified → make changes
```

### Debugging

```
1. agentic_context({"query": "[error message or symptom]"})
2. agentic_impact({"query": "what depends on [suspected area]?"})
3. Read identified files → fix issue
```

### Understanding a Codebase

```
1. agentic_architecture({"query": "overall structure"})
2. agentic_context({"query": "main interfaces and entry points"})
```

### Refactoring

```
1. agentic_impact({"query": "what depends on [target]?"})
2. agentic_quality({"query": "risk hotspots around [target]"})
3. Plan changes based on full impact understanding
4. Read and modify files systematically
```

---

## Quick Reference

**Start with CodeGraph when:**
- Exploring unfamiliar code
- Understanding relationships
- Planning changes
- Answering "how" or "why" questions

**Use file tools when:**
- Making actual edits
- CodeGraph didn't find what you need
- Checking very recent changes
- Reading known, specific files

**Always:**
- Read CodeGraph results fully before requesting more
- Use the file locations provided in results
- Verify critical information before major changes

---

## Project-Only Indexing Policy

CodeGraph is project-scoped: it indexes the repository workspace, not third-party dependency source trees (e.g. downloaded crates, `node_modules/`, `site-packages/`).

When a question is about third-party library behavior, prefer library documentation or API references rather than expecting CodeGraph to show vendored source.
