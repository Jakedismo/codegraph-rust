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
from datetime import datetime, timedelta

from agentic_test_cases import AGENTIC_TESTS

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


async def run_tests():
    from mcp import ClientSession
    from mcp.client.streamable_http import streamablehttp_client

    print("\n" + "=" * 72)
    print("CodeGraph HTTP Agentic Tools Test (Official MCP SDK)")
    print("Testing 4 Consolidated Agentic Tools")
    print("=" * 72)
    print(f"Server: {SERVER_URL}")
    print("=" * 72 + "\n")

    try:
        # Keep the transport's idle-read budget above the longest case deadline.
        # Otherwise the SDK's default 300-second SSE read limit ends a 600s case early.
        read_timeout = timedelta(seconds=max(case[3] for case in AGENTIC_TESTS) + 5)
        async with streamablehttp_client(
            SERVER_URL, sse_read_timeout=read_timeout
        ) as (read_stream, write_stream, _):
            async with ClientSession(read_stream, write_stream) as session:
                # Initialize
                print("Initializing MCP connection...")
                await session.initialize()
                print("✓ MCP connection initialized\n")

                results = []

                # Create output directory
                os.makedirs("test_output_http", exist_ok=True)

                for i, test_case in enumerate(AGENTIC_TESTS, 1):
                    tool_name, query, focus, timeout = test_case

                    print(f"{'=' * 72}")
                    print(f"Testing: {tool_name}" + (f" (focus={focus})" if focus else ""))
                    print(f"Query: {query}")
                    print(f"Timeout: {timeout}s")
                    print('=' * 72)

                    start_time = asyncio.get_event_loop().time()
                    result_text = None
                    structured_output = None
                    file_locations = []

                    try:
                        # Build parameters
                        params = {"query": query}
                        if focus:
                            params["focus"] = focus

                        # Call tool with timeout
                        result = await asyncio.wait_for(
                            session.call_tool(tool_name, params),
                            timeout=timeout
                        )

                        # Parse result
                        if result and len(result.content) > 0:
                            result_text = result.content[0].text
                            data = json.loads(result_text)

                            # Prefer structured_output if provided
                            if "structured_output" in data:
                                structured_output = data["structured_output"]
                            # Fallback: parse answer if it looks like JSON
                            elif "answer" in data and isinstance(data["answer"], str):
                                try:
                                    structured_output = json.loads(data["answer"])
                                except json.JSONDecodeError:
                                    structured_output = None

                            # Extract file locations from structured output
                            if structured_output:
                                for field in ['components', 'hub_nodes', 'evidence', 'core_components', 'items', 'highlights']:
                                    if field in structured_output:
                                        for item in structured_output[field]:
                                            if isinstance(item, dict) and 'file_path' in item:
                                                file_locations.append(item)

                        duration = asyncio.get_event_loop().time() - start_time

                        steps = data.get("steps_taken", "?") if result_text else "?"
                        print(f"\nℹ️  Completed in {duration:.1f}s ({steps} steps)")
                        if structured_output:
                            print(f"   📊 Structured Output: PRESENT")
                            if file_locations:
                                print(f"   📁 File Locations: {len(file_locations)}")
                                for loc in file_locations[:3]:
                                    line = f":{loc['line_number']}" if loc.get('line_number') else ""
                                    print(f"      - {loc.get('name', loc.get('title', 'unnamed'))} in {loc['file_path']}{line}")
                                if len(file_locations) > 3:
                                    print(f"      ... and {len(file_locations) - 3} more")
                        else:
                            print("   📊 Structured Output: (none parsed)")

                        results.append({
                            "test": tool_name,
                            "focus": focus,
                            "files": len(file_locations),
                            "duration": duration,
                        })

                    except asyncio.TimeoutError:
                        duration = asyncio.get_event_loop().time() - start_time
                        print(f"\n❌ TIMEOUT after {duration:.1f}s")
                        results.append({"test": tool_name, "focus": focus, "files": 0, "duration": duration})
                    except Exception as e:
                        duration = asyncio.get_event_loop().time() - start_time
                        print(f"\n❌ ERROR: {e}")
                        results.append({"test": tool_name, "focus": focus, "files": 0, "duration": duration})

                    # Write log file
                    try:
                        timestamp = datetime.now().strftime("%Y%m%d_%H%M%S")
                        focus_suffix = f"_{focus}" if focus else ""
                        log_filename = f"test_output_http/{str(i).zfill(2)}_{tool_name}{focus_suffix}_{timestamp}.log"

                        with open(log_filename, "w") as f:
                            f.write("=" * 80 + "\n")
                            f.write(f"Test: {tool_name}\n")
                            if focus:
                                f.write(f"Focus: {focus}\n")
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
                                    f.write(f"  {loc.get('name', loc.get('title', 'unnamed'))} in {loc['file_path']}{line_info}\n")
                            elif result_text:
                                f.write(result_text)
                            else:
                                f.write("(No result received)\n")

                            f.write("-" * 80 + "\n\n")
                            f.write(f"Duration: {duration:.1f}s\n")
                            f.write("Status: RECORDED\n")

                        print(f"   💾 Log saved: {log_filename}")
                    except Exception as e:
                        print(f"   ⚠️  Failed to write log: {e}")

                # Summary
                print("\n" + "=" * 72)
                print("Test Summary")
                print("=" * 72)
                total_files = sum(r["files"] for r in results)
                total_time = sum(r["duration"] for r in results)
                print(f"Tests run: {len(results)} | File locations found: {total_files} | Total time: {total_time:.1f}s")
                print("=" * 72)

                return 0

    except Exception as e:
        print(f"\n❌ Failed to connect to server: {e}")
        print(f"\nMake sure server is running:")
        print(f"  codegraph start http --port {HTTP_PORT}")
        return 1

if __name__ == "__main__":
    sys.exit(asyncio.run(run_tests()))
