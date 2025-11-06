# CodeGraph Native Addon (NAPI-RS)

**Zero-overhead TypeScript integration** - Direct function calls to Rust, no process spawning, no HTTP.

## Features

⚡ **Maximum Performance** - Direct FFI calls, no IPC overhead
🎯 **Type-Safe** - Auto-generated TypeScript definitions
🔒 **Memory-Safe** - Rust's safety guarantees
🌍 **Cross-Platform** - Windows, macOS, Linux (x64, ARM64)
📦 **Easy Integration** - Drop-in npm package

## Installation

### Option 1: Build from Source

```bash
# Install NAPI-RS CLI
npm install -g @napi-rs/cli

# Build the addon
cd crates/codegraph-napi
npm install
npm run build

# The compiled addon will be in ./codegraph.*.node
```

### Option 2: Pre-built Binaries (Coming Soon)

```bash
npm install codegraph
```

## Quick Start

```typescript
import {
  createVersion,
  listVersions,
  createBranch,
  mergeBranches,
} from 'codegraph';

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
import { initialize, getVersion } from 'codegraph';

// Optional - initializes automatically on first call
await initialize();

// Get addon version
const version = getVersion();
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
import { createVersion, listVersions } from 'codegraph';

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
import { createVersion, listVersions } from 'codegraph';

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
import { createVersion, mergeBranches } from 'codegraph';

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
import { createVersion, listVersions } from 'codegraph';

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

import { initialize } from 'codegraph';
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
