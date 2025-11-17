# CodeGraph

> Turn your codebase into a searchable knowledge graph powered by embeddings and LLMs

CodeGraph indexes your source code to a graph database, creates semantic embeddings, and exposes a **Model Context Protocol (MCP)** server that AI tools (Claude Desktop, LM Studio, etc.) can query for project-aware context.

**✨ What you get:**
- 🧩 **NEW:** SurrealDB indexing now supports local embeddings (Ollama + LM Studio) and local reranking—just set the env vars and we’ll write to the right HNSW column automatically
- 🔍 Semantic code search across your entire codebase
- 🧠 LLM-powered code intelligence and analysis
- 📊 Automatic dependency graphs and code relationships
- ⚡ Fast vector search with FAISS or cloud SurrealDB HNSW (2-5ms query latency)
- 🔌 MCP server for AI tool integration (stdio and streamable HTTP)
- ⚙️ Easy-to-use CLI interface
- ☁️ **NEW:** Jina AI cloud embeddings with modifiable models and dimensions and reranking
- 🗄️ **NEW:** SurrealDB HNSW backend for cloud-native and local vector search
- 📦 **NEW:** Node.js NAPI bindings for zero-overhead TypeScript integration
- 🤖 **NEW:** Agentic code-agent tools with tier-aware multi-step reasoning
- 🔄 **NEW:** Incremental indexing with SHA-256 file change detection—only re-index modified files
- 🔬 **EXPERIMENTAL:** AutoAgents framework integration for improved agent orchestration

### Local Embeddings & Reranking (SurrealDB)

CodeGraph now writes Ollama/LM Studio embeddings directly into SurrealDB’s dedicated HNSW columns. Pick the model you want and set the matching env vars before running `codegraph index`:

```bash
export CODEGRAPH_EMBEDDING_PROVIDER=ollama
export CODEGRAPH_EMBEDDING_MODEL=qwen3-embedding:0.6b   # or all-mini-llm, qwen3-embedding:4b, etc.
export CODEGRAPH_EMBEDDING_DIMENSION=1024               # 384, 1024, 2048, or 4096

# Optional local reranking (LM Studio exposes an OpenAI-compatible reranker endpoint)
export CODEGRAPH_RERANKING_PROVIDER=lmstudio
```

We automatically route embeddings to `embedding_384`, `embedding_1024`, `embedding_2048`, or `embedding_4096` and keep reranking disabled unless a provider is configured. No FAISS/RocksDB plumbing is needed for MCP anymore.

---

## ⚠️ Important: MCP Server Architecture Change

**FAISS+RocksDB support in MCP server is deprecated** in favor of SurrealDB-based architecture.

### What Changed:
- ❌ **MCP server** no longer uses FAISS vector search or RocksDB graph storage
- ✅ **CLI and SDK** continue to support FAISS/RocksDB for local operations
- ✅ **NAPI bindings** still provide TypeScript access to all features
- 🆕 **MCP code-agent tools** now require SurrealDB for graph analysis

### Required Setup for Code-Agent Tools:

The new agentic MCP tools (`agentic_code_search`, `agentic_dependency_analysis`, etc.) require SurrealDB:

**Option 1: Free Cloud Instance (Recommended)**
- Sign up at [Surreal Cloud](https://surrealdb.com/cloud)
- **Get 1GB FREE instance** - perfect for testing and small projects
- Configure connection details in environment variables

**Option 2: Local Installation**
```bash
# Install SurrealDB
curl -sSf https://install.surrealdb.com | sh

# Run locally
surreal start --bind 127.0.0.1:3004 --user root --pass root memory
```

**Free Cloud Resources:**
- 🆓 **SurrealDB Cloud**: 1GB free instance at [surrealdb.com/cloud](https://surrealdb.com/cloud)
- 🆓 **Jina AI**: 10 million free API tokens at [jina.ai](https://jina.ai) for embeddings and reranking

### Why This Change:
- **Native graph capabilities**: SurrealDB provides built-in graph database features
- **Unified storage**: Single database for both vectors and graph relationships and extendable to relational and document use-cases!
- **Cloud-native**: Better support for distributed deployments
- **Reduced complexity**: Eliminates custom RocksDB integration layer

**See [CHANGELOG.md](CHANGELOG.md) for detailed migration guide.**

---

## 🔬 Experimental: AutoAgents Framework

CodeGraph now supports the **AutoAgents framework** for agentic orchestration as an experimental feature.

### What is AutoAgents?
- Modern Rust-based agent framework with ReAct (Reasoning + Acting) pattern
- Replaces ~1,200 lines of custom orchestration code
- Maintains compatibility with all 7 existing agentic MCP tools
- Same tier-aware prompting system (Small/Medium/Large/Massive)

### Enabling AutoAgents

**Build with experimental feature:**
```bash
# Using Makefile
make build-mcp-autoagents

# Or directly with cargo
cargo build --release -p codegraph-mcp --features "ai-enhanced,autoagents-experimental,faiss,ollama"

# HTTP server with AutoAgents
cargo build --release -p codegraph-mcp --features "ai-enhanced,autoagents-experimental,faiss,embeddings-ollama,server-http"
```

**Without AutoAgents (default):**
```bash
cargo build --release -p codegraph-mcp --features "ai-enhanced,faiss,ollama"
```

### Status
- ✅ Core implementation complete
- ⏳ Testing and validation in progress
- 📝 Feedback welcome via GitHub issues
- 🔄 Legacy orchestrator remains as stable fallback

The experimental feature is opt-in via build flag and does not affect existing functionality when disabled.

---

## 📋 Table of Contents

- [Choose Your Setup](#choose-your-setup)
- [Installation](#installation)
- [Configuration](#configuration)
- [Usage](#usage)
- [Feature Flags Reference](#feature-flags-reference)
- [Performance](#performance)
- [Troubleshooting](#troubleshooting)
- [Advanced Features](#advanced-features)

---

## 🎯 Choose Your Setup

Pick the setup that matches your needs:

### Option 1: Local Setup (Free, Private) 🏠

**Best for:** Privacy-conscious users, offline work, no API costs

**Providers:**
- **Embeddings:** ONNX or Ollama
- **LLM:** Ollama (Qwen2.5-Coder, CodeLlama, etc.)

**Pros:** ✅ Free, ✅ Private, ✅ No internet required after setup
**Cons:** ❌ Slower, ❌ Requires local GPU/CPU resources

[→ Jump to Local Setup Instructions](#local-setup-onnx--ollama)

---

### Option 2: LM Studio (Best Performance on Mac) 🚀

**Best for:** Mac users (Apple Silicon), best local performance

**Providers:**
- **Embeddings:** LM Studio (Jina embeddings)
- **LLM:** LM Studio (DeepSeek Coder, etc.)

**Pros:** ✅ 120 embeddings/sec, ✅ MLX + Flash Attention 2, ✅ Free
**Cons:** ❌ Mac only, ❌ Requires LM Studio app

[→ Jump to LM Studio Setup Instructions](#lm-studio-setup)

---

### Option 3: Cloud Providers (Best Quality) ☁️

**Best for:** Production use, best quality, don't want to manage local models

**Providers:**
- **Embeddings:** Jina (You get 10 million tokens for free when you just create an account!)
- **LLM:** Anthropic Claude or OpenAI GPT-5-*
- **Backend**: SurrealDB graph database (You get a free cloud instance up-to 1gb! Or run it completely locally!)

**Pros:** ✅ Best quality, ✅ Fast, ✅ 1M context (sonnet[1m])
**Cons:** ❌ API costs, ❌ Requires internet, ❌ Data sent to cloud

[→ Jump to Cloud Setup Instructions](#cloud-setup-anthropic--openai)

---

### Option 4: Hybrid (Mix & Match) 🔀

**Best for:** Balancing cost and quality

**Example combinations:**
- Local embeddings (ONNX) + Cloud LLM (OpenAI, Claude, x.ai)
- LMStudio embeddings + Cloud LLM (OpenAI, Claude, x.ai)
- Jina AI embeddings + Local LLM (Ollama, LMStudio)

[→ Jump to Hybrid Setup Instructions](#hybrid-setup)

---

## 🛠️ Installation

### Prerequisites (All Setups)

```bash
# 1. Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# 2. Install FAISS (vector search library)
# macOS:
brew install faiss

# Ubuntu/Debian:
sudo apt-get install libfaiss-dev

# Arch Linux:
sudo pacman -S faiss
```

---

### Local Setup (ONNX + Ollama)

**Step 1: Install Ollama**
```bash
# macOS/Linux:
curl -fsSL https://ollama.com/install.sh | sh

# Or download from: https://ollama.com/download

brew install onnx-runtime
```

**Step 2: Pull models**
```bash
# Pull embedding model
hf (cli) download qdrant/all-minillm-onnx

# Pull LLM for code intelligence (optional)
ollama pull qwen2.5-coder:14b
```

**Step 3: Build CodeGraph**
```bash
cd codegraph-rust

# Build with ONNX embeddings and Ollama support
cargo build --release --features "onnx,ollama,faiss"
```

**Step 4: Configure**

Create `~/.codegraph/config.toml`:
```toml
[embedding]
provider = "onnx"  # or "ollama" if you prefer
model = "qdrant/all-minillm-onnx"
dimension = 384

[llm]
enabled = true
provider = "ollama"
model = "qwen2.5-coder:14b"
ollama_url = "http://localhost:11434"
```

**Step 5: Index and run**
```bash
# Index your project
./target/release/codegraph index /path/to/your/project

# Start MCP server
./target/release/codegraph start stdio
```

✅ **Done!** Your local setup is ready.

---

### LM Studio Setup

**Step 1: Install LM Studio**
- Download from [lmstudio.ai](https://lmstudio.ai/)
- Install and launch the app

**Step 2: Download models in LM Studio**
- **Embedding model:** `jinaai/jina-embeddings-v4`
- **LLM model (optional):** `lmstudio-community/DeepSeek-Coder-V2-Lite-Instruct-GGUF`

**Step 3: Start LM Studio server**
- In LM Studio, go to "Local Server" tab
- Click "Start Server" (runs on `http://localhost:1234`)

**Step 4: Build CodeGraph**
```bash
cd codegraph-rust

# Build with OpenAI-compatible support (for LM Studio)
cargo build --release --features "openai-compatible,faiss"
```

**Step 5: Configure**

Create `~/.codegraph/config.toml`:
```toml
[embedding]
provider = "lmstudio"
model = "jinaai/jina-embeddings-v4"
lmstudio_url = "http://localhost:1234"
dimension = 2048

[llm]
enabled = true
provider = "lmstudio"
model = "lmstudio-community/DeepSeek-Coder-V2-Lite-Instruct-GGUF"
lmstudio_url = "http://localhost:1234"
```

**Step 6: Index and run**
```bash
# Index your project
./target/release/codegraph index /path/to/your/project

# Start MCP server
./target/release/codegraph start stdio
```

✅ **Done!** LM Studio setup complete.

---

### Cloud Setup (Anthropic, OpenAI, xAI & Jina AI)

**Step 1: Get API keys**
- Anthropic: [console.anthropic.com](https://console.anthropic.com/)(Claude 4.5 models 1M/200k ctx)
- OpenAI: [platform.openai.com](https://platform.openai.com/)(GPT-5 models 400k/200k ctx)
- xAI: [x.ai](https://x.ai/) (Grok-4-fast with 2M ctx, $0.50-$1.50/M tokens)
- Jina AI: [jina.ai](https://jina.ai/) (for SOTA embeddings & reranking)
- SurrealDB [https://www.surrealdb.com] (for graph dabase backend local or cloud based setup)

**Step 2: Build CodeGraph with cloud features**
```bash
cd codegraph-rust

# Build with all cloud providers
cargo build --release --features "anthropic,openai-llm,openai,faiss"

# Or with Jina AI cloud embeddings (Matryoska dimensions + reranking)
cargo build --release --features "cloud-jina,anthropic,faiss"

# Or with SurrealDB HNSW cloud/local vector backend
cargo build --release --features "cloud-surrealdb,openai,faiss"
```

**Step 3: Run setup wizard (easiest)**
```bash
./target/release/codegraph-setup
```

The wizard will guide you through configuration.

**Or manually configure** `~/.codegraph/config.toml`:

**For Anthropic Claude:**
```toml
[embedding]
provider = "jina" # or openai
model = "jina-embeddings-v4"
openai_api_key = "sk-..."  # or set OPENAI_API_KEY env var
dimension = 2048

[llm]
enabled = true
provider = "anthropic"
model = "claude-haiku"
anthropic_api_key = "sk-ant-..."  # or set ANTHROPIC_API_KEY env var
context_window = 200000
```

**For OpenAI (with reasoning models):**
```toml
[embedding]
provider = "jina" # or openai
model = "jina-embeddings-v4"
openai_api_key = "sk-..."
dimension = 2048

[llm]
enabled = true
provider = "openai"
model = "gpt-5-codex-mini"
openai_api_key = "sk-..."
max_completion_token = 128000
reasoning_effort = "medium"  # reasoning models: "minimal", "medium", "high"
```

**For Jina AI (cloud embeddings with reranking):**
```toml
[embedding]
provider = "jina"
model = "jina-embeddings-v4"
jina_api_key = "jina_..."  # or set JINA_API_KEY env var
dimension = 2048 # or matryoshka 1024,512,256 adjust the schemas/*.surql file HNSW vector index to match your embedding model dimensions
jina_enable_reranking = true  # Optional two-stage retrieval
jina_reranking_model = "jina-reranker-v3"

[llm]
enabled = true
provider = "anthropic"
model = "claude-haiku"
anthropic_api_key = "sk-ant-..."
```

**For xAI Grok (2M context window, $0.50-$1.50/M tokens):**
```toml
[embedding]
provider = "openai"  # or "jina"
model = "text-embedding-3-small"
openai_api_key = "sk-..."
dimension = 2048

[llm]
enabled = true
provider = "xai"
model = "grok-4-fast"  # or "grok-4-turbo"
xai_api_key = "xai-..."  # or set XAI_API_KEY env var
xai_base_url = "https://api.x.ai/v1"  # default, can be omitted
reasoning_effort = "medium"  # Options: "minimal", "medium", "high"
context_window = 2000000  # 2M tokens!
```

**For SurrealDB HNSW (graph database backend with advanced features):**
```toml
[embedding]
provider = "jina"  # or "openai"
model = "jina-embeddings-v4"
openai_api_key = "sk-..."
dimension = 2048

[vector_store]
backend = "surrealdb"  # Instead of "faiss"
surrealdb_url = "ws://localhost:8000"  # or cloud instance
surrealdb_namespace = "codegraph"
surrealdb_database = "production"

[llm]
enabled = true
provider = "anthropic"
model = "claude-haiku"
```

**Step 4: Index and run**
```bash
# Index your project
./target/release/codegraph index /path/to/your/project

# Start MCP server
./target/release/codegraph start stdio
```

✅ **Done!** Cloud setup complete.

---

### Hybrid Setup

Mix local and cloud providers to balance cost and quality:

**Example: Local embeddings + Cloud LLM**
```toml
[embedding]
provider = "onnx"  # Free, local
model = "sentence-transformers/all-MiniLM-L6-v2"
dimension = 384

[llm]
enabled = true
provider = "anthropic"  # Best quality for analysis
model = "sonnet[1m]"
anthropic_api_key = "sk-ant-..."
```

Build with required features:
```bash
cargo build --release --features "onnx,anthropic,faiss"
```

---

## ⚙️ Configuration

### Quick Configuration

**Use the interactive wizard:**
```bash
cargo build --release --bin codegraph-setup --features all-cloud-providers
./target/release/codegraph-setup
```

### Manual Configuration

**Configuration directory: `~/.codegraph/`**

All configuration files are stored in `~/.codegraph/` in TOML format.

Configuration is loaded from (in order):
1. `~/.codegraph/default.toml` (base configuration)
2. `~/.codegraph/{environment}.toml` (e.g., development.toml, production.toml)
3. `~/.codegraph/local.toml` (local overrides, machine-specific)
4. `./config/` (fallback for backward compatibility)
5. Environment variables (CODEGRAPH__* prefix)

**See [Configuration Guide](docs/CONFIGURATION_GUIDE.md) for complete documentation.**

**Full configuration example:**
```toml
[embedding]
provider = "lmstudio"  # or "onnx", "ollama", "openai"
model = "jinaai/jina-embeddings-v4"
dimension = 2048
batch_size = 64

[llm]
enabled = true
provider = "anthropic"  # or "openai", "ollama", "lmstudio" or "xai"
model = "haiku"
anthropic_api_key = "sk-ant-..."
context_window = 200000
temperature = 0.1
max_completion_token = 4096

[performance]
num_threads = 0  # 0 = auto-detect
cache_size_mb = 512
max_concurrent_requests = 4

[logging]
level = "warn"  # trace, debug, info, warn, error
format = "pretty"  # pretty, json, compact
```

**See [`.codegraph.toml.example`](.codegraph.toml.example) for all options.**

---

## 🚀 Usage

### Basic Commands

```bash
# Index a project
codegraph index -r /path/to/project

# Start MCP server (for Claude Desktop, LM Studio, etc.)
codegraph start stdio

# List available MCP tools
codegraph tools list
```

**Note:** HTTP transport is not yet implemented with the official rmcp SDK. Use STDIO transport for all MCP integrations.

### Using with Claude Desktop

Add to your Claude Desktop config (`~/Library/Application Support/Claude/claude_desktop_config.json` on Mac):

```json
{
  "mcpServers": {
    "codegraph": {
      "command": "/path/to/codegraph",
      "args": ["start", "stdio"],
      "env": {
        "RUST_LOG": "warn"
      }
    }
  }
}
```

### Using with LM Studio

1. Start CodeGraph MCP server: `codegraph start stdio`
2. In LM Studio, enable MCP support in settings
3. CodeGraph tools will appear in LM Studio's tool palette

---

### HTTP Server Mode (Experimental)

For web integrations and multi-client scenarios:

```bash
# Build with HTTP support
make build-mcp-http

# Start server
./target/release/codegraph start http

# Test endpoints
curl http://127.0.0.1:3000/health
curl -X POST http://127.0.0.1:3000/mcp \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"test","version":"1.0"}}}'
```

**Features:**
- ✅ Session-based stateful connections
- ✅ SSE streaming for real-time progress
- ✅ Automatic session management
- ✅ Reconnection support with Last-Event-Id

**Use Cases:**
- Web-based code analysis dashboards
- Multi-client collaborative environments
- API integrations
- Development/debugging (easier to inspect than STDIO)

**Note:** For production use with Claude Desktop, use STDIO mode.

---

## 📚 For AI Agents: Using CodeGraph MCP

CodeGraph provides powerful code intelligence tools via the Model Context Protocol (MCP).

### Available MCP Tools

After indexing your codebase, AI agents can use these agentic workflows (requires SurrealDB-backed graphs):

- 🔍 **`agentic_code_search`** - Autonomous semantic exploration for finding and understanding code
- 📊 **`agentic_dependency_analysis`** - Impact and coupling analysis across forward and reverse dependencies
- 🔗 **`agentic_call_chain_analysis`** - Execution path tracing through controllers, services, and downstream systems
- 🏗️ **`agentic_architecture_analysis`** - Architectural pattern assessment plus layer and cohesion breakdowns
- 🌐 **`agentic_api_surface_analysis`** - Public API surface analysis with consumer mapping and change risk detection
- 📦 **`agentic_context_builder`** - Comprehensive context gathering ahead of implementing or modifying features
- ❓ **`agentic_semantic_question`** - Deep semantic Q&A that synthesizes answers across multiple subsystems

These multi-step workflows typically take 30–90 seconds to complete because they traverse the code graph and build detailed reasoning summaries.

### Quick Start

```bash
# 1. Index your codebase
codegraph index /path/to/your/project

# 2. Start MCP server
codegraph start stdio

# 3. Use tools from your AI agent
agentic_code_search("how does authentication work?")
agentic_dependency_analysis("what depends on AuthService?")
```

---

## 📊 Feature Flags Reference

When building, include features for the providers you want to use:

| Feature | Providers Enabled | Use Case |
|---------|------------------|----------|
| `onnx` | ONNX embeddings | Local CPU/GPU embeddings |
| `ollama` | Ollama embeddings + LLM | Local models via Ollama |
| `openai` | OpenAI embeddings | Cloud embeddings (text-embedding-3-large/small) |
| `openai-llm` | OpenAI | Cloud LLM (gpt-5, gpt-5-codex, gpt-5-codex-mini) |
| `anthropic` | Anthropic Claude | Cloud LLM (Claude 4.5, Haiku 4.5) |
| `openai-compatible` | LMStudio, custom providers | OpenAI Responses API compatible | (f.ex xai/grok-4-fast)|
| `cloud-jina` | Jina AI embeddings + reranking | Cloud embeddings & Free usage (SOTA and variable dims) |
| `cloud-surrealdb` | SurrealDB HNSW | Local & Free Cloud-native graph database backend (up-to 1gb)|
| `cloud` | Jina AI + SurrealDB | All cloud vector & graph features |
| `faiss` | FAISS vector search | Local vector search graph backend (rocksdb persisted)|
| `all-cloud-providers` | All cloud LLM providers | Shortcut for Jina + Surreal + Anthropic + OpenAI |

### Common Build Commands

```bash
# Local only (ONNX + Ollama)
cargo build --release --features "onnx,ollama,faiss"

# LM Studio
cargo build --release --features "openai-compatible,faiss"

# Cloud only (Anthropic + OpenAI)
cargo build --release --features "anthropic,openai-llm,openai,faiss"

# Jina AI cloud embeddings + local FAISS
cargo build --release --features "cloud-jina,faiss"

# SurrealDB cloud vector backend
cargo build --release --features "cloud-surrealdb,openai,faiss"

# Full cloud (Jina + SurrealDB + Anthropic)
cargo build --release --features "cloud,anthropic,faiss"

# Everything (local + cloud)
cargo build --release --features "all-cloud-providers,onnx,ollama,cloud,faiss"

# HTTP server with AutoAgents (experimental)
cargo build --release -p codegraph-mcp --features "ai-enhanced,autoagents-experimental,faiss,embeddings-ollama,server-http"
```

---

## ⚡ Performance

### Speed Metrics (Apple Silicon + LM Studio)

| Operation | Performance | Notes |
|-----------|------------|-------|
| **Embedding generation** | 120 embeddings/sec | LM Studio with MLX |
| **Vector search (local)** | 2-5ms latency | FAISS with index caching |
| **Vector search (cloud)** | 2-5ms latency | SurrealDB HNSW |
| **Jina AI embeddings** | 50-150ms per query | Cloud API call overhead |
| **Jina reranking** | 80-200ms for top-K | Two-stage retrieval |
| **Ollama embeddings** | ~60 embeddings/sec | About half LM Studio speed |

### Optimizations (Enabled by Default)

| Optimization | Speedup | Memory Cost |
|-------------|---------|-------------|
| FAISS index cache | 10-50× | 300-600 MB |
| Embedding cache | 10-100× | ~90 MB |
| Query cache | 100× | ~10 MB |
| Parallel search | 2-3× | Minimal |

---

## 🔧 Troubleshooting

### Build Issues

**"Could not find library faiss"**
```bash
# Install FAISS first
brew install faiss  # macOS
sudo apt-get install libfaiss-dev  # Ubuntu
```

**"Feature X is not enabled"**
- Make sure you included the feature flag when building
- Example: `cargo build --release --features "anthropic,faiss"`

### Runtime Issues

**"API key not found"**
- Set environment variable: `export ANTHROPIC_API_KEY="sk-ant-..."`
- Or add to config file: `anthropic_api_key = "sk-ant-..."`

**"Model not found"**
- For Ollama: Run `ollama pull <model-name>` first
- For LM Studio: Download the model in LM Studio app
- For cloud: Check your model name matches available models

**"Connection refused"**
- LM Studio: Make sure the local server is running
- Ollama: Check Ollama is running with `ollama list`
- Cloud: Check your internet connection

### Getting Help

1. Check [docs/CLOUD_PROVIDERS.md](docs/CLOUD_PROVIDERS.md) for detailed provider setup
2. See [LMSTUDIO_SETUP.md](LMSTUDIO_SETUP.md) for LM Studio specifics
3. Open an issue on GitHub with your error message

---

## 📦 Node.js Integration (NAPI Bindings)

### Zero-Overhead TypeScript Integration

CodeGraph provides native Node.js bindings through NAPI-RS for seamless TypeScript/JavaScript integration:

**Key Features:**
- 🚀 **Native Performance**: Direct Rust-to-Node.js bindings with zero serialization overhead
- 📘 **Auto-Generated Types**: TypeScript definitions generated directly from Rust code
- ⚡ **Async Runtime**: Full tokio async support integrated with Node.js event loop
- 🔄 **Hot-Reload Config**: Update configuration without restarting your Node.js process
- 🌐 **Dual-Mode Search**: Automatic routing between local FAISS and cloud SurrealDB

### Installation

**Option 1: Direct Install (Recommended)**
```bash
# Build the addon
cd crates/codegraph-napi
npm install
npm run build

# Install in your project
cd /path/to/your-project
npm install /path/to/codegraph-rust/crates/codegraph-napi
```

**Option 2: Pack and Install**
```bash
# Build and pack
cd crates/codegraph-napi
npm install
npm run build
npm pack  # Creates codegraph-napi-1.0.0.tgz

# Install in your project
cd /path/to/your-project
npm install /path/to/codegraph-rust/crates/codegraph-napi/codegraph-napi-1.0.0.tgz
```

### API Examples

**Semantic Search:**
```typescript
import { semanticSearch } from 'codegraph-napi';

const results = await semanticSearch('find authentication code', {
  limit: 10,
  useCloud: true,      // Use cloud search with automatic fallback
  reranking: true      // Enable Jina reranking (if configured)
});

console.log(`Found ${results.totalCount} results in ${results.searchTimeMs}ms`);
console.log(`Search mode: ${results.modeUsed}`);  // "local" or "cloud"
```

**Configuration Management:**
```typescript
import { getCloudConfig, reloadConfig } from 'codegraph-napi';

// Check cloud feature availability
const config = await getCloudConfig();
console.log('Jina AI enabled:', config.jina_enabled);
console.log('SurrealDB enabled:', config.surrealdb_enabled);

// Hot-reload configuration without restart
await reloadConfig();
```

**Embedding Operations:**
```typescript
import { getEmbeddingStats, countTokens } from 'codegraph-napi';

// Get embedding provider stats
const stats = await getEmbeddingStats();
console.log(`Provider: ${stats.provider}, Dimension: ${stats.dimension}`);

// Count tokens for cost estimation (Jina AI)
const tokens = await countTokens("query text");
console.log(`Token count: ${tokens}`);
```

**Graph Navigation:**
```typescript
import { getNeighbors, getGraphStats } from 'codegraph-napi';

// Get connected nodes
const neighbors = await getNeighbors(nodeId);

// Get graph statistics
const stats = await getGraphStats();
console.log(`Nodes: ${stats.node_count}, Edges: ${stats.edge_count}`);
```

### Build Options

**Feature flags for selective compilation:**
```bash
# Local-only (FAISS, no cloud)
npm run build  # Uses default = ["local"]

# Cloud-only (no FAISS)
npm run build -- --features cloud

# Full build (local + cloud)
npm run build -- --features full
```

**See [NAPI README](crates/codegraph-napi/README.md) for complete documentation.**


## 🤝 Contributing

We welcome contributions!

```bash
# Format code
cargo fmt --all

# Run linter
cargo clippy --workspace --all-targets

# Run tests
cargo test --workspace
```

Open an issue to discuss large changes before starting.

---

## 📄 License

Dual-licensed under MIT and Apache 2.0. See `LICENSE-MIT` and `LICENSE-APACHE` for details.

---

## 📚 Learn More

- **[NAPI Bindings Guide](crates/codegraph-napi/README.md)** - Complete TypeScript integration documentation
- **[Cloud Providers Guide](docs/CLOUD_PROVIDERS.md)** - Detailed cloud provider setup
- **[Configuration Reference](config/.codegraph.toml.example)** - All configuration options
- **[Changelog](CHANGELOG.md)** - Version history and release notes
- **[Legacy Docs](docs/legacy/)** - Historical experiments and architecture notes
