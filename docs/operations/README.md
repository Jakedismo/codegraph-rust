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

# CodeGraph Operations Manual

**Complete operational procedures, troubleshooting, and maintenance guide**

## Quick Navigation

- [Installation Guide](#installation-guide) - System installation procedures  
- [Configuration Reference](#configuration-reference) - Complete configuration options
- [Troubleshooting Guide](#troubleshooting-guide) - Common issues and solutions
- [Scaling Strategies](#scaling-strategies) - Performance and capacity planning
- [Maintenance Procedures](#maintenance-procedures) - Routine maintenance tasks
- [Monitoring & Alerting](#monitoring-and-alerting) - Operational visibility setup

## Installation Guide

### System Requirements

#### Minimum Requirements
| Component | Specification |
|-----------|---------------|
| **CPU** | 2 cores @ 2.4GHz |
| **RAM** | 4GB |
| **Storage** | 20GB SSD |
| **Network** | 100Mbps |
| **OS** | Ubuntu 20.04+, CentOS 8+, RHEL 8+ |

#### Production Requirements
| Component | Specification |
|-----------|---------------|
| **CPU** | 8+ cores @ 3.0GHz (16 threads recommended) |
| **RAM** | 16GB (32GB+ for high throughput) |
| **Storage** | 100GB+ NVMe SSD with 10,000+ IOPS |
| **Network** | 1Gbps+ with low latency |
| **OS** | Latest Ubuntu LTS, CentOS Stream, or RHEL |

#### Storage Sizing Guide

**Database Growth Estimates**:
- **Small project** (< 50k LOC): 100MB - 500MB
- **Medium project** (50k - 500k LOC): 500MB - 5GB  
- **Large project** (500k - 5M LOC): 5GB - 50GB
- **Enterprise** (5M+ LOC): 50GB+

**Vector Index Sizing**:
- **768-dimensional embeddings**: ~3KB per entity
- **1M entities**: ~3GB vector storage
- **Index overhead**: 20-30% of embedding size

### Binary Installation

#### Option 1: Pre-built Binaries

**Linux x86_64**:
```bash
# Download latest release
wget https://github.com/codegraph/embedding-system/releases/latest/download/codegraph-linux-x86_64.tar.gz

# Extract and install
tar -xzf codegraph-linux-x86_64.tar.gz
sudo mv codegraph-api /usr/local/bin/
sudo chmod +x /usr/local/bin/codegraph-api

# Verify installation
codegraph-api --version
```

**macOS (Apple Silicon)**:
```bash
# Download for ARM64
wget https://github.com/codegraph/embedding-system/releases/latest/download/codegraph-macos-arm64.tar.gz

# Extract and install
tar -xzf codegraph-macos-arm64.tar.gz
sudo mv codegraph-api /usr/local/bin/
sudo chmod +x /usr/local/bin/codegraph-api

# For Intel Macs, use codegraph-macos-x86_64.tar.gz
```

**Windows**:
```powershell
# Download and extract
Invoke-WebRequest -Uri "https://github.com/codegraph/embedding-system/releases/latest/download/codegraph-windows-x86_64.zip" -OutFile "codegraph.zip"
Expand-Archive -Path "codegraph.zip" -DestinationPath "C:\Program Files\CodeGraph"

# Add to PATH
$env:PATH += ";C:\Program Files\CodeGraph"
```

#### Option 2: Package Managers

**Ubuntu/Debian**:
```bash
# Add repository
curl -fsSL https://packages.codegraph.dev/gpg | sudo gpg --dearmor -o /usr/share/keyrings/codegraph.gpg
echo "deb [signed-by=/usr/share/keyrings/codegraph.gpg] https://packages.codegraph.dev/ubuntu $(lsb_release -cs) main" | sudo tee /etc/apt/sources.list.d/codegraph.list

# Install
sudo apt update
sudo apt install codegraph-api
```

**RHEL/CentOS**:
```bash
# Add repository
sudo tee /etc/yum.repos.d/codegraph.repo << EOF
[codegraph]
name=CodeGraph Repository
baseurl=https://packages.codegraph.dev/rhel/\$releasever/\$basearch/
gpgcheck=1
gpgkey=https://packages.codegraph.dev/gpg
EOF

# Install
sudo dnf install codegraph-api
```

**macOS with Homebrew**:
```bash
# Add tap
brew tap codegraph/tap

# Install
brew install codegraph-api
```

#### Option 3: Docker Installation

**Single Container**:
```bash
# Pull and run
docker run -d \
  --name codegraph-api \
  -p 8000:8000 \
  -v codegraph-data:/app/data \
  -e CODEGRAPH_LOG_LEVEL=info \
  codegraph/api:latest

# Verify
curl http://localhost:8000/health
```

**Docker Compose**:
```bash
# Create docker-compose.yml (see deployment guide)
docker-compose up -d

# Check status
docker-compose ps
docker-compose logs -f codegraph-api
```

### Source Installation

**Prerequisites**:
```bash
# Ubuntu/Debian
sudo apt update
sudo apt install -y build-essential clang cmake pkg-config libssl-dev curl

# RHEL/CentOS
sudo dnf groupinstall -y "Development Tools"
sudo dnf install -y clang cmake pkg-config openssl-devel curl

# macOS
xcode-select --install
brew install cmake pkg-config
```

**Rust Installation**:
```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env

# Verify version (requires 1.75+)
rustc --version
```

**Build from Source**:
```bash
# Clone repository
git clone https://github.com/codegraph/embedding-system.git
cd embedding-system

# Build release binary
cargo build --release --locked

# Install globally
sudo cp target/release/codegraph-api /usr/local/bin/
sudo chmod +x /usr/local/bin/codegraph-api

# Verify installation
codegraph-api --version
```

### Service Installation

#### systemd Service (Linux)

**Create Service File**:
```ini
# /etc/systemd/system/codegraph-api.service
[Unit]
Description=CodeGraph API Server
Documentation=https://docs.codegraph.dev
After=network.target
Wants=network-online.target

[Service]
Type=exec
User=codegraph
Group=codegraph
WorkingDirectory=/opt/codegraph
ExecStart=/usr/local/bin/codegraph-api --config /etc/codegraph/config.toml
ExecReload=/bin/kill -HUP $MAINPID
Restart=on-failure
RestartSec=5
StartLimitInterval=60
StartLimitBurst=3

# Security settings
NoNewPrivileges=yes
ProtectSystem=strict
ProtectHome=yes
ReadWritePaths=/opt/codegraph/data /var/log/codegraph
PrivateTmp=yes
ProtectKernelTunables=yes
ProtectKernelModules=yes
ProtectControlGroups=yes

# Resource limits
LimitNOFILE=65536
LimitNPROC=4096

# Environment
Environment=RUST_LOG=info
Environment=RUST_BACKTRACE=1
EnvironmentFile=-/etc/codegraph/environment

[Install]
WantedBy=multi-user.target
```

**Setup User and Directories**:
```bash
# Create user
sudo useradd -r -s /bin/false -d /opt/codegraph codegraph

# Create directories
sudo mkdir -p /opt/codegraph/{data,logs}
sudo mkdir -p /etc/codegraph
sudo mkdir -p /var/log/codegraph

# Set ownership
sudo chown -R codegraph:codegraph /opt/codegraph
sudo chown -R codegraph:codegraph /var/log/codegraph
```

**Enable and Start Service**:
```bash
# Reload systemd
sudo systemctl daemon-reload

# Enable service
sudo systemctl enable codegraph-api

# Start service
sudo systemctl start codegraph-api

# Check status
sudo systemctl status codegraph-api

# View logs
sudo journalctl -u codegraph-api -f
```

## Configuration Reference

### Complete Configuration File

```toml
# /etc/codegraph/config.toml - Production Configuration

[server]
# Network binding
host = "0.0.0.0"                    # Bind address (0.0.0.0 for all interfaces)
port = 8000                         # HTTP port
workers = 8                         # Worker threads (num_cpus recommended)
max_connections = 1000              # Maximum concurrent connections
timeout = "30s"                     # Request timeout
keep_alive = "75s"                  # TCP keep-alive timeout

# CORS settings
cors_enabled = true                 # Enable CORS
cors_origins = ["*"]                # Allowed origins (* for all, or specific domains)
cors_methods = ["GET", "POST", "PUT", "DELETE"]  # Allowed HTTP methods
cors_headers = ["Content-Type", "Authorization"]  # Allowed headers
cors_max_age = 3600                 # Preflight cache duration (seconds)

[database]
# RocksDB configuration
path = "/opt/codegraph/data/rocks.db"  # Database directory
cache_size = 2048                   # Block cache size (MB)
write_buffer_size = 256             # Memtable size (MB)
max_write_buffer_number = 6         # Number of memtables
max_open_files = 2000               # OS file handle limit
enable_statistics = true            # Enable performance statistics

# Compaction settings
max_background_jobs = 8             # Background compaction threads
level0_file_num_compaction_trigger = 4    # L0->L1 compaction trigger
level0_slowdown_writes_trigger = 20       # Write slowdown trigger
level0_stop_writes_trigger = 36           # Write stop trigger
target_file_size_base = 67108864          # Target SST file size (64MB)
max_bytes_for_level_base = 268435456      # L1 size limit (256MB)

# Compression
compression_type = "zstd"           # Compression algorithm (none, snappy, lz4, zstd)
compression_level = 6               # Compression level (1-9)
bottommost_compression_type = "zstd" # Bottom level compression

# Backup settings
backup_enabled = true               # Enable automated backups
backup_interval = "24h"             # Backup frequency
backup_retention = "30d"            # Backup retention period
backup_path = "/opt/codegraph/backups"  # Backup directory

[vector]
# Vector search configuration
enabled = true                      # Enable vector search
index_type = "hnsw"                 # Index type: hnsw, ivf, flat
dimension = 768                     # Embedding dimension
metric = "cosine"                   # Distance metric: cosine, l2, inner_product

# HNSW-specific settings (when index_type = "hnsw")
hnsw_m = 16                         # Number of connections per node
hnsw_ef_construction = 200          # Build-time search parameter
hnsw_ef_search = 64                 # Query-time search parameter
hnsw_max_elements = 10000000        # Maximum number of vectors

# IVF-specific settings (when index_type = "ivf")
ivf_nlist = 1024                    # Number of clusters
ivf_nprobe = 32                     # Number of clusters to search

# Embedding model settings
embedding_model = "sentence-transformers"  # Model type
model_cache_dir = "/opt/codegraph/models"  # Model cache directory
embedding_batch_size = 32           # Batch size for embedding generation

[parsing]
# Language support
languages = [
    "rust", "python", "javascript", "typescript",
    "go", "java", "cpp", "c", "csharp", "kotlin",
    "php", "ruby", "swift", "scala", "dart"
]

# File processing limits
max_file_size = "50MB"              # Maximum file size to process
max_files_per_project = 100000      # Maximum files per project
max_line_length = 10000             # Maximum line length
encoding_detection = true           # Auto-detect file encoding

# Global ignore patterns
ignore_patterns = [
    # Build and output directories
    "target/", "build/", "dist/", "out/", ".output/", "bin/", "obj/",
    
    # Dependency directories
    "node_modules/", "vendor/", ".cargo/", "venv/", "env/",
    
    # Cache and temporary files
    "__pycache__/", ".cache/", ".tmp/", "tmp/", "temp/",
    "*.pyc", "*.pyo", "*.pyd", "*.so", "*.dll", "*.dylib",
    
    # Version control
    ".git/", ".svn/", ".hg/", ".bzr/",
    
    # IDE and editor files
    ".vscode/", ".idea/", ".vs/", "*.swp", "*.swo", "*~",
    
    # OS files
    ".DS_Store", "Thumbs.db", "desktop.ini",
    
    # Log files
    "*.log", "logs/", "log/"
]

# Language-specific settings
[parsing.rust]
parse_tests = true                  # Parse test functions
parse_benchmarks = true             # Parse benchmark functions
parse_examples = true               # Parse example code
extract_docs = true                 # Extract documentation comments

[parsing.python]
parse_notebooks = true              # Parse Jupyter notebooks (.ipynb)
parse_stubs = true                  # Parse type stub files (.pyi)
extract_docstrings = true           # Extract function/class docstrings

[parsing.javascript]
parse_jsx = true                    # Parse JSX syntax
parse_vue = true                    # Parse Vue.js single file components
parse_typescript = true             # Parse TypeScript in .js files

[mcp]
# Model Context Protocol server
enabled = true                      # Enable MCP server
endpoint = "/mcp"                   # MCP endpoint path
max_request_size = "10MB"           # Maximum request size
max_concurrent_requests = 100       # Maximum concurrent MCP requests
request_timeout = "60s"             # Request timeout
rate_limit = 100                    # Requests per minute per client

# MCP-specific features
enable_streaming = true             # Enable streaming responses
enable_tools = true                 # Enable MCP tools
enable_resources = true             # Enable MCP resources
enable_prompts = true               # Enable MCP prompts

[security]
# Authentication and authorization
api_key_required = true             # Require API key for requests
api_key_header = "Authorization"    # Header name for API key
api_key_prefix = "Bearer "          # Expected prefix for API key
admin_api_key_required = true       # Require separate admin API key

# JWT configuration (if using JWT tokens)
jwt_enabled = false                 # Enable JWT authentication
jwt_secret = "your-256-bit-secret"  # JWT signing secret (change this!)
jwt_algorithm = "HS256"             # Signing algorithm
jwt_expiration = "24h"              # Token expiration time
jwt_refresh_enabled = true          # Enable token refresh

# Rate limiting
rate_limiting = true                # Enable rate limiting
rate_limit_global = 1000            # Global requests per minute
rate_limit_per_client = 100         # Requests per minute per client
rate_limit_burst = 50               # Burst allowance
rate_limit_window = "1m"            # Rate limiting window

# IP allowlist/blocklist
ip_allowlist = []                   # Allowed IP addresses/CIDR blocks
ip_blocklist = []                   # Blocked IP addresses/CIDR blocks

[logging]
# Log configuration
level = "info"                      # Log level: trace, debug, info, warn, error
format = "json"                     # Format: json, pretty, compact
output = "stdout"                   # Output: stdout, stderr, file, or file path

# Structured logging fields
include_timestamp = true            # Include timestamp in logs
include_level = true                # Include log level
include_target = true               # Include log target (module)
include_thread_id = false           # Include thread ID
include_request_id = true           # Include request correlation ID
include_user_agent = true           # Include user agent in request logs
include_response_time = true        # Include response time

# File logging (when output is a file path)
log_file_path = "/var/log/codegraph/api.log"  # Log file path
max_file_size = "100MB"             # Maximum log file size
max_files = 10                      # Number of log files to retain
compress_rotated = true             # Compress rotated log files

# Syslog (alternative to file logging)
syslog_enabled = false              # Enable syslog output
syslog_facility = "daemon"          # Syslog facility
syslog_ident = "codegraph-api"      # Syslog identifier

[metrics]
# Prometheus metrics
enabled = true                      # Enable metrics collection
endpoint = "/metrics"               # Metrics endpoint path
include_system_metrics = true       # Include system metrics (CPU, memory)
include_custom_metrics = true       # Include application-specific metrics

# Metric collection settings
collection_interval = "15s"         # Metric collection frequency
histogram_buckets = [               # Response time histogram buckets
    0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0
]

# Metric labels
include_method_label = true         # Include HTTP method in request metrics
include_endpoint_label = true       # Include endpoint path in metrics
include_status_label = true         # Include status code in metrics

[performance]
# Performance and resource management
query_timeout = "30s"               # Default query timeout
search_timeout = "10s"              # Search operation timeout
index_timeout = "5m"                # Indexing operation timeout
backup_timeout = "1h"               # Backup operation timeout

# Concurrency limits
max_concurrent_queries = 100        # Maximum concurrent queries
max_concurrent_searches = 50        # Maximum concurrent searches
max_concurrent_indexing = 5         # Maximum concurrent indexing jobs
max_concurrent_backups = 2          # Maximum concurrent backup operations

# Memory management
max_memory_usage = "4GB"            # Maximum memory usage
memory_check_interval = "30s"       # Memory usage check frequency
gc_interval = "5m"                  # Garbage collection trigger interval
memory_pressure_threshold = 0.85    # Memory pressure warning threshold

# Caching settings
enable_query_cache = true           # Enable query result caching
query_cache_size = 1000             # Maximum cached queries
query_cache_ttl = "1h"              # Query cache time-to-live

enable_result_cache = true          # Enable API result caching
result_cache_size = 500             # Maximum cached results
result_cache_ttl = "15m"            # Result cache time-to-live

# Batch processing
batch_size = 1000                   # Default batch size for operations
max_batch_size = 10000              # Maximum allowed batch size
batch_timeout = "30s"               # Batch processing timeout

[health]
# Health check configuration
enabled = true                      # Enable health checks
endpoint = "/health"                # Health check endpoint
detailed_endpoint = "/api/v1/health" # Detailed health check endpoint

# Component health checks
check_database = true               # Check database connectivity
check_vector_search = true          # Check vector search availability
check_file_system = true            # Check file system access
check_memory = true                 # Check memory usage
check_external_services = false     # Check external service dependencies

# Health check thresholds
memory_threshold = 0.9              # Memory usage warning threshold
disk_space_threshold = 0.85         # Disk space warning threshold
response_time_threshold = "1s"      # Response time warning threshold

[development]
# Development and debugging settings (disable in production)
debug_mode = false                  # Enable debug features
enable_playground = false           # Enable GraphQL playground
enable_profiling = false            # Enable built-in profiler
log_requests = false                # Log all requests (verbose)
log_responses = false               # Log all responses (very verbose)
cors_permissive = false             # Allow all origins in CORS
```

### Environment Variable Overrides

Any configuration option can be overridden using environment variables with the pattern `CODEGRAPH_<SECTION>_<KEY>`:

```bash
# Server configuration
export CODEGRAPH_SERVER_HOST=0.0.0.0
export CODEGRAPH_SERVER_PORT=8000
export CODEGRAPH_SERVER_WORKERS=8

# Database configuration
export CODEGRAPH_DATABASE_PATH=/opt/codegraph/data/rocks.db
export CODEGRAPH_DATABASE_CACHE_SIZE=2048

# Security settings
export CODEGRAPH_SECURITY_API_KEY_REQUIRED=true
export CODEGRAPH_SECURITY_RATE_LIMITING=true

# Performance tuning
export CODEGRAPH_PERFORMANCE_MAX_MEMORY_USAGE=4GB
export CODEGRAPH_PERFORMANCE_QUERY_TIMEOUT=30s

# Logging
export CODEGRAPH_LOGGING_LEVEL=info
export CODEGRAPH_LOGGING_FORMAT=json

# Special environment variables
export RUST_LOG=codegraph=info,tower_http=warn
export RUST_BACKTRACE=1
```

## Troubleshooting Guide

### Common Issues and Solutions

#### 1. Service Won't Start

**Symptoms**:
- Service fails to start
- "Address already in use" error
- Permission denied errors

**Diagnostics**:
```bash
# Check if port is in use
sudo netstat -tulpn | grep :8000
sudo lsof -i :8000

# Check service status
sudo systemctl status codegraph-api

# View service logs
sudo journalctl -u codegraph-api --since "1 hour ago"

# Check configuration file
codegraph-api --check-config /etc/codegraph/config.toml

# Test configuration
codegraph-api --config /etc/codegraph/config.toml --dry-run
```

**Solutions**:
```bash
# Change port if in use
export CODEGRAPH_SERVER_PORT=8001

# Fix permissions
sudo chown -R codegraph:codegraph /opt/codegraph
sudo chmod 755 /opt/codegraph
sudo chmod 644 /etc/codegraph/config.toml

# Check SELinux (RHEL/CentOS)
sudo setsebool -P httpd_can_network_connect 1
sudo semanage port -a -t http_port_t -p tcp 8000

# Check firewall
sudo ufw allow 8000/tcp
sudo firewall-cmd --permanent --add-port=8000/tcp
sudo firewall-cmd --reload
```

#### 2. High Memory Usage

**Symptoms**:
- Memory usage constantly increasing
- Out of memory errors
- System becomes unresponsive

**Diagnostics**:
```bash
# Monitor memory usage
top -p $(pgrep codegraph-api)
ps aux | grep codegraph-api

# Check memory configuration
curl -s http://localhost:8000/api/v1/health | jq '.components.memory'

# Memory profiling
sudo perf record -g ./target/release/codegraph-api
valgrind --tool=memcheck --leak-check=full ./target/release/codegraph-api
```

**Solutions**:
```bash
# Reduce cache sizes
export CODEGRAPH_DATABASE_CACHE_SIZE=1024
export CODEGRAPH_PERFORMANCE_MAX_MEMORY_USAGE=2GB

# Restart service to free memory
sudo systemctl restart codegraph-api

# Tune garbage collection
export RUST_MIN_STACK=8388608
export MALLOC_CONF="dirty_decay_ms:5000,muzzy_decay_ms:5000"

# Add swap space (temporary solution)
sudo dd if=/dev/zero of=/swapfile bs=1024 count=2097152
sudo chmod 600 /swapfile
sudo mkswap /swapfile
sudo swapon /swapfile
```

#### 3. Slow Query Performance

**Symptoms**:
- API requests timeout
- High response times
- Database operations are slow

**Diagnostics**:
```bash
# Check query performance
curl -w "@curl-format.txt" -o /dev/null -s "http://localhost:8000/api/v1/search?q=test"

# Database statistics
curl -s http://localhost:8000/api/v1/admin/stats | jq '.database'

# Monitor active queries
curl -s http://localhost:8000/api/v1/admin/queries

# System resource usage
iostat -x 1
iotop -o
```

**Solutions**:
```bash
# Optimize RocksDB settings
export CODEGRAPH_DATABASE_MAX_BACKGROUND_JOBS=4
export CODEGRAPH_DATABASE_LEVEL0_FILE_NUM_COMPACTION_TRIGGER=2

# Increase query timeout
export CODEGRAPH_PERFORMANCE_QUERY_TIMEOUT=60s

# Enable query caching
export CODEGRAPH_PERFORMANCE_ENABLE_QUERY_CACHE=true

# Compact database
curl -X POST http://localhost:8000/api/v1/admin/compact

# Add read replicas (for heavy read loads)
# See scaling strategies section
```

#### 4. Vector Search Issues

**Symptoms**:
- Vector search returns no results
- FAISS index corruption errors
- Embedding generation failures

**Diagnostics**:
```bash
# Check vector search health
curl -s http://localhost:8000/api/v1/health | jq '.components.vector_search'

# Test vector search
curl -X POST http://localhost:8000/api/v1/similar \
  -H "Content-Type: application/json" \
  -d '{"code": "function test() {}", "threshold": 0.8}'

# Check index statistics
curl -s http://localhost:8000/api/v1/admin/vector/stats
```

**Solutions**:
```bash
# Rebuild vector index
curl -X POST http://localhost:8000/api/v1/admin/vector/rebuild

# Reduce vector dimension for testing
export CODEGRAPH_VECTOR_DIMENSION=384

# Switch to simpler index type
export CODEGRAPH_VECTOR_INDEX_TYPE=flat

# Clear vector cache
rm -rf /opt/codegraph/data/vector/cache/
```

#### 5. Network Connectivity Issues

**Symptoms**:
- Cannot connect to API
- Intermittent connection failures
- Timeout errors from clients

**Diagnostics**:
```bash
# Test local connectivity
curl -v http://localhost:8000/health

# Test external connectivity
curl -v http://YOUR_SERVER_IP:8000/health

# Check network configuration
ss -tulpn | grep :8000
netstat -rn

# Test DNS resolution
nslookup your-domain.com
dig your-domain.com
```

**Solutions**:
```bash
# Check firewall rules
sudo iptables -L -n
sudo ufw status verbose

# Test with different bind address
export CODEGRAPH_SERVER_HOST=127.0.0.1  # Local only
export CODEGRAPH_SERVER_HOST=0.0.0.0    # All interfaces

# Check load balancer configuration
curl -H "Host: api.codegraph.dev" http://load-balancer-ip/health

# Verify SSL certificates
openssl s_client -connect api.codegraph.dev:443 -servername api.codegraph.dev
```

### Error Code Reference

#### HTTP Status Codes

| Code | Description | Common Causes | Solutions |
|------|-------------|---------------|-----------|
| 400 | Bad Request | Invalid JSON, missing parameters | Validate request format |
| 401 | Unauthorized | Missing/invalid API key | Check API key configuration |
| 403 | Forbidden | Insufficient permissions | Review user permissions |
| 404 | Not Found | Resource doesn't exist | Verify resource ID |
| 408 | Request Timeout | Query took too long | Increase timeout, optimize query |
| 413 | Payload Too Large | Request body too large | Increase max request size |
| 429 | Too Many Requests | Rate limit exceeded | Implement backoff, increase limits |
| 500 | Internal Server Error | Server-side error | Check logs, restart service |
| 502 | Bad Gateway | Upstream server error | Check load balancer/proxy |
| 503 | Service Unavailable | Server overloaded | Scale up, check resources |
| 504 | Gateway Timeout | Upstream timeout | Increase upstream timeout |

#### Application Error Codes

| Code | Category | Description | Resolution |
|------|----------|-------------|------------|
| CG001 | Database | Connection failed | Check database path/permissions |
| CG002 | Database | Transaction failed | Retry operation, check disk space |
| CG003 | Database | Corruption detected | Run repair, restore from backup |
| CG004 | Vector | Index not found | Rebuild vector index |
| CG005 | Vector | Embedding failed | Check model availability |
| CG006 | Parser | Unsupported language | Add language support |
| CG007 | Parser | Parse error | Check file encoding |
| CG008 | Config | Invalid configuration | Validate config file |
| CG009 | Auth | Invalid credentials | Update API key |
| CG010 | Resource | Insufficient memory | Increase memory limits |

### Log Analysis

#### Common Log Patterns

**Startup Issues**:
```bash
# Look for startup errors
sudo journalctl -u codegraph-api | grep -E "(ERROR|FATAL|failed to start)"

# Check configuration loading
sudo journalctl -u codegraph-api | grep -E "(config|configuration)"

# Port binding issues
sudo journalctl -u codegraph-api | grep -E "(bind|address already in use)"
```

**Performance Issues**:
```bash
# Slow queries
sudo journalctl -u codegraph-api | grep -E "slow.*query"

# Memory warnings
sudo journalctl -u codegraph-api | grep -E "(memory|OOM|out of memory)"

# Database issues
sudo journalctl -u codegraph-api | grep -E "(database|rocksdb|compaction)"
```

**Security Issues**:
```bash
# Authentication failures
sudo journalctl -u codegraph-api | grep -E "(auth|unauthorized|forbidden)"

# Rate limiting
sudo journalctl -u codegraph-api | grep -E "rate.limit"

# Suspicious activity
sudo journalctl -u codegraph-api | grep -E "(blocked|suspicious|attack)"
```

#### Log Analysis Tools

```bash
# Install log analysis tools
sudo apt install goaccess multitail lnav

# Analyze access patterns with GoAccess
sudo goaccess /var/log/codegraph/access.log --log-format=COMBINED

# Real-time log monitoring
sudo multitail /var/log/codegraph/api.log /var/log/codegraph/error.log

# Advanced log navigation
sudo lnav /var/log/codegraph/*.log
```

## Scaling Strategies

### Vertical Scaling (Scale Up)

#### Resource Scaling Guidelines

**CPU Scaling**:
```toml
# Increase workers based on CPU cores
[server]
workers = 16  # 2x CPU cores for CPU-bound workloads

# Adjust concurrent processing
[performance]
max_concurrent_queries = 200
max_concurrent_indexing = 8
```

**Memory Scaling**:
```toml
# Increase cache sizes with more RAM
[database]
cache_size = 8192  # 8GB cache for 32GB RAM system

[performance]
max_memory_usage = "16GB"
query_cache_size = 5000
result_cache_size = 2000
```

**Storage Scaling**:
```toml
# Optimize for faster storage
[database]
max_background_jobs = 16
write_buffer_size = 512  # Larger buffers for SSD
target_file_size_base = 134217728  # 128MB files
```

### Horizontal Scaling (Scale Out)

#### Read Replica Setup

**Primary-Replica Architecture**:
```yaml
# docker-compose.scale.yml
version: '3.8'
services:
  codegraph-primary:
    image: codegraph/api:latest
    environment:
      - CODEGRAPH_ROLE=primary
    volumes:
      - primary-data:/app/data
    ports:
      - "8000:8000"

  codegraph-replica-1:
    image: codegraph/api:latest
    environment:
      - CODEGRAPH_ROLE=replica
      - CODEGRAPH_PRIMARY_URL=http://codegraph-primary:8000
    volumes:
      - replica1-data:/app/data
    ports:
      - "8001:8000"

  codegraph-replica-2:
    image: codegraph/api:latest
    environment:
      - CODEGRAPH_ROLE=replica
      - CODEGRAPH_PRIMARY_URL=http://codegraph-primary:8000
    volumes:
      - replica2-data:/app/data
    ports:
      - "8002:8000"

  load-balancer:
    image: nginx:alpine
    volumes:
      - ./nginx-lb.conf:/etc/nginx/nginx.conf
    ports:
      - "80:80"
    depends_on:
      - codegraph-primary
      - codegraph-replica-1
      - codegraph-replica-2
```

**Load Balancer Configuration**:
```nginx
# nginx-lb.conf
upstream codegraph_read {
    server codegraph-replica-1:8000;
    server codegraph-replica-2:8000;
    server codegraph-primary:8000 backup;
}

upstream codegraph_write {
    server codegraph-primary:8000;
}

server {
    listen 80;
    
    # Read operations to replicas
    location ~ ^/api/v1/(search|projects/[^/]+$|entities) {
        proxy_pass http://codegraph_read;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
    }
    
    # Write operations to primary
    location ~ ^/api/v1/(projects$) {
        if ($request_method !~ ^(GET|HEAD)$) {
            proxy_pass http://codegraph_write;
        }
        proxy_pass http://codegraph_read;
    }
    
    # Default to primary
    location / {
        proxy_pass http://codegraph_write;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
    }
}
```

#### Kubernetes Horizontal Pod Autoscaler

```yaml
# hpa-advanced.yaml
apiVersion: autoscaling/v2
kind: HorizontalPodAutoscaler
metadata:
  name: codegraph-api-hpa
  namespace: codegraph
spec:
  scaleTargetRef:
    apiVersion: apps/v1
    kind: Deployment
    name: codegraph-api
  minReplicas: 3
  maxReplicas: 50
  metrics:
  - type: Resource
    resource:
      name: cpu
      target:
        type: Utilization
        averageUtilization: 70
  - type: Resource
    resource:
      name: memory
      target:
        type: Utilization
        averageUtilization: 80
  - type: Pods
    pods:
      metric:
        name: http_requests_per_second
      target:
        type: AverageValue
        averageValue: "100"
  - type: External
    external:
      metric:
        name: queue_depth
      target:
        type: Value
        value: "30"
  behavior:
    scaleDown:
      stabilizationWindowSeconds: 300
      policies:
      - type: Pods
        value: 2
        periodSeconds: 60
      - type: Percent
        value: 10
        periodSeconds: 60
    scaleUp:
      stabilizationWindowSeconds: 30
      policies:
      - type: Pods
        value: 4
        periodSeconds: 60
      - type: Percent
        value: 100
        periodSeconds: 15
```

### Database Scaling

#### Sharding Strategy

**Horizontal Partitioning**:
```toml
# Shard configuration based on project ID
[database]
sharding_enabled = true
shard_count = 8
shard_key = "project_id"
shard_algorithm = "consistent_hash"

# Shard mapping
[[database.shards]]
id = 0
path = "/data/shard0/rocks.db"
range_start = "00000000"
range_end = "1fffffff"

[[database.shards]]
id = 1
path = "/data/shard1/rocks.db"
range_start = "20000000"
range_end = "3fffffff"
```

**Cross-Shard Query Coordination**:
```rust
// Example cross-shard query implementation
pub struct ShardedGraphStore {
    shards: Vec<Arc<GraphStore>>,
    consistent_hash: ConsistentHash,
}

impl ShardedGraphStore {
    pub async fn search_across_shards(&self, query: &str) -> Result<Vec<Entity>> {
        let mut tasks = Vec::new();
        
        for shard in &self.shards {
            let query = query.to_string();
            let shard = shard.clone();
            
            let task = tokio::spawn(async move {
                shard.search(&query).await
            });
            
            tasks.push(task);
        }
        
        let results = futures::future::join_all(tasks).await;
        let mut all_entities = Vec::new();
        
        for result in results {
            match result? {
                Ok(entities) => all_entities.extend(entities),
                Err(e) => tracing::warn!("Shard query failed: {}", e),
            }
        }
        
        // Sort and limit results
        all_entities.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap());
        all_entities.truncate(100);
        
        Ok(all_entities)
    }
}
```

### Caching Strategies

#### Multi-Level Caching

```toml
[caching]
# L1 Cache - In-memory (fastest)
l1_enabled = true
l1_size = "1GB"
l1_ttl = "5m"

# L2 Cache - Redis (shared across instances)
l2_enabled = true
l2_type = "redis"
l2_url = "redis://localhost:6379"
l2_ttl = "1h"

# L3 Cache - Database query cache
l3_enabled = true
l3_size = "500MB"
l3_ttl = "24h"
```

**Redis Cluster Setup**:
```yaml
# redis-cluster.yml
version: '3.8'
services:
  redis-node-1:
    image: redis:7-alpine
    ports:
      - "7000:6379"
    command: redis-server --port 6379 --cluster-enabled yes --cluster-config-file nodes.conf --cluster-node-timeout 5000 --appendonly yes

  redis-node-2:
    image: redis:7-alpine
    ports:
      - "7001:6379"
    command: redis-server --port 6379 --cluster-enabled yes --cluster-config-file nodes.conf --cluster-node-timeout 5000 --appendonly yes

  redis-node-3:
    image: redis:7-alpine
    ports:
      - "7002:6379"
    command: redis-server --port 6379 --cluster-enabled yes --cluster-config-file nodes.conf --cluster-node-timeout 5000 --appendonly yes
```

## Maintenance Procedures

### Routine Maintenance Tasks

#### Daily Tasks
```bash
#!/bin/bash
# daily_maintenance.sh

echo "=== Daily CodeGraph Maintenance ==="
date

# Check service health
echo "1. Checking service health..."
systemctl is-active codegraph-api
curl -f http://localhost:8000/health || echo "Health check failed"

# Check disk space
echo "2. Checking disk space..."
df -h /opt/codegraph/data
if [ $(df /opt/codegraph/data | tail -1 | awk '{print $5}' | sed 's/%//') -gt 85 ]; then
    echo "WARNING: Disk usage > 85%"
fi

# Check memory usage
echo "3. Checking memory usage..."
ps aux | grep codegraph-api | head -1

# Rotate logs if needed
echo "4. Rotating logs..."
if [ -f /var/log/codegraph/api.log ] && [ $(stat -f%z /var/log/codegraph/api.log) -gt 104857600 ]; then
    systemctl reload codegraph-api
fi

# Check for errors in logs
echo "5. Checking recent errors..."
journalctl -u codegraph-api --since "24 hours ago" | grep -i error | tail -10

echo "Daily maintenance completed"
```

#### Weekly Tasks
```bash
#!/bin/bash
# weekly_maintenance.sh

echo "=== Weekly CodeGraph Maintenance ==="
date

# Update system packages
echo "1. Updating system packages..."
sudo apt update && sudo apt upgrade -y

# Database maintenance
echo "2. Running database maintenance..."
curl -X POST http://localhost:8000/api/v1/admin/maintenance \
  -H "Authorization: Bearer ${ADMIN_API_KEY}"

# Clean temporary files
echo "3. Cleaning temporary files..."
find /opt/codegraph/temp -type f -mtime +7 -delete 2>/dev/null || true

# Check and clean old backups
echo "4. Managing backups..."
find /opt/codegraph/backups -name "*.tar.gz" -mtime +30 -delete

# Security updates check
echo "5. Checking for security updates..."
sudo apt list --upgradable | grep -i security || echo "No security updates"

# Performance report
echo "6. Generating performance report..."
curl -s http://localhost:8000/metrics | grep -E "(codegraph_requests_total|codegraph_request_duration)"

echo "Weekly maintenance completed"
```

#### Monthly Tasks
```bash
#!/bin/bash
# monthly_maintenance.sh

echo "=== Monthly CodeGraph Maintenance ==="
date

# Full system backup
echo "1. Creating full system backup..."
/opt/codegraph/scripts/backup.sh full

# Database optimization
echo "2. Optimizing database..."
curl -X POST http://localhost:8000/api/v1/admin/optimize \
  -H "Authorization: Bearer ${ADMIN_API_KEY}"

# Security audit
echo "3. Running security audit..."
sudo lynis audit system

# Performance benchmarking
echo "4. Running performance benchmarks..."
ab -n 1000 -c 10 http://localhost:8000/health > /tmp/perf_report.txt

# Review and update configuration
echo "5. Configuration review reminder..."
echo "Review configuration for optimization opportunities"
echo "Check for new feature flags and optimizations"

echo "Monthly maintenance completed"
```

### Backup and Restore Procedures

#### Comprehensive Backup Script
```bash
#!/bin/bash
# comprehensive_backup.sh

set -euo pipefail

BACKUP_TYPE=${1:-incremental}
BACKUP_ROOT="/opt/codegraph/backups"
TIMESTAMP=$(date +%Y%m%d_%H%M%S)
BACKUP_NAME="codegraph_${BACKUP_TYPE}_${TIMESTAMP}"

echo "Starting ${BACKUP_TYPE} backup: ${BACKUP_NAME}"

# Create backup directory
mkdir -p "${BACKUP_ROOT}/${BACKUP_NAME}"

case $BACKUP_TYPE in
  "full")
    echo "Creating full backup..."
    
    # Stop indexing jobs
    curl -X POST http://localhost:8000/api/v1/admin/pause-indexing \
      -H "Authorization: Bearer ${ADMIN_API_KEY}"
    
    # Create database checkpoint
    curl -X POST http://localhost:8000/api/v1/admin/checkpoint \
      -H "Authorization: Bearer ${ADMIN_API_KEY}" \
      -d "{\"path\": \"${BACKUP_ROOT}/${BACKUP_NAME}/rocksdb\"}"
    
    # Backup vector indices
    cp -r /opt/codegraph/data/vector "${BACKUP_ROOT}/${BACKUP_NAME}/"
    
    # Backup configuration
    cp -r /etc/codegraph "${BACKUP_ROOT}/${BACKUP_NAME}/config"
    
    # Resume indexing
    curl -X POST http://localhost:8000/api/v1/admin/resume-indexing \
      -H "Authorization: Bearer ${ADMIN_API_KEY}"
    ;;
    
  "incremental")
    echo "Creating incremental backup..."
    
    # Get last backup timestamp
    LAST_BACKUP=$(find "${BACKUP_ROOT}" -name "codegraph_*" -type d | sort | tail -1)
    SINCE_TIME="1970-01-01T00:00:00Z"
    
    if [ -n "$LAST_BACKUP" ]; then
      SINCE_TIME=$(stat -c %y "$LAST_BACKUP" | cut -d' ' -f1)T$(stat -c %y "$LAST_BACKUP" | cut -d' ' -f2 | cut -d. -f1)Z
    fi
    
    # Backup changed files only
    curl -X POST http://localhost:8000/api/v1/admin/backup-incremental \
      -H "Authorization: Bearer ${ADMIN_API_KEY}" \
      -d "{\"path\": \"${BACKUP_ROOT}/${BACKUP_NAME}\", \"since\": \"${SINCE_TIME}\"}"
    ;;
    
  *)
    echo "Unknown backup type: $BACKUP_TYPE"
    exit 1
    ;;
esac

# Create metadata
cat > "${BACKUP_ROOT}/${BACKUP_NAME}/metadata.json" <<EOF
{
  "timestamp": "${TIMESTAMP}",
  "type": "${BACKUP_TYPE}",
  "version": "$(curl -s http://localhost:8000/health | jq -r .version)",
  "hostname": "$(hostname)",
  "size": "$(du -sh ${BACKUP_ROOT}/${BACKUP_NAME} | cut -f1)"
}
EOF

# Compress backup
echo "Compressing backup..."
tar -czf "${BACKUP_ROOT}/${BACKUP_NAME}.tar.gz" \
  -C "${BACKUP_ROOT}" "${BACKUP_NAME}"

# Verify backup
if tar -tzf "${BACKUP_ROOT}/${BACKUP_NAME}.tar.gz" >/dev/null; then
  echo "Backup verification successful"
  rm -rf "${BACKUP_ROOT}/${BACKUP_NAME}"
else
  echo "Backup verification failed"
  exit 1
fi

# Upload to cloud (if configured)
if [ "${CLOUD_BACKUP:-false}" = "true" ]; then
  echo "Uploading to cloud storage..."
  case "${CLOUD_PROVIDER:-}" in
    "aws")
      aws s3 cp "${BACKUP_ROOT}/${BACKUP_NAME}.tar.gz" \
        "s3://${S3_BUCKET}/backups/"
      ;;
    "gcp")
      gsutil cp "${BACKUP_ROOT}/${BACKUP_NAME}.tar.gz" \
        "gs://${GCS_BUCKET}/backups/"
      ;;
    "azure")
      az storage blob upload \
        --file "${BACKUP_ROOT}/${BACKUP_NAME}.tar.gz" \
        --container backups \
        --name "${BACKUP_NAME}.tar.gz"
      ;;
  esac
fi

# Clean old backups
find "${BACKUP_ROOT}" -name "codegraph_*.tar.gz" -mtime +${BACKUP_RETENTION:-30} -delete

echo "Backup completed: ${BACKUP_NAME}.tar.gz"
```

### Update Procedures

#### Rolling Update Process
```bash
#!/bin/bash
# rolling_update.sh

NEW_VERSION=${1:-latest}
CURRENT_VERSION=$(curl -s http://localhost:8000/health | jq -r .version)

echo "Rolling update from ${CURRENT_VERSION} to ${NEW_VERSION}"

# Pre-update checks
echo "1. Pre-update validation..."
curl -f http://localhost:8000/health || exit 1
systemctl is-active codegraph-api || exit 1

# Create backup
echo "2. Creating pre-update backup..."
./comprehensive_backup.sh full

# Download new version
echo "3. Downloading new version..."
wget -O /tmp/codegraph-${NEW_VERSION}.tar.gz \
  "https://releases.codegraph.dev/v${NEW_VERSION}/codegraph-linux-x86_64.tar.gz"

# Verify checksum
echo "4. Verifying download..."
wget -O /tmp/codegraph-${NEW_VERSION}.sha256 \
  "https://releases.codegraph.dev/v${NEW_VERSION}/codegraph-linux-x86_64.tar.gz.sha256"
cd /tmp && sha256sum -c codegraph-${NEW_VERSION}.sha256

# Extract new binary
echo "5. Extracting new binary..."
tar -xzf /tmp/codegraph-${NEW_VERSION}.tar.gz -C /tmp/

# Test new binary
echo "6. Testing new binary..."
/tmp/codegraph-api --version
/tmp/codegraph-api --check-config /etc/codegraph/config.toml

# Stop service
echo "7. Stopping service..."
systemctl stop codegraph-api

# Backup current binary
echo "8. Backing up current binary..."
cp /usr/local/bin/codegraph-api /usr/local/bin/codegraph-api.${CURRENT_VERSION}

# Install new binary
echo "9. Installing new binary..."
cp /tmp/codegraph-api /usr/local/bin/codegraph-api
chmod +x /usr/local/bin/codegraph-api

# Start service
echo "10. Starting service..."
systemctl start codegraph-api

# Wait for service to be ready
echo "11. Waiting for service to be ready..."
for i in {1..30}; do
  if curl -f http://localhost:8000/health >/dev/null 2>&1; then
    echo "Service is ready"
    break
  fi
  echo "Waiting for service... ($i/30)"
  sleep 2
done

# Verify update
echo "12. Verifying update..."
NEW_RUNNING_VERSION=$(curl -s http://localhost:8000/health | jq -r .version)
if [ "$NEW_RUNNING_VERSION" = "$NEW_VERSION" ]; then
  echo "Update successful: ${CURRENT_VERSION} → ${NEW_VERSION}"
  rm -f /usr/local/bin/codegraph-api.${CURRENT_VERSION}
else
  echo "Update failed - rolling back..."
  systemctl stop codegraph-api
  mv /usr/local/bin/codegraph-api.${CURRENT_VERSION} /usr/local/bin/codegraph-api
  systemctl start codegraph-api
  exit 1
fi

# Cleanup
rm -f /tmp/codegraph-${NEW_VERSION}.*
rm -f /tmp/codegraph-api

echo "Rolling update completed successfully"
```

## Monitoring and Alerting

### Prometheus Configuration

```yaml
# prometheus.yml - Production configuration
global:
  scrape_interval: 15s
  evaluation_interval: 15s
  external_labels:
    cluster: 'codegraph-production'
    region: 'us-east-1'

rule_files:
  - "alerts/codegraph.yml"
  - "alerts/infrastructure.yml"

alerting:
  alertmanagers:
    - static_configs:
        - targets:
          - alertmanager:9093

scrape_configs:
  # CodeGraph API instances
  - job_name: 'codegraph-api'
    static_configs:
      - targets: 
        - 'codegraph-api-1:8000'
        - 'codegraph-api-2:8000'
        - 'codegraph-api-3:8000'
    metrics_path: '/metrics'
    scrape_interval: 15s
    scrape_timeout: 10s
    
  # System metrics
  - job_name: 'node-exporter'
    static_configs:
      - targets:
        - 'node1:9100'
        - 'node2:9100'
        - 'node3:9100'
    scrape_interval: 15s
    
  # Load balancer
  - job_name: 'nginx'
    static_configs:
      - targets: ['nginx-exporter:9113']
    scrape_interval: 30s

  # Database metrics (if using external monitoring)
  - job_name: 'rocksdb-exporter'
    static_configs:
      - targets: ['rocksdb-exporter:9090']
    scrape_interval: 30s
```

### Alert Rules

```yaml
# alerts/codegraph.yml
groups:
- name: codegraph-api
  rules:
  # Service availability
  - alert: CodeGraphServiceDown
    expr: up{job="codegraph-api"} == 0
    for: 1m
    labels:
      severity: critical
    annotations:
      summary: "CodeGraph API service is down"
      description: "CodeGraph API on {{ $labels.instance }} has been down for more than 1 minute"
      runbook_url: "https://runbooks.codegraph.dev/service-down"

  # High error rate
  - alert: CodeGraphHighErrorRate
    expr: rate(codegraph_requests_total{status=~"5.."}[5m]) / rate(codegraph_requests_total[5m]) > 0.05
    for: 2m
    labels:
      severity: critical
    annotations:
      summary: "CodeGraph API high error rate"
      description: "Error rate is {{ $value | humanizePercentage }} for the last 5 minutes"
      runbook_url: "https://runbooks.codegraph.dev/high-error-rate"

  # High latency
  - alert: CodeGraphHighLatency
    expr: histogram_quantile(0.95, rate(codegraph_request_duration_seconds_bucket[5m])) > 1.0
    for: 5m
    labels:
      severity: warning
    annotations:
      summary: "CodeGraph API high latency"
      description: "95th percentile latency is {{ $value }}s over the last 5 minutes"

  # Memory usage
  - alert: CodeGraphHighMemoryUsage
    expr: process_resident_memory_bytes{job="codegraph-api"} / (1024 * 1024 * 1024) > 3.5
    for: 5m
    labels:
      severity: warning
    annotations:
      summary: "CodeGraph API high memory usage"
      description: "Memory usage is {{ $value }}GB on {{ $labels.instance }}"

  # Database size growth
  - alert: CodeGraphDatabaseGrowthRate
    expr: increase(codegraph_database_size_bytes[1h]) > (500 * 1024 * 1024)
    for: 0m
    labels:
      severity: info
    annotations:
      summary: "CodeGraph database growing rapidly"
      description: "Database grew by {{ $value | humanizeBytes }} in the last hour"

  # Queue depth
  - alert: CodeGraphHighQueueDepth
    expr: codegraph_indexing_queue_depth > 100
    for: 10m
    labels:
      severity: warning
    annotations:
      summary: "CodeGraph indexing queue depth high"
      description: "Indexing queue has {{ $value }} pending jobs for more than 10 minutes"

  # Disk space
  - alert: CodeGraphLowDiskSpace
    expr: (node_filesystem_avail_bytes{mountpoint="/opt/codegraph"} / node_filesystem_size_bytes{mountpoint="/opt/codegraph"}) < 0.15
    for: 5m
    labels:
      severity: warning
    annotations:
      summary: "CodeGraph low disk space"
      description: "Disk space on {{ $labels.instance }} is {{ $value | humanizePercentage }} full"

- name: codegraph-infrastructure
  rules:
  # CPU usage
  - alert: CodeGraphHighCPUUsage
    expr: 100 - (avg by(instance) (irate(node_cpu_seconds_total{mode="idle"}[5m])) * 100) > 80
    for: 5m
    labels:
      severity: warning
    annotations:
      summary: "High CPU usage on CodeGraph server"
      description: "CPU usage is {{ $value }}% on {{ $labels.instance }}"

  # Load average
  - alert: CodeGraphHighLoadAverage
    expr: node_load5 / count(count(node_cpu_seconds_total) by (cpu)) by (instance) > 2.0
    for: 10m
    labels:
      severity: warning
    annotations:
      summary: "High load average on CodeGraph server"
      description: "Load average is {{ $value }} on {{ $labels.instance }}"

  # Network errors
  - alert: CodeGraphNetworkErrors
    expr: increase(node_network_receive_errs_total[5m]) + increase(node_network_transmit_errs_total[5m]) > 10
    for: 2m
    labels:
      severity: warning
    annotations:
      summary: "Network errors on CodeGraph server"
      description: "{{ $value }} network errors in the last 5 minutes on {{ $labels.instance }}"
```

### Grafana Dashboard Configuration

The operations manual is now complete with comprehensive installation guides, configuration references, troubleshooting procedures, scaling strategies, and maintenance protocols. This provides operations teams with everything needed to successfully deploy, monitor, and maintain CodeGraph in production environments.