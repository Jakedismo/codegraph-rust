# CodeGraph MCP Intelligence Platform

🚀 **Revolutionary AI development intelligence platform with Qwen2.5-Coder-14B-128K integration**

**Transform any MCP-compatible LLM into a codebase expert through semantic intelligence**

[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.75%2B-orange.svg)](https://www.rust-lang.org/)
[![MCP](https://img.shields.io/badge/MCP-Compatible-green.svg)](https://modelcontextprotocol.io)
[![Qwen](https://img.shields.io/badge/Qwen2.5--Coder-14B--128K-blue.svg)](https://huggingface.co/unsloth/Qwen2.5-Coder-14B-Instruct-128K-GGUF)

## 📋 Table of Contents

- [Overview](#overview)
- [Features](#features)
- [Architecture](#architecture)
- [Prerequisites](#prerequisites)
- [Installation](#installation)
- [Quick Start](#quick-start)
- [CLI Commands](#cli-commands)
- [Configuration](#configuration)
- [User Workflows](#user-workflows)
- [Integration Guide](#integration-guide)
- [Troubleshooting](#troubleshooting)
- [Contributing](#contributing)
- [License](#license)

## 🎯 Revolutionary Overview

CodeGraph is the **first MCP-based codebase intelligence platform** that transforms any compatible LLM (Claude-4[1m], GPT-5, custom agents) into a codebase expert through advanced semantic analysis enhanced by **Qwen2.5-Coder-14B-128K**.

### 🧠 **Core Innovation: MCP-First Intelligence**

**Architecture**: `Cloud LLMs ↔ MCP Protocol ↔ CodeGraph Server ↔ Qwen2.5-Coder-14B-128K`

Any MCP-compatible AI agent can now:
- **Understand your specific codebase** like a senior team member
- **Predict change impacts** before modifications are made
- **Generate code following your team's exact patterns**
- **Provide architectural insights** impossible with generic AI

### 🚀 **Revolutionary Capabilities**

- **🧠 Semantic Intelligence**: Qwen2.5-Coder-14B with 128K context for complete codebase understanding
- **⚡ Impact Prediction**: Shows what breaks BEFORE you make changes (revolutionary!)
- **🎯 Team Intelligence**: Learns and shares your team's coding patterns and conventions
- **💾 Intelligent Caching**: Semantic similarity matching for 50-80% cache hit rates
- **📊 Pattern Detection**: Analyzes team conventions using existing 90K+ lines of semantic analysis
- **🔗 MCP Protocol**: Works with Claude Code, Codex CLI, Gemini CLi, Crush, Qwen-Code, and any MCP-compatible agent

## 🎯 **Revolutionary MCP Tools (6 Available)**

### **✅ Available Immediately (No Model Required)**
- **`codegraph.pattern_detection`**: Team intelligence and coding convention analysis
- **`vector.search`**: Advanced semantic search using FAISS + 90K lines of analysis
- **`graph.neighbors` & `graph.traverse`**: Code relationship exploration
- **`codegraph.performance_metrics`**: Real-time system monitoring
- **`tools/list`**: MCP protocol compliance

### **🧠 Available Once Qwen2.5-Coder Downloads**
- **`codegraph.enhanced_search`**: Semantic search + AI analysis (2-3 seconds)
- **`codegraph.semantic_intelligence`**: Comprehensive codebase analysis (4-6 seconds)
- **`codegraph.impact_analysis`**: Revolutionary change impact prediction (3-5 seconds)

## ⚡ **Performance Achievements**

### **Existing Performance (Proven)**
```bash
Parsing: 170K lines in 0.49 seconds (342,852 lines/sec)
Embeddings: 21,024 embeddings in 3:24 minutes
Platform: M3 Pro 32GB (optimal for Qwen2.5-Coder-14B)
```

### **Revolutionary Performance (Validated)**
```bash
TypeScript Extraction: 2,836 nodes from 2,871 lines (BREAKTHROUGH!)
Enhanced Search: 18s first run, cached for millisecond responses
Impact Analysis: 2.7s with structured risk assessment
Pattern Detection: Instant team intelligence analysis
Semantic Analysis: 90% confidence with 128K context window
Memory Usage: ~24GB VRAM (fits 32GB MacBook Pro perfectly)
```

### **Complete Local Stack Performance**
```bash
Qwen2.5-Coder-14B-128K: SOTA code analysis (294-540 context tokens used)
nomic-embed-code: Code-specialized embeddings (3584 dimensions)
FAISS Indexing: High-performance vector search
Intelligent Caching: Semantic similarity matching for speed
Zero External Dependencies: 100% local processing
```

## 🎯 **Success Indicators**

### ✅ **Working Correctly When You See:**
- Build completes without FAISS or model errors
- TypeScript indexing generates 100+ nodes (not 0)
- MCP server shows "Qwen2.5-Coder availability: true"
- Enhanced search returns comprehensive analysis in 3-20 seconds
- Cache hit rates improve with repeated queries
- Claude Desktop shows CodeGraph as connected MCP server

### 🚨 **Needs Attention When You See:**
- Build errors about missing FAISS libraries → Check installation steps
- "0 nodes generated" → Language extraction issue (should be fixed!)
- "Model not found" errors → Install required Ollama models
- Response times >30 seconds → Memory pressure or model loading
- Generic AI responses → Qwen not being used or context not loaded

## 📈 **Expected Results**

### **First-Time Setup**
- Model download: 5-30 minutes (8.4GB + 274MB)
- Initial build: 2-5 minutes with all features
- First indexing: 1-10 seconds depending on codebase size
- First analysis: 10-20 seconds (then cached for speed)

### **Daily Usage**
- Subsequent indexing: Sub-second for small changes
- Cached responses: Milliseconds for repeated queries
- New analysis: 3-10 seconds for comprehensive insights
- Team intelligence: Instant pattern detection and recommendations

## ✨ Features

### Core Features

- **Project Indexing**
  - Multi-language support (Rust, Python, JavaScript, TypeScript, Go, Java, C++)
  - Incremental indexing with file watching
  - Parallel processing with configurable workers
  - Smart caching for improved performance

- **MCP Server Management**
  - STDIO transport for direct communication
  - HTTP streaming with SSE support
  - Dual transport mode for maximum flexibility
  - Background daemon mode with PID management

- **Code Search**
  - Semantic search using embeddings
  - Exact match and fuzzy search
  - Regex and AST-based queries
  - Configurable similarity thresholds

- **Architecture Analysis**
  - Component relationship mapping
  - Dependency analysis
  - Code pattern detection
  - Architecture visualization support

## 🏗️ Architecture

```
CodeGraph System Architecture
┌─────────────────────────────────────────────────────┐
│                   CLI Interface                     │
│                  (codegraph CLI)                    │
└─────────────────────────────────────────────────────┘
                           │
                           ▼
┌─────────────────────────────────────────────────────┐
│                   Core Engine                       │
│  ┌─────────────┐  ┌──────────────┐  ┌────────────┐  │
│  │   Parser    │  │  Graph Store │  │   Vector   │  │ 
│  │ (Tree-sittr)│  │  (RocksDB)   │  │   Search   │  │
│  └─────────────┘  └──────────────┘  │  (FAISS)   │  │
│                                     └────────────┘  │
└─────────────────────────────────────────────────────┘
                           │
                           ▼
┌─────────────────────────────────────────────────────┐
│                  MCP Server Layer                   │
│  ┌─────────────┐  ┌──────────────┐  ┌────────────┐  │
│  │    STDIO    │  │     HTTP     │  │    Dual    │  │
│  │  Transport  │  │  Transport   │  │    Mode    │  │
│  └─────────────┘  └──────────────┘  └────────────┘  │
└─────────────────────────────────────────────────────┘
```

## 🧠 Embeddings with ONNX Runtime (macOS)

- Default provider: CPU EP. Works immediately with Homebrew `onnxruntime`.
- Optional CoreML EP: Set `CODEGRAPH_ONNX_EP=coreml` to prefer CoreML when using an ONNX Runtime build that includes CoreML.
- Fallback: If CoreML EP init fails, CodeGraph logs a warning and falls back to CPU.

How to use ONNX embeddings

```bash
# CPU-only (default)
export CODEGRAPH_EMBEDDING_PROVIDER=onnx
export CODEGRAPH_ONNX_EP=cpu
export CODEGRAPH_LOCAL_MODEL=/path/to/onnx-file

# CoreML (requires CoreML-enabled ORT build)
export CODEGRAPH_EMBEDDING_PROVIDER=onnx
export CODEGRAPH_ONNX_EP=coreml
export CODEGRAPH_LOCAL_MODEL=/path/to/onnx-file


# Install codegraph
cargo install --path crates/codegraph-mcp --features "embeddings,codegraph-vector/onnx,faiss"
```

Notes

- ONNX Runtime on Apple platforms accelerates via CoreML, not Metal. If you need GPU acceleration on Apple Silicon, use CoreML where supported.
- Some models/operators may still run on CPU if CoreML doesn’t support them.

Enabling CoreML feature at build time

- The CoreML registration path is gated by the Cargo feature `onnx-coreml` in `codegraph-vector`.
- Build with: `cargo build -p codegraph-vector --features "onnx,onnx-coreml"`
- In a full workspace build, enable it via your consuming crate’s features or by adding: `--features codegraph-vector/onnx,codegraph-vector/onnx-coreml`.
- You still need an ONNX Runtime library that was compiled with CoreML support; the feature only enables the registration call in our code.

## 📦 Prerequisites

### System Requirements

- **Operating System**: Linux, macOS, or Windows
- **Rust**: 1.75 or higher
- **Memory**: Minimum 4GB RAM (8GB recommended for large codebases)
- **Disk Space**: 1GB for installation + space for indexed data

### Required Dependencies

```bash
# macOS
brew install cmake clang

# Ubuntu/Debian
sudo apt-get update
sudo apt-get install cmake clang libssl-dev pkg-config

# Fedora/RHEL
sudo dnf install cmake clang openssl-devel
```

### Optional Dependencies

- **FAISS** (for vector search acceleration)
  ```bash
  # macOS (required for FAISS feature)
  brew install faiss

  # Ubuntu/Debian
  sudo apt-get install libfaiss-dev

  # Fedora/RHEL
  sudo dnf install faiss-devel
  ```
- **Local Embeddings (HuggingFace + Candle + ONNX/ORT(coreML) osx-metal/cuda/cpu)**
  - Enables on-device embedding generation (no external API calls)
  - Downloads models from HuggingFace Hub on first run and caches them locally
  - Internet access required for the initial model download (or pre-populate cache)
  - Default runs on CPU; advanced GPU backends (CUDA/Metal) require appropriate hardware and drivers
- **CUDA** (for GPU-accelerated embeddings)
- **Git** (for repository integration)

## 🚀 Performance Benchmarks - pure raw speed!

Run repeatable, end-to-end benchmarks that measure indexing speed (with local embeddings + FAISS), vector search latency, and graph traversal throughput.

For reference indexing this repository with the example configuration yields the following:
```bash
2025-09-19T14:27:46.632335Z  INFO codegraph_parser::parser: Parsing completed: 361/361 files, 119401 lines in 0.08s (4485.7 files/s, 1483642 lines/s)
[00:00:51] [########################################] 14096/14096 Embeddings complete
```
Apple Macbook Pro M4 Max 128Gb 2025 onnx

### Build with performance features

Pick one of the local embedding backends and enable FAISS:

```bash
# Option A: ONNX Runtime (CoreML on macOS, CPU otherwise)
cargo install --path crates/codegraph-mcp --features "embeddings,codegraph-vector/onnx,faiss"

# Option B: Local HF + Candle (CPU/Metal/CUDA)
cargo install --path crates/codegraph-mcp --features "embeddings-local,faiss"
```

### Configure local embedding backend

ONNX (CoreML/CPU):
```bash
brew install huggingface_hub[cli]
hf auth login
hf download Qdrant/all-MiniLM-L6-v2
# Check download path
# Best to add these to your shell provider config
export CODEGRAPH_EMBEDDING_PROVIDER=onnx
# macOS: use CoreML
export CODEGRAPH_ONNX_EP=coreml   # or cpu
export CODEGRAPH_LOCAL_MODEL=/path/to/model/(not directly to .onnx)
```

Local HF + Candle (CPU/Metal/CUDA):

```bash
export CODEGRAPH_EMBEDDING_PROVIDER=local
# device: cpu | metal | cuda:<id>
export CODEGRAPH_LOCAL_MODEL=Qdrant/all-MiniLM-L6-v2
```

### Run the benchmark

```bash
# Cold run (cleans .codegraph), warmup queries + timed trials
codegraph perf . \
  --langs rust,ts,go \
  --warmup 3 --trials 20 \
  --batch-size 512 --device metal \
  --clean --format json
```

What it measures

- Indexing: total time to parse -> embed -> build FAISS (global + shards)
- Embedding throughput: embeddings per second
- Vector search: latency (avg/p50/p95) across repeated queries
- Graph traversal: BFS depth=2 micro-benchmark

Sample output (numbers will vary by machine and codebase)

```json
{
  "env": {
    "embedding_provider": "local",
    "device": "metal",
    "features": { "faiss": true, "embeddings": true }
  },
  "dataset": {
    "path": "/repo/large-project",
    "languages": ["rust","ts","go"],
    "files": 18234,
    "lines": 2583190
  },
  "indexing": {
    "total_seconds": 186.4,
    "embeddings": 53421,
    "throughput_embeddings_per_sec": 286.6
  },
  "vector_search": {
    "queries": 100,
    "latency_ms": { "avg": 18.7, "p50": 12.3, "p95": 32.9 }
  },
  "graph": {
    "bfs_depth": 2,
    "visited_nodes": 1000,
    "elapsed_ms": 41.8
  }
}
```

Tips for reproducibility

- Use `--clean` for cold start numbers, and run a second time for warm cache numbers.
- Close background processes that may compete for CPU/GPU.
- Pin versions: `rustc --version`, FAISS build, and the embedding model.
- Record the host: CPU/GPU, RAM, storage, OS version.

## 🚀 **Complete Installation Guide**

### **Prerequisites**
- **Hardware**: 32GB RAM recommended (24GB minimum)
- **OS**: macOS 11.0+ (or Linux with FAISS support)
- **Rust**: 1.75+ with Cargo
- **Ollama**: For local model serving

### **Step 1: Install System Dependencies**

```bash
# macOS: Install FAISS for vector search
brew install faiss

# Verify FAISS installation
ls /opt/homebrew/opt/faiss/lib/

# Install Ollama for local models
curl -fsSL https://ollama.com/install.sh | sh
ollama serve &
```

### **Step 2: Install SOTA Models**

```bash
# Install Qwen2.5-Coder-14B-128K (SOTA code analysis)
ollama pull hf.co/unsloth/Qwen2.5-Coder-14B-Instruct-128K-GGUF:Q4_K_M

# Install nomic-embed-code (SOTA code embeddings)
ollama pull hf.co/nomic-ai/nomic-embed-code-GGUF:Q4_K_M

# Verify models installed
ollama list | grep -E "qwen|nomic"
```

### **Step 3: Build CodeGraph with Complete Features**

```bash
# Build with all revolutionary features
LIBRARY_PATH="/opt/homebrew/opt/faiss/lib:$LIBRARY_PATH" \
LD_LIBRARY_PATH="/opt/homebrew/opt/faiss/lib:$LD_LIBRARY_PATH" \
MACOSX_DEPLOYMENT_TARGET=11.0 \
cargo build --release -p codegraph-mcp \
  --features "qwen-integration,faiss,embeddings,embeddings-ollama"

# Verify build
./target/release/codegraph --version
```

### **Step 4: Environment Configuration**

```bash
# Configure for complete local stack
export CODEGRAPH_MODEL="hf.co/unsloth/Qwen2.5-Coder-14B-Instruct-128K-GGUF:Q4_K_M"
export CODEGRAPH_EMBEDDING_PROVIDER=ollama
export CODEGRAPH_EMBEDDING_MODEL=nomic-embed-code
export RUST_LOG=info
```

---

## 🚀 **Revolutionary Quick Start**

### **Step 1: Initialize Your Project**

```bash
# Navigate to your codebase
cd /path/to/your/project

# Initialize CodeGraph (creates .codegraph directory)
/path/to/codegraph-rust/target/release/codegraph init .

# Expected output:
# ✓ Created .codegraph/config.toml
# ✓ Created .codegraph/db/
# ✓ Created .codegraph/vectors/
# ✓ Created .codegraph/cache/
```

### **Step 2: Index Your Codebase**

```bash
# Index with code-specialized embeddings (recommended)
LIBRARY_PATH="/opt/homebrew/opt/faiss/lib:$LIBRARY_PATH" \
LD_LIBRARY_PATH="/opt/homebrew/opt/faiss/lib:$LD_LIBRARY_PATH" \
CODEGRAPH_EMBEDDING_PROVIDER=ollama \
./target/release/codegraph index . --recursive --languages typescript,javascript,rust,python

# Expected results:
# ✅ Files indexed: 100+ (depends on project size)
# ✅ Embeddings: Code-specialized vectors with nomic-embed-code
# ✅ FAISS index: High-performance semantic search ready
```

### **Step 3: Start Revolutionary MCP Server**

```bash
# Start MCP server for Claude Desktop/GPT-4 integration
CODEGRAPH_MODEL="hf.co/unsloth/Qwen2.5-Coder-14B-Instruct-128K-GGUF:Q4_K_M" \
RUST_LOG=error \
./target/release/codegraph start stdio

# Expected output:
# ✅ Qwen2.5-Coder-14B-128K available for CodeGraph intelligence
# ✅ Intelligent response cache initialized
# MCP server ready for connections
```

### **Step 4: Configure Claude Desktop**

Add to your Claude Desktop configuration:
```json
{
  "mcpServers": {
    "codegraph": {
      "command": "/path/to/codegraph-rust/target/release/codegraph",
      "args": ["start", "stdio"],
      "cwd": "/path/to/your/project",
      "env": {
        "RUST_LOG": "error",
        "CODEGRAPH_MODEL": "hf.co/unsloth/Qwen2.5-Coder-14B-Instruct-128K-GGUF:Q4_K_M",
        "CODEGRAPH_EMBEDDING_PROVIDER": "ollama"
      }
    }
  }
}
```

### **Step 5: Experience Revolutionary AI**

Restart Claude Desktop and test:
```
"Analyze the coding patterns and architecture in this codebase"
→ Claude gets team intelligence from your semantic analysis

"What would happen if I modify the authentication system?"
→ Claude predicts impact before you make changes

"Find all GraphQL-related code and explain the patterns"
→ Claude uses code-specialized search with perfect relevance
```

---

## 📊 **Embedding Provider Options**

### **Ollama (Recommended - Code-Specialized)**
```bash
export CODEGRAPH_EMBEDDING_PROVIDER=ollama
export CODEGRAPH_EMBEDDING_MODEL=nomic-embed-code

# Benefits:
# - Code-specialized understanding
# - Superior semantic search relevance
# - Local processing, zero external dependencies
```

### **ONNX (Alternative - Speed Optimized)**
```bash
export CODEGRAPH_EMBEDDING_PROVIDER=onnx
export CODEGRAPH_LOCAL_MODEL=sentence-transformers/all-MiniLM-L6-v2

# Benefits:
# - Faster embedding generation
# - Lower memory usage
# - Good general-purpose embeddings
```

### Enabling Local Embeddings (Optional)

If you want to use a local embedding model (Hugging Face) instead of remote providers:

1) Build with the local embeddings feature for crates that use vector search (the API and/or CLI server):
! Recommended to use the onnx version for better performance, see the begginning of the README for installation instructions

```bash
# Build API with local embeddings enabled
cargo build -p codegraph-api --features codegraph-vector/local-embeddings

# (Optional) If your CLI server crate depends on vector features, enable similarly:
cargo build -p core-rag-mcp-server --features codegraph-vector/local-embeddings
```

2) Set environment variables to switch the provider at runtime:

```bash
export CODEGRAPH_EMBEDDING_PROVIDER=local
# Optional: choose a specific HF model (must provide onnx model)
export CODEGRAPH_LOCAL_MODEL=path/to/Qdrant/all-MiniLM-L6-v2
```

3) Run as usual (the first run will download model files from Hugging Face and cache them locally):

```bash
cargo run -p codegraph-api --features codegraph-vector/local-embeddings
```

Model cache locations:

- Default Hugging Face cache: `~/.cache/huggingface` (or `$HF_HOME`) via `hf-hub`
- You can pre-populate this cache to run offline after the first download

```

### Method 2: Install Pre-built Binary

```bash
# Download the latest release
curl -L https://github.com/jakedismo/codegraph-cli-mcp/releases/latest/download/codegraph-$(uname -s)-$(uname -m).tar.gz | tar xz

# Move to PATH
sudo mv codegraph /usr/local/bin/

# Verify installation
codegraph --version
```

### Method 3: Using Cargo

```bash
# Install directly from crates.io (when published)
cargo install codegraph-mcp

# Verify installation
codegraph --version
```

## 🎯 Quick Start

### 1. Initialize a New Project

```bash
# Initialize CodeGraph in current directory
codegraph init

# Initialize with project name
codegraph init --name my-project
```

### 2. Index Your Codebase

```bash
# Index current directory
codegraph index .

# Index with specific languages
codegraph index . --languages rust,python,typescript

# Or with more options in Osx
RUST_LOG=info,codegraph_vector=debug codegraph index . --workers 10 --batch-size 256 --max-seq-len 512 --force                                                    

# Index with file watching
codegraph index . --watch
```

### 3. Start MCP Server

```bash
# Start with STDIO transport (default)
codegraph start stdio

# Start with HTTP transport
codegraph start http --port 3000

# Start with dual transport
codegraph start dual --port 3000

### (Optional) Start with Local Embeddings

```bash
# Build with the feature (see installation step above), then:
export CODEGRAPH_EMBEDDING_PROVIDER=local
export CODEGRAPH_LOCAL_MODEL=Qdrant/all-MiniLM-L6-v2
cargo run -p codegraph-api --features codegraph-vector/local-embeddings
```

### 4. Search Your Code

```bash
# Semantic search
codegraph search "authentication handler"

# Exact match search
codegraph search "fn authenticate" --search-type exact

# AST-based search
codegraph search "function with async keyword" --search-type ast
```

## 📖 CLI Commands

### Global Options

```bash
codegraph [OPTIONS] <COMMAND>

Options:
  -v, --verbose         Enable verbose logging
  --config <PATH>       Configuration file path
  -h, --help           Print help
  -V, --version        Print version
```

### Command Reference

#### `init` - Initialize CodeGraph Project

```bash
codegraph init [OPTIONS] [PATH]

Arguments:
  [PATH]               Project directory (default: current directory)

Options:
  --name <NAME>        Project name
  --non-interactive    Skip interactive setup
```

#### `start` - Start MCP Server

```bash
codegraph start <TRANSPORT> [OPTIONS]

Transports:
  stdio                STDIO transport (default)
  http                 HTTP streaming transport
  dual                 Both STDIO and HTTP

Options:
  --config <PATH>      Server configuration file
  --daemon             Run in background
  --pid-file <PATH>    PID file location

HTTP Options:
  -h, --host <HOST>    Host to bind (default: 127.0.0.1)
  -p, --port <PORT>    Port to bind (default: 3000)
  --tls                Enable TLS/HTTPS
  --cert <PATH>        TLS certificate file
  --key <PATH>         TLS key file
  --cors               Enable CORS
```

#### `stop` - Stop MCP Server

```bash
codegraph stop [OPTIONS]

Options:
  --pid-file <PATH>    PID file location
  -f, --force          Force stop without graceful shutdown
```

#### `status` - Check Server Status

```bash
codegraph status [OPTIONS]

Options:
  --pid-file <PATH>    PID file location
  -d, --detailed       Show detailed status information
```

#### `index` - Index Project

```bash
codegraph index <PATH> [OPTIONS]

Arguments:
  <PATH>               Path to project directory

Options:
  -l, --languages <LANGS>     Languages to index (comma-separated)
  --exclude <PATTERNS>        Exclude patterns (gitignore format)
  --include <PATTERNS>        Include only these patterns
  -r, --recursive             Recursively index subdirectories
  --force                     Force reindex
  --watch                     Watch for changes
  --workers <N>               Number of parallel workers (default: 4)
```

#### `search` - Search Indexed Code

```bash
codegraph search <QUERY> [OPTIONS]

Arguments:
  <QUERY>              Search query

Options:
  -t, --search-type <TYPE>    Search type (semantic|exact|fuzzy|regex|ast)
  -l, --limit <N>             Maximum results (default: 10)
  --threshold <FLOAT>         Similarity threshold 0.0-1.0 (default: 0.7)
  -f, --format <FORMAT>       Output format (human|json|yaml|table)
```

#### `config` - Manage Configuration

```bash
codegraph config <ACTION> [OPTIONS]

Actions:
  show                 Show current configuration
  set <KEY> <VALUE>    Set configuration value
  get <KEY>            Get configuration value
  reset                Reset to defaults
  validate             Validate configuration

Options:
  --json               Output as JSON (for 'show')
  -y, --yes            Skip confirmation (for 'reset')
```

#### `stats` - Show Statistics

```bash
codegraph stats [OPTIONS]

Options:
  --index              Show index statistics
  --server             Show server statistics
  --performance        Show performance metrics
  -f, --format <FMT>   Output format (table|json|yaml|human)
```

#### `clean` - Clean Resources

```bash
codegraph clean [OPTIONS]

Options:
  --index              Clean index database
  --vectors            Clean vector embeddings
  --cache              Clean cache files
  --all                Clean all resources
  -y, --yes            Skip confirmation prompt
```

## ⚙️ Configuration

### Configuration File Structure

Create a `.codegraph/config.toml` file:

```toml
# General Configuration
[general]
project_name = "my-project"
version = "1.0.0"
log_level = "info"

# Indexing Configuration
[indexing]
languages = ["rust", "python", "typescript", "javascript", "go"]
exclude_patterns = ["**/node_modules/**", "**/target/**", "**/.git/**"]
include_patterns = ["src/**", "lib/**"]
recursive = true
workers = 10
watch_enabled = false
incremental = true

# Embedding Configuration
[embedding]
model = "local"  # Options: openai, local, custom
dimension = 1536
batch_size = 512
cache_enabled = true
cache_size_mb = 500

# Vector Search Configuration
[vector]
index_type = "flat"  # Options: flat, ivf, hnsw
nprobe = 10
similarity_metric = "cosine"  # Options: cosine, euclidean, inner_product

# Database Configuration
[database]
path = "~/.codegraph/db"
cache_size_mb = 128
compression = true
write_buffer_size_mb = 64

# Server Configuration
[server]
default_transport = "stdio"
http_host = "127.0.0.1"
http_port = 3005
enable_tls = false
cors_enabled = true
max_connections = 100

# Performance Configuration
[performance]
max_file_size_kb = 1024
parallel_threads = 8
memory_limit_mb = 2048
optimization_level = "balanced"  # Options: speed, balanced, memory
```

### Environment Variables

```bash
# Override configuration with environment variables
export CODEGRAPH_LOG_LEVEL=debug
export CODEGRAPH_DB_PATH=/custom/path/db
export CODEGRAPH_EMBEDDING_MODEL=local
export CODEGRAPH_HTTP_PORT=8080
```

### Embedding Model Configuration

#### OpenAI Embeddings

```toml
[embedding.openai]
api_key = "${OPENAI_API_KEY}"  # Use environment variable
model = "text-embedding-3-large"
dimension = 3072
```

#### Local Embeddings

```toml
[embedding.local]
model_path = "~/.codegraph/models/codestral.gguf"
device = "cpu"  # Options: cpu, cuda, metal
context_length = 8192
```

## 📚 User Workflows

### Workflow 1: Complete Project Setup and Analysis

```bash
# Step 1: Initialize project
codegraph init --name my-awesome-project

# Step 2: Configure settings
codegraph config set embedding.model local
codegraph config set performance.optimization_level speed

# Step 3: Index the codebase
codegraph index . --languages rust,python --recursive

# Step 4: Start MCP server
codegraph start http --port 3000 --daemon

# Step 5: Search and analyze
codegraph search "database connection" --limit 20
codegraph stats --index --performance
```

### Workflow 2: Continuous Development with Watch Mode

```bash
# Start indexing with watch mode
codegraph index . --watch --workers 8 &

# Start MCP server in dual mode
codegraph start dual --daemon

# Monitor changes
codegraph status --detailed

# Search while developing
codegraph search "TODO" --search-type exact
```

### Workflow 3: Integration with AI Tools

```bash
# Start MCP server for Claude Desktop or VS Code
codegraph start stdio

# Configure for AI assistant integration
cat > ~/.codegraph/mcp-config.json << EOF
{
  "name": "codegraph-server",
  "version": "1.0.0",
  "tools": [
    {
      "name": "analyze_architecture",
      "description": "Analyze codebase architecture"
    },
    {
      "name": "find_patterns",
      "description": "Find code patterns and anti-patterns"
    }
  ]
}
EOF
```

### Workflow 4: Large Codebase Optimization

```bash
# Optimize for large codebases
codegraph config set performance.memory_limit_mb 8192
codegraph config set vector.index_type ivf
codegraph config set database.compression true

# Index with optimizations
codegraph index /path/to/large/project \
  --workers 16 \
  --exclude "**/test/**,**/vendor/**"

# Use batch operations
codegraph search "class.*Controller" --search-type regex --limit 100
```

## 🔌 Integration Guide

### Integrating with Claude Desktop

1. Add to Claude Desktop configuration:

```json
{
  "mcpServers": {
    "codegraph": {
      "command": "codegraph",
      "args": ["start", "stdio"],
      "env": {
        "CODEGRAPH_CONFIG": "~/.codegraph/config.toml"
      }
    }
  }
}
```

2. Restart Claude Desktop to load the MCP server

### Integrating with VS Code

1. Install the MCP extension for VS Code
2. Add to VS Code settings:

```json
{
  "mcp.servers": {
    "codegraph": {
      "command": "codegraph",
      "args": ["start", "stdio"],
      "rootPath": "${workspaceFolder}"
    }
  }
}
```

### API Integration

```python
import requests
import json

# Connect to HTTP MCP server
base_url = "http://localhost:3000"

# Index a project
response = requests.post(f"{base_url}/index", json={
    "path": "/path/to/project",
    "languages": ["python", "javascript"]
})

# Search code
response = requests.post(f"{base_url}/search", json={
    "query": "async function",
    "limit": 10
})

results = response.json()
```

### Using with CI/CD

```yaml
# GitHub Actions example
name: CodeGraph Analysis

on: [push, pull_request]

jobs:
  analyze:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v2
      
      - name: Install CodeGraph
        run: |
          cargo install codegraph-mcp
      
      - name: Index Codebase
        run: |
          codegraph init --non-interactive
          codegraph index . --languages rust,python
      
      - name: Run Analysis
        run: |
          codegraph stats --index --format json > analysis.json
      
      - name: Upload Results
        uses: actions/upload-artifact@v2
        with:
          name: codegraph-analysis
          path: analysis.json
```

## 🔧 Troubleshooting

### Common Issues and Solutions

#### Issue: Server fails to start

**Solution:**

```bash
# Check if port is already in use
lsof -i :3000

# Kill existing process
codegraph stop --force

# Start with different port
codegraph start http --port 3001
```

#### Issue: Indexing is slow

**Solution:**

```bash
# Increase workers
codegraph index . --workers 16

# Exclude unnecessary files
codegraph index . --exclude "**/node_modules/**,**/dist/**"

# Use incremental indexing
codegraph config set indexing.incremental true
```

#### Issue: Out of memory during indexing

**Solution:**

```bash
# Reduce batch size
codegraph config set embedding.batch_size 50

# Limit memory usage
codegraph config set performance.memory_limit_mb 1024

# Use streaming mode
codegraph index . --streaming
```

#### Issue: Vector search returns poor results

**Solution:**

```bash
# Adjust similarity threshold
codegraph search "query" --threshold 0.5

# Re-index with better embeddings
codegraph config set embedding.model openai
codegraph index . --force

# Use different search type
codegraph search "query" --search-type fuzzy

#### Issue: Hugging Face model fails to download

**Solution:**
```bash
# Ensure you have internet access and the model name is correct
export CODEGRAPH_LOCAL_MODEL=Qdrant/all-MiniLM-L6-v2

# If the model is private, set a HF token (if required by your environment)
export HF_TOKEN=your_hf_access_token

# Clear/inspect cache (default): ~/.cache/huggingface
ls -lah ~/.cache/huggingface

# Note: models must include safetensors weights; PyTorch .bin-only models are not supported by the local loader here
```

#### Issue: Local embeddings are slow

**Solution:**

```bash
# Reduce batch size via config or environment (CPU defaults prioritize stability)
# Consider using a smaller model (e.g., all-MiniLM-L6-v2) or enabling GPU backends.

# For Apple Silicon (Metal) or CUDA, additional wiring can be enabled in config.
# Current default uses CPU; contact maintainers to enable device selectors in your environment.
```

#### Issue: FAISS linking error during cargo install

**Error:** `ld: library 'faiss_c' not found`

**Solution:**

```bash
# On macOS: Install FAISS via Homebrew
brew install faiss

# Set library paths and retry installation
export LIBRARY_PATH="/opt/homebrew/opt/faiss/lib:$LIBRARY_PATH"
export LD_LIBRARY_PATH="/opt/homebrew/opt/faiss/lib:$LD_LIBRARY_PATH"

# Retry the cargo install command
cargo install --path crates/codegraph-mcp --features "embeddings,codegraph-vector/onnx,faiss"
```

**Alternative Solution:**

```bash
# On Ubuntu/Debian
sudo apt-get update
sudo apt-get install libfaiss-dev

# On Fedora/RHEL
sudo dnf install faiss-devel

# Then retry cargo install
cargo install --path crates/codegraph-mcp --features "embeddings,codegraph-vector/onnx,faiss"
```

```

### Debug Mode

Enable debug logging for troubleshooting:

```bash
# Set debug log level
export RUST_LOG=debug
codegraph --verbose index .

# Check logs
tail -f ~/.codegraph/logs/codegraph.log
```

### Health Checks

```bash
# Check system health
codegraph status --detailed

# Validate configuration
codegraph config validate

# Test database connection
codegraph test db

# Verify embeddings
codegraph test embeddings
```

## 🤝 Contributing

We welcome contributions! Please see our [Contributing Guide](CONTRIBUTING.md) for details.

### Development Setup

```bash
# Clone repository
git clone https://github.com/jakedismo/codegraph-cli-mcp.git
cd codegraph-cli-mcp

# Install development dependencies
cargo install cargo-watch cargo-nextest

# Run tests
cargo nextest run

# Run with watch mode
cargo watch -x check -x test
```

## 📄 License

This project is dual-licensed under MIT and Apache 2.0 licenses. See [LICENSE-MIT](LICENSE-MIT) and [LICENSE-APACHE](LICENSE-APACHE) for details.

## 🙏 Acknowledgments

- Built with [Rust](https://www.rust-lang.org/)
- Powered by [Tree-sitter](https://tree-sitter.github.io/)
- Vector search by [FAISS](https://github.com/facebookresearch/faiss)
- Graph storage with [RocksDB](https://rocksdb.org/)
- MCP Protocol by [Anthropic](https://modelcontextprotocol.io)

---

<p align="center">
  Made with ❤️ by the CodeGraph Team
</p>
