# CodeGraph Installation Guide

## Overview

CodeGraph supports two installation profiles optimized for different use cases:

1. **Local AI Profile** (`install-codegraph-osx.sh`) - Original script with local models
2. **Cloud Profile** (`install-codegraph-cloud.sh`) - New script with cloud services ⭐ **Recommended for production**

## Quick Comparison

| Feature | Local AI (`install-codegraph-osx.sh`) | Cloud (`install-codegraph-cloud.sh`) |
|---------|--------------------------------------|-------------------------------------|
| **Embeddings** | ONNX (local), Ollama (local) | Jina (cloud, includes reranking) |
| **LLM** | Qwen (local, resource-intensive) | Anthropic, OpenAI, Compatible APIs |
| **Database** | RocksDB (embedded) | SurrealDB (scalable, distributed) |
| **Setup** | Simpler, no external services | Requires SurrealDB + API keys |
| **Cost** | Free, uses local compute | Paid API usage |
| **Performance** | Good for small projects | Excellent for large codebases |
| **Quality** | Good embeddings | Superior with Jina reranking |

## Cloud Profile Installation (Recommended)

### Prerequisites

1. **macOS with Homebrew**
   ```bash
   # Install Homebrew if not already installed
   /bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"
   ```

2. **Rust toolchain**
   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   ```

### Installation Steps

1. **Run the installation script:**
   ```bash
   ./install-codegraph-cloud.sh
   ```

   This will:
   - Install FAISS (via Homebrew)
   - Install SurrealDB (via Homebrew)
   - Build CodeGraph with cloud features
   - Install to `~/.local/bin/codegraph`

2. **Start SurrealDB:**

   Choose one of these options based on your needs:

   ```bash
   # Development (memory mode - data lost on restart)
   surreal start --log trace memory

   # Development with persistence
   surreal start --log trace file://$HOME/.codegraph/surrealdb

   # Production (recommended)
   surreal start --log trace rocksdb://$HOME/.codegraph/surrealdb
   ```

   Keep this running in a separate terminal or use a process manager like `launchd`.

3. **Set up API keys:**

   Add to your `~/.zshrc` or `~/.bashrc`:

   ```bash
   # Jina (required for embeddings)
   export JINA_API_KEY='your-jina-api-key'

   # Anthropic (recommended for LLM)
   export ANTHROPIC_API_KEY='your-anthropic-api-key'

   # Or OpenAI
   export OPENAI_API_KEY='your-openai-api-key'

   # CodeGraph configuration
   export CODEGRAPH_EMBEDDING_PROVIDER=jina
   export CODEGRAPH_LLM_PROVIDER=anthropic
   export CODEGRAPH_SURREALDB_URL=ws://localhost:8000

   # Add to PATH
   export PATH="$HOME/.local/bin:$PATH"
   ```

   Then reload: `source ~/.zshrc`

4. **Verify installation:**
   ```bash
   codegraph --version
   ```

### Getting API Keys

- **Jina AI**: https://jina.ai/ - Sign up for API access
- **Anthropic**: https://console.anthropic.com/ - Get Claude API key
- **OpenAI**: https://platform.openai.com/ - Get OpenAI API key

## Local AI Profile Installation

For development without cloud dependencies:

```bash
./install-codegraph-osx.sh
```

This installs with:
- ONNX embeddings (local, fast)
- Ollama embeddings (local, higher quality)
- Qwen integration (local LLM)
- RocksDB (embedded database)

No API keys needed, but requires more local compute resources.

## Feature Flags Breakdown

### Cloud Profile Features
```bash
ai-enhanced                          # Core AI + FAISS + embeddings
codegraph-vector/jina                # Jina cloud embeddings + reranking
codegraph-graph/surrealdb            # SurrealDB backend
codegraph-ai/all-cloud-providers     # Anthropic + OpenAI + compatible
```

### Local Profile Features
```bash
ai-enhanced                    # Core AI + FAISS + embeddings
qwen-integration              # Local Qwen LLM
embeddings                    # Base embedding support
faiss                        # Vector search
embeddings-ollama            # Ollama embeddings
codegraph-vector/onnx        # ONNX embeddings
```

## Usage After Installation

### Initialize a Project

```bash
cd /path/to/your/project
codegraph init .
```

### Index Your Codebase

```bash
codegraph index .
```

This auto-detects and indexes all supported languages:
- Rust, Python, JavaScript, TypeScript
- Swift, C#, Ruby, PHP
- Go, Java, C++

### Using with Claude Code

CodeGraph tools are automatically available in Claude Code via MCP. No additional configuration needed!

Available tools:
- `enhanced_search` - Semantic code search
- `semantic_intelligence` - AI-powered code analysis
- `impact_analysis` - Change impact assessment
- `pattern_detection` - Find code patterns
- `vector_search` - Similarity search
- `graph_neighbors` - Explore dependencies
- `graph_traverse` - Navigate code graph
- `performance_metrics` - System monitoring

## Troubleshooting

### SurrealDB Connection Issues

```bash
# Check if SurrealDB is running
ps aux | grep surreal

# Verify connection
curl http://localhost:8000/health
```

### FAISS Linking Errors

```bash
brew reinstall faiss
brew link faiss
```

### API Key Issues

Verify your keys are set:
```bash
echo $JINA_API_KEY
echo $ANTHROPIC_API_KEY
```

### Build Failures

```bash
# Clean and rebuild
cargo clean
./install-codegraph-cloud.sh
```

## Advanced Configuration

### Custom SurrealDB URL

```bash
export CODEGRAPH_SURREALDB_URL=ws://custom-host:8000
```

### Custom Install Directory

```bash
export CODEGRAPH_INSTALL_DIR=/custom/path
./install-codegraph-cloud.sh
```

### Mixing Local and Cloud Features

You can customize feature flags by editing the script:

```bash
# Example: Use Jina but keep RocksDB
FEATURE_FLAGS="ai-enhanced,codegraph-vector/jina,codegraph-ai/anthropic"
```

## Recommendation

**For production and serious development**: Use the **Cloud Profile**
- Superior embedding quality with Jina
- Scalable with SurrealDB
- Better LLM responses with Claude/GPT
- Professional reranking capabilities

**For quick testing or offline work**: Use the **Local AI Profile**
- No API costs
- Works offline
- Good for smaller projects
- Faster initial setup

## Next Steps

1. Install using your chosen profile
2. Set up API keys (if using cloud profile)
3. Start SurrealDB (if using cloud profile)
4. Index your first project
5. Try the tools in Claude Code!

For detailed tool documentation, see `CODEGRAPH-MCP-TOOLS-GUIDE.md`.
