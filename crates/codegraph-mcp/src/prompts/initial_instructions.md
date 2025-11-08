# CodeGraph MCP - Initial Instructions for AI Agents

## Introduction

CodeGraph is a semantic code intelligence system that provides **condensed, context-aware information** about codebases through vector search, graph analysis, and LLM-powered insights.

**Critical principle:** Use CodeGraph tools FIRST before reading files manually. These tools give you targeted, semantically-relevant context without burning your context window on irrelevant code.

---

## Core Philosophy: Context Efficiency

### ❌ The Old Way (Context-Inefficient)
```
User: "How does authentication work in this codebase?"

Agent reads:
- auth/login.rs (300 lines)
- auth/session.rs (250 lines)
- middleware/auth_middleware.rs (400 lines)
- models/user.rs (500 lines)
- config/auth_config.rs (150 lines)

Total: 1,600 lines read, 95% irrelevant to the question
```

### ✅ The CodeGraph Way (Context-Efficient)
```
User: "How does authentication work in this codebase?"

Agent: enhanced_search("authentication flow and session management")

Returns:
- Relevant 20-line excerpts from 5 files
- AI explanation of the auth architecture
- Key functions and their relationships
- Entry points and integration patterns

Total: ~200 lines of RELEVANT context
```

**Context savings: 87% reduction with better understanding**

---

## When to Use CodeGraph Tools

### 🎯 ALWAYS Use CodeGraph Tools For:

**1. Discovering Code**
- ❓ "Where is X implemented?"
- ❓ "How does Y work?"
- ❓ "Find code that does Z"
- ✅ **Tool:** `enhanced_search` or `vector_search`

**2. Understanding Architecture**
- ❓ "What depends on this function?"
- ❓ "What calls this code?"
- ❓ "Trace this execution path"
- ✅ **Tool:** `graph_neighbors` or `graph_traverse`

**3. Analyzing Patterns**
- ❓ "What patterns does this codebase use?"
- ❓ "How are errors handled?"
- ❓ "What's the naming convention?"
- ✅ **Tool:** `pattern_detection`

**4. Finding Similar Code**
- ❓ "Where else do we do X?"
- ❓ "Find code similar to this snippet"
- ✅ **Tool:** `vector_search`

**5. Complex Questions**
- ❓ "Explain the data flow through the system"
- ❓ "How does feature X integrate with Y?"
- ✅ **Tool:** `codebase_qa` (if available)

### 🚫 DON'T Use Manual File Reading When:

- You're exploring unfamiliar code (**use `enhanced_search` first**)
- You need to understand how components connect (**use `graph_traverse`**)
- You're looking for a pattern across the codebase (**use `pattern_detection`**)
- You want to find similar implementations (**use `vector_search`**)
- You need high-level architecture understanding (**use `enhanced_search` + `graph_neighbors`**)

### ✅ Manual File Reading IS Appropriate When:

- CodeGraph returned the exact file/function you need and you want full implementation details
- You're making targeted edits to a specific file
- You need to see the exact line-by-line logic after CodeGraph narrowed down the location
- You're reviewing code that CodeGraph already identified as relevant

**Rule of thumb:** CodeGraph for FINDING and UNDERSTANDING, manual reading for DETAILS and EDITING.

---

## Available MCP Tools

### 🔍 Search & Discovery Tools

#### `enhanced_search` - Your Primary Discovery Tool
**What it does:** Semantic search with AI-powered analysis
**Speed:** 2-5 seconds
**Returns:** Ranked code excerpts + AI explanations of patterns and architecture

**When to use:**
- Starting any code exploration task
- Questions like "how does X work?"
- Finding implementations of features
- Understanding high-level architecture

**Example:**
```javascript
enhanced_search("JWT token validation and refresh logic")
// Returns: Relevant code + explanation of auth flow
```

**Parameters:**
- `query` (required): Natural language question or search term
- `limit` (optional): Number of results (default: 5)

---

#### `vector_search` - Fast Similarity Search
**What it does:** Fast vector-based code similarity matching
**Speed:** 0.5 seconds
**Returns:** Code snippets with similarity scores

**When to use:**
- Need quick results without AI analysis
- Finding code similar to a snippet
- "Where else do we do this?" questions
- Following up on `enhanced_search` for more examples

**Example:**
```javascript
vector_search("async error handling patterns", {
  paths: ["src/"],
  langs: ["rust"],
  limit: 10
})
```

**Parameters:**
- `query` (required): Search text or code snippet
- `paths` (optional): Filter by file paths (e.g., ["src/", "lib/"])
- `langs` (optional): Filter by language (e.g., ["rust", "typescript"])
- `limit` (optional): Number of results (default: 5)

---

#### `pattern_detection` - Codebase-Wide Pattern Analysis
**What it does:** Analyzes coding patterns, conventions, and team standards
**Speed:** 1-3 seconds
**Returns:** Naming conventions, organization patterns, error handling styles, quality metrics

**When to use:**
- Onboarding to a new codebase
- Understanding team conventions
- Before writing new code (to match existing patterns)
- Code review preparation

**Example:**
```javascript
pattern_detection()
// Returns: Full analysis of codebase patterns and conventions
```

**Parameters:** None

---

### 🗺️ Graph Analysis Tools

#### `graph_neighbors` - Direct Dependency Analysis
**What it does:** Find what directly calls or is called by a code element
**Speed:** 0.3 seconds
**Returns:** 1-hop dependency graph (immediate neighbors)

**When to use:**
- "What calls this function?"
- "What does this function call?"
- Quick impact assessment
- Understanding immediate relationships

**Example:**
```javascript
// First, get the node UUID from enhanced_search or vector_search
enhanced_search("AuthService.login method")
// Returns: {..., node_id: "uuid-1234"}

graph_neighbors("uuid-1234", { limit: 20 })
// Returns: All functions that call or are called by login()
```

**Parameters:**
- `node` (required): UUID of the code element (get from search results)
- `limit` (optional): Max neighbors to return (default: 20)

**Important:** You MUST get node UUIDs from search tools first!

---

#### `graph_traverse` - Deep Dependency Chain Analysis
**What it does:** Follow dependency chains through multiple levels
**Speed:** 0.5-2 seconds
**Returns:** Full dependency paths up to specified depth

**When to use:**
- Tracing execution flows
- Understanding transitive dependencies
- "What's the full call chain from X to Y?"
- Deep impact analysis

**Example:**
```javascript
// First, get the starting node UUID from search
vector_search("main entry point")
// Returns: {..., node_id: "uuid-5678"}

graph_traverse("uuid-5678", {
  depth: 3,
  limit: 20
})
// Returns: Dependency tree 3 levels deep from entry point
```

**Parameters:**
- `start` (required): UUID of starting node (get from search results)
- `depth` (optional): How many levels to traverse (default: 2)
- `limit` (optional): Max nodes to return (default: 20)

---

### 🤖 Advanced AI Tools (Feature-Gated)

#### `codebase_qa` - RAG-Powered Q&A
**What it does:** Answer complex questions using Retrieval-Augmented Generation
**Speed:** 5-30 seconds (SLOW)
**Returns:** AI-generated answer with citations from codebase

**When to use:**
- Complex architectural questions requiring deep context
- Questions that span multiple systems
- When enhanced_search isn't providing enough depth

**When NOT to use:**
- Simple "where is X" questions (use `enhanced_search` instead)
- Quick lookups (use `vector_search` instead)

**Example:**
```javascript
codebase_qa("How does the authentication system integrate with the database layer and what security measures are in place?", {
  max_results: 5,
  streaming: false
})
```

**Parameters:**
- `question` (required): Your question in natural language
- `max_results` (optional): Context chunks to retrieve (default: 5)
- `streaming` (optional): Stream response (default: false)

---

#### `code_documentation` - AI Documentation Generator
**What it does:** Generate comprehensive documentation for functions/classes
**Speed:** 10-45 seconds (VERY SLOW)
**Returns:** Full documentation with dependencies, usage patterns, examples

**When to use:**
- Creating comprehensive docs for complex code
- Understanding undocumented legacy code

**When NOT to use:**
- Quick understanding (use `enhanced_search` instead)
- Simple functions (write docs manually faster)

**Example:**
```javascript
code_documentation("AuthService.validateToken", {
  file_path: "src/auth/service.rs",
  style: "comprehensive"  // or "concise" or "tutorial"
})
```

**Parameters:**
- `target_name` (required): Function/class name to document
- `file_path` (optional): Path to file containing target
- `style` (optional): "comprehensive", "concise", or "tutorial" (default: "comprehensive")

---

## Decision Framework: Which Tool to Use?

### Quick Reference Tree

```
What do you need to do?
│
├─ 🔍 FIND CODE
│  ├─ "Where is X?" → enhanced_search
│  ├─ "Find similar code" → vector_search
│  └─ "Quick lookup" → vector_search
│
├─ 🧠 UNDERSTAND ARCHITECTURE
│  ├─ "How does X work?" → enhanced_search
│  ├─ "What patterns exist?" → pattern_detection
│  └─ "Complex question" → codebase_qa (if available)
│
├─ 🗺️ MAP DEPENDENCIES
│  ├─ "What calls this?" → graph_neighbors
│  ├─ "Trace call chain" → graph_traverse
│  └─ "Impact of change" → graph_neighbors + graph_traverse
│
└─ 📖 GENERATE DOCS
   └─ "Document this code" → code_documentation (if available)
```

### Tool Selection Gates

**Before calling ANY tool, ask yourself:**

1. **Have I used CodeGraph to explore this area yet?**
   - ❌ No → Start with `enhanced_search` or `vector_search`
   - ✅ Yes → Proceed to specific tool or manual reading

2. **Do I need AI insights or just code matches?**
   - 🧠 AI insights → `enhanced_search`
   - ⚡ Fast matches → `vector_search`

3. **Do I have a node UUID for graph operations?**
   - ❌ No → Run search first to get UUIDs
   - ✅ Yes → Use `graph_neighbors` or `graph_traverse`

4. **Will this answer require >30 seconds?**
   - ⏰ Yes → Consider if `codebase_qa` is worth it
   - ⚡ No → Use faster tools

---

## Common Workflows

### 🚀 Workflow 1: Implementing a New Feature

```
Task: "Add rate limiting to the API"

Step 1: Understand existing patterns
  → pattern_detection()

Step 2: Find similar features
  → enhanced_search("middleware and request handling patterns")

Step 3: Check dependencies
  → graph_neighbors("uuid-of-middleware-entry-point")

Step 4: Find similar implementations
  → vector_search("rate limiting or request throttling")

Step 5: NOW read specific files identified by CodeGraph
  → Read the 2-3 files CodeGraph identified as most relevant

Step 6: Implement feature using discovered patterns
```

**Context efficiency:** Used CodeGraph to go from 50 potential files to 2-3 relevant ones.

---

### 🐛 Workflow 2: Debugging an Issue

```
Task: "Auth tokens expire too quickly"

Step 1: Find the auth token logic
  → enhanced_search("token expiration and TTL configuration")

Step 2: Understand the flow
  → graph_traverse("uuid-of-token-creation", { depth: 2 })

Step 3: Find where it's configured
  → vector_search("token.expires_in or TTL configuration")

Step 4: Check for similar patterns
  → vector_search("session duration or timeout settings")

Step 5: Read the specific config file CodeGraph identified
  → Read the exact config file/function found

Step 6: Fix and verify
```

**Context efficiency:** Went straight to the problem without reading auth/*.

---

### 📚 Workflow 3: Learning a New Codebase

```
Task: "Understand this React application"

Step 1: Learn the patterns
  → pattern_detection()

Step 2: Understand architecture
  → enhanced_search("application architecture and main components")

Step 3: Map the entry points
  → enhanced_search("app entry point and routing structure")

Step 4: Explore key components
  → graph_neighbors("uuid-of-main-component")

Step 5: Find examples
  → vector_search("API data fetching patterns")

Step 6: Deep dive on specific areas only after mapping
  → Now read specific files for implementation details
```

**Context efficiency:** Built mental model before reading any complete files.

---

### 🔄 Workflow 4: Refactoring Code

```
Task: "Extract database logic into a repository layer"

Step 1: Find all database access
  → enhanced_search("database queries and data access patterns")

Step 2: Map dependencies
  → graph_neighbors("uuid-of-db-access-function")

Step 3: Analyze current patterns
  → pattern_detection()

Step 4: Check for existing abstractions
  → vector_search("repository pattern or data access layer")

Step 5: Trace impact
  → graph_traverse("uuid-of-db-access", { depth: 3 })

Step 6: Read specific files for refactoring
  → Read only the files that CodeGraph identified as using DB

Step 7: Implement repository layer following discovered patterns
```

**Context efficiency:** Knew exactly what to refactor before touching any files.

---

## Anti-Patterns: What NOT to Do

### ❌ Anti-Pattern 1: Reading Files First

**DON'T:**
```
User: "How does caching work?"
Agent: Let me read cache.rs, cache_manager.rs, redis.rs...
```

**DO:**
```
User: "How does caching work?"
Agent: enhanced_search("caching implementation and strategy")
→ Get condensed, relevant excerpts and AI explanation
→ THEN read specific files if needed
```

---

### ❌ Anti-Pattern 2: Ignoring Graph Tools for Dependencies

**DON'T:**
```
User: "What uses this function?"
Agent: Let me grep through all files for the function name...
```

**DO:**
```
User: "What uses this function?"
Agent:
  Step 1: enhanced_search("FunctionName")
  Step 2: graph_neighbors("uuid-from-search-result")
→ Get exact call graph with locations
```

---

### ❌ Anti-Pattern 3: Using Slow Tools for Simple Questions

**DON'T:**
```
User: "Where is the login function?"
Agent: codebase_qa("Where is the login function?")
→ Waits 20 seconds for simple lookup
```

**DO:**
```
User: "Where is the login function?"
Agent: vector_search("login function")
→ Get answer in 0.5 seconds
```

---

### ❌ Anti-Pattern 4: Not Getting UUIDs First

**DON'T:**
```
Agent: graph_neighbors("AuthService")  # WRONG - needs UUID not name
→ Error: Invalid node UUID
```

**DO:**
```
Agent:
  Step 1: enhanced_search("AuthService")
  Step 2: Extract UUID from result
  Step 3: graph_neighbors("uuid-1234")
→ Success
```

---

### ❌ Anti-Pattern 5: Reading When You Should Search

**DON'T:**
```
User: "Find all error handling code"
Agent: Let me read through every .rs file...
→ Burns 10,000 lines of context
```

**DO:**
```
User: "Find all error handling code"
Agent:
  Option 1: enhanced_search("error handling patterns and strategies")
  Option 2: pattern_detection() → See error handling conventions
  Option 3: vector_search("error handling or Result type usage")
→ Get condensed, relevant matches
```

---

## Context Efficiency Guidelines

### 📊 Estimated Context Costs

| Approach | Context Used | Relevance | Efficiency |
|----------|--------------|-----------|------------|
| Read 10 files manually | ~3,000 lines | ~10% relevant | ❌ Very Low |
| `enhanced_search` first | ~200 lines | ~80% relevant | ✅ Very High |
| `vector_search` first | ~150 lines | ~60% relevant | ✅ High |
| Read after CodeGraph | ~500 lines | ~95% relevant | ✅ High |

### 💡 Best Practices

1. **Always start with CodeGraph tools** - Don't guess which files to read
2. **Use fast tools for simple questions** - vector_search over codebase_qa
3. **Get UUIDs from search before graph operations** - Required workflow
4. **Chain tools logically** - Search → Graph → Read
5. **Read files only after narrowing down** - Use CodeGraph to filter

### 🎯 Success Metrics

You're using CodeGraph effectively if:
- ✅ You use search tools before reading files
- ✅ You extract UUIDs from search results for graph tools
- ✅ You choose the fastest appropriate tool
- ✅ You read fewer than 5 files to answer most questions
- ✅ You can explain architecture without reading complete files

You're NOT using CodeGraph effectively if:
- ❌ You read files before searching
- ❌ You try to use graph tools without UUIDs
- ❌ You use slow tools for simple questions
- ❌ You read 10+ files to understand a feature
- ❌ You ignore pattern_detection when onboarding

---

## Quick Start Checklist

When starting work on a codebase with CodeGraph:

- [ ] Run `pattern_detection()` to understand conventions
- [ ] Use `enhanced_search` for your first exploration question
- [ ] Extract node UUIDs from search results before using graph tools
- [ ] Chain tools: Search → Graph → Read (not Read → Read → Read)
- [ ] Only read complete files after CodeGraph narrows down locations
- [ ] Use fast `vector_search` for quick lookups, `enhanced_search` for understanding
- [ ] Remember: CodeGraph for FINDING, manual reading for EDITING

---

## Remember

**CodeGraph is your context-efficient codebase navigator.**

The goal is to:
1. **Find** relevant code quickly (search tools)
2. **Understand** architecture efficiently (enhanced_search + graph tools)
3. **Map** dependencies accurately (graph_neighbors + graph_traverse)
4. **Save** context for what matters (targeted reading)

**Default workflow:** Search → Graph → Read (not Read → Read → Read)

**When in doubt:** Start with `enhanced_search` - it's your Swiss Army knife.

---

**Last Updated:** 2025-01-08
**Version:** 2.0.0 (Correct Tools Edition)
