#!/bin/bash
# ABOUTME: Builds the CodeGraph CLI from source with the fast local preset on macOS.
# ABOUTME: Produces a release binary, installs it into \$CODEGRAPH_INSTALL_DIR, and documents SurrealDB-only setup.

set -euo pipefail

FEATURES="daemon,ai-enhanced,autoagents-experimental,qwen-integration,embeddings-ollama,server-http,codegraph-graph/surrealdb"
INSTALL_DIR="${CODEGRAPH_INSTALL_DIR:-$HOME/.local/bin}"
TARGET_BIN="target/release/codegraph"

info() { printf '[INFO] %s\n' "$1"; }
warn() { printf '[WARN] %s\n' "$1"; }
fail() { printf '[ERROR] %s\n' "$1"; exit 1; }

info "Building CodeGraph (local speed preset)"

[[ "${OSTYPE:-}" == darwin* ]] || fail "This script is optimized for macOS."
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
info "Running cargo build --release with features: ${FEATURES}"

cargo build --release \
    --package codegraph-mcp \
    --bin codegraph \
    --features "${FEATURES}"

test -f "${TARGET_BIN}" || fail "Release binary was not produced at ${TARGET_BIN}"

info "Copying binary to ${INSTALL_DIR}"
mkdir -p "${INSTALL_DIR}"
cp -f "${TARGET_BIN}" "${INSTALL_DIR}/codegraph"
chmod +x "${INSTALL_DIR}/codegraph"

info "CodeGraph ready at ${INSTALL_DIR}/codegraph"
cat <<EOF

Next steps
----------
1. Start SurrealDB before indexing:
     surreal start --log trace file://\$HOME/.codegraph/surreal.db
2. Create a .env file per repository:
     CODEGRAPH_SURREALDB_URL=ws://localhost:3004
     CODEGRAPH_SURREALDB_NAMESPACE=ouroboros
     CODEGRAPH_SURREALDB_DATABASE=codegraph
     CODEGRAPH_EMBEDDING_PROVIDER=ollama
     CODEGRAPH_LLM_PROVIDER=ollama
3. Warm up your project:
     codegraph init .
     codegraph index . --force
4. Launch the MCP server:
     codegraph start stdio          # Claude Desktop
     codegraph start http --port 3000

Add ${INSTALL_DIR} to your PATH (e.g. export PATH="${INSTALL_DIR}:\$PATH") if it is not already available.
EOF
