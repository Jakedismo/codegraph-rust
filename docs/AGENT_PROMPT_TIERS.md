# CodeGraph agentic tools: 4-tier prompt system

This document explains how CodeGraph’s built-in agent chooses prompt “tiers” (verbosity/strategy) based on your configured LLM context window, and why this makes small local models viable.

## What “tier” means in CodeGraph

When you call an agentic MCP tool (e.g. `agentic_code_search`), CodeGraph runs a server-side agent that:

1. Uses graph tools against SurrealDB (semantic search, dependency tracing, call-chain tracing, hotspots, etc.)
2. Synthesizes a final answer (and structured pinpoint references) using your configured LLM

CodeGraph picks a context tier based on `llm.context_window` (or `CODEGRAPH_CONTEXT_WINDOW`) and then selects:

- A tier-appropriate system prompt
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

### System prompt

The Rig agent backend builds its system prompt in `crates/codegraph-mcp-rig/src/prompts/tier_prompts.rs`. Every tier shares one layout: identity, task, working instructions, one worked example, the limits for the run, and the answer format. The task text depends on the analysis type. The tier changes three things:

- how far the agent is asked to investigate before answering,
- how long and detailed the answer should be,
- the tool-round budget stated in the prompt.

The prompt does not list the graph tools. The model receives each tool's description and parameter documentation through function calling, from `crates/codegraph-mcp-rig/src/tools/graph_tools.rs`; edit tool semantics there.

The prompts follow OpenAI's guidance for the GPT-6 family (`gpt-6-astra`, `gpt-6.1-sol`, `gpt-6-luna`): state the goal and what a complete answer contains instead of scripting each step, tell the agent to act on the most likely reading of an ambiguous request because nobody can answer a clarifying question, avoid instructions that pull in opposite directions, and ask for plain prose with lists only for parallel items.

### Max tool rounds

- Small: 3
- Medium: 5
- Large: 6
- Massive: 8

`get_max_turns` in `tier_prompts.rs` is the single source for these numbers. The tool loop enforces the value and the same value is written into the system prompt, so the two cannot drift apart.

For LATS, each candidate and final synthesis uses that tool-loop budget. The same
number also bounds tree depth and search expansions, with three candidates per
expansion plus evaluator calls. It does not bound total model calls across the
tree. Candidates share the graph-tool registration, run-level result-size budget
and tool-use tracking with ReAct, while keeping their observations in separate
branch histories. See [agent architectures](AGENTIC_CLI.md#agent-architectures).

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
