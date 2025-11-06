# CodeGraph Configuration Files

## Important: Configuration Directory Migration

**As of the latest version, CodeGraph uses `~/.codegraph` as the primary configuration directory.**

### New Location: `~/.codegraph`

All user-level configuration files should now be placed in:

```
~/.codegraph/
```

This provides a centralized, uniform location for all CodeGraph configuration across your system.

### Why the Change?

- **Centralized**: All CodeGraph configs in one place, regardless of project
- **User-level**: Configurations follow you across different projects
- **Standard practice**: Follows Unix/Linux convention for user configuration
- **Cleaner projects**: Keeps project directories focused on code

### Migration

To migrate your existing configurations:

```bash
# Create the directory
mkdir -p ~/.codegraph

# Copy existing configs
cp config/*.toml ~/.codegraph/

# Or symlink for development (keeps backward compatibility)
ln -s $(pwd)/config ~/.codegraph
```

### Backward Compatibility

CodeGraph maintains backward compatibility by checking directories in this order:

1. **`~/.codegraph/`** (Primary)
2. **`./config/`** (This directory - fallback)
3. **Current directory** (last resort)

If `~/.codegraph` exists, it will be used. Otherwise, CodeGraph falls back to `./config/`.

## Configuration Files in This Directory

This directory contains **example configuration files** that can be copied to `~/.codegraph/`:

- `default.toml` - Base configuration example
- `surrealdb_example.toml` - SurrealDB configuration
- `example_embedding.toml` - Embedding provider configuration
- `example_performance.toml` - Performance tuning
- `production.toml` - Production settings example

## Quick Start

### 1. Initialize User Config

```bash
# Create ~/.codegraph with default configs
mkdir -p ~/.codegraph
cp config/default.toml ~/.codegraph/
```

### 2. Customize Your Configuration

```bash
# Edit your user config
nano ~/.codegraph/default.toml

# Or create environment-specific configs
cp ~/.codegraph/default.toml ~/.codegraph/development.toml
cp ~/.codegraph/default.toml ~/.codegraph/production.toml
```

### 3. Set Environment

```bash
export APP_ENV=development  # Loads ~/.codegraph/development.toml
# or
export APP_ENV=production   # Loads ~/.codegraph/production.toml
```

## Environment Variables

Override any config value using environment variables:

```bash
# Database backend
export CODEGRAPH__DATABASE__BACKEND=surrealdb

# SurrealDB connection
export CODEGRAPH__DATABASE__SURREALDB__CONNECTION=ws://localhost:8000

# Server port
export CODEGRAPH__SERVER__PORT=8080
```

## Further Documentation

- **[Full Configuration Guide](../docs/CONFIGURATION_GUIDE.md)** - Complete configuration documentation
- **[SurrealDB Guide](../docs/SURREALDB_GUIDE.md)** - SurrealDB-specific configuration
- **[Environment Variables](../docs/CONFIGURATION_GUIDE.md#environment-variables)** - Full list of env vars

## Development

For development, you can continue using this `./config` directory, but we recommend migrating to `~/.codegraph` for consistency:

```bash
# Option 1: Copy to ~/.codegraph
mkdir -p ~/.codegraph && cp config/*.toml ~/.codegraph/

# Option 2: Symlink (for active development)
ln -s $(pwd)/config ~/.codegraph

# Option 3: Keep using ./config (backward compatible)
# CodeGraph will use ./config if ~/.codegraph doesn't exist
```

## Need Help?

- Check the [Configuration Guide](../docs/CONFIGURATION_GUIDE.md)
- See example configs in this directory
- Use `CODEGRAPH__LOGGING__LEVEL=debug` to see which config directory is being used
