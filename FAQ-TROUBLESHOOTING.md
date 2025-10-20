# CodeGraph MCP FAQ & Troubleshooting

**Complete guide for setting up and troubleshooting CodeGraph with Qwen2.5-Coder integration**

---

## 🚀 **Quick Start FAQ**

### Q: What can I use immediately while Qwen2.5-Coder downloads?
**A: Several powerful features work without the external model:**

✅ **Available Now:**
- `codegraph.pattern_detection`: Team intelligence and convention analysis
- `vector.search`: Basic semantic search using existing FAISS
- `graph.neighbors` & `graph.traverse`: Code relationship analysis
- `codegraph.performance_metrics`: System monitoring
- `tools/list`: MCP protocol compliance

⏳ **Available Once Model Downloads:**
- `codegraph.enhanced_search`: AI-powered search analysis
- `codegraph.semantic_intelligence`: Comprehensive codebase analysis
- `codegraph.impact_analysis`: Revolutionary change impact prediction

### Q: How do I know if everything is working?
**A: Run this test sequence:**

```bash
# 1. Check build
cargo build -p codegraph-mcp --features qwen-integration

# 2. Test MCP server
echo '{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}' | ./target/debug/codegraph start stdio

# 3. Test pattern detection (works without model)
echo '{"jsonrpc":"2.0","id":2,"method":"codegraph.pattern_detection","params":{"focus_area":"all_patterns"}}' | ./target/debug/codegraph start stdio --features qwen-integration
```

### Q: How do I configure Claude Desktop?
**A: Add this to your Claude Desktop settings:**

```json
{
  "mcpServers": {
    "codegraph": {
      "command": "/path/to/codegraph-rust/target/debug/codegraph",
      "args": ["start", "stdio", "--features", "qwen-integration"],
      "env": {
        "RUST_LOG": "info"
      }
    }
  }
}
```

---

## 🔧 **Installation Troubleshooting**

### Issue: "C compiler cannot create executables"
**Solution:**
```bash
# Install Xcode command line tools
xcode-select --install

# Set macOS deployment target
export MACOSX_DEPLOYMENT_TARGET=11.0

# Retry build
cargo build -p codegraph-mcp --features qwen-integration
```

### Issue: "library 'faiss_c' not found"
**Solution on macOS:**
```bash
# Install FAISS via Homebrew
brew install faiss

# Set library paths
export LIBRARY_PATH="/opt/homebrew/opt/faiss/lib:$LIBRARY_PATH"
export LD_LIBRARY_PATH="/opt/homebrew/opt/faiss/lib:$LD_LIBRARY_PATH"

# Build with FAISS support
cargo build -p codegraph-mcp --features "qwen-integration,faiss"
```

**Solution on Ubuntu/Debian:**
```bash
sudo apt-get update
sudo apt-get install libfaiss-dev
```

### Issue: "Qwen2.5-Coder model not found"
**Solutions:**

**Option 1: Install full model (recommended)**
```bash
ollama pull hf.co/unsloth/Qwen2.5-Coder-14B-Instruct-128K-GGUF:Q4_K_M
```

**Option 2: Use smaller test model**
```bash
ollama pull qwen2.5-coder:7b
export CODEGRAPH_MODEL=qwen2.5-coder:7b
```

**Option 3: Work without model (pattern detection still works)**
```bash
# Many features work without external model
export CODEGRAPH_ENABLE_QWEN=false
```

### Issue: "Failed to connect to Ollama"
**Solution:**
```bash
# Check if Ollama is running
curl http://localhost:11434/api/tags

# If not running, start Ollama
ollama serve

# Check available models
ollama list

# Test model directly
ollama run qwen2.5-coder:7b "Hello, can you analyze code?"
```

---

## ⚡ **Performance Troubleshooting**

### Issue: "Analysis taking >10 seconds"
**Diagnosis:**
```bash
# Check memory usage
top -pid $(pgrep ollama)

# Check model size and quantization
ollama show qwen2.5-coder-14b-128k
```

**Solutions:**
```bash
# 1. Reduce context window
export CODEGRAPH_CONTEXT_WINDOW=64000

# 2. Enable caching for repeated queries
export CODEGRAPH_ENABLE_CACHE=true

# 3. Reduce concurrent requests
export CODEGRAPH_MAX_CONCURRENT=2

# 4. Use smaller model for testing
export CODEGRAPH_MODEL=qwen2.5-coder:7b
```

### Issue: "High memory usage"
**Solutions:**
```bash
# 1. Reduce cache size
export CODEGRAPH_CACHE_SIZE=500

# 2. Reduce cache memory limit
export CODEGRAPH_CACHE_MEMORY_MB=256

# 3. Shorter cache TTL
export CODEGRAPH_CACHE_TTL=900  # 15 minutes

# 4. Monitor cache performance
# Send MCP request: codegraph.cache_stats
```

### Issue: "Low cache hit rates"
**Diagnosis:**
```bash
# Check cache statistics
echo '{"jsonrpc":"2.0","id":1,"method":"codegraph.cache_stats","params":{}}' | ./target/debug/codegraph start stdio --features qwen-integration
```

**Solutions:**
```bash
# 1. Lower similarity threshold for more hits
export CODEGRAPH_CACHE_SIMILARITY=0.8

# 2. Enable semantic matching
export CODEGRAPH_ENABLE_SEMANTIC_CACHE=true

# 3. Increase cache size
export CODEGRAPH_CACHE_SIZE=2000
```

---

## 🧠 **Model Integration Troubleshooting**

### Issue: "Qwen analysis returns generic responses"
**Diagnosis:**
- Check if Qwen model is actually being used
- Verify context is being passed correctly
- Check confidence scores in responses

**Solutions:**
```bash
# 1. Verify model configuration
ollama show qwen2.5-coder-14b-128k

# 2. Check context window utilization
# Look for "context_tokens_used" in MCP responses

# 3. Adjust temperature for more focused responses
export CODEGRAPH_TEMPERATURE=0.05

# 4. Increase context window if using partial context
export CODEGRAPH_CONTEXT_WINDOW=120000
```

### Issue: "Model responses are inconsistent"
**Solutions:**
```bash
# 1. Lower temperature for consistency
export CODEGRAPH_TEMPERATURE=0.1

# 2. Use structured prompts (already implemented)
# 3. Enable caching for consistent responses
export CODEGRAPH_ENABLE_CACHE=true

# 4. Check model quantization
ollama show qwen2.5-coder-14b-128k | grep -i quantization
```

---

## 🔗 **Integration Troubleshooting**

### Issue: "Claude Desktop doesn't see CodeGraph"
**Solutions:**
```bash
# 1. Check Claude Desktop config location
ls -la "$HOME/Library/Application Support/Claude/claude_desktop_config.json"

# 2. Verify config syntax
cat "$HOME/Library/Application Support/Claude/claude_desktop_config.json" | jq .

# 3. Test MCP server manually
./target/debug/codegraph start stdio --features qwen-integration

# 4. Check logs in Claude Desktop (View > Developer Tools)
```

### Issue: "MCP tools not appearing in Claude"
**Solutions:**
```bash
# 1. Verify tools list
echo '{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}' | ./target/debug/codegraph start stdio --features qwen-integration

# 2. Check for JSON-RPC errors in Claude Developer Tools

# 3. Verify feature flags
export CODEGRAPH_ENABLE_QWEN=true

# 4. Restart Claude Desktop after config changes
```

### Issue: "Custom agent integration not working"
**Example Python integration:**
```python
import asyncio
import websockets
import json

async def test_codegraph():
    uri = "ws://localhost:3000/mcp"  # If using HTTP transport

    async with websockets.connect(uri) as websocket:
        # Test tools list
        request = {
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/list",
            "params": {}
        }

        await websocket.send(json.dumps(request))
        response = await websocket.recv()
        print("Tools:", json.loads(response))

# Run with: asyncio.run(test_codegraph())
```

---

## 📊 **Performance Optimization**

### Memory Optimization
```bash
# For 32GB+ systems (optimal)
export CODEGRAPH_CONTEXT_WINDOW=128000
export CODEGRAPH_CACHE_SIZE=2000
export CODEGRAPH_MAX_CONCURRENT=5

# For 24-31GB systems (good)
export CODEGRAPH_CONTEXT_WINDOW=100000
export CODEGRAPH_CACHE_SIZE=1500
export CODEGRAPH_MAX_CONCURRENT=3

# For 16-23GB systems (minimal)
export CODEGRAPH_CONTEXT_WINDOW=64000
export CODEGRAPH_CACHE_SIZE=1000
export CODEGRAPH_MAX_CONCURRENT=2

# For <16GB systems (basic)
export CODEGRAPH_ENABLE_QWEN=false  # Disable Qwen
export CODEGRAPH_CACHE_SIZE=500
export CODEGRAPH_MAX_CONCURRENT=1
```

### Response Time Optimization
```bash
# Enable all optimizations
export CODEGRAPH_ENABLE_CACHE=true
export CODEGRAPH_CACHE_SIMILARITY=0.85
export CODEGRAPH_CONTEXT_WINDOW=80000  # Faster analysis

# Monitor performance
echo '{"jsonrpc":"2.0","id":1,"method":"codegraph.performance_metrics","params":{}}' | ./target/debug/codegraph start stdio --features qwen-integration
```

---

## 🎯 **Real-World Usage Examples**

### Example 1: New Developer Onboarding
```
Claude Query: "I'm new to this codebase. Can you help me understand the overall architecture and coding patterns?"

Expected Response:
- Claude calls codegraph.pattern_detection
- Gets team conventions, naming patterns, architectural insights
- Calls codegraph.semantic_intelligence (if Qwen available)
- Provides comprehensive onboarding guide specific to your codebase
```

### Example 2: Safe Refactoring
```
Claude Query: "I want to modify the authentication system. What are the risks and how should I proceed?"

Expected Response:
- Claude calls codegraph.impact_analysis
- Gets comprehensive dependency analysis
- Provides step-by-step safe refactoring plan
- Shows exactly what will be affected
```

### Example 3: Code Quality Assessment
```
Claude Query: "Analyze the code quality and consistency of this project"

Expected Response:
- Claude calls codegraph.pattern_detection
- Gets quality metrics and consistency scores
- Provides specific improvement recommendations
- Identifies team strengths and areas for improvement
```

---

## 🔍 **Debugging Commands**

### Check System Status
```bash
# Overall system health
echo '{"jsonrpc":"2.0","id":1,"method":"codegraph.performance_metrics","params":{}}' | ./target/debug/codegraph start stdio --features qwen-integration

# Cache performance
echo '{"jsonrpc":"2.0","id":2,"method":"codegraph.cache_stats","params":{}}' | ./target/debug/codegraph start stdio --features qwen-integration

# Available tools
echo '{"jsonrpc":"2.0","id":3,"method":"tools/list","params":{}}' | ./target/debug/codegraph start stdio --features qwen-integration
```

### Debug Logging
```bash
# Enable debug logging
export RUST_LOG=debug

# Start with verbose output
./target/debug/codegraph start stdio --features qwen-integration

# Check specific component logging
export RUST_LOG=codegraph_mcp=debug,qwen=info
```

### Performance Analysis
```bash
# Time MCP requests
time echo '{"jsonrpc":"2.0","id":1,"method":"codegraph.pattern_detection","params":{"focus_area":"naming"}}' | ./target/debug/codegraph start stdio --features qwen-integration

# Monitor memory usage
top -pid $(pgrep codegraph)

# Check Ollama model status
ollama ps
```

---

## 🎉 **Success Indicators**

### ✅ **Everything Working When You See:**
- MCP server starts without errors
- Tools list returns 6+ tools
- Pattern detection completes in <3 seconds
- Cache hit rates >30% after warmup
- Claude Desktop shows CodeGraph as connected
- Responses include team-specific insights

### 🚨 **Needs Attention When You See:**
- Compilation errors about missing dependencies
- "Model not found" errors (install Qwen model)
- Response times >10 seconds (memory pressure)
- Cache hit rates <10% (similarity threshold too high)
- Generic responses (model not being used)

---

## 📞 **Getting Help**

### Check Implementation Status
```bash
# Version and features
./target/debug/codegraph --version

# Git commit information
git log --oneline -5

# Feature compilation
cargo check -p codegraph-mcp --features qwen-integration
```

### Community Resources
- **Documentation**: See docs/realistic-system-design/
- **Examples**: Interactive infographics and demos
- **Issues**: Check git status and recent commits
- **Performance**: Use built-in monitoring tools

---

**Remember: You have a revolutionary system that makes any MCP-compatible LLM a codebase expert through your unique 90K+ lines of semantic analysis. This is impossible for competitors to replicate!**