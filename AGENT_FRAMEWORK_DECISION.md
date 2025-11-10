# CodeGraph Agent Framework Decision Guide

**Status**: Research Complete - Ready for Decision  
**Last Updated**: 2025-11-09  
**Research Scope**: 8 production-ready Rust LLM agent frameworks analyzed

---

## TL;DR - The Recommendation

### 🏆 Primary Choice: **AutoAgents**
- **Best fit** for CodeGraph's ReAct pattern (Reason → Act → Observe loops)
- **Eliminates 900+ lines** of custom orchestrator + prompts code
- **Type-safe tool definitions** via macros (vs hand-rolled JSON schemas)
- **WASM sandboxing** for tool safety
- **Active development** with good architectural patterns

**Decision Point**: Use if you prioritize clean architecture and are willing to adopt a newer framework (medium risk).

### 🥈 Alternative: **Rig**
- **Highest production maturity** (used by St Jude, major companies)
- **Most documentation** and examples
- **20+ LLM providers** built-in
- **Battle-tested** over years

**Decision Point**: Use if you want maximum stability over elegance (lower risk but more code).

---

## Quick Comparison of Top Contenders

| Criterion | AutoAgents | Rig | Kowalski | axiom-ai-agents |
|-----------|-----------|-----|---------|-----------------|
| **Fit for CodeGraph** | ⭐⭐⭐⭐⭐ | ⭐⭐⭐⭐ | ⭐⭐⭐ | ⭐⭐⭐⭐ |
| **Production Ready** | ✅ High | ✅✅ Highest | ✅ Medium-High | ✅ Medium |
| **Code Reduction** | 900 lines | 500 lines | 600 lines | 800 lines |
| **Learning Curve** | 2-3 days | 2-3 days | 3-4 days | 3-4 days |
| **Maturity Risk** | Medium | Low | Low | Medium-High |
| **Provider Ecosystem** | Good | Excellent | Good | Good |
| **Documentation** | Good | Excellent | Scattered | Evolving |
| **Community** | Growing | Large | Growing | Small |

---

## Current Implementation Breakdown

**Total Custom Agentic Code**: ~1,200 lines

- `agentic_orchestrator.rs` - 627 lines (main orchestrator)
- `agentic_api_surface_prompts.rs` - 358 lines (4 tier variants)
- `code_search_prompts.rs` - 413 lines  
- `call_chain_prompts.rs` - 403 lines
- Tool schema definitions - 252 lines
- Supporting: `graph_tool_executor.rs` (100 lines), error handling, etc.

**Complexity Hotspots**:
1. Manual step-by-step loop (150 lines)
2. Hand-rolled JSON tool call parsing (80 lines)
3. Message/conversation state management (100 lines)
4. 28 prompt variants (7 workflows × 4 tiers) manually maintained
5. Tier detection & token budgeting (60 lines)

---

## What Would Framework Adoption Look Like?

### Code Reduction

| Component | Now | With AutoAgents | Savings |
|-----------|-----|-----------------|---------|
| Orchestration | 627 | 0 | -627 |
| Prompt variants | 358 | ~50 (framework handles) | -308 |
| Tool schemas | 252 | ~20 (macros) | -232 |
| Conversation mgmt | 100 | 0 | -100 |
| Tier configuration | 60 | ~30 | -30 |
| **Subtotal** | **1,397** | **~100** | **-1,297** |

### Immediate Benefits

1. ✅ **Fewer bugs**: Proven, tested orchestration logic
2. ✅ **Easier maintenance**: No custom state machine
3. ✅ **Better safety**: Type-safe tool definitions
4. ✅ **Faster feature development**: Less boilerplate
5. ✅ **WASM sandboxing**: Better tool safety guarantees

### Risks

| Risk | Likelihood | Mitigation |
|------|-----------|-----------|
| AutoAgents breaking changes | Medium | Pin versions, keep compatibility shim |
| Ractor learning curve | Low | Good examples, simple API |
| Performance regression | Low | Benchmark before deploy |
| Community size | Low | Framework feature-complete for your needs |

---

## Framework Capabilities vs Current

### What You Keep ✅
- 4-tier context-aware prompting (can be reimplemented as plugin)
- LLM provider abstraction (both frameworks support multiple providers)
- Tool result caching (can be added as wrapper)
- Progress notifications (supported by AutoAgents natively)
- Token tracking (native in both)

### What You Gain ✅
- Automatic conversation history management
- Built-in multi-step reasoning orchestration
- Type-safe tool definitions (AutoAgents)
- WASM sandbox execution (AutoAgents)
- Better error handling and recovery
- Streaming support (native)

### What You Lose ❌
- Custom 4-tier prompt tuning (framework doesn't understand your tiers)
  - *Mitigation*: Implement as post-hook or custom prompt prefix

---

## Migration Strategy

### Option A: AutoAgents (Recommended)
```
Week 1: Evaluate & PoC
├─ Clone repo, study ReAct pattern
├─ Implement 1 tool (agentic_code_search)
└─ Compare with current output

Week 2-3: Prototype
├─ Add AutoAgents to Cargo.toml
├─ Implement 2-3 agentic tools
└─ Parallel test with agentic_tools.py

Week 4-6: Gradual Migration
├─ Migrate tools one-by-one with feature flags
├─ Keep fallback to old implementation
└─ Collect performance data

Week 7: Consolidation
├─ Remove old code
├─ Update docs
└─ Deploy & monitor
```

### Option B: Rig (Conservative)
```
Similar timeline but:
- Slightly longer prototyping (explicit tool loop needed)
- More integration code
- Faster stability (proven ecosystem)
```

---

## Questions to Ask Before Deciding

1. **How risk-tolerant are we?**
   - Risk-averse → Rig
   - Willing to adopt newer → AutoAgents

2. **What's more important?**
   - Code cleanliness & type safety → AutoAgents
   - Maximum stability & community → Rig

3. **Do we need the tier-aware prompting?**
   - Yes → Custom plugin on top of framework
   - No → Use framework as-is

4. **Future roadmap?**
   - Multi-agent federation? → AutoAgents (built for it)
   - Sandboxed execution? → AutoAgents (WASM native)
   - General agentic tasks? → Rig (safer bet)

---

## Next Steps

### If choosing AutoAgents:
1. Read: https://github.com/liquidos-ai/AutoAgents
2. Study: ReAct executor implementation
3. Create PoC: Single agentic tool implementation
4. Benchmark: Compare against current (latency, tokens, correctness)

### If choosing Rig:
1. Read: https://rig.rs and https://github.com/0xPlaygrounds/rig
2. Study: Multi-turn conversation examples
3. Create PoC: Tool calling pattern
4. Benchmark: Compare against current

### Either way:
- Keep current code as fallback initially
- Use feature flags for gradual migration
- Measure before & after metrics
- Document lessons learned

---

## References

- Full analysis: See `RUST_AGENT_FRAMEWORKS_ANALYSIS.md` (comprehensive 450+ line reference)
- AutoAgents GitHub: https://github.com/liquidos-ai/AutoAgents
- Rig GitHub: https://github.com/0xPlaygrounds/rig
- Kowalski GitHub: https://github.com/yarenty/kowalski

---

**Recommendation Created By**: AI Research  
**For Discussion With**: Jokke  
**Status**: Ready for architectural decision
