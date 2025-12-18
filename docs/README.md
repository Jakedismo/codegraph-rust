# CodeGraph Documentation

Welcome to the comprehensive documentation for the CodeGraph project.

## Guides

- [Installation & setup](INSTALLATION_GUIDE.md)
- [Usage guide (for AI assistants)](USAGE_GUIDE.md)
- [AI provider configuration (embeddings + LLMs)](AI_PROVIDERS.md)
- [Agentic prompt tiers (4-tier system)](AGENT_PROMPT_TIERS.md)
- [Supported languages](SUPPORTED_LANGUAGES.md)

## Architecture Visualization
Explore the interactive [Architecture Diagram](architecture-visualization.html) to understand the system components and data flow.

## Crate Documentation

### Core & Data
- [codegraph-core](crates/codegraph-core/README.md): Fundamental types and models (`CodeNode`, `ExtractionResult`).
- [codegraph-graph](crates/codegraph-graph/README.md): Database Access Layer (SurrealDB).
- [codegraph-zerocopy](crates/codegraph-zerocopy/README.md): Zero-copy serialization utilities.

### Processing & AI
- [codegraph-parser](crates/codegraph-parser/README.md): Tree-sitter parsing and unified extraction.
- [codegraph-vector](crates/codegraph-vector/README.md): Embeddings and chunking.
- [codegraph-ai](crates/codegraph-ai/README.md): LLM provider abstractions.
- [codegraph-cache](crates/codegraph-cache/README.md): Caching and read-ahead mechanisms.
- [codegraph-concurrent](crates/codegraph-concurrent/README.md): Concurrency primitives.

### MCP Ecosystem
- [codegraph-mcp](crates/codegraph-mcp/README.md): Main Orchestrator and Indexer.
- [codegraph-mcp-server](crates/codegraph-mcp-server/README.md): Server binary and setup.
- [codegraph-mcp-daemon](crates/codegraph-mcp-daemon/README.md): Background service and file watching.
- [codegraph-mcp-tools](crates/codegraph-mcp-tools/README.md): Specific tool implementations.
- [codegraph-mcp-core](crates/codegraph-mcp-core/README.md): Shared MCP traits.
- [codegraph-mcp-autoagents](crates/codegraph-mcp-autoagents/README.md): Experimental agents.
- [codegraph-mcp-rig](crates/codegraph-mcp-rig/README.md): Testing rig.

## Specifications
Detailed specs can be found in the [specifications](specifications/) directory.
