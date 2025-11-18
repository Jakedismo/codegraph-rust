#!/bin/bash
# Test agentic tools via HTTP transport with structured outputs

set -e

PORT="${CODEGRAPH_HTTP_PORT:-3003}"
HOST="${CODEGRAPH_HTTP_HOST:-127.0.0.1}"
BASE_URL="http://${HOST}:${PORT}"

echo "🌐 Testing CodeGraph HTTP Server with Structured Outputs"
echo "=================================================="
echo "Server: $BASE_URL"
echo ""

# Check if server is running
echo "1. Health Check..."
if curl -sf "${BASE_URL}/health" > /dev/null; then
    echo "✅ Server is running"
else
    echo "❌ Server not running. Start with:"
    echo "   ./target/release/codegraph start http --port $PORT"
    exit 1
fi

echo ""
echo "2. Initialize MCP connection..."
INIT_RESPONSE=$(curl -s -X POST "${BASE_URL}/mcp" \
  -H "Content-Type: application/json" \
  -d '{
    "jsonrpc": "2.0",
    "id": 1,
    "method": "initialize",
    "params": {
      "protocolVersion": "2025-06-18",
      "capabilities": {},
      "clientInfo": {"name": "http-tester", "version": "1.0"}
    }
  }')

echo "$INIT_RESPONSE" | jq '.' 2>/dev/null || echo "$INIT_RESPONSE"

if echo "$INIT_RESPONSE" | grep -q '"result"'; then
    echo "✅ MCP initialized"
else
    echo "❌ Initialize failed"
    exit 1
fi

echo ""
echo "3. Test agentic_code_search with structured output..."
echo "   Query: How is configuration loaded in this codebase?"
echo "   Expected: structured_output with components[] containing file paths"
echo ""

SEARCH_RESPONSE=$(curl -s -X POST "${BASE_URL}/mcp" \
  -H "Content-Type: application/json" \
  -d '{
    "jsonrpc": "2.0",
    "id": 201,
    "method": "tools/call",
    "params": {
      "name": "agentic_code_search",
      "arguments": {
        "query": "How is configuration loaded in this codebase? Find all config loading mechanisms."
      }
    }
  }')

# Save full response
echo "$SEARCH_RESPONSE" > test_http_search_response.json
echo "📁 Full response saved to: test_http_search_response.json"

# Extract and display structured output
echo ""
echo "Response Analysis:"
echo "=================================================="

if echo "$SEARCH_RESPONSE" | jq -e '.result[0].content[0].text' > /dev/null 2>&1; then
    RESULT_TEXT=$(echo "$SEARCH_RESPONSE" | jq -r '.result[0].content[0].text')

    # Parse the result text as JSON
    echo "$RESULT_TEXT" > /tmp/result_parsed.json

    if echo "$RESULT_TEXT" | jq -e '.structured_output' > /dev/null 2>&1; then
        echo "✅ Structured output present!"
        echo ""
        echo "Components found:"
        echo "$RESULT_TEXT" | jq -r '.structured_output.components[] | "  - \(.name) in \(.file_path):\(.line_number // 0)"' 2>/dev/null | head -5

        echo ""
        echo "Patterns identified:"
        echo "$RESULT_TEXT" | jq -r '.structured_output.patterns[]' 2>/dev/null | head -3

        echo ""
        COMP_COUNT=$(echo "$RESULT_TEXT" | jq '.structured_output.components | length' 2>/dev/null)
        echo "Total components with file paths: $COMP_COUNT"
        echo "Steps taken: $(echo "$RESULT_TEXT" | jq -r '.steps_taken')"
    else
        echo "⚠️  No structured_output field found"
        echo "Response keys:"
        echo "$RESULT_TEXT" | jq 'keys' 2>/dev/null || echo "$RESULT_TEXT" | head -20
    fi
else
    echo "❌ Failed to extract result from response"
    echo "$SEARCH_RESPONSE" | jq '.' 2>/dev/null || echo "$SEARCH_RESPONSE"
fi

echo ""
echo "=================================================="
echo "📋 Full results in: test_http_search_response.json"
echo "💡 Inspect with: jq '.result[0].content[0].text | fromjson' test_http_search_response.json"
