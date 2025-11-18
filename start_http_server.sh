#!/bin/bash
# Start CodeGraph HTTP server with all features

set -e

PORT="${CODEGRAPH_HTTP_PORT:-3000}"
HOST="${CODEGRAPH_HTTP_HOST:-127.0.0.1}"

echo "🚀 Starting CodeGraph HTTP Server"
echo "=================================================="
echo "Host: $HOST"
echo "Port: $PORT"
echo ""
echo "Features:"
echo "  ✅ AutoAgents framework (experimental)"
echo "  ✅ Structured outputs with JSON schemas"
echo "  ✅ 7 agentic tools with file path enforcement"
echo "  ✅ SSE streaming support"
echo ""
echo "Configuration:"
echo "  LLM Provider: ${CODEGRAPH_LLM_PROVIDER:-ollama}"
echo "  LLM Model: ${CODEGRAPH_MODEL:-qwen2.5-coder:14b}"
echo "  Context Window: ${CODEGRAPH_CONTEXT_WINDOW:-32768}"
echo ""
echo "  SurrealDB URL: ${SURREALDB_URL:-ws://localhost:3004}"
echo "  SurrealDB Namespace: ${SURREALDB_NAMESPACE:-codegraph}"
echo "  SurrealDB Database: ${SURREALDB_DATABASE:-main}"
echo ""
echo "Press Ctrl+C to stop server"
echo "=================================================="
echo ""

# Check if binary exists
if [ ! -f "./target/release/codegraph" ]; then
    echo "❌ Release binary not found. Building..."
    echo ""
    cargo build --release \
        -p codegraph-mcp \
        --bin codegraph \
        --features "ai-enhanced,autoagents-experimental,embeddings-ollama,server-http,faiss"
    echo ""
    echo "✅ Build complete"
    echo ""
fi

# Start server
./target/release/codegraph start http --host "$HOST" --port "$PORT"
