# CodeGraph Test Suite Documentation

This document provides comprehensive documentation for the CodeGraph project's test infrastructure, covering how to run tests, what's tested across all components, test coverage analysis, and guidelines for contributing new tests.

## Table of Contents

1. [Quick Start](#quick-start)
2. [Test Execution Guide](#test-execution-guide)
3. [Test Coverage Overview](#test-coverage-overview)
4. [Performance & Benchmark Testing](#performance--benchmark-testing)
5. [Testing Architecture](#testing-architecture)
6. [Coverage Analysis](#coverage-analysis)
7. [Testing Guidelines](#testing-guidelines)
8. [CI Integration](#ci-integration)
9. [Troubleshooting](#troubleshooting)

## Quick Start

```bash
# Run all tests
make test
# or
cargo test --workspace

# Run tests with code coverage
make test-coverage

# Run performance benchmarks
make bench

# Run specific crate tests
cargo test -p codegraph-vector

# Run E2E/integration tests for API
make e2e
```

## Test Execution Guide

### 1. Basic Test Execution

**Run All Tests:**
```bash
# Using Makefile (recommended)
make test

# Direct cargo command
cargo test --workspace

# With verbose output
cargo test --workspace -- --nocapture

# Run tests in release mode for performance
cargo test --workspace --release
```

**Run Tests by Crate:**
```bash
# Vector operations and FAISS integration
cargo test -p codegraph-vector

# API endpoints and HTTP/GraphQL functionality  
cargo test -p codegraph-api

# Graph data structures and RocksDB operations
cargo test -p codegraph-graph

# Parser functionality for multiple languages
cargo test -p codegraph-parser

# Caching layer operations
cargo test -p codegraph-cache

# MCP (Model Context Protocol) integration
cargo test -p codegraph-mcp

# Queue operations and batch processing
cargo test -p codegraph-queue

# Git integration functionality
cargo test -p codegraph-git

# Load balancing functionality
cargo test -p codegraph-lb
```

**Run Specific Test Types:**
```bash
# Unit tests only
cargo test --workspace --lib

# Integration tests only
cargo test --workspace --test '*'

# Specific test by name
cargo test test_faiss_index_types --workspace

# Tests matching pattern
cargo test vector --workspace

# Async tests with tokio-test
cargo test async --workspace
```

### 2. Advanced Test Execution

**Feature Flag Testing:**
```bash
# Test with all features enabled
cargo test --workspace --all-features

# Test with specific features
cargo test -p codegraph-vector --features faiss

# Test without default features
cargo test --workspace --no-default-features
```

**Memory Safety Testing:**
```bash
# Run tests with Miri (memory safety checker)
make miri

# Run tests with Address Sanitizer
make asan-test

# Test with leak detection
make run-api-leaks
```

**Concurrent and Stress Testing:**
```bash
# High-performance stress tests
make sync-validate

# Load testing (requires k6)
make load-test

# Performance regression testing
make perf-regression
```

### 3. Test Coverage Analysis

**Generate Coverage Reports:**
```bash
# Generate HTML coverage report (requires cargo-tarpaulin)
make test-coverage

# Coverage with specific output format
cargo tarpaulin --workspace --out lcov --output-path lcov.info

# Coverage for specific crate
cargo tarpaulin -p codegraph-vector --out html
```

## Test Coverage Overview

### Coverage by Component

#### codegraph-core (100% critical paths covered)
- **What's Tested:**
  - Core types and traits (NodeId, CodeNode, CodeGraphError)
  - Error handling and propagation
  - Shared utilities and helper functions
  - Serialization/deserialization logic
- **Test Files:** Inline unit tests in `src/` modules
- **Key Test Areas:** Type safety, error conversion, UUID generation

#### codegraph-vector (85% coverage)
- **What's Tested:**
  - FAISS index operations (Flat, IVF, HNSW)
  - Vector embedding storage and retrieval  
  - Batch processing and optimization
  - Persistent storage with compression
  - Search accuracy and performance
  - RAG (Retrieval-Augmented Generation) system
- **Test Files:**
  - `tests/integration_tests.rs` - FAISS integration testing
  - `tests/knn_tests.rs` - K-nearest neighbor search
  - `tests/persistent_integration_tests.rs` - Storage persistence
  - `tests/rag_tests.rs` - RAG system functionality
  - `tests/embedding_provider_tests.rs` - Embedding providers
  - `tests/model_optimization_tests.rs` - Model optimization
- **Performance Tests:** Sub-200ms response time validation, accuracy benchmarks

#### codegraph-api (75% coverage)
- **What's Tested:**
  - REST API endpoints (/health, /parse, /graphql)
  - GraphQL schema and resolvers
  - HTTP/2 optimization features
  - Request/response serialization
  - Error handling and status codes
- **Test Files:**
  - `tests/api_integration.rs` - End-to-end API testing
  - `tests/health_monitoring_test.rs` - Health check functionality
  - `src/graphql/tests.rs` - GraphQL functionality
- **Integration Points:** Database connections, external service calls

#### codegraph-graph (60% coverage)
- **What's Tested:**
  - Graph data structure operations
  - RocksDB storage backend
  - Node and edge management
  - Graph traversal algorithms
- **Test Files:**
  - `tests/versioning_tests.rs` - Version control integration
- **Coverage Gaps:** Complex traversal scenarios, concurrent access patterns

#### codegraph-cache (20% coverage - Major Gap!)
- **What's Tested:** Currently minimal - mostly placeholder tests
- **Test Files:**
  - `tests/cache_tests.rs` - Contains TODO placeholders for most functionality
- **Critical Gaps:**
  - LRU cache operations
  - TTL and cache invalidation
  - Memory optimization
  - Concurrent access patterns
  - Persistence integration

#### codegraph-parser (70% coverage)
- **What's Tested:**
  - Tree-sitter parser integration
  - Multi-language support (Rust, Python, JavaScript, TypeScript, Go)
  - AST conversion and processing
- **Test Files:**
  - `src/tests.rs` and `src/tests/mod.rs` - Parser unit tests
  - `src/integration_tests.rs` - Language integration tests

#### codegraph-mcp (80% coverage)
- **What's Tested:**
  - Model Context Protocol integration
  - Server and client functionality
  - Protocol message handling
- **Test Files:**
  - `tests/integration_tests.rs` - MCP protocol testing
  - `tests/unit_tests.rs` - Individual component tests
  - `tests/benchmark_tests.rs` - Performance validation

### Test Infrastructure Components

#### Benchmarking System
- **Framework:** Criterion.rs for statistical benchmarking
- **Benchmark Files:**
  - `benches/embedding_benchmark.rs` - Vector operations
  - `benches/vector_benchmark.rs` - Search performance
  - `benches/parser_benchmark.rs` - Parsing speed
  - `benches/graph_benchmark.rs` - Graph operations
  - `crates/*/benches/*.rs` - Crate-specific benchmarks
- **Automation:** Automated via `scripts/run_benchmarks.sh`

#### Test Utilities
- **Async Testing:** `tokio-test` for async function testing
- **Temporary Resources:** `tempfile` for filesystem testing
- **Test Data Generation:** Deterministic test vector generation
- **Assertions:** Enhanced with `approx` for floating-point comparisons

## Performance & Benchmark Testing

### Running Benchmarks

**Basic Benchmark Execution:**
```bash
# Run all benchmarks with baseline saving
make bench

# Generate benchmark report
make bench-report

# Compare against baseline
make bench-compare

# Enable FAISS-dependent benchmarks
ENABLE_FAISS_BENCH=1 make bench
```

**Advanced Benchmark Options:**
```bash
# Run specific benchmark file
cargo bench --bench vector_benchmark

# Run benchmarks with custom baseline name
BASELINE_NAME=feature_branch make bench

# Performance regression detection
THRESHOLD=0.15 make perf-regression
```

### Performance Requirements

- **Vector Search:** Sub-millisecond response time for cached queries
- **RAG System:** Sub-200ms end-to-end response time
- **API Endpoints:** Sub-100ms for health checks, sub-1s for complex operations
- **Batch Processing:** Linear scaling with batch size
- **Memory Usage:** Efficient memory per vector ratios

### Benchmark Coverage

1. **Vector Operations**
   - FAISS index performance across different types
   - Search accuracy vs. speed trade-offs
   - Batch processing throughput

2. **Parser Performance**
   - Parsing speed per language
   - AST conversion efficiency
   - Memory usage patterns

3. **Graph Operations**
   - Traversal algorithm performance
   - Storage/retrieval latency
   - Concurrent access throughput

4. **API Performance**
   - Request/response latency
   - Throughput under load
   - Resource utilization

## Testing Architecture

### Test Organization Patterns

1. **Unit Tests** - Located in `src/` files alongside code
2. **Integration Tests** - Located in `tests/` directories per crate
3. **Benchmark Tests** - Located in `benches/` directories
4. **End-to-End Tests** - API integration tests simulating real usage

### Test Frameworks Used

- **Standard Library Testing** - Built-in Rust test framework
- **Tokio-Test** - Async testing utilities
- **Criterion** - Statistical benchmarking
- **axum-test** - HTTP API testing
- **tempfile** - Temporary resource management

### Test Data Management

- **Deterministic Generation** - Seeded random data for reproducible tests
- **Fixture Management** - Reusable test setups and teardowns  
- **Resource Cleanup** - Automatic cleanup of temporary files and connections

## Coverage Analysis

### High Coverage Areas ✅

1. **Core Functionality (codegraph-core)** - 100%
   - Critical path coverage complete
   - Error handling comprehensive
   - Type safety verified

2. **Vector Operations (codegraph-vector)** - 85%
   - Multiple FAISS index types tested
   - Performance characteristics validated
   - RAG system functionality covered

3. **MCP Integration (codegraph-mcp)** - 80%
   - Protocol implementation tested
   - Performance benchmarks included

### Medium Coverage Areas ⚠️

1. **API Layer (codegraph-api)** - 75%
   - Basic endpoint testing complete
   - Missing complex error scenarios
   - GraphQL testing partial

2. **Parser (codegraph-parser)** - 70%
   - Multi-language support tested
   - Edge cases in AST conversion need coverage

3. **Graph Operations (codegraph-graph)** - 60%
   - Basic operations covered
   - Complex traversal scenarios missing

### Critical Coverage Gaps ❌

1. **Cache Layer (codegraph-cache)** - 20%
   - **Critical Issue:** Most functionality has TODO placeholders
   - **Missing:** LRU operations, TTL handling, memory optimization
   - **Impact:** High - caching is performance-critical

2. **Concurrent Access Patterns** - Across multiple components
   - **Missing:** Race condition testing
   - **Missing:** Deadlock detection
   - **Missing:** Performance under concurrent load

3. **Error Recovery and Resilience**
   - **Missing:** Network failure simulation
   - **Missing:** Disk space exhaustion handling
   - **Missing:** Memory pressure scenarios

4. **Integration Testing**
   - **Missing:** End-to-end workflows across multiple crates
   - **Missing:** Real-world data scenarios
   - **Missing:** Performance under realistic loads

## Testing Guidelines

### Writing New Tests

#### Test Structure Standards

```rust
// Unit test example
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_feature_name() {
        // Arrange
        let input = setup_test_data();
        
        // Act
        let result = function_under_test(input);
        
        // Assert
        assert_eq!(result.status, ExpectedStatus::Success);
    }
}

// Async test example
#[tokio::test]
async fn test_async_feature() {
    let service = TestService::new().await;
    let result = service.async_operation().await;
    assert!(result.is_ok());
}
```

#### Integration Test Guidelines

```rust
// Integration test structure
use codegraph_core::*;
use tempfile::TempDir;
use tokio_test;

#[tokio::test]
async fn test_end_to_end_workflow() {
    // Setup
    let temp_dir = TempDir::new().unwrap();
    let config = TestConfig::new(temp_dir.path());
    
    // Test workflow
    let system = System::new(config).await.unwrap();
    let result = system.complete_workflow().await;
    
    // Verify
    assert!(result.is_ok());
    verify_side_effects(&temp_dir);
}
```

#### Performance Test Standards

```rust
// Performance test with Criterion
use criterion::{criterion_group, criterion_main, Criterion};

fn benchmark_critical_path(c: &mut Criterion) {
    c.bench_function("critical_operation", |b| {
        b.iter(|| {
            // Operation under test
            critical_operation(test_data())
        })
    });
}

criterion_group!(benches, benchmark_critical_path);
criterion_main!(benches);
```

### Test Naming Conventions

- **Unit Tests:** `test_[functionality]_[scenario]`
- **Integration Tests:** `test_[component]_[integration_point]`
- **Performance Tests:** `bench_[operation]_[conditions]`
- **Error Tests:** `test_[function]_[error_condition]_fails`

### Test Data Management

```rust
// Deterministic test data generation
fn generate_test_vectors(count: usize, dimension: usize, seed: u64) -> Vec<Vec<f32>> {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    
    (0..count)
        .map(|i| {
            let mut hasher = DefaultHasher::new();
            seed.hash(&mut hasher);
            i.hash(&mut hasher);
            // Generate deterministic data...
        })
        .collect()
}
```

### Adding Tests for New Features

1. **Before Implementation:**
   - Write failing tests that describe expected behavior
   - Cover happy path and edge cases
   - Include performance requirements

2. **During Implementation:**
   - Ensure tests pass incrementally
   - Add integration points
   - Validate error handling

3. **After Implementation:**
   - Add performance benchmarks
   - Test concurrent usage patterns
   - Verify resource cleanup

### Code Coverage Requirements

- **New Features:** Minimum 80% line coverage
- **Critical Paths:** 100% coverage required
- **Error Handling:** All error paths must be tested
- **Public APIs:** All public functions must have tests

## CI Integration

### GitHub Actions Integration

The project uses comprehensive CI workflows:

**Main CI Pipeline (`.github/workflows/ci.yml`):**
- Runs on Ubuntu, macOS, and Windows
- Tests stable and beta Rust toolchains
- Includes clippy linting and format checking
- Generates code coverage reports

**Performance Validation:**
- Automated benchmark execution
- Regression detection against baselines
- Performance metric collection

**Security Testing:**
- `cargo audit` for dependency vulnerabilities
- Memory safety testing with Miri
- Address sanitizer testing

### CI Commands

```bash
# Full CI check locally
make ci

# Individual CI components
make fmt-check      # Format verification
make lint          # Clippy linting
make test          # Test execution
make audit         # Security audit
```

### CI Performance Requirements

- **Test Execution Time:** Total test suite must complete within 10 minutes
- **Coverage Upload:** Automated coverage report upload to codecov
- **Artifact Storage:** Benchmark results stored for regression analysis

## Troubleshooting

### Common Test Issues

#### 1. FAISS-related Test Failures

**Symptom:** Tests fail with FAISS linking errors
```bash
# Solution: Ensure FAISS is available
# On macOS:
brew install faiss

# On Ubuntu:
sudo apt-get install libfaiss-dev

# Run tests with FAISS feature enabled
cargo test -p codegraph-vector --features faiss
```

#### 2. Async Test Timeouts

**Symptom:** Async tests hang or timeout
```rust
// Solution: Use proper async testing setup
#[tokio::test]
async fn test_with_timeout() {
    let timeout = Duration::from_secs(5);
    tokio::time::timeout(timeout, async_operation()).await
        .expect("Operation should complete within timeout");
}
```

#### 3. Temporary File Cleanup Issues

**Symptom:** Tests fail due to leftover files
```rust
// Solution: Use proper resource management
#[tokio::test]
async fn test_with_cleanup() {
    let temp_dir = TempDir::new().unwrap();
    // Test operations...
    // Automatic cleanup on drop
}
```

#### 4. Database Connection Issues

**Symptom:** RocksDB tests fail with lock errors
```bash
# Solution: Run tests single-threaded for database tests
cargo test test_database -- --test-threads=1
```

### Performance Test Issues

#### Benchmark Inconsistency

**Symptom:** Benchmark results vary significantly
```bash
# Solution: Use consistent environment
# Run with CPU frequency scaling disabled
sudo cpufreq-set -g performance

# Use longer sample periods
cargo bench -- --sample-size 1000
```

#### Memory Leak Detection

**Symptom:** Tests pass but memory usage grows
```bash
# Solution: Use leak detection features
make run-api-leaks

# Check memory reports
make leak-report
```

### Test Environment Setup

```bash
# Install all testing dependencies
make install-deps

# Verify test environment
cargo test --workspace --dry-run

# Check for missing test dependencies
cargo check --workspace --tests
```

### Debug Test Failures

```bash
# Run tests with debug output
RUST_LOG=debug cargo test test_name -- --nocapture

# Run single test with full output
cargo test specific_test_name -- --exact --nocapture

# Enable backtraces for test failures
RUST_BACKTRACE=1 cargo test
```

---

## Contributing Test Improvements

### Priority Areas for Test Enhancement

1. **Cache Layer Testing** (Critical Priority)
   - Implement comprehensive cache operation tests
   - Add TTL and invalidation testing
   - Test memory pressure scenarios

2. **Concurrent Access Testing** (High Priority)
   - Add race condition detection
   - Test deadlock prevention
   - Validate performance under load

3. **Error Recovery Testing** (Medium Priority)
   - Simulate network failures
   - Test disk space exhaustion
   - Validate graceful degradation

### Test Contribution Process

1. **Identify Coverage Gaps**
   ```bash
   make test-coverage
   # Review HTML report for uncovered areas
   ```

2. **Write Tests Following Guidelines**
   - Follow established patterns
   - Include both positive and negative test cases
   - Add performance validation where relevant

3. **Validate Changes**
   ```bash
   make dev  # Run format, lint, and test
   make bench  # Ensure no performance regressions
   ```

4. **Submit with Coverage Report**
   - Include before/after coverage metrics
   - Document test scenarios covered
   - Validate CI pipeline success

This documentation serves as the authoritative guide for testing in the CodeGraph project. Regular updates ensure it stays current with project evolution and testing best practices.