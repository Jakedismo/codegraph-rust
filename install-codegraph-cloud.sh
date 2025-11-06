#!/bin/bash
# CodeGraph Cloud-Enhanced Installation Script
# Features: Jina embeddings, cloud LLM providers (Anthropic/OpenAI), SurrealDB backend

set -e  # Exit on any error

echo "🚀 Installing CodeGraph with Cloud & SurrealDB Support..."
echo "📋 Features: Jina embeddings, Cloud LLM providers, SurrealDB backend"
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
echo "☁️  Cloud: Jina embeddings, Anthropic/OpenAI LLM providers"
echo "💾 Database: SurrealDB backend"
echo ""

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
# - codegraph-graph/surrealdb: SurrealDB backend support
# - codegraph-ai/all-cloud-providers: Anthropic, OpenAI, and OpenAI-compatible providers
FEATURE_FLAGS="ai-enhanced,codegraph-vector/jina,codegraph-graph/surrealdb,codegraph-ai/all-cloud-providers"
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
    echo "   🤖 Cloud LLM: Anthropic (Claude), OpenAI, and compatible providers"
    echo "   💾 Database: SurrealDB backend for scalable graph storage"
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
    echo -e "${BLUE}🔑 API Keys Setup:${NC}"
    echo "   ${GREEN}# For Jina embeddings${NC}"
    echo "   export JINA_API_KEY='your-jina-api-key'"
    echo ""
    echo "   ${GREEN}# For Anthropic (Claude)${NC}"
    echo "   export ANTHROPIC_API_KEY='your-anthropic-api-key'"
    echo ""
    echo "   ${GREEN}# For OpenAI${NC}"
    echo "   export OPENAI_API_KEY='your-openai-api-key'"
    echo ""
    echo -e "${BLUE}🚀 Quick Start:${NC}"
    echo "   1. Start SurrealDB: ${GREEN}surreal start memory${NC}"
    echo "   2. Navigate to any project directory"
    echo "   3. Run: ${GREEN}codegraph init .${NC}"
    echo "   4. Run: ${GREEN}codegraph index .${NC} (auto-detects all 11 languages)"
    echo "   5. Ensure ${INSTALL_DIR} is on your PATH (e.g. export PATH=\"${INSTALL_DIR}:\$PATH\")"
    echo "   6. Use CodeGraph tools in Claude Code!"
    echo ""
    echo -e "${BLUE}⚙️  Configuration:${NC}"
    echo "   Set embedding provider: ${GREEN}export CODEGRAPH_EMBEDDING_PROVIDER=jina${NC}"
    echo "   Set LLM provider: ${GREEN}export CODEGRAPH_LLM_PROVIDER=anthropic${NC}"
    echo "   Set SurrealDB URL: ${GREEN}export CODEGRAPH_SURREALDB_URL=ws://localhost:8000${NC}"
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
