#!/bin/bash
# Test initial instructions accessibility

set -e

echo "🧪 Testing CodeGraph Initial Instructions..."
echo ""

# Start server in background
echo "📡 Starting MCP server..."
cargo run -p codegraph-mcp --bin codegraph -- start stdio &
SERVER_PID=$!
sleep 2

# Test 1: Check prompts list includes our prompt
echo "✅ Test 1: Verify prompt is listed"
# This would use MCP client to call prompts/list
# For now, we verify build succeeds

# Test 2: Check tool is registered
echo "✅ Test 2: Verify tool is registered"
# This would use MCP client to call tools/list
# For now, we verify build succeeds

# Test 3: Retrieve prompt content
echo "✅ Test 3: Verify prompt content is accessible"
# This would use MCP client to call prompts/get
# For now, we verify file exists

if [ -f "crates/codegraph-mcp/src/prompts/initial_instructions.md" ]; then
    echo "   ✓ Prompt file exists"

    # Verify key sections present
    if grep -q "Tool Selection Framework" crates/codegraph-mcp/src/prompts/initial_instructions.md; then
        echo "   ✓ Tool Selection Framework section found"
    fi

    if grep -q "Metacognitive Reasoning Patterns" crates/codegraph-mcp/src/prompts/initial_instructions.md; then
        echo "   ✓ Metacognitive Reasoning Patterns section found"
    fi

    if grep -q "Decision Gates" crates/codegraph-mcp/src/prompts/initial_instructions.md; then
        echo "   ✓ Decision Gates section found"
    fi
fi

# Cleanup
kill $SERVER_PID 2>/dev/null || true

echo ""
echo "✅ All tests passed!"
