#!/usr/bin/env python3
# test_mcp_tools.py
#
# Automatic tester for CodeGraph MCP tools using `codegraph start stdio`.
# - Loads configuration from .env file automatically
# - Sends MCP initialize + notifications/initialized first (handshake)
# - Then runs 6 tool calls
# - Auto-detects a node UUID from vector_search for graph_neighbors/graph_traverse
#
# Usage:
#   python3 test_mcp_tools.py
#
# The script will automatically load settings from .env in the project root
#
# Optional env overrides:
#   CODEGRAPH_MODEL="..."  # Override model from .env
#   CODEGRAPH_LLM_PROVIDER="..."  # Override provider from .env
#   CODEGRAPH_EMBEDDING_PROVIDER="..."  # Override embedding provider
#   MCP_PROTOCOL_VERSION="2025-06-18"  # default below

import json, os, re, select, signal, subprocess, sys, time
import shlex
from pathlib import Path

# Load .env file if python-dotenv is available
try:
    from dotenv import load_dotenv
    # Find .env file in project root
    env_path = Path(__file__).resolve().parent / ".env"
    if env_path.exists():
        load_dotenv(env_path)
        print(f"✓ Loaded configuration from {env_path}")
    else:
        print(f"⚠️  No .env file found at {env_path}")
except ImportError:
    print("⚠️  python-dotenv not installed. Install with: pip install python-dotenv")
    print("   Falling back to environment variables only")

# Defaults (can be overridden by .env or environment variables)
MODEL_DEFAULT = "qwen2.5-coder:14b"  # For Ollama local testing
PROTO_DEFAULT = os.environ.get("MCP_PROTOCOL_VERSION", "2025-06-18")

# Read configuration from environment (.env loaded above)
LLM_PROVIDER = os.environ.get("CODEGRAPH_LLM_PROVIDER") or os.environ.get("LLM_PROVIDER", "ollama")
LLM_MODEL = os.environ.get("CODEGRAPH_MODEL", MODEL_DEFAULT)
EMBEDDING_PROVIDER = os.environ.get("CODEGRAPH_EMBEDDING_PROVIDER", "onnx")

# Feature flags for building (if using cargo run)
# Update based on your build configuration
DEFAULT_FEATURES = (
    "onnx,ollama,faiss"  # Local-only default
    # For cloud features, use: "cloud-jina,cloud-surrealdb,anthropic,faiss"
    # For full build, use: "all-cloud-providers,onnx,ollama,cloud,faiss"
)

UUID_RE = re.compile(r"[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[1-5][0-9a-fA-F]{3}-[89abAB][0-9a-fA-F]{3}-[0-9a-fA-F]{12}")

TESTS = [
    ("1. search (semantic search)", {
        "jsonrpc": "2.0", "method": "tools/call",
        "params": {"name": "search", "arguments": {"query": "configuration management", "limit": 3}}, "id": 101
    }),
    ("2. vector_search", {
        "jsonrpc": "2.0", "method": "tools/call",
        "params": {"name": "vector_search", "arguments": {"query": "async function implementation", "limit": 3}}, "id": 102
    }),
    ("3. graph_neighbors (auto-fill node UUID)", {
        "jsonrpc": "2.0", "method": "tools/call",
        "params": {"name": "graph_neighbors", "arguments": {"node": "REPLACE_WITH_NODE_UUID", "limit": 5}}, "id": 103
    }),
    ("4. graph_traverse", {
        "jsonrpc": "2.0", "method": "tools/call",
        "params": {"name": "graph_traverse", "arguments": {"start": "REPLACE_WITH_NODE_UUID", "depth": 2, "limit": 10}}, "id": 104
    }),
    ("5. semantic_intelligence", {
        "jsonrpc": "2.0", "method": "tools/call",
        "params": {"name": "semantic_intelligence", "arguments": {"query": "How is configuration loaded from .env files?", "task_type": "semantic_search", "max_context_tokens": 10000}}, "id": 105
    }),
    ("6. impact_analysis", {
        "jsonrpc": "2.0", "method": "tools/call",
        "params": {"name": "impact_analysis", "arguments": {"target_function": "load", "file_path": "crates/codegraph-core/src/config_manager.rs", "change_type": "modify"}}, "id": 106
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
    # Wait indefinitely (in 5-second chunks) until we receive output or the process exits.
    output = ""
    while True:
        chunk = drain(proc, 5.0)
        output += chunk
        if chunk:
            return output
        # If the process exited and nothing new arrived, break to avoid hanging forever.
        if proc.poll() is not None:
            return output

def extract_uuid(text: str):
    m = UUID_RE.search(text or "")
    return m.group(0) if m else None

def ensure_codegraph_model():
    """Ensure CODEGRAPH_MODEL is set, either from .env or default."""
    if "CODEGRAPH_MODEL" not in os.environ:
        os.environ["CODEGRAPH_MODEL"] = LLM_MODEL

    # Print configuration being used
    print("\n" + "="*72)
    print("CodeGraph Configuration:")
    print("="*72)
    print(f"  LLM Provider: {LLM_PROVIDER}")
    print(f"  LLM Model: {os.environ.get('CODEGRAPH_MODEL', 'not set')}")
    print(f"  Embedding Provider: {EMBEDDING_PROVIDER}")

    # Show cloud config if available
    if os.environ.get("JINA_API_KEY"):
        jina_model = os.environ.get("JINA_EMBEDDING_MODEL", "jina-embeddings-v4")
        jina_dim = os.environ.get("JINA_EMBEDDING_DIMENSION", "2048")
        print(f"  Jina Model: {jina_model} ({jina_dim}D)")
        if os.environ.get("JINA_RERANKING_ENABLED", "").lower() == "true":
            reranker = os.environ.get("JINA_RERANKING_MODEL", "jina-reranker-v3")
            print(f"  Jina Reranking: {reranker}")

    if os.environ.get("SURREALDB_CONNECTION"):
        print(f"  SurrealDB: {os.environ.get('SURREALDB_CONNECTION')}")

    print(f"  Protocol Version: {PROTO_DEFAULT}")
    print("="*72 + "\n")

def resolve_codegraph_command():
    """Determine which command should launch the CodeGraph MCP server."""
    # Allow full override with CODEGRAPH_CMD (space-separated command string).
    if cmd := os.environ.get("CODEGRAPH_CMD"):
        return shlex.split(cmd)

    # Allow pointing directly to a binary path via CODEGRAPH_BIN.
    if binary := os.environ.get("CODEGRAPH_BIN"):
        return [binary]

    # Prefer locally-built release binary if available.
    repo_root = Path(__file__).resolve().parent
    release_bin = repo_root / "target" / "release" / "codegraph"
    if release_bin.exists():
        print(f"Using release binary: {release_bin}")
        return [str(release_bin)]

    # Try debug binary
    debug_bin = repo_root / "target" / "debug" / "codegraph"
    if debug_bin.exists():
        print(f"Using debug binary: {debug_bin}")
        return [str(debug_bin)]

    # Fallback to cargo run with all required features.
    print(f"No binary found, using cargo run with features: {DEFAULT_FEATURES}")
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

    try:
        init_msg = json.loads(init_out.strip().splitlines()[-1])
        server_proto = init_msg.get("result", {}).get("protocolVersion")
        if server_proto and server_proto != PROTO_DEFAULT:
            print(
                f"⚠️ Server reported protocolVersion={server_proto} but expected {PROTO_DEFAULT}."
                " Ensure you are running a freshly-built CodeGraph binary with the updated protocol."
            )
    except Exception:
        pass

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
    node_uuid = None

    for title, payload in TESTS:
        print(f"\n### {title} ###")

        # Extract UUID from vector_search for later tests
        if payload["id"] == 102:
            out = send(proc, payload, wait=0)
            vec2_output = out
            node_uuid = extract_uuid(vec2_output)
            if node_uuid:
                print(f"✓ Detected node UUID: {node_uuid}")
            continue

        # Auto-fill node UUID for graph operations
        if payload["id"] in [103, 104]:  # graph_neighbors and graph_traverse
            if not node_uuid:
                print("⚠️ No UUID found in vector_search output. Skipping graph operation.")
                continue
            print(f"Using node UUID from vector_search: {node_uuid}")

            # Replace the placeholder UUID
            args = payload["params"]["arguments"]
            if "node" in args and args["node"] == "REPLACE_WITH_NODE_UUID":
                args["node"] = node_uuid
            if "start" in args and args["start"] == "REPLACE_WITH_NODE_UUID":
                args["start"] = node_uuid

        out = send(proc, payload, wait=0)

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
