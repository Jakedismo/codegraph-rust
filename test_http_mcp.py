#!/usr/bin/env python3
"""
Test agentic tools via HTTP using official MCP Python SDK.
Properly follows MCP protocol with streamable HTTP transport.
"""

import asyncio
import json
import os
import sys
from pathlib import Path
from datetime import datetime

# Load .env
try:
    from dotenv import load_dotenv
    env_path = Path(__file__).resolve().parent / ".env"
    if env_path.exists():
        load_dotenv(env_path)
except ImportError:
    pass

# Configuration
HTTP_HOST = os.environ.get("CODEGRAPH_HTTP_HOST", "127.0.0.1")
HTTP_PORT = os.environ.get("CODEGRAPH_HTTP_PORT", "3003")
SERVER_URL = f"http://{HTTP_HOST}:{HTTP_PORT}/mcp"

# Test cases (same as STDIO version, extended timeouts)
AGENTIC_TESTS = [
    ("agentic_code_search", "How is configuration loaded in this codebase? Find all config loading mechanisms.", 300),
    ("agentic_dependency_analysis", "Analyze the dependency chain for the AgenticOrchestrator. What does it depend on?", 300),
    ("agentic_call_chain_analysis", "Trace the call chain from execute_agentic_workflow to the graph analysis tools", 300),
    ("agentic_architecture_analysis", "Analyze the architecture of the MCP server. Find coupling metrics and hub nodes.", 300),
    ("agentic_api_surface_analysis", "What is the public API surface of the GraphToolExecutor?", 300),
    ("agentic_context_builder", "Gather comprehensive context about the tier-aware prompt selection system", 300),
    ("agentic_semantic_question", "How does the LRU cache work in GraphToolExecutor? What gets cached and when?", 300),
]

async def run_tests():
    from mcp import ClientSession
    from mcp.client.streamable_http import streamablehttp_client

    print("\n" + "=" * 72)
    print("CodeGraph HTTP Agentic Tools Test (Official MCP SDK)")
    print("=" * 72)
    print(f"Server: {SERVER_URL}")
    print("=" * 72 + "\n")

    try:
        async with streamablehttp_client(SERVER_URL) as (read_stream, write_stream, _):
            async with ClientSession(read_stream, write_stream) as session:
                # Initialize
                print("Initializing MCP connection...")
                await session.initialize()
                print("✓ MCP connection initialized\n")

                results = []

                # Create output directory
                os.makedirs("test_output_http", exist_ok=True)

                for i, (tool_name, query, timeout) in enumerate(AGENTIC_TESTS, 1):
                    print(f"{'=' * 72}")
                    print(f"Testing: {tool_name}")
                    print(f"Query: {query}")
                    print(f"Timeout: {timeout}s")
                    print('=' * 72)

                    start_time = asyncio.get_event_loop().time()
                    result_text = None
                    structured_output = None
                    file_locations = []
                    success = False

                    try:
                        # Call tool with timeout
                        result = await asyncio.wait_for(
                            session.call_tool(tool_name, {"query": query}),
                            timeout=timeout
                        )

                        # Parse result
                        if result and len(result.content) > 0:
                            result_text = result.content[0].text
                            data = json.loads(result_text)

                            if "structured_output" in data:
                                structured_output = data["structured_output"]
                                success = True

                                # Extract file locations
                                for field in ['components', 'hub_nodes', 'evidence', 'core_components']:
                                    if field in structured_output:
                                        for item in structured_output[field]:
                                            if isinstance(item, dict) and 'file_path' in item:
                                                file_locations.append(item)

                        duration = asyncio.get_event_loop().time() - start_time

                        if success:
                            steps = data.get("steps_taken", "?")
                            print(f"\n✅ SUCCESS in {duration:.1f}s ({steps} steps)")
                            if structured_output:
                                print(f"   📊 Structured Output: ✅ PRESENT")
                                if file_locations:
                                    print(f"   📁 File Locations: {len(file_locations)}")
                                    for loc in file_locations[:3]:
                                        line = f":{loc['line_number']}" if loc.get('line_number') else ""
                                        print(f"      - {loc['name']} in {loc['file_path']}{line}")
                                    if len(file_locations) > 3:
                                        print(f"      ... and {len(file_locations) - 3} more")

                        results.append({
                            "test": tool_name,
                            "success": success,
                            "files": len(file_locations)
                        })

                    except asyncio.TimeoutError:
                        duration = asyncio.get_event_loop().time() - start_time
                        print(f"\n❌ TIMEOUT after {duration:.1f}s")
                        results.append({"test": tool_name, "success": False, "files": 0})
                    except Exception as e:
                        duration = asyncio.get_event_loop().time() - start_time
                        print(f"\n❌ ERROR: {e}")
                        results.append({"test": tool_name, "success": False, "files": 0})

                    # Write log file
                    try:
                        timestamp = datetime.now().strftime("%Y%m%d_%H%M%S")
                        log_filename = f"test_output_http/{str(i).zfill(2)}_{tool_name}_{timestamp}.log"

                        with open(log_filename, "w") as f:
                            f.write("=" * 80 + "\n")
                            f.write(f"Test: {tool_name}\n")
                            f.write(f"Transport: HTTP (MCP SDK)\n")
                            f.write(f"Timestamp: {timestamp}\n")
                            f.write(f"Timeout: {timeout}s\n")
                            f.write("=" * 80 + "\n\n")

                            f.write("INPUT QUERY:\n")
                            f.write("-" * 80 + "\n")
                            f.write(f"{query}\n")
                            f.write("-" * 80 + "\n\n")

                            f.write("OUTPUT:\n")
                            f.write("-" * 80 + "\n")

                            if structured_output:
                                f.write(json.dumps(structured_output, indent=2))
                                f.write("\n\n")
                                f.write("FILE LOCATIONS EXTRACTED:\n")
                                f.write("-" * 80 + "\n")
                                for loc in file_locations:
                                    line_info = f":{loc['line_number']}" if loc.get('line_number') else ""
                                    f.write(f"  {loc['name']} in {loc['file_path']}{line_info}\n")
                            elif result_text:
                                f.write(result_text)
                            else:
                                f.write("(No result received)\n")

                            f.write("-" * 80 + "\n\n")
                            f.write(f"Duration: {duration:.1f}s\n")
                            f.write(f"Status: {'SUCCESS' if success else 'FAILED'}\n")

                        print(f"   💾 Log saved: {log_filename}")
                    except Exception as e:
                        print(f"   ⚠️  Failed to write log: {e}")

                # Summary
                print("\n" + "=" * 72)
                print("Test Summary")
                print("=" * 72)
                passed = sum(1 for r in results if r["success"])
                total_files = sum(r["files"] for r in results)
                print(f"Total: {passed}/{len(results)} passed")
                print(f"File locations found: {total_files}")
                print("=" * 72)

                return 0 if passed == len(results) else 1

    except Exception as e:
        print(f"\n❌ Failed to connect to server: {e}")
        print(f"\nMake sure server is running:")
        print(f"  codegraph start http --port {HTTP_PORT}")
        return 1

if __name__ == "__main__":
    sys.exit(asyncio.run(run_tests()))
