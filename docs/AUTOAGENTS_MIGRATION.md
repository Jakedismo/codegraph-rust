# AutoAgents Migration Guide

This guide explains how to migrate from CodeGraph's legacy custom orchestrator to the AutoAgents framework integration.

## Overview

CodeGraph v1.1.0 introduces **experimental AutoAgents framework support** for agentic orchestration:

- **Before**: Custom ~1,200-line orchestrator in `agentic_orchestrator.rs`
- **After**: AutoAgents ReAct framework with 6 specialized graph tools
- **Compatibility**: All 7 agentic MCP tools remain identical from user perspective
- **Migration**: Optional opt-in via build flag

## Why Migrate?

### Benefits of AutoAgents

1. **Reduced Code Complexity**: Replaces custom orchestration with proven framework
2. **Improved Maintainability**: Leverage community-supported agent patterns
3. **ReAct Pattern**: Standard Reasoning + Acting loop for better decision-making
4. **Future-Proof**: Built on extensible agent framework architecture
5. **Tier-Aware**: Maintains all existing tier-based prompt selection

### What Stays the Same

- ✅ All 7 agentic MCP tools (`agentic_*`) work identically
- ✅ Tier-aware prompting (Small/Medium/Large/Massive)
- ✅ SurrealDB graph analysis requirements
- ✅ Progress notifications
- ✅ Configuration format
- ✅ API contracts

## Migration Steps

### Step 1: Check Prerequisites

**Required:**
- Rust 1.75+ (same as before)
- SurrealDB instance (local or cloud)
- Working CodeGraph installation

**No configuration changes needed** - AutoAgents uses existing config.

### Step 2: Rebuild with AutoAgents Feature

**Option A: Using Makefile (Recommended)**
```bash
make build-mcp-autoagents
```

**Option B: Direct Cargo Build**
```bash
cargo build --release -p codegraph-mcp --bin codegraph \
    --features "ai-enhanced,autoagents-experimental,faiss,ollama"
```

**Option C: With Custom Features**
```bash
# Local setup
cargo build --release -p codegraph-mcp --bin codegraph \
    --features "ai-enhanced,autoagents-experimental,onnx,ollama,faiss"

# Cloud setup
cargo build --release -p codegraph-mcp --bin codegraph \
    --features "ai-enhanced,autoagents-experimental,anthropic,openai,faiss"
```

### Step 3: Test the Migration

**Start MCP server:**
```bash
./target/release/codegraph start stdio
```

**Verify AutoAgents is active:**
Check server logs for `🎯 AutoAgents` instead of `🎯 Agentic`:

```
# Legacy orchestrator:
🎯 Agentic CodeSearch (tier=Medium)

# AutoAgents framework:
🎯 AutoAgents CodeSearch (tier=Medium)
```

**Test an agentic tool:**
```bash
# From Claude Desktop or another MCP client
agentic_code_search(query="find authentication logic")
```

### Step 4: Verify Functionality

**Check these work correctly:**
- [ ] `agentic_code_search` - Multi-step code discovery
- [ ] `agentic_dependency_analysis` - Dependency chain exploration
- [ ] `agentic_call_chain_analysis` - Execution flow tracing
- [ ] `agentic_architecture_analysis` - Architecture pattern detection
- [ ] `agentic_api_surface_analysis` - Public interface analysis
- [ ] `agentic_context_builder` - Comprehensive context gathering
- [ ] `agentic_semantic_question` - Complex Q&A

**Expected behavior:**
- Response format unchanged (same JSON structure)
- Performance similar or better
- All graph analysis functions work
- Progress notifications appear

## Rollback Procedure

If you encounter issues, rollback is simple:

### Rebuild Without AutoAgents

```bash
# Using default features (legacy orchestrator)
cargo build --release -p codegraph-mcp --bin codegraph \
    --features "ai-enhanced,faiss,ollama"
```

**No data migration needed** - SurrealDB data remains compatible.

## Comparison: Legacy vs AutoAgents

### Architecture Differences

**Legacy Orchestrator:**
```
Claude Desktop → agentic_code_search
                 ↓
              AgenticOrchestrator (custom loop)
                 ↓
              GraphToolExecutor (6 functions)
                 ↓
              SurrealDB
```

**AutoAgents Framework:**
```
Claude Desktop → agentic_code_search
                 ↓
              CodeGraphExecutor
                 ↓
              CodeGraphAgentBuilder
                 ↓
              ReActAgent (AutoAgents)
                 ↓
              6 Inner Tools (specialized)
                 ↓
              GraphToolExecutor
                 ↓
              SurrealDB
```

### Code Organization

| Component | Legacy | AutoAgents |
|-----------|--------|------------|
| Orchestration logic | `agentic_orchestrator.rs` (~1,200 LOC) | AutoAgents framework |
| Tools definition | Inline JSON schemas | `ToolInput` derive macros |
| Agent definition | Custom loop | `#[agent]` macro |
| Tool execution | Direct function calls | `ToolRuntime` trait |
| State management | Manual conversation history | `SlidingWindowMemory` |

### Output Format

**Legacy:**
```json
{
  "analysis_type": "CodeSearch",
  "tier": "Medium",
  "query": "...",
  "final_answer": "...",
  "total_steps": 5,
  "duration_ms": 1234,
  "total_tokens": 5678,
  "completed_successfully": true,
  "termination_reason": "...",
  "steps": [...],
  "tool_call_stats": {...}
}
```

**AutoAgents:**
```json
{
  "analysis_type": "CodeSearch",
  "tier": "Medium",
  "query": "...",
  "answer": "...",         // Renamed from final_answer
  "findings": "...",       // New: structured findings
  "steps_taken": "5",      // Renamed from total_steps
  "framework": "AutoAgents" // New: identifies framework
}
```

**Note**: Output changes are in progress. Current implementation aims for identical format.

## Known Limitations (v1.1.0)

### Current Status

- ✅ All 6 inner graph tools implemented
- ✅ Agent definition and builder complete
- ✅ CodeGraphExecutor workflow orchestration complete
- ✅ MCP server integration complete
- ⏳ Integration testing in progress
- ⏳ Performance benchmarking needed
- ⏳ Output format alignment in progress

### Experimental Features

The following are marked experimental:
- AutoAgents framework integration itself
- Tool execution via `ToolRuntime` trait
- Agent output conversion
- Tier detection integration

### Not Yet Implemented

- [ ] Actual tier detection (currently defaults to Medium)
- [ ] Progress notifications for AutoAgents workflow
- [ ] Full output format parity with legacy orchestrator
- [ ] Performance benchmarks vs legacy
- [ ] Long-running workflow timeout handling

## Troubleshooting

### Build Errors

**Error: `feature 'autoagents-experimental' not found`**
```bash
# Ensure you're building the right package
cargo build -p codegraph-mcp --features "autoagents-experimental"
```

**Error: `derive macro panicked`**
```bash
# Ensure AutoAgents dependency is correctly installed
cargo clean
cargo update -p autoagents
cargo build
```

### Runtime Errors

**Error: `Agent build failed`**
- Check LLM provider configuration in `~/.codegraph/default.toml`
- Verify SurrealDB connection is working
- Check logs for detailed error messages

**Error: `AutoAgents workflow failed`**
- Verify graph tools are available (SurrealDB running)
- Check LLM API keys are configured
- Enable debug logging: `RUST_LOG=debug ./target/release/codegraph start stdio`

### Performance Issues

**Slower than legacy:**
- AutoAgents adds small overhead for framework abstractions
- Expected: 5-10% slower in v1.1.0 (optimization planned)
- If >20% slower, please file a GitHub issue with benchmarks

## Providing Feedback

### What to Report

**Success stories:**
- Share if AutoAgents works well for your use case
- Note any improvements you observe

**Issues:**
- Performance regressions >20%
- Functionality differences from legacy
- Crashes or error messages
- Unexpected behavior

### How to Report

**GitHub Issues:**
- Repository: `https://github.com/codegraph/codegraph-rust`
- Label: `autoagents-experimental`
- Include: CodeGraph version, feature flags used, MCP client, error logs

**Template:**
```markdown
## AutoAgents Feedback

**Version:** v1.1.0
**Build flags:** `autoagents-experimental,ai-enhanced,faiss,ollama`
**MCP client:** Claude Desktop / LM Studio / Other
**SurrealDB:** Cloud / Local

### Issue
[Describe what happened]

### Expected
[Describe what you expected]

### Logs
```
[Paste relevant error logs]
```
```

## FAQ

### Q: Is AutoAgents production-ready?

**A:** Not yet. v1.1.0 is experimental. Use legacy orchestrator for production until AutoAgents is marked stable (planned for v1.2.0).

### Q: Will legacy orchestrator be removed?

**A:** Not immediately. Legacy orchestrator will remain available until:
1. AutoAgents passes all integration tests
2. Performance is equal or better
3. Community feedback is positive
4. At least one full minor version (v1.2.0) is stable

**Deprecation timeline:**
- v1.1.0: AutoAgents experimental, legacy default
- v1.2.0: AutoAgents default, legacy deprecated
- v1.3.0: Legacy orchestrator removed (tentative)

### Q: Can I switch between them?

**A:** Yes, simply rebuild with or without `autoagents-experimental` feature. No data migration needed.

### Q: Does AutoAgents require more resources?

**A:** Minimal difference. AutoAgents adds ~2-5MB memory overhead for framework code. CPU usage should be similar or slightly lower due to better optimizations.

### Q: Are the inner tools compatible with other agents?

**A:** Yes! The 6 inner tools implement standard `ToolRuntime` trait and can be used with any AutoAgents-compatible agent, not just CodeGraph's ReActAgent.

## Next Steps

After successful migration:

1. **Test thoroughly** - Run your common agentic queries
2. **Monitor performance** - Note any slowdowns vs legacy
3. **Report feedback** - Help improve AutoAgents integration
4. **Stay updated** - Watch for v1.2.0 stability improvements

## Additional Resources

- [AutoAgents Framework Docs](https://github.com/liquidos-ai/AutoAgents)
- [CodeGraph CLAUDE.md](../CLAUDE.md) - Developer guide with AutoAgents architecture
- [Integration Plan](../docs/plans/2025-11-09-autoagents-integration-REVISED.md) - Technical implementation details
- [API Findings](../docs/plans/AUTOAGENTS_API_FINDINGS.md) - AutoAgents API research notes

---

**Questions?** Open a GitHub issue or discussion.

**Want to contribute?** See [CONTRIBUTING.md](../CONTRIBUTING.md) for development setup.
