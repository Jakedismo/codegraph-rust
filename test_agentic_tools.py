#!/usr/bin/env python3
# test_agentic_tools.py
#
# Automatic tester for CodeGraph Agentic MCP tools using `codegraph start stdio`.
# - Tests all 7 agentic analysis tools with multi-step reasoning workflows
# - Handles long-running tasks (10-60 seconds per tool)
# - Shows real-time progress as reasoning steps complete
# - Loads configuration from .env file automatically
#
# REQUIREMENTS:
#   - SurrealDB must be running (local or cloud)
#   - python-dotenv: pip install python-dotenv
#
# Usage:
#   python3 test_agentic_tools.py
#
# Environment variables (from .env or shell):
#   SURREALDB_URL=ws://localhost:3004  # or wss://your-instance.surrealdb.cloud
#   SURREALDB_NAMESPACE=codegraph
#   SURREALDB_DATABASE=main
#   SURREALDB_USERNAME=root  # if using auth
#   SURREALDB_PASSWORD=root  # if using auth
#   CODEGRAPH_MODEL=qwen2.5-coder:14b  # or claude-3-5-sonnet-20241022
#   CODEGRAPH_LLM_PROVIDER=ollama  # or anthropic
#   CODEGRAPH_CONTEXT_WINDOW=32768  # affects tier detection
#   MCP_CODE_AGENT_MAX_OUTPUT_TOKENS=4096  # optional override

import json, os, re, select, signal, subprocess, sys, time
import shlex
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

# Defaults
PROTO_DEFAULT = os.environ.get("MCP_PROTOCOL_VERSION", "2025-06-18")
LLM_PROVIDER = os.environ.get("CODEGRAPH_LLM_PROVIDER", "ollama")
LLM_MODEL = os.environ.get("CODEGRAPH_MODEL", "qwen2.5-coder:14b")
CONTEXT_WINDOW = int(os.environ.get("CODEGRAPH_CONTEXT_WINDOW", "32768"))

# Feature flags - agentic tools require ai-enhanced feature
DEFAULT_FEATURES = "ai-enhanced,faiss,ollama"

# Agentic tool tests - these are LONG RUNNING (10-60 seconds each)
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
    }, 60),  # max 60 seconds

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
    }, 90),  # architecture analysis can take longer

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
    }, 90),  # context building can be extensive

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

def drain(proc, seconds=2.0):
    """Read stdout for up to `seconds`, print as it arrives, and return captured text."""
    out = []
    end = time.time() + seconds
    while time.time() < end:
        r, _, _ = select.select([proc.stdout], [], [], 0.2)
        if not r:
            continue
        line = proc.stdout.readline()
        if not line:
            time.sleep(0.05)
            continue
        sys.stdout.write(line)
        out.append(line)
    return "".join(out)

def send(proc, obj, timeout=60, show=True):
    """Send a JSON-RPC message and wait for response with timeout."""
    s = json.dumps(obj, ensure_ascii=False)
    if show:
        print("\n→", s[:200] + ("..." if len(s) > 200 else ""))
        print("=" * 72)

    proc.stdin.write(s + "\n")
    proc.stdin.flush()

    # For agentic tools, we need to wait much longer
    output = ""
    start_time = time.time()
    last_output_time = start_time

    print(f"⏳ Waiting for response (timeout: {timeout}s)...", end="", flush=True)

    while True:
        elapsed = time.time() - start_time

        # Check for timeout
        if elapsed > timeout:
            print(f"\n⚠️  Timeout after {timeout}s")
            return output

        # Read available output
        chunk = drain(proc, 1.0)
        if chunk:
            output += chunk
            last_output_time = time.time()

            # Try to parse JSON responses to show progress
            for line in chunk.split("\n"):
                if not line.strip():
                    continue
                try:
                    msg = json.loads(line)
                    if "result" in msg:
                        print(f"\n✓ Got response after {elapsed:.1f}s")
                        return output
                except json.JSONDecodeError:
                    pass

        # Show progress dots
        if int(elapsed) % 5 == 0 and elapsed > 0:
            print(".", end="", flush=True)

        # If process exited, break
        if proc.poll() is not None:
            print("\n⚠️  Process exited")
            return output

        # If we haven't seen output in a while but haven't timed out, keep waiting
        if time.time() - last_output_time > 10:
            print(".", end="", flush=True)

    return output

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
    print("CodeGraph Agentic Tools Configuration:")
    print("=" * 72)
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
    """Determine which command should launch the CodeGraph MCP server."""
    if cmd := os.environ.get("CODEGRAPH_CMD"):
        return shlex.split(cmd)

    if binary := os.environ.get("CODEGRAPH_BIN"):
        return [binary]

    repo_root = Path(__file__).resolve().parent
    release_bin = repo_root / "target" / "release" / "codegraph"
    if release_bin.exists():
        print(f"Using release binary: {release_bin}")
        return [str(release_bin)]

    debug_bin = repo_root / "target" / "debug" / "codegraph"
    if debug_bin.exists():
        print(f"Using debug binary: {debug_bin}")
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
    launch_cmd = base_cmd + ["start", "stdio"]

    print(f"Starting CodeGraph MCP server...")
    print(f"Command: {' '.join(launch_cmd)}\n")

    # Start server
    proc = subprocess.Popen(
        launch_cmd,
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True,
        bufsize=1,
    )

    # Give it a moment to boot
    time.sleep(1.0)
    drain(proc, 1.0)

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
            "clientInfo": {"name": "codegraph-agentic-tester", "version": "0.1.0"},
            "capabilities": {}
        }
    }

    init_out = send(proc, init_req, timeout=5, show=True)
    if "error" in init_out.lower():
        print("\n❌ Initialization failed. Output above.")
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
    send(proc, inited_note, timeout=2, show=False)
    drain(proc, 0.5)

    print("\n✓ MCP connection initialized")

    # Run agentic tests
    print("\n" + "=" * 72)
    print("Running Agentic Tool Tests")
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
        out = send(proc, payload, timeout=timeout, show=True)
        duration = time.time() - start_time

        # Try to parse the result
        success = False
        steps = 0
        final_answer = None

        for line in out.split("\n"):
            if not line.strip():
                continue
            try:
                msg = json.loads(line)
                if "result" in msg:
                    result = msg["result"]
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
                # Print first 200 chars of answer
                preview = final_answer[:200] + ("..." if len(final_answer) > 200 else "")
                print(f"   Answer preview: {preview}")
        else:
            print(f"\n❌ FAILED or TIMEOUT after {duration:.1f}s")

    # Graceful shutdown
    try:
        proc.send_signal(signal.SIGINT)
        proc.wait(timeout=2)
    except Exception:
        try:
            proc.terminate()
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
