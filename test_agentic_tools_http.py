#!/usr/bin/env python3
# test_agentic_tools_http.py
#
# Automatic tester for CodeGraph Agentic MCP tools using HTTP server mode.
# - Tests all 7 agentic analysis tools with multi-step reasoning workflows
# - Uses HTTP POST /mcp endpoint with SSE streaming
# - Handles long-running tasks (10-60 seconds per tool)
# - Shows real-time progress as reasoning steps complete
# - Loads configuration from .env file automatically
#
# REQUIREMENTS:
#   - SurrealDB must be running (local or cloud)
#   - Binary built with server-http feature:
#     cargo build --release -p codegraph-mcp --features "ai-enhanced,autoagents-experimental,faiss,embeddings-ollama,server-http"
#   - Python dependencies:
#     pip install requests python-dotenv sseclient-py
#
# Usage:
#   python3 test_agentic_tools_http.py
#
# Environment variables (from .env or shell):
#   CODEGRAPH_HTTP_PORT=3000  # HTTP server port
#   CODEGRAPH_HTTP_HOST=127.0.0.1  # HTTP server host
#   (plus all the SurrealDB and LLM config from original)

import json, os, signal, subprocess, sys, time
import shlex
from pathlib import Path
from datetime import datetime

try:
    import requests
    from sseclient import SSEClient
except ImportError:
    print("❌ Missing required packages. Install with:")
    print("   pip install requests sseclient-py")
    sys.exit(1)

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

# Defaults
PROTO_DEFAULT = os.environ.get("MCP_PROTOCOL_VERSION", "2025-06-18")
LLM_PROVIDER = os.environ.get("CODEGRAPH_LLM_PROVIDER", "ollama")
LLM_MODEL = os.environ.get("CODEGRAPH_MODEL", "qwen2.5-coder:14b")
CONTEXT_WINDOW = int(os.environ.get("CODEGRAPH_CONTEXT_WINDOW", "32768"))
HTTP_HOST = os.environ.get("CODEGRAPH_HTTP_HOST", "127.0.0.1")
HTTP_PORT = int(os.environ.get("CODEGRAPH_HTTP_PORT", "3000"))

# Feature flags
DEFAULT_FEATURES = "ai-enhanced,autoagents-experimental,faiss,embeddings-ollama,server-http"

# Agentic tool tests
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

def send_http_request(url, payload, timeout=60):
    """Send HTTP POST request and parse SSE stream responses."""
    print(f"\n→ Sending to {url}")
    print(f"   Payload: {str(payload)[:200]}...")
    print("=" * 72)

    start_time = time.time()
    responses = []

    try:
        print(f"⏳ Waiting for response (timeout: {timeout}s)...", end="", flush=True)

        # Send request with streaming enabled
        response = requests.post(
            url,
            json=payload,
            headers={"Content-Type": "application/json"},
            stream=True,
            timeout=timeout
        )
        response.raise_for_status()

        # Parse SSE stream
        client = SSEClient(response)
        for event in client.events():
            elapsed = time.time() - start_time

            if elapsed > timeout:
                print(f"\n⚠️  Timeout after {timeout}s")
                break

            if event.data:
                try:
                    data = json.loads(event.data)
                    responses.append(data)

                    # Show progress
                    if "result" in data:
                        print(f"\n✓ Got response after {elapsed:.1f}s")
                        break

                    # Progress dots every 5 seconds
                    if int(elapsed) % 5 == 0 and elapsed > 0:
                        print(".", end="", flush=True)

                except json.JSONDecodeError:
                    pass

        return responses

    except requests.exceptions.Timeout:
        print(f"\n⚠️  HTTP request timeout after {timeout}s")
        return responses
    except requests.exceptions.RequestException as e:
        print(f"\n❌ HTTP request failed: {e}")
        return responses

def check_surrealdb():
    """Check if SurrealDB configuration is present."""
    url = os.environ.get("SURREALDB_URL")
    if not url:
        print("\n" + "=" * 72)
        print("⚠️  WARNING: SURREALDB_URL not configured!")
        print("=" * 72)
        print("\nAgentic tools require SurrealDB to be running.")
        print("\nOption 1: Local SurrealDB")
        print("  1. Install: curl -sSf https://install.surrealdb.com | sh")
        print("  2. Run: surreal start --bind 127.0.0.1:3004 --user root --pass root memory")
        print("  3. Set in .env:")
        print("     SURREALDB_URL=ws://localhost:3004")
        print("     SURREALDB_NAMESPACE=codegraph")
        print("     SURREALDB_DATABASE=main")
        print("\nOption 2: SurrealDB Cloud (Free 1GB instance)")
        print("  1. Sign up at https://surrealdb.com/cloud")
        print("  2. Get connection details from dashboard")
        print("  3. Set in .env:")
        print("     SURREALDB_URL=wss://your-instance.surrealdb.cloud")
        print("     SURREALDB_NAMESPACE=codegraph")
        print("     SURREALDB_DATABASE=main")
        print("     SURREALDB_USERNAME=your-username")
        print("     SURREALDB_PASSWORD=your-password")
        print("\n" + "=" * 72)

        response = input("\nContinue anyway? (y/N): ")
        if response.lower() != 'y':
            sys.exit(1)
    else:
        print(f"✓ SurrealDB configured: {url}")

def print_config():
    """Print configuration being used."""
    print("\n" + "=" * 72)
    print("CodeGraph Agentic Tools Configuration (HTTP Mode):")
    print("=" * 72)
    print(f"  HTTP Server: http://{HTTP_HOST}:{HTTP_PORT}")
    print(f"  LLM Provider: {LLM_PROVIDER}")
    print(f"  LLM Model: {LLM_MODEL}")
    print(f"  Context Window: {CONTEXT_WINDOW}")

    # Determine tier
    if CONTEXT_WINDOW < 50000:
        tier = "Small (<50K)"
        prompt_type = "TERSE"
        max_steps = 5
    elif CONTEXT_WINDOW < 150000:
        tier = "Medium (50K-150K)"
        prompt_type = "BALANCED"
        max_steps = 10
    elif CONTEXT_WINDOW < 500000:
        tier = "Large (150K-500K)"
        prompt_type = "DETAILED"
        max_steps = 15
    else:
        tier = "Massive (>500K)"
        prompt_type = "EXPLORATORY"
        max_steps = 20

    print(f"  Context Tier: {tier}")
    print(f"  Prompt Type: {prompt_type}")
    print(f"  Max Steps: {max_steps}")

    if max_tokens := os.environ.get("MCP_CODE_AGENT_MAX_OUTPUT_TOKENS"):
        print(f"  Max Output Tokens: {max_tokens} (custom)")

    print(f"\n  SurrealDB URL: {os.environ.get('SURREALDB_URL', 'NOT SET')}")
    print(f"  SurrealDB Namespace: {os.environ.get('SURREALDB_NAMESPACE', 'codegraph')}")
    print(f"  SurrealDB Database: {os.environ.get('SURREALDB_DATABASE', 'main')}")

    print(f"\n  Protocol Version: {PROTO_DEFAULT}")
    print("=" * 72 + "\n")

def resolve_codegraph_command():
    """Determine which command should launch the CodeGraph HTTP server."""
    if cmd := os.environ.get("CODEGRAPH_CMD"):
        print(f"Using CODEGRAPH_CMD from environment")
        print(f"⚠️  Ensure it was built with: --features \"{DEFAULT_FEATURES}\"")
        return shlex.split(cmd)

    if binary := os.environ.get("CODEGRAPH_BIN"):
        print(f"Using CODEGRAPH_BIN from environment: {binary}")
        print(f"⚠️  Ensure it was built with: --features \"{DEFAULT_FEATURES}\"")
        return [binary]

    repo_root = Path(__file__).resolve().parent
    release_bin = repo_root / "target" / "release" / "codegraph"
    if release_bin.exists():
        print(f"Using release binary: {release_bin}")
        print(f"⚠️  Ensure it was built with: --features \"{DEFAULT_FEATURES}\"")
        return [str(release_bin)]

    debug_bin = repo_root / "target" / "debug" / "codegraph"
    if debug_bin.exists():
        print(f"Using debug binary: {debug_bin}")
        print(f"⚠️  Ensure it was built with: --features \"{DEFAULT_FEATURES}\"")
        return [str(debug_bin)]

    print(f"No binary found, using cargo run with features: {DEFAULT_FEATURES}")
    return [
        "cargo", "run", "--quiet",
        "-p", "codegraph-mcp",
        "--bin", "codegraph",
        "--features", DEFAULT_FEATURES,
        "--",
    ]

def run():
    check_surrealdb()
    print_config()

    base_cmd = resolve_codegraph_command()
    launch_cmd = base_cmd + ["start", "http", "--host", HTTP_HOST, "--port", str(HTTP_PORT)]

    print(f"Starting CodeGraph HTTP server...")
    print(f"Command: {' '.join(launch_cmd)}\n")

    # Start server
    proc = subprocess.Popen(
        launch_cmd,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True,
        bufsize=1,
    )

    # Wait for server to be ready
    base_url = f"http://{HTTP_HOST}:{HTTP_PORT}"
    print(f"Waiting for HTTP server at {base_url}...")

    for i in range(30):  # Wait up to 30 seconds
        try:
            health_response = requests.get(f"{base_url}/health", timeout=1)
            if health_response.status_code == 200:
                print(f"✓ Server ready after {i+1}s")
                break
        except requests.exceptions.RequestException:
            time.sleep(1)
    else:
        print("❌ Server failed to start within 30 seconds")
        try:
            proc.terminate()
        except Exception:
            pass
        sys.exit(1)

    # MCP handshake
    print("\n" + "=" * 72)
    print("Initializing MCP connection...")
    print("=" * 72)

    init_req = {
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": PROTO_DEFAULT,
            "clientInfo": {"name": "codegraph-agentic-tester-http", "version": "0.1.0"},
            "capabilities": {}
        }
    }

    mcp_url = f"{base_url}/mcp"
    init_responses = send_http_request(mcp_url, init_req, timeout=5)

    if not init_responses or any("error" in str(r).lower() for r in init_responses):
        print("\n❌ Initialization failed")
        try:
            proc.terminate()
        except Exception:
            pass
        sys.exit(1)

    # notifications/initialized
    inited_note = {
        "jsonrpc": "2.0",
        "method": "notifications/initialized",
        "params": {}
    }
    send_http_request(mcp_url, inited_note, timeout=2)

    print("\n✓ MCP connection initialized")

    # Run agentic tests
    print("\n" + "=" * 72)
    print("Running Agentic Tool Tests (HTTP Mode)")
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
        responses = send_http_request(mcp_url, payload, timeout=timeout)
        duration = time.time() - start_time

        # Try to parse the result
        success = False
        steps = 0
        final_answer = None

        for response in responses:
            if "result" in response:
                result = response["result"]
                if isinstance(result, list) and len(result) > 0:
                    content = result[0].get("content", [])
                    for item in content:
                        if item.get("type") == "text":
                            try:
                                data = json.loads(item.get("text", "{}"))
                                steps = data.get("total_steps", 0)
                                final_answer = data.get("final_answer", "")
                                success = True
                            except json.JSONDecodeError:
                                pass

        results.append({
            "test": title,
            "success": success,
            "duration": duration,
            "steps": steps,
            "timeout": timeout
        })

        if success:
            print(f"\n✅ SUCCESS in {duration:.1f}s ({steps} reasoning steps)")
            if final_answer:
                preview = final_answer[:200] + ("..." if len(final_answer) > 200 else "")
                print(f"   Answer preview: {preview}")
        else:
            print(f"\n❌ FAILED or TIMEOUT after {duration:.1f}s")

    # Graceful shutdown
    try:
        proc.terminate()
        proc.wait(timeout=2)
    except Exception:
        try:
            proc.kill()
        except Exception:
            pass

    # Print summary
    print("\n" + "=" * 72)
    print("Test Summary")
    print("=" * 72)

    total = len(results)
    passed = sum(1 for r in results if r["success"])

    for r in results:
        status = "✅ PASS" if r["success"] else "❌ FAIL"
        print(f"{status} {r['test']}: {r['duration']:.1f}s", end="")
        if r["steps"]:
            print(f" ({r['steps']} steps)", end="")
        print()

    print(f"\nTotal: {passed}/{total} passed")
    print("=" * 72)

    return 0 if passed == total else 1

if __name__ == "__main__":
    try:
        sys.exit(run())
    except KeyboardInterrupt:
        print("\n\n⚠️  Interrupted by user")
        sys.exit(1)
