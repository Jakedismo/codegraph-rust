# AutoAgents Tool Verification Against Schema

This document verifies that AutoAgents internal tools correctly map to GraphToolExecutor and SurrealDB schema functions.

## Analysis Date
2025-11-10

## Summary

**Status: 🔴 CRITICAL ISSUES FOUND**

Found 4 critical mismatches that will cause runtime failures and 3 minor inefficiencies.

---

## Tool-by-Tool Analysis

### ✅ 1. GetTransitiveDependencies - CORRECT

**AutoAgents Tool:**
```rust
// crates/codegraph-mcp/src/autoagents/tools/graph_tools.rs:11-21
struct GetTransitiveDependenciesArgs {
    node_id: String,
    edge_type: String,  // default: "Calls"
    depth: i32,         // default: 3
}
```

**Executor Call:**
```rust
// Line 50-63
execute_sync("get_transitive_dependencies", json!({
    "node_id": typed_args.node_id,
    "edge_type": typed_args.edge_type,
    "depth": typed_args.depth
}))
```

**Executor Implementation:**
```rust
// graph_tool_executor.rs:195-223
let node_id = params["node_id"].as_str()  // ✅
let edge_type = params["edge_type"].as_str()  // ✅
let depth = params["depth"].as_i64().unwrap_or(3) as i32  // ✅
```

**Database Schema:**
```sql
-- schema/codegraph.surql:260
DEFINE FUNCTION fn::get_transitive_dependencies(
    $node_id: string,    -- ✅
    $edge_type: string,  -- ✅
    $depth: int          -- ✅
)
```

**Verdict:** ✅ All parameters match correctly

---

### ✅ 2. GetReverseDependencies - CORRECT

**AutoAgents Tool:**
```rust
// graph_tools.rs:76-86
struct GetReverseDependenciesArgs {
    node_id: String,
    edge_type: String,  // default: "Calls"
    depth: i32,         // default: 3
}
```

**Executor Call:**
```rust
// Line 106-118
execute_sync("get_reverse_dependencies", json!({
    "node_id": typed_args.node_id,
    "edge_type": typed_args.edge_type,
    "depth": typed_args.depth
}))
```

**Executor Implementation:**
```rust
// graph_tool_executor.rs:313-338
let node_id = params["node_id"].as_str()  // ✅
let edge_type = params["edge_type"].as_str()  // ✅
let depth = params["depth"].as_i64().unwrap_or(3) as i32  // ✅
```

**Database Schema:**
```sql
-- schema/codegraph.surql:569
DEFINE FUNCTION fn::get_reverse_dependencies(
    $node_id: string,    -- ✅
    $edge_type: string,  -- ✅
    $depth: int          -- ✅
)
```

**Verdict:** ✅ All parameters match correctly

---

### 🔴 3. TraceCallChain - CRITICAL PARAMETER MISMATCH

**AutoAgents Tool:**
```rust
// graph_tools.rs:131-138
struct TraceCallChainArgs {
    start_node_id: String,  // ❌ WRONG NAME
    max_depth: i32,         // ✅ default: 5
}
```

**Executor Call:**
```rust
// Line 162-173
execute_sync("trace_call_chain", json!({
    "start_node_id": typed_args.start_node_id,  // ❌ WRONG KEY
    "max_depth": typed_args.max_depth           // ✅
}))
```

**Executor Implementation:**
```rust
// graph_tool_executor.rs:249-269
let from_node = params["from_node"].as_str()  // ❌ EXPECTS DIFFERENT KEY!
let max_depth = params["max_depth"].as_i64().unwrap_or(5) as i32  // ✅
```

**Database Schema:**
```sql
-- schema/codegraph.surql:391
DEFINE FUNCTION fn::trace_call_chain(
    $from_node: string,  -- ✅ Executor is correct
    $max_depth: int      -- ✅
)
```

**Issue:** Tool sends `"start_node_id"` but executor expects `"from_node"`

**Impact:** 🔴 Runtime error - executor will fail with "Missing from_node"

**Fix Required:** Change tool to use `from_node` instead of `start_node_id`

---

### 🔴 4. DetectCycles - CRITICAL FUNCTION NAME MISMATCH

**AutoAgents Tool:**
```rust
// graph_tools.rs:186-194
struct DetectCyclesArgs {
    edge_type: String,        // ✅ default: "Calls"
    max_cycle_length: i32,    // ⚠️ default: 10 (UNUSED)
}
```

**Executor Call:**
```rust
// Line 218-229
execute_sync("detect_cycles", json!({  // ❌ WRONG FUNCTION NAME
    "edge_type": typed_args.edge_type,
    "max_cycle_length": typed_args.max_cycle_length  // ⚠️ UNUSED BY EXECUTOR
}))
```

**Executor Implementation:**
```rust
// graph_tool_executor.rs:154-157
"detect_circular_dependencies" => {  // ❌ DIFFERENT NAME EXPECTED!
    self.execute_detect_circular_dependencies(parameters.clone()).await?
}

// Lines 226-245
let edge_type = params["edge_type"].as_str()  // ✅
// max_cycle_length is IGNORED
```

**Database Schema:**
```sql
-- schema/codegraph.surql:341
DEFINE FUNCTION fn::detect_circular_dependencies(  -- ✅ Executor is correct
    $edge_type: string  -- ✅
    -- Note: No max_cycle_length parameter exists
)
```

**Issues:**
1. 🔴 Tool calls `"detect_cycles"` but executor expects `"detect_circular_dependencies"`
2. ⚠️ Tool sends `max_cycle_length` parameter that doesn't exist in schema

**Impact:** 🔴 Runtime error - executor will reject unknown tool "detect_cycles"

**Fix Required:** Change tool call to `"detect_circular_dependencies"` and remove `max_cycle_length` param

---

### 🔴 5. CalculateCoupling - CRITICAL FUNCTION NAME MISMATCH

**AutoAgents Tool:**
```rust
// graph_tools.rs:242-249
struct CalculateCouplingArgs {
    node_id: String,      // ✅
    edge_type: String,    // ⚠️ default: "Calls" (UNUSED)
}
```

**Executor Call:**
```rust
// Line 269-280
execute_sync("calculate_coupling", json!({  // ❌ WRONG FUNCTION NAME
    "node_id": typed_args.node_id,
    "edge_type": typed_args.edge_type  // ⚠️ UNUSED BY EXECUTOR
}))
```

**Executor Implementation:**
```rust
// graph_tool_executor.rs:159-162
"calculate_coupling_metrics" => {  // ❌ DIFFERENT NAME EXPECTED!
    self.execute_calculate_coupling_metrics(parameters.clone()).await?
}

// Lines 273-290
let node_id = params["node_id"].as_str()  // ✅
// edge_type is IGNORED
```

**Database Schema:**
```sql
-- schema/codegraph.surql:426
DEFINE FUNCTION fn::calculate_coupling_metrics(  -- ✅ Executor is correct
    $node_id: string  -- ✅
    -- Note: No edge_type parameter exists
)
```

**Issues:**
1. 🔴 Tool calls `"calculate_coupling"` but executor expects `"calculate_coupling_metrics"`
2. ⚠️ Tool sends `edge_type` parameter that doesn't exist in schema

**Impact:** 🔴 Runtime error - executor will reject unknown tool "calculate_coupling"

**Fix Required:** Change tool call to `"calculate_coupling_metrics"` and remove `edge_type` param

---

### 🔴 6. GetHubNodes - CRITICAL PARAMETER NAME MISMATCH

**AutoAgents Tool:**
```rust
// graph_tools.rs:293-304
struct GetHubNodesArgs {
    edge_type: String,         // ⚠️ default: "Calls" (UNUSED)
    min_connections: i32,      // ❌ WRONG NAME, default: 5
    limit: i32,                // ⚠️ default: 20 (UNUSED)
}
```

**Executor Call:**
```rust
// Line 332-344
execute_sync("get_hub_nodes", json!({
    "edge_type": typed_args.edge_type,              // ⚠️ UNUSED
    "min_connections": typed_args.min_connections,  // ❌ WRONG KEY
    "limit": typed_args.limit                       // ⚠️ UNUSED
}))
```

**Executor Implementation:**
```rust
// graph_tool_executor.rs:294-309
let min_degree = params["min_degree"].as_i64().unwrap_or(5) as i32  // ❌ EXPECTS DIFFERENT KEY!
// edge_type and limit are IGNORED
```

**Database Schema:**
```sql
-- schema/codegraph.surql:482
DEFINE FUNCTION fn::get_hub_nodes(
    $min_degree: int  -- ✅ Executor is correct
    -- Note: No edge_type or limit parameters exist
)
```

**Issues:**
1. 🔴 Tool sends `"min_connections"` but executor expects `"min_degree"`
2. ⚠️ Tool sends unused `edge_type` parameter
3. ⚠️ Tool sends unused `limit` parameter

**Impact:** 🔴 Silent failure - will always use default value of 5 instead of LLM-provided value

**Fix Required:**
- Change tool to use `min_degree` instead of `min_connections`
- Remove `edge_type` and `limit` params

---

## Summary of Required Fixes

### Critical Fixes (Runtime Failures)

1. **TraceCallChain (graph_tools.rs:170)**
   ```diff
   - "start_node_id": typed_args.start_node_id,
   + "from_node": typed_args.start_node_id,
   ```
   Also rename struct field from `start_node_id` to `from_node` (line 134)

2. **DetectCycles (graph_tools.rs:224)**
   ```diff
   - "detect_cycles",
   + "detect_circular_dependencies",
   ```

3. **CalculateCoupling (graph_tools.rs:275)**
   ```diff
   - "calculate_coupling",
   + "calculate_coupling_metrics",
   ```

4. **GetHubNodes (graph_tools.rs:340)**
   ```diff
   - "min_connections": typed_args.min_connections,
   + "min_degree": typed_args.min_connections,
   ```
   Also rename struct field from `min_connections` to `min_degree` (line 300)

### Minor Fixes (Wasted LLM Context)

5. **DetectCycles (graph_tools.rs:186-194, 227)**
   - Remove `max_cycle_length` field from args struct
   - Remove from JSON payload

6. **CalculateCoupling (graph_tools.rs:242-249, 278)**
   - Remove `edge_type` field from args struct
   - Remove from JSON payload

7. **GetHubNodes (graph_tools.rs:293-304, 340-342)**
   - Remove `edge_type` field from args struct
   - Remove `limit` field from args struct
   - Remove both from JSON payload

---

## Verification Checklist

After fixes are applied:

- [ ] All 6 tools use correct function names matching executor
- [ ] All parameter names match what executor expects
- [ ] No unused parameters are sent to executor
- [ ] Tool descriptions accurately reflect what parameters are used
- [ ] Integration tests pass with real SurrealDB backend
- [ ] LLM can successfully execute all 6 graph analysis tools

---

## Files Requiring Changes

**Primary file:**
- `crates/codegraph-mcp/src/autoagents/tools/graph_tools.rs`
  - Lines 134, 170 (TraceCallChain)
  - Lines 186-194, 224, 227 (DetectCycles)
  - Lines 242-249, 275, 278 (CalculateCoupling)
  - Lines 293-304, 340-342 (GetHubNodes)

**Reference files (no changes needed):**
- `crates/codegraph-mcp/src/graph_tool_executor.rs` (correct)
- `schema/codegraph.surql` (correct)
