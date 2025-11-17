#!/usr/bin/env python3
"""
Test CodeGraph agentic MCP tools using official MCP Python SDK.

This implementation uses the official MCP Python SDK with proper protocol
support for both stdio and SSE (HTTP) transports.

Requirements:
    pip install mcp python-dotenv

Configuration (.env file):
    # Transport mode
    MCP_TRANSPORT=http  # or "stdio"

    # HTTP transport
    CODEGRAPH_HTTP_PORT=3000
    CODEGRAPH_HTTP_HOST=127.0.0.1

    # Stdio transport (optional - defaults to release binary)
    CODEGRAPH_BIN=./target/release/codegraph

    # SurrealDB (required for agentic tools)
    SURREALDB_URL=ws://localhost:3004
    SURREALDB_NAMESPACE=ouroboros
    SURREALDB_DATABASE=codegraph
    SURREALDB_USERNAME=root
    SURREALDB_PASSWORD=root

    # LLM Configuration
    CODEGRAPH_LLM_PROVIDER=ollama
    CODEGRAPH_MODEL=qwen2.5-coder:14b
    CODEGRAPH_CONTEXT_WINDOW=32768

Usage:
    # HTTP transport (server must be running)
    MCP_TRANSPORT=http python3 test_agentic_mcp.py

    # Stdio transport (starts server automatically)
    MCP_TRANSPORT=stdio python3 test_agentic_mcp.py
"""

import asyncio
import json
import os
import sys
from pathlib import Path
from datetime import datetime
from typing import Optional

# Load .env first
try:
    from dotenv import load_dotenv
    env_path = Path(__file__).resolve().parent / ".env"
    if env_path.exists():
        load_dotenv(env_path, override=True)
        ENV_LOADED = True
    else:
        ENV_LOADED = False
except ImportError:
    ENV_LOADED = False

# Check MCP SDK
try:
    from mcp import ClientSession, StdioServerParameters
    from mcp.client.stdio import stdio_client
    from mcp.client.sse import sse_client
except ImportError:
    print("❌ MCP Python SDK not installed")
    print("\nInstall with:")
    print("  pip install mcp python-dotenv")
    sys.exit(1)

# Configuration
TRANSPORT = os.environ.get("MCP_TRANSPORT", "stdio").lower()
HTTP_HOST = os.environ.get("CODEGRAPH_HTTP_HOST", "127.0.0.1")
HTTP_PORT = int(os.environ.get("CODEGRAPH_HTTP_PORT", "3000"))
CODEGRAPH_BIN = os.environ.get("CODEGRAPH_BIN")
LLM_PROVIDER = os.environ.get("CODEGRAPH_LLM_PROVIDER", "ollama")
LLM_MODEL = os.environ.get("CODEGRAPH_MODEL", "qwen2.5-coder:14b")
CONTEXT_WINDOW = int(os.environ.get("CODEGRAPH_CONTEXT_WINDOW", "32768"))

# Test cases: (tool_name, query, timeout_seconds)
AGENTIC_TESTS = [
    ("agentic_code_search",
     "How is configuration loaded in this codebase? Find all config loading mechanisms.",
     300),

    ("agentic_dependency_analysis",
     "Analyze the dependency chain for the AgenticOrchestrator. What does it depend on?",
     300),

    ("agentic_call_chain_analysis",
     "Trace the call chain from execute_agentic_workflow to the graph analysis tools",
     300),

    ("agentic_architecture_analysis",
     "Analyze the architecture of the MCP server. Find coupling metrics and hub nodes.",
     300),

    ("agentic_api_surface_analysis",
     "What is the public API surface of the GraphToolExecutor?",
     300),

    ("agentic_context_builder",
     "Gather comprehensive context about the tier-aware prompt selection system",
     300),

    ("agentic_semantic_question",
     "How does the LRU cache work in GraphToolExecutor? What gets cached and when?",
     300),
]

# Create output directory for test logs
TEST_OUTPUT_DIR = Path(__file__).resolve().parent / "test_output"
TEST_OUTPUT_DIR.mkdir(exist_ok=True)


def print_config():
    """Print resolved configuration."""
    print("\n" + "=" * 72)
    config_source = "from .env file" if ENV_LOADED else "from environment"
    print(f"MCP Client Configuration ({config_source}):")
    print("=" * 72)
    print(f"  Transport: {TRANSPORT}")

    if TRANSPORT == "http":
        print(f"  HTTP Server: http://{HTTP_HOST}:{HTTP_PORT}/mcp")
    else:
        binary = resolve_codegraph_binary()
        print(f"  Binary: {binary}")

    print(f"\n  LLM: {LLM_PROVIDER} / {LLM_MODEL}")
    print(f"  Context Window: {CONTEXT_WINDOW}")

    # Tier detection
    if CONTEXT_WINDOW < 50000:
        tier, prompt_type, max_steps = "Small (<50K)", "TERSE", 5
    elif CONTEXT_WINDOW < 150000:
        tier, prompt_type, max_steps = "Medium (50K-150K)", "BALANCED", 10
    elif CONTEXT_WINDOW < 500000:
        tier, prompt_type, max_steps = "Large (150K-500K)", "DETAILED", 15
    else:
        tier, prompt_type, max_steps = "Massive (>500K)", "EXPLORATORY", 20

    print(f"  Tier: {tier} | {prompt_type} | {max_steps} steps")

    # SurrealDB
    url = os.environ.get("CODEGRAPH_SURREALDB_URL", "localhost:3004")
    ns = os.environ.get("CODEGRAPH_SURREALDB_NAMESPACE", "ouroboros")
    db = os.environ.get("CODEGRAPH_SURREALDB_DATABASE", "codegraph")
    print(f"\n  SurrealDB: {url}")
    print(f"  Namespace/DB: {ns}/{db}")

    print(f"\n  📁 Test Output Directory: {TEST_OUTPUT_DIR}")
    print("=" * 72)


def resolve_codegraph_binary() -> str:
    """Find the codegraph binary to use for stdio transport."""
    if CODEGRAPH_BIN:
        return CODEGRAPH_BIN

    repo_root = Path(__file__).resolve().parent

    # Try release first
    release_bin = repo_root / "target" / "release" / "codegraph"
    if release_bin.exists():
        return str(release_bin)

    # Try debug
    debug_bin = repo_root / "target" / "debug" / "codegraph"
    if debug_bin.exists():
        return str(debug_bin)

    # Fallback to just "codegraph" and hope it's in PATH
    return "codegraph"


async def run_stdio_tests():
    """Run tests using stdio transport."""
    print("\n🔌 Using STDIO transport")

    binary = resolve_codegraph_binary()

    # Check if binary exists
    binary_path = Path(binary)
    if not binary_path.is_absolute() or not binary_path.exists():
        print(f"⚠️  Binary not found: {binary}")
        print("   Assuming it's in PATH or will use cargo run")

    server_params = StdioServerParameters(
        command=binary,
        args=["start", "stdio"],
        env=None  # Inherit environment variables
    )

    print(f"  Command: {binary} start stdio")
    print("  Starting server...\n")

    results = []

    try:
        async with stdio_client(server_params) as (read, write):
            async with ClientSession(read, write) as session:
                # Initialize
                await session.initialize()
                print("✓ MCP session initialized\n")

                # List tools
                tools_result = await session.list_tools()
                tool_names = [t.name for t in tools_result.tools]
                print(f"✓ Found {len(tool_names)} tools")

                agentic_tools = [t for t in tool_names if t.startswith("agentic_")]
                print(f"  Agentic tools: {len(agentic_tools)}")

                if not agentic_tools:
                    print("❌ No agentic tools found!")
                    return 1

                # Run tests
                print("\n" + "=" * 72)
                print("Running Agentic Tool Tests (stdio transport)")
                print("=" * 72)

                for idx, (tool_name, query, timeout) in enumerate(AGENTIC_TESTS, 1):
                    print(f"\n[{idx}/{len(AGENTIC_TESTS)}] Testing: {tool_name}")
                    print(f"  Query: {query[:60]}...")
                    print(f"  Timeout: {timeout}s")

                    # Create log file for this test
                    timestamp = datetime.now().strftime("%Y%m%d_%H%M%S")
                    log_file = TEST_OUTPUT_DIR / f"{idx:02d}_{tool_name}_{timestamp}.log"

                    start_time = asyncio.get_event_loop().time()

                    # Write input to log file
                    with open(log_file, "w", encoding="utf-8") as f:
                        f.write("=" * 80 + "\n")
                        f.write(f"Test: {tool_name}\n")
                        f.write(f"Timestamp: {timestamp}\n")
                        f.write(f"Timeout: {timeout}s\n")
                        f.write("=" * 80 + "\n\n")
                        f.write("INPUT QUERY:\n")
                        f.write("-" * 80 + "\n")
                        f.write(query + "\n")
                        f.write("-" * 80 + "\n\n")

                    try:
                        # Call tool with timeout
                        result = await asyncio.wait_for(
                            session.call_tool(tool_name, arguments={"query": query}),
                            timeout=timeout
                        )

                        duration = asyncio.get_event_loop().time() - start_time

                        # Parse result
                        if result.content and len(result.content) > 0:
                            text_content = result.content[0].text

                            # Write output to log file
                            with open(log_file, "a", encoding="utf-8") as f:
                                f.write("OUTPUT:\n")
                                f.write("-" * 80 + "\n")
                                f.write(text_content + "\n")
                                f.write("-" * 80 + "\n\n")
                                f.write(f"Duration: {duration:.1f}s\n")
                                f.write(f"Status: SUCCESS\n")

                            try:
                                data = json.loads(text_content)
                                steps = data.get("total_steps", 0)
                                final_answer = data.get("final_answer", "")

                                print(f"  ✅ SUCCESS in {duration:.1f}s ({steps} steps)")
                                print(f"  📝 Log saved: {log_file.name}")
                                if final_answer:
                                    preview = final_answer[:100].replace('\n', ' ')
                                    print(f"     {preview}...")

                                results.append({
                                    "test": tool_name,
                                    "success": True,
                                    "duration": duration,
                                    "steps": steps
                                })
                            except json.JSONDecodeError:
                                print(f"  ✅ SUCCESS in {duration:.1f}s (non-JSON response)")
                                print(f"  📝 Log saved: {log_file.name}")
                                results.append({
                                    "test": tool_name,
                                    "success": True,
                                    "duration": duration,
                                    "steps": 0
                                })
                        else:
                            with open(log_file, "a", encoding="utf-8") as f:
                                f.write("OUTPUT:\n")
                                f.write("-" * 80 + "\n")
                                f.write("(Empty result)\n")
                                f.write("-" * 80 + "\n\n")
                                f.write(f"Duration: {duration:.1f}s\n")
                                f.write(f"Status: FAILED (empty result)\n")

                            print(f"  ❌ FAILED: Empty result")
                            print(f"  📝 Log saved: {log_file.name}")
                            results.append({
                                "test": tool_name,
                                "success": False,
                                "duration": duration,
                                "steps": 0
                            })

                    except asyncio.TimeoutError:
                        duration = timeout
                        with open(log_file, "a", encoding="utf-8") as f:
                            f.write("OUTPUT:\n")
                            f.write("-" * 80 + "\n")
                            f.write(f"TIMEOUT after {timeout}s\n")
                            f.write("-" * 80 + "\n\n")
                            f.write(f"Duration: {duration:.1f}s\n")
                            f.write(f"Status: TIMEOUT\n")

                        print(f"  ❌ TIMEOUT after {timeout}s")
                        print(f"  📝 Log saved: {log_file.name}")
                        results.append({
                            "test": tool_name,
                            "success": False,
                            "duration": duration,
                            "steps": 0
                        })

                    except Exception as e:
                        duration = asyncio.get_event_loop().time() - start_time
                        import traceback
                        error_trace = traceback.format_exc()

                        with open(log_file, "a", encoding="utf-8") as f:
                            f.write("OUTPUT:\n")
                            f.write("-" * 80 + "\n")
                            f.write(f"ERROR: {e}\n\n")
                            f.write("Full traceback:\n")
                            f.write(error_trace)
                            f.write("-" * 80 + "\n\n")
                            f.write(f"Duration: {duration:.1f}s\n")
                            f.write(f"Status: ERROR\n")

                        print(f"  ❌ ERROR: {e}")
                        print(f"  📝 Log saved: {log_file.name}")
                        print(f"\n  📋 Full error details:")
                        traceback.print_exc()
                        results.append({
                            "test": tool_name,
                            "success": False,
                            "duration": duration,
                            "steps": 0
                        })

    except Exception as e:
        print(f"\n❌ Failed to connect via stdio: {e}")
        return 1

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
    print(f"\n📁 Detailed logs saved to: {TEST_OUTPUT_DIR}")
    print("=" * 72)

    return 0 if passed == total else 1


async def run_http_tests():
    """Run tests using SSE (HTTP) transport."""
    print("\n🌐 Using HTTP/SSE transport")

    mcp_url = f"http://{HTTP_HOST}:{HTTP_PORT}/mcp"
    print(f"  Connecting to: {mcp_url}")
    print(f"  (Server must be running: ./target/release/codegraph start http --port {HTTP_PORT})\n")

    results = []

    try:
        async with sse_client(url=mcp_url) as streams:
            async with ClientSession(*streams) as session:
                # Initialize
                await session.initialize()
                print("✓ MCP session initialized\n")

                # List tools
                tools_result = await session.list_tools()
                tool_names = [t.name for t in tools_result.tools]
                print(f"✓ Found {len(tool_names)} tools")

                agentic_tools = [t for t in tool_names if t.startswith("agentic_")]
                print(f"  Agentic tools: {len(agentic_tools)}")

                if not agentic_tools:
                    print("❌ No agentic tools found!")
                    return 1

                # Run tests
                print("\n" + "=" * 72)
                print("Running Agentic Tool Tests (HTTP/SSE transport)")
                print("=" * 72)

                for idx, (tool_name, query, timeout) in enumerate(AGENTIC_TESTS, 1):
                    print(f"\n[{idx}/{len(AGENTIC_TESTS)}] Testing: {tool_name}")
                    print(f"  Query: {query[:60]}...")
                    print(f"  Timeout: {timeout}s")

                    # Create log file for this test
                    timestamp = datetime.now().strftime("%Y%m%d_%H%M%S")
                    log_file = TEST_OUTPUT_DIR / f"{idx:02d}_{tool_name}_{timestamp}.log"

                    start_time = asyncio.get_event_loop().time()

                    # Write input to log file
                    with open(log_file, "w", encoding="utf-8") as f:
                        f.write("=" * 80 + "\n")
                        f.write(f"Test: {tool_name}\n")
                        f.write(f"Timestamp: {timestamp}\n")
                        f.write(f"Timeout: {timeout}s\n")
                        f.write("=" * 80 + "\n\n")
                        f.write("INPUT QUERY:\n")
                        f.write("-" * 80 + "\n")
                        f.write(query + "\n")
                        f.write("-" * 80 + "\n\n")

                    try:
                        # Call tool with timeout
                        result = await asyncio.wait_for(
                            session.call_tool(tool_name, arguments={"query": query}),
                            timeout=timeout
                        )

                        duration = asyncio.get_event_loop().time() - start_time

                        # Parse result
                        if result.content and len(result.content) > 0:
                            text_content = result.content[0].text

                            # Write output to log file
                            with open(log_file, "a", encoding="utf-8") as f:
                                f.write("OUTPUT:\n")
                                f.write("-" * 80 + "\n")
                                f.write(text_content + "\n")
                                f.write("-" * 80 + "\n\n")
                                f.write(f"Duration: {duration:.1f}s\n")
                                f.write(f"Status: SUCCESS\n")

                            try:
                                data = json.loads(text_content)
                                steps = data.get("total_steps", 0)
                                final_answer = data.get("final_answer", "")

                                print(f"  ✅ SUCCESS in {duration:.1f}s ({steps} steps)")
                                print(f"  📝 Log saved: {log_file.name}")
                                if final_answer:
                                    preview = final_answer[:100].replace('\n', ' ')
                                    print(f"     {preview}...")

                                results.append({
                                    "test": tool_name,
                                    "success": True,
                                    "duration": duration,
                                    "steps": steps
                                })
                            except json.JSONDecodeError:
                                print(f"  ✅ SUCCESS in {duration:.1f}s (non-JSON response)")
                                print(f"  📝 Log saved: {log_file.name}")
                                results.append({
                                    "test": tool_name,
                                    "success": True,
                                    "duration": duration,
                                    "steps": 0
                                })
                        else:
                            with open(log_file, "a", encoding="utf-8") as f:
                                f.write("OUTPUT:\n")
                                f.write("-" * 80 + "\n")
                                f.write("(Empty result)\n")
                                f.write("-" * 80 + "\n\n")
                                f.write(f"Duration: {duration:.1f}s\n")
                                f.write(f"Status: FAILED (empty result)\n")

                            print(f"  ❌ FAILED: Empty result")
                            print(f"  📝 Log saved: {log_file.name}")
                            results.append({
                                "test": tool_name,
                                "success": False,
                                "duration": duration,
                                "steps": 0
                            })

                    except asyncio.TimeoutError:
                        duration = timeout
                        with open(log_file, "a", encoding="utf-8") as f:
                            f.write("OUTPUT:\n")
                            f.write("-" * 80 + "\n")
                            f.write(f"TIMEOUT after {timeout}s\n")
                            f.write("-" * 80 + "\n\n")
                            f.write(f"Duration: {duration:.1f}s\n")
                            f.write(f"Status: TIMEOUT\n")

                        print(f"  ❌ TIMEOUT after {timeout}s")
                        print(f"  📝 Log saved: {log_file.name}")
                        results.append({
                            "test": tool_name,
                            "success": False,
                            "duration": duration,
                            "steps": 0
                        })

                    except Exception as e:
                        duration = asyncio.get_event_loop().time() - start_time
                        import traceback
                        error_trace = traceback.format_exc()

                        with open(log_file, "a", encoding="utf-8") as f:
                            f.write("OUTPUT:\n")
                            f.write("-" * 80 + "\n")
                            f.write(f"ERROR: {e}\n\n")
                            f.write("Full traceback:\n")
                            f.write(error_trace)
                            f.write("-" * 80 + "\n\n")
                            f.write(f"Duration: {duration:.1f}s\n")
                            f.write(f"Status: ERROR\n")

                        print(f"  ❌ ERROR: {e}")
                        print(f"  📝 Log saved: {log_file.name}")
                        print(f"\n  📋 Full error details:")
                        traceback.print_exc()
                        results.append({
                            "test": tool_name,
                            "success": False,
                            "duration": duration,
                            "steps": 0
                        })

    except Exception as e:
        print(f"\n❌ Failed to connect via SSE: {e}")
        print(f"\n📋 Full error details:")
        import traceback
        traceback.print_exc()
        print(f"\n⚠️  Troubleshooting:")
        print(f"   - Is the server running? Check: ps aux | grep codegraph")
        print(f"   - Verify server is listening on port {HTTP_PORT}")
        print(f"   - Check server logs for errors")
        print(f"\nStart server with:")
        print(f"  ./target/release/codegraph start http --host {HTTP_HOST} --port {HTTP_PORT}")
        print(f"\nIf you need detailed error info, run with RUST_LOG=debug")
        return 1

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
    print(f"\n📁 Detailed logs saved to: {TEST_OUTPUT_DIR}")
    print("=" * 72)

    return 0 if passed == total else 1


async def main():
    """Main entry point."""
    print("CodeGraph Agentic MCP Tools Test")
    print("Using official MCP Python SDK")
    print_config()

    if TRANSPORT == "http":
        return await run_http_tests()
    elif TRANSPORT == "stdio":
        return await run_stdio_tests()
    else:
        print(f"\n❌ Unknown transport: {TRANSPORT}")
        print("   Set MCP_TRANSPORT to 'http' or 'stdio'")
        return 1


if __name__ == "__main__":
    try:
        sys.exit(asyncio.run(main()))
    except KeyboardInterrupt:
        print("\n\n⚠️  Interrupted")
        sys.exit(1)
