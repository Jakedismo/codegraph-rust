# TypeScript CLI Integration Guide

This guide explains how to integrate CodeGraph's Rust CLI binary into your TypeScript application.

## Architecture Overview

```
┌────────────────────────────────────────┐
│   Your TypeScript CLI Application     │
│   ├─ import CodeGraph from 'sdk'      │
│   ├─ const cg = new CodeGraph()       │
│   └─ await cg.createVersion(...)      │
└────────────────┬───────────────────────┘
                 │ child_process.spawn()
                 │ (JSON over stdin/stdout)
                 │
┌────────────────▼───────────────────────┐
│   Rust CLI Binary (codegraph)         │
│   ├─ codegraph version create         │
│   ├─ codegraph branch list            │
│   └─ codegraph transaction begin      │
└────────────────────────────────────────┘
```

**Key Benefits:**
- ✅ **No HTTP server needed** - Direct process spawning
- ✅ **Type-safe** - Full TypeScript types for all operations
- ✅ **Standalone** - Rust CLI works independently
- ✅ **Simple IPC** - JSON over stdin/stdout
- ✅ **Fast** - Native Rust performance

## Installation

### 1. Build the Rust CLI

```bash
cd codegraph-rust
cargo build --release --bin codegraph

# Verify it works
./target/release/codegraph --version

# Optional: Install to system PATH
cargo install --path crates/codegraph-cli
```

### 2. Install TypeScript Wrapper

The wrapper is included in the SDK:

```typescript
// sdk/codegraph-cli-wrapper.ts
import CodeGraph from './sdk/codegraph-cli-wrapper';
```

## Quick Start

### Basic Usage

```typescript
import CodeGraph from './sdk/codegraph-cli-wrapper';

const cg = new CodeGraph({
  binaryPath: 'codegraph',  // Or full path: './target/release/codegraph'
  storagePath: '~/.codegraph',
  verbose: false,
  timeout: 30000
});

// Create a version
const version = await cg.createVersion({
  name: 'v1.0.0',
  description: 'Initial release',
  author: 'user@example.com',
  parents: []
});

console.log(`Version created: ${version.version_id}`);
```

### Integration into Your CLI

```typescript
#!/usr/bin/env node
import { Command } from 'commander';
import CodeGraph from './sdk/codegraph-cli-wrapper';

const program = new Command();
const cg = new CodeGraph();

program
  .name('my-cli')
  .description('My CLI with CodeGraph integration')
  .version('1.0.0');

program
  .command('version:create')
  .description('Create a new version')
  .requiredOption('-n, --name <name>', 'Version name')
  .requiredOption('-d, --description <desc>', 'Description')
  .option('-a, --author <author>', 'Author', 'cli-user')
  .action(async (options) => {
    try {
      const version = await cg.createVersion({
        name: options.name,
        description: options.description,
        author: options.author,
        parents: []
      });

      console.log(`✓ Created version ${version.name}`);
      console.log(`  ID: ${version.version_id}`);
    } catch (error) {
      console.error('Error:', error.message);
      process.exit(1);
    }
  });

program
  .command('branch:list')
  .description('List all branches')
  .action(async () => {
    try {
      const branches = await cg.listBranches();

      console.log(`Found ${branches.length} branches:\n`);
      branches.forEach(b => {
        console.log(`  ${b.name}`);
        console.log(`    Head: ${b.head}`);
        console.log(`    By: ${b.created_by}`);
        console.log();
      });
    } catch (error) {
      console.error('Error:', error.message);
      process.exit(1);
    }
  });

program.parse();
```

## API Reference

### Constructor

```typescript
new CodeGraph(options?: CodeGraphOptions)
```

**Options:**
- `binaryPath?: string` - Path to codegraph binary (default: 'codegraph')
- `storagePath?: string` - Storage path for data (default: ~/.codegraph)
- `verbose?: boolean` - Enable verbose output (default: false)
- `timeout?: number` - Command timeout in ms (default: 30000)

### Transaction Management

```typescript
// Begin a transaction
const tx = await cg.beginTransaction('serializable');
// Returns: { transaction_id, isolation_level, status }

// Commit
await cg.commitTransaction(tx.transaction_id);

// Rollback
await cg.rollbackTransaction(tx.transaction_id);

// Get stats
const stats = await cg.getTransactionStats();
// Returns: { active_transactions, committed_transactions, ... }
```

### Version Management

```typescript
// Create version
const version = await cg.createVersion({
  name: 'v1.0.0',
  description: 'Release notes',
  author: 'user@example.com',
  parents: ['parent-id-1', 'parent-id-2']  // Optional
});

// List versions
const versions = await cg.listVersions(50);  // limit optional

// Get version
const version = await cg.getVersion('version-id');

// Tag version
await cg.tagVersion({
  versionId: 'version-id',
  tag: 'stable',
  author: 'user@example.com',
  message: 'Stable release'  // Optional
});

// Compare versions
const diff = await cg.compareVersions('from-id', 'to-id');
// Returns: { added_nodes, modified_nodes, deleted_nodes }
```

### Branch Management

```typescript
// Create branch
const branch = await cg.createBranch({
  name: 'feature/auth',
  from: 'version-id',
  author: 'user@example.com',
  description: 'Authentication feature'  // Optional
});

// List branches
const branches = await cg.listBranches();

// Get branch
const branch = await cg.getBranch('branch-name');

// Delete branch
await cg.deleteBranch('branch-name');

// Merge branches
const result = await cg.mergeBranches({
  source: 'feature/auth',
  target: 'main',
  author: 'user@example.com',
  message: 'Merge auth feature'  // Optional
});

if (result.success) {
  console.log('Merged successfully!');
} else {
  console.log(`${result.conflicts} conflicts detected`);
}
```

## Error Handling

```typescript
try {
  const version = await cg.createVersion({
    name: 'v1.0',
    description: 'Release',
    author: 'user'
  });
} catch (error) {
  if (error.message.includes('Command failed with code')) {
    // CLI execution error
    console.error('CLI error:', error.message);
  } else if (error.message.includes('timed out')) {
    // Timeout error
    console.error('Command timed out');
  } else if (error.message.includes('Failed to spawn')) {
    // Binary not found
    console.error('Codegraph binary not found');
    console.error('Build it with: cargo build --release --bin codegraph');
  } else {
    // Other errors
    console.error('Unexpected error:', error);
  }
}
```

## Advanced Patterns

### Workflow Automation

```typescript
async function releaseWorkflow(version: string) {
  const cg = new CodeGraph();

  // Start transaction
  const tx = await cg.beginTransaction('serializable');

  try {
    // Create version
    const ver = await cg.createVersion({
      name: version,
      description: `Release ${version}`,
      author: 'release-bot',
      parents: []
    });

    // Tag as stable
    await cg.tagVersion({
      versionId: ver.version_id,
      tag: 'stable',
      author: 'release-bot',
      message: 'Production release'
    });

    // Commit transaction
    await cg.commitTransaction(tx.transaction_id);

    return ver;
  } catch (error) {
    // Rollback on error
    await cg.rollbackTransaction(tx.transaction_id);
    throw error;
  }
}

// Usage
const version = await releaseWorkflow('v2.0.0');
console.log(`Released: ${version.version_id}`);
```

### Concurrent Operations

```typescript
// Run multiple operations in parallel
const [versions, branches, stats] = await Promise.all([
  cg.listVersions(10),
  cg.listBranches(),
  cg.getTransactionStats()
]);

console.log(`${versions.length} versions, ${branches.length} branches`);
console.log(`${stats.active_transactions} active transactions`);
```

### Custom Binary Path

```typescript
// Use custom binary location
const cg = new CodeGraph({
  binaryPath: './target/release/codegraph',
  storagePath: './data/.codegraph'
});

// Or from environment
const cg = new CodeGraph({
  binaryPath: process.env.CODEGRAPH_BIN || 'codegraph',
  storagePath: process.env.CODEGRAPH_STORAGE
});
```

## Testing

```typescript
import CodeGraph from './sdk/codegraph-cli-wrapper';

describe('CodeGraph Integration', () => {
  let cg: CodeGraph;

  beforeAll(async () => {
    cg = new CodeGraph({
      storagePath: './test-storage',
      timeout: 10000
    });

    // Check binary availability
    const available = await cg.checkBinary();
    if (!available) {
      throw new Error('codegraph binary not found');
    }
  });

  test('should create version', async () => {
    const version = await cg.createVersion({
      name: 'test-v1',
      description: 'Test version',
      author: 'test@example.com',
      parents: []
    });

    expect(version.version_id).toBeDefined();
    expect(version.name).toBe('test-v1');
  });

  test('should list versions', async () => {
    const versions = await cg.listVersions(10);
    expect(Array.isArray(versions)).toBe(true);
  });
});
```

## Comparison with Alternatives

### CLI Spawning (✅ Recommended)

```typescript
const cg = new CodeGraph();
await cg.createVersion({...});  // Spawns: codegraph version create ...
```

**Pros:**
- Simple, no server needed
- Works offline
- Type-safe wrapper
- Fast for CLI operations

**Cons:**
- Process spawn overhead (~10-50ms)
- No real-time updates

### HTTP API Server

```typescript
const api = axios.create({ baseURL: 'http://localhost:3000' });
await api.post('/versions', {...});
```

**Pros:**
- Can be shared across processes
- RESTful interface

**Cons:**
- Must run server process
- Network overhead
- More complex deployment

### Native Addon (NAPI-RS)

```typescript
import { createVersion } from './codegraph.node';
await createVersion('v1.0', 'Release', 'user');
```

**Pros:**
- Zero IPC overhead
- Fastest option

**Cons:**
- Complex build setup
- Platform-specific binaries
- Harder to debug

## Troubleshooting

### Binary Not Found

```
Error: Failed to spawn codegraph binary: spawn codegraph ENOENT
```

**Solution:**
```typescript
// Use absolute path
const cg = new CodeGraph({
  binaryPath: '/absolute/path/to/codegraph'
});

// Or add to PATH
export PATH="$PATH:./target/release"
```

### Timeout Errors

```
Error: Command timed out after 30000ms
```

**Solution:**
```typescript
const cg = new CodeGraph({
  timeout: 60000  // Increase to 60 seconds
});
```

### JSON Parse Errors

```
Error: Failed to parse JSON output
```

**Solution:** Ensure you're using `--output json`:
```bash
# Test manually
codegraph --output json version list
```

## Performance Tips

1. **Reuse client instance**
   ```typescript
   // Good: Reuse client
   const cg = new CodeGraph();
   await cg.createVersion({...});
   await cg.createBranch({...});

   // Bad: Create new client each time
   await new CodeGraph().createVersion({...});
   await new CodeGraph().createBranch({...});
   ```

2. **Batch operations**
   ```typescript
   // Use Promise.all for independent operations
   await Promise.all([
     cg.listVersions(),
     cg.listBranches(),
     cg.getTransactionStats()
   ]);
   ```

3. **Adjust timeout for long operations**
   ```typescript
   const cg = new CodeGraph({
     timeout: 120000  // 2 minutes for large operations
   });
   ```

## Next Steps

- Review the [CLI documentation](../crates/codegraph-cli/README.md)
- Check out [example usage](../sdk/examples/cli-usage.ts)
- Explore the [API routes](../crates/codegraph-api/src/routes.rs) for HTTP alternative
