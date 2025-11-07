# Testing CodeGraph MCP Server

This guide explains how to test the CodeGraph MCP server after indexing your project.

## Prerequisites

1. **Python 3.8+** with pip
2. **SurrealDB** running on port 3004
3. **CodeGraph binary** built and ready
4. **Project indexed** with `codegraph index`

## Setup

### 1. Install Python Dependencies

```bash
pip install -r requirements-test.txt
```

This installs `python-dotenv` for loading `.env` configuration.

### 2. Verify Your .env Configuration

Make sure your `.env` file has the necessary settings:

```bash
# LLM Provider Configuration
CODEGRAPH_LLM_PROVIDER=openai          # or anthropic, ollama, lmstudio
CODEGRAPH_MODEL=gpt-5-codex            # your model

# API Keys (as needed)
OPENAI_API_KEY=sk-...                  # if using OpenAI
ANTHROPIC_API_KEY=sk-ant-...           # if using Anthropic

# Embedding Provider
CODEGRAPH_EMBEDDING_PROVIDER=jina      # or ollama, openai
JINA_API_KEY=jina_...                  # if using Jina

# Optional: Ollama settings
CODEGRAPH_OLLAMA_URL=http://localhost:11434
```

### 3. Index Your Project

Before testing, make sure your codebase is indexed:

```bash
# Index the current project
codegraph index .

# Or index a specific path
codegraph index /path/to/your/code
```

This creates the `.codegraph/` directory with the indexed data.

## Running Tests

### Basic Usage

Simply run the test script:

```bash
python3 test_mcp_tools.py
```

The script will:
1. ✅ Load configuration from `.env`
2. ✅ Display the configuration being used
3. ✅ Start the MCP server
4. ✅ Run MCP handshake (initialize + notifications)
5. ✅ Execute 6 test tool calls
6. ✅ Automatically extract node UUIDs for graph operations

### What Gets Tested

The script tests the following MCP tools:

1. **`search`** - Semantic code search
   - Query: "configuration management"
   - Tests basic semantic search functionality

2. **`vector_search`** - Vector similarity search
   - Query: "async function implementation"
   - Returns code nodes with embeddings
   - UUID extracted for graph operations

3. **`graph_neighbors`** - Graph traversal
   - Uses UUID from vector_search
   - Finds connected nodes in the code graph

4. **`graph_traverse`** - Deep graph exploration
   - Uses UUID from vector_search
   - Traverses 2 levels deep

5. **`semantic_intelligence`** - LLM-powered analysis
   - Query: "How is configuration loaded from .env files?"
   - Tests LLM integration with your configured provider

6. **`impact_analysis`** - Change impact prediction
   - Target: `load` function in `config_manager.rs`
   - Analyzes what would be affected by changes

### Expected Output

```
✓ Loaded configuration from /path/to/.env

========================================================================
CodeGraph Configuration:
========================================================================
  LLM Provider: openai
  LLM Model: gpt-5-codex
  Embedding Provider: jina
  Protocol Version: 2025-06-18
========================================================================

Starting CodeGraph MCP Server...

→ {"jsonrpc":"2.0","id":1,"method":"initialize",...}
========================================================================
[Server response with capabilities]

### 1. search (semantic search) ###
→ {"jsonrpc":"2.0","method":"tools/call","params":{"name":"search"...
========================================================================
[Search results]

### 2. vector_search ###
→ {"jsonrpc":"2.0","method":"tools/call","params":{"name":"vector_search"...
========================================================================
✓ Detected node UUID: abc123...
[Vector search results]

### 3. graph_neighbors (auto-fill node UUID) ###
Using node UUID from vector_search: abc123...
→ {"jsonrpc":"2.0","method":"tools/call","params":{"name":"graph_neighbors"...
========================================================================
[Graph neighbors]

... (and so on)

✅ Finished all tests.
```

## Customizing Tests

### Override Configuration

You can override `.env` settings with environment variables:

```bash
# Use a different model
CODEGRAPH_MODEL=gpt-4o python3 test_mcp_tools.py

# Use local Ollama
CODEGRAPH_LLM_PROVIDER=ollama \
CODEGRAPH_MODEL=qwen2.5-coder:14b \
python3 test_mcp_tools.py
```

### Use Custom Binary

Point to a specific codegraph binary:

```bash
# Use debug build
CODEGRAPH_BIN=./target/debug/codegraph python3 test_mcp_tools.py

# Use release build
CODEGRAPH_BIN=./target/release/codegraph python3 test_mcp_tools.py

# Use custom command
CODEGRAPH_CMD="cargo run -p codegraph-mcp --bin codegraph --" python3 test_mcp_tools.py
```

### Modify Test Queries

Edit `test_mcp_tools.py` to change the test queries:

```python
TESTS = [
    ("1. search (semantic search)", {
        "jsonrpc": "2.0", "method": "tools/call",
        "params": {"name": "search", "arguments": {
            "query": "YOUR QUERY HERE",  # <-- Change this
            "limit": 5                    # <-- Or this
        }},
        "id": 101
    }),
    # ... more tests
]
```

## Troubleshooting

### Error: "python-dotenv not installed"

```bash
pip install python-dotenv
```

### Error: "No .env file found"

Create a `.env` file in the project root with your configuration:

```bash
cat > .env << 'EOF'
CODEGRAPH_LLM_PROVIDER=openai
CODEGRAPH_MODEL=gpt-4o
OPENAI_API_KEY=sk-your-key-here
EOF
```

### Error: "No UUID found in vector_search output"

This means the indexed database doesn't have nodes yet. Index your project first:

```bash
codegraph index .
```

### Error: Connection refused

Make sure SurrealDB is running:

```bash
# Start SurrealDB
surreal start --bind 0.0.0.0:3004 --user root --pass root file://data/surreal.db
```

### Server startup fails

Check if the codegraph binary is available:

```bash
# Test if binary exists
which codegraph

# Or use debug build
CODEGRAPH_BIN=./target/debug/codegraph python3 test_mcp_tools.py
```

### Slow semantic_intelligence responses

The `semantic_intelligence` tool uses your configured LLM provider. Response time depends on:
- LLM provider speed (cloud vs local)
- Model size (larger = slower but better)
- Context size (more context = slower)

Reduce `max_context_tokens` in the test for faster responses:

```python
("5. semantic_intelligence", {
    "params": {"name": "semantic_intelligence", "arguments": {
        "query": "...",
        "max_context_tokens": 5000  # Reduced from 10000
    }},
    "id": 105
}),
```

## Advanced Testing

### Test with Different Providers

```bash
# Test with OpenAI
CODEGRAPH_LLM_PROVIDER=openai python3 test_mcp_tools.py

# Test with Anthropic
CODEGRAPH_LLM_PROVIDER=anthropic \
CODEGRAPH_MODEL=claude-3-5-sonnet-20241022 \
python3 test_mcp_tools.py

# Test with local Ollama
CODEGRAPH_LLM_PROVIDER=ollama \
CODEGRAPH_MODEL=qwen2.5-coder:14b \
python3 test_mcp_tools.py
```

### Capture Output for Analysis

```bash
# Save full output
python3 test_mcp_tools.py 2>&1 | tee test_output.log

# Filter for errors only
python3 test_mcp_tools.py 2>&1 | grep -i error

# Show only test summaries
python3 test_mcp_tools.py 2>&1 | grep "^###"
```

### Debug Mode

For more verbose output, modify the script to show raw JSON:

```python
# In test_mcp_tools.py, change:
def send(proc, obj, wait=2.0, show=True):  # Keep show=True for debugging
```

## Integration with CI/CD

You can use this script in CI/CD pipelines:

```yaml
# .github/workflows/test-mcp.yml
name: Test MCP Server

on: [push, pull_request]

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v2

      - name: Install dependencies
        run: pip install -r requirements-test.txt

      - name: Start SurrealDB
        run: |
          surreal start --bind 0.0.0.0:3004 &
          sleep 2

      - name: Build CodeGraph
        run: cargo build --release -p codegraph-mcp

      - name: Index test project
        run: ./target/release/codegraph index .

      - name: Run MCP tests
        env:
          CODEGRAPH_LLM_PROVIDER: ollama
          CODEGRAPH_MODEL: qwen2.5-coder:7b
        run: python3 test_mcp_tools.py
```

## See Also

- [SETUP_VERIFICATION.md](SETUP_VERIFICATION.md) - Setup verification guide
- [schema/README.md](schema/README.md) - Database schema documentation
- [CHANGES_SUMMARY.md](CHANGES_SUMMARY.md) - Recent changes summary
