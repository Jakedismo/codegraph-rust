# LM Studio Setup Guide for CodeGraph

## Overview

CodeGraph now defaults to using **LM Studio** for superior MLX support and Flash Attention 2 performance on macOS. This guide covers setup for the recommended configuration:

- **Embeddings**: Jina Code Embeddings 1.5B (1536 dimensions)
- **LLM**: DeepSeek Coder v2 Lite Instruct Q4_K_M
- **Benefits**: MLX acceleration, Flash Attention 2, optimized for code understanding

## Why LM Studio?

**LM Studio Advantages:**
- ✅ Native MLX support (10x faster on Apple Silicon)
- ✅ Flash Attention 2 (2-3x memory efficiency + speedup)
- ✅ Superior quantization (Q4_K_M > GGUF Q4)
- ✅ Better model loading/management
- ✅ OpenAI-compatible API (easy integration)

**vs Ollama:**
- Ollama: Good for general use, simpler setup
- LM Studio: Better for production, higher performance on macOS

## Installation

### 1. Install LM Studio

Download from [lmstudio.ai](https://lmstudio.ai/)

```bash
# macOS (Apple Silicon recommended)
# Download the .dmg and install
open ~/Downloads/LMStudio-*.dmg
```

### 2. Download Models

**Option A: Via LM Studio UI (Recommended)**

1. Open LM Studio
2. Go to "Discover" tab
3. Search and download:
   - **Embeddings**: `jinaai/jina-code-embeddings-1.5b` (1.5B parameters, 1536-dim)
   - **LLM**: `lmstudio-community/DeepSeek-Coder-V2-Lite-Instruct-GGUF`
     - Select: `DeepSeek-Coder-V2-Lite-Instruct-Q4_K_M.gguf`

**Option B: Manual Download**

```bash
# Download embeddings model
huggingface-cli download jinaai/jina-code-embeddings-1.5b' \
  --local-dir ~/.cache/lm-studio/models/jinaai/jina-code-embeddings-1.5b'

# Download LLM model
huggingface-cli download lmstudio-community/DeepSeek-Coder-V2-Lite-Instruct-GGUF \
  DeepSeek-Coder-V2-Lite-Instruct-Q4_K_M.gguf \
  --local-dir ~/.cache/lm-studio/models/lmstudio-community/DeepSeek-Coder-V2-Lite-Instruct-GGUF
```

### 3. Start LM Studio Server

**Load Embedding Model:**
1. In LM Studio, go to "Local Server" tab
2. Load model: `jinaai/jina-embeddings-v3`
3. Start server on port `1234` (default)
4. Verify: `http://localhost:1234/v1/embeddings`

**Server will handle both embeddings and LLM requests on port 1234**

## Configuration

### Quick Setup (Zero Config)

CodeGraph defaults to LM Studio automatically:

```bash
# Just start indexing - it will use LM Studio defaults
codegraph index /path/to/project
```

### Custom Configuration

#### Option 1: Environment Variables

Create `.env` file:

```bash
# Embedding Configuration
CODEGRAPH_EMBEDDING_PROVIDER=lmstudio
CODEGRAPH_EMBEDDING_MODEL=jinaai/jina-embeddings-v3
CODEGRAPH_LMSTUDIO_URL=http://localhost:1234
CODEGRAPH_EMBEDDING_DIMENSION=1536

# LLM Configuration (optional, for insights)
CODEGRAPH_LLM_PROVIDER=lmstudio
CODEGRAPH_MODEL=lmstudio-community/DeepSeek-Coder-V2-Lite-Instruct-GGUF
CODEGRAPH_CONTEXT_WINDOW=32000
CODEGRAPH_TEMPERATURE=0.1

# Clean TUI output during indexing
RUST_LOG=warn
```

#### Option 2: Config File

Create `.codegraph.toml`:

```toml
[embedding]
provider = "lmstudio"
model = "jinaai/jina-code-embeddings-1.5b"
lmstudio_url = "http://localhost:1234"
dimension = 1536
batch_size = 64

[llm]
enabled = false  # Set true for local insights
provider = "lmstudio"
model = "lmstudio-community/DeepSeek-Coder-V2-Lite-Instruct-GGUF"
lmstudio_url = "http://localhost:1234"
context_window = 32000
temperature = 0.1
insights_mode = "context-only"

[logging]
level = "warn"  # Clean TUI output
```

## Usage

### Index a Codebase

```bash
# Basic indexing (uses LM Studio defaults)
codegraph index /path/to/project

# With clean output (recommended)
RUST_LOG=warn codegraph index /path/to/project

# With custom batch size for GPU optimization
codegraph index /path/to/project --batch-size 128
```

**Expected Output:**
```
⠁ Indexing project: /path/to/project
┌─────────────────────────────────────────────────────┐
│ 📦 Collecting files                                │
│ [████████████████████] 1250/1250 (100%)            │
└─────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────┐
│ 🌳 Parsing AST                                     │
│ [████████████████████] 1250/1250 (100%)            │
└─────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────┐
│ 💾 Generating embeddings                           │
│ [████████████████████] 5432/5432 (100%)            │
│ ⚡ 127.3 embeddings/sec                            │
└─────────────────────────────────────────────────────┘

🎉 INDEXING COMPLETE!

📊 Performance Summary
┌─────────────────────────────────────────────────┐
│ ⏱️  Total time: 42.7s                           │
│ ⚡ Throughput: 29.28 files/sec                  │
├─────────────────────────────────────────────────┤
│ 📄 Files:   1250 indexed,   12 skipped         │
│ 📝 Lines:  87432 processed                     │
│ 💾 Embeddings:   5432 generated                │
└─────────────────────────────────────────────────┘

⚙️  Configuration Summary
Workers: 8 | Batch Size: 64 | Languages: rust, typescript
✅ Excellent embedding success rate (>90%)
```

### Start MCP Server

```bash
# Start MCP server for Claude Desktop
cd /path/to/project
codegraph start stdio
```

### Search Codebase

```bash
# Semantic search with Jina embeddings
codegraph search "authentication middleware" --limit 10

# Search specific languages
codegraph search "database connection pool" --langs rust,go

# Search specific paths
codegraph search "API routes" --paths src/api,lib/routes
```

## Performance Optimization

### Embedding Performance

**Jina Code Embeddings 1.5B Performance:**
- **M1/M2/M3 Max**: ~100-150 embeddings/sec (MLX)
- **M1/M2/M3 Pro**: ~60-90 embeddings/sec (MLX)
- **M1/M2/M3 Base**: ~30-50 embeddings/sec (MLX)
- **Intel Mac**: ~15-25 embeddings/sec (CPU)

**Optimization Tips:**
```bash
# Increase batch size for larger GPU/Unified Memory
codegraph index . --batch-size 128  # For 32GB+ unified memory
codegraph index . --batch-size 64   # For 16-24GB unified memory (default)
codegraph index . --batch-size 32   # For 8-16GB unified memory
```

### LLM Performance

**DeepSeek Coder v2 Lite Instruct Q4_K_M:**
- **Context Window**: 32,768 tokens
- **Parameters**: 2.4B (quantized to ~1.5GB)
- **Speed**: ~40-60 tokens/sec (M2 Max)
- **Quality**: Comparable to 7B models due to architecture

**Memory Requirements:**
- **Minimum**: 8GB unified memory (model + context)
- **Recommended**: 16GB+ (for comfortable multi-tasking)
- **Optimal**: 24GB+ (for large context windows)

## Troubleshooting

### Issue: "Connection refused" error

**Solution:**
```bash
# Verify LM Studio server is running
curl http://localhost:1234/v1/models

# If not running, start LM Studio and:
# 1. Go to "Local Server" tab
# 2. Load embedding model
# 3. Click "Start Server"
```

### Issue: Slow embedding generation

**Solution:**
```bash
# 1. Verify MLX is being used (LM Studio should show "MLX" in UI)
# 2. Reduce batch size for lower memory systems
RUST_LOG=info codegraph index . --batch-size 32

# 3. Close other applications to free up unified memory
```

### Issue: "Model not found" error

**Solution:**
```bash
# Download models via LM Studio UI or CLI
# Then verify in LM Studio → Models tab

# Or specify exact model path:
export CODEGRAPH_EMBEDDING_MODEL="jinaai/jina-embeddings-v3"
codegraph index .
```

### Issue: Too many logs cluttering output

**Solution:**
```bash
# Set RUST_LOG=warn for clean TUI output
export RUST_LOG=warn
codegraph index .

# Or add to .env file:
echo "RUST_LOG=warn" >> .env
```

## Comparison: LM Studio vs Ollama

| Feature | LM Studio | Ollama |
|---------|-----------|---------|
| **MLX Support** | Native (10x faster) | Basic |
| **Flash Attention 2** | Yes (2-3x faster) | No |
| **Quantization** | Q4_K_M (better) | Q4 (standard) |
| **Setup** | GUI + API | CLI only |
| **Model Management** | Excellent UI | Good CLI |
| **API** | OpenAI-compatible | Custom |
| **Performance (M2 Max)** | ~120 emb/sec | ~60 emb/sec |
| **Memory Efficiency** | Excellent | Good |
| **Best For** | Production, macOS | Development, Linux |

## Alternative: Ollama Setup

If you prefer Ollama:

```bash
# Install Ollama
brew install ollama

# Pull models
ollama pull nomic-embed-code  # Embeddings
ollama pull qwen2.5-coder:14b  # LLM

# Configure CodeGraph
export CODEGRAPH_EMBEDDING_PROVIDER=ollama
export CODEGRAPH_EMBEDDING_MODEL=nomic-embed-code
export CODEGRAPH_OLLAMA_URL=http://localhost:11434
export CODEGRAPH_EMBEDDING_DIMENSION=384  # nomic-embed-code uses 384-dim

# Index
codegraph index /path/to/project
```

## Best Practices

### 1. Clean TUI Output
```bash
# Always use RUST_LOG=warn during indexing
export RUST_LOG=warn
codegraph index .
```

### 2. Batch Size Tuning
```bash
# Start with default (64), adjust based on memory
codegraph index . --batch-size 64

# Monitor embedding speed in output:
# ⚡ 127.3 embeddings/sec (good)
# ⚡ 45.2 embeddings/sec (increase batch size if memory allows)
# ⚡ OOM error (decrease batch size)
```

### 3. Multi-Project Workflow
```bash
# Each project gets isolated .codegraph/ storage
cd ~/projects/api && codegraph index .
cd ~/projects/frontend && codegraph index .
cd ~/projects/mobile && codegraph index .

# Serve different projects in different terminals
cd ~/projects/api && codegraph start stdio      # Terminal 1
cd ~/projects/frontend && codegraph start stdio  # Terminal 2
```

### 4. Claude Desktop Integration
```json
{
  "mcpServers": {
    "codegraph-api": {
      "command": "codegraph",
      "args": ["start", "stdio"],
      "cwd": "/Users/you/projects/api",
      "env": {
        "RUST_LOG": "warn",
        "CODEGRAPH_EMBEDDING_PROVIDER": "lmstudio",
        "CODEGRAPH_LMSTUDIO_URL": "http://localhost:1234"
      }
    }
  }
}
```

## Model Recommendations

### Embeddings

| Model | Dimensions | Size | Quality | Speed |
|-------|-----------|------|---------|-------|
| **jina-code-embeddings-1.5b** | 1536 | 1.5GB | Excellent | Fast |
| jinaai/jina-embeddings-v2-base-code | 768 | 500MB | Very Good | Very Fast |
| nomic-embed-code (Ollama) | 384 | 300MB | Good | Very Fast |
| all-MiniLM-L6-v2 (ONNX) | 384 | 90MB | Good | Ultra Fast |

**Recommendation**: jina-code-embeddings-1.5b (best quality/speed tradeoff)

### LLMs

| Model | Context | Size | Quality | Speed |
|-------|---------|------|---------|-------|
| **DeepSeek Coder v2 Lite Q4_K_M** | 32K | 1.5GB | Excellent | Fast |
| Qwen2.5-Coder 7B Q4 | 128K | 4GB | Excellent | Medium |
| CodeLlama 13B Q4 | 16K | 7GB | Very Good | Slow |
| DeepSeek Coder 6.7B Q4 | 16K | 4GB | Very Good | Medium |

**Recommendation**: DeepSeek Coder v2 Lite (best for most use cases)

## Support

For issues or questions:
- GitHub Issues: [codegraph-rust/issues](https://github.com/Jakedismo/codegraph-rust/issues)
- Documentation: [docs/](../docs/)
- LM Studio Docs: [lmstudio.ai/docs](https://lmstudio.ai/docs)
