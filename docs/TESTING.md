# Testing CodeGraph MCP Server

This guide explains how to test the CodeGraph MCP server after indexing your project.

## Prerequisites

1. **Python 3.8+** with pip
2. **CodeGraph binary** built and ready
3. **Project indexed** with `codegraph index`
4. **Optional**: SurrealDB (only if using cloud-surrealdb feature)
5. **Optional**: API keys for cloud providers (Jina AI, OpenAI, Anthropic)

## Setup

### 1. Install Python Dependencies

```bash
pip install -r requirements-test.txt
```

This installs `python-dotenv` for loading `.env` configuration.

### 2. Verify Your .env Configuration

Make sure your `.env` file has the necessary settings:

**For Local-Only Setup (ONNX + Ollama):**
```bash
# LLM Provider Configuration
CODEGRAPH_LLM_PROVIDER=ollama
CODEGRAPH_MODEL=qwen2.5-coder:14b
CODEGRAPH_OLLAMA_URL=http://localhost:11434

# Embedding Provider (local)
CODEGRAPH_EMBEDDING_PROVIDER=onnx
```

**For Cloud Setup (Jina AI + Anthropic):**
```bash
# LLM Provider Configuration
CODEGRAPH_LLM_PROVIDER=anthropic
CODEGRAPH_MODEL=claude-haiku
ANTHROPIC_API_KEY=sk-ant-...

# Embedding Provider (Jina AI)
CODEGRAPH_EMBEDDING_PROVIDER=jina
JINA_API_KEY=jina_...
JINA_EMBEDDING_MODEL=jina-embeddings-v4
JINA_EMBEDDING_DIMENSION=2048
JINA_RERANKING_ENABLED=true
JINA_RERANKING_MODEL=jina-reranker-v3
```

**For OpenAI Setup:**
```bash
# LLM Provider Configuration
CODEGRAPH_LLM_PROVIDER=openai
CODEGRAPH_MODEL=gpt-5-codex
OPENAI_API_KEY=sk-...

# Embedding Provider (OpenAI)
CODEGRAPH_EMBEDDING_PROVIDER=openai
OPENAI_EMBEDDING_MODEL=text-embedding-3-small
OPENAI_EMBEDDING_DIMENSION=1536
```

**For xAI Grok Setup (2M context, super cheap):**
```bash
# LLM Provider Configuration
CODEGRAPH_LLM_PROVIDER=xai
CODEGRAPH_MODEL=grok-4-fast
XAI_API_KEY=xai-...

# Embedding Provider (OpenAI or Jina)
CODEGRAPH_EMBEDDING_PROVIDER=openai
OPENAI_EMBEDDING_MODEL=text-embedding-3-small
OPENAI_EMBEDDING_DIMENSION=1536
```

**For SurrealDB HNSW Backend:**
```bash
# Vector Store Backend
SURREALDB_CONNECTION=ws://localhost:8000
SURREALDB_NAMESPACE=codegraph
SURREALDB_DATABASE=production
```

### 3. Build CodeGraph with Required Features

Build CodeGraph with the features you want to test:

```bash
# Local-only (ONNX + Ollama + SurrealDB)
cargo build --release --features "onnx,ollama,codegraph-graph/surrealdb"

# Cloud features (Jina AI + SurrealDB)
cargo build --release --features "cloud-jina,cloud-surrealdb,codegraph-graph/surrealdb"

# Full build (all features)
cargo build --release --features "all-cloud-providers,onnx,ollama,cloud,codegraph-graph/surrealdb"
```

### 4. Index Your Project

Before testing, make sure your codebase is indexed:

```bash
# Index the current project
./target/release/codegraph index .

# Or index a specific path
./target/release/codegraph index /path/to/your/code
```

This creates the `.codegraph/` directory with the indexed data.

## Running Tests

### Basic Usage

Simply run the test script:

```bash
python3 test_mcp_tools.py
```

**The script will automatically:**
1. ✅ Load configuration from `.env` file in project root
2. ✅ Display comprehensive configuration (LLM provider, model, embedding provider, cloud features)
3. ✅ Detect and use local binary (`target/release/codegraph` or `target/debug/codegraph`)
4. ✅ Fall back to `cargo run` with appropriate feature flags if no binary found
5. ✅ Start the MCP server with proper stdio communication
6. ✅ Run MCP handshake (initialize + notifications)
7. ✅ Execute 6 test tool calls (search, vector_search, graph operations, semantic intelligence, impact analysis)
8. ✅ Automatically extract node UUIDs from search results for graph operations

**Recent improvements:**
- Updated default model to `qwen2.5-coder:14b` (better for local testing)
- Updated feature flags to use current features (`onnx,ollama,codegraph-graph/surrealdb` instead of deprecated `faiss`)
- Added cloud configuration display (shows Jina AI and SurrealDB settings if configured)
- Prefers release binary over debug binary for better performance

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
  LLM Provider: anthropic
  LLM Model: claude-haiku
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

### Error: Connection refused (SurrealDB)

Only needed if you're testing with the `cloud-surrealdb` feature:

```bash
# Start SurrealDB
surreal start --bind 0.0.0.0:8000 --user root --pass root file://data/surreal.db
```

If you're not using SurrealDB, build without the `cloud-surrealdb` feature.

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
CODEGRAPH_LLM_PROVIDER=openai \
CODEGRAPH_MODEL=gpt-5-codex \
python3 test_mcp_tools.py

# Test with Anthropic
CODEGRAPH_LLM_PROVIDER=anthropic \
CODEGRAPH_MODEL=claude-haiku \
python3 test_mcp_tools.py

# Test with local Ollama
CODEGRAPH_LLM_PROVIDER=ollama \
CODEGRAPH_MODEL=qwen2.5-coder:14b \
python3 test_mcp_tools.py

# Test with xAI Grok (2M context window!)
CODEGRAPH_LLM_PROVIDER=xai \
CODEGRAPH_MODEL=grok-4-fast \
python3 test_mcp_tools.py

# Test with Jina AI embeddings
CODEGRAPH_EMBEDDING_PROVIDER=jina \
JINA_EMBEDDING_MODEL=jina-embeddings-v4 \
JINA_EMBEDDING_DIMENSION=2048 \
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

## Testing NAPI Bindings

### Prerequisites

```bash
# Install Node.js dependencies
cd crates/codegraph-napi
npm install

# Build with desired features
npm run build  # Default: local features only
# Or: npm run build -- --features full
```

### Quick Test

```typescript
// test.mjs
import { semanticSearch, getCloudConfig, getEmbeddingStats } from './index.js';

// Test search
const results = await semanticSearch('configuration management', {
  limit: 5,
  useCloud: false  // Set to true if cloud features enabled
});
console.log(`Found ${results.totalCount} results`);

// Test cloud config
const config = await getCloudConfig();
console.log('Cloud config:', config);

// Test embedding stats
const stats = await getEmbeddingStats();
console.log('Embedding stats:', stats);
```

```bash
# Run test
node test.mjs
```

### TypeScript Integration Test

```typescript
// test.ts
import { semanticSearch, SearchOptions, DualModeSearchResult } from 'codegraph-napi';

async function testSearch() {
  const options: SearchOptions = {
    limit: 10,
    useCloud: true,
    reranking: true
  };

  const results: DualModeSearchResult = await semanticSearch(
    'find authentication code',
    options
  );

  console.log(`Search mode: ${results.modeUsed}`);
  console.log(`Found ${results.totalCount} results in ${results.searchTimeMs}ms`);

  for (const result of results.results) {
    console.log(`- ${result.name} (score: ${result.score})`);
  }
}

testSearch().catch(console.error);
```

```bash
# Compile and run TypeScript
npx tsx test.ts
```

## See Also

- **[NAPI Bindings README](crates/codegraph-napi/README.md)** - Complete TypeScript integration guide
- **[CHANGELOG.md](CHANGELOG.md)** - Version history with cloud features
- [SETUP_VERIFICATION.md](SETUP_VERIFICATION.md) - Setup verification guide
- [schema/README.md](schema/README.md) - Database schema documentation
