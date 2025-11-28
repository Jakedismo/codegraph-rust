#!/bin/bash
# ABOUTME: Installs the CodeGraph CLI with the SurrealDB-only local preset.
# ABOUTME: Checks macOS prerequisites and compiles codegraph-mcp with agentic tooling enabled.

set -euo pipefail

FEATURES="daemon,ai-enhanced,embeddings,embeddings-local,embeddings-openai,embeddings-ollama,embeddings-jina,cloud,server-http,qwen-integration,codegraph-graph/surrealdb"
SURR_URL="ws://localhost:3004"
SURR_NAMESPACE="ouroboros"
SURR_DATABASE="codegraph"
INSTALL_PATH="${CARGO_HOME:-$HOME/.cargo}/bin"

info() { printf '[INFO] %s\n' "$1"; }
warn() { printf '[WARN] %s\n' "$1"; }
fail() { printf '[ERROR] %s\n' "$1"; exit 1; }

info "Preparing to install CodeGraph (local stack)"

[[ "${OSTYPE:-}" == darwin* ]] || fail "This installer targets macOS."
command -v brew >/dev/null 2>&1 || fail "Homebrew is required (https://brew.sh)."
command -v cargo >/dev/null 2>&1 || fail "Rust is required (https://rustup.rs)."

if ! command -v surreal >/dev/null 2>&1; then
    warn "SurrealDB CLI not found; installing via Homebrew..."
    brew install surrealdb/tap/surreal >/dev/null
    info "SurrealDB CLI installed"
else
    info "SurrealDB CLI detected"
fi

export MACOSX_DEPLOYMENT_TARGET=11.0
info "Compiling CodeGraph with features: ${FEATURES}"

cargo install --path crates/codegraph-mcp --features "${FEATURES}" --force

info "CodeGraph installed to ${INSTALL_PATH}"
cat <<EOF

Next steps
----------
1. Start SurrealDB before indexing:
     surreal start --log trace file://\$HOME/.codegraph/surreal.db
   (Default URL ${SURR_URL}, namespace ${SURR_NAMESPACE}, database ${SURR_DATABASE})
2. Create a .env file in your repo:
     CODEGRAPH_SURREALDB_URL=${SURR_URL}
     CODEGRAPH_SURREALDB_NAMESPACE=${SURR_NAMESPACE}
     CODEGRAPH_SURREALDB_DATABASE=${SURR_DATABASE}
     CODEGRAPH_EMBEDDING_PROVIDER=ollama
     CODEGRAPH_LLM_PROVIDER=ollama
3. From your repo, run:
     codegraph init .
     codegraph index . --force   # rerun indexing when needed
4. Start the MCP server when needed:
     codegraph start stdio       # for Claude Desktop
     codegraph start http --port 3000

Add ${INSTALL_PATH} to your PATH if cargo's bin directory is not already exported.
EOF
