# MCP Server Tools - Audit & Improvements

## Overview

This document audits all MCP server tools for:
1. **Insight Quality**: Do they generate actionable insights or just return raw data?
2. **Performance**: Do they execute quickly or risk timing out?
3. **Documentation**: Are descriptions clear and actionable?
4. **Agent Value**: Do they help agents accomplish tasks effectively?

## Current Tools (9 Active)

### 1. ✅ `enhanced_search` - GOOD (with improvements needed)

**Current Description:**
> "Search your codebase with AI analysis. Finds code patterns, architectural insights, and team conventions. Use when you need intelligent analysis of search results. Required: query (what to search for). Optional: limit (max results, default 10)."

**What it returns:**
- Search results (file paths, code snippets, similarity scores)
- AI analysis with insights
- Generation guidance
- Quality assessment
- Performance metrics

**Issues:**
- ⚠️ No timeout on Qwen API calls (could hang)
- ⚠️ Default limit=10, but fetches limit*2=20 results (inefficient)
- ⚠️ Description too short - doesn't explain value

**Improvements Needed:**
1. Add 30-second timeout to Qwen calls
2. Reduce default limit to 5, fetch limit*1.5 instead of limit*2
3. Improve description to explain when to use vs vector_search

**Estimated Time:** 2-5 seconds (with timeout)
**Value:** ⭐⭐⭐⭐⭐ (Excellent - provides AI insights)

---

### 2. ✅ `pattern_detection` - EXCELLENT

**Current Description:**
> "Analyze your team's coding patterns and conventions. Detects naming conventions, code organization patterns, error handling styles, and quality metrics. Use to understand team standards or onboard new developers. No parameters required."

**What it returns:**
- Detected patterns (naming, architecture, error handling)
- Team conventions
- Quality metrics
- Recommended patterns
- Patterns to avoid
- Actionable insights

**Issues:**
- ✅ No AI calls - uses semantic analysis only (fast)
- ✅ Returns structured insights
- ⚠️ Fetches 50 results by default (could be heavy)

**Improvements Needed:**
1. Reduce default max_results to 30
2. Add caching for pattern detection (patterns don't change often)

**Estimated Time:** 1-3 seconds
**Value:** ⭐⭐⭐⭐⭐ (Excellent - unique insights, no external dependencies)

---

### 3. ⚠️ `vector_search` - NEEDS IMPROVEMENT

**Current Description:**
> "Fast vector similarity search to find code similar to your query. Returns raw search results without AI analysis (faster than enhanced_search). Use for quick code discovery. Required: query (what to find). Optional: paths (filter by directories), langs (filter by languages), limit (max results, default 10)."

**What it returns:**
- Raw JSON with:
  - Node IDs
  - File paths
  - Node names
  - Similarity scores
  - Code snippets (truncated)

**Issues:**
- ❌ Returns raw JSON - no insights or summaries
- ❌ Doesn't explain WHY results are relevant
- ❌ No grouping by file or pattern
- ⚠️ Default limit=10 might be too many for agents to process

**Improvements Needed:**
1. Add result summarization (group by file, highlight common patterns)
2. Add relevance explanations ("This matches because...")
3. Reduce default limit to 5
4. Format output for agent consumption (structured insights)

**Example Improvement:**
```json
{
  "query": "authentication",
  "total_results": 5,
  "summary": "Found authentication code across 3 files: auth middleware (2 results), user service (2 results), and JWT helper (1 result)",
  "grouped_by_file": {
    "src/middleware/auth.rs": {
      "result_count": 2,
      "relevance": "High - core authentication logic",
      "results": [...]
    }
  },
  "insights": [
    "Primary authentication is in middleware/auth.rs",
    "JWT token validation is centralized",
    "Uses bcrypt for password hashing"
  ]
}
```

**Estimated Time:** 0.5-1 second
**Value:** ⭐⭐⭐ (Good speed, but lacks insights)

---

### 4. ⚠️ `graph_neighbors` - NEEDS IMPROVEMENT

**Current Description:**
> "Find all code that depends on or is used by a specific code element. Shows dependencies, imports, and relationships. Use to understand code impact before refactoring. Required: node (UUID from search results). Optional: limit (max results, default 20). Note: Get node UUIDs from vector_search or enhanced_search results."

**What it returns:**
- Raw list of neighbor nodes:
  - ID, name, file path, node type, language

**Issues:**
- ❌ Returns raw node data - no relationship insights
- ❌ Doesn't explain dependency types (import vs call vs inheritance)
- ❌ No impact summary
- ❌ Requires UUID (not intuitive - should accept function name)

**Improvements Needed:**
1. Add relationship type annotations ("imports", "calls", "extends")
2. Provide impact summary ("3 files import this, 5 functions call it")
3. Group by relationship type
4. Add safety guidance ("High impact - used by 10+ files")

**Example Improvement:**
```json
{
  "target": {
    "name": "authenticate_user",
    "file": "src/auth/service.rs"
  },
  "impact_summary": {
    "total_dependents": 8,
    "direct_callers": 5,
    "importers": 3,
    "risk_level": "HIGH"
  },
  "relationships": {
    "called_by": [
      {"name": "login_handler", "file": "src/api/handlers.rs", "type": "direct_call"},
      {"name": "refresh_token", "file": "src/api/handlers.rs", "type": "direct_call"}
    ],
    "imported_by": [
      {"file": "src/middleware/auth.rs", "type": "module_import"}
    ]
  },
  "refactoring_guidance": "⚠️ HIGH IMPACT: This function is called by 5 handlers and imported by 3 modules. Changes will affect multiple API endpoints. Consider: (1) versioning, (2) deprecation period, (3) comprehensive testing."
}
```

**Estimated Time:** 0.2-0.5 seconds
**Value:** ⭐⭐ (Fast but lacks actionable insights)

---

### 5. ⚠️ `graph_traverse` - NEEDS IMPROVEMENT

**Current Description:**
> "Follow dependency chains through your codebase to understand architectural flow and code relationships. Use to trace execution paths or understand system architecture. Required: start (UUID from search results). Optional: depth (how far to traverse, default 2), limit (max results, default 100). Note: Get start UUIDs from vector_search or enhanced_search results."

**What it returns:**
- List of nodes with depth info
- Raw traversal data

**Issues:**
- ❌ Returns 100 results by default (way too much for agents!)
- ❌ No flow visualization or summary
- ❌ No architectural insights
- ❌ Doesn't explain the dependency chain

**Improvements Needed:**
1. Reduce default limit to 20
2. Reduce default depth to 1 (one hop away)
3. Add dependency chain visualization
4. Add architectural insights
5. Group by depth level

**Example Improvement:**
```json
{
  "start": {
    "name": "handle_request",
    "file": "src/api/handler.rs"
  },
  "traversal_summary": {
    "total_nodes": 12,
    "max_depth_reached": 2,
    "architecture_pattern": "Layered architecture: Handler → Service → Repository"
  },
  "dependency_chain": [
    {
      "depth": 0,
      "node": "handle_request",
      "file": "src/api/handler.rs",
      "description": "Entry point - HTTP handler"
    },
    {
      "depth": 1,
      "nodes": [
        {"name": "authenticate_user", "file": "src/auth/service.rs", "description": "Authentication service"},
        {"name": "get_user_data", "file": "src/user/service.rs", "description": "User data service"}
      ]
    },
    {
      "depth": 2,
      "nodes": [
        {"name": "query_database", "file": "src/db/repository.rs", "description": "Database layer"}
      ]
    }
  ],
  "architectural_insights": [
    "Clean separation of concerns: API → Business Logic → Data Access",
    "No circular dependencies detected",
    "Follows dependency inversion principle"
  ],
  "recommended_action": "This function follows good architectural patterns. Safe to refactor within its layer."
}
```

**Estimated Time:** 0.5-2 seconds (depending on depth)
**Value:** ⭐⭐ (Could be great with proper insights)

---

### 6. ⚠️ `codebase_qa` - GOOD but SLOW

**Current Description:**
> "Ask natural language questions about the codebase and get intelligent, cited responses. Uses hybrid retrieval (vector search + graph traversal + keyword matching) with AI generation. Provides streaming responses with source citations and confidence scoring. Examples: 'How does authentication work?', 'Explain the data flow', 'What would break if I change this function?'. Required: question (natural language query). Optional: max_results (default 10), streaming (default false)."

**What it returns:**
- AI-generated answer
- Source citations
- Confidence scores
- Context used

**Issues:**
- ⚠️ Description is way too long (136 words!)
- ⚠️ No timeout on AI generation (could hang indefinitely)
- ⚠️ Fetches 10 results by default for RAG (heavy)
- ⚠️ Could take 10-30 seconds with large context

**Improvements Needed:**
1. Shorten description to 1-2 sentences max
2. Add 45-second timeout
3. Reduce default max_results to 5
4. Add progress indication
5. Consider disabling by default (too slow for many use cases)

**Estimated Time:** 5-30 seconds ⏰
**Value:** ⭐⭐⭐⭐ (High value but slow - use sparingly)

---

### 7. ⚠️ `code_documentation` - GOOD but SLOW

**Current Description:**
> "Generate comprehensive documentation for functions, classes, or modules using AI analysis with graph context. Analyzes dependencies, usage patterns, and architectural relationships to create intelligent documentation with source citations. Required: target_name (function/class/module name). Optional: file_path (focus scope), style (comprehensive/concise/tutorial)."

**What it returns:**
- AI-generated documentation
- Dependency analysis
- Usage examples
- Architectural context

**Issues:**
- ⚠️ No timeout on AI generation
- ⚠️ Fetches 15 results + 3-hop neighbor expansion (VERY heavy)
- ⚠️ Could take 15-45 seconds
- ⚠️ No caching

**Improvements Needed:**
1. Add 45-second timeout
2. Reduce to 10 results + 2-hop neighbors
3. Add caching (docs don't change often)
4. Consider making async/background task

**Estimated Time:** 10-45 seconds ⏰⏰
**Value:** ⭐⭐⭐⭐ (High value but very slow - use sparingly)

---

### 8. ⚠️ `semantic_intelligence` - SLOWEST

**Current Description:**
> "Perform deep architectural analysis of your entire codebase using AI. Explains system design, component relationships, and overall architecture. Use for understanding large codebases or documenting architecture. Required: query (analysis focus). Optional: task_type (analysis type, default 'semantic_search'), max_context_tokens (AI context limit, default 80000)."

**What it returns:**
- Comprehensive AI analysis
- Architectural insights
- System design explanations
- Component relationships

**Issues:**
- ❌ Uses 80,000 context tokens by default (MASSIVE!)
- ❌ No timeout (could take minutes)
- ❌ Extremely expensive operation
- ❌ Not suitable for real-time agent use
- ❌ Should be async/background only

**Improvements Needed:**
1. **CRITICAL**: Add 60-second timeout
2. Reduce default max_context_tokens to 20,000
3. Add warning in description about performance
4. Consider making this a background/async task only
5. Add progress indication
6. Cache aggressively

**Estimated Time:** 30-120 seconds ⏰⏰⏰
**Value:** ⭐⭐⭐⭐⭐ (Extremely high value but WAY too slow for sync use)

---

### 9. ⚠️ `impact_analysis` - GOOD but could be SLOW

**Current Description:**
> "Predict the impact of modifying a specific function or class. Shows what code depends on it and might break. Use before refactoring to avoid breaking changes. Required: target_function (function/class name), file_path (path to file containing it). Optional: change_type (type of change, default 'modify')."

**What it returns:**
- Impact prediction
- Dependent code
- Risk assessment
- Refactoring recommendations

**Issues:**
- ⚠️ Uses AI for analysis (could be slow)
- ⚠️ No timeout
- ⚠️ No performance estimate in description

**Improvements Needed:**
1. Add 30-second timeout
2. Add performance warning to description
3. Consider hybrid approach (graph analysis + optional AI)

**Estimated Time:** 3-15 seconds
**Value:** ⭐⭐⭐⭐ (High value, moderate speed)

---

## Summary of Issues

### Critical Issues (Fix Immediately)

1. **No Timeouts**: All AI tools lack timeouts - could hang indefinitely
   - `enhanced_search`: needs 30s timeout
   - `codebase_qa`: needs 45s timeout
   - `code_documentation`: needs 45s timeout
   - `semantic_intelligence`: needs 60s timeout
   - `impact_analysis`: needs 30s timeout

2. **semantic_intelligence is TOO SLOW**: 80K context tokens = 30-120 seconds
   - Should be async/background only
   - Or reduce to 20K tokens max
   - Add big warning in description

3. **Raw Data Tools Lack Insights**:
   - `vector_search`: needs result summarization
   - `graph_neighbors`: needs relationship insights
   - `graph_traverse`: needs architectural insights

### High Priority Issues

4. **Excessive Default Limits**:
   - `graph_traverse`: 100 results → reduce to 20
   - `codebase_qa`: 10 results → reduce to 5
   - `code_documentation`: 15 results + 3 hops → reduce to 10 results + 2 hops
   - `pattern_detection`: 50 results → reduce to 30

5. **Tool Descriptions Too Long**:
   - `codebase_qa`: 136 words → max 40 words
   - Others: Should be 20-30 words max

6. **Missing Performance Indicators**:
   - Should indicate estimated time in description
   - Should warn about slow operations

### Medium Priority Issues

7. **No Result Caching**:
   - `pattern_detection`: patterns don't change often
   - `code_documentation`: docs don't change often
   - Should cache results for 5-10 minutes

8. **Poor Agent UX**:
   - Tools return UUIDs instead of names (hard to use)
   - Raw JSON not formatted for agent consumption
   - No usage examples in descriptions

## Recommended Tool Usage Matrix

| Tool | Speed | Insight Quality | When to Use | When NOT to Use |
|------|-------|-----------------|-------------|-----------------|
| `vector_search` | ⚡⚡⚡ Fast (0.5s) | ⭐⭐⭐ Good | Quick code discovery, finding similar code | When you need explanation of WHY code is relevant |
| `enhanced_search` | ⚡⚡ Medium (2-5s) | ⭐⭐⭐⭐⭐ Excellent | Understanding code patterns, getting insights | Simple lookups (use vector_search) |
| `pattern_detection` | ⚡⚡⚡ Fast (1-3s) | ⭐⭐⭐⭐⭐ Excellent | Understanding team conventions, onboarding | Analyzing specific code (use enhanced_search) |
| `graph_neighbors` | ⚡⚡⚡ Fast (0.3s) | ⭐⭐ Limited | Finding direct dependencies | Understanding impact (use impact_analysis) |
| `graph_traverse` | ⚡⚡ Medium (0.5-2s) | ⭐⭐ Limited | Tracing execution flow | Deep analysis (needs improvements) |
| `codebase_qa` | ⏰ Slow (5-30s) | ⭐⭐⭐⭐ High | Answering complex questions | Quick lookups, simple queries |
| `code_documentation` | ⏰⏰ Very Slow (10-45s) | ⭐⭐⭐⭐ High | Generating comprehensive docs | Quick reference, exploring |
| `semantic_intelligence` | ⏰⏰⏰ Extremely Slow (30-120s) | ⭐⭐⭐⭐⭐ Excellent | Deep architectural analysis | Real-time queries, agent workflows |
| `impact_analysis` | ⏰ Slow (3-15s) | ⭐⭐⭐⭐ High | Pre-refactoring impact assessment | Quick dependency checks |

## Proposed Improvements

### Phase 1: Critical Fixes (Immediate)

1. **Add Timeouts to All AI Tools**
   - `enhanced_search`: 30s
   - `codebase_qa`: 45s
   - `code_documentation`: 45s
   - `semantic_intelligence`: 60s (or disable for sync use)
   - `impact_analysis`: 30s

2. **Reduce Default Limits**
   - `enhanced_search`: limit*2 → limit*1.5
   - `graph_traverse`: 100 → 20
   - `codebase_qa`: 10 → 5
   - `code_documentation`: 15 + 3 hops → 10 + 2 hops
   - `semantic_intelligence`: 80K tokens → 20K tokens
   - `pattern_detection`: 50 → 30

3. **Fix semantic_intelligence**
   - Add WARNING in description about performance
   - Reduce default context
   - Consider async-only mode

### Phase 2: Insight Improvements (High Priority)

4. **Enhance vector_search Output**
   - Add result summarization
   - Group by file
   - Add relevance explanations
   - Format for agent consumption

5. **Enhance graph_neighbors Output**
   - Add relationship types
   - Add impact summary
   - Add refactoring guidance

6. **Enhance graph_traverse Output**
   - Add dependency chain visualization
   - Add architectural insights
   - Reduce default limit and depth

### Phase 3: UX Improvements (Medium Priority)

7. **Shorten Tool Descriptions**
   - Max 40 words
   - Focus on value proposition
   - Add performance indicators

8. **Add Caching**
   - `pattern_detection`: 10min cache
   - `code_documentation`: 5min cache
   - `semantic_intelligence`: 15min cache

9. **Improve Parameter Handling**
   - Accept function names instead of UUIDs
   - Better default values
   - Clear parameter documentation

## Implementation Priority

### Must Fix (P0):
- [ ] Add timeouts to all AI operations
- [ ] Fix semantic_intelligence performance (reduce to 20K tokens)
- [ ] Reduce default limits on all tools

### Should Fix (P1):
- [ ] Enhance vector_search with insights
- [ ] Enhance graph_neighbors with insights
- [ ] Enhance graph_traverse with insights
- [ ] Shorten tool descriptions

### Nice to Have (P2):
- [ ] Add caching to pattern_detection
- [ ] Add caching to code_documentation
- [ ] Accept function names instead of UUIDs
- [ ] Add progress indicators

## Conclusion

CodeGraph MCP tools provide excellent functionality but need:

1. **Performance safeguards**: Timeouts and reduced defaults
2. **Better insights**: Transform raw data into actionable intelligence
3. **Clearer documentation**: Shorter descriptions with value propositions
4. **Agent-friendly output**: Structured insights instead of raw JSON

Fixing these issues will make CodeGraph MCP tools much more valuable and reliable for agent workflows.
