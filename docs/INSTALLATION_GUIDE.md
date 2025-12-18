# CodeGraph Installation & Setup Guide

This guide covers the complete installation process for CodeGraph, from building the binary to configuring the MCP server for use with Claude Code.

## Table of Contents

1. [Prerequisites](#prerequisites)
2. [Building CodeGraph](#building-codegraph)
3. [Setting Up SurrealDB](#setting-up-surrealdb)
4. [Creating the Database Schema](#creating-the-database-schema)
5. [Configuration](#configuration)
6. [Indexing Your Codebase](#indexing-your-codebase)
7. [Running the MCP Server](#running-the-mcp-server)
8. [Daemon Mode](#daemon-mode)
9. [Using Agentic Tools](#using-agentic-tools)

---

## Supported CLI Commands

- `start` / `stop` / `status` — manage the MCP server transports (stdio/http)
- `index` — index a project (supports `--force`, language filters, watch mode)
- `estimate` — estimate indexing time/cost without persisting
- `config` — init/show/set/get/validate configuration; agent-status/db-check live here
- `dbcheck` — quick Surreal connectivity/schema canary
- `daemon` — (feature-gated) file-watch daemon control

Legacy helper commands (`code`, `test`, `perf`, `stats`, `clean`, `init`) are no longer part of the CLI.

---

## Prerequisites

Before installing CodeGraph, ensure you have:

- **macOS** (the installer scripts target macOS; Linux users can adapt the commands)
- **Rust toolchain** - Install from [rustup.rs](https://rustup.rs)
- **Homebrew** - Install from [brew.sh](https://brew.sh)
- **Ollama** (recommended) - Install from [ollama.com](https://ollama.com) for local LLM/embedding support

---

## Building CodeGraph

Use the full-features installation script to build CodeGraph with all capabilities:

```bash
cd /path/to/codegraph-rust
./install-codegraph-full-features.sh
```

This script:
- Installs SurrealDB CLI via Homebrew if not present
- Builds CodeGraph with all features enabled (daemon, AI-enhanced, all providers, LATS)
- Installs the binary to `~/.cargo/bin/codegraph`

### What Gets Enabled

The full-features build includes:
- All embedding providers (Ollama, LM Studio, Jina AI, OpenAI, ONNX)
- All LLM providers (Anthropic, OpenAI, xAI Grok, Ollama, LM Studio)
- Daemon mode (file watching & auto re-indexing)
- HTTP server with SSE streaming
- AutoAgents framework with LATS (Language Agent Tree Search)

### Manual Build (Alternative)

If you prefer to build manually:

```bash
cargo install --path crates/codegraph-mcp-server --bin codegraph \
  --all-features --features autoagents-lats --force
```

---

## Setting Up SurrealDB

CodeGraph requires SurrealDB for graph storage and vector search. You have two options:

### Option 1: Local Installation (Recommended for Development)

```bash
# Install SurrealDB CLI
brew install surrealdb/tap/surreal

# Start SurrealDB with persistent storage
surreal start \
  --bind 0.0.0.0:3004 \
  --user root \
  --pass root \
  file://$HOME/.codegraph/surreal.db
```

For in-memory storage (data lost on restart):
```bash
surreal start --bind 0.0.0.0:3004 --user root --pass root memory
```

### Option 2: Surreal Cloud (Free Tier Available)

1. Sign up at [surrealdb.com/cloud](https://surrealdb.com/cloud)
2. Create a free 1GB instance
3. Note your connection URL, namespace, and credentials

### Option 3: Surrealist IDE

[Surrealist](https://surrealdb.com/surrealist) provides a graphical IDE for SurrealDB that makes database management easy:

- Visual query editor with syntax highlighting
- Schema visualization
- Data browser and editor
- Easy connection management

Download from [surrealdb.com/surrealist](https://surrealdb.com/surrealist) or use the web version.

---

## Creating the Database Schema

After starting SurrealDB, apply the CodeGraph schema:

### Using the Apply Script

```bash
cd /path/to/codegraph-rust/schema

# Apply to local database (defaults: localhost:3004, root/root)
./apply-schema.sh

# Apply with custom settings
./apply-schema.sh \
  --endpoint ws://localhost:3004 \
  --namespace ouroboros \
  --database codegraph \
  --username root \
  --password root
```

### Using SurrealDB CLI Directly

```bash
surreal sql \
  --endpoint ws://localhost:3004 \
  --namespace ouroboros \
  --database codegraph \
  --username root \
  --password root \
  < schema/codegraph.surql
```

### Using Surrealist IDE

1. Open Surrealist and connect to your database
2. Navigate to the Query tab
3. Open `schema/codegraph.surql`
4. Execute the schema

### Verify Schema Installation

```bash
surreal sql \
  --endpoint ws://localhost:3004 \
  --namespace ouroboros \
  --database codegraph \
  --username root \
  --password root \
  --command "INFO FOR DB;"
```

You should see tables like `nodes`, `edges`, `chunks`, `symbol_embeddings`, and functions like `fn::semantic_search_chunks_with_context`.

---

## Configuration

CodeGraph uses a hierarchical configuration system. Settings can be specified via:

1. Global config file (`~/.codegraph/config.toml`)
2. Project-level config file (`.codegraph/config.toml` in project root)
3. Environment variables (with `CODEGRAPH_` prefix)
4. `.env` file in project root

For detailed configuration examples (Ollama/LM Studio/Jina embeddings and OpenAI/xAI/Anthropic/OpenAI-compatible LLMs), see `docs/AI_PROVIDERS.md`.

### Global Configuration (`~/.codegraph/config.toml`)

Create the directory and config file:

```bash
mkdir -p ~/.codegraph
cp config/example.toml ~/.codegraph/config.toml
```

Edit `~/.codegraph/config.toml`:

```toml
# Embedding Configuration
[embedding]
provider = "ollama"                    # ollama | lmstudio | jina | openai
model = "qwen3-embedding:0.6b"         # Model name for your provider
dimension = 1024                       # 384, 768, 1024, 1536, 2048, 2560, 3072, 4096
batch_size = 32
normalize_embeddings = true
cache_enabled = true
ollama_url = "http://localhost:11434"
# lmstudio_url = "http://localhost:1234"  # For LM Studio

# LLM Configuration (for agentic tools)
[llm]
provider = "ollama"                    # ollama | anthropic | openai | xai | lmstudio
model = "qwen3:4b"                     # Model for reasoning
context_window = 32000                 # Affects tier selection for prompts
max_retries = 3

# Reranking (Optional - improves search quality)
[rerank]
provider = "jina"                      # jina | ollama
model = "jina-reranker-v3"
top_n = 10
candidates = 256

# Database Configuration
[database]
backend = "surrealdb"

[database.surrealdb]
connection = "ws://localhost:3004"
namespace = "ouroboros"
database = "codegraph"
# username = "root"
# password is best set via env: CODEGRAPH__DATABASE__SURREALDB__PASSWORD
strict_mode = false
auto_migrate = true

# Server Configuration
[server]
host = "0.0.0.0"
port = 3003

# Performance Tuning
[performance]
batch_size = 64                        # Embedding batch size
workers = 4                            # Rayon thread count
max_concurrent = 4                     # Concurrent embedding requests
max_texts_per_request = 256

# Daemon Configuration (for --watch mode)
[daemon]
auto_start_with_mcp = true             # Auto-start when MCP server starts
debounce_ms = 30
batch_timeout_ms = 200
exclude_patterns = ["**/node_modules/**", "**/target/**", "**/.git/**"]

# Monitoring
[monitoring]
enabled = true
metrics_enabled = true
trace_enabled = false
metrics_interval_secs = 60

# Security
[security]
require_auth = false
rate_limit_per_minute = 1200
```

### Project-Level Configuration

Create `.codegraph/config.toml` in your project root for project-specific overrides:

```toml
# Project-specific settings override global config
[embedding]
model = "jina-embeddings-v4"           # Different model for this project
dimension = 2048

[llm]
provider = "anthropic"
model = "claude-sonnet-4"
```

### Environment Variables

Environment variables override config files. Use the `CODEGRAPH_` prefix:

```bash
# Embedding
export CODEGRAPH_EMBEDDING_PROVIDER=ollama
export CODEGRAPH_EMBEDDING_MODEL=qwen3-embedding:0.6b
export CODEGRAPH_EMBEDDING_DIMENSION=1024
export CODEGRAPH_MAX_CHUNK_TOKENS=32000

# LLM
export CODEGRAPH_LLM_PROVIDER=anthropic
export CODEGRAPH_LLM_MODEL=claude-sonnet-4

# Database
export CODEGRAPH_SURREALDB_URL=ws://localhost:3004
export CODEGRAPH_SURREALDB_NAMESPACE=ouroboros
export CODEGRAPH_SURREALDB_DATABASE=codegraph
export CODEGRAPH_SURREALDB_USERNAME=root
export CODEGRAPH_SURREALDB_PASSWORD=root

# API Keys (for cloud providers)
export ANTHROPIC_API_KEY=sk-ant-...
export OPENAI_API_KEY=sk-...
export JINA_API_KEY=jina_...
export XAI_API_KEY=xai-...

# Agent Architecture
export CODEGRAPH_AGENT_ARCHITECTURE=react    # react | lats

# Debugging
export CODEGRAPH_DEBUG=1                     # Enable debug logging
```

### Using .env File

Create a `.env` file in your project root:

```bash
# Copy the example
cp .env.example .env

# Edit with your settings
```

---

## Indexing Your Codebase

Before using the MCP server, you must index your codebase to create the graph and embeddings.

### Basic Indexing

```bash
# Index current directory recursively
codegraph index . -r

# Index with specific languages
codegraph index . -r -l rust,typescript,python

# Index a specific path
codegraph index /path/to/codebase -r -l rust
```

### Indexing Options

```bash
codegraph index <PATH> [OPTIONS]

Options:
  -r, --recursive           Recursively index subdirectories
  -l, --languages <LANGS>   Comma-separated list of languages to index
                            (rust, python, typescript, javascript, go, java,
                             cpp, c, swift, kotlin, csharp, ruby, php, dart)
  --exclude <PATTERN>       Glob patterns to exclude (can be repeated)
  --force                   Force re-index of all files
  --project-id <ID>         Custom project identifier
```

### Examples

```bash
# Index a Rust project
codegraph index /path/to/rust-project -r -l rust

# Index a full-stack project with multiple languages
codegraph index . -r -l rust,typescript,python --exclude "**/node_modules/**" --exclude "**/target/**"

# Force complete re-index
codegraph index . -r -l rust --force
```

### Verify Indexing

Check that data was indexed:

```bash
surreal sql \
  --endpoint ws://localhost:3004 \
  --namespace ouroboros \
  --database codegraph \
  --username root \
  --password root \
  --command "SELECT count() FROM nodes GROUP ALL; SELECT count() FROM chunks GROUP ALL;"
```

---

## Running the MCP Server

### With Claude Code

Add CodeGraph to your Claude Code MCP configuration. In your `~/.claude/claude_desktop_config.json` or MCP settings:

```json
{
  "mcpServers": {
    "codegraph": {
      "command": "/full/path/to/codegraph",
      "args": ["start", "stdio", "--watch"]
    }
  }
}
```

**Important:** Use the **full absolute path** to the binary. Find it with:

```bash
which codegraph
# Usually: /Users/<username>/.cargo/bin/codegraph
```

### STDIO Mode (Recommended)

```bash
# Start MCP server with file watching
/full/path/to/codegraph start stdio --watch

# Without file watching
/full/path/to/codegraph start stdio

# Watch a specific directory
/full/path/to/codegraph start stdio --watch --watch-path /path/to/project
```

### HTTP Mode (For Web Clients)

```bash
# Start HTTP server with SSE streaming
codegraph start http --host 127.0.0.1 --port 3000

# Test the endpoint
curl http://127.0.0.1:3000/health
```

### The `--watch` Flag

When you include `--watch`, the MCP server automatically:
- Monitors your project for file changes
- Re-indexes modified files in the background
- Keeps search results up-to-date

Without `--watch`, you must manually re-run `codegraph index` after making changes.

---

## Daemon Mode

For more control over file watching, use the standalone daemon:

### Starting the Daemon

```bash
# Start watching a project (runs in background)
codegraph daemon start /path/to/project

# Start in foreground (for debugging)
codegraph daemon start /path/to/project --foreground

# Filter by languages
codegraph daemon start /path/to/project --languages rust,typescript

# Exclude patterns
codegraph daemon start /path/to/project \
  --exclude "**/node_modules/**" \
  --exclude "**/target/**"
```

### Managing the Daemon

```bash
# Check daemon status
codegraph daemon status /path/to/project
codegraph daemon status /path/to/project --json

# Stop the daemon
codegraph daemon stop /path/to/project
```

### Daemon vs --watch

| Feature | `--watch` flag | `daemon` command |
|---------|----------------|------------------|
| Runs with MCP server | Yes | Independent |
| Background process | No (inline) | Yes |
| Multiple projects | No | Yes |
| Fine-grained control | Limited | Full |
| Status monitoring | No | Yes |

**Use `--watch`** for simple single-project setups with Claude Code.
**Use `daemon`** for complex multi-project environments or when you need independent control.

---

## Using Agentic Tools

CodeGraph provides 8 agentic MCP tools that use multi-step reasoning to analyze your codebase:

### Available Tools

| Tool | Purpose | When to Use |
|------|---------|-------------|
| `agentic_code_search` | Semantic code search with AI insights | Finding code patterns, exploring unfamiliar codebases |
| `agentic_dependency_analysis` | Analyze dependencies and impact | Before refactoring, understanding coupling |
| `agentic_call_chain_analysis` | Trace execution paths | Debugging, understanding data flow |
| `agentic_architecture_analysis` | Assess system architecture | Architecture reviews, onboarding |
| `agentic_api_surface_analysis` | Analyze public interfaces | API design reviews, breaking change detection |
| `agentic_context_builder` | Gather comprehensive context | Before implementing new features |
| `agentic_semantic_question` | Answer complex codebase questions | Deep understanding questions |
| `agentic_complexity_analysis` | Identifies high-risk code hotspots for refactoring | architectural flaws and complexities |

### Example Queries

In Claude Code, simply ask questions - the agentic tools will be used automatically:

**Code Search:**
```
"Find all places where authentication is handled in this codebase"
```

**Dependency Analysis:**
```
"What would be affected if I change the UserService class?"
```

**Call Chain Analysis:**
```
"Trace the execution path from HTTP request to database query in the payment flow"
```

**Architecture Analysis:**
```
"Give me an overview of this project's architecture and main components"
```

**API Surface:**
```
"What public APIs does the auth module expose?"
```

**Context Building:**
```
"I need to add rate limiting to the API. Gather all relevant context."
```

### Agent Architecture Selection

CodeGraph supports two reasoning architectures:

**ReAct (Default)** - Fast, single-pass reasoning:
```bash
export CODEGRAPH_AGENT_ARCHITECTURE=react
```

**LATS** - Deeper, tree-search reasoning (slower but more thorough):
```bash
export CODEGRAPH_AGENT_ARCHITECTURE=lats
```

### Tier-Aware Prompting

The agent automatically adjusts its behavior based on the context window of the LLM you configured for CodeGraph (set .env CODEGRAPH_CONTEXT_WINDOW=ctx):

| Tier | Context Window | Behavior |
|------|----------------|----------|
| Small | < 50K tokens | Terse prompts, 5 max steps |
| Medium | 50K-150K | Balanced prompts, 10 max steps |
| Large | 150K-500K | Detailed prompts, 15 max steps |
| Massive | > 500K | Exploratory prompts, 20 max steps |

This means you can use smaller local models for quick queries and larger cloud models for comprehensive analysis.

### Debugging Agent Behavior

Enable debug logging to see agent reasoning:

```bash
export CODEGRAPH_DEBUG=1
```

View debug logs:
```bash
python tools/view_debug_logs.py --follow
```

---

## Troubleshooting

### Common Issues

**"Cannot connect to SurrealDB"**
- Ensure SurrealDB is running: `surreal start ...`
- Check the connection URL matches your config
- Verify namespace and database exist

**"No embeddings found"**
- Ensure you've indexed the project: `codegraph index . -r -l <languages>`
- Check your embedding provider is running (Ollama, LM Studio, etc.)

**"GraphFunctions not initialized"**
- Apply the schema: `./schema/apply-schema.sh`
- Verify functions exist: `INFO FOR DB;`

**MCP server not appearing in Claude Code**
- Use the full absolute path to the binary
- Check Claude Code's MCP logs for errors
- Verify the binary is executable: `chmod +x /path/to/codegraph`

### Getting Help

- Check logs with `CODEGRAPH_DEBUG=1`
- View the README for additional configuration options
- Check `codegraph --help` for CLI documentation
