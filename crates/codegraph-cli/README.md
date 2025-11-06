# CodeGraph CLI

A command-line interface for CodeGraph version control and dependency management.

## Installation

### Build from source

```bash
# Build the CLI binary
cargo build --release --bin codegraph

# Install to system path
cargo install --path crates/codegraph-cli
```

### Verify installation

```bash
codegraph --version
```

## Quick Start

```bash
# Check status
codegraph status

# Create a version
codegraph version create \
  --name "v1.0.0" \
  --description "Initial release" \
  --author "user@example.com"

# List versions
codegraph version list

# Create a branch
codegraph branch create \
  --name "feature/new-api" \
  --from <version-id> \
  --author "user@example.com"
```

## Commands

### Transaction Management

```bash
# Begin a transaction
codegraph transaction begin --isolation serializable

# Commit a transaction
codegraph transaction commit <transaction-id>

# Rollback a transaction
codegraph transaction rollback <transaction-id>

# Get transaction statistics
codegraph transaction stats
```

### Version Management

```bash
# Create a version
codegraph version create \
  --name "v1.0.0" \
  --description "Initial release" \
  --author "user@example.com" \
  --parents <parent-id-1>,<parent-id-2>

# List versions (default: 50)
codegraph version list --limit 100

# Get version details
codegraph version get <version-id>

# Tag a version
codegraph version tag <version-id> \
  --tag "stable" \
  --author "user@example.com" \
  --message "Stable release"

# Compare versions
codegraph version compare <from-id> <to-id>
```

### Branch Management

```bash
# Create a branch
codegraph branch create \
  --name "feature/auth" \
  --from <version-id> \
  --author "user@example.com" \
  --description "Authentication feature"

# List branches
codegraph branch list

# Get branch details
codegraph branch get <branch-name>

# Delete a branch
codegraph branch delete <branch-name>

# Merge branches
codegraph branch merge \
  --source "feature/auth" \
  --target "main" \
  --author "user@example.com" \
  --message "Merge authentication feature"
```

## Output Formats

The CLI supports multiple output formats:

```bash
# JSON output (default for programmatic use)
codegraph --output json version list

# Pretty formatted output (human-readable)
codegraph --output pretty version list

# Table output
codegraph --output table version list
```

## Global Options

```bash
--output, -o <format>    Output format: json, pretty, table (default: pretty)
--storage <path>         Storage path for CodeGraph data
--verbose, -v            Enable verbose output
```

## Environment Variables

```bash
CODEGRAPH_STORAGE    Storage path for CodeGraph data
```

## Integration with TypeScript

The CLI is designed to be easily integrated with TypeScript applications:

```typescript
import { CodeGraph } from './sdk/codegraph-cli-wrapper';

const cg = new CodeGraph({
  binaryPath: 'codegraph',  // Optional, defaults to searching PATH
  storagePath: '~/.codegraph',  // Optional
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

console.log(`Created version: ${version.version_id}`);

// List versions
const versions = await cg.listVersions(10);
versions.forEach(v => {
  console.log(`${v.name}: ${v.description}`);
});

// Create a branch
const branch = await cg.createBranch({
  name: 'feature/api',
  from: version.version_id,
  author: 'user@example.com'
});

// Merge branches
const result = await cg.mergeBranches({
  source: 'feature/api',
  target: 'main',
  author: 'user@example.com'
});

if (result.success) {
  console.log('Merge successful!');
} else {
  console.log(`Merge had ${result.conflicts} conflicts`);
}
```

## Examples

### Creating a Release Workflow

```bash
#!/bin/bash

# Start a transaction
TX=$(codegraph --output json transaction begin --isolation serializable | jq -r '.transaction_id')

# Create a version
VERSION=$(codegraph --output json version create \
  --name "v2.0.0" \
  --description "Major release with breaking changes" \
  --author "release-bot@example.com" | jq -r '.version_id')

# Tag the version
codegraph version tag $VERSION \
  --tag "stable" \
  --author "release-bot@example.com" \
  --message "Stable release ready for production"

# Commit the transaction
codegraph transaction commit $TX

echo "Release v2.0.0 created: $VERSION"
```

### Branch and Merge Workflow

```bash
#!/bin/bash

# Get current main branch head
MAIN_HEAD=$(codegraph --output json branch get main | jq -r '.head')

# Create feature branch
codegraph branch create \
  --name "feature/search" \
  --from $MAIN_HEAD \
  --author "dev@example.com" \
  --description "Add search functionality"

# ... do work ...

# Merge back to main
codegraph branch merge \
  --source "feature/search" \
  --target "main" \
  --author "dev@example.com" \
  --message "feat: Add search functionality"
```

## Architecture

The CLI is a thin wrapper around the CodeGraph Rust libraries:

```
┌─────────────────────────────┐
│   codegraph CLI             │
│   (this crate)              │
└──────────┬──────────────────┘
           │
           ├──> codegraph-core
           ├──> codegraph-api (AppState)
           ├──> codegraph-graph (TransactionalGraph)
           └──> codegraph-vector
```

All commands operate on the local storage (default: `~/.codegraph`) using the same transactional graph implementation used by the API server.

## Troubleshooting

### Binary not found

```bash
# Check if codegraph is in PATH
which codegraph

# If not, use full path
/path/to/codegraph version list

# Or add to PATH
export PATH="$PATH:/path/to/cargo/bin"
```

### Storage path issues

```bash
# Specify storage path explicitly
codegraph --storage /custom/path version list

# Or set environment variable
export CODEGRAPH_STORAGE=/custom/path
codegraph version list
```

### Verbose debugging

```bash
# Enable verbose output
codegraph -v version create --name "v1.0" --description "Test" --author "user"
```

## License

MIT OR Apache-2.0
