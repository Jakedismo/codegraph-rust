# CodeGraph Qwen2.5-Coder Integration

**SOTA local model integration for enhanced MCP intelligence**

## 🚀 Quick Start

### 1. Install Qwen2.5-Coder-14B-128K

```bash
# Install the SOTA model (while you're downloading it in background)
ollama pull hf.co/unsloth/Qwen2.5-Coder-14B-Instruct-128K-GGUF:Q4_K_M

# Or create alias
ollama create qwen2.5-coder-14b-128k -f - << 'EOF'
FROM hf.co/unsloth/Qwen2.5-Coder-14B-Instruct-128K-GGUF:Q4_K_M
PARAMETER num_ctx 128000
PARAMETER temperature 0.1
EOF
```

### 2. Build CodeGraph with Qwen Integration

```bash
# Build with Qwen integration feature
MACOSX_DEPLOYMENT_TARGET=11.0 cargo build -p codegraph-mcp --features qwen-integration

# Verify build
./target/debug/codegraph --version
```

### 3. Start MCP Server

```bash
# Start MCP server with Qwen integration
RUST_LOG=info ./target/debug/codegraph start stdio --features qwen-integration
```

## 🔧 Available MCP Tools

### `codegraph.enhanced_search`
Enhanced semantic search with Qwen2.5-Coder intelligence

**Usage:**
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "codegraph.enhanced_search",
  "params": {
    "query": "user authentication flow",
    "include_analysis": true,
    "max_results": 10
  }
}
```

**Response:**
```json
{
  "search_results": [...],
  "ai_analysis": "Comprehensive Qwen2.5-Coder analysis...",
  "intelligence_metadata": {
    "model_used": "qwen2.5-coder-14b-128k",
    "processing_time_ms": 3200,
    "context_tokens": 15420,
    "completion_tokens": 2840,
    "confidence_score": 0.92,
    "context_window_used": 128000
  },
  "generation_guidance": "...",
  "quality_assessment": "..."
}
```

### `codegraph.semantic_intelligence`
Comprehensive codebase intelligence for MCP-calling LLMs

**Usage:**
```json
{
  "jsonrpc": "2.0",
  "id": 2,
  "method": "codegraph.semantic_intelligence",
  "params": {
    "query": "authentication system architecture",
    "task_type": "comprehensive_analysis",
    "max_context_tokens": 100000
  }
}
```

## 📊 Performance Expectations

### Model Performance
- **Context Window**: 128,000 tokens (complete codebase understanding)
- **Analysis Time**: 3-5 seconds for comprehensive analysis
- **Memory Usage**: ~24GB VRAM (fits in 32GB MacBook Pro)
- **Quality**: SOTA performance for 14B parameter class

### Response Times
- **Enhanced Search**: 2-3 seconds
- **Semantic Intelligence**: 4-6 seconds
- **Pattern Analysis**: 3-4 seconds
- **Impact Assessment**: 3-5 seconds

## 🔗 Integration Examples

### Claude Desktop Integration

```json
{
  "mcpServers": {
    "codegraph": {
      "command": "./target/debug/codegraph",
      "args": ["start", "stdio", "--features", "qwen-integration"],
      "env": {
        "RUST_LOG": "info"
      }
    }
  }
}
```

### Testing with curl

```bash
# Test enhanced search
curl -X POST http://localhost:3000/mcp \
  -H "Content-Type: application/json" \
  -d '{
    "jsonrpc": "2.0",
    "id": 1,
    "method": "codegraph.enhanced_search",
    "params": {
      "query": "authentication function",
      "include_analysis": true
    }
  }'
```

## 🎯 Next Steps

1. **Test with your codebase** once Qwen2.5-Coder finishes downloading
2. **Integrate with Claude Desktop** for real-world testing
3. **Measure performance** and optimize prompts
4. **Add more MCP tools** for impact analysis and pattern detection

## 🚨 Requirements

- **macOS**: macOS 11.0+ (for compilation)
- **Memory**: 32GB RAM recommended for optimal performance
- **Ollama**: Latest version with model support
- **Qwen2.5-Coder**: 14B model with 128K context window

**Status**: ✅ Build successful, ready for testing once model downloads complete!