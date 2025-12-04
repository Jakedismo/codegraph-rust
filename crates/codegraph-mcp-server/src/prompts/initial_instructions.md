# CodeGraph Usage Guide

## When to Use CodeGraph Tools

Use CodeGraph tools as your **first choice** for understanding code. They return synthesized, relevant information instead of raw file contents, saving context for actual work.

### Tool Selection

| Question | Tool | What You Get |
|----------|------|--------------|
| "Where is X implemented?" | `agentic_code_search` | Code snippets with file:line locations |
| "What depends on X?" | `agentic_dependency_analysis` | Dependency chains and impact assessment |
| "How does execution flow?" | `agentic_call_chain_analysis` | Function call sequences with locations |
| "How is this structured?" | `agentic_architecture_analysis` | Module organization and patterns |
| "What's the public API?" | `agentic_api_surface_analysis` | Exported interfaces and signatures |
| "What do I need to know to change X?" | `agentic_context_builder` | Related code, patterns, dependencies |
| "How does X work across the system?" | `agentic_semantic_question` | Explanations with code evidence |

### Example Queries

```json
{"query": "how does authentication work?"}
{"query": "what depends on UserService?"}
{"query": "trace request handling from API to database"}
{"query": "gather context for adding rate limiting"}
```

---

## When to Fall Back to File Reading

CodeGraph tools work best for **understanding and exploration**. Use regular file tools when:

### 1. You Need Exact File Contents for Editing

After CodeGraph tells you which file to modify, read that specific file to make edits:

```
1. agentic_code_search("where is the login handler?")
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
4. Use dependency analysis to check impact

---

## Recommended Workflows

### Implementing a Feature

```
1. agentic_architecture_analysis("how is [similar feature] structured?")
2. agentic_context_builder("context for implementing [feature]")
3. Read specific files identified → make changes
```

### Debugging

```
1. agentic_code_search("[error message or symptom]")
2. agentic_call_chain_analysis("execution path through [area]")
3. Read identified files → fix issue
```

### Understanding a Codebase

```
1. agentic_architecture_analysis("overall structure")
2. agentic_api_surface_analysis("main interfaces")
3. agentic_code_search for specific areas of interest
```

### Refactoring

```
1. agentic_dependency_analysis("what depends on [target]?")
2. agentic_call_chain_analysis("execution paths through [target]")
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
