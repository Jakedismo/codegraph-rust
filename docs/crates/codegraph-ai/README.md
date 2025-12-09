# CodeGraph AI (`codegraph-ai`)

## Overview
`codegraph-ai` provides high-level LLM integration capabilities, distinct from simple vector embeddings.

## Features
- **LLM Providers**: Abstractions for Anthropic, OpenAI, and local LLMs.
- **Completion & Chat**: Unified traits for text generation.
- **Usage**: Used for complex tasks like "Semantic Edge Resolution" where a simple vector similarity isn't enough (e.g., determining if a generic `handle_request` call maps to a specific controller).

## Configuration
Supports switching providers via `codegraph.toml` or environment variables.
