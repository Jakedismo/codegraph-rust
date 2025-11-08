# CodeGraph Configuration Guide

## Configuration Directory

CodeGraph uses a centralized configuration directory to store all configuration files in TOML format.

### Default Location: `~/.codegraph`

All CodeGraph configuration files should be placed in `~/.codegraph/` in your home directory. This ensures a uniform, centralized location for all CodeGraph settings across your system.

### Directory Priority

CodeGraph looks for configuration files in the following order:

1. **`~/.codegraph/`** (Primary) - User-level configuration
2. **`./config/`** (Fallback) - Project-level configuration (backward compatibility)
3. **Current directory** (Last resort)

## Initialization

### Creating the Configuration Directory

To initialize the `~/.codegraph` directory:

```rust
use codegraph_core::config::ConfigManager;

// Create directory and copy default config files
let config_dir = ConfigManager::init_user_config_dir(true)?;
println!("Initialized config at: {:?}", config_dir);
```

Or manually:

```bash
mkdir -p ~/.codegraph
cd ~/.codegraph
# Copy your config files here
```

## Configuration Files

### File Loading Order

Configuration files are loaded and merged in this order (later files override earlier ones):

1. `~/.codegraph/default.toml` - Base configuration
2. `~/.codegraph/{environment}.toml` - Environment-specific (e.g., `development.toml`, `production.toml`)
3. `~/.codegraph/local.toml` - Local overrides (git-ignored, machine-specific)
4. Environment variables with `CODEGRAPH__` prefix

### Example Directory Structure

```
~/.codegraph/
├── README.txt              # Documentation
├── default.toml            # Base configuration
├── development.toml        # Development settings
├── production.toml         # Production settings
├── surrealdb.toml          # SurrealDB-specific config
├── embedding.toml          # Embedding model configuration
├── local.toml              # Local overrides (git-ignored)
└── secrets.enc             # Encrypted secrets (optional)
```

## Configuration File Examples

### `~/.codegraph/default.toml`

```toml
env = "development"

[server]
host = "0.0.0.0"
port = 3000

[database]
backend = "rocksdb"  # or "surrealdb"

[database.rocksdb]
path = "data/graph.db"

[database.surrealdb]
connection = "ws://localhost:8000"
namespace = "codegraph"
database = "graph"

[vector]
dimension = 1024

[logging]
level = "info"
```

### `~/.codegraph/production.toml`

```toml
env = "production"

[server]
port = 8080

[database]
backend = "surrealdb"

[database.surrealdb]
connection = "wss://prod-db.example.com:8000"
namespace = "production"
database = "codegraph"
strict_mode = true
auto_migrate = false

[logging]
level = "warn"

[security]
require_auth = true
rate_limit_per_minute = 600
```

### `~/.codegraph/surrealdb.toml`

```toml
# Full SurrealDB configuration example

[database]
backend = "surrealdb"

[database.surrealdb]
connection = "ws://localhost:8000"
namespace = "codegraph"
database = "graph"
username = "admin"
# Password via env: CODEGRAPH__DATABASE__SURREALDB__PASSWORD
strict_mode = false
auto_migrate = true
```

### `~/.codegraph/embedding.toml`

```toml
# Embedding configuration with Jina

[embedding]
provider = "jina"
dimension = 1024
cache_enabled = true
normalize_embeddings = true

[embedding.jina]
model = "jina-embeddings-v4"
api_key_env = "JINA_API_KEY"
task = "code.passage"
late_chunking = true
enable_reranking = true
reranking_model = "jina-reranker-v3"
reranking_top_n = 10
```

### `~/.codegraph/local.toml`

```toml
# Local overrides - NOT tracked in version control
# Machine-specific settings

[server]
port = 3001  # Use different port on this machine

[database.rocksdb]
path = "/mnt/fast-ssd/codegraph/graph.db"  # Custom path

[logging]
level = "debug"  # More verbose logging for development
```

## Environment Variables

Override any configuration value using environment variables with the `CODEGRAPH__` prefix:

```bash
# Database backend
export CODEGRAPH__DATABASE__BACKEND=surrealdb

# SurrealDB password (recommended for secrets)
export CODEGRAPH__DATABASE__SURREALDB__PASSWORD=your_password

# Jina API key
export JINA_API_KEY=your_jina_key

# Server port
export CODEGRAPH__SERVER__PORT=8080

# Logging level
export CODEGRAPH__LOGGING__LEVEL=debug
```

### Environment Variable Syntax

Use double underscores (`__`) to navigate nested configuration:

```toml
[database.surrealdb]
connection = "..."
```

Becomes:

```bash
export CODEGRAPH__DATABASE__SURREALDB__CONNECTION="ws://localhost:8000"
```

## Environment-Specific Configuration

Switch environments using the `APP_ENV` or `RUST_ENV` environment variable:

```bash
# Development (loads ~/.codegraph/development.toml)
export APP_ENV=development
codegraph serve

# Production (loads ~/.codegraph/production.toml)
export APP_ENV=production
codegraph serve

# Staging (loads ~/.codegraph/staging.toml)
export APP_ENV=staging
codegraph serve
```

## Encrypted Secrets

For sensitive data, use encrypted configuration files:

### 1. Generate Encryption Key

```bash
codegraph config generate-key
# Outputs: base64-encoded 32-byte key
```

### 2. Create Secrets File

```toml
# secrets.toml
[secrets]
openai_api_key = "sk-..."
jwt_secret = "your-secret-key"

[database.surrealdb]
password = "db_password"
```

### 3. Encrypt

```bash
export CONFIG_ENC_KEY=<your-base64-key>
codegraph config encrypt ~/.codegraph/secrets.toml
# Creates: ~/.codegraph/secrets.enc
```

### 4. Use Encrypted Secrets

```bash
export CONFIG_ENC_KEY=<your-base64-key>
codegraph serve
# Automatically decrypts and loads secrets.enc
```

## Programmatic Access

### Load Configuration

```rust
use codegraph_core::config::ConfigManager;

// Load with default directory (~/.codegraph)
let manager = ConfigManager::new_watching(None)?;
let settings = manager.settings().read().await;

println!("Server port: {}", settings.server.port);
println!("Database backend: {:?}", settings.database.backend);
```

### Override Environment

```rust
// Load specific environment
let manager = ConfigManager::new_watching(Some("production".to_string()))?;
```

### Custom Config Directory

```rust
use std::path::PathBuf;

let custom_dir = PathBuf::from("/etc/codegraph");
let config_dir = ConfigManager::get_config_dir(Some(custom_dir));
```

## Migration from `./config` to `~/.codegraph`

If you have existing configurations in `./config`, you can migrate:

```bash
# Copy existing configs to ~/.codegraph
mkdir -p ~/.codegraph
cp -r ./config/* ~/.codegraph/

# Or symlink for development
ln -s $(pwd)/config ~/.codegraph
```

CodeGraph will automatically use `~/.codegraph` if it exists, falling back to `./config` for backward compatibility.

## Best Practices

1. **Use `~/.codegraph` for all configs** - Centralized, user-level configuration
2. **Keep `local.toml` for machine-specific overrides** - Don't commit to git
3. **Use environment variables for secrets** - Never commit passwords
4. **Encrypt sensitive files** - Use `secrets.enc` for production
5. **Separate environments** - Use `{env}.toml` files (dev/staging/prod)
6. **Document your configs** - Add comments explaining custom settings
7. **Version control templates** - Keep example configs in your repository

## Troubleshooting

### Config Not Found

```bash
# Check which directory is being used
CODEGRAPH__LOGGING__LEVEL=debug codegraph serve
# Look for: "Using config directory: ..."
```

### Check Current Configuration

```rust
let settings = manager.settings().read().await;
println!("{:#?}", settings);
```

### Validate Configuration

```rust
settings.validate()?;  // Returns error if invalid
```

### Watch for Changes

Configuration files are automatically watched and reloaded when modified (when using `ConfigManager::new_watching()`).

## Further Reading

- [SurrealDB Configuration Guide](./SURREALDB_GUIDE.md)
- [Embedding Configuration](../config/example_embedding.toml)
- [Security Best Practices](./SECURITY.md)
