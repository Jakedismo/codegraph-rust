# CodeGraph Native Addon (NAPI-RS)

**Zero-overhead TypeScript integration** - Direct function calls to Rust, no process spawning, no HTTP.

## Features

⚡ **Maximum Performance** - Direct FFI calls, no IPC overhead
🎯 **Type-Safe** - Auto-generated TypeScript definitions
🔒 **Memory-Safe** - Rust's safety guarantees
🌍 **Cross-Platform** - Windows, macOS, Linux (x64, ARM64)
📦 **Easy Integration** - Drop-in npm package

### 🆕 Cloud Features (New!)

🔍 **Dual-Mode Semantic Search** - Automatic routing between local FAISS and cloud SurrealDB HNSW
☁️ **Jina AI Integration** - Cloud embeddings with 8192 dimensions and reranking
🔄 **Hot-Reload Configuration** - Update settings without process restart
📊 **Embedding Statistics** - Real-time metrics for provider and cache performance
🎯 **Smart Search Routing** - Automatic fallback from cloud to local on failures

## Installation from Local Repository

### Option 1: Install Directly from Directory (Recommended)

```bash
# Build the addon once
cd /path/to/codegraph-rust/crates/codegraph-napi
npm install
npm run build

# Install in your project
cd /path/to/your-project
npm install /path/to/codegraph-rust/crates/codegraph-napi
```

### Option 2: Pack and Install

```bash
# Build and pack the addon
cd /path/to/codegraph-rust/crates/codegraph-napi
npm install
npm run build
npm pack  # Or: bun run pack

# This creates: codegraph-napi-1.0.0.tgz

# Install the tarball in your project
cd /path/to/your-project
npm install /path/to/codegraph-rust/crates/codegraph-napi/codegraph-napi-1.0.0.tgz
```

The tarball includes:
- ✅ Compiled `.node` binary for your platform
- ✅ TypeScript definitions (`index.d.ts`)
- ✅ `package.json` with all metadata

**Quick Example:**

```bash
# One-time: Build and pack
cd ~/codegraph-rust/crates/codegraph-napi
npm install && npm run build && npm pack

# Share the tarball or install locally
cd ~/my-awesome-app
npm install ~/codegraph-rust/crates/codegraph-napi/codegraph-napi-1.0.0.tgz

# Start using immediately
cat > search.ts << 'EOF'
import { semanticSearch } from 'codegraph-napi';

const results = await semanticSearch('authentication');
console.log(results);
EOF

npx tsx search.ts
```

### Option 3: Add to package.json

```json
{
  "dependencies": {
    "codegraph-napi": "file:../codegraph-rust/crates/codegraph-napi"
  }
}
```

Then run `npm install` or `bun install`.

### Option 4: Pre-built Binaries (Coming Soon)

```bash
npm install codegraph
```

## Configuration

Create `.codegraph/config.toml` in your project root:

```toml
[embedding]
# Local ONNX model (always available)
model = "all-MiniLM-L6-v2"
dimension = 384

# Optional: Jina AI cloud embeddings
jina_api_key = "jina_xxx"  # Or use JINA_API_KEY env var
jina_model = "jina-embeddings-v3"
jina_task_type = "retrieval.query"
jina_enable_reranking = true

[vector]
# FAISS configuration
index_type = "IVFFlat"
n_lists = 100
n_probe = 10

[storage]
data_dir = ".codegraph/data"
cache_dir = ".codegraph/cache"
```

### Environment Variables

```bash
# Jina AI API key (alternative to config file)
export JINA_API_KEY="jina_xxx"

# SurrealDB connection (for cloud HNSW search)
export SURREALDB_CONNECTION="ws://localhost:8000"

# Enable cloud features
export CODEGRAPH_CLOUD_ENABLED=true
```

## Quick Start

> **Note**: All examples work with both `npm` and `bun`. Simply replace `npm` with `bun` in any command.

### Semantic Search (New!)

```typescript
import { semanticSearch, getCloudConfig } from 'codegraph-napi';

// Check cloud availability
const cloudConfig = await getCloudConfig();
console.log('Jina AI enabled:', cloudConfig.jinaEnabled);
console.log('SurrealDB enabled:', cloudConfig.surrealdbEnabled);

// Dual-mode semantic search (automatic routing)
const results = await semanticSearch('authentication logic', {
  limit: 10,
  minSimilarity: 0.7,
  filterByType: 'function',
});

console.log(`Mode used: ${results.modeUsed}`); // 'local' or 'cloud'
console.log(`Found ${results.totalCount} results in ${results.searchTimeMs}ms`);

for (const result of results.localResults) {
  console.log(`[${result.similarity.toFixed(3)}] ${result.name}`);
}
```

### Version Management

```typescript
import {
  createVersion,
  listVersions,
  createBranch,
  mergeBranches,
} from 'codegraph-napi';

// Create a version - direct function call!
const version = await createVersion({
  name: 'v1.0.0',
  description: 'Initial release',
  author: 'user@example.com',
  parents: undefined,
});

console.log(`Created: ${version.versionId}`);

// List versions
const versions = await listVersions(50);
versions.forEach(v => console.log(v.name));

// Create branch
await createBranch({
  name: 'feature/api',
  from: version.versionId,
  author: 'user@example.com',
});
```

## API Reference

### Initialization

```typescript
import { initialize, getAddonVersion } from 'codegraph-napi';

// Optional - initializes automatically on first call
await initialize();

// Get addon version
const version = getAddonVersion();
```

### Semantic Search API (New!)

```typescript
// Semantic search with dual-mode support
interface SearchOptions {
  query?: string;
  limit?: number;              // Default: 10
  offset?: number;             // Default: 0
  minSimilarity?: number;      // Default: 0.0 (range: 0.0-1.0)
  filterByType?: string;       // 'function' | 'class' | 'module' | 'variable'
}

interface SearchResult {
  id: string;
  name: string;
  description?: string;
  similarity: number;
  metadata?: string;           // JSON stringified
}

interface DualModeSearchResult {
  localResults: SearchResult[];
  cloudResults?: SearchResult[];      // Only if cloud enabled
  rerankedResults?: SearchResult[];   // Only if Jina reranking enabled
  totalCount: number;
  searchTimeMs: number;
}

const results = await semanticSearch('error handling patterns', {
  limit: 25,
  minSimilarity: 0.6,
  filterByType: 'function',
});

// Find similar functions by node ID
const similarFunctions = await searchSimilarFunctions('node-id-uuid', 10);
```

### Cloud Configuration API (New!)

```typescript
interface CloudConfig {
  jinaEnabled: boolean;
  jinaModel: string;
  jinaRerankingEnabled: boolean;
  surrealdbEnabled: boolean;
  surrealdbUrl?: string;
}

interface EmbeddingStats {
  provider: string;            // 'jina-ai' | 'onnx-local'
  model: string;
  dimension: number;
  totalEmbeddings: number;
  cacheHitRate: number;        // 0.0-1.0
}

// Get current cloud configuration
const config = await getCloudConfig();

// Hot-reload configuration without restart
const changed = await reloadConfig();
if (changed) {
  console.log('Configuration updated!');
}

// Get embedding statistics
const stats = await getEmbeddingStats();
console.log(`Provider: ${stats.provider}`);
console.log(`Cache hit rate: ${(stats.cacheHitRate * 100).toFixed(1)}%`);

// Check if cloud features are available
const available = await isCloudAvailable();

// Get configuration file path
const configPath = await getConfigPath();
```

### Transaction Management

```typescript
// Begin transaction
const tx = await beginTransaction('serializable');
// Options: 'read-uncommitted', 'read-committed', 'repeatable-read', 'serializable'

// Commit transaction
await commitTransaction(tx.transactionId);

// Rollback transaction
await rollbackTransaction(tx.transactionId);

// Get statistics
const stats = await getTransactionStats();
console.log(stats.activeTransactions);
```

### Version Management

```typescript
// Create version
const version = await createVersion({
  name: 'v1.0.0',
  description: 'Release notes',
  author: 'user@example.com',
  parents: ['parent-id-1', 'parent-id-2'],  // Optional
});

// List versions
const versions = await listVersions(50);  // limit optional

// Get version by ID
const version = await getVersion('version-id');

// Tag version
await tagVersion('version-id', 'stable');

// Compare versions
const diff = await compareVersions('from-id', 'to-id');
console.log(`${diff.addedNodes} added, ${diff.modifiedNodes} modified`);
```

### Branch Management

```typescript
// Create branch
const branch = await createBranch({
  name: 'feature/auth',
  from: 'version-id',
  author: 'user@example.com',
  description: 'Authentication feature',  // Optional
});

// List branches
const branches = await listBranches();

// Get branch
const branch = await getBranch('branch-name');

// Delete branch
await deleteBranch('branch-name');

// Merge branches
const result = await mergeBranches({
  source: 'feature/auth',
  target: 'main',
  author: 'user@example.com',
  message: 'Merge auth feature',  // Optional
});

if (result.success) {
  console.log('Merged!');
} else {
  console.log(`${result.conflicts} conflicts`);
}
```

## Types

### TransactionResult

```typescript
interface TransactionResult {
  transactionId: string;
  isolationLevel: string;
  status: string;
}
```

### VersionResult

```typescript
interface VersionResult {
  versionId: string;
  name: string;
  description: string;
  author: string;
  createdAt: string;  // ISO 8601 format
}
```

### BranchResult

```typescript
interface BranchResult {
  name: string;
  head: string;  // Version ID
  createdAt: string;
  createdBy: string;
}
```

### TransactionStats

```typescript
interface TransactionStats {
  activeTransactions: number;
  committedTransactions: string;  // u64 as string
  abortedTransactions: string;    // u64 as string
  averageCommitTimeMs: number;
}
```

## Performance

### Benchmark: Native Addon vs CLI Spawning

```typescript
// Native addon (direct function call)
console.time('native');
for (let i = 0; i < 1000; i++) {
  await getTransactionStats();
}
console.timeEnd('native');
// ~150ms (0.15ms per call)

// CLI spawning
console.time('cli');
for (let i = 0; i < 1000; i++) {
  await exec('codegraph transaction stats');
}
console.timeEnd('cli');
// ~45,000ms (45ms per call)
```

**Native addon is ~300x faster** for high-frequency operations!

## Integration Examples

### Express API Server

```typescript
import express from 'express';
import { createVersion, listVersions } from 'codegraph-napi';

const app = express();
app.use(express.json());

app.post('/api/versions', async (req, res) => {
  try {
    const version = await createVersion(req.body);
    res.json(version);
  } catch (error) {
    res.status(500).json({ error: error.message });
  }
});

app.get('/api/versions', async (req, res) => {
  const versions = await listVersions(50);
  res.json(versions);
});

app.listen(3000);
```

### CLI Tool

```typescript
#!/usr/bin/env node
import { Command } from 'commander';
import { createVersion, listVersions } from 'codegraph-napi';

const program = new Command();

program
  .command('version:create')
  .option('-n, --name <name>')
  .option('-d, --description <desc>')
  .action(async (options) => {
    const version = await createVersion({
      name: options.name,
      description: options.description,
      author: 'cli',
      parents: undefined,
    });
    console.log(`Created: ${version.versionId}`);
  });

program
  .command('version:list')
  .action(async () => {
    const versions = await listVersions(50);
    versions.forEach(v => console.log(`${v.name}: ${v.description}`));
  });

program.parse();
```

### Background Worker

```typescript
import { Queue, Worker } from 'bullmq';
import { createVersion, mergeBranches } from 'codegraph-napi';

const worker = new Worker('codegraph-tasks', async job => {
  switch (job.name) {
    case 'create-version':
      return await createVersion(job.data);

    case 'merge-branches':
      return await mergeBranches(job.data);

    default:
      throw new Error('Unknown job type');
  }
});
```

## Cloud Features Usage Examples

### Example 1: Semantic Code Search with Fallback

```typescript
import { semanticSearch, getCloudConfig } from 'codegraph-napi';

async function searchCode(query: string) {
  // Check cloud availability first
  const config = await getCloudConfig();
  console.log(`Cloud mode: ${config.jinaEnabled ? 'enabled' : 'local-only'}`);

  const results = await semanticSearch(query, {
    limit: 20,
    minSimilarity: 0.65,
    filterByType: 'function',
  });

  console.log(`\nSearch: "${query}"`);
  console.log(`Mode used: ${results.modeUsed}`);
  console.log(`Time: ${results.searchTimeMs.toFixed(2)}ms`);
  console.log(`Found ${results.totalCount} results\n`);

  // Display results
  const displayResults = results.cloudResults || results.localResults;
  for (const result of displayResults) {
    console.log(`[${result.similarity.toFixed(3)}] ${result.name}`);
    if (result.description) {
      console.log(`  ${result.description.substring(0, 80)}...`);
    }
  }
}

await searchCode('JWT token validation');
```

### Example 2: Hot Configuration Reload

```typescript
import { watch } from 'fs';
import { reloadConfig, getCloudConfig, getConfigPath } from 'codegraph-napi';

async function watchConfiguration() {
  const configPath = await getConfigPath();
  console.log(`Watching config: ${configPath}`);

  watch(configPath, async (eventType) => {
    if (eventType === 'change') {
      console.log('📝 Config file changed, reloading...');

      const changed = await reloadConfig();
      if (changed) {
        const config = await getCloudConfig();
        console.log('✅ Configuration reloaded');
        console.log(`  Jina AI: ${config.jinaEnabled ? 'enabled' : 'disabled'}`);
        console.log(`  Model: ${config.jinaModel}`);
        console.log(`  Reranking: ${config.jinaRerankingEnabled ? 'enabled' : 'disabled'}`);
      } else {
        console.log('ℹ️  Configuration unchanged');
      }
    }
  });

  // Keep the process running
  await new Promise(() => {});
}

watchConfiguration().catch(console.error);
```

### Example 3: Embedding Provider Monitoring

```typescript
import { getEmbeddingStats, semanticSearch } from 'codegraph-napi';

async function monitorEmbeddings() {
  // Get initial stats
  const initialStats = await getEmbeddingStats();
  console.log('Initial Stats:');
  console.log(`  Provider: ${initialStats.provider}`);
  console.log(`  Model: ${initialStats.model}`);
  console.log(`  Dimension: ${initialStats.dimension}`);
  console.log(`  Total embeddings: ${initialStats.totalEmbeddings}`);
  console.log(`  Cache hit rate: ${(initialStats.cacheHitRate * 100).toFixed(1)}%`);

  // Perform some searches
  console.log('\nPerforming searches...');
  await semanticSearch('authentication', { limit: 5 });
  await semanticSearch('database connection', { limit: 5 });
  await semanticSearch('error handling', { limit: 5 });

  // Check updated stats
  const updatedStats = await getEmbeddingStats();
  console.log('\nUpdated Stats:');
  console.log(`  Total embeddings: ${updatedStats.totalEmbeddings}`);
  console.log(`  Cache hit rate: ${(updatedStats.cacheHitRate * 100).toFixed(1)}%`);

  // Alert if cache hit rate is low
  if (updatedStats.cacheHitRate < 0.5) {
    console.warn('⚠️  Low cache hit rate - consider warming up cache');
  }
}

monitorEmbeddings().catch(console.error);
```

### Example 4: Progressive Search (Local → Cloud)

```typescript
import { semanticSearch, isCloudAvailable } from 'codegraph-napi';

async function progressiveSearch(query: string) {
  // Try local search first (fast)
  console.log('🔍 Searching locally...');
  const localStart = Date.now();
  const localResults = await semanticSearch(query, {
    limit: 10,
    minSimilarity: 0.8,  // High threshold for local
  });
  const localTime = Date.now() - localStart;

  console.log(`Local search: ${localResults.totalCount} results in ${localTime}ms`);

  // If we don't have good local results and cloud is available
  if (localResults.totalCount < 5 && await isCloudAvailable()) {
    console.log('☁️  Trying cloud search for better results...');
    const cloudStart = Date.now();
    const cloudResults = await semanticSearch(query, {
      limit: 10,
      minSimilarity: 0.6,  // Lower threshold for cloud
    });
    const cloudTime = Date.now() - cloudStart;

    console.log(`Cloud search: ${cloudResults.totalCount} results in ${cloudTime}ms`);
    return cloudResults.cloudResults || cloudResults.localResults;
  }

  return localResults.localResults;
}

const results = await progressiveSearch('OAuth2 implementation');
results.forEach(r => console.log(`- ${r.name} (${r.similarity.toFixed(3)})`));
```

### Example 5: Feature-Gated Cloud Integration

```typescript
import {
  semanticSearch,
  getCloudConfig,
  reloadConfig
} from 'codegraph-napi';

class SearchService {
  private cloudEnabled = false;

  async initialize() {
    const config = await getCloudConfig();
    this.cloudEnabled = config.jinaEnabled || config.surrealdbEnabled;
    console.log(`Search service initialized (cloud: ${this.cloudEnabled})`);
  }

  async search(query: string, options: any = {}) {
    const results = await semanticSearch(query, {
      ...options,
      // Override cloud preference if not available
      useCloud: this.cloudEnabled ? options.useCloud : false,
    });

    return {
      results: results.localResults,
      mode: results.modeUsed,
      timeMs: results.searchTimeMs,
      cloudAvailable: this.cloudEnabled,
    };
  }

  async refreshConfig() {
    const changed = await reloadConfig();
    if (changed) {
      const config = await getCloudConfig();
      this.cloudEnabled = config.jinaEnabled || config.surrealdbEnabled;
      console.log('Configuration refreshed');
    }
    return changed;
  }
}

// Usage
const searchService = new SearchService();
await searchService.initialize();

const results = await searchService.search('user authentication');
console.log(`Found ${results.results.length} results using ${results.mode} mode`);
```

## Building for Production

### Build for All Platforms

```bash
npm run build
```

This generates platform-specific binaries:
- `codegraph.darwin-x64.node` - macOS Intel
- `codegraph.darwin-arm64.node` - macOS Apple Silicon
- `codegraph.linux-x64-gnu.node` - Linux x64
- `codegraph.win32-x64-msvc.node` - Windows x64

### Cross-Compilation

```bash
# Build for specific platform
napi build --platform --target x86_64-unknown-linux-musl

# Build universal binary (macOS)
npm run universal
```

## Deployment

### Docker

```dockerfile
FROM node:18-alpine

# Install build dependencies
RUN apk add --no-cache python3 make g++

WORKDIR /app

# Copy package files
COPY package*.json ./

# Install dependencies
RUN npm ci --only=production

# Copy compiled addon
COPY codegraph.linux-x64-musl.node ./

# Copy your app
COPY . .

CMD ["node", "server.js"]
```

### Serverless (AWS Lambda)

```typescript
// lambda/handler.ts
import { createVersion, listVersions } from 'codegraph-napi';

export const handler = async (event) => {
  if (event.action === 'create') {
    const version = await createVersion(JSON.parse(event.body));
    return {
      statusCode: 200,
      body: JSON.stringify(version),
    };
  }

  if (event.action === 'list') {
    const versions = await listVersions(50);
    return {
      statusCode: 200,
      body: JSON.stringify(versions),
    };
  }
};
```

## Error Handling

```typescript
try {
  const version = await createVersion({
    name: 'v1.0',
    description: 'Release',
    author: 'user',
  });
} catch (error) {
  if (error.message.includes('Invalid version ID')) {
    console.error('Bad version ID format');
  } else if (error.message.includes('Version not found')) {
    console.error('Version does not exist');
  } else if (error.message.includes('Failed to create version')) {
    console.error('Creation failed:', error.message);
  } else {
    console.error('Unexpected error:', error);
  }
}
```

## Comparison: Native Addon vs Alternatives

| Feature | Native Addon | CLI Spawning | HTTP API |
|---------|-------------|--------------|----------|
| **Performance** | ⭐⭐⭐⭐⭐ | ⭐⭐ | ⭐⭐⭐ |
| **Setup Complexity** | ⭐⭐⭐ | ⭐⭐⭐⭐⭐ | ⭐⭐ |
| **Memory Overhead** | ⭐⭐⭐⭐⭐ | ⭐⭐ | ⭐⭐⭐ |
| **Type Safety** | ⭐⭐⭐⭐⭐ | ⭐⭐⭐⭐ | ⭐⭐⭐ |
| **Build Complexity** | ⭐⭐ | ⭐⭐⭐⭐⭐ | ⭐⭐⭐⭐ |
| **Deployment** | ⭐⭐⭐ | ⭐⭐⭐⭐⭐ | ⭐⭐⭐ |

**Use Native Addon when:**
- ✅ High-frequency operations (>100 calls/sec)
- ✅ Low-latency requirements (<1ms)
- ✅ Building production APIs
- ✅ Memory efficiency matters

**Use CLI Spawning when:**
- ✅ Simple one-off commands
- ✅ Shell scripting
- ✅ Minimal dependencies
- ✅ Easy debugging needed

**Use HTTP API when:**
- ✅ Multiple languages/services
- ✅ Network distribution
- ✅ Existing HTTP infrastructure

## Troubleshooting

### Module Not Found

```
Error: Cannot find module './codegraph.linux-x64-gnu.node'
```

**Solution:** Rebuild for your platform:
```bash
npm run build
```

### Build Errors

```
error: failed to run custom build command for `codegraph-napi`
```

**Solution:** Install build dependencies:
```bash
# macOS
xcode-select --install

# Ubuntu/Debian
sudo apt-get install build-essential

# Alpine
apk add python3 make g++
```

### Runtime Errors

```
Error: Failed to load config
```

**Solution:** Ensure storage path exists:
```bash
mkdir -p ~/.codegraph
```

## Advanced Usage

### Custom Storage Path

```typescript
// Set environment variable before importing
process.env.CODEGRAPH_STORAGE = '/custom/path';

import { initialize } from 'codegraph-napi';
await initialize();
```

### Concurrent Operations

```typescript
// Safe to call in parallel - uses internal locking
await Promise.all([
  createVersion({ name: 'v1', ... }),
  createVersion({ name: 'v2', ... }),
  createVersion({ name: 'v3', ... }),
]);
```

### Memory Management

```typescript
// The addon uses Arc<Mutex<>> internally
// No manual cleanup needed - garbage collector handles it

const versions = await listVersions(1000000);
// Large arrays are properly freed when out of scope
```

## Development

### Build Debug Version

```bash
npm run build:debug
```

### Run Tests

```bash
npm test
```

### Generate Type Definitions

```bash
# Types are auto-generated during build
npm run build
# Creates: index.d.ts
```

## License

MIT OR Apache-2.0
