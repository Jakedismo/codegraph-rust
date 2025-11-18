#!/usr/bin/env python3
# test_http_agentic_tools.py
#
# HTTP transport version of test_agentic_tools.py
# Tests all 7 agentic MCP tools via HTTP server with SSE streaming
# Same test cases, same inputs, same output structure as STDIO version
#
# REQUIREMENTS:
#   - HTTP server running on port 3003:
#     codegraph start http --port 3003
#     OR
#     ./start_http_server.sh
#   - Python dependencies:
#     pip install -r requirements-test.txt  # Includes requests, python-dotenv
#     OR
#     uv sync
#
# Usage:
#   python3 test_http_agentic_tools.py
#
# Environment variables:
#   CODEGRAPH_HTTP_HOST=127.0.0.1  # Default
#   CODEGRAPH_HTTP_PORT=3000       # Default

import json
import os
import sys
import time
import requests
from pathlib import Path
from datetime import datetime

# Load .env file if python-dotenv is available
try:
    from dotenv import load_dotenv
    env_path = Path(__file__).resolve().parent / ".env"
    if env_path.exists():
        load_dotenv(env_path)
        print(f"✓ Loaded configuration from {env_path}")
    else:
        print(f"⚠️  No .env file found at {env_path}")
except ImportError:
    print("⚠️  python-dotenv not installed. Install with: pip install python-dotenv")
    print("   Falling back to environment variables only")

# Configuration
HTTP_HOST = os.environ.get("CODEGRAPH_HTTP_HOST", "127.0.0.1")
HTTP_PORT = os.environ.get("CODEGRAPH_HTTP_PORT", "3003")
BASE_URL = f"http://{HTTP_HOST}:{HTTP_PORT}"
PROTO_DEFAULT = os.environ.get("MCP_PROTOCOL_VERSION", "2025-06-18")

# Same test cases as STDIO version
AGENTIC_TESTS = [
    ("1. agentic_code_search", {
        "jsonrpc": "2.0",
        "method": "tools/call",
        "params": {
            "name": "agentic_code_search",
            "arguments": {
                "query": "How is configuration loaded in this codebase? Find all config loading mechanisms."
            }
        },
        "id": 201
    }, 60),

    ("2. agentic_dependency_analysis", {
        "jsonrpc": "2.0",
        "method": "tools/call",
        "params": {
            "name": "agentic_dependency_analysis",
            "arguments": {
                "query": "Analyze the dependency chain for the AgenticOrchestrator. What does it depend on?"
            }
        },
        "id": 202
    }, 60),

    ("3. agentic_call_chain_analysis", {
        "jsonrpc": "2.0",
        "method": "tools/call",
        "params": {
            "name": "agentic_call_chain_analysis",
            "arguments": {
                "query": "Trace the call chain from execute_agentic_workflow to the graph analysis tools"
            }
        },
        "id": 203
    }, 60),

    ("4. agentic_architecture_analysis", {
        "jsonrpc": "2.0",
        "method": "tools/call",
        "params": {
            "name": "agentic_architecture_analysis",
            "arguments": {
                "query": "Analyze the architecture of the MCP server. Find coupling metrics and hub nodes."
            }
        },
        "id": 204
    }, 90),

    ("5. agentic_api_surface_analysis", {
        "jsonrpc": "2.0",
        "method": "tools/call",
        "params": {
            "name": "agentic_api_surface_analysis",
            "arguments": {
                "query": "What is the public API surface of the GraphToolExecutor?"
            }
        },
        "id": 205
    }, 60),

    ("6. agentic_context_builder", {
        "jsonrpc": "2.0",
        "method": "tools/call",
        "params": {
            "name": "agentic_context_builder",
            "arguments": {
                "query": "Gather comprehensive context about the tier-aware prompt selection system"
            }
        },
        "id": 206
    }, 90),

    ("7. agentic_semantic_question", {
        "jsonrpc": "2.0",
        "method": "tools/call",
        "params": {
            "name": "agentic_semantic_question",
            "arguments": {
                "query": "How does the LRU cache work in GraphToolExecutor? What gets cached and when?"
            }
        },
        "id": 207
    }, 60),
]

def check_server():
    """Check if HTTP server is running."""
    # Skip health check - just assume server is running and test MCP endpoint directly
    # Health endpoint has session requirements in current implementation
    print(f"Assuming server is running at {BASE_URL}")
    print(f"(Will verify during MCP initialization)")
    return True

class MCPHttpSession:
    """Manages stateful MCP session over HTTP with SSE."""

    def __init__(self, base_url):
        self.base_url = base_url
        self.session = requests.Session()
        self.session_id = None

    def send_request(self, payload, timeout=60):
        """Send MCP request via HTTP POST and wait for SSE response."""
        try:
            start_time = time.time()

            # HTTP server requires both JSON and SSE content types
            headers = {
                "Content-Type": "application/json",
                "Accept": "application/json, text/event-stream"
            }

            # Add session ID if we have one
            if self.session_id:
                headers["Mcp-Session-Id"] = self.session_id

            response = self.session.post(
                f"{self.base_url}/mcp",
                headers=headers,
                json=payload,
                timeout=timeout,
                stream=True
            )

            if response.status_code == 200:
                # Extract session ID from response header (first request only)
                # Headers are case-insensitive in requests
                if self.session_id is None:
                    # Try different case variations
                    for key in ['Mcp-Session-Id', 'mcp-session-id', 'MCP-SESSION-ID']:
                        if key in response.headers:
                            self.session_id = response.headers[key]
                            print(f"   📝 Session ID: {self.session_id[:16]}...")
                            break
                    if self.session_id is None:
                        print(f"   ⚠️  No session ID in headers: {list(response.headers.keys())}")

                # Parse SSE stream for JSON-RPC responses
                # Use iter_lines with chunk_size to properly read streaming data
                result_data = None
                line_count = 0

                # iter_lines can buffer - use raw iteration instead
                buffer = ""
                for chunk in response.iter_content(chunk_size=None, decode_unicode=True):
                    if chunk:
                        buffer += chunk
                        # Process complete lines
                        while '\n' in buffer:
                            line, buffer = buffer.split('\n', 1)
                            line = line.strip()
                            line_count += 1

                            if not line:
                                continue

                            # Skip SSE comments
                            if line.startswith(':'):
                                continue

                            # SSE data events
                            if line.startswith('data: '):
                                data = line[6:]  # Remove 'data: ' prefix
                                try:
                                    event = json.loads(data)
                                    # Look for the final result
                                    if "result" in event:
                                        result_data = event
                                        # For agentic tools, there's only one result - can break
                                        break
                                    elif "error" in event:
                                        print(f"❌ MCP error: {event['error']}")
                                        return None, time.time() - start_time
                                except json.JSONDecodeError as e:
                                    print(f"⚠️  Failed to parse SSE data (line {line_count}): {e}")
                                    print(f"      Data preview: {data[:200]}")
                                    continue

                duration = time.time() - start_time
                if result_data:
                    return result_data, duration
                else:
                    print(f"⚠️  No result found in SSE stream ({line_count} lines read)")
                    return None, duration
            else:
                print(f"❌ HTTP {response.status_code}: {response.text[:200]}")
                return None, time.time() - start_time

        except requests.exceptions.Timeout:
            print(f"⚠️  Request timed out after {timeout}s")
            return None, timeout
        except requests.exceptions.RequestException as e:
            print(f"❌ Request failed: {e}")
            return None, 0

def extract_file_locations(structured_output):
    """Extract file locations from structured output."""
    locations = []

    # Try different field names based on schema type
    for field in ['components', 'evidence', 'endpoints', 'hub_nodes', 'core_components']:
        if field in structured_output:
            for item in structured_output[field]:
                if isinstance(item, dict) and 'file_path' in item:
                    locations.append({
                        "name": item.get("name", ""),
                        "file_path": item["file_path"],
                        "line_number": item.get("line_number")
                    })

    # Check for entry_point (CallChain)
    if 'entry_point' in structured_output:
        ep = structured_output['entry_point']
        if isinstance(ep, dict) and 'file_path' in ep:
            locations.append({
                "name": ep.get("name", ""),
                "file_path": ep["file_path"],
                "line_number": ep.get("line_number")
            })

    return locations

def run():
    print("\n" + "=" * 72)
    print("CodeGraph HTTP Agentic Tools Test Suite")
    print("=" * 72)
    print(f"  Server: {BASE_URL}")
    print(f"  Protocol: {PROTO_DEFAULT}")
    print("=" * 72 + "\n")

    if not check_server():
        return 1

    # Create stateful MCP session
    session = MCPHttpSession(BASE_URL)

    # MCP initialization
    print("\n" + "=" * 72)
    print("Initializing MCP connection...")
    print("=" * 72)

    init_req = {
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": PROTO_DEFAULT,
            "clientInfo": {"name": "codegraph-http-tester", "version": "0.1.0"},
            "capabilities": {}
        }
    }

    init_response, _ = session.send_request(init_req, timeout=5)
    if not init_response or "error" in init_response:
        print("❌ Initialization failed")
        if init_response:
            print(json.dumps(init_response, indent=2))
        return 1

    print("✓ MCP connection initialized")

    # Run agentic tests
    print("\n" + "=" * 72)
    print("Running Agentic Tool Tests via HTTP")
    print("=" * 72)
    print("⚠️  These tests can take 10-90 seconds each due to multi-step reasoning")
    print()

    results = []

    for title, payload, timeout in AGENTIC_TESTS:
        print(f"\n{'=' * 72}")
        print(f"### {title} ###")
        print(f"Query: {payload['params']['arguments']['query']}")
        print(f"Max timeout: {timeout}s")
        print('=' * 72)

        start_time = time.time()
        print(f"⏳ Sending request...", end="", flush=True)

        response, duration = session.send_request(payload, timeout=timeout)

        # Parse result
        success = False
        steps = 0
        structured_output = None
        file_locations = []

        if response and "result" in response:
            try:
                result = response["result"]
                if isinstance(result, list) and len(result) > 0:
                    content = result[0].get("content", [])
                    for item in content:
                        if item.get("type") == "text":
                            data = json.loads(item.get("text", "{}"))
                            steps = int(data.get("steps_taken", 0))

                            # Check for structured output
                            if "structured_output" in data:
                                structured_output = data["structured_output"]
                                success = True
                                file_locations = extract_file_locations(structured_output)
                            elif "answer" in data:
                                # Fallback to legacy format
                                success = True
            except (json.JSONDecodeError, KeyError, ValueError) as e:
                print(f"\n⚠️  Parse error: {e}")

        results.append({
            "test": title,
            "success": success,
            "duration": duration,
            "steps": steps,
            "file_locations": file_locations,
            "has_structured_output": structured_output is not None
        })

        if success:
            print(f"\n✅ SUCCESS in {duration:.1f}s ({steps} reasoning steps)")

            if structured_output:
                print(f"   📊 Structured Output: ✅ PRESENT")
                if file_locations:
                    print(f"   📁 File Locations Found: {len(file_locations)}")
                    for i, loc in enumerate(file_locations[:3]):
                        line_info = f":{loc['line_number']}" if loc['line_number'] else ""
                        print(f"      - {loc['name']} in {loc['file_path']}{line_info}")
                    if len(file_locations) > 3:
                        print(f"      ... and {len(file_locations) - 3} more")
                else:
                    print(f"   ⚠️  No file locations in structured output")

                # Show analysis preview
                if "analysis" in structured_output:
                    preview = structured_output["analysis"][:200] + ("..." if len(structured_output["analysis"]) > 200 else "")
                    print(f"   📝 Analysis: {preview}")
                elif "answer" in structured_output:
                    preview = structured_output["answer"][:200] + ("..." if len(structured_output["answer"]) > 200 else "")
                    print(f"   📝 Answer: {preview}")
            else:
                print(f"   ⚠️  Legacy format (no structured output)")
        else:
            print(f"\n❌ FAILED or TIMEOUT after {duration:.1f}s")

        # Write detailed log
        try:
            os.makedirs("test_output_http", exist_ok=True)
            timestamp = datetime.now().strftime("%Y%m%d_%H%M%S")
            log_filename = f"test_output_http/{title.split('.')[0].zfill(2)}_{title.split(' ')[1]}_{timestamp}.log"

            with open(log_filename, "w") as f:
                f.write("=" * 80 + "\n")
                f.write(f"Test: {title}\n")
                f.write(f"Transport: HTTP\n")
                f.write(f"Timestamp: {timestamp}\n")
                f.write(f"Timeout: {timeout}s\n")
                f.write("=" * 80 + "\n\n")

                f.write("INPUT QUERY:\n")
                f.write("-" * 80 + "\n")
                f.write(f"{payload['params']['arguments']['query']}\n")
                f.write("-" * 80 + "\n\n")

                f.write("OUTPUT:\n")
                f.write("-" * 80 + "\n")

                if structured_output:
                    f.write(json.dumps(structured_output, indent=2))
                    f.write("\n\n")
                    f.write("FILE LOCATIONS EXTRACTED:\n")
                    f.write("-" * 80 + "\n")
                    for loc in file_locations:
                        line_info = f":{loc['line_number']}" if loc['line_number'] else ""
                        f.write(f"  {loc['name']} in {loc['file_path']}{line_info}\n")
                elif response:
                    f.write(json.dumps(response, indent=2))
                else:
                    f.write("(No response received)\n")

                f.write("-" * 80 + "\n\n")
                f.write(f"Duration: {duration:.1f}s\n")
                f.write(f"Status: {'SUCCESS' if success else 'FAILED'}\n")

            print(f"   💾 Log saved: {log_filename}")
        except Exception as e:
            print(f"   ⚠️  Failed to write log: {e}")

    # Print summary
    print("\n" + "=" * 72)
    print("Test Summary")
    print("=" * 72)

    total = len(results)
    passed = sum(1 for r in results if r["success"])
    structured = sum(1 for r in results if r.get("has_structured_output", False))
    total_files = sum(len(r.get("file_locations", [])) for r in results)

    for r in results:
        status = "✅ PASS" if r["success"] else "❌ FAIL"
        print(f"{status} {r['test']}: {r['duration']:.1f}s", end="")
        if r["steps"]:
            print(f" ({r['steps']} steps)", end="")
        if r.get("has_structured_output"):
            print(f" [📊 structured]", end="")
            if r.get("file_locations"):
                print(f" [{len(r['file_locations'])} files]", end="")
        print()

    print(f"\nTotal: {passed}/{total} passed")
    print(f"Structured outputs: {structured}/{total}")
    print(f"File locations found: {total_files}")
    print(f"Transport: HTTP ({BASE_URL})")
    print("=" * 72)

    return 0 if passed == total else 1

if __name__ == "__main__":
    try:
        sys.exit(run())
    except KeyboardInterrupt:
        print("\n\n⚠️  Interrupted by user")
        sys.exit(1)
