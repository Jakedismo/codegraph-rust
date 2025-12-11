# CodeGraph Agentic Tools

> **ALWAYS use CodeGraph tools instead of manual file reading/grepping.** CodeGraph provides semantic understanding, not just text search.

## Session Initialization

**AT THE START OF EVERY SESSION**, call:
```
read_initial_instructions from codegraph
```

This loads tool-specific guidance. **Do this ONCE per session before using any other CodeGraph tool.**

## Tool Selection Decision Tree

```
┌─────────────────────────────────────────────────────────────────┐
│                    WHAT DO YOU NEED?                            │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  Finding code / exploring?                                      │
│  └─→ agentic_code_search                                        │
│                                                                 │
│  Understanding dependencies / impact of changes?                │
│  └─→ agentic_dependency_analysis                                │
│                                                                 │
│  Tracing execution flow / debugging?                            │
│  └─→ agentic_call_chain_analysis                                │
│                                                                 │
│  Big picture / architecture overview?                           │
│  └─→ agentic_architecture_analysis                              │
│                                                                 │
│  Public interfaces / API surface?                               │
│  └─→ agentic_api_surface_analysis                               │
│                                                                 │
│  Gathering context for implementation?                          │
│  └─→ agentic_context_builder                                    │
│                                                                 │
│  Complex cross-cutting questions?                               │
│  └─→ agentic_semantic_question                                  │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

## Tool Reference

### `agentic_code_search`
**Use for:** Finding code, exploring unfamiliar areas, discovering patterns

**When:**
- "Where is X implemented?"
- "Find how Y works"
- "Show me all API endpoints"
- "Where is Z configured?"

**Returns:** Synthesized answers with code references (not just file lists)

---

### `agentic_dependency_analysis`
**Use for:** Impact analysis, coupling assessment, refactoring preparation

**When:**
- "What depends on X?"
- "What would break if I change Y?"
- "How coupled is A to B?"
- "Show dependency tree for Z"

**Returns:** Transitive dependencies, coupling metrics, hub identification

**IDD Integration:** Use in **Evaluation phase** to assess connascence

---

### `agentic_call_chain_analysis`
**Use for:** Execution flow tracing, debugging, understanding data paths

**When:**
- "Trace execution from A to B"
- "How does data flow through X?"
- "What's the call chain for Y?"
- "Follow the error handling path"

**Returns:** Call chains with execution context

---

### `agentic_architecture_analysis`
**Use for:** Onboarding, architecture reviews, understanding structure

**When:**
- "Explain the architecture of X"
- "What are the main components?"
- "What design patterns are used?"
- "How do layers interact?"

**Returns:** Component relationships, patterns, layer structure

**IDD Integration:** Use in **Specification phase** to understand existing context

---

### `agentic_api_surface_analysis`
**Use for:** API review, breaking change detection, interface documentation

**When:**
- "What does X expose publicly?"
- "List exported functions from Y"
- "What interfaces does Z implement?"
- "Would changing this break consumers?"

**Returns:** Public interfaces, exports, consumer analysis

**IDD Integration:** Use in **Evaluation phase** for boundary connascence verification

---

### `agentic_context_builder`
**Use for:** Pre-implementation context gathering

**When:**
- "Gather context for implementing X"
- "What do I need to know to add Y?"
- "Prepare context for Z feature"

**Returns:** Related code, patterns, dependencies, conventions

**IDD Integration:** Use before **Realization phase** to ensure complete context

---

### `agentic_semantic_question`
**Use for:** Complex questions spanning multiple areas

**When:**
- "How does X work across all layers?"
- "What's the testing strategy?"
- "How are Y managed throughout?"
- "What conventions exist for Z?"

**Returns:** Deep semantic reasoning across entire codebase

---

## IDD Integration Patterns

### Specification Phase (S)
```
1. agentic_architecture_analysis → Understand existing structure
2. agentic_code_search → Find similar implementations
3. agentic_semantic_question → Understand conventions
```

### Test Phase (T)
```
1. agentic_code_search → Find existing test patterns
2. agentic_semantic_question → "What testing conventions exist?"
```

### Realization Phase (R)
```
1. agentic_context_builder → Gather all implementation context
2. agentic_code_search → Find patterns to follow
3. agentic_api_surface_analysis → Understand interfaces
```

### Evaluation Phase (E)
```
1. agentic_dependency_analysis → Map dependencies (connascence)
2. agentic_api_surface_analysis → Verify boundary coupling
3. agentic_call_chain_analysis → Trace execution dependencies
```

### Adaptation Phase (A)
```
1. agentic_dependency_analysis → Impact analysis before refactoring
2. agentic_call_chain_analysis → Understand affected flows
3. agentic_code_search → Find all instances to modify
```

## Workflow Patterns

### Pattern 1: Exploration (New Area)
```
1. agentic_architecture_analysis → "Explain the X module"
2. agentic_code_search → "Where does X start?"
3. agentic_call_chain_analysis → "Trace execution through X"
4. agentic_dependency_analysis → "What does X depend on?"
```

### Pattern 2: Pre-Refactoring
```
1. agentic_dependency_analysis → "What depends on X?"
2. agentic_call_chain_analysis → "How is X used?"
3. agentic_api_surface_analysis → "What does X expose?"
4. agentic_context_builder → "Gather refactoring context for X"
```

### Pattern 3: Feature Implementation
```
1. agentic_code_search → "How are similar features implemented?"
2. agentic_context_builder → "Gather context for adding X"
3. agentic_semantic_question → "What conventions should I follow?"
4. agentic_dependency_analysis → "Where should X integrate?"
```

### Pattern 4: Debugging
```
1. agentic_code_search → "Where might X originate?"
2. agentic_call_chain_analysis → "Trace the error path"
3. agentic_dependency_analysis → "What could affect this?"
4. agentic_code_search → "Find similar working patterns"
```

## Critical Rules

### 🛑 NEVER Do This
- **NEVER** manually grep files when CodeGraph can search semantically
- **NEVER** read files one-by-one to trace dependencies
- **NEVER** manually trace call chains through file reading
- **NEVER** skip `read_initial_instructions` at session start

### ✅ ALWAYS Do This
- **ALWAYS** use `agentic_dependency_analysis` before refactoring
- **ALWAYS** use `agentic_context_builder` before implementing features
- **ALWAYS** use `agentic_code_search` instead of manual grep
- **ALWAYS** trust synthesized answers (they include graph relationships)

## Tool Chaining

For complex tasks, chain multiple tools:

```
"I need to refactor UserService. Please:
1. Use agentic_dependency_analysis to see what depends on it
2. Use agentic_call_chain_analysis to understand usage patterns
3. Use agentic_api_surface_analysis to see its public interface
4. Use agentic_context_builder to gather complete context"
```

## Natural Language Mapping

These natural language requests map to tools:

| Request | Tool |
|---------|------|
| "Find where X is implemented" | `agentic_code_search` |
| "What depends on X?" | `agentic_dependency_analysis` |
| "How does X flow through the system?" | `agentic_call_chain_analysis` |
| "Give me an overview of X" | `agentic_architecture_analysis` |
| "What does X expose?" | `agentic_api_surface_analysis` |
| "Gather context for implementing X" | `agentic_context_builder` |
| "How does X work across the codebase?" | `agentic_semantic_question` |

## Troubleshooting

| Problem | Solution |
|---------|----------|
| Results seem stale | Index may need refresh (`--watch` flag) |
| Results incomplete | Try more specific query or different tool |
| Tool not recognized | Call `read_initial_instructions` first |
| Manual file reading happening | Redirect to appropriate CodeGraph tool |