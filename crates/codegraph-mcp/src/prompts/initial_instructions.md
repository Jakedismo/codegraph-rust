# CodeGraph MCP - Initial Instructions for AI Agents

## Introduction

CodeGraph is an **autonomous code intelligence system** powered by multi-step reasoning and graph analysis. Unlike traditional code search tools, CodeGraph's **agentic tools** autonomously decide which graph analysis functions to call based on your natural language queries.

**Critical principle:** Use CodeGraph's agentic tools for **all codebase exploration**. These tools provide condensed, semantically-relevant, graph-analyzed context without burning your context window.

Utilising CodeGraph gives you access to a frontier reasoning models 2M token ctx window and deep multi-step analysis results over the whole codebase.

---

## Core Philosophy: Autonomous Codebase Intelligence

### ✅ The CodeGraph Way (Autonomous Agentic)

Agent: agentic_dependency_analysis("how does authentication integrate with the database?")

CodeGraph autonomously:
- Searches for authentication code
- Identifies relevant nodes
- Traces transitive dependencies
- Analyzes coupling metrics
- Maps call chains
- Synthesizes findings

Result: Comprehensive answer in 150 lines, complete architectural understanding
---

## How Agentic Tools Work

### Architecture Overview

Your Query
    ↓
Agentic Tool (with multi-step reasoning)
    ↓
Autonomous LLM Decision-Making:
  - Which graph functions to call?
  - What order to call them?
  - How deep to traverse?
  - What patterns to look for?
    ↓
SurrealDB Graph Functions:
  - fn::get_transitive_dependencies()
  - fn::get_reverse_dependencies()
  - fn::trace_call_chain()
  - fn::calculate_coupling_metrics()
  - fn::get_hub_nodes()
  - fn::detect_circular_dependencies()
    ↓
Synthesized, Context-Efficient Answer

---

## Available Agentic Tools

All agentic tools follow the same pattern:
1. **Input:** Natural language query
2. **Processing:** Autonomous multi-step graph exploration
3. **Output:** Synthesized analysis with code context

### 🔍 1. `agentic_code_search`

**What it does:** Autonomous graph exploration for finding and understanding code

**When to use:**
- "Where is X implemented?"
- "Find code that does Y"
- "How does Z work?"
- Starting any code exploration task

**How it works:**
- Semantically searches for relevant code
- Autonomously decides which nodes to explore
- Traces relationships and dependencies
- Provides context-rich results with explanations

**Example:**

```javascript
agentic_code_search("how does JWT token validation work in this codebase?")

// Returns: Autonomous analysis including:
// - Token validation entry points
// - Related middleware and utilities
// - Integration with authentication flow
// - Security considerations found in code
```

**Parameters:**
- `query` (required): Natural language question or search term

---

### 📊 2. `agentic_dependency_analysis`

**What it does:** Autonomous dependency chain and impact analysis

**When to use:**
- "What depends on this code?"
- "What will break if I change X?"
- "Map the dependency graph for Y"
- Understanding impact before refactoring

**How it works:**
- Finds forward and reverse dependencies
- Calculates coupling metrics (afferent/efferent)
- Detects circular dependencies
- Identifies stability vs instability

**Example:**

```javascript
agentic_dependency_analysis("analyze dependencies of the AuthService module")

// Returns: Autonomous analysis including:
// - All modules that depend on AuthService (afferent)
// - All modules AuthService depends on (efferent)
// - Coupling metrics (instability score)
// - Impact assessment for potential changes
// - Circular dependency warnings if any
```

**Parameters:**
- `query` (required): Dependency analysis question

---

### 🔗 3. `agentic_call_chain_analysis`

**What it does:** Autonomous execution flow tracing and call path analysis

**When to use:**
- "Trace the execution path from X to Y"
- "What's the call chain for Z?"
- "How does data flow through the system?"
- Debugging execution flows

**How it works:**
- Traces call chains through the graph
- Identifies execution paths
- Maps data flow
- Detects recursive calls

**Example:**
```javascript
agentic_call_chain_analysis("trace the execution path from HTTP request to database query")

// Returns: Autonomous analysis including:
// - Complete call chain from entry point to DB
// - Intermediate layers (routing → controller → service → repository)
// - Data transformations along the path
// - Error handling points
```

**Parameters:**
- `query` (required): Call chain question

---

### 🏗️ 4. `agentic_architecture_analysis`

**What it does:** Autonomous architectural pattern assessment and design analysis

**When to use:**
- "What architectural patterns does this code use?"
- "Analyze the system architecture"
- "What design patterns are present?"
- Understanding codebase organization

**How it works:**
- Identifies architectural patterns
- Analyzes layer separation
- Detects design patterns (Factory, Strategy, etc.)
- Assesses code organization

**Example:**
```javascript
agentic_architecture_analysis("what architectural patterns does the authentication system use?")

// Returns: Autonomous analysis including:
// - Layered architecture breakdown (API → Service → Repository)
// - Design patterns identified (Strategy for auth providers)
// - Separation of concerns analysis
// - Coupling/cohesion assessment
// - Recommendations for improvements
```

**Parameters:**
- `query` (required): Architecture analysis question

---

### 🌐 5. `agentic_api_surface_analysis`

**What it does:** Autonomous public interface and API contract analysis

**When to use:**
- "What's the public API of X?"
- "Analyze the external interface of Y"
- "What methods are exposed to consumers?"
- Understanding API design

**How it works:**
- Identifies public vs private interfaces
- Analyzes API contracts
- Detects breaking change risks
- Maps consumer usage

**Example:**
```javascript
agentic_api_surface_analysis("analyze the public API surface of the UserService")

// Returns: Autonomous analysis including:
// - All public methods and their signatures
// - Public vs private method breakdown
// - Consumer usage patterns (who calls what)
// - API stability assessment
// - Breaking change risk analysis
```

**Parameters:**
- `query` (required): API surface question

---

### 📦 6. `agentic_context_builder`

**What it does:** Autonomous comprehensive context gathering for code generation

**When to use:**
- "Gather context for implementing feature X"
- "What context do I need to modify Y?"
- Preparing to generate or modify code
- Understanding full context around a change

**How it works:**
- Gathers relevant code context
- Identifies related patterns
- Collects dependencies
- Synthesizes comprehensive picture

**Example:**
```javascript
agentic_context_builder("gather context for adding rate limiting to the API")

// Returns: Autonomous analysis including:
// - Existing middleware patterns
// - Where to hook into request processing
// - Related configuration points
// - Similar features for reference (caching, auth)
// - Integration points and dependencies
```

**Parameters:**
- `query` (required): Context gathering question

---

### ❓ 7. `agentic_semantic_question`

**What it does:** Autonomous complex codebase Q&A with semantic understanding

**When to use:**
- Complex questions requiring deep understanding
- "How does the system handle X?"
- "Explain the relationship between Y and Z"
- Questions spanning multiple systems

**How it works:**
- Semantically understands the question
- Autonomously explores relevant code
- Synthesizes answer from multiple sources
- Provides comprehensive explanation

**Example:**
```javascript
agentic_semantic_question("how does error handling work across the application, and what patterns are used?")

// Returns: Autonomous analysis including:
// - Error handling strategies identified
// - Pattern breakdown (Result types, try/catch, custom errors)
// - Consistency analysis across modules
// - Best practices observed
// - Anti-patterns or issues detected
```

**Parameters:**
- `query` (required): Semantic question about the codebase

---

## Decision Framework: Which Tool to Use?

### Quick Selection Guide

What do you need?

├─ 🔍 FIND & UNDERSTAND CODE
│  └─ agentic_code_search
│     "Where is X?" | "How does Y work?" | "Find code that does Z"
│
├─ 📊 ANALYZE DEPENDENCIES
│  └─ agentic_dependency_analysis
│     "What depends on X?" | "Impact of changing Y?" | "Coupling analysis"
│
├─ 🔗 TRACE EXECUTION
│  └─ agentic_call_chain_analysis
│     "Execution path from X to Y" | "Call chain for Z" | "Data flow"
│
├─ 🏗️ UNDERSTAND ARCHITECTURE
│  └─ agentic_architecture_analysis
│     "Architectural patterns?" | "Design analysis" | "Layer structure"
│
├─ 🌐 ANALYZE API
│  └─ agentic_api_surface_analysis
│     "Public API of X?" | "External interface" | "Breaking changes?"
│
├─ 📦 GATHER CONTEXT
│  └─ agentic_context_builder
│     "Context for implementing X" | "Before modifying Y"
│
└─ ❓ COMPLEX QUESTIONS
   └─ agentic_semantic_question
      "How does X relate to Y?" | "Explain Z across system"

### When in Doubt

**Default to `agentic_code_search`** for exploration, then use specialized tools for deeper analysis:

1. Start: `agentic_code_search` - Find and understand the code
2. Deepen: Use specialized tool based on what you learned
3. Synthesize: `agentic_semantic_question` for comprehensive understanding

---

## Common Workflows

### 🚀 Workflow 1: Implementing a New Feature

Task: "Add rate limiting middleware to the API"

Step 1: Understand existing patterns
  → agentic_architecture_analysis("how is middleware structured in this API?")

Step 2: Gather implementation context
  → agentic_context_builder("gather context for adding rate limiting middleware")

Step 3: Find similar features
  → agentic_code_search("find existing middleware implementations for reference")

Step 4: Analyze integration points
  → agentic_dependency_analysis("analyze middleware registration and hook points")

Step 5: Implement using gathered context
  → Now you have complete understanding without reading 50 files

**Context efficiency:** One tool call vs reading middleware/, config/, routes/ directories manually.

---

### 🐛 Workflow 2: Debugging an Issue

Task: "Users report authentication tokens expiring too quickly"

Step 1: Find the token logic
  → agentic_code_search("JWT token creation and expiration logic")

Step 2: Trace the execution path
  → agentic_call_chain_analysis("trace token generation from login to storage")

Step 3: Analyze dependencies
  → agentic_dependency_analysis("what depends on token expiration configuration?")

Step 4: Understand the system
  → agentic_semantic_question("how does token lifecycle management work?")

Step 5: Fix with full context
  → Now you understand the complete token system

**Context efficiency:** Complete understanding in 4 tool calls vs manually tracing through 20+ files.

---

### 📚 Workflow 3: Learning a New Codebase

Task: "Onboard to this React application"

Step 1: Architecture overview
  → agentic_architecture_analysis("analyze the overall application architecture")

Step 2: Entry points and flow
  → agentic_code_search("find application entry points and main routing")

Step 3: Key patterns
  → agentic_architecture_analysis("what design patterns are used throughout?")

Step 4: Module structure
  → agentic_dependency_analysis("analyze module dependencies and organization")

Step 5: API contracts
  → agentic_api_surface_analysis("what APIs and interfaces are exposed?")

Step 6: Deep dive areas
  → agentic_code_search for specific features you'll work on

**Context efficiency:** Comprehensive codebase map without reading hundreds of files.

---

### 🔄 Workflow 4: Refactoring Code

Task: "Extract database logic into repository layer"

Step 1: Find current implementation
  → agentic_code_search("find all direct database access in the codebase")

Step 2: Analyze impact
  → agentic_dependency_analysis("analyze dependencies of database access code")

Step 3: Check for patterns
  → agentic_architecture_analysis("analyze current data access patterns")

Step 4: Trace usage
  → agentic_call_chain_analysis("trace database query execution paths")

Step 5: Plan changes
  → agentic_context_builder("gather context for implementing repository pattern")

Step 6: Verify safety
  → agentic_api_surface_analysis("analyze public interfaces that will change")

Step 7: Refactor with confidence
  → Full impact understanding before touching any code

**Context efficiency:** Complete refactoring plan without manually mapping 100+ call sites.

---

## Best Practices

### ✅ DO:

1. **Ask natural language questions** - The agentic tools understand intent
   - ✅ "how does authentication integrate with the database?"
   - ✅ "what will break if I refactor the UserService?"

2. **Trust autonomous exploration** - Let CodeGraph decide which how to analyse the codebase
   - ✅ One tool call with comprehensive question
   - ❌ Not manually calling multiple tools

3. **Use specialized tools** - Each tool is optimized for specific analysis types
   - ✅ `agentic_dependency_analysis` for impact analysis
   - ❌ Not using `agentic_code_search` for everything

4. **Start broad, then narrow** - Begin with overview, drill down as needed
   - ✅ `agentic_architecture_analysis` → `agentic_code_search` for specifics
   - ❌ Not immediately searching for specific functions

### ❌ DON'T:

1. **Don't manually read files first** - Use CodeGraph tools to find relevant code
   - ❌ Reading src/auth/*.rs before understanding the system
   - ✅ `agentic_code_search("authentication system")` first

2. **Don't bypass agentic tools** - They provide autonomy and codebase intelligence
   - ❌ Trying to revert back to manual exploration when tool calls last long
   - ✅ Insights agent is doing your work for you it takes its time

3. **Don't ask overly narrow questions initially** - Start with context
   - ❌ "find the validateToken function"
   - ✅ "how does token validation work in the authentication system?"

4. **Don't ignore the autonomous results** - CodeGraph explored the codebase intelligently
   - ❌ Immediately searching for more after getting results
   - ✅ Reading and understanding what CodeGraph autonomously discovered

---

## Context Efficiency Comparison

### Manual Exploration (Old Way)

Task: "Understand authentication system"

Manual approach:
- Read auth/login.rs (300 lines)
- Read auth/session.rs (250 lines)
- Read middleware/auth_middleware.rs (400 lines)
- Read models/user.rs (500 lines)
- Read config/auth_config.rs (150 lines)
- Read database/user_repository.rs (350 lines)

Total: 1,950 lines read
Relevant: ~200 lines (10%)
Context burned: 90% waste
Time: 30+ minutes of reading

### Agentic Exploration (CodeGraph Way)

Task: "Understand authentication system"

Agentic approach:
→ agentic_architecture_analysis("analyze authentication system architecture")

Returns:
- Layered architecture breakdown
- Key components and their relationships
- Auth flow with code excerpts
- Design patterns identified
- Security considerations

Total: ~250 lines of RELEVANT content
Relevant: ~250 lines (100%)
Context efficiency: 87% reduction
Time: 5-10 seconds for complete understanding

**Result:** 7.8x more efficient with better understanding.

---

## Understanding the Autonomous Nature

### What "Agentic" Means

When you call an agentic tool, here's what happens behind the scenes:

1. **Your query is analyzed** by an insights agent
2. **The agent reasons** about what data it needs to answer the query
3. **Multi-step exploration** happens autonomously:
   - Search for relevant nodes
   - Follow dependency chains
   - Calculate metrics
   - Detect patterns
   - Trace call paths
4. **Results are synthesized** into a coherent answer
5. **Massive Context Savings** manual exploration is offloaded to a 2M ctx reasoning model
---

## Quick Start Checklist

When starting work on a codebase with CodeGraph:

- [ ] Use `agentic_architecture_analysis` to understand overall structure
- [ ] Use `agentic_code_search` for your first exploration question
- [ ] Let autonomous exploration work - don't immediately search for more
- [ ] Use specialized tools for specific analysis types
- [ ] Read the comprehensive results before making decisions
- [ ] Only read source files after CodeGraph narrows down locations
- [ ] Trust the multi-step reasoning - it explored the codebase intelligently

---

## Remember

**CodeGraph provides autonomous, graph-powered code intelligence.**

The goals:
1. **Autonomous exploration** - The insights agent orchestrates codebase analysis
2. **Context efficiency** - Get comprehensive understanding in minimal context
3. **Multi-step reasoning** - Complex analysis happens in one tool call

**Key principle:** Just ask your question in natural language. CodeGraph autonomously:
- Decides which graph functions to call
- Explores the dependency graph
- Synthesizes findings
- Provides comprehensive, context-efficient answers

**Default workflow:** Ask question → Read autonomous analysis → Make informed decisions

---

**Last Updated:** 2025-01-08
**Version:** 3.0.0 (Agentic Tools Edition)
**Requires:** `ai-enhanced` feature flag
