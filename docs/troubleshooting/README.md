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

# Troubleshooting Guide

This guide covers common issues and their solutions when working with CodeGraph.

## 📋 Quick Diagnosis

### System Check Commands

```bash
# Check Rust installation
rustc --version
cargo --version

# Check system dependencies
which clang
which cmake
which git

# Check CodeGraph build
cargo check
cargo test --no-run
```

### Common Symptoms Quick Reference

| Symptom | Likely Cause | Quick Fix |
|---------|--------------|-----------|
| Build fails with linker errors | Missing system dependencies | Install clang/cmake |
| Tests fail with permission errors | File permissions | Check write access |
| API server won't start | Port already in use | Change port or kill process |
| High memory usage | Large codebase + default settings | Tune RocksDB limits |
| Slow parsing | Single-threaded processing | Enable parallel parsing |

## 🔧 Build and Installation Issues

### Issue: Cargo Build Fails with Linker Errors

**Symptoms:**
```
error: linking with `cc` failed: exit status: 1
note: ld: library not found for -lclang
```

**Causes:**
- Missing clang development libraries
- Incorrect library paths
- Outdated system packages

**Solutions:**

**macOS:**
```bash
# Install Xcode command line tools
xcode-select --install

# Install via Homebrew
brew install llvm cmake

# Set environment variables if needed
export LIBCLANG_PATH="/opt/homebrew/lib"
```

**Ubuntu/Debian:**
```bash
sudo apt update
sudo apt install build-essential clang libclang-dev cmake pkg-config

# For older versions, also install
sudo apt install llvm-dev
```

**Windows:**
```powershell
# Install Visual Studio Build Tools
# Or use chocolatey
choco install llvm cmake

# Set environment variables
$env:LIBCLANG_PATH = "C:\Program Files\LLVM\lib"
```

### Issue: Tree-sitter Build Failures

**Symptoms:**
```
error: failed to run custom build command for `tree-sitter`
```

**Solutions:**
```bash
# Update tree-sitter dependencies
cargo update tree-sitter

# Clean and rebuild
cargo clean
cargo build

# If issues persist, check for conflicts
cargo tree | grep tree-sitter
```

### Issue: FAISS Integration Problems

**Symptoms:**
```
error: could not find native static library `faiss`
```

**Solutions:**
```bash
# Install FAISS system-wide (optional feature)
# macOS
brew install faiss

# Ubuntu
sudo apt install libfaiss-dev

# Or build without FAISS
cargo build --no-default-features
```

## 🏃 Runtime Issues

### Issue: API Server Won't Start

**Symptoms:**
```
Error: Failed to bind to address 0.0.0.0:8080
Address already in use
```

**Solutions:**
```bash
# Check what's using the port
lsof -i :8080
netstat -tlnp | grep 8080

# Kill the process
kill -9 <PID>

# Or use a different port
cargo run --bin codegraph-api -- --port 8081

# Check configuration
export CODEGRAPH_PORT=8081
```

### Issue: Database Connection Errors

**Symptoms:**
```
Error: Failed to open RocksDB: IO error: lock hold by current process
```

**Solutions:**
```bash
# Check for stale locks
ls -la /path/to/database/LOCK

# Remove stale lock files (if safe)
rm /path/to/database/LOCK

# Check permissions
chmod 755 /path/to/database
chown -R $USER /path/to/database

# Use a different database path
export CODEGRAPH_DB_PATH="/tmp/codegraph_db"
```

### Issue: File Permission Errors

**Symptoms:**
```
Error: Permission denied (os error 13)
```

**Solutions:**
```bash
# Check file permissions
ls -la /path/to/files

# Fix permissions for data directory
chmod -R 755 /path/to/codegraph/data
chown -R $USER:$GROUP /path/to/codegraph/data

# Run with appropriate user
sudo -u codegraph cargo run

# Check SELinux/AppArmor if applicable
getenforce  # Should be Permissive or Disabled for testing
```

## 🚀 Performance Issues

### Issue: High Memory Usage

**Symptoms:**
- System becomes slow during analysis
- Out of memory errors
- Process killed by OOM killer

**Solutions:**

**RocksDB Configuration:**
```rust
// In your configuration
use rocksdb::{Options, DB};

let mut opts = Options::default();
opts.set_max_total_wal_size(256 * 1024 * 1024); // 256MB
opts.set_write_buffer_size(64 * 1024 * 1024);   // 64MB
opts.set_max_write_buffer_number(3);
opts.set_target_file_size_base(64 * 1024 * 1024);

// Enable compression
opts.set_compression_type(rocksdb::DBCompressionType::Lz4);
```

**System-level:**
```bash
# Monitor memory usage
htop
free -h

# Limit memory for the process
systemd-run --scope -p MemoryLimit=2G cargo run

# Increase swap if needed
sudo fallocate -l 2G /swapfile
sudo chmod 600 /swapfile
sudo mkswap /swapfile
sudo swapon /swapfile
```

### Issue: Slow Parsing Performance

**Symptoms:**
- Long analysis times for large codebases
- CPU usage stuck at single core

**Solutions:**

**Enable Parallel Processing:**
```rust
use rayon::prelude::*;

// Parse files in parallel
let results: Vec<_> = file_paths
    .par_iter()
    .map(|path| parser.parse_file(path))
    .collect();
```

**Batch Processing:**
```rust
// Process in batches to manage memory
const BATCH_SIZE: usize = 100;

for batch in file_paths.chunks(BATCH_SIZE) {
    let nodes: Vec<_> = batch
        .par_iter()
        .map(|path| parser.parse_file(path))
        .collect();
    
    graph.add_nodes_batch(nodes)?;
}
```

**Configuration:**
```bash
# Set appropriate thread count
export RAYON_NUM_THREADS=8

# Monitor CPU usage
htop -t  # Tree view to see thread usage
```

### Issue: Vector Search Performance

**Symptoms:**
- Slow semantic search queries
- High latency for similarity searches

**Solutions:**

**FAISS Index Optimization:**
```rust
// Use appropriate FAISS index type
use faiss::IndexImpl;

// For small datasets (< 1M vectors)
let index = IndexImpl::new_flat(dimension)?;

// For larger datasets
let index = IndexImpl::new_ivf_flat(dimension, nlist)?;

// For very large datasets
let index = IndexImpl::new_ivf_pq(dimension, nlist, m, bits)?;
```

**Batch Queries:**
```rust
// Query multiple vectors at once
let results = index.search_batch(&query_vectors, k)?;
```

## 🔍 Debugging Techniques

### Enabling Debug Logging

```bash
# Set log level
export RUST_LOG=debug
export RUST_LOG=codegraph=debug,rocksdb=info

# Run with logging
cargo run --bin codegraph-api 2>&1 | tee debug.log

# Filter specific components
export RUST_LOG="codegraph_parser=trace,codegraph_graph=debug"
```

### Memory Profiling

```bash
# Install valgrind (Linux)
sudo apt install valgrind

# Run with memory check
valgrind --tool=memcheck cargo run --bin codegraph-api

# Use heaptrack for detailed analysis
heaptrack cargo run --bin codegraph-api
heaptrack_gui heaptrack.*.gz
```

### Performance Profiling

```bash
# Install flamegraph
cargo install flamegraph

# Generate performance profile
cargo flamegraph --bin codegraph-api

# Use perf on Linux
perf record -g cargo run --bin codegraph-api
perf report
```

## 🧪 Testing Issues

### Issue: Tests Fail in CI/CD

**Common Causes:**
- Different environment from local
- Missing test dependencies
- Race conditions in parallel tests

**Solutions:**
```bash
# Run tests serially
cargo test -- --test-threads=1

# Set environment for tests
export TEST_DATABASE_PATH="/tmp/test_db"
export RUST_TEST_THREADS=1

# Clean test artifacts
cargo clean
rm -rf /tmp/test_*
```

### Issue: Flaky Tests

**Solutions:**
```rust
// Add proper cleanup
#[cfg(test)]
mod tests {
    use tempfile::TempDir;
    
    #[test]
    fn test_with_cleanup() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path();
        
        // Test code here
        
        // temp_dir automatically cleaned up
    }
}
```

## 📊 Monitoring and Health Checks

### Health Check Script

```bash
#!/bin/bash
# health_check.sh

echo "=== CodeGraph Health Check ==="

# Check API server
curl -f http://localhost:8080/health || echo "❌ API server not responding"

# Check database
if [ -d "./graph_db" ]; then
    echo "✅ Database directory exists"
else
    echo "❌ Database directory missing"
fi

# Check memory usage
MEMORY=$(ps -o pid,vsz,rss,comm -p $(pgrep codegraph))
echo "Memory usage: $MEMORY"

# Check disk space
df -h ./graph_db || echo "❌ Cannot check database disk usage"

echo "=== Health check complete ==="
```

### Automated Monitoring

```bash
# System monitoring with systemd
sudo systemctl status codegraph-api

# Log monitoring
journalctl -u codegraph-api -f

# Resource monitoring
watch 'ps aux | grep codegraph'
```

## 🆘 Getting Help

### Information to Include in Bug Reports

```bash
# System information
uname -a
rustc --version
cargo --version

# CodeGraph version
git rev-parse HEAD
cargo --version

# Error reproduction
RUST_LOG=debug cargo run --bin codegraph-api > debug.log 2>&1

# System resources
free -h
df -h
lscpu
```

### Before Reporting Issues

1. **Check this troubleshooting guide**
2. **Search existing GitHub issues**
3. **Try with minimal reproduction case**
4. **Include complete error messages**
5. **Provide system information**

### Support Channels

- **GitHub Issues**: Bug reports and feature requests
- **Documentation**: [docs/](../index.md) directory
- **Community**: Check for community forums or chat

---

**Navigation**: [Documentation Hub](../index.md) | [Getting Started](../guides/getting-started.md) | [Examples](../examples/)