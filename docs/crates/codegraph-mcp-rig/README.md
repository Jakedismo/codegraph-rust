# CodeGraph MCP Rig Agent (`codegraph-mcp-rig`)

## Overview
`codegraph-mcp-rig` provides an alternative agent backend for the CodeGraph MCP server, built on top of the [Rig](https://github.com/0xPlaygrounds/rig) framework. It is **not** a testing rig, but a fully functional agent implementation that orchestrates LLMs to solve complex tasks using the code graph.

## Purpose
This crate serves as a robust, production-ready alternative to the experimental `codegraph-mcp-autoagents`. It leverages the `rig` library's abstractions for:
- **Provider Abstraction**: Unified interface for OpenAI, Anthropic, Ollama, xAI, and generic OpenAI-compatible providers (like LM Studio).
- **Agent Construction**: Builder pattern (`RigAgentBuilder`) to configure agents with specific system prompts, context tiers, and tool sets.
- **Tool Integration**: Automatically exposes CodeGraph tools (dependency analysis, semantic search, etc.) to the LLM agent.

## Key Components

### `RigAgentBuilder`
The core entry point for creating agents.
- **Context Awareness**: Automatically configures token limits and "turn" counts based on the detected `ContextTier` (e.g., Small, Medium, Large, XLarge).
- **Provider Support**: Feature-gated support for different LLM providers (`openai`, `anthropic`, `ollama`, `xai`).
- **Tool Factory**: Uses `GraphToolFactory` to inject graph-aware tools into the agent's context.

### `RigExecutor`
Handles the execution of agent queries.
- Manages the conversation history.
- Executes the tool use loop (Agent -> Tool -> Agent).
- Returns the final answer to the MCP client.

## Usage
This crate is typically used by `codegraph-mcp-server` when the `rig-experimental` feature is enabled. It allows the server to delegate complex "agentic" requests (like "Refactor this module" or "Explain the data flow") to a sophisticated Rig-based agent.