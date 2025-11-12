#!/bin/bash
# CodeGraph Cloud-Enhanced Installation Script
# Features: Jina embeddings, cloud LLM providers (Anthropic/x.AI/OpenAI), SurrealDB backend, HTTP transport

set -e  # Exit on any error

echo "🚀 Installing CodeGraph with Cloud & SurrealDB Support..."
echo "📋 Features: Jina embeddings, Cloud LLM providers, SurrealDB backend, HTTP transport"
echo ""

# Color codes for output
RED='\033[0;31m'
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Check if we're on macOS
if [[ "$OSTYPE" != "darwin"* ]]; then
    echo -e "${RED}❌ This script is optimized for macOS. For other platforms, adjust the FAISS paths accordingly.${NC}"
    exit 1
fi

# Check if Homebrew is installed
if ! command -v brew &> /dev/null; then
    echo -e "${RED}❌ Homebrew not found. Please install Homebrew first: https://brew.sh${NC}"
    exit 1
fi

# Check if FAISS is installed
if ! brew list faiss &> /dev/null; then
    echo -e "${YELLOW}⚠️  FAISS not found. Installing FAISS via Homebrew...${NC}"
    brew install faiss
    echo -e "${GREEN}✅ FAISS installed successfully${NC}"
else
    echo -e "${GREEN}✅ FAISS found${NC}"
fi

# Check if SurrealDB is installed
if ! command -v surreal &> /dev/null; then
    echo -e "${YELLOW}⚠️  SurrealDB not found. Installing SurrealDB via Homebrew...${NC}"
    brew install surrealdb/tap/surreal
    echo -e "${GREEN}✅ SurrealDB installed successfully${NC}"
else
    echo -e "${GREEN}✅ SurrealDB found${NC}"
fi

# Check if Cargo is available
if ! command -v cargo &> /dev/null; then
    echo -e "${RED}❌ Cargo (Rust) not found. Please install Rust: https://rustup.rs${NC}"
    exit 1
fi

echo -e "${BLUE}🔧 Building CodeGraph with cloud features...${NC}"
echo "📊 Languages: Rust, Python, JavaScript, TypeScript, Swift, C#, Ruby, PHP, Go, Java, C++"
echo "🛠️  Tools: enhanced_search, semantic_intelligence, impact_analysis, pattern_detection, vector_search, graph_neighbors, graph_traverse, performance_metrics"
echo "☁️  Cloud: Jina embeddings, Anthropic/x.AI/OpenAI LLM providers"
echo "🏠 Local: Ollama embeddings (local models)"
echo "💾 Database: SurrealDB backend"
echo "🌐 Transport: STDIO, HTTP with SSE streaming, Dual mode"
echo ""

# Load environment variables from .env file if it exists
if [ -f .env ]; then
    echo -e "${BLUE}📄 Loading configuration from .env file...${NC}"
    set -a  # automatically export all variables
    source .env
    set +a
    echo -e "${GREEN}✅ Configuration loaded${NC}"
    echo ""
fi

# Set up environment variables for FAISS linking
export LIBRARY_PATH="/opt/homebrew/opt/faiss/lib:$LIBRARY_PATH"
export LD_LIBRARY_PATH="/opt/homebrew/opt/faiss/lib:$LD_LIBRARY_PATH"
export DYLD_LIBRARY_PATH="/opt/homebrew/opt/faiss/lib:$DYLD_LIBRARY_PATH"
export MACOSX_DEPLOYMENT_TARGET=11.0

echo -e "${BLUE}🔗 Environment configured:${NC}"
echo "   LIBRARY_PATH: /opt/homebrew/opt/faiss/lib"
echo "   LD_LIBRARY_PATH: /opt/homebrew/opt/faiss/lib"
echo "   DYLD_LIBRARY_PATH: /opt/homebrew/opt/faiss/lib"
echo "   MACOSX_DEPLOYMENT_TARGET: 11.0"
echo ""

# Feature flags for cloud setup:
# - ai-enhanced: Core AI features (includes faiss + embeddings + codegraph-ai)
# - codegraph-vector/jina: Jina embeddings provider
# - embeddings-ollama: Ollama embeddings provider (local models)
# - codegraph-graph/surrealdb: SurrealDB backend support
# - codegraph-ai/all-cloud-providers: Anthropic, OpenAI, x.AI and OpenAI-compatible providers
# - server-http: HTTP transport with SSE streaming for MCP server
# - autoagents-experimental: EXPERIMENTAL AutoAgents framework for improved agentic orchestration
FEATURE_FLAGS="ai-enhanced,codegraph-vector/jina,embeddings-ollama,codegraph-graph/surrealdb,codegraph-ai/all-cloud-providers,server-http,autoagents-experimental"
INSTALL_DIR="${CODEGRAPH_INSTALL_DIR:-$HOME/.local/bin}"

echo -e "${BLUE}🚀 Building CodeGraph with Cloud Features...${NC}"
echo "⏱️  This may take a few minutes depending on your system..."
echo "   Features: ${FEATURE_FLAGS}"
echo ""

cargo build --release \
    --package codegraph-mcp \
    --bin codegraph \
    --features "${FEATURE_FLAGS}" || {
    echo ""
    echo -e "${RED}❌ Build failed. Please review the error log above.${NC}"
    exit 1
}

echo -e "${BLUE}📦 Installing binary to ${INSTALL_DIR}${NC}"
mkdir -p "${INSTALL_DIR}"
cp -f "$(pwd)/target/release/codegraph" "${INSTALL_DIR}/codegraph"
chmod +x "${INSTALL_DIR}/codegraph"

if [ $? -eq 0 ]; then
    echo ""
    echo -e "${GREEN}🎉 SUCCESS! CodeGraph with Cloud Features installed!${NC}"
    echo ""
    echo -e "${BLUE}📋 What you now have:${NC}"
    echo "   🌍 Universal Language Support: 11 programming languages"
    echo "   🧠 AI Intelligence Tools: 4 revolutionary analysis tools"
    echo "   🔍 Graph Navigation Tools: 3 dependency analysis tools"
    echo "   📊 Performance Tools: 1 system monitoring tool"
    echo "   ☁️  Cloud Embeddings: Jina reranking and embeddings"
    echo "   🏠 Local Embeddings: Ollama (local models)"
    echo "   🤖 Cloud LLM: Anthropic (Claude), OpenAI, and compatible providers"
    echo "   💾 Database: SurrealDB backend for scalable graph storage"
    echo "   🌐 MCP Transports: STDIO, HTTP (with SSE streaming), and Dual mode"
    echo "   🔬 EXPERIMENTAL: AutoAgents framework for improved agentic orchestration"
    echo ""
    echo -e "${BLUE}🗄️  SurrealDB Setup:${NC}"
    echo "   Before using CodeGraph, you need to start SurrealDB:"
    echo ""
    echo "   ${GREEN}# Option 1: Memory mode (development)${NC}"
    echo "   surreal start --log trace memory"
    echo ""
    echo "   ${GREEN}# Option 2: File-based (persistent)${NC}"
    echo "   surreal start --log trace file://\$HOME/.codegraph/surrealdb"
    echo ""
    echo "   ${GREEN}# Option 3: RocksDB (production)${NC}"
    echo "   surreal start --log trace rocksdb://\$HOME/.codegraph/surrealdb"
    echo ""
    echo "   Default connection: ws://localhost:8000"
    echo "   Set CODEGRAPH_SURREALDB_URL to customize"
    echo ""
    echo -e "${BLUE}🔑 Configuration Setup (.env file):${NC}"
    echo "   Create a ${GREEN}.env${NC} file in your project root with:"
    echo ""
    echo "   ${GREEN}# Embedding Provider${NC}"
    echo "   CODEGRAPH_EMBEDDING_PROVIDER=jina"
    echo "   JINA_API_KEY=your-jina-api-key"
    echo ""
    echo "   ${GREEN}# LLM Provider (choose one)${NC}"
    echo "   CODEGRAPH_LLM_PROVIDER=anthropic"
    echo "   ANTHROPIC_API_KEY=your-anthropic-api-key"
    echo ""
    echo "   ${GREEN}# Or OpenAI${NC}"
    echo "   # CODEGRAPH_LLM_PROVIDER=openai"
    echo "   # OPENAI_API_KEY=your-openai-api-key"
    echo ""
    echo "   ${GREEN}# Or x:AI${NC}"
    echo "   # CODEGRAPH_LLM_PROVIDER=xai"
    echo "   # OPENAI_API_KEY=your-xai-api-key"
    echo ""
    echo "   ${GREEN}# Or use both (specify which to use)${NC}"
    echo "   CODEGRAPH_MODEL=gpt-5-codex  # or claude-4-5-sonnet-20250925 or f.ex. grok-4-fast"
    echo ""
    echo -e "${BLUE}🚀 Quick Start:${NC}"
    echo "   1. Start SurrealDB: ${GREEN}surreal start --bind 0.0.0.0:3004 --user root --pass root file://data/surreal.db${NC}"
    echo "   2. Navigate to your project directory"
    echo "   3. Create ${GREEN}.env${NC} file with API keys (see above)"
    echo "   4. Run: ${GREEN}codegraph index .${NC} (auto-detects all 11 languages)"
    echo "   5. Ensure ${INSTALL_DIR} is on your PATH (add to ~/.zshrc: export PATH=\"${INSTALL_DIR}:\$PATH\")"
    echo "   6. Use CodeGraph tools in Claude Code!"
    echo ""
    echo -e "${BLUE}🌐 MCP Server Transport Options:${NC}"
    echo "   ${GREEN}# STDIO transport (default, for Claude Desktop)${NC}"
    echo "   codegraph start stdio"
    echo ""
    echo "   ${GREEN}# HTTP transport with SSE streaming (for web clients)${NC}"
    echo "   codegraph start http --host 127.0.0.1 --port 3000"
    echo ""
    echo "   ${GREEN}# Dual mode (both STDIO and HTTP simultaneously)${NC}"
    echo "   codegraph start dual --host 127.0.0.1 --port 3000"
    echo ""
    echo "   ${GREEN}# HTTP with TLS (production)${NC}"
    echo "   codegraph start http --tls --cert cert.pem --key key.pem"
    echo ""
    echo -e "${BLUE}⚙️  Note:${NC}"
    echo "   • All configuration now uses ${GREEN}.env${NC} files (no exports needed)"
    echo "   • The indexer automatically reads from .env in the current directory"
    echo "   • SurrealDB runs on port ${GREEN}3004${NC} with namespace ${GREEN}ouroboros${NC}"
    echo ""
    echo -e "${BLUE}📖 Documentation:${NC}"
    echo "   • CODEGRAPH-MCP-TOOLS-GUIDE.md - Copy to other projects"
    echo "   • CHANGELOG.md - Complete release notes"
    echo "   • README.md - Full platform documentation"
    echo ""
    echo -e "${GREEN}🌟 Ready to revolutionize development with cloud-powered AI!${NC}"
else
    echo ""
    echo -e "${RED}❌ Installation failed. Common issues and solutions:${NC}"
    echo ""
    echo -e "${YELLOW}🔧 If FAISS linking fails:${NC}"
    echo "   brew reinstall faiss"
    echo "   brew link faiss"
    echo ""
    echo -e "${YELLOW}🔧 If SurrealDB installation fails:${NC}"
    echo "   brew install surrealdb/tap/surreal"
    echo ""
    echo -e "${YELLOW}🔧 If Rust compilation fails:${NC}"
    echo "   rustup update"
    echo "   cargo clean"
    echo ""
    echo -e "${YELLOW}🔧 If dependency issues:${NC}"
    echo "   brew update"
    echo "   brew upgrade"
    echo ""
    echo "Please resolve the issue and run this script again."
    exit 1
fi
