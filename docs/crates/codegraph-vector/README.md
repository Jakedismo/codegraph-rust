# CodeGraph Vector (`codegraph-vector`)

## Overview
`codegraph-vector` is responsible for the "AI" part of the graph generation—specifically, creating vector embeddings for code nodes.

## Key Features

### Chunking
Code is often too large for a single embedding vector.
- **Smart Chunking**: Splits code based on semantic boundaries (functions, blocks) rather than just line counts, ensuring coherent embeddings.

### Embedding Generation
- **Provider Abstraction**: Supports multiple backends (OpenAI, Ollama, Local ONNX models via `ort`).
- **`EmbeddingGenerator`**: The main interface for converting text to `Vec<f32>`.

### Integration
Used by `codegraph-mcp` to enrich `CodeNode`s before persistence.
