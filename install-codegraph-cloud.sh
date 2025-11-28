#!/bin/bash
# ABOUTME: Installs the CodeGraph CLI with cloud-capable features (Jina + HTTP server).
# ABOUTME: Validates macOS prerequisites and sets up a SurrealDB-only deployment path.

set -euo pipefail

FEATURES="daemon,ai-enhanced,codegraph-vector/jina,embeddings-ollama,codegraph-graph/surrealdb,codegraph-ai/all-cloud-providers,server-http,autoagents-experimental"
SURR_URL="${CODEGRAPH_SURREALDB_URL:-ws://localhost:3004}"
SURR_NAMESPACE="${CODEGRAPH_SURREALDB_NAMESPACE:-ouroboros}"
SURR_DATABASE="${CODEGRAPH_SURREALDB_DATABASE:-codegraph}"
HTTP_PORT="${CODEGRAPH_HTTP_PORT:-3000}"
INSTALL_PATH="${CARGO_HOME:-$HOME/.cargo}/bin"

info() { printf '[INFO] %s\n' "$1"; }
warn() { printf '[WARN] %s\n' "$1"; }
fail() { printf '[ERROR] %s\n' "$1"; exit 1; }

info "Preparing to install CodeGraph (cloud stack)"

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
1. Start SurrealDB with persistent storage (adjust path or port as needed):
     surreal start --bind 0.0.0.0:3004 --user root --pass root file://\$HOME/.codegraph/surreal.db
2. Configure cloud providers in your project .env:
     CODEGRAPH_SURREALDB_URL=${SURR_URL}
     CODEGRAPH_SURREALDB_NAMESPACE=${SURR_NAMESPACE}
     CODEGRAPH_SURREALDB_DATABASE=${SURR_DATABASE}
     CODEGRAPH_EMBEDDING_PROVIDER=ollama          # or jina if remote only
     CODEGRAPH_LLM_PROVIDER=openai                # or anthropic/xai/ollama
     OPENAI_API_KEY=sk-...                        # set keys for the providers you enable
3. Index your repository:
     codegraph init .
     codegraph index . --languages rust,python    # optional language filter
4. Run the MCP server over HTTP (SSE enabled):
     codegraph start http --host 127.0.0.1 --port ${HTTP_PORT}
   STDIO transport remains available via: codegraph start stdio

Ensure ${INSTALL_PATH} is on your PATH so editors can find the binary.
EOF
