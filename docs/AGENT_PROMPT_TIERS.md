# CodeGraph agentic tools: 4-tier prompt system

This document explains how CodeGraph’s built-in agent chooses prompt “tiers” (verbosity/strategy) based on your configured LLM context window, and why this makes small local models viable.

## What “tier” means in CodeGraph

When you call an agentic MCP tool (e.g. `agentic_code_search`), CodeGraph runs a server-side agent that:

1. Uses graph tools against SurrealDB (semantic search, dependency tracing, call-chain tracing, hotspots, etc.)
2. Synthesizes a final answer (and structured pinpoint references) using your configured LLM

CodeGraph picks a context tier based on `llm.context_window` (or `CODEGRAPH_CONTEXT_WINDOW`) and then selects:

- A tier-appropriate system prompt (Terse/Balanced/Detailed/Exploratory)
- A recommended tool/step budget per analysis type
- Retrieval and over-retrieval limits to avoid “too much context” and MCP output caps

## The 4 tiers (Small / Medium / Large / Massive)

Tier detection is purely based on the configured context window:

- **Small**: `0..=50_000`
- **Medium**: `50_001..=150_000`
- **Large**: `150_001..=500_000`
- **Massive**: `>500_000` (e.g. multi-hundred-K to 2M context window models)

Where CodeGraph reads it from:

- `CODEGRAPH_CONTEXT_WINDOW` env var (highest priority), else
- `llm.context_window` from config loaded via `ConfigManager::load()`

## What changes per tier

### Prompt verbosity

Each analysis type has 4 prompt variants, selected via:

- Small → **Terse**
- Medium → **Balanced**
- Large → **Detailed**
- Massive → **Exploratory**

This selection happens in `crates/codegraph-mcp-server/src/prompt_selector.rs`.

### Recommended max steps

The agent uses a base max step count by tier:

- Small: 5
- Medium: 10
- Large: 15
- Massive: 20

Then it applies an analysis-type multiplier (e.g. architecture analysis tends to get a larger budget than code search).

### Retrieval limits (and MCP-safe output)

CodeGraph also scales how much it retrieves:

- Base max results:
  - Small: 10
  - Medium: 25
  - Large: 50
  - Massive: 100
- Over-retrieval multipliers:
  - Local search: 5 / 8 / 10 / 15 (Small→Massive)
  - Cloud+rerank: 3 / 4 / 5 / 8 (Small→Massive)

Separately, MCP responses are capped to stay under common client limits: CodeGraph uses a safe ceiling of **44,200 output tokens** for tool responses even if your model can generate more.

## Why small local models can still work well

With “vanilla” vector search, a client-side code agent typically has to:

- guess what to search for,
- run multiple searches,
- fetch large blobs of code,
- spend tokens to stitch and reason over results,
- repeat until it “finds the right area”.

CodeGraph shifts much of that exploration cost into:

- a graph database (structural relationships), and
- an agent that can chain purpose-built graph tools.

So even if your configured model is small (Small/Medium tier), the agent often only needs:

1. a few targeted tool calls to pull the right snippets + relationships, and
2. a short synthesis step to explain the result.

The result is less “token burn” on exploration and more remaining context budget for actually implementing changes in your external code agent.

## Why massive-context models still matter

Massive tier models (hundreds of thousands to ~2M context windows) can be genuinely helpful when you want:

- deeper multi-perspective architectural reasoning,
- broad “whole codebase” review narratives,
- more exhaustive call-chain exploration with multiple alternative hypotheses.

CodeGraph’s exploratory tier prompts and higher retrieval/step budgets are designed to take advantage of those models without forcing smaller models into failure modes (too much retrieved context, too many steps, or huge outputs).

## Practical configuration tips

1. If you use agentic tools, set `llm.enabled = true` and a working provider in `./.codegraph.toml` or `~/.codegraph/config.toml`.
2. Set `llm.context_window` to match your actual model, or override with `CODEGRAPH_CONTEXT_WINDOW`.
3. If you hit MCP client output issues, reduce `llm.context_window` or lower your tool requests (smaller limits), rather than increasing outputs.

For provider setup examples, see `docs/AI_PROVIDERS.md`.
