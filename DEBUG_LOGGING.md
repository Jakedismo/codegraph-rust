# Debug Logging for Agentic Tools

This document explains how to use the debug logging feature to troubleshoot agentic tool execution.

## Quick Start

### 1. Enable Debug Logging

Set the environment variable before starting the MCP server:

```bash
export CODEGRAPH_DEBUG=1
codegraph start http --port 3000
```

Or add to your `.env` file:

```bash
CODEGRAPH_DEBUG=1
```

### 2. Run Your Tests

```bash
python3 test_agentic_tools_http.py
```

### 3. View the Debug Logs

```bash
python3 tools/view_debug_logs.py
```

## What Gets Logged

The debug logger captures:

1. **Agent Execution Start/Finish**
   - Query text
   - Analysis type (code_search, dependency_analysis, etc.)
   - Context tier (Small, Medium, Large, Massive)
   - Success/failure status
   - Error messages

2. **Tool Calls**
   - Tool name (semantic_code_search, get_transitive_dependencies, etc.)
   - Input parameters
   - Output results with summary
   - Result counts and sample data

3. **Reasoning Steps** (if available)
   - Step number
   - Thought process
   - Action taken

## Log File Location

Debug logs are written to:

```
.codegraph/debug/agentic_debug_YYYYMMDD_HHMMSS.jsonl
```

Each line is a JSON object representing one event.

## Configuration

### Custom Debug Directory

```bash
export CODEGRAPH_DEBUG_DIR=/path/to/custom/debug/dir
```

Default: `.codegraph/debug/` in current working directory

## Viewing Logs

### View Latest Log

```bash
python3 tools/view_debug_logs.py
```

### View Specific Log

```bash
python3 tools/view_debug_logs.py .codegraph/debug/agentic_debug_20250101_120000.jsonl
```

### Follow Live (like `tail -f`)

```bash
python3 tools/view_debug_logs.py --follow
```

## Log Format

Each log entry is a JSON object on a single line (JSONL format):

```json
{
  "timestamp": "2025-01-01T12:00:00.123456Z",
  "event": "tool_call_start",
  "tool": "semantic_code_search",
  "parameters": {
    "query": "configuration loading"
  }
}
```

```json
{
  "timestamp": "2025-01-01T12:00:05.678910Z",
  "event": "tool_call_finish",
  "tool": "semantic_code_search",
  "result": {
    "tool": "semantic_code_search",
    "result": [...]
  },
  "result_summary": {
    "type": "array",
    "count": 5,
    "sample": {...}
  }
}
```

## Common Debugging Scenarios

### Tool Returns Empty Results

Look for `tool_call_finish` events:

```
✅ TOOL RESULT: semantic_code_search
  📊 Returned 0 results
  ⚠️  WARNING: Empty result array!
```

**Possible causes:**
- `project_id` mismatch between server and database
- No embeddings in database
- Query threshold too high
- Full-text search indexes missing

### Agent Gets Stuck

Look for repeated reasoning steps or missing tool calls:

```
💭 STEP 1: I need to search for configuration code
  🎬 Action: semantic_code_search

💭 STEP 2: The search returned no results, trying different query
  🎬 Action: semantic_code_search

💭 STEP 3: Still no results, unable to proceed
```

**Possible causes:**
- Tools not returning expected data
- Agent prompt doesn't handle empty results
- Context tier too small for the task

### Tool Parameters Wrong

Look at `tool_call_start` events:

```
🔧 TOOL CALL: semantic_code_search
  📥 Parameters:
     query: "config"
     limit: 10
```

**Check:**
- Are parameters being passed correctly?
- Is `project_id` being inferred from PWD?
- Are embeddings being generated with correct dimension?

## Raw Log Analysis

For advanced debugging, you can parse the JSONL directly:

```bash
# Extract all tool results
jq 'select(.event == "tool_call_finish")' .codegraph/debug/agentic_debug_*.jsonl

# Count empty results
jq 'select(.event == "tool_call_finish" and .result_summary.count == 0) | .tool' .codegraph/debug/agentic_debug_*.jsonl

# Extract all reasoning steps
jq 'select(.event == "reasoning_step")' .codegraph/debug/agentic_debug_*.jsonl
```

## Performance Impact

Debug logging has minimal performance impact:

- Events are written asynchronously
- File I/O is buffered and flushed immediately
- Logging only occurs when `CODEGRAPH_DEBUG=1`
- No performance impact when disabled

## Troubleshooting

### No Debug Logs Generated

1. Check `CODEGRAPH_DEBUG=1` is set
2. Check server startup output for:
   ```
   🐛 Debug logging enabled: .codegraph/debug/agentic_debug_20250101_120000.jsonl
   ```
3. Check directory is writable:
   ```bash
   ls -ld .codegraph/debug/
   ```

### Logs Not Updating

1. Ensure server is actually running
2. Check if agentic tools are being called
3. Try `--follow` mode to watch live updates

### Parse Errors

If `view_debug_logs.py` shows parse errors:

1. Check log file isn't corrupted
2. Ensure server finished writing (stop server first)
3. Try parsing with `jq` to find malformed line:
   ```bash
   jq -c . .codegraph/debug/agentic_debug_*.jsonl
   ```

## See Also

- [TESTING.md](TESTING.md) - Running agentic tool tests
- [CLAUDE.md](CLAUDE.md) - MCP server configuration
- [crates/codegraph-mcp/src/debug_logger.rs](crates/codegraph-mcp/src/debug_logger.rs) - Debug logger implementation
