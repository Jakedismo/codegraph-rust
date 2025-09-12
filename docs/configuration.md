# CodeGraph Configuration Guide

Comprehensive guide to configuring CodeGraph CLI MCP Server for optimal performance and customization.

## Table of Contents

1. [Configuration Overview](#configuration-overview)
2. [Configuration File Structure](#configuration-file-structure)
3. [Configuration Sections](#configuration-sections)
4. [Environment Variables](#environment-variables)
5. [Configuration Templates](#configuration-templates)
6. [Advanced Configuration](#advanced-configuration)
7. [Performance Tuning](#performance-tuning)
8. [Security Configuration](#security-configuration)
9. [Troubleshooting Configuration](#troubleshooting-configuration)

## Configuration Overview

CodeGraph uses a hierarchical configuration system with multiple sources:

1. **Default Configuration** - Built-in defaults
2. **Global Configuration** - `~/.codegraph/config.toml`
3. **Project Configuration** - `.codegraph/config.toml` in project root
4. **Environment Variables** - Override any setting
5. **Command-line Arguments** - Highest priority

### Configuration Priority

```
Command-line > Environment > Project > Global > Default
```

### Configuration File Format

CodeGraph uses TOML format for configuration files:

```toml
# This is a comment
[section]
key = "value"
number = 42
boolean = true
array = ["item1", "item2"]

[section.subsection]
nested_key = "nested_value"
```

## Configuration File Structure

### Complete Configuration Template

```toml
# CodeGraph Configuration File
# Version: 1.0.0

[general]
# Project identification
project_name = "my-project"
project_version = "1.0.0"
description = "My awesome project"

# Logging configuration
log_level = "info"  # trace, debug, info, warn, error
log_format = "pretty"  # pretty, json, compact
log_file = "~/.codegraph/logs/codegraph.log"
log_rotation = "daily"  # daily, size, never
log_max_size = "100MB"
log_max_age = 30  # days
log_compress = true

# General behavior
color_output = true
progress_bars = true
confirm_destructive = true
auto_update_check = true

[indexing]
# Languages to index
languages = [
    "rust",
    "python",
    "javascript",
    "typescript",
    "go",
    "java",
    "cpp",
    "c",
    "csharp"
]

# File patterns
exclude_patterns = [
    "**/node_modules/**",
    "**/target/**",
    "**/.git/**",
    "**/build/**",
    "**/dist/**",
    "**/__pycache__/**",
    "**/*.pyc",
    "**/.venv/**"
]

include_patterns = [
    "src/**",
    "lib/**",
    "tests/**",
    "examples/**"
]

# Indexing behavior
recursive = true
follow_symlinks = false
ignore_hidden = true
respect_gitignore = true
incremental = true
watch_enabled = false
watch_debounce_ms = 500

# Performance
workers = 8  # 0 = auto-detect
batch_size = 100
max_file_size = "50MB"
chunk_size = 4096
parallel_parsing = true

[embedding]
# Embedding model configuration
model = "openai"  # openai, local, anthropic, cohere, custom
provider = "openai"
dimension = 1536
batch_size = 100
max_tokens = 8192

# Caching
cache_enabled = true
cache_size_mb = 500
cache_ttl_hours = 168  # 1 week
cache_path = "~/.codegraph/cache/embeddings"

# Model-specific settings
[embedding.openai]
api_key = "${OPENAI_API_KEY}"  # Environment variable
model_name = "text-embedding-3-large"
organization = ""
base_url = "https://api.openai.com/v1"
timeout = 30
max_retries = 3
retry_delay = 1

[embedding.local]
model_path = "~/.codegraph/models/codestral.gguf"
model_type = "gguf"  # gguf, onnx, pytorch
device = "cpu"  # cpu, cuda, metal, rocm
threads = 8
context_length = 8192
gpu_layers = 0  # Number of layers to offload to GPU

[embedding.anthropic]
api_key = "${ANTHROPIC_API_KEY}"
model_name = "claude-3-sonnet"
max_tokens = 100000

[vector]
# Vector index configuration
index_type = "hnsw"  # flat, ivf, hnsw, lsh
metric = "cosine"  # cosine, euclidean, inner_product

# Index-specific parameters
[vector.flat]
# No additional parameters for flat index

[vector.ivf]
nlist = 100  # Number of clusters
nprobe = 10  # Number of clusters to search

[vector.hnsw]
m = 16  # Number of bi-directional links
ef_construction = 200  # Size of dynamic candidate list
ef_search = 64  # Size of dynamic candidate list for search
max_elements = 1000000

[vector.lsh]
n_bits = 8
n_tables = 16

# Search parameters
search_k = 100  # Number of nearest neighbors to retrieve
rerank = true
rerank_k = 10

[database]
# Database configuration
type = "rocksdb"  # rocksdb, sqlite, postgres
path = "~/.codegraph/db"

# Connection settings
[database.rocksdb]
create_if_missing = true
max_open_files = 1000
cache_size_mb = 512
write_buffer_size_mb = 64
max_write_buffer_number = 3
compression = "zstd"  # none, snappy, zlib, bzip2, lz4, zstd
compression_level = 3
bloom_filter_bits = 10
block_size_kb = 16

[database.sqlite]
path = "~/.codegraph/codegraph.db"
journal_mode = "wal"
synchronous = "normal"
cache_size = -64000  # 64MB
busy_timeout = 5000

[database.postgres]
host = "localhost"
port = 5432
database = "codegraph"
username = "codegraph"
password = "${POSTGRES_PASSWORD}"
ssl_mode = "prefer"  # disable, prefer, require
pool_size = 10
connection_timeout = 30

[server]
# Server configuration
default_transport = "stdio"  # stdio, http, dual

[server.stdio]
buffer_size = 8192
line_buffered = false
read_timeout = 0  # 0 = no timeout
write_timeout = 0

[server.http]
host = "127.0.0.1"
port = 3000
base_path = ""
worker_threads = 4

# TLS configuration
tls_enabled = false
tls_cert = ""
tls_key = ""
tls_ca = ""
tls_client_auth = false

# CORS configuration
cors_enabled = true
cors_origins = ["*"]
cors_methods = ["GET", "POST", "PUT", "DELETE", "OPTIONS"]
cors_headers = ["Content-Type", "Authorization"]
cors_credentials = false
cors_max_age = 3600

# Request limits
max_request_size = "10MB"
max_connections = 1000
connection_timeout = 30
keep_alive = true
keep_alive_timeout = 75

# Rate limiting
rate_limit_enabled = true
rate_limit_requests = 100
rate_limit_window = 60  # seconds
rate_limit_burst = 10

[server.websocket]
enabled = true
ping_interval = 30
pong_timeout = 10
max_frame_size = "1MB"
max_message_size = "10MB"

[mcp]
# Model Context Protocol configuration
enabled = true
version = "1.0"
capabilities = [
    "tools",
    "resources",
    "prompts",
    "completions"
]

# MCP server settings
max_request_size = "50MB"
timeout = 60
batch_requests = true
batch_max_size = 10

# Tool definitions
[mcp.tools.analyze_code]
name = "analyze_code"
description = "Analyze code structure and patterns"
parameters_schema = "schemas/analyze_code.json"

[mcp.tools.find_similar]
name = "find_similar"
description = "Find similar code snippets"
parameters_schema = "schemas/find_similar.json"

[performance]
# Performance tuning
max_memory_mb = 4096
gc_interval_seconds = 300
gc_threshold_mb = 1024

# Thread pool settings
thread_pool_size = 16
async_runtime_threads = 4
blocking_threads = 8

# Query optimization
query_timeout_seconds = 30
query_cache_enabled = true
query_cache_size = 1000
query_cache_ttl = 3600

# Batch processing
batch_timeout_ms = 100
batch_max_size = 1000

[monitoring]
# Monitoring and metrics
enabled = true
metrics_endpoint = "/metrics"
metrics_format = "prometheus"  # prometheus, json

# Metrics collection
collect_system_metrics = true
collect_process_metrics = true
collect_custom_metrics = true
metrics_interval = 10  # seconds

# Health checks
health_endpoint = "/health"
liveness_endpoint = "/liveness"
readiness_endpoint = "/readiness"

# Tracing
tracing_enabled = false
tracing_backend = "jaeger"  # jaeger, zipkin, otlp
tracing_endpoint = "http://localhost:14268/api/traces"
tracing_sample_rate = 0.1

[security]
# Security configuration
api_key_required = false
api_key_header = "X-API-Key"
api_keys = []  # List of valid API keys

# Authentication
auth_enabled = false
auth_provider = "jwt"  # jwt, oauth2, basic
jwt_secret = "${JWT_SECRET}"
jwt_expiry = 3600
jwt_algorithm = "HS256"

# Authorization
rbac_enabled = false
default_role = "user"
admin_users = []

# Security headers
security_headers = true
csp_policy = "default-src 'self'"
hsts_enabled = true
hsts_max_age = 31536000

[backup]
# Backup configuration
enabled = true
schedule = "0 2 * * *"  # Cron expression
retention_days = 30
compress = true
encrypt = false
encryption_key = "${BACKUP_ENCRYPTION_KEY}"

# Backup destinations
[backup.local]
enabled = true
path = "~/.codegraph/backups"

[backup.s3]
enabled = false
bucket = "codegraph-backups"
region = "us-east-1"
access_key = "${AWS_ACCESS_KEY_ID}"
secret_key = "${AWS_SECRET_ACCESS_KEY}"
prefix = "backups/"

[backup.gcs]
enabled = false
bucket = "codegraph-backups"
credentials_path = "~/.codegraph/gcs-credentials.json"

[experimental]
# Experimental features (use with caution)
enable_gpu_acceleration = false
enable_distributed_indexing = false
enable_quantum_embeddings = false
enable_neural_search = false
```

## Configuration Sections

### General Section

Controls overall application behavior and logging.

```toml
[general]
log_level = "info"  # Verbosity: trace < debug < info < warn < error
log_format = "json"  # Output format for structured logging
color_output = true  # Enable colored terminal output
```

### Indexing Section

Configure how CodeGraph indexes your codebase.

```toml
[indexing]
languages = ["rust", "python"]  # Languages to process
workers = 8  # Parallel workers (0 = CPU count)
incremental = true  # Only index changed files
watch_enabled = true  # Auto-reindex on changes
```

### Embedding Section

Configure embedding models for semantic search.

```toml
[embedding]
model = "openai"
dimension = 1536

[embedding.openai]
api_key = "${OPENAI_API_KEY}"
model_name = "text-embedding-3-large"
```

### Vector Section

Configure vector index for similarity search.

```toml
[vector]
index_type = "hnsw"  # Algorithm: flat, ivf, hnsw
metric = "cosine"  # Distance metric

[vector.hnsw]
m = 16  # HNSW connectivity
ef_construction = 200  # Construction quality
```

### Database Section

Configure persistent storage.

```toml
[database]
type = "rocksdb"
path = "~/.codegraph/db"

[database.rocksdb]
cache_size_mb = 512
compression = "zstd"
```

### Server Section

Configure MCP server transports.

```toml
[server]
default_transport = "http"

[server.http]
host = "0.0.0.0"
port = 8080
tls_enabled = true
```

## Environment Variables

### System Environment Variables

Override configuration via environment:

```bash
# General
export CODEGRAPH_HOME="/opt/codegraph"
export CODEGRAPH_CONFIG="/etc/codegraph/config.toml"
export CODEGRAPH_LOG_LEVEL="debug"

# Performance
export CODEGRAPH_WORKERS=16
export CODEGRAPH_MEMORY_LIMIT=8192

# Server
export CODEGRAPH_HTTP_HOST="0.0.0.0"
export CODEGRAPH_HTTP_PORT=8080

# Database
export CODEGRAPH_DB_PATH="/var/lib/codegraph/db"
export CODEGRAPH_DB_CACHE_SIZE=1024

# Vector search
export CODEGRAPH_VECTOR_INDEX="hnsw"
export CODEGRAPH_VECTOR_DIMENSION=768
```

### API Keys and Secrets

```bash
# Embedding providers
export OPENAI_API_KEY="sk-..."
export ANTHROPIC_API_KEY="sk-ant-..."
export COHERE_API_KEY="..."
export HUGGINGFACE_TOKEN="hf_..."

# Cloud storage
export AWS_ACCESS_KEY_ID="..."
export AWS_SECRET_ACCESS_KEY="..."
export GCS_CREDENTIALS="/path/to/credentials.json"

# Security
export JWT_SECRET="your-secret-key"
export API_KEY="your-api-key"
export BACKUP_ENCRYPTION_KEY="..."
```

## Configuration Templates

### Minimal Configuration

For quick start and testing:

```toml
[general]
project_name = "test"

[indexing]
languages = ["python", "javascript"]

[embedding]
model = "local"

[server]
default_transport = "stdio"
```

### Development Configuration

For local development:

```toml
[general]
log_level = "debug"
color_output = true

[indexing]
workers = 4
watch_enabled = true
incremental = true

[embedding]
model = "local"
cache_enabled = true

[server.http]
host = "localhost"
port = 3000
cors_enabled = true

[performance]
max_memory_mb = 2048
```

### Production Configuration

For production deployments:

```toml
[general]
log_level = "info"
log_format = "json"
log_file = "/var/log/codegraph/app.log"

[indexing]
workers = 16
batch_size = 500

[embedding]
model = "openai"
cache_enabled = true
cache_size_mb = 2048

[database.rocksdb]
cache_size_mb = 4096
compression = "zstd"
compression_level = 6

[server.http]
host = "0.0.0.0"
port = 8080
tls_enabled = true
rate_limit_enabled = true

[security]
api_key_required = true
auth_enabled = true

[monitoring]
enabled = true
tracing_enabled = true

[backup]
enabled = true
schedule = "0 2 * * *"
```

### High-Performance Configuration

For maximum performance:

```toml
[indexing]
workers = 32
batch_size = 1000
parallel_parsing = true

[embedding]
batch_size = 500
cache_size_mb = 8192

[vector.hnsw]
m = 32
ef_construction = 400
ef_search = 128

[database.rocksdb]
cache_size_mb = 8192
max_open_files = 5000
write_buffer_size_mb = 256

[performance]
max_memory_mb = 16384
thread_pool_size = 64
query_cache_size = 10000
```

## Advanced Configuration

### Multi-Project Setup

Configure multiple projects:

```toml
[[projects]]
name = "frontend"
path = "/code/frontend"
languages = ["typescript", "javascript"]

[[projects]]
name = "backend"
path = "/code/backend"
languages = ["rust", "python"]

[[projects]]
name = "mobile"
path = "/code/mobile"
languages = ["swift", "kotlin"]
```

### Custom Language Parsers

Add custom language support:

```toml
[[languages.custom]]
name = "solidity"
extensions = [".sol"]
parser = "tree-sitter-solidity"
grammar_path = "~/.codegraph/grammars/solidity.so"

[[languages.custom]]
name = "move"
extensions = [".move"]
parser = "custom-move-parser"
```

### Distributed Configuration

For distributed deployments:

```toml
[cluster]
enabled = true
node_id = "node-1"
coordinator = "coordinator.example.com:9000"

[cluster.nodes]
"node-1" = { host = "10.0.1.1", port = 9001 }
"node-2" = { host = "10.0.1.2", port = 9001 }
"node-3" = { host = "10.0.1.3", port = 9001 }

[cluster.sharding]
strategy = "consistent_hash"
replicas = 2
```

## Performance Tuning

### Memory Optimization

```toml
[performance]
# Limit memory usage
max_memory_mb = 4096
gc_interval_seconds = 60
gc_threshold_mb = 512

# Optimize allocations
string_pool_size = 10000
buffer_pool_size = 100
```

### CPU Optimization

```toml
[performance]
# Thread configuration
thread_pool_size = 16  # General threads
async_runtime_threads = 4  # Tokio threads
blocking_threads = 8  # Blocking I/O threads

# CPU affinity
cpu_affinity = [0, 1, 2, 3]  # Pin to specific cores
```

### I/O Optimization

```toml
[performance]
# Disk I/O
io_buffer_size = 65536
prefetch_size = 10
write_batch_size = 1000

# Network I/O
tcp_nodelay = true
tcp_keepalive = 60
socket_buffer_size = 262144
```

### Cache Optimization

```toml
[cache]
# Multi-level caching
l1_size_mb = 128  # Fast, small
l2_size_mb = 1024  # Medium
l3_size_mb = 8192  # Large, slower

# Cache strategies
eviction_policy = "lru"  # lru, lfu, fifo
ttl_seconds = 3600
warm_up = true
```

## Security Configuration

### API Security

```toml
[security]
# API key authentication
api_key_required = true
api_key_header = "X-API-Key"
api_keys = [
    "key1_hash",
    "key2_hash"
]

# Rate limiting
rate_limit_enabled = true
rate_limit_by = "ip"  # ip, api_key, user
rate_limit_requests = 100
rate_limit_window = 60
```

### TLS Configuration

```toml
[server.http]
tls_enabled = true
tls_cert = "/etc/ssl/certs/codegraph.crt"
tls_key = "/etc/ssl/private/codegraph.key"
tls_ca = "/etc/ssl/certs/ca-bundle.crt"

# TLS options
tls_min_version = "1.2"
tls_ciphers = [
    "TLS_ECDHE_RSA_WITH_AES_256_GCM_SHA384",
    "TLS_ECDHE_RSA_WITH_AES_128_GCM_SHA256"
]
```

### Encryption

```toml
[security.encryption]
# Data encryption at rest
encrypt_database = true
encryption_algorithm = "aes-256-gcm"
key_derivation = "argon2id"

# Key management
key_rotation_days = 90
key_storage = "hsm"  # file, hsm, kms
```

## Troubleshooting Configuration

### Common Issues

#### Issue: Configuration not loading

```bash
# Check configuration path
codegraph config show --debug

# Validate configuration
codegraph config validate

# Test with minimal config
codegraph --config minimal.toml start
```

#### Issue: Environment variables not working

```bash
# Debug environment
env | grep CODEGRAPH

# Test specific variable
CODEGRAPH_LOG_LEVEL=debug codegraph status
```

#### Issue: Performance problems

```toml
# Enable profiling
[debug]
profiling = true
profile_output = "~/.codegraph/profiles"
trace_level = "verbose"
```

### Configuration Debugging

```bash
# Show effective configuration
codegraph config show --effective

# Show configuration sources
codegraph config show --sources

# Test configuration changes
codegraph config test --key performance.workers --value 16
```

### Reset Configuration

```bash
# Reset to defaults
codegraph config reset

# Reset specific section
codegraph config reset --section embedding

# Backup before reset
cp ~/.codegraph/config.toml ~/.codegraph/config.toml.backup
```

---

## See Also

- [CLI Reference](./CLI_REFERENCE.md)
- [Installation Guide](./INSTALLATION.md)
- [User Workflows](./WORKFLOWS.md)
- [Troubleshooting Guide](./TROUBLESHOOTING.md)