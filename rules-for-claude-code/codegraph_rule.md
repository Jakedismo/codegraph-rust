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
│  Finding code / building context / answering questions?         │
│  └─→ agentic_context                                            │
│      • focus="search" - code discovery                          │
│      • focus="builder" - comprehensive context                  │
│      • focus="question" - semantic Q&A                          │
│                                                                 │
│  Understanding impact / dependencies / call flows?              │
│  └─→ agentic_impact                                             │
│      • focus="dependencies" - dependency chains                 │
│      • focus="call_chain" - execution flow tracing              │
│                                                                 │
│  Architecture overview / API surfaces?                          │
│  └─→ agentic_architecture                                       │
│      • focus="structure" - system structure                     │
│      • focus="api_surface" - public interfaces                  │
│                                                                 │
│  Risk assessment / complexity / coupling hotspots?              │
│  └─→ agentic_quality                                            │
│      • focus="complexity" - complexity analysis                 │
│      • focus="coupling" - coupling metrics                      │
│      • focus="hotspots" - high-risk areas                       │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

## Tool Reference

### `agentic_context`

**Use for:** Finding code, exploring unfamiliar areas, building context, answering questions

**When:**

- "Where is X implemented?"
- "Find how Y works"
- "Gather context for implementing Z"
- "How does X work across the system?"

**Focus options:**

- `"search"` - Code discovery and exploration
- `"builder"` - Comprehensive context for implementation
- `"question"` - Deep semantic questions

**Returns:** Synthesized answers with code references, file:line locations

---

### `agentic_impact`

**Use for:** Impact analysis, dependency mapping, execution flow tracing

**When:**

- "What depends on X?"
- "What would break if I change Y?"
- "Trace execution from A to B"
- "Show dependency tree for Z"

**Focus options:**

- `"dependencies"` - Transitive dependency chains
- `"call_chain"` - Execution flow tracing

**Returns:** Dependency maps, call chains, coupling metrics, hub identification

**IDD Integration:** Use in **Evaluation phase** to assess connascence

---

### `agentic_architecture`

**Use for:** Onboarding, architecture reviews, understanding structure, API surface analysis

**When:**

- "Explain the architecture of X"
- "What are the main components?"
- "What does X expose publicly?"
- "List exported functions from Y"

**Focus options:**

- `"structure"` - Component relationships, patterns, layers
- `"api_surface"` - Public interfaces, exports

**Returns:** Component relationships, patterns, layer structure, public interfaces

**IDD Integration:** Use in **Specification phase** to understand existing context

---

### `agentic_quality`

**Use for:** Risk assessment, complexity analysis, refactoring prioritization

**When:**

- "Find complexity hotspots"
- "What are the highest-risk areas?"
- "Assess coupling in module X"
- "What should I refactor first?"

**Focus options:**

- `"complexity"` - Cyclomatic complexity analysis
- `"coupling"` - Coupling metrics (Ca, Ce, I)
- `"hotspots"` - High-risk code areas

**Returns:** Risk scores, complexity metrics, coupling analysis, refactoring priorities

---

## Workflow Patterns

### Pattern 1: Exploration (New Area)

```
1. agentic_architecture → "Explain the X module"
2. agentic_context → "Where does X start?"
3. agentic_impact(focus="call_chain") → "Trace execution through X"
4. agentic_impact(focus="dependencies") → "What does X depend on?"
```

### Pattern 2: Pre-Refactoring

```
1. agentic_impact → "What depends on X?"
2. agentic_quality → "What are the risk hotspots?"
3. agentic_architecture(focus="api_surface") → "What does X expose?"
4. agentic_context(focus="builder") → "Gather refactoring context for X"
```

### Pattern 3: Feature Implementation

```
1. agentic_context → "How are similar features implemented?"
2. agentic_context(focus="builder") → "Gather context for adding X"
3. agentic_context(focus="question") → "What conventions should I follow?"
4. agentic_impact → "Where should X integrate?"
```

### Pattern 4: Debugging

```
1. agentic_context → "Where might X originate?"
2. agentic_impact(focus="call_chain") → "Trace the error path"
3. agentic_impact(focus="dependencies") → "What could affect this?"
4. agentic_context → "Find similar working patterns"
```

## Critical Rules

### 🛑 NEVER Do This

- **NEVER** manually grep files when CodeGraph can search semantically
- **NEVER** read files one-by-one to trace dependencies
- **NEVER** manually trace call chains through file reading
- **NEVER** skip `read_initial_instructions` at session start

### ✅ ALWAYS Do This

- **ALWAYS** use `agentic_impact` before refactoring
- **ALWAYS** use `agentic_context(focus="builder")` before implementing features
- **ALWAYS** use `agentic_context` instead of manual grep
- **ALWAYS** trust synthesized answers (they include graph relationships)

## Tool Chaining

For complex tasks, chain multiple tools:

```
"I need to refactor UserService. Please:
1. Use agentic_impact to see what depends on it
2. Use agentic_quality to identify risk hotspots
3. Use agentic_architecture(focus="api_surface") to see its public interface
4. Use agentic_context(focus="builder") to gather complete context"
```

## Natural Language Mapping

These natural language requests map to tools:

| Request                                | Tool                                        |
| -------------------------------------- | ------------------------------------------- |
| "Find where X is implemented"          | `agentic_context`                           |
| "What depends on X?"                   | `agentic_impact`                            |
| "How does X flow through the system?"  | `agentic_impact(focus="call_chain")`        |
| "Give me an overview of X"             | `agentic_architecture`                      |
| "What does X expose?"                  | `agentic_architecture(focus="api_surface")` |
| "Gather context for implementing X"    | `agentic_context(focus="builder")`          |
| "How does X work across the codebase?" | `agentic_context(focus="question")`         |
| "What are the risky areas?"            | `agentic_quality`                           |

## Troubleshooting

| Problem                       | Solution                                  |
| ----------------------------- | ----------------------------------------- |
| Results seem stale            | Index may need refresh (`--watch` flag)   |
| Results incomplete            | Try more specific query or different tool |
| Tool not recognized           | Call `read_initial_instructions` first    |
| Manual file reading happening | Redirect to appropriate CodeGraph tool    |
