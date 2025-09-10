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

# CodeGraph Troubleshooting Guide

## Table of Contents

1. [Common Issues](#common-issues)
2. [Service Issues](#service-issues)
3. [Performance Issues](#performance-issues)
4. [Database Issues](#database-issues)
5. [Network and Connectivity](#network-and-connectivity)
6. [Memory and Resource Issues](#memory-and-resource-issues)
7. [API and Request Issues](#api-and-request-issues)
8. [Vector Search Issues](#vector-search-issues)
9. [Configuration Issues](#configuration-issues)
10. [Log Analysis](#log-analysis)
11. [Diagnostic Tools](#diagnostic-tools)
12. [Recovery Procedures](#recovery-procedures)

## Common Issues

### Service Won't Start

#### Symptoms
- `systemctl status codegraph` shows "failed" or "inactive"
- Error messages in journal logs
- API endpoints not responding

#### Diagnosis Steps

```bash
# 1. Check service status
systemctl status codegraph -l

# 2. Check recent logs
journalctl -u codegraph -n 50 --no-pager

# 3. Check configuration syntax
/opt/codegraph/bin/codegraph-api --config /opt/codegraph/config/config.toml --check-config

# 4. Check file permissions
ls -la /opt/codegraph/bin/codegraph-api
ls -la /opt/codegraph/config/

# 5. Check port availability
netstat -tlnp | grep 8080
```

#### Common Causes and Solutions

**1. Configuration File Errors**
```bash
# Error: Failed to parse config file
# Solution: Validate TOML syntax
toml-cli validate /opt/codegraph/config/config.toml

# Fix common TOML issues:
# - Missing quotes around strings
# - Incorrect boolean values (use true/false, not True/False)
# - Invalid escape sequences
```

**2. Permission Issues**
```bash
# Error: Permission denied
# Solution: Fix permissions
sudo chown -R codegraph:codegraph /opt/codegraph/
sudo chmod +x /opt/codegraph/bin/codegraph-api
sudo chmod 600 /opt/codegraph/config/.env
```

**3. Port Already in Use**
```bash
# Error: Address already in use
# Solution: Find and stop conflicting process
sudo lsof -i :8080
sudo kill <PID>

# Or change port in config
sed -i 's/port = 8080/port = 8081/' /opt/codegraph/config/config.toml
```

**4. Missing Dependencies**
```bash
# Error: Library not found
# Solution: Install missing dependencies
sudo apt install -y libssl-dev libclang-dev

# For CentOS/RHEL:
sudo yum install -y openssl-devel clang-devel
```

---

### High CPU Usage

#### Symptoms
- CPU usage consistently above 80%
- Slow API response times
- High system load average

#### Diagnosis Steps

```bash
# 1. Check CPU usage by process
top -c -p $(pgrep codegraph)

# 2. Check CPU usage over time
sar -u 1 10

# 3. Profile the application
perf top -p $(pgrep codegraph)

# 4. Check for busy loops in logs
journalctl -u codegraph | grep -i "busy\|loop\|spin"

# 5. Check compaction activity
curl -H "Authorization: Bearer $API_KEY" \
     https://localhost:8080/stats/rocksdb | jq '.compaction_stats'
```

#### Common Causes and Solutions

**1. Heavy Compaction Activity**
```bash
# Check compaction stats
curl -s -H "Authorization: Bearer $API_KEY" \
     https://localhost:8080/stats/rocksdb | jq '.compaction_pending'

# Solution: Adjust compaction settings
# In config.toml:
[database]
max_background_jobs = 4  # Reduce from higher value
level0_file_num_compaction_trigger = 8  # Increase to delay compaction
```

**2. Vector Index Rebuilding**
```bash
# Check if index is rebuilding
curl -s -H "Authorization: Bearer $API_KEY" \
     https://localhost:8080/vector/index/stats | jq '.status'

# Solution: Wait for completion or schedule during low-traffic hours
```

**3. Inefficient Queries**
```bash
# Check slow queries in logs
journalctl -u codegraph | grep "slow_query\|timeout"

# Solution: Add query limits and optimize search parameters
```

---

### High Memory Usage

#### Symptoms
- Memory usage above 85%
- Out of memory errors
- Frequent garbage collection
- Slow performance

#### Diagnosis Steps

```bash
# 1. Check memory usage
free -h
cat /proc/$(pgrep codegraph)/status | grep -E "VmRSS|VmSize"

# 2. Check memory breakdown
curl -s -H "Authorization: Bearer $API_KEY" \
     https://localhost:8080/stats/memory

# 3. Check for memory leaks (if enabled)
curl -s -H "Authorization: Bearer $API_KEY" \
     https://localhost:8080/memory/stats

# 4. Check cache sizes
curl -s -H "Authorization: Bearer $API_KEY" \
     https://localhost:8080/stats/rocksdb | jq '.cache_usage'
```

#### Common Causes and Solutions

**1. Oversized Block Cache**
```bash
# Check current cache usage
curl -s -H "Authorization: Bearer $API_KEY" \
     https://localhost:8080/stats/rocksdb | jq '.block_cache'

# Solution: Reduce cache size in config.toml
[cache]
block_cache_size = 1073741824  # Reduce from 4GB to 1GB
```

**2. Too Many Write Buffers**
```bash
# Check write buffer usage
curl -s -H "Authorization: Bearer $API_KEY" \
     https://localhost:8080/stats/rocksdb | jq '.memtable_stats'

# Solution: Reduce write buffer configuration
[database]
write_buffer_size = 67108864  # Reduce from 256MB to 64MB
max_write_buffer_number = 3  # Reduce from 6 to 3
```

**3. Vector Index Size**
```bash
# Check vector index memory usage
curl -s -H "Authorization: Bearer $API_KEY" \
     https://localhost:8080/vector/index/stats | jq '.memory_usage_mb'

# Solution: Optimize index parameters or use disk-based index
```

## Service Issues

### Service Crashes Frequently

#### Diagnosis Steps

```bash
# 1. Check crash logs
journalctl -u codegraph | grep -A 10 -B 10 "segfault\|panic\|abort"

# 2. Check system logs for OOM killer
dmesg | grep -i "killed process\|out of memory"

# 3. Check core dumps
ls -la /var/crash/ /tmp/core.*

# 4. Monitor resource usage
watch -n 1 'ps aux | grep codegraph'
```

#### Solutions

**1. Memory-Related Crashes**
```bash
# Enable core dumps for analysis
echo 'kernel.core_pattern = /tmp/core.%e.%p.%t' | sudo tee -a /etc/sysctl.conf
sudo sysctl -p

# Reduce memory usage
# Update config.toml with lower memory limits
[cache]
block_cache_size = 536870912  # 512MB instead of 1GB

[database]
write_buffer_size = 33554432  # 32MB instead of 64MB
```

**2. Rust Panic Recovery**
```bash
# Enable panic logs
export RUST_BACKTRACE=1

# Add panic hook in service file
Environment=RUST_BACKTRACE=full
Environment=RUST_LOG=debug
```

---

### Service Becomes Unresponsive

#### Symptoms
- HTTP requests timeout
- Health check fails
- Service process exists but doesn't respond

#### Diagnosis Steps

```bash
# 1. Check if process is running but hung
ps aux | grep codegraph
kill -USR1 $(pgrep codegraph)  # Send signal to dump state

# 2. Check thread status
cat /proc/$(pgrep codegraph)/status | grep Threads
ls /proc/$(pgrep codegraph)/task/

# 3. Check for deadlocks in logs
journalctl -u codegraph | grep -i "deadlock\|blocked\|waiting"

# 4. Network connectivity test
nc -zv localhost 8080
```

#### Solutions

**1. Deadlock Recovery**
```bash
# Send interrupt signal to break deadlock
kill -INT $(pgrep codegraph)

# If unresponsive, force restart
systemctl restart codegraph
```

**2. Thread Pool Exhaustion**
```bash
# Increase thread pool size in config.toml
[performance]
max_workers = 16  # Increase from 8
thread_pool_size = 32  # Increase from 16
```

## Performance Issues

### Slow API Response Times

#### Diagnosis Steps

```bash
# 1. Measure response times
time curl -H "Authorization: Bearer $API_KEY" https://localhost:8080/health

# 2. Check per-endpoint metrics
curl -s https://localhost:9090/metrics | grep http_request_duration_seconds

# 3. Check database performance
curl -s -H "Authorization: Bearer $API_KEY" \
     https://localhost:8080/stats/rocksdb | jq '.read_stats'

# 4. Profile slow queries
journalctl -u codegraph | grep "slow_query" | tail -10
```

#### Solutions

**1. Database Optimization**
```bash
# Add more aggressive caching
# In config.toml:
[cache]
block_cache_size = 2147483648  # Increase to 2GB
cache_index_and_filter_blocks = true
pin_l0_filter_and_index_blocks_in_cache = true

[performance]
bloom_locality = 1
optimize_filters_for_memory = true
```

**2. Connection Pool Tuning**
```bash
# Increase connection limits
# In config.toml:
[server]
max_connections = 2000  # Increase from 1000
keep_alive_timeout = 120  # Increase from 75

# Add connection pooling
[database]
max_open_files = -1  # Keep all files open
```

---

### High Response Time Variance

#### Symptoms
- Some requests fast (<100ms), others very slow (>2s)
- Intermittent timeouts
- Unpredictable performance

#### Diagnosis Steps

```bash
# 1. Check response time distribution
curl -s https://localhost:9090/metrics | grep http_request_duration_seconds_bucket

# 2. Monitor compaction activity
watch -n 1 'curl -s -H "Authorization: Bearer $API_KEY" \
             https://localhost:8080/stats/rocksdb | jq .compaction_pending'

# 3. Check I/O wait times
iostat -x 1 5

# 4. Monitor garbage collection (if applicable)
# Look for GC pauses in application logs
```

#### Solutions

**1. Smooth Compaction**
```bash
# Distribute compaction load
# In config.toml:
[database]
level0_file_num_compaction_trigger = 4  # Earlier compaction
max_bytes_for_level_base = 536870912  # Smaller L1 size
bytes_per_sync = 1048576  # More frequent syncing
```

**2. Request Queuing**
```bash
# Add request buffering
# In config.toml:
[server]
request_buffer_size = 1024
request_timeout = 45  # Slightly higher timeout
```

## Database Issues

### RocksDB Corruption

#### Symptoms
- Database fails to open
- Read/write errors
- Checksum mismatch errors

#### Diagnosis Steps

```bash
# 1. Check database status
/opt/codegraph/bin/codegraph-api --check-db

# 2. Check for corruption in logs
journalctl -u codegraph | grep -i "corrupt\|checksum\|crc"

# 3. Manual database inspection
# Stop service first
systemctl stop codegraph

# Use RocksDB tools (if available)
rocksdb_dump --db=/opt/codegraph/data/rocksdb --summary_only
```

#### Recovery Steps

**1. Automatic Repair**
```bash
# Stop service
systemctl stop codegraph

# Backup current database
cp -r /opt/codegraph/data/rocksdb /opt/codegraph/data/rocksdb.backup

# Attempt repair
/opt/codegraph/bin/codegraph-api --repair-db

# Restart service
systemctl start codegraph

# Verify integrity
curl -X POST -H "Authorization: Bearer $API_KEY" \
     https://localhost:8080/integrity/check
```

**2. Restore from Backup**
```bash
# If repair fails, restore from backup
systemctl stop codegraph
rm -rf /opt/codegraph/data/rocksdb
./restore_backup.sh latest
systemctl start codegraph
```

---

### Write Stalls

#### Symptoms
- Write operations become very slow
- "Stopping writes" messages in logs
- High write amplification

#### Diagnosis Steps

```bash
# 1. Check write stall conditions
curl -s -H "Authorization: Bearer $API_KEY" \
     https://localhost:8080/stats/rocksdb | jq '.write_stall_stats'

# 2. Check L0 file count
curl -s -H "Authorization: Bearer $API_KEY" \
     https://localhost:8080/stats/rocksdb | jq '.level_stats[0].files'

# 3. Monitor compaction queue
curl -s -H "Authorization: Bearer $API_KEY" \
     https://localhost:8080/stats/rocksdb | jq '.compaction_pending'
```

#### Solutions

**1. Adjust Write Buffer Settings**
```bash
# In config.toml:
[database]
write_buffer_size = 134217728  # Increase to 128MB
max_write_buffer_number = 6  # Increase from 3
min_write_buffer_number_to_merge = 1  # Faster merging

# Compaction triggers
level0_file_num_compaction_trigger = 4  # Earlier compaction
level0_slowdown_writes_trigger = 12  # Higher threshold
level0_stop_writes_trigger = 20  # Higher threshold
```

**2. Increase Compaction Resources**
```bash
# In config.toml:
[database]
max_background_jobs = 8  # Increase from 6
bytes_per_sync = 1048576  # More frequent syncing

[performance]
max_background_compactions = 6  # Increase from 4
max_background_flushes = 3  # Increase from 2
```

## Network and Connectivity

### Connection Timeouts

#### Symptoms
- Clients receive timeout errors
- "Connection reset by peer" errors
- Intermittent connectivity issues

#### Diagnosis Steps

```bash
# 1. Check network connectivity
netstat -tlnp | grep 8080
ss -tlnp | grep 8080

# 2. Check connection limits
cat /proc/sys/net/core/somaxconn
ulimit -n

# 3. Monitor active connections
watch -n 1 'netstat -an | grep :8080 | wc -l'

# 4. Check firewall rules
sudo iptables -L -n
sudo ufw status
```

#### Solutions

**1. Increase Connection Limits**
```bash
# System-wide limits
echo 'net.core.somaxconn = 4096' | sudo tee -a /etc/sysctl.conf
echo 'net.ipv4.tcp_max_syn_backlog = 4096' | sudo tee -a /etc/sysctl.conf
sudo sysctl -p

# Service limits
# In /etc/systemd/system/codegraph.service:
[Service]
LimitNOFILE=65536
```

**2. TCP Tuning**
```bash
# In /etc/sysctl.conf:
net.ipv4.tcp_fin_timeout = 30
net.ipv4.tcp_keepalive_time = 120
net.ipv4.tcp_keepalive_intvl = 30
net.ipv4.tcp_keepalive_probes = 3
```

---

### TLS/SSL Issues

#### Symptoms
- SSL handshake failures
- Certificate validation errors
- "SSL_ERROR_*" messages

#### Diagnosis Steps

```bash
# 1. Test SSL configuration
openssl s_client -connect localhost:8080 -servername localhost

# 2. Check certificate validity
openssl x509 -in /opt/codegraph/config/server.crt -text -noout

# 3. Verify certificate chain
curl -vvv https://localhost:8080/health

# 4. Check cipher suites
nmap --script ssl-enum-ciphers -p 8080 localhost
```

#### Solutions

**1. Certificate Issues**
```bash
# Generate new self-signed certificate
openssl req -x509 -newkey rsa:4096 -keyout /opt/codegraph/config/server.key \
        -out /opt/codegraph/config/server.crt -days 365 -nodes \
        -subj "/CN=localhost"

# Set proper permissions
chown codegraph:codegraph /opt/codegraph/config/server.*
chmod 600 /opt/codegraph/config/server.key
chmod 644 /opt/codegraph/config/server.crt
```

**2. TLS Configuration**
```bash
# Modern TLS settings in config.toml:
[security]
tls_min_version = "1.2"
cipher_suites = ["TLS_AES_256_GCM_SHA384", "TLS_CHACHA20_POLY1305_SHA256"]
```

## Memory and Resource Issues

### Memory Leaks

#### Symptoms
- Continuously increasing memory usage
- Eventually triggers OOM killer
- Performance degradation over time

#### Diagnosis Steps

```bash
# 1. Monitor memory growth
watch -n 10 'cat /proc/$(pgrep codegraph)/status | grep VmRSS'

# 2. Check for leak detection (if enabled)
curl -s -H "Authorization: Bearer $API_KEY" \
     https://localhost:8080/memory/leaks

# 3. Memory profiling
valgrind --tool=memcheck --leak-check=full \
         /opt/codegraph/bin/codegraph-api --config /opt/codegraph/config/config.toml

# 4. Check cache growth
curl -s -H "Authorization: Bearer $API_KEY" \
     https://localhost:8080/stats/rocksdb | jq '.cache_usage'
```

#### Solutions

**1. Enable Memory Monitoring**
```bash
# Compile with leak detection (if available)
# Restart with memory monitoring
systemctl stop codegraph

# Add memory monitoring to service
Environment=CODEGRAPH_ENABLE_MEMORY_TRACKING=true
systemctl daemon-reload
systemctl start codegraph
```

**2. Cache Size Limits**
```bash
# Implement strict cache limits
# In config.toml:
[cache]
block_cache_size = 1073741824  # Fixed 1GB limit
strict_capacity_limit = true  # Enforce limits
```

---

### File Descriptor Exhaustion

#### Symptoms
- "Too many open files" errors
- Cannot create new connections
- Database operations fail

#### Diagnosis Steps

```bash
# 1. Check current usage
lsof -p $(pgrep codegraph) | wc -l
cat /proc/$(pgrep codegraph)/limits | grep "Max open files"

# 2. Check system limits
ulimit -n
cat /proc/sys/fs/file-max

# 3. Identify file types
lsof -p $(pgrep codegraph) | awk '{print $5}' | sort | uniq -c | sort -nr
```

#### Solutions

**1. Increase File Descriptor Limits**
```bash
# In /etc/security/limits.conf:
codegraph soft nofile 65536
codegraph hard nofile 65536

# In systemd service file:
[Service]
LimitNOFILE=65536

# Restart service
systemctl daemon-reload
systemctl restart codegraph
```

**2. Database Configuration**
```bash
# Optimize file usage in config.toml:
[database]
max_open_files = 8192  # Limit instead of -1
table_cache_numshardbits = 6  # Better file sharing
```

## API and Request Issues

### Request Parsing Errors

#### Symptoms
- 400 Bad Request responses
- JSON parsing errors
- Malformed request errors

#### Diagnosis Steps

```bash
# 1. Check request logs
journalctl -u codegraph | grep -i "parse\|json\|malformed"

# 2. Test with valid requests
curl -X POST -H "Content-Type: application/json" \
     -H "Authorization: Bearer $API_KEY" \
     -d '{"file_path": "/test/file.rs"}' \
     https://localhost:8080/parse

# 3. Check content-type handling
curl -v -X POST -H "Content-Type: text/plain" \
     -d "invalid json" https://localhost:8080/parse
```

#### Solutions

**1. Input Validation**
```bash
# Check API documentation for correct request format
# Ensure Content-Type: application/json is set
# Validate JSON syntax before sending
```

**2. Request Size Limits**
```bash
# In config.toml:
[api]
max_request_size = 10485760  # 10MB limit
max_json_depth = 100  # Prevent deeply nested JSON
```

---

### Authentication Failures

#### Symptoms
- 401 Unauthorized responses
- "Invalid API key" errors
- Authentication middleware errors

#### Diagnosis Steps

```bash
# 1. Verify API key format
echo $API_KEY | base64 -d  # Should decode properly if base64

# 2. Check authentication logs
journalctl -u codegraph | grep -i "auth\|unauthorized\|token"

# 3. Test authentication
curl -v -H "Authorization: Bearer $API_KEY" \
     https://localhost:8080/health

# 4. Check header format
curl -H "Authorization: Bearer wrong-key" \
     https://localhost:8080/health
```

#### Solutions

**1. API Key Verification**
```bash
# Check current API key in config
grep CODEGRAPH_API_KEY /opt/codegraph/config/.env

# Generate new API key if needed
NEW_KEY=$(openssl rand -base64 32)
sed -i "s/CODEGRAPH_API_KEY=.*/CODEGRAPH_API_KEY=$NEW_KEY/" /opt/codegraph/config/.env
systemctl reload codegraph
```

**2. Authentication Configuration**
```bash
# In config.toml:
[security]
require_auth = true
api_key_header = "Authorization"  # Default
jwt_secret = "your-jwt-secret"  # For JWT tokens
```

## Vector Search Issues

### Vector Index Problems

#### Symptoms
- Vector search returns no results
- "Index not found" errors
- Very slow vector searches

#### Diagnosis Steps

```bash
# 1. Check index status
curl -s -H "Authorization: Bearer $API_KEY" \
     https://localhost:8080/vector/index/stats

# 2. Check index configuration
curl -s -H "Authorization: Bearer $API_KEY" \
     https://localhost:8080/vector/index/config

# 3. Monitor search performance
curl -s -H "Authorization: Bearer $API_KEY" \
     https://localhost:8080/vector/performance

# 4. Check vector storage
ls -la /opt/codegraph/data/vectors/
```

#### Solutions

**1. Rebuild Vector Index**
```bash
# Rebuild with optimal parameters
curl -X POST -H "Authorization: Bearer $API_KEY" \
     -H "Content-Type: application/json" \
     -d '{
       "index_type": "HNSW",
       "parameters": {
         "M": 16,
         "efConstruction": 200,
         "efSearch": 100
       },
       "force": true
     }' \
     https://localhost:8080/vector/index/rebuild
```

**2. Index Configuration Tuning**
```bash
# In config.toml:
[vector]
embedding_dim = 768  # Match your embeddings
index_type = "HNSW"  # or "IVF" for large datasets
nlist = 1024  # For IVF index

[vector.hnsw]
M = 16  # Connections per layer
efConstruction = 200  # Build quality
efSearch = 100  # Search quality
```

---

### Embedding Generation Issues

#### Symptoms
- "Failed to generate embeddings" errors
- Vector search returns poor results
- Embedding dimension mismatches

#### Diagnosis Steps

```bash
# 1. Check embedding service status
curl -s -H "Authorization: Bearer $API_KEY" \
     https://localhost:8080/vector/embedding/health

# 2. Test embedding generation
curl -X POST -H "Authorization: Bearer $API_KEY" \
     -H "Content-Type: application/json" \
     -d '{"text": "test function"}' \
     https://localhost:8080/vector/embed

# 3. Check embedding dimensions
curl -s -H "Authorization: Bearer $API_KEY" \
     https://localhost:8080/vector/index/stats | jq '.dimension'
```

#### Solutions

**1. Embedding Service Configuration**
```bash
# In config.toml:
[vector.embedding]
model_name = "sentence-transformers/all-MiniLM-L6-v2"
batch_size = 32
max_length = 512
device = "cpu"  # or "cuda" if available
```

**2. Dimension Compatibility**
```bash
# Ensure all embeddings have same dimension
# Rebuild index if dimension changed
curl -X POST -H "Authorization: Bearer $API_KEY" \
     -d '{"force": true}' \
     https://localhost:8080/vector/index/rebuild
```

## Configuration Issues

### Configuration File Validation

#### Common Configuration Errors

```bash
# 1. Invalid TOML syntax
# Use toml validator
python3 -c "import toml; toml.load('/opt/codegraph/config/config.toml')"

# 2. Missing required fields
/opt/codegraph/bin/codegraph-api --validate-config

# 3. Invalid data types
# Check for:
# - String values without quotes
# - Boolean values as strings
# - Numeric values as strings
```

#### Environment Variable Issues

```bash
# 1. Check environment file syntax
# No spaces around = sign
# Quote values with special characters
cat /opt/codegraph/config/.env

# 2. Verify environment variables are loaded
systemctl show codegraph | grep Environment

# 3. Test variable expansion
sudo -u codegraph env | grep CODEGRAPH
```

## Log Analysis

### Log Patterns for Common Issues

#### Error Patterns to Search For

```bash
# 1. Memory issues
journalctl -u codegraph | grep -E "out of memory|oom|allocation failed"

# 2. Database issues
journalctl -u codegraph | grep -E "rocksdb|database|corruption|io error"

# 3. Network issues
journalctl -u codegraph | grep -E "connection|timeout|refused|reset"

# 4. Performance issues
journalctl -u codegraph | grep -E "slow|timeout|stall|blocked"

# 5. Authentication issues
journalctl -u codegraph | grep -E "auth|unauthorized|forbidden|token"
```

#### Log Analysis Scripts

```bash
#!/bin/bash
# Log analysis script

echo "=== CodeGraph Log Analysis ==="

# Error frequency in last hour
echo "Errors in last hour:"
journalctl -u codegraph --since "1 hour ago" | grep -i error | wc -l

# Warning frequency
echo "Warnings in last hour:"
journalctl -u codegraph --since "1 hour ago" | grep -i warning | wc -l

# Most common errors
echo "Most common errors:"
journalctl -u codegraph --since "24 hours ago" | grep -i error | \
  awk '{for(i=4;i<=NF;i++) printf "%s ", $i; print ""}' | \
  sort | uniq -c | sort -nr | head -5

# Performance warnings
echo "Performance issues:"
journalctl -u codegraph --since "24 hours ago" | grep -E "slow|timeout|stall"

# Memory warnings
echo "Memory issues:"
journalctl -u codegraph --since "24 hours ago" | grep -E "memory|oom|allocation"
```

## Diagnostic Tools

### Health Check Script

```bash
#!/bin/bash
# Comprehensive health check

echo "=== CodeGraph Diagnostic Report ==="
echo "Generated at: $(date)"
echo

# Service status
echo "1. SERVICE STATUS"
systemctl is-active codegraph
systemctl is-enabled codegraph
echo

# Process information
echo "2. PROCESS INFORMATION"
ps aux | grep codegraph | grep -v grep
echo

# Resource usage
echo "3. RESOURCE USAGE"
echo "CPU: $(top -bn1 | grep codegraph | awk '{print $9}')"
echo "Memory: $(cat /proc/$(pgrep codegraph)/status | grep VmRSS)"
echo "Open files: $(lsof -p $(pgrep codegraph) | wc -l)"
echo

# Network status
echo "4. NETWORK STATUS"
netstat -tlnp | grep 8080
echo

# API health
echo "5. API HEALTH"
curl -s -w "Response time: %{time_total}s\nHTTP code: %{http_code}\n" \
     https://localhost:8080/health
echo

# Database status
echo "6. DATABASE STATUS"
curl -s -H "Authorization: Bearer $API_KEY" \
     https://localhost:8080/stats/rocksdb | jq -r '.status, .total_keys'
echo

# Disk usage
echo "7. DISK USAGE"
df -h /opt/codegraph/data
echo

# Recent errors
echo "8. RECENT ERRORS (last 10)"
journalctl -u codegraph --since "1 hour ago" | grep -i error | tail -10
```

### Performance Monitoring Script

```bash
#!/bin/bash
# Performance monitoring script

DURATION=${1:-60}  # Default 60 seconds
INTERVAL=${2:-5}   # Default 5 seconds

echo "Monitoring CodeGraph performance for ${DURATION} seconds..."

# Create monitoring log
LOG_FILE="/tmp/codegraph-perf-$(date +%Y%m%d_%H%M%S).log"

for ((i=0; i<DURATION; i+=INTERVAL)); do
    echo "=== $(date) ===" >> $LOG_FILE
    
    # CPU and Memory
    top -bn1 | grep codegraph >> $LOG_FILE
    
    # API response time
    response_time=$(curl -s -w "%{time_total}" -o /dev/null https://localhost:8080/health)
    echo "API response time: ${response_time}s" >> $LOG_FILE
    
    # Active connections
    connections=$(netstat -an | grep :8080 | grep ESTABLISHED | wc -l)
    echo "Active connections: $connections" >> $LOG_FILE
    
    # Database stats
    curl -s -H "Authorization: Bearer $API_KEY" \
         https://localhost:8080/stats/rocksdb | \
         jq -r '.compaction_pending, .cache_usage' >> $LOG_FILE
    
    echo >> $LOG_FILE
    sleep $INTERVAL
done

echo "Performance monitoring complete. Log saved to: $LOG_FILE"

# Generate summary
echo "=== PERFORMANCE SUMMARY ==="
echo "Average API response time:"
grep "API response time" $LOG_FILE | awk '{sum+=$4; count++} END {print sum/count "s"}'

echo "Peak memory usage:"
grep codegraph $LOG_FILE | awk '{print $6}' | sort -nr | head -1

echo "Peak CPU usage:"
grep codegraph $LOG_FILE | awk '{print $9}' | sort -nr | head -1
```

## Recovery Procedures

### Service Recovery

```bash
#!/bin/bash
# Automated service recovery

echo "=== Service Recovery Procedure ==="

# 1. Check if service is running
if systemctl is-active --quiet codegraph; then
    echo "Service is running, checking health..."
    
    # Test API health
    if curl -s -f https://localhost:8080/health > /dev/null; then
        echo "Service is healthy"
        exit 0
    else
        echo "Service is running but unhealthy"
    fi
else
    echo "Service is not running"
fi

# 2. Attempt soft restart
echo "Attempting graceful restart..."
systemctl restart codegraph
sleep 10

# 3. Check if restart was successful
if systemctl is-active --quiet codegraph; then
    echo "Restart successful"
    
    # Wait for API to be ready
    for i in {1..30}; do
        if curl -s -f https://localhost:8080/health > /dev/null; then
            echo "API is healthy after restart"
            exit 0
        fi
        sleep 2
    done
    
    echo "API is not responding after restart"
else
    echo "Restart failed"
fi

# 4. Check for common issues
echo "Diagnosing issues..."

# Check disk space
disk_usage=$(df /opt/codegraph/data | awk 'NR==2 {print $5}' | sed 's/%//')
if [ $disk_usage -gt 95 ]; then
    echo "CRITICAL: Disk usage is ${disk_usage}%"
    # Clean up old logs
    find /opt/codegraph/logs -name "*.log.gz" -mtime +7 -delete
fi

# Check memory
mem_usage=$(free | grep Mem | awk '{printf "%.0f", $3/$2 * 100.0}')
if [ $mem_usage -gt 95 ]; then
    echo "CRITICAL: Memory usage is ${mem_usage}%"
    # Clear system caches
    sync && echo 3 > /proc/sys/vm/drop_caches
fi

# Check configuration
echo "Validating configuration..."
if ! /opt/codegraph/bin/codegraph-api --check-config; then
    echo "Configuration validation failed"
    exit 1
fi

# 5. Force restart
echo "Attempting force restart..."
systemctl stop codegraph
sleep 5
killall -9 codegraph 2>/dev/null || true
systemctl start codegraph

# 6. Final health check
sleep 15
if systemctl is-active --quiet codegraph && \
   curl -s -f https://localhost:8080/health > /dev/null; then
    echo "Service recovery successful"
    exit 0
else
    echo "Service recovery failed - manual intervention required"
    exit 1
fi
```

For additional support and escalation procedures, see the [Operations Runbook](OPERATIONS_RUNBOOK.md).

For architectural context, see the [Architecture Documentation](ARCHITECTURE_DOCUMENTATION.md).