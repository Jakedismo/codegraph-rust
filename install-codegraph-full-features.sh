#!/bin/bash
# ABOUTME: Installs CodeGraph CLI with ALL available features enabled.
# ABOUTME: Maximum capability build including all embedding providers, LLMs, server modes, and experimental features.

set -euo pipefail

# Comprehensive feature set - everything enabled
FEATURES="daemon,ai-enhanced,embeddings-local,embeddings-openai,embeddings-ollama,embeddings-jina,embeddings-lmstudio,codegraph-vector/jina,codegraph-graph/surrealdb,codegraph-ai/all-cloud-providers,server-http,autoagents-experimental,qwen-integration"
SURR_URL="${CODEGRAPH_SURREALDB_URL:-ws://localhost:3004}"
SURR_NAMESPACE="${CODEGRAPH_SURREALDB_NAMESPACE:-ouroboros}"
SURR_DATABASE="${CODEGRAPH_SURREALDB_DATABASE:-codegraph}"
HTTP_PORT="${CODEGRAPH_HTTP_PORT:-3000}"
INSTALL_PATH="${CARGO_HOME:-$HOME/.cargo}/bin"

info() { printf '[INFO] %s\n' "$1"; }
warn() { printf '[WARN] %s\n' "$1"; }
fail() { printf '[ERROR] %s\n' "$1"; exit 1; }

info "Preparing to install CodeGraph (FULL FEATURES)"
info "This build includes ALL embedding providers, LLMs, and experimental features"

[[ "${OSTYPE:-}" == darwin* ]] || fail "This installer targets macOS."
command -v brew >/dev/null 2>&1 || fail "Homebrew is required (https://brew.sh)."
command -v cargo >/dev/null 2>&1 || fail "Rust is required (https://rustup.rs)."

# Check for SurrealDB
if ! command -v surreal >/dev/null 2>&1; then
    warn "SurrealDB CLI not found; installing via Homebrew..."
    brew install surrealdb/tap/surreal >/dev/null
    info "SurrealDB CLI installed"
else
    info "SurrealDB CLI detected"
fi

# Check for Ollama (optional but recommended)
if ! command -v ollama >/dev/null 2>&1; then
    warn "Ollama not found - local LLM/embedding support will be limited"
    warn "Install from: https://ollama.com/download"
else
    info "Ollama detected"
fi

export MACOSX_DEPLOYMENT_TARGET=11.0
info "Compiling CodeGraph with ALL features..."
info "Features: ${FEATURES}"
info ""
info "This may take 5-10 minutes depending on your machine..."

cargo install --path crates/codegraph-mcp --features "${FEATURES}" --force

info "CodeGraph (full features) installed to ${INSTALL_PATH}"
cat <<EOF

✅ Installation Complete - Full Feature Build
=============================================

Enabled Features:
-----------------
🔧 Daemon mode (file watching & auto re-indexing)
🤖 AI-enhanced (agentic tools with tier-aware reasoning)
🧠 All embedding providers:
   - Local: Candle (CPU/GPU), ONNX Runtime
   - Cloud: OpenAI, Jina AI
   - Local API: Ollama, LM Studio
🗣️ All LLM providers:
   - Anthropic Claude (Sonnet, Opus)
   - OpenAI (GPT-4, GPT-5)
   - xAI (Grok)
   - Ollama (local)
   - LM Studio (local)
   - OpenAI-compatible endpoints
🌐 HTTP server (SSE streaming support)
🔬 AutoAgents framework (experimental)
🗄️ SurrealDB backend with HNSW vector search

Next Steps
----------
1. Start SurrealDB with persistent storage:
     surreal start --bind 0.0.0.0:3004 --user root --pass root file://\$HOME/.codegraph/surreal.db

2. Configure your preferred providers in ~/.codegraph/config.toml:

   [embedding]
   provider = "lmstudio"           # or ollama, jina, openai, onnx, local
   model = "jina-embeddings-v4"
   lmstudio_url = "http://localhost:1234/v1"
   dimension = 2048

   [llm]
   enabled = true
   provider = "anthropic"          # or openai, xai, ollama, lmstudio
   model = "claude-sonnet-4"

   [surrealdb]
   url = "${SURR_URL}"
   namespace = "${SURR_NAMESPACE}"
   database = "${SURR_DATABASE}"

3. Set API keys for cloud providers (if using):
     export ANTHROPIC_API_KEY=sk-ant-...
     export OPENAI_API_KEY=sk-...
     export JINA_API_KEY=jina_...

4. Index your repository:
     codegraph init .
     codegraph index . --languages rust,python,typescript

5. Start the MCP server:
     # STDIO mode (Claude Desktop, etc.)
     codegraph start stdio

     # HTTP mode (SSE streaming)
     codegraph start http --host 127.0.0.1 --port ${HTTP_PORT}

6. Enable daemon mode for auto-indexing (optional):
     codegraph daemon start . --foreground

Ensure ${INSTALL_PATH} is on your PATH so editors can find the binary.

For more info: https://github.com/yourusername/codegraph-rust
EOF
