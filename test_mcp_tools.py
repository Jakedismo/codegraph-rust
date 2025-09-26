#!/usr/bin/env python3
# test_mcp_tools_stdio.py
#
# Automatic tester for CodeGraph MCP tools using `codegraph stdio-serve`.
# - Sends MCP initialize + notifications/initialized first (handshake)
# - Then runs the 7 tool calls
# - Auto-detects a node UUID from vector_search for graph_neighbors
#
# Usage:
#   CODEGRAPH_MODEL="hf.co/unsloth/Qwen2.5-Coder-14B-Instruct-128K-GGUF:Q4_K_M" \
#   python3 test_mcp_tools_stdio.py
#
# Optional env:
#   MCP_PROTOCOL_VERSION="2025-06-18"  # default below

import json, os, re, select, signal, subprocess, sys, time
import shlex
from pathlib import Path

MODEL_DEFAULT = "hf.co/unsloth/Qwen2.5-Coder-14B-Instruct-128K-GGUF:Q4_K_M"
PROTO_DEFAULT = os.environ.get("MCP_PROTOCOL_VERSION", "2025-06-18")

DEFAULT_FEATURES = (
    "ai-enhanced,qwen-integration,embeddings,faiss,"
    "embeddings-ollama,codegraph-vector/onnx"
)

UUID_RE = re.compile(r"[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[1-5][0-9a-fA-F]{3}-[89abAB][0-9a-fA-F]{3}-[0-9a-fA-F]{12}")

TESTS = [
    ("1. pattern_detection", {
        "jsonrpc": "2.0", "method": "tools/call",
        "params": {"name": "pattern_detection","arguments": {"_unused": None}}, "id": 101
    }),
    ("2. vector_search", {
        "jsonrpc": "2.0", "method": "tools/call",
        "params": {"name": "vector_search","arguments": {"query": "async function implementation","limit": 3}}, "id": 102
    }),
    ("3. enhanced_search", {
        "jsonrpc": "2.0", "method": "tools/call",
        "params": {"name": "enhanced_search","arguments": {"query": "RAG engine streaming implementation","limit": 3}}, "id": 103
    }),
    ("4. codebase_qa", {
        "jsonrpc": "2.0", "method": "tools/call",
        "params": {"name": "codebase_qa","arguments": {"question": "How does the RAG engine handle streaming responses?","max_results": 3,"streaming": False}}, "id": 104
    }),
    ("5. graph_neighbors (auto-fill node UUID)", {
        "jsonrpc": "2.0", "method": "tools/call",
        "params": {"name": "graph_neighbors","arguments": {"node": "REPLACE_WITH_NODE_UUID","limit": 5}}, "id": 105
    }),
    ("6. impact_analysis", {
        "jsonrpc": "2.0", "method": "tools/call",
        "params": {"name": "impact_analysis","arguments": {"target_function": "analyze_codebase","file_path": "crates/codegraph-mcp/src/qwen.rs","change_type": "modify"}}, "id": 106
    }),
    ("7. code_documentation", {
        "jsonrpc": "2.0", "method": "tools/call",
        "params": {"name": "code_documentation","arguments": {"target_name": "QwenClient","file_path": "crates/codegraph-mcp/src/qwen.rs","style": "concise"}}, "id": 107
    }),
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

def send(proc, obj, wait=2.0, show=True):
    """Send a JSON-RPC message (one line) and optionally wait for output."""
    s = json.dumps(obj, ensure_ascii=False)
    if show:
        print("\n→", s)
        print("="*72)
    proc.stdin.write(s + "\n")
    proc.stdin.flush()
    return drain(proc, wait) if wait > 0 else ""

def extract_uuid(text: str):
    m = UUID_RE.search(text or "")
    return m.group(0) if m else None

def ensure_codegraph_model():
    if "CODEGRAPH_MODEL" not in os.environ:
        os.environ["CODEGRAPH_MODEL"] = MODEL_DEFAULT

def resolve_codegraph_command():
    """Determine which command should launch the CodeGraph MCP server."""
    # Allow full override with CODEGRAPH_CMD (space-separated command string).
    if cmd := os.environ.get("CODEGRAPH_CMD"):
        return shlex.split(cmd)

    # Allow pointing directly to a binary path via CODEGRAPH_BIN.
    if binary := os.environ.get("CODEGRAPH_BIN"):
        return [binary]

    # Prefer locally-built binary if available.
    repo_root = Path(__file__).resolve().parent
    local_bin = repo_root / "target" / "debug" / "codegraph"
    if local_bin.exists():
        return [str(local_bin)]

    # Fallback to cargo run with all required features.
    return [
        "cargo",
        "run",
        "--quiet",
        "-p",
        "codegraph-mcp",
        "--bin",
        "codegraph",
        "--features",
        DEFAULT_FEATURES,
        "--",
    ]


def run():
    ensure_codegraph_model()

    base_cmd = resolve_codegraph_command()
    launch_cmd = base_cmd + ["start", "stdio"]

    # Start server
    proc = subprocess.Popen(
        launch_cmd,
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True,
        bufsize=1,
    )

    # Give it a moment to boot and print banner
    time.sleep(0.6)
    drain(proc, 0.8)

    # ── MCP handshake ─────────────────────────────────────────────────────────
    # 1) initialize
    init_req = {
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": PROTO_DEFAULT,
            "clientInfo": {"name": "codegraph-auto-tester", "version": "0.1.0"},
            # Minimal capabilities; expand if your server requires specifics
            "capabilities": {}
        }
    }
    init_out = send(proc, init_req, wait=2.5)
    if "error" in init_out.lower():
        print("\n❌ Initialization appears to have failed. Output above.")
        try:
            proc.terminate()
        except Exception:
            pass
        sys.exit(1)

    # 2) notifications/initialized (notification; no id)
    inited_note = {
        "jsonrpc": "2.0",
        "method": "notifications/initialized",
        "params": {}
    }
    send(proc, inited_note, wait=0.6)

    # (Optional) servers sometimes emit capability info now
    drain(proc, 0.6)

    # ── Run tests ─────────────────────────────────────────────────────────────
    vec2_output = ""
    for title, payload in TESTS:
        print(f"\n### {title} ###")
        out = send(proc, payload, wait=3.0)
        if payload["id"] == 102:
            vec2_output = out

        if payload["id"] == 105:  # graph_neighbors placeholder
            if "REPLACE_WITH_NODE_UUID" in json.dumps(payload):
                node = extract_uuid(vec2_output)
                if node:
                    print(f"Auto-detected node UUID from vector_search: {node}")
                    payload = {
                        **payload,
                        "params": {
                            **payload["params"],
                            "arguments": {
                                **payload["params"]["arguments"],
                                "node": node
                            }
                        }
                    }
                    send(proc, payload, wait=3.0)
                else:
                    print("⚠️ No UUID found in vector_search output. Skipping graph_neighbors actual call.")

    # Graceful shutdown
    try:
        proc.send_signal(signal.SIGINT)
        proc.wait(timeout=1.5)
    except Exception:
        try:
            proc.terminate()
        except Exception:
            pass

    print("\n✅ Finished all tests.")

if __name__ == "__main__":
    try:
        run()
    except KeyboardInterrupt:
        pass
