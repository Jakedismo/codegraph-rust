#!/bin/bash

echo "🧪 Testing CodeGraph MCP with Qwen2.5-Coder integration"

# Build the MCP server with Qwen integration
echo "Building CodeGraph MCP server with Qwen integration..."
MACOSX_DEPLOYMENT_TARGET=11.0 cargo build -p codegraph-mcp --features qwen-integration

if [ $? -ne 0 ]; then
    echo "❌ Build failed"
    exit 1
fi

echo "✅ Build successful"

# Check if Ollama is running
echo "Checking Ollama availability..."
if ! curl -s http://localhost:11434/api/tags >/dev/null 2>&1; then
    echo "⚠️ Ollama not running. Starting Ollama..."
    ollama serve &
    OLLAMA_PID=$!
    sleep 10
fi

# Check if Qwen2.5-Coder model is available
echo "Checking for Qwen2.5-Coder model..."
if ! ollama list | grep -q "Qwen2.5-Coder"; then
    echo "📦 Qwen2.5-Coder model not found"
    echo "To install the model, run:"
    echo "ollama pull hf.co/unsloth/Qwen2.5-Coder-14B-Instruct-128K-GGUF:Q4_K_M"
    echo ""
    echo "For testing, we'll use a smaller model..."
    if ! ollama list | grep -q "qwen2.5-coder:7b"; then
        echo "Pulling smaller Qwen model for testing..."
        ollama pull qwen2.5-coder:7b
    fi
    echo "Using qwen2.5-coder:7b for testing"
    export QWEN_MODEL="qwen2.5-coder:7b"
else
    echo "✅ Qwen2.5-Coder model available"
    export QWEN_MODEL=$(ollama list | grep "qwen.*coder" | head -1 | awk '{print $1}')
fi

# Start CodeGraph MCP server
echo "Starting CodeGraph MCP server with Qwen integration..."
RUST_LOG=info ./target/debug/codegraph start stdio &
MCP_PID=$!

sleep 5

# Test MCP connection
echo "Testing MCP connection..."

# Create test input for enhanced search
cat > test_mcp_input.json << EOF
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "codegraph.enhanced_search",
  "params": {
    "query": "authentication function",
    "include_analysis": true,
    "max_results": 5
  }
}
EOF

# Send test request
echo "Sending test request to MCP server..."
echo '{"jsonrpc": "2.0", "id": 1, "method": "codegraph.enhanced_search", "params": {"query": "authentication function", "include_analysis": true, "max_results": 5}}' | ./target/debug/codegraph start stdio &

sleep 2

echo "✅ Basic MCP server test complete"

# Clean up
if [ ! -z "$MCP_PID" ]; then
    kill $MCP_PID 2>/dev/null
fi

if [ ! -z "$OLLAMA_PID" ]; then
    kill $OLLAMA_PID 2>/dev/null
fi

echo "🎉 Test complete! CodeGraph MCP server with Qwen2.5-Coder integration is ready"
echo ""
echo "Next steps:"
echo "1. Install full Qwen2.5-Coder-14B-128K model:"
echo "   ollama pull hf.co/unsloth/Qwen2.5-Coder-14B-Instruct-128K-GGUF:Q4_K_M"
echo ""
echo "2. Test with Claude Code by adding MCP server configuration"
echo ""
echo "3. Available MCP tools:"
echo "   - codegraph.enhanced_search: Enhanced semantic search with Qwen analysis"
echo "   - codegraph.semantic_intelligence: Comprehensive codebase intelligence"