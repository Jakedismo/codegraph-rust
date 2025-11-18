#!/bin/bash
# CodeGraph Universal AI Development Platform - Installation Script
# Revolutionary 11-language semantic analysis with optimized tool descriptions

set -e  # Exit on any error

echo "🚀 Installing CodeGraph Universal AI Development Platform..."
echo "📋 Features: 12 languages, 15 MCP tools (8 standard + 7 agentic), AutoAgents framework, structured outputs with JSON schemas"
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

# Check if Cargo is available
if ! command -v cargo &> /dev/null; then
    echo -e "${RED}❌ Cargo (Rust) not found. Please install Rust: https://rustup.rs${NC}"
    exit 1
fi

echo -e "${BLUE}🔧 Building CodeGraph with universal language support...${NC}"
echo "📊 Languages: Rust, Python, JavaScript, TypeScript, Swift, C#, Ruby, PHP, Go, Java, C++, Dart"
echo "🛠️  Standard Tools: enhanced_search, semantic_intelligence, impact_analysis, pattern_detection, vector_search, graph_neighbors, graph_traverse, performance_metrics"
echo "🤖 Agentic Tools: agentic_code_search, agentic_dependency_analysis, agentic_call_chain_analysis, agentic_architecture_analysis, agentic_api_surface_analysis, agentic_context_builder, agentic_semantic_question"
echo "📊 All agentic tools return structured JSON with REQUIRED file paths"
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

FEATURE_FLAGS="ai-enhanced,autoagents-experimental,qwen-integration,embeddings-ollama,server-http,faiss"
INSTALL_DIR="${CODEGRAPH_INSTALL_DIR:-$HOME/.local/bin}"

echo -e "${BLUE}🚀 Building CodeGraph Universal AI Development Platform...${NC}"
echo "⏱️  This may take a few minutes depending on your system..."
echo "   Core: ai-enhanced (agentic tools), autoagents-experimental (structured outputs)"
echo "   Embeddings: embeddings-ollama (Ollama), built-in ONNX support"
echo "   LLM: qwen-integration (Qwen models)"
echo "   Vector Store: faiss (local high-performance search)"
echo "   Transport: server-http (HTTP + SSE streaming)"
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
    echo -e "${GREEN}🎉 SUCCESS! CodeGraph Universal AI Development Platform installed!${NC}"
    echo ""
    echo -e "${BLUE}📋 What you now have:${NC}"
    echo "   🌍 Universal Language Support: 12 programming languages"
    echo "   🧠 AI Intelligence Tools: 8 standard + 7 agentic tools"
    echo "   🔍 Graph Navigation Tools: 6 SurrealDB graph functions"
    echo "   🤖 AutoAgents Framework: Multi-step reasoning with structured outputs"
    echo "   📊 JSON Schema Enforcement: Required file paths in all agentic responses"
    echo "   🚀 HTTP Transport: SSE streaming support for web integrations"
    echo "   ⚡ Performance: Pattern learning, semantic caching, parallel processing"
    echo ""
    echo -e "${BLUE}🚀 Quick Start:${NC}"
    echo "   1. Navigate to any project directory"
    echo "   2. Run: ${GREEN}codegraph init .${NC}"
    echo "   3. Run: ${GREEN}codegraph index .${NC} (auto-detects all 11 languages)"
    echo "   4. Ensure ${INSTALL_DIR} is on your PATH (e.g. export PATH=\"${INSTALL_DIR}:4PATH\")"
    echo "   5. Use CodeGraph tools in Claude Code!"
    echo ""
    echo -e "${BLUE}🔗 MCP Configuration:${NC}"
    echo "   Global config works from any directory - no manual setup needed!"
        echo ""
        echo -e "${BLUE}🤖 AI Embedding Providers:${NC}"
    echo "   • Ollama (recommended): 384-768-1024-2048-4096 dim models with semantic chunking"
    echo "     - all-minilm:latest (384-dim), embeddinggemma:latest (768-dim)"
    echo "     - qwen3-embedding:0.6b/4b/8b (1024/2048/4096-dim)"
    echo "   • ONNX (built-in): Fast local embeddings for quick prototyping"
    echo "   Set CODEGRAPH_EMBEDDING_PROVIDER=ollama (default) or =onnx"
    echo ""
    echo -e "${BLUE}🧠 LLM Providers for Agentic Tools:${NC}"
    echo "   • Ollama (local): qwen2.5-coder:14b (recommended), deepseek-coder-v2:16b"
    echo "   • OpenAI: gpt-4o, o3-mini (requires OPENAI_API_KEY)"
    echo "   • Anthropic: claude-3-5-sonnet (requires ANTHROPIC_API_KEY)"
    echo "   Set CODEGRAPH_LLM_PROVIDER=ollama (default) or =openai/anthropic"
    echo ""
    echo -e "${BLUE}⚡ Agentic Features:${NC}"
    echo "   • AutoAgents Framework: Multi-step reasoning with tool execution"
    echo "   • Structured Outputs: JSON schemas enforce file paths in responses"
    echo "   • Tier-Aware Prompts: Automatically adapts to LLM context window"
    echo "   • 7 Agentic Tools: code_search, dependency_analysis, call_chain, architecture, api_surface, context_builder, semantic_question"
    echo ""
    echo -e "${BLUE}🌐 Transport Modes:${NC}"
    echo "   • STDIO: Standard MCP protocol (recommended for Claude Desktop)"
    echo "   • HTTP: SSE streaming on port 3000 (experimental, for web integrations)"
    echo "   Start: codegraph start stdio  OR  codegraph start http --port 3000"
    echo ""
    echo -e "${BLUE}📖 Documentation:${NC}"
    echo "   • CHANGELOG.md - Complete v1.1.0 release notes"
    echo "   • README.md - Full platform documentation"
    echo "   • crates/codegraph-napi/GRAPH_FUNCTIONS_GUIDE.md - Graph analysis API"
    echo ""
    echo -e "${GREEN}🌟 Ready for next-generation AI-powered code intelligence!${NC}"
else
    echo ""
    echo -e "${RED}❌ Installation failed. Common issues and solutions:${NC}"
    echo ""
    echo -e "${YELLOW}🔧 If FAISS linking fails:${NC}"
    echo "   brew reinstall faiss"
    echo "   brew link faiss"
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
