#!/usr/bin/env python3
"""
Test HTTP MCP server with SSE streaming for agentic tools.

REQUIREMENTS:
  - Binary built with: cargo build --release --features "ai-enhanced,autoagents-experimental,faiss,ollama,server-http"
  - Start server: ./target/release/codegraph start http --port 3000

Usage:
  python3 test_http_mcp_client.py
"""

import json
import requests
import sseclient
from typing import Optional

MCP_URL = "http://127.0.0.1:3000/mcp"
SSE_URL = "http://127.0.0.1:3000/sse"

def send_mcp_request(request: dict, session_id: Optional[str] = None):
    """Send MCP request and handle SSE response stream."""
    headers = {"Content-Type": "application/json"}
    if session_id:
        headers["Mcp-Session-Id"] = session_id

    response = requests.post(MCP_URL, json=request, headers=headers, stream=True)
    response.raise_for_status()

    # Extract session ID from response
    new_session_id = response.headers.get("Mcp-Session-Id")

    # Parse SSE stream
    client = sseclient.SSEClient(response)
    results = []
    for event in client.events():
        if event.data:
            results.append(json.loads(event.data))

    return results, new_session_id

def test_initialize():
    """Test MCP initialize handshake."""
    print("Testing initialize...")

    request = {
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": "2025-06-18",
            "capabilities": {},
            "clientInfo": {
                "name": "test-http-client",
                "version": "1.0.0"
            }
        }
    }

    results, session_id = send_mcp_request(request)
    assert session_id, "No session ID returned"
    assert results, "No response received"

    print(f"✓ Initialize successful (session: {session_id})")
    return session_id

def test_list_tools(session_id: str):
    """Test listing available tools."""
    print("Testing list tools...")

    request = {
        "jsonrpc": "2.0",
        "id": 2,
        "method": "tools/list",
        "params": {}
    }

    results, _ = send_mcp_request(request, session_id)
    assert results, "No tools returned"

    tools = results[0].get("result", {}).get("tools", [])
    print(f"✓ Found {len(tools)} tools")

    # Verify agentic tools present
    agentic_tools = [t for t in tools if t["name"].startswith("agentic_")]
    print(f"✓ Found {len(agentic_tools)} agentic tools")

    return tools

def test_vector_search(session_id: str):
    """Test vector search tool."""
    print("Testing vector search...")

    request = {
        "jsonrpc": "2.0",
        "id": 3,
        "method": "tools/call",
        "params": {
            "name": "vector_search",
            "arguments": {
                "query": "graph database",
                "limit": 3
            }
        }
    }

    results, _ = send_mcp_request(request, session_id)
    assert results, "No search results"

    print(f"✓ Vector search returned {len(results)} events")

def main():
    """Run all HTTP MCP tests."""
    print("=" * 72)
    print("CodeGraph HTTP MCP Server Test")
    print("=" * 72)

    try:
        # Test initialize handshake
        session_id = test_initialize()

        # Test list tools
        test_list_tools(session_id)

        # Test vector search
        test_vector_search(session_id)

        print("\n" + "=" * 72)
        print("All tests passed! ✓")
        print("=" * 72)

    except Exception as e:
        print(f"\n✗ Test failed: {e}")
        raise

if __name__ == "__main__":
    main()
