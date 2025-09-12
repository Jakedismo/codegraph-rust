---
pdf-engine: lualatex
mainfont: "DejaVu Serif"
monofont: "DejaVu Sans Mono"
header-includes: |
  \usepackage{fontspec}
  \directlua{
    luaotfload.add_fallback("emojifallback", {"NotoColorEmoji:mode=harf;"})
  }
  \setmainfont[
    RawFeature={fallback=emojifallback}
  ]{DejaVu Serif}
---

# Enhanced MCP Server with CodeGraph Integration

## Overview

The Enhanced MCP Server extends the standard Model Context Protocol implementation with powerful code analysis capabilities powered by the CodeGraph engine. It provides architecture discovery, dependency analysis, and performance-optimized query processing through a unified MCP interface.

## Key Features

### 🚀 Core Capabilities

- **Dual Transport Support**: Simultaneous stdio and HTTP streaming transports
- **CodeGraph Integration**: Deep code analysis using Rust-based graph engine
- **Performance Optimization**: Advanced caching, batching, and query optimization
- **Architecture Discovery**: Automatic detection of patterns and anti-patterns
- **Dependency Analysis**: Forward and reverse dependency tracking
- **Real-time Metrics**: Performance monitoring and optimization insights

### 🛠️ Analytical Tools

#### 1. Dependency Analysis Tools

##### `codegraph_find_dependencies`
Analyzes dependencies of files, functions, or modules.

**Parameters:**
- `target` (string): File path, function name, or module to analyze
- `depth` (number, optional): Maximum dependency depth (default: 3)
- `includeTests` (boolean, optional): Include test files
- `includeVendor` (boolean, optional): Include vendor/node_modules

**Returns:**
```json
{
  "directDependencies": [...],
  "transitiveDependencies": [...],
  "dependencyGraph": {
    "nodes": [...],
    "edges": [...],
    "metrics": {...}
  }
}
```

##### `codegraph_reverse_dependencies`
Finds what depends on a given target (impact analysis).

**Parameters:**
- `target` (string): File path, function, or module
- `depth` (number, optional): Maximum depth to traverse

**Returns:**
```json
{
  "directDependents": [...],
  "transitiveDependents": [...],
  "impactAnalysis": {
    "affectedFiles": [...],
    "affectedFunctions": [...],
    "riskLevel": "low|medium|high"
  }
}
```

#### 2. Architecture Discovery Tools

##### `codegraph_architecture_analysis`
Analyzes codebase architecture and identifies patterns.

**Parameters:**
- `rootPath` (string): Root directory to analyze
- `maxNodes` (number, optional): Maximum nodes to analyze
- `timeout` (number, optional): Analysis timeout in seconds

**Returns:**
```json
{
  "architecture": {
    "nodes": [...],
    "edges": [...],
    "clusters": {...},
    "metrics": {...}
  },
  "patterns": [
    {
      "type": "singleton|factory|observer|...",
      "instances": [...],
      "severity": "info|warning|error"
    }
  ],
  "recommendations": [...]
}
```

##### `codegraph_function_relationships`
Discovers function call graphs and relationships.

**Parameters:**
- `filePath` (string): File to analyze
- `maxNodes` (number, optional): Maximum nodes to return
- `includeTests` (boolean, optional): Include test functions

**Returns:**
```json
{
  "functions": [...],
  "callGraph": [...],
  "clusters": {...}
}
```

#### 3. Code Quality Tools

##### `codegraph_complexity_metrics`
Calculates complexity metrics for code analysis.

**Parameters:**
- `target` (string): File or function to analyze

**Returns:**
```json
{
  "cyclomaticComplexity": 10,
  "cognitiveComplexity": 15,
  "halsteadMetrics": {
    "volume": 250,
    "difficulty": 8,
    "effort": 2000
  },
  "maintainabilityIndex": 75
}
```

##### `codegraph_cyclic_dependencies`
Detects circular dependencies in the codebase.

**Parameters:**
- `rootPath` (string): Root directory to analyze
- `includeTests` (boolean, optional): Include test files

**Returns:**
```json
{
  "cycles": [
    {
      "nodes": ["file1.ts", "file2.ts", "file3.ts"],
      "type": "import|call|mixed",
      "severity": "low|medium|high"
    }
  ],
  "suggestions": [...]
}
```

#### 4. Search and Discovery Tools

##### `codegraph_pattern_search`
Searches for code patterns using semantic analysis.

**Parameters:**
- `pattern` (string): Pattern to search for
- `semantic` (boolean, optional): Use AI-powered semantic search
- `maxNodes` (number, optional): Maximum results

**Returns:**
```json
{
  "matches": [
    {
      "file": "src/utils.ts",
      "line": 42,
      "confidence": 0.95,
      "snippet": "..."
    }
  ],
  "totalMatches": 15
}
```

##### `codegraph_file_mapping`
Maps files to their contained functions and classes.

**Parameters:**
- `pattern` (string): File pattern (e.g., "src/**/*.ts")
- `includeTests` (boolean, optional): Include test files

**Returns:**
```json
{
  "src/index.ts": [
    {
      "id": "node-123",
      "name": "main",
      "type": "function",
      "line": 10
    }
  ]
}
```

#### 5. Performance and Utility Tools

##### `batch_analysis`
Performs multiple analyses in parallel for better performance.

**Parameters:**
- `analyses` (array): Array of analysis requests

**Example:**
```json
{
  "analyses": [
    {
      "type": "dependencies",
      "target": "src/index.ts",
      "options": { "depth": 2 }
    },
    {
      "type": "complexity",
      "target": "src/utils.ts"
    }
  ]
}
```

##### `discover_capabilities`
Lists all available CodeGraph analysis capabilities.

**Parameters:**
- `category` (string, optional): Filter by category (all|analysis|architecture|metrics|search)

##### `get_performance_stats`
Returns performance metrics for CodeGraph operations.

##### `clear_analysis_cache`
Clears the analysis cache to force fresh results.

## Transport Configuration

### Stdio Transport
```bash
# Start with stdio transport only
npm run server -- --transport stdio
```

### HTTP Transport
```bash
# Start with HTTP transport only
npm run server -- --transport http --port 3000
```

### Dual Transport Mode
```bash
# Start with both transports (default)
npm run server -- --transport both --port 3000
```

## Usage Examples

### TypeScript Client Example

```typescript
import { Client } from '@modelcontextprotocol/sdk/client/index.js';
import { StreamableHTTPClientTransport } from '@modelcontextprotocol/sdk/client/streamableHttp.js';

// Connect to server
const client = new Client({
  name: 'analysis-client',
  version: '1.0.0'
});

const transport = new StreamableHTTPClientTransport(
  new URL('http://localhost:3000/mcp')
);

await client.connect(transport);

// Find dependencies
const deps = await client.callTool({
  name: 'codegraph_find_dependencies',
  arguments: {
    target: 'src/server/index.ts',
    depth: 3,
    includeTests: false
  }
});

// Analyze architecture
const architecture = await client.callTool({
  name: 'codegraph_architecture_analysis',
  arguments: {
    rootPath: 'src',
    maxNodes: 1000
  }
});

// Batch analysis for performance
const results = await client.callTool({
  name: 'batch_analysis',
  arguments: {
    analyses: [
      { type: 'dependencies', target: 'src/index.ts' },
      { type: 'complexity', target: 'src/utils.ts' },
      { type: 'functions', target: 'src/handlers.ts' }
    ]
  }
});
```

### CLI Usage

```bash
# Start server with specific configuration
./start-server.ts \
  --transport both \
  --port 3000 \
  --api-url http://localhost:3030 \
  --cors "http://localhost:*" \
  --allowed-hosts "localhost,127.0.0.1"

# With performance optimizations
./start-server.ts \
  --transport http \
  --port 3000 \
  --cache \
  --metrics
```

## Performance Optimization

### Caching Strategy

The server implements multi-level caching:
1. **Query Result Cache**: LRU cache for query results
2. **Pattern Cache**: Optimized file pattern matching
3. **Batch Processing**: Automatic query batching

### Query Optimization

- Automatic depth limiting for deep queries
- Pattern optimization for file searches
- Parallel execution for independent queries
- Result compression for large datasets

### Metrics and Monitoring

Access real-time metrics:
- **HTTP Endpoint**: `http://localhost:3000/metrics`
- **MCP Resource**: `metrics://server` and `metrics://codegraph`

## Configuration Options

```typescript
interface EnhancedMCPServerConfig {
  name: string;                    // Server name
  version: string;                 // Server version
  transport: 'stdio' | 'http' | 'both';
  httpPort?: number;               // HTTP port (default: 3000)
  corsOrigins?: string[];          // CORS origins
  codeGraphApiUrl?: string;        // CodeGraph API URL
  enableCache?: boolean;           // Enable caching (default: true)
  enableMetrics?: boolean;         // Enable metrics (default: true)
  enableDnsRebindingProtection?: boolean;
  allowedHosts?: string[];
  sessionIdGenerator?: () => string;
}
```

## Security Features

- **DNS Rebinding Protection**: Prevents DNS rebinding attacks
- **CORS Configuration**: Configurable CORS origins
- **Session Management**: Secure session handling with UUIDs
- **Rate Limiting**: Built-in request throttling (configurable)

## API Endpoints

### HTTP Endpoints
- `POST /mcp` - Main MCP message handler
- `GET /mcp` - SSE notifications stream
- `DELETE /mcp` - Session termination
- `GET /health` - Health check endpoint
- `GET /metrics` - Performance metrics

### MCP Resources
- `codegraph://metrics` - CodeGraph performance metrics
- `metrics://server` - Server metrics
- `metrics://codegraph` - CodeGraph operation metrics

## Error Handling

The server provides comprehensive error handling:
- Graceful degradation for CodeGraph API failures
- Automatic retry with exponential backoff
- Detailed error messages with context
- Error metrics tracking

## Development

### Running Tests
```bash
npm test
```

### Building
```bash
npm run build
```

### Development Mode
```bash
npm run dev -- --transport both --port 3000
```

## Troubleshooting

### Common Issues

1. **Port Already in Use**
   ```bash
   # Use a different port
   npm run server -- --port 3001
   ```

2. **CodeGraph API Connection Failed**
   ```bash
   # Specify correct API URL
   npm run server -- --api-url http://localhost:3030
   ```

3. **Cache Issues**
   ```bash
   # Disable cache for debugging
   npm run server -- --no-cache
   ```

### Debug Mode

Enable debug logging:
```bash
DEBUG=mcp:* npm run server
```

## Performance Benchmarks

Typical performance metrics (on standard hardware):
- Dependency analysis: ~50-200ms
- Architecture analysis: ~200-500ms
- Pattern search: ~100-300ms
- Batch operations: 30-50% faster than sequential

## License

MIT License - See LICENSE file for details.

## Contributing

Contributions are welcome! Please see CONTRIBUTING.md for guidelines.