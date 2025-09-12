# CodeGraph CLI Reference

Complete command-line interface documentation for CodeGraph CLI MCP Server.

## Table of Contents

1. [Global Options](#global-options)
2. [Commands Overview](#commands-overview)
3. [Command Details](#command-details)
4. [Examples](#examples)
5. [Environment Variables](#environment-variables)
6. [Exit Codes](#exit-codes)

## Global Options

These options can be used with any command:

```bash
codegraph [GLOBAL OPTIONS] <COMMAND> [COMMAND OPTIONS]
```

| Option | Short | Description | Default |
|--------|-------|-------------|---------|
| `--verbose` | `-v` | Enable verbose output. Use multiple times for more verbosity (-vv, -vvv) | Off |
| `--quiet` | `-q` | Suppress all output except errors | Off |
| `--config` | | Path to configuration file | `~/.codegraph/config.toml` |
| `--no-color` | | Disable colored output | Auto-detect |
| `--json` | | Output results in JSON format | Off |
| `--help` | `-h` | Show help information | |
| `--version` | `-V` | Show version information | |

## Commands Overview

| Command | Description |
|---------|-------------|
| `init` | Initialize a new CodeGraph project |
| `start` | Start MCP server with specified transport |
| `stop` | Stop running MCP server |
| `status` | Check MCP server status |
| `index` | Index a project or directory |
| `search` | Search indexed code |
| `config` | Manage configuration |
| `stats` | Show statistics and metrics |
| `clean` | Clean up resources and cache |
| `doctor` | Run diagnostics and health checks |
| `update` | Update CodeGraph components |

## Command Details

### `init` - Initialize Project

Initialize a new CodeGraph project in the specified directory.

```bash
codegraph init [OPTIONS] [PATH]
```

#### Arguments

- `PATH` - Project directory path (default: current directory)

#### Options

| Option | Description | Default |
|--------|-------------|---------|
| `--name <NAME>` | Project name | Directory name |
| `--description <DESC>` | Project description | None |
| `--languages <LANGS>` | Comma-separated list of languages | Auto-detect |
| `--template <TEMPLATE>` | Use project template (minimal, standard, full) | standard |
| `--non-interactive` | Skip interactive prompts | false |
| `--force` | Overwrite existing configuration | false |

#### Examples

```bash
# Initialize in current directory
codegraph init

# Initialize with project name
codegraph init --name "my-project"

# Initialize with specific languages
codegraph init --languages rust,python,typescript

# Non-interactive initialization
codegraph init --non-interactive --name "api-service"
```

---

### `start` - Start MCP Server

Start the MCP server with the specified transport protocol.

```bash
codegraph start <TRANSPORT> [OPTIONS]
```

#### Transports

- `stdio` - Standard I/O transport (for IDE integration)
- `http` - HTTP streaming transport with SSE
- `dual` - Both STDIO and HTTP simultaneously

#### Common Options

| Option | Description | Default |
|--------|-------------|---------|
| `--config <PATH>` | Server configuration file | Auto-detect |
| `--daemon` | Run server in background | false |
| `--pid-file <PATH>` | PID file location for daemon mode | `~/.codegraph/codegraph.pid` |
| `--log-file <PATH>` | Log file location | `~/.codegraph/logs/server.log` |
| `--timeout <SECONDS>` | Server startup timeout | 30 |

#### STDIO Transport Options

| Option | Description | Default |
|--------|-------------|---------|
| `--buffer-size <BYTES>` | I/O buffer size | 8192 |
| `--line-buffered` | Use line buffering | false |

#### HTTP Transport Options

| Option | Short | Description | Default |
|--------|-------|-------------|---------|
| `--host <HOST>` | `-h` | Host address to bind | 127.0.0.1 |
| `--port <PORT>` | `-p` | Port number | 3000 |
| `--tls` | | Enable TLS/HTTPS | false |
| `--cert <PATH>` | | TLS certificate file | None |
| `--key <PATH>` | | TLS private key file | None |
| `--cors` | | Enable CORS | true |
| `--cors-origins <ORIGINS>` | | Allowed CORS origins | * |
| `--max-connections <N>` | | Maximum concurrent connections | 100 |

#### Examples

```bash
# Start with STDIO transport
codegraph start stdio

# Start HTTP server
codegraph start http --port 8080

# Start with TLS
codegraph start http --tls --cert cert.pem --key key.pem

# Start in daemon mode
codegraph start http --daemon --pid-file /var/run/codegraph.pid

# Start dual transport
codegraph start dual --port 3000
```

---

### `stop` - Stop Server

Stop a running CodeGraph MCP server.

```bash
codegraph stop [OPTIONS]
```

#### Options

| Option | Short | Description | Default |
|--------|-------|-------------|---------|
| `--pid-file <PATH>` | | PID file location | `~/.codegraph/codegraph.pid` |
| `--force` | `-f` | Force stop without graceful shutdown | false |
| `--timeout <SECONDS>` | | Shutdown timeout | 10 |
| `--all` | | Stop all running instances | false |

#### Examples

```bash
# Stop server gracefully
codegraph stop

# Force stop
codegraph stop --force

# Stop specific instance
codegraph stop --pid-file /var/run/codegraph.pid

# Stop all instances
codegraph stop --all
```

---

### `status` - Server Status

Check the status of CodeGraph MCP server.

```bash
codegraph status [OPTIONS]
```

#### Options

| Option | Short | Description | Default |
|--------|-------|-------------|---------|
| `--pid-file <PATH>` | | PID file location | `~/.codegraph/codegraph.pid` |
| `--detailed` | `-d` | Show detailed status information | false |
| `--metrics` | | Include performance metrics | false |
| `--format <FORMAT>` | | Output format (text, json, yaml) | text |
| `--watch` | `-w` | Continuously monitor status | false |
| `--interval <SECONDS>` | | Watch interval | 2 |

#### Examples

```bash
# Basic status check
codegraph status

# Detailed status
codegraph status --detailed

# Watch status
codegraph status --watch --interval 5

# JSON output
codegraph status --format json
```

---

### `index` - Index Project

Index a project directory for code analysis.

```bash
codegraph index <PATH> [OPTIONS]
```

#### Arguments

- `PATH` - Path to project directory

#### Options

| Option | Short | Description | Default |
|--------|-------|-------------|---------|
| `--languages <LANGS>` | `-l` | Languages to index (comma-separated) | Auto-detect |
| `--exclude <PATTERNS>` | | Exclude patterns (gitignore format) | `.gitignore` |
| `--include <PATTERNS>` | | Include only these patterns | All files |
| `--recursive` | `-r` | Recursively index subdirectories | true |
| `--force` | | Force reindex even if up-to-date | false |
| `--watch` | `-w` | Watch for changes and auto-reindex | false |
| `--workers <N>` | | Number of parallel workers | CPU count |
| `--batch-size <N>` | | Files per batch | 100 |
| `--max-file-size <SIZE>` | | Maximum file size (e.g., 10MB) | 50MB |
| `--follow-symlinks` | | Follow symbolic links | false |
| `--ignore-hidden` | | Ignore hidden files and directories | true |
| `--progress` | | Show progress bar | true |

#### Examples

```bash
# Index current directory
codegraph index .

# Index with specific languages
codegraph index /path/to/project --languages rust,python

# Index with watch mode
codegraph index . --watch

# Force reindex with custom workers
codegraph index . --force --workers 16

# Index with custom patterns
codegraph index . --include "src/**/*.rs" --exclude "tests/**"
```

---

### `search` - Search Code

Search indexed code using various search strategies.

```bash
codegraph search <QUERY> [OPTIONS]
```

#### Arguments

- `QUERY` - Search query string

#### Options

| Option | Short | Description | Default |
|--------|-------|-------------|---------|
| `--type <TYPE>` | `-t` | Search type (semantic, exact, fuzzy, regex, ast) | semantic |
| `--limit <N>` | `-l` | Maximum results | 10 |
| `--offset <N>` | | Results offset for pagination | 0 |
| `--threshold <FLOAT>` | | Similarity threshold (0.0-1.0) | 0.7 |
| `--project <NAME>` | `-p` | Search specific project | All projects |
| `--language <LANG>` | | Filter by language | All languages |
| `--file-pattern <PATTERN>` | | Filter by file pattern | All files |
| `--format <FORMAT>` | `-f` | Output format (human, json, yaml, table) | human |
| `--context <N>` | `-c` | Lines of context around matches | 2 |
| `--highlight` | | Highlight matches in output | true |
| `--case-sensitive` | | Case-sensitive search | false |
| `--whole-word` | | Match whole words only | false |

#### Search Types

- `semantic` - Vector similarity search using embeddings
- `exact` - Exact string matching
- `fuzzy` - Fuzzy string matching with typo tolerance
- `regex` - Regular expression search
- `ast` - Abstract Syntax Tree pattern matching

#### Examples

```bash
# Semantic search
codegraph search "authentication handler"

# Exact match search
codegraph search "fn process_data" --type exact

# Regex search with context
codegraph search "async\s+fn\s+\w+" --type regex --context 5

# Search in specific project
codegraph search "TODO" --project my-api --limit 50

# AST pattern search
codegraph search "function with async await" --type ast

# JSON output
codegraph search "database connection" --format json
```

---

### `config` - Configuration Management

Manage CodeGraph configuration settings.

```bash
codegraph config <ACTION> [OPTIONS]
```

#### Actions

- `show` - Display current configuration
- `get <KEY>` - Get specific configuration value
- `set <KEY> <VALUE>` - Set configuration value
- `unset <KEY>` - Remove configuration value
- `reset` - Reset to default configuration
- `validate` - Validate configuration
- `edit` - Open configuration in editor

#### Options

| Option | Description | Default |
|--------|-------------|---------|
| `--global` | Use global configuration | false |
| `--local` | Use local project configuration | false |
| `--json` | Output as JSON | false |
| `--yes` | Skip confirmation prompts | false |

#### Configuration Keys

Common configuration keys:

- `general.log_level` - Logging level (trace, debug, info, warn, error)
- `indexing.languages` - Default languages to index
- `indexing.workers` - Number of indexing workers
- `embedding.model` - Embedding model (openai, local, custom)
- `embedding.dimension` - Vector dimension
- `server.default_transport` - Default server transport
- `server.http_port` - Default HTTP port
- `performance.memory_limit_mb` - Memory limit in MB

#### Examples

```bash
# Show all configuration
codegraph config show

# Show as JSON
codegraph config show --json

# Get specific value
codegraph config get embedding.model

# Set configuration value
codegraph config set indexing.workers 8
codegraph config set embedding.model local

# Reset configuration
codegraph config reset --yes

# Edit configuration
codegraph config edit

# Validate configuration
codegraph config validate
```

---

### `stats` - Statistics

Display statistics and metrics about indexed projects.

```bash
codegraph stats [OPTIONS]
```

#### Options

| Option | Description | Default |
|--------|-------------|---------|
| `--index` | Show indexing statistics | false |
| `--server` | Show server statistics | false |
| `--performance` | Show performance metrics | false |
| `--all` | Show all statistics | false |
| `--project <NAME>` | Statistics for specific project | All projects |
| `--format <FORMAT>` | Output format (table, json, yaml, csv) | table |
| `--period <PERIOD>` | Time period (hour, day, week, month) | all-time |
| `--export <FILE>` | Export statistics to file | None |

#### Examples

```bash
# Show all statistics
codegraph stats --all

# Index statistics
codegraph stats --index

# Server performance metrics
codegraph stats --server --performance

# Project-specific stats
codegraph stats --project my-api --format json

# Export to CSV
codegraph stats --all --format csv --export stats.csv
```

---

### `clean` - Clean Resources

Clean up CodeGraph resources and cache.

```bash
codegraph clean [OPTIONS]
```

#### Options

| Option | Description | Default |
|--------|-------------|---------|
| `--index` | Clean index database | false |
| `--vectors` | Clean vector embeddings | false |
| `--cache` | Clean cache files | false |
| `--logs` | Clean log files | false |
| `--temp` | Clean temporary files | false |
| `--all` | Clean all resources | false |
| `--older-than <DAYS>` | Clean files older than N days | None |
| `--dry-run` | Show what would be cleaned | false |
| `--yes` | Skip confirmation prompt | false |

#### Examples

```bash
# Clean all resources
codegraph clean --all --yes

# Clean specific resources
codegraph clean --cache --logs

# Dry run
codegraph clean --all --dry-run

# Clean old files
codegraph clean --logs --older-than 30
```

---

### `doctor` - Diagnostics

Run diagnostics and health checks.

```bash
codegraph doctor [OPTIONS]
```

#### Options

| Option | Description | Default |
|--------|-------------|---------|
| `--verbose` | Show detailed diagnostics | false |
| `--fix` | Attempt to fix issues | false |
| `--system-info` | Include system information | false |
| `--check <CHECK>` | Run specific check | All checks |

#### Available Checks

- `rust` - Rust installation and version
- `dependencies` - System dependencies
- `config` - Configuration validity
- `database` - Database connectivity
- `permissions` - File permissions
- `network` - Network connectivity
- `memory` - Memory availability
- `disk` - Disk space

#### Examples

```bash
# Run all diagnostics
codegraph doctor

# Verbose diagnostics
codegraph doctor --verbose --system-info

# Fix issues
codegraph doctor --fix

# Specific check
codegraph doctor --check database
```

---

### `update` - Update CodeGraph

Update CodeGraph components.

```bash
codegraph update [OPTIONS]
```

#### Options

| Option | Description | Default |
|--------|-------------|---------|
| `--check` | Check for updates only | false |
| `--prerelease` | Include prerelease versions | false |
| `--force` | Force update even if up-to-date | false |
| `--version <VERSION>` | Update to specific version | Latest |

#### Examples

```bash
# Check for updates
codegraph update --check

# Update to latest
codegraph update

# Update to specific version
codegraph update --version 1.2.0
```

## Examples

### Complete Workflow Examples

#### Initial Setup

```bash
# Install and initialize
cargo install codegraph-mcp
codegraph init --name my-project

# Configure for your needs
codegraph config set embedding.model local
codegraph config set performance.workers 8

# Index your codebase
codegraph index . --languages rust,python
```

#### Development Workflow

```bash
# Start server in background
codegraph start http --daemon --port 8080

# Index with watching
codegraph index . --watch &

# Search while developing
codegraph search "TODO" --type exact
codegraph search "error handling" --type semantic

# Check status
codegraph status --detailed
```

#### CI/CD Integration

```bash
#!/bin/bash
# CI script example

# Index codebase
codegraph index . --force --workers 16

# Run analysis
codegraph stats --all --format json > metrics.json

# Search for issues
codegraph search "FIXME|TODO|HACK" --type regex --format json > issues.json

# Check for security patterns
codegraph search "password|secret|token" --type fuzzy --limit 100
```

## Environment Variables

CodeGraph respects the following environment variables:

| Variable | Description | Default |
|----------|-------------|---------|
| `CODEGRAPH_HOME` | CodeGraph home directory | `~/.codegraph` |
| `CODEGRAPH_CONFIG` | Configuration file path | `$CODEGRAPH_HOME/config.toml` |
| `CODEGRAPH_LOG_LEVEL` | Log level | info |
| `CODEGRAPH_LOG_FILE` | Log file path | `$CODEGRAPH_HOME/logs/codegraph.log` |
| `CODEGRAPH_DB_PATH` | Database path | `$CODEGRAPH_HOME/db` |
| `CODEGRAPH_CACHE_DIR` | Cache directory | `$CODEGRAPH_HOME/cache` |
| `CODEGRAPH_WORKERS` | Default worker count | CPU count |
| `CODEGRAPH_MEMORY_LIMIT` | Memory limit in MB | 4096 |
| `CODEGRAPH_HTTP_PORT` | Default HTTP port | 3000 |
| `CODEGRAPH_NO_COLOR` | Disable colored output | false |
| `RUST_LOG` | Rust log configuration | info |
| `RUST_BACKTRACE` | Show backtrace on panic | 0 |

### API Keys

| Variable | Description |
|----------|-------------|
| `OPENAI_API_KEY` | OpenAI API key for embeddings |
| `ANTHROPIC_API_KEY` | Anthropic API key |
| `HUGGINGFACE_TOKEN` | Hugging Face API token |

## Exit Codes

CodeGraph uses standard exit codes:

| Code | Description |
|------|-------------|
| 0 | Success |
| 1 | General error |
| 2 | Misuse of command |
| 3 | Configuration error |
| 4 | Database error |
| 5 | Network error |
| 6 | Permission denied |
| 7 | Resource not found |
| 8 | Operation timeout |
| 9 | User cancelled |
| 10 | Update available |
| 127 | Command not found |
| 130 | Interrupted (Ctrl+C) |

## Shell Completion

Generate shell completion scripts:

```bash
# Bash
codegraph completions bash > ~/.local/share/bash-completion/completions/codegraph

# Zsh
codegraph completions zsh > ~/.zfunc/_codegraph

# Fish
codegraph completions fish > ~/.config/fish/completions/codegraph.fish

# PowerShell
codegraph completions powershell > $PROFILE
```

---

## See Also

- [Installation Guide](./INSTALLATION.md)
- [Configuration Guide](./CONFIGURATION.md)
- [User Workflows](./WORKFLOWS.md)
- [Troubleshooting](./TROUBLESHOOTING.md)