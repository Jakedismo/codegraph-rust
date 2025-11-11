# CLAUDE.md

This file provides guidance to Claude Code instances working in the CodeGraph repository.

## Project Overview

CodeGraph is a **semantic code intelligence platform** that transforms codebases into searchable knowledge graphs using embeddings and LLMs. It provides an **MCP (Model Context Protocol) server** for AI tool integration with **agentic code analysis capabilities**.

**Core Value Proposition:**
- Semantic code search across entire codebases
- LLM-powered code intelligence and dependency analysis
- Automatic dependency graphs and code relationships
- Fast vector search (FAISS local or SurrealDB cloud HNSW)
- Agentic MCP tools with tier-aware multi-step reasoning

## Common Commands

### Build Commands

```bash
# Development build (all crates)
cargo build --workspace

# Release build
cargo build --workspace --release

# Release with specific features (examples):
cargo build --release --features "onnx,ollama,faiss"              # Local only
cargo build --release --features "cloud-jina,anthropic,faiss"    # Cloud embeddings
cargo build --release --features "cloud-surrealdb,openai,faiss"  # SurrealDB backend
cargo build --release --features "all-cloud-providers,faiss"     # Everything
```

### Test Commands

```bash
# Run all tests
make test
# or
cargo test --workspace

# Run tests for specific crate
cargo test -p codegraph-core

# Run specific test
cargo test --workspace -- <test_name>

# E2E tests for API
make e2e
# or
cargo test -p codegraph-api -- --nocapture

# MCP tools integration tests (Python)
# First, install Python dependencies:
uv sync  # Recommended
# OR
pip install -r requirements-test.txt

# Then run tests:
python3 test_mcp_tools.py           # Standard MCP tools
python3 test_agentic_tools.py       # Agentic tools (requires SurrealDB)
```

### Linting and Formatting

```bash
# Format all code
make fmt
# or
cargo fmt --all

# Check formatting without modifying
make fmt-check

# Lint code
make lint
# or (full workspace)
cargo clippy --workspace --all-targets

# Full check (format + lint + test)
make check
```

### Documentation

```bash
# Generate and open docs
make doc
# or
cargo doc --workspace --no-deps --open
```

### Running the MCP Server

```bash
# Build the MCP server binary (with AutoAgents experimental feature)
cargo build --release -p codegraph-mcp --bin codegraph --features "ai-enhanced,autoagents-experimental,faiss,ollama"

# Or use Makefile target
make build-mcp-autoagents

# Or build without AutoAgents (uses legacy orchestrator)
cargo build --release -p codegraph-mcp --bin codegraph --features "ai-enhanced,faiss,ollama"

# Start MCP server (stdio mode - RECOMMENDED)
./target/release/codegraph start stdio

# Check agentic tool configuration
./target/release/codegraph config agent-status
```

**HTTP Transport (Experimental):**
- HTTP transport with SSE streaming is now available
- Requires `server-http` feature flag
- Build: `cargo build --release --features "ai-enhanced,autoagents-experimental,faiss,ollama,server-http"`
- Start: `./target/release/codegraph-official serve --transport http --port 3000`
- Endpoints:
  - `POST /mcp` - Send MCP requests (returns SSE stream)
  - `GET /sse` - Reconnect to existing session
  - `GET /health` - Health check
- **Production Status**: Experimental - use STDIO for production
- **Best For**: Web integrations, multi-client scenarios, debugging

### HTTP Server Mode

```bash
# Build with HTTP support
cargo build --release -p codegraph-mcp --features "ai-enhanced,autoagents-experimental,faiss,ollama,server-http"

# Start HTTP server (default: http://127.0.0.1:3000)
./target/release/codegraph-official serve --transport http

# Custom host and port
./target/release/codegraph-official serve --transport http --host 0.0.0.0 --port 8080

# Test with curl
curl http://127.0.0.1:3000/health  # Should return "OK"

# Send MCP initialize request
curl -X POST http://127.0.0.1:3000/mcp \
  -H "Content-Type: application/json" \
  -d '{
    "jsonrpc": "2.0",
    "id": 1,
    "method": "initialize",
    "params": {
      "protocolVersion": "2025-06-18",
      "capabilities": {},
      "clientInfo": {"name": "curl", "version": "1.0"}
    }
  }'
```

**Environment Variables:**
```bash
CODEGRAPH_HTTP_HOST=127.0.0.1  # Bind address
CODEGRAPH_HTTP_PORT=3000        # Listen port
CODEGRAPH_HTTP_KEEP_ALIVE=15   # SSE keep-alive seconds
```

### NAPI (Node.js Bindings)

```bash
cd crates/codegraph-napi

# Install dependencies
npm install

# Build native addon
npm run build

# Run tests
npm test

# Run graph functions example
npm run example:graph
```

## Architecture

CodeGraph uses a **layered workspace architecture** with 16 specialized crates:

### Layer 1: Core Foundation
- **codegraph-core**: Configuration management, shared types, logging
  - Hierarchical config system (`~/.codegraph/*.toml` + environment vars)
  - Provider abstractions (embeddings, LLM, vector stores)
  - IMPORTANT: Configuration is loaded from multiple sources with precedence

### Layer 2: Data Storage & Processing
- **codegraph-vector**: Vector embeddings and search (FAISS local + SurrealDB HNSW cloud)
  - Supports ONNX, Ollama, LM Studio, OpenAI, Jina AI embedding providers
  - Dual-mode search: automatic fallback from cloud to local
- **codegraph-cache**: Multi-tier caching (memory, disk, LRU)
- **codegraph-graph**: Graph database operations
  - **Transactional graph** with versioning and branch support
  - SurrealDB integration for advanced graph queries
  - 6 graph analysis functions (dependencies, cycles, coupling, hubs)
- **codegraph-parser**: Source code parsing with tree-sitter
  - Supports 12+ languages (Rust, Python, TypeScript, Go, Java, C++, Swift, Kotlin, C#, Ruby, PHP, Dart)

### Layer 3: Intelligence Layer
- **codegraph-ai**: LLM integration and agentic orchestration
  - **7 agentic MCP tools** with multi-step reasoning:
    1. `agentic_code_search` - Autonomous graph exploration
    2. `agentic_dependency_analysis` - Dependency chain analysis
    3. `agentic_call_chain_analysis` - Execution flow tracing
    4. `agentic_architecture_analysis` - Architectural assessment
    5. `agentic_api_surface_analysis` - Public interface analysis
    6. `agentic_context_builder` - Comprehensive context gathering
    7. `agentic_semantic_question` - Complex Q&A
  - **Tier-aware prompting**: Automatically adapts to LLM context window size
    - Small (<50K): TERSE prompts, 5 max steps, 2,048 tokens
    - Medium (50K-150K): BALANCED prompts, 10 max steps, 4,096 tokens
    - Large (150K-500K): DETAILED prompts, 15 max steps, 8,192 tokens
    - Massive (>500K): EXPLORATORY prompts, 20 max steps, 16,384 tokens
  - **LRU caching**: Transparent result caching for graph queries
  - **LLM Providers**: Ollama, Anthropic, OpenAI, LM Studio, xAI (Grok)

### Layer 4: Integration Layer
- **codegraph-mcp**: MCP server implementation (stdio + streamable HTTP)
  - Uses official `rmcp` Rust SDK (v0.7.0)
  - **IMPORTANT**: MCP server now requires SurrealDB for agentic tools
  - **DEPRECATED**: FAISS+RocksDB support in MCP server (still available in CLI/SDK)
  - Progress notifications for long-running agentic workflows
- **codegraph-napi**: Node.js native bindings via napi-rs
  - Zero-overhead TypeScript integration
  - Auto-generated type definitions
  - Dual-mode search, graph functions, config management

### Layer 5: User Interfaces
- **codegraph-cli**: Command-line interface
- **codegraph-api**: GraphQL + REST API with Swagger/OpenAPI docs

### Supporting Infrastructure
- **codegraph-git**: Git integration (diffs, commits, blame)
- **codegraph-concurrent**: Parallel processing utilities
- **codegraph-queue**: Task queue management
- **codegraph-lb**: Load balancing for distributed deployments
- **codegraph-zerocopy**: Zero-copy serialization (rkyv)

## Key Architectural Decisions

### 1. AutoAgents Integration (v1.1.0 - Experimental)
**🔬 EXPERIMENTAL**: AutoAgents framework integration for agentic orchestration.

**What is it:**
- Replaces custom ~1,200-line `agentic_orchestrator.rs` with AutoAgents ReAct framework
- 6 inner graph analysis tools for agent: GetTransitiveDependencies, GetReverseDependencies, TraceCallChain, DetectCycles, CalculateCoupling, GetHubNodes
- Maintains all 7 existing agentic MCP tools: `agentic_code_search`, `agentic_dependency_analysis`, `agentic_call_chain_analysis`, `agentic_architecture_analysis`, `agentic_api_surface_analysis`, `agentic_context_builder`, `agentic_semantic_question`
- Tier-aware prompting preserved (Small/Medium/Large/Massive context tiers)

**Feature flag:** `autoagents-experimental`

**Status:**
- ✅ Implementation complete (Tasks 1-12 from integration plan)
- ⏳ Testing in progress (Tasks 13-18)
- 📝 Documentation updates in progress
- 🔄 Legacy orchestrator remains as fallback

**Build with AutoAgents:**
```bash
make build-mcp-autoagents
# or
cargo build --release -p codegraph-mcp --features "ai-enhanced,autoagents-experimental,faiss,ollama"
```

**Architecture:**
```
Claude Desktop → agentic_* MCP tools → CodeGraphExecutor
                                       ↓
                                    CodeGraphAgentBuilder
                                       ↓
                                    ReActAgent (AutoAgents)
                                       ↓
                              6 inner graph analysis tools
                                       ↓
                                GraphToolExecutor → SurrealDB
```

### 2. MCP Server Architecture Change (v1.0.0)
**⚠️ IMPORTANT**: The MCP server deprecated FAISS+RocksDB in favor of SurrealDB.

**Why this matters:**
- **Agentic tools require SurrealDB**: The 7 agentic MCP tools (`agentic_*`) need SurrealDB for graph analysis
- **Legacy support remains**: FAISS/RocksDB still work in CLI, SDK, and NAPI bindings
- **Setup required**: Must configure SurrealDB connection (local or free cloud instance)

**Required Environment Variables for Agentic Tools:**
```bash
SURREALDB_URL=ws://localhost:3004  # or wss://cloud-instance.surrealdb.cloud
SURREALDB_NAMESPACE=codegraph
SURREALDB_DATABASE=main
# Optional for cloud:
SURREALDB_USERNAME=your-username
SURREALDB_PASSWORD=your-password
```

### 2. Feature Flag System
CodeGraph uses **extensive feature flags** for conditional compilation. This is critical to understand:

**Common Feature Combinations:**
- `onnx,ollama,faiss` - Local-only setup
- `cloud-jina,anthropic,faiss` - Cloud embeddings + local vector store
- `cloud-surrealdb,openai,faiss` - Cloud graph backend
- `all-cloud-providers,faiss` - Everything

**When working on code:**
- Check `#[cfg(feature = "...")]` attributes
- Some modules only compile with specific features
- NAPI bindings have separate feature sets: `local`, `cloud`, `full`

### 3. Configuration Hierarchy
Configuration is loaded in this order (later overrides earlier):
1. `~/.codegraph/default.toml` (base)
2. `~/.codegraph/{environment}.toml` (e.g., `development.toml`)
3. `~/.codegraph/local.toml` (machine-specific)
4. `./config/` (fallback for backward compatibility)
5. Environment variables (with `CODEGRAPH__*` prefix)

**Key config sections:**
- `[embedding]` - Embedding provider, model, dimensions
- `[llm]` - LLM provider, model, context window (affects tier detection!)
- `[vector_store]` - Backend selection (faiss vs surrealdb)
- `[surrealdb]` - SurrealDB connection details

### 4. Dual-Mode Search Architecture
The semantic search system has two modes:

**Local Mode (FAISS):**
- Uses FAISS vector index + RocksDB for graph
- 2-5ms query latency
- No network calls
- Default fallback if cloud unavailable

**Cloud Mode (SurrealDB HNSW):**
- Native graph database with HNSW vector index
- 2-5ms query latency
- Supports advanced graph queries
- Optional Jina reranking for 2-stage retrieval

**NAPI Implementation:**
- `useCloud: true` → Try cloud first, fallback to local
- `useCloud: false` → Use local only
- Automatic mode detection based on config

## Important Files

### Configuration
- `config/default.toml` - Minimal required config for compilation
- `.codegraph.toml.example` - Full configuration example with all options

### Documentation
- `README.md` - User-facing setup and usage guide
- `CHANGELOG.md` - Version history and migration guides
- `crates/codegraph-napi/README.md` - Node.js integration guide
- `crates/codegraph-napi/GRAPH_FUNCTIONS_GUIDE.md` - Graph analysis API reference

### Testing
- `test_mcp_tools.py` - MCP tools integration test
- `test_agentic_tools.py` - Agentic tools test (requires SurrealDB, long-running)
- `Makefile` - Build targets and test runners

### Schema
- `schema/*.surql` - SurrealDB schema definitions for graph database

## Working with This Codebase

### Before Starting Work

1. **Check feature requirements**: Does your work require specific features to be enabled?
2. **Verify configuration**: Is `config/default.toml` present? (Required for compilation)
3. **For agentic tools**: Is SurrealDB running and configured?

### Common Patterns

**Adding a new MCP tool:**
1. Define in `crates/codegraph-mcp/src/official_server.rs`
2. Add to `list_tools()` method
3. Handle in `call_tool()` match statement
4. Update NAPI types if exposing to TypeScript

**Adding a new LLM provider:**
1. Implement `LLMProvider` trait in `codegraph-ai/src/llm_factory.rs`
2. Add feature flag to `Cargo.toml`
3. Update config schema in `codegraph-core`
4. Add build flag documentation to `README.md`

**Adding a new embedding provider:**
1. Implement in `codegraph-vector/src/embedding/`
2. Update `EmbeddingProvider` enum in `codegraph-core`
3. Add to factory in `codegraph-vector/src/lib.rs`
4. Update configuration examples

### Testing Strategy

**Unit tests**: Each crate has `#[cfg(test)]` modules
**Integration tests**: `tests/integration/` directory
**E2E tests**:
- MCP protocol: `test_mcp_tools.py`, `test_agentic_tools.py`
- API: `cargo test -p codegraph-api`

**Important**: Agentic tool tests can take 10-90 seconds each due to multi-step LLM reasoning.

### Performance Considerations

**Caching is critical:**
- FAISS index cache: 300-600 MB, 10-50× speedup
- Embedding cache: ~90 MB, 10-100× speedup
- Query cache: ~10 MB, 100× speedup
- LRU cache for agentic results: 100 entries default

**Parallel processing:**
- Rayon for CPU-bound work
- Tokio for I/O-bound work
- Crossbeam channels for message passing

### Dependencies

**Vendored crates** (in `vendor/`):
- `rmcp` - Official MCP SDK (patched for features)
- `semchunk-rs` - Semantic text chunking

**Key external dependencies:**
- `tree-sitter-*` - Language parsing (12+ languages)
- `faiss` - Vector search (requires system library)
- `surrealdb` - Graph database
- `napi-rs` - Node.js native bindings
- `axum` - Web framework for API

## Free Cloud Resources

CodeGraph can use completely free cloud services:

- **SurrealDB Cloud**: 1GB free instance at [surrealdb.com/cloud](https://surrealdb.com/cloud)
- **Jina AI**: 10 million free API tokens at [jina.ai](https://jina.ai) for embeddings and reranking

This makes cloud deployment free for testing and small projects.

## Common Issues

### Build Failures

**"Could not find library faiss"**
```bash
brew install faiss  # macOS
sudo apt-get install libfaiss-dev  # Ubuntu
```

**"Feature X not enabled"**
- Check your build command includes the required feature flag
- Example: `cargo build --features "cloud-surrealdb"`

**Missing `config/default.toml`**
- This file is required for compilation
- Contains minimal LLM and SurrealDB configuration

### Runtime Issues

**"GraphFunctions not initialized"**
- Agentic tools require SurrealDB connection
- Set `SURREALDB_URL` environment variable
- Check SurrealDB is running: `surreal start ...`

**"API key not found"**
- Set environment variable: `ANTHROPIC_API_KEY`, `OPENAI_API_KEY`, `JINA_API_KEY`
- Or add to `~/.codegraph/local.toml`

**NAPI linking errors**
- Run `npm install` in `crates/codegraph-napi/`
- For M-series Macs, ensure Rosetta 2 is not interfering

### Test Failures

**Agentic tests timeout**
- Normal - these tests take 10-90 seconds due to multi-step reasoning
- Check `test_agentic_tools.py` has correct timeout values
- Verify SurrealDB connection is working

**MCP tests fail**
- Check binary exists: `./target/release/codegraph` or `./target/debug/codegraph`
- Try building with AutoAgents: `cargo build -p codegraph-mcp --bin codegraph --features "ai-enhanced,autoagents-experimental,faiss,ollama"`
- Or without AutoAgents (legacy): `cargo build -p codegraph-mcp --bin codegraph --features "ai-enhanced,faiss,ollama"`

## Additional Resources

- **Cloud Providers Guide**: `docs/CLOUD_PROVIDERS.md`
- **LM Studio Setup**: `LMSTUDIO_SETUP.md`
- **Configuration Guide**: `docs/CONFIGURATION_GUIDE.md`
- **NAPI Graph Functions**: `crates/codegraph-napi/GRAPH_FUNCTIONS_GUIDE.md`

---

## MCP Tools Overview

CodeGraph provides these MCP tools for code intelligence:

**Search & Discovery:**
- `enhanced_search` - Semantic search with AI insights
- `vector_search` - Fast similarity-based search
- `pattern_detection` - Analyze coding patterns

**Graph Analysis:**
- `graph_neighbors` - Find direct dependencies
- `graph_traverse` - Follow dependency chains

**Advanced (feature-gated):**
- `codebase_qa` - RAG-powered Q&A
- `code_documentation` - AI documentation generation
