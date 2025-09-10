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

# CodeGraph Production Installation Guide

## Table of Contents

1. [System Requirements](#system-requirements)
2. [Pre-Installation Setup](#pre-installation-setup)
3. [Installation Methods](#installation-methods)
4. [Configuration](#configuration)
5. [Database Setup](#database-setup)
6. [Service Configuration](#service-configuration)
7. [Security Setup](#security-setup)
8. [Performance Tuning](#performance-tuning)
9. [Verification](#verification)
10. [Troubleshooting](#troubleshooting)

## System Requirements

### Hardware Requirements

**Minimum Requirements:**
- CPU: 4 cores, 2.4 GHz
- RAM: 8 GB
- Storage: 50 GB SSD
- Network: 1 Gbps

**Recommended for Production:**
- CPU: 8+ cores, 3.0+ GHz
- RAM: 32 GB+
- Storage: 500 GB+ NVMe SSD
- Network: 10 Gbps

### Software Requirements

**Operating System:**
- Ubuntu 20.04 LTS or later
- CentOS 8 or later
- RHEL 8 or later
- Amazon Linux 2

**Dependencies:**
- Rust 1.70+ (MSRV as per Tokio requirements)
- clang 10+
- cmake 3.16+
- OpenSSL 1.1.1+
- pkg-config

## Pre-Installation Setup

### 1. Update System

```bash
# Ubuntu/Debian
sudo apt update && sudo apt upgrade -y

# CentOS/RHEL
sudo yum update -y
# or for newer versions
sudo dnf update -y
```

### 2. Install System Dependencies

```bash
# Ubuntu/Debian
sudo apt install -y \
    build-essential \
    clang \
    cmake \
    pkg-config \
    libssl-dev \
    libclang-dev \
    curl \
    git

# CentOS/RHEL
sudo yum groupinstall -y "Development Tools"
sudo yum install -y \
    clang \
    cmake \
    pkgconfig \
    openssl-devel \
    curl \
    git
```

### 3. Install Rust

```bash
# Install rustup
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Source the environment
source $HOME/.cargo/env

# Verify installation
rustc --version
cargo --version

# Ensure minimum version (1.70+)
rustup update
```

### 4. Configure System Limits

Create `/etc/security/limits.d/codegraph.conf`:

```
codegraph soft nofile 65536
codegraph hard nofile 65536
codegraph soft nproc 4096
codegraph hard nproc 4096
```

### 5. Create Service User

```bash
sudo useradd --system --shell /bin/false --home /opt/codegraph codegraph
sudo mkdir -p /opt/codegraph
sudo chown codegraph:codegraph /opt/codegraph
```

## Installation Methods

### Method 1: Building from Source (Recommended)

#### 1. Clone Repository

```bash
sudo -u codegraph git clone https://github.com/codegraph/embedding-system.git /opt/codegraph/src
cd /opt/codegraph/src
```

#### 2. Build CodeGraph

```bash
# Quick verification build
sudo -u codegraph cargo check --all-features

# Production build with optimizations
sudo -u codegraph cargo build --release --all-features

# Run tests to verify build
sudo -u codegraph cargo test --release
```

#### 3. Install Binaries

```bash
sudo -u codegraph mkdir -p /opt/codegraph/bin
sudo -u codegraph cp target/release/codegraph-api /opt/codegraph/bin/
sudo -u codegraph chmod +x /opt/codegraph/bin/*
```

### Method 2: Pre-built Binaries

```bash
# Download latest release
LATEST_VERSION=$(curl -s https://api.github.com/repos/codegraph/embedding-system/releases/latest | grep '"tag_name"' | sed -E 's/.*"([^"]+)".*/\1/')
wget https://github.com/codegraph/embedding-system/releases/download/${LATEST_VERSION}/codegraph-linux-x86_64.tar.gz

# Extract and install
sudo -u codegraph tar -xzf codegraph-linux-x86_64.tar.gz -C /opt/codegraph/
sudo -u codegraph chmod +x /opt/codegraph/bin/*
```

## Configuration

### 1. Directory Structure

```bash
sudo -u codegraph mkdir -p /opt/codegraph/{config,data,logs,scripts}
sudo -u codegraph mkdir -p /opt/codegraph/data/{rocksdb,cache,vectors}
```

### 2. Main Configuration File

Create `/opt/codegraph/config/config.toml`:

```toml
[server]
host = "0.0.0.0"
port = 8080
max_connections = 1000
request_timeout = 30
keep_alive_timeout = 75

[database]
# RocksDB configuration
path = "/opt/codegraph/data/rocksdb"
max_open_files = -1
write_buffer_size = 134217728  # 128MB
max_write_buffer_number = 3
target_file_size_base = 67108864  # 64MB
max_background_jobs = 6
bytes_per_sync = 1048576
compaction_style = "level"

# Enable level_compaction_dynamic_level_bytes for better space usage
level_compaction_dynamic_level_bytes = true

[cache]
# Cache configuration
block_cache_size = 1073741824  # 1GB
cache_index_and_filter_blocks = true
pin_l0_filter_and_index_blocks_in_cache = true

[vector]
# Vector search configuration
faiss_index_path = "/opt/codegraph/data/vectors"
embedding_dim = 768
index_type = "IVF"
nlist = 1024

[performance]
# Performance tuning
max_background_compactions = 4
max_background_flushes = 2
bloom_locality = 1
optimize_filters_for_memory = true

[compression]
# Compression settings
compression = "lz4"
bottommost_compression = "zstd"
compression_per_level = ["none", "none", "lz4", "lz4", "lz4", "zstd", "zstd"]

[logging]
level = "info"
file = "/opt/codegraph/logs/codegraph.log"
max_size = "100MB"
max_files = 10

[security]
# Security configuration
enable_https = true
cert_file = "/opt/codegraph/config/server.crt"
key_file = "/opt/codegraph/config/server.key"
require_auth = true

[api]
# API configuration
rate_limit_requests_per_second = 1000
max_request_size = 10485760  # 10MB
cors_enabled = true
cors_origins = ["https://your-frontend.com"]

[monitoring]
# Monitoring and metrics
enable_metrics = true
metrics_port = 9090
health_check_path = "/health"
```

### 3. Environment Configuration

Create `/opt/codegraph/config/.env`:

```bash
# Application settings
RUST_LOG=info
RUST_BACKTRACE=1

# Database settings
CODEGRAPH_DB_PATH=/opt/codegraph/data/rocksdb
CODEGRAPH_CACHE_SIZE=1073741824

# Security settings
CODEGRAPH_JWT_SECRET=your-strong-jwt-secret-here
CODEGRAPH_API_KEY=your-api-key-here

# Performance settings
CODEGRAPH_MAX_WORKERS=8
CODEGRAPH_THREAD_POOL_SIZE=16
```

### 4. Set Permissions

```bash
sudo chown -R codegraph:codegraph /opt/codegraph/config
sudo chmod 600 /opt/codegraph/config/.env
sudo chmod 644 /opt/codegraph/config/config.toml
```

## Database Setup

### 1. RocksDB Initialization

```bash
# Create database directories
sudo -u codegraph mkdir -p /opt/codegraph/data/rocksdb/{main,metadata,vectors}

# Initialize database (run as codegraph user)
sudo -u codegraph /opt/codegraph/bin/codegraph-api --init-db
```

### 2. Database Performance Tuning

Create `/opt/codegraph/scripts/tune-rocksdb.sh`:

```bash
#!/bin/bash
# RocksDB tuning script

# Set I/O scheduler for better performance
echo noop | sudo tee /sys/block/nvme0n1/queue/scheduler

# Increase max map count for memory-mapped files
echo 'vm.max_map_count = 262144' | sudo tee -a /etc/sysctl.conf

# Optimize dirty page writeback
echo 'vm.dirty_ratio = 5' | sudo tee -a /etc/sysctl.conf
echo 'vm.dirty_background_ratio = 2' | sudo tee -a /etc/sysctl.conf

# Apply changes
sudo sysctl -p
```

Make executable and run:

```bash
sudo chmod +x /opt/codegraph/scripts/tune-rocksdb.sh
sudo /opt/codegraph/scripts/tune-rocksdb.sh
```

## Service Configuration

### 1. Create Systemd Service

Create `/etc/systemd/system/codegraph.service`:

```ini
[Unit]
Description=CodeGraph API Server
Documentation=https://github.com/codegraph/embedding-system
After=network.target
Wants=network.target

[Service]
Type=exec
User=codegraph
Group=codegraph
ExecStart=/opt/codegraph/bin/codegraph-api --config /opt/codegraph/config/config.toml
ExecReload=/bin/kill -HUP $MAINPID
Restart=on-failure
RestartSec=5
TimeoutStopSec=30

# Environment
Environment=RUST_LOG=info
EnvironmentFile=/opt/codegraph/config/.env

# Working directory
WorkingDirectory=/opt/codegraph

# Security settings
NoNewPrivileges=true
PrivateTmp=true
ProtectSystem=strict
ProtectHome=true
ReadWritePaths=/opt/codegraph/data /opt/codegraph/logs

# Resource limits
LimitNOFILE=65536
LimitNPROC=4096

[Install]
WantedBy=multi-user.target
```

### 2. Enable and Start Service

```bash
# Reload systemd configuration
sudo systemctl daemon-reload

# Enable service to start on boot
sudo systemctl enable codegraph

# Start the service
sudo systemctl start codegraph

# Check status
sudo systemctl status codegraph
```

## Security Setup

### 1. Generate SSL Certificates

```bash
# Generate private key
sudo -u codegraph openssl genrsa -out /opt/codegraph/config/server.key 2048

# Generate certificate signing request
sudo -u codegraph openssl req -new -key /opt/codegraph/config/server.key \
    -out /opt/codegraph/config/server.csr \
    -subj "/C=US/ST=State/L=City/O=Organization/CN=codegraph.example.com"

# Generate self-signed certificate (for testing)
sudo -u codegraph openssl x509 -req -days 365 \
    -in /opt/codegraph/config/server.csr \
    -signkey /opt/codegraph/config/server.key \
    -out /opt/codegraph/config/server.crt

# Set permissions
sudo chmod 600 /opt/codegraph/config/server.key
sudo chmod 644 /opt/codegraph/config/server.crt
```

### 2. Firewall Configuration

```bash
# Ubuntu/Debian (ufw)
sudo ufw allow 8080/tcp
sudo ufw allow 9090/tcp  # metrics port
sudo ufw --force enable

# CentOS/RHEL (firewalld)
sudo firewall-cmd --permanent --add-port=8080/tcp
sudo firewall-cmd --permanent --add-port=9090/tcp
sudo firewall-cmd --reload
```

### 3. Generate API Keys

```bash
# Generate strong API key
API_KEY=$(openssl rand -base64 32)
echo "Generated API Key: $API_KEY"

# Update environment file
sudo -u codegraph sed -i "s/CODEGRAPH_API_KEY=.*/CODEGRAPH_API_KEY=$API_KEY/" /opt/codegraph/config/.env
```

## Performance Tuning

### 1. System-Level Optimizations

Create `/opt/codegraph/scripts/system-tuning.sh`:

```bash
#!/bin/bash
# System performance tuning for CodeGraph

# Increase file descriptor limits
echo '* soft nofile 65536' >> /etc/security/limits.conf
echo '* hard nofile 65536' >> /etc/security/limits.conf

# Optimize TCP settings
cat >> /etc/sysctl.conf << EOF
# TCP performance tuning
net.core.rmem_max = 16777216
net.core.wmem_max = 16777216
net.ipv4.tcp_rmem = 4096 87380 16777216
net.ipv4.tcp_wmem = 4096 65536 16777216
net.core.netdev_max_backlog = 5000
net.ipv4.tcp_congestion_control = bbr

# Memory management
vm.swappiness = 1
vm.dirty_ratio = 15
vm.dirty_background_ratio = 5
vm.vfs_cache_pressure = 50
EOF

# Apply settings
sysctl -p
```

### 2. RocksDB Specific Tuning

Update configuration based on your workload:

```toml
# For write-heavy workloads
[database]
write_buffer_size = 268435456  # 256MB
max_write_buffer_number = 6
level0_file_num_compaction_trigger = 8
level0_slowdown_writes_trigger = 17
level0_stop_writes_trigger = 24

# For read-heavy workloads
[cache]
block_cache_size = 4294967296  # 4GB
cache_index_and_filter_blocks = true
pin_l0_filter_and_index_blocks_in_cache = true

[performance]
bloom_locality = 1
optimize_filters_for_memory = true
```

## Verification

### 1. Health Check

```bash
# Check service status
sudo systemctl status codegraph

# Check if API is responding
curl -k https://localhost:8080/health

# Check logs
sudo journalctl -u codegraph -f
```

### 2. API Functionality Test

```bash
# Test basic API endpoints
curl -k -H "Authorization: Bearer YOUR_API_KEY" \
    https://localhost:8080/api/v1/status

# Test metrics endpoint
curl -k https://localhost:9090/metrics
```

### 3. Performance Verification

```bash
# Monitor resource usage
htop

# Check RocksDB statistics
curl -k -H "Authorization: Bearer YOUR_API_KEY" \
    https://localhost:8080/api/v1/stats/rocksdb

# Monitor I/O performance
iostat -x 1
```

## Troubleshooting

### Common Issues

#### 1. Service Fails to Start

**Check logs:**
```bash
sudo journalctl -u codegraph -n 50
```

**Common causes:**
- Missing dependencies
- Configuration file errors
- Permission issues
- Port conflicts

#### 2. High Memory Usage

**Monitor memory:**
```bash
sudo -u codegraph /opt/codegraph/bin/codegraph-api --memory-stats
```

**Solutions:**
- Reduce `block_cache_size`
- Adjust `write_buffer_size`
- Enable `optimize_filters_for_memory`

#### 3. Performance Issues

**Check system resources:**
```bash
top
iotop
netstat -i
```

**Tuning steps:**
1. Increase `max_background_jobs`
2. Adjust compression settings
3. Optimize RocksDB configuration
4. Monitor disk I/O patterns

#### 4. Connection Issues

**Verify network configuration:**
```bash
netstat -tlnp | grep 8080
ss -tlnp | grep 8080
```

**Check firewall:**
```bash
sudo ufw status
sudo iptables -L
```

### Log Locations

- Service logs: `journalctl -u codegraph`
- Application logs: `/opt/codegraph/logs/codegraph.log`
- System logs: `/var/log/syslog`

### Recovery Procedures

#### Database Corruption

```bash
# Stop service
sudo systemctl stop codegraph

# Backup current data
sudo -u codegraph cp -r /opt/codegraph/data/rocksdb /opt/codegraph/data/rocksdb.backup

# Attempt repair
sudo -u codegraph /opt/codegraph/bin/codegraph-api --repair-db

# Restart service
sudo systemctl start codegraph
```

#### Configuration Reset

```bash
# Backup current config
sudo cp /opt/codegraph/config/config.toml /opt/codegraph/config/config.toml.backup

# Reset to defaults
sudo -u codegraph /opt/codegraph/bin/codegraph-api --generate-config > /opt/codegraph/config/config.toml

# Restart service
sudo systemctl restart codegraph
```

For additional support, check the [operations runbook](OPERATIONS_RUNBOOK.md) and [troubleshooting guide](TROUBLESHOOTING_GUIDE.md).