# CodeGraph

> Turn your codebase into a searchable knowledge graph powered by embeddings and LLMs

CodeGraph indexes your source code, creates semantic embeddings, and exposes a **Model Context Protocol (MCP)** server that AI tools (Claude Desktop, LM Studio, etc.) can query for project-aware context.

**✨ What you get:**
- 🔍 Semantic code search across your entire codebase
- 🧠 LLM-powered code intelligence and analysis
- 📊 Automatic dependency graphs and code relationships
- ⚡ Fast vector search with FAISS (2-5ms query latency)
- 🔌 MCP server for AI tool integration

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
- **Embeddings:** OpenAI
- **LLM:** Anthropic Claude or OpenAI GPT (with o1/o3 reasoning models)

**Pros:** ✅ Best quality, ✅ Fast, ✅ 200K context (Claude)
**Cons:** ❌ API costs, ❌ Requires internet, ❌ Data sent to cloud

[→ Jump to Cloud Setup Instructions](#cloud-setup-anthropic--openai)

---

### Option 4: Hybrid (Mix & Match) 🔀

**Best for:** Balancing cost and quality

**Example combinations:**
- Local embeddings (ONNX) + Cloud LLM (Claude)
- LM Studio embeddings + Cloud LLM (OpenAI o3)
- OpenAI embeddings + Local LLM (Ollama)

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
```

**Step 2: Pull models**
```bash
# Pull embedding model
ollama pull nomic-embed-code

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
model = "sentence-transformers/all-MiniLM-L6-v2"
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
- **Embedding model:** `jinaai/jina-code-embeddings-1.5b`
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
model = "jinaai/jina-code-embeddings-1.5b"
lmstudio_url = "http://localhost:1234"
dimension = 1536

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

### Cloud Setup (Anthropic & OpenAI)

**Step 1: Get API keys**
- Anthropic: [console.anthropic.com](https://console.anthropic.com/)
- OpenAI: [platform.openai.com](https://platform.openai.com/)

**Step 2: Build CodeGraph with cloud features**
```bash
cd codegraph-rust

# Build with all cloud providers
cargo build --release --features "anthropic,openai-llm,openai,faiss"
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
provider = "openai"
model = "text-embedding-3-small"
openai_api_key = "sk-..."  # or set OPENAI_API_KEY env var
dimension = 1536

[llm]
enabled = true
provider = "anthropic"
model = "claude-3-5-sonnet-20241022"
anthropic_api_key = "sk-ant-..."  # or set ANTHROPIC_API_KEY env var
context_window = 200000
```

**For OpenAI (with reasoning models):**
```toml
[embedding]
provider = "openai"
model = "text-embedding-3-small"
openai_api_key = "sk-..."
dimension = 1536

[llm]
enabled = true
provider = "openai"
model = "o3-mini"  # or "gpt-4o", "o1", "o4-mini"
openai_api_key = "sk-..."
max_output_tokens = 25000
reasoning_effort = "medium"  # for reasoning models: "low", "medium", "high"
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
model = "claude-3-5-sonnet-20241022"
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
model = "jinaai/jina-code-embeddings-1.5b"
dimension = 1536
batch_size = 64

[llm]
enabled = true
provider = "anthropic"  # or "openai", "ollama", "lmstudio"
model = "claude-3-5-sonnet-20241022"
anthropic_api_key = "sk-ant-..."
context_window = 200000
temperature = 0.1
max_output_tokens = 4096

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
codegraph index /path/to/project

# Start MCP server (for Claude Desktop, LM Studio, etc.)
codegraph start stdio

# Start HTTP server (alternative)
codegraph start http

# List available MCP tools
codegraph tools list
```

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

## 📊 Feature Flags Reference

When building, include features for the providers you want to use:

| Feature | Providers Enabled | Use Case |
|---------|------------------|----------|
| `onnx` | ONNX embeddings | Local CPU/GPU embeddings |
| `ollama` | Ollama embeddings + LLM | Local models via Ollama |
| `openai` | OpenAI embeddings | Cloud embeddings |
| `openai-llm` | OpenAI GPT/o-series LLMs | Cloud LLM (GPT-4o, o3, etc.) |
| `anthropic` | Anthropic Claude | Cloud LLM (Claude 3.5) |
| `openai-compatible` | LM Studio, custom APIs | OpenAI-compatible endpoints |
| `faiss` | FAISS vector search | **Required for search** |
| `all-cloud-providers` | All cloud features | Shortcut for all cloud providers |

### Common Build Commands

```bash
# Local only (ONNX + Ollama)
cargo build --release --features "onnx,ollama,faiss"

# LM Studio
cargo build --release --features "openai-compatible,faiss"

# Cloud only (Anthropic + OpenAI)
cargo build --release --features "anthropic,openai-llm,openai,faiss"

# Everything
cargo build --release --features "all-cloud-providers,onnx,ollama,faiss"
```

---

## ⚡ Performance

### Speed Metrics (Apple Silicon + LM Studio)

| Operation | Performance | Notes |
|-----------|------------|-------|
| **Embedding generation** | 120 embeddings/sec | LM Studio with MLX |
| **Vector search** | 2-5ms latency | FAISS with index caching |
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

## 🚀 Advanced Features

### Graph Analytics
- The indexer produces ownership graphs and dependency edges
- Automatic metadata extraction for RAG consumption

### MCP Tools
Exposed through the MCP server:
- `vector.search` - Semantic code search
- `graph.traverse` - Navigate code relationships
- `code.read` - Read source files
- `insights.generate` - LLM-powered code analysis

### FAISS Tuning
For large codebases:
- Set `CODEGRAPH_FAISS_TRAINING_THRESHOLD` environment variable
- Or provide pre-built index in `.codegraph/faiss.index`

**See `docs/` for:**
- Architecture details
- RAG pipeline documentation
- Deployment guides
- Operator playbooks

---

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

- **[Cloud Providers Guide](docs/CLOUD_PROVIDERS.md)** - Detailed cloud provider setup
- **[LM Studio Setup](LMSTUDIO_SETUP.md)** - LM Studio-specific configuration
- **[Configuration Reference](.codegraph.toml.example)** - All configuration options
- **[Legacy Docs](docs/legacy/)** - Historical experiments and architecture notes
