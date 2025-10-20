# Simplified Configuration Guide

## The Problem (Before)

You had to export **8+ environment variables** every time:

```bash
export CODEGRAPH_LOCAL_MODEL=/Users/you/.cache/huggingface/hub/models--Qdrant--all-MiniLM-L6-v2-onnx/snapshots/abc123
export CODEGRAPH_EMBEDDING_PROVIDER=onnx
export CODEGRAPH_OLLAMA_URL=http://localhost:11434
export CODEGRAPH_MODEL="hf.co/unsloth/Qwen2.5-Coder-14B-Instruct-128K-GGUF:Q4_K_M"
export CODEGRAPH_CONTEXT_WINDOW=128000
export CODEGRAPH_TEMPERATURE=0.1
export RUST_LOG=info
export PATH="$HOME/.local/bin:$PATH"
```

**This is terrible!** 😫

---

## The Solution (After)

### Option 1: Just Run It (Auto-Detection) ⚡

```bash
# That's it! No configuration needed
cargo run -- index .
```

**Auto-detection will**:
- ✅ Find ONNX models in your HuggingFace cache
- ✅ Detect if Ollama is running
- ✅ Use sensible defaults
- ✅ Enable context-only mode (fastest)

---

### Option 2: One-Line `.env` File 📋

Create `.env` in your project:

```bash
# For Ollama users (one line!)
CODEGRAPH_EMBEDDING_PROVIDER=ollama
```

Or for ONNX:

```bash
# Auto-detect ONNX model from HF cache
CODEGRAPH_EMBEDDING_PROVIDER=onnx
```

**That's it!**

CodeGraph will:
- Load `.env` automatically
- Auto-detect model paths
- Use smart defaults
- Just work™

---

### Option 3: Config File (Once and Done) ⚙️

Create `.codegraph.toml` (one time):

```toml
[embedding]
provider = "auto"  # or "onnx", "ollama", "openai"

[llm]
enabled = false  # true if you want local LLM insights
```

Then just run:

```bash
cargo run -- index .
```

No environment variables ever again!

---

## Configuration Hierarchy

CodeGraph uses this precedence (highest to lowest):

1. **Environment variables** (`.env` file or exports)
2. **Config file** (`.codegraph.toml` or `~/.codegraph/config.toml`)
3. **Auto-detection**
4. **Sensible defaults**

---

## Quick Start Guides

### For Agent Workflows (Claude, GPT-4) - Recommended

**Goal**: Maximum speed, let agent analyze context

**Setup** (choose one):

#### Auto (easiest):
```bash
# No setup needed!
cargo run -- index .
```

#### `.env` file:
```bash
echo "CODEGRAPH_EMBEDDING_PROVIDER=auto" > .env
cargo run -- index .
```

#### `.codegraph.toml`:
```toml
[embedding]
provider = "auto"

[llm]
enabled = false  # Context-only mode
insights_mode = "context-only"
```

**Result**: <200ms insights generation, context sent to agent

---

### For Local LLM (Qwen2.5-Coder) - Balanced

**Goal**: Use local LLM for insights

**Setup** `.env`:

```bash
# Embedding model (auto-detect or specify)
CODEGRAPH_EMBEDDING_PROVIDER=auto

# LLM model (enables LLM insights)
CODEGRAPH_MODEL=qwen2.5-coder:14b
```

**Or** `.codegraph.toml`:

```toml
[embedding]
provider = "auto"

[llm]
enabled = true
model = "qwen2.5-coder:14b"
context_window = 128000
temperature = 0.1
insights_mode = "balanced"
```

**Result**: 2-5s insights with local LLM (vs 50s before)

---

## Auto-Detection Details

### What Gets Auto-Detected?

1. **ONNX Models**:
   - Searches `~/.cache/huggingface/hub/`
   - Finds `all-MiniLM-L6-v2-onnx` automatically
   - Uses latest snapshot

2. **Ollama**:
   - Checks if Ollama is running
   - Uses `all-minilm:latest` if available

3. **Paths**:
   - Home directory
   - HuggingFace cache
   - Common model locations

---

## CLI Commands

### Show Current Configuration

```bash
cargo run -- config show
```

**Output**:
```
🔧 CodeGraph Configuration
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

📋 Source: .env + auto-detection

🤖 Embedding:
   Provider: auto (detected: onnx)
   Model: /Users/you/.cache/huggingface/.../snapshot/xyz
   Batch size: 64

💬 LLM:
   Enabled: false (context-only mode)
   Insights mode: context-only

⚡ Performance:
   Threads: 12 (auto-detected)
   Cache: 512MB
   GPU: disabled

📊 Logging:
   Level: info
   Format: pretty
```

---

### Create Example Config

```bash
cargo run -- config init
```

Creates `.codegraph.toml` with defaults.

---

### Validate Configuration

```bash
cargo run -- config validate
```

Checks for:
- ✅ Valid providers
- ✅ Model accessibility
- ✅ Ollama connectivity
- ✅ Required dependencies

---

## Migration from Old Config

### Before (8 exports):

```bash
export CODEGRAPH_LOCAL_MODEL=/Users/you/.cache/huggingface/hub/models--Qdrant--all-MiniLM-L6-v2-onnx/snapshots/abc123
export CODEGRAPH_EMBEDDING_PROVIDER=onnx
export CODEGRAPH_OLLAMA_URL=http://localhost:11434
export CODEGRAPH_MODEL="hf.co/unsloth/Qwen2.5-Coder-14B-Instruct-128K-GGUF:Q4_K_M"
export CODEGRAPH_CONTEXT_WINDOW=128000
export CODEGRAPH_TEMPERATURE=0.1
export RUST_LOG=info
export PATH="$HOME/.local/bin:$PATH"

cargo run -- index .
```

### After (0-1 exports):

#### Option A: Auto-detection
```bash
# Nothing!
cargo run -- index .
```

#### Option B: `.env` file
```bash
# Create once:
cat > .env <<EOF
CODEGRAPH_EMBEDDING_PROVIDER=auto
CODEGRAPH_MODEL=qwen2.5-coder:14b
EOF

# Then just run:
cargo run -- index .
```

#### Option C: Config file
```bash
# Create once:
cargo run -- config init

# Edit .codegraph.toml as needed

# Then just run:
cargo run -- index .
```

---

## Example Configurations

### Minimal (Auto-Everything)

**`.env`**:
```bash
# Empty or this single line:
CODEGRAPH_EMBEDDING_PROVIDER=auto
```

---

### Ollama User

**`.env`**:
```bash
CODEGRAPH_EMBEDDING_PROVIDER=ollama
CODEGRAPH_EMBEDDING_MODEL=all-minilm:latest
```

---

### ONNX User (Explicit Path)

**`.env`**:
```bash
CODEGRAPH_EMBEDDING_PROVIDER=onnx
CODEGRAPH_LOCAL_MODEL=/path/to/your/onnx/model
```

---

### OpenAI User

**`.env`**:
```bash
CODEGRAPH_EMBEDDING_PROVIDER=openai
CODEGRAPH_EMBEDDING_MODEL=text-embedding-3-small
OPENAI_API_KEY=sk-your-key-here
```

---

### Power User (Full Control)

**`.codegraph.toml`**:
```toml
[embedding]
provider = "onnx"
model = "/custom/path/to/model"
batch_size = 128

[llm]
enabled = true
model = "qwen2.5-coder:14b"
ollama_url = "http://192.168.1.100:11434"
context_window = 128000
temperature = 0.1
insights_mode = "balanced"

[performance]
num_threads = 16
cache_size_mb = 1024
enable_gpu = true
max_concurrent_requests = 8

[logging]
level = "debug"
format = "json"
```

---

## File Locations

### `.env` File

Searched in order:
1. `./.env` (project directory)
2. `~/.codegraph.env` (user home)

### `.codegraph.toml` Config

Searched in order:
1. `./.codegraph.toml` (project directory)
2. `~/.codegraph/config.toml` (user config)

**Recommendation**: Use project-specific `.env` for simple cases, `~/.codegraph/config.toml` for global settings.

---

## Environment Variables Reference

### Embedding

| Variable | Description | Example |
|----------|-------------|---------|
| `CODEGRAPH_EMBEDDING_PROVIDER` | Provider type | `auto`, `onnx`, `ollama`, `openai` |
| `CODEGRAPH_EMBEDDING_MODEL` | Model name/ID | `all-minilm:latest` |
| `CODEGRAPH_LOCAL_MODEL` | ONNX model path | `/path/to/model` |
| `CODEGRAPH_OLLAMA_URL` | Ollama server | `http://localhost:11434` |
| `OPENAI_API_KEY` | OpenAI API key | `sk-...` |

### LLM

| Variable | Description | Example |
|----------|-------------|---------|
| `CODEGRAPH_MODEL` | LLM model | `qwen2.5-coder:14b` |
| `CODEGRAPH_CONTEXT_WINDOW` | Context size | `128000` |
| `CODEGRAPH_TEMPERATURE` | Generation temp | `0.1` |

### Other

| Variable | Description | Example |
|----------|-------------|---------|
| `RUST_LOG` | Log level | `info`, `debug`, `trace` |

---

## Troubleshooting

### "No embedding model found"

**Solution**:
```bash
# Option 1: Let auto-detect find it
CODEGRAPH_EMBEDDING_PROVIDER=auto

# Option 2: Install Ollama and pull model
ollama pull all-minilm

# Option 3: Specify ONNX path
CODEGRAPH_LOCAL_MODEL=/path/to/model
```

---

### "Config file not found"

**This is fine!** CodeGraph works without a config file.

To create one:
```bash
cargo run -- config init
```

---

### "Ollama connection failed"

**Check Ollama is running**:
```bash
curl http://localhost:11434/api/tags
```

If not:
```bash
ollama serve
```

---

### "Model path not accessible"

**Check the path**:
```bash
ls -la "$CODEGRAPH_LOCAL_MODEL"
```

Or let auto-detect find it:
```bash
CODEGRAPH_EMBEDDING_PROVIDER=auto
```

---

## Best Practices

### 1. Start Simple
```bash
# First time: Just run it
cargo run -- index .
```

### 2. Use `.env` for Projects
```bash
# Per-project settings
echo "CODEGRAPH_EMBEDDING_PROVIDER=auto" > .env
```

### 3. Use `~/.codegraph/config.toml` for Global
```bash
# One-time global setup
mkdir -p ~/.codegraph
cp .codegraph.toml.example ~/.codegraph/config.toml
# Edit as needed
```

### 4. Keep It Minimal
**Only specify what differs from defaults:**

```toml
# Good
[embedding]
provider = "ollama"

# Unnecessary (these are defaults)
[embedding]
provider = "auto"
batch_size = 32
ollama_url = "http://localhost:11434"
```

---

## Summary

### Before 😫
- 8+ environment variables
- Long, complex paths
- Easy to make mistakes
- Needed every time

### After 🎉
- 0-1 environment variables
- Auto-detection
- `.env` or config file (once)
- Just works™

**Most users can now just run**:
```bash
cargo run -- index .
```

**No configuration needed!** 🚀
