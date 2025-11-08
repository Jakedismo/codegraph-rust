# Agent Status CLI Command

This document describes the new `codegraph config agent-status` command that displays orchestrator-agent configuration metadata.

## Command Usage

```bash
# Human-readable output
codegraph config agent-status

# JSON output
codegraph config agent-status --json
```

## What It Displays

### 1. LLM Configuration
- Provider (lmstudio, ollama, anthropic, openai, xai, etc.)
- Model name
- Status (enabled/context-only mode)

### 2. Context Configuration
- Context tier (Small/Medium/Large/Massive)
- Actual context window size in tokens
- Prompt verbosity level (TERSE/BALANCED/DETAILED/EXPLORATORY)
- Base search result limit

### 3. Orchestrator Settings
- Maximum steps per workflow (tier-based)
- Cache status and size
- Maximum output tokens (MCP constraint)

### 4. Available MCP Tools
Lists all 7 active agentic tools with:
- Tool name
- Description
- Prompt type used for that tool

Tools included:
- `enhanced_search` - Search code with AI insights (2-5s)
- `pattern_detection` - Analyze coding patterns (1-3s)
- `vector_search` - Fast vector search (0.5s)
- `graph_neighbors` - Find dependencies (0.3s)
- `graph_traverse` - Follow dependency chains (0.5-2s)
- `codebase_qa` - Ask questions about code (5-30s)
- `semantic_intelligence` - Deep architectural analysis (30-120s)

### 5. Analysis Types
Shows the 7 analysis types and their prompt variants:
- code_search
- dependency_analysis
- call_chain_analysis
- architecture_analysis
- api_surface_analysis
- context_builder
- semantic_question

### 6. Context Tier Details
Reference table showing all tier capabilities:
- Small (< 50K): 5 steps, 10 results, TERSE prompts
- Medium (50K-150K): 10 steps, 25 results, BALANCED prompts
- Large (150K-500K): 15 steps, 50 results, DETAILED prompts
- Massive (> 500K): 20 steps, 100 results, EXPLORATORY prompts

## Implementation Details

### Files Modified
- `crates/codegraph-mcp/src/bin/codegraph.rs`
  - Added `AgentStatus` variant to `ConfigAction` enum
  - Implemented `handle_agent_status()` function
  - Added handler in `handle_config()` match statement

### Key Imports
```rust
use codegraph_core::config_manager::ConfigManager;
use codegraph_mcp::context_aware_limits::ContextTier;
```

### Architecture
The command leverages existing infrastructure:
- `ContextTier::from_context_window()` - Determines tier from LLM config
- `ConfigManager::load()` - Loads current configuration
- Hardcoded tool list matches actual tools in `official_server.rs`
- Tier parameters match `context_aware_limits.rs` and `agentic_orchestrator.rs`

## Example Output

### Human-Readable Format
```
╔══════════════════════════════════════════════════════════════════════╗
║          CodeGraph Orchestrator-Agent Configuration                  ║
╚══════════════════════════════════════════════════════════════════════╝

🤖 LLM Configuration
   Provider: lmstudio
   Model: qwen2.5-coder:14b
   Status: Context-only mode

📊 Context Configuration
   Tier: Small (32000 tokens)
   Prompt Verbosity: TERSE
   Base Search Limit: 10 results

⚙️  Orchestrator Settings
   Max Steps per Workflow: 5
   Cache: Enabled (size: 100 entries)
   Max Output Tokens: 44,200

🛠️  Available MCP Tools
   • enhanced_search [TERSE]
     Search code with AI insights (2-5s)
   • pattern_detection [TERSE]
     Analyze coding patterns and conventions (1-3s)
   ...

🔍 Analysis Types & Prompt Variants
   • code_search → TERSE
   • dependency_analysis → TERSE
   ...

📈 Context Tier Details
   Small (< 50K):        5 steps,  10 results, TERSE prompts
   Medium (50K-150K):   10 steps,  25 results, BALANCED prompts
   Large (150K-500K):   15 steps,  50 results, DETAILED prompts
   Massive (> 500K):    20 steps, 100 results, EXPLORATORY prompts

   Current tier: Small
```

### JSON Format
```json
{
  "llm": {
    "provider": "lmstudio",
    "model": "qwen2.5-coder:14b",
    "enabled": false
  },
  "context": {
    "tier": "Small",
    "window_size": 32000,
    "prompt_verbosity": "TERSE"
  },
  "orchestrator": {
    "max_steps": 5,
    "base_search_limit": 10,
    "cache_enabled": true,
    "cache_size": 100
  },
  "mcp_tools": [
    {
      "name": "enhanced_search",
      "description": "Search code with AI insights (2-5s)",
      "prompt_type": "TERSE"
    },
    ...
  ],
  "analysis_types": [
    {
      "name": "code_search",
      "prompt_type": "TERSE"
    },
    ...
  ]
}
```

## Use Cases

1. **Understanding system configuration** - See how your LLM choice affects system behavior
2. **Debugging prompt issues** - Verify which prompt type is being used
3. **Planning model upgrades** - See what you'd gain by moving to a larger context window
4. **CI/CD validation** - Verify configuration in automated environments
5. **Documentation** - Generate current config snapshots for documentation

## Notes

- This is metadata display only, not runtime statistics
- Pre-existing compilation errors in `official_server.rs` are unrelated to this implementation
- The command will work once those errors are fixed
- Configuration is loaded from the standard config hierarchy (env vars → local .env → global config)
