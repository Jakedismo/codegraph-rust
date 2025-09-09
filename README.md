# CodeGraph

A sophisticated code analysis and embedding system built in Rust. CodeGraph provides graph-based code representation, vector search capabilities, and a comprehensive API for code understanding and analysis.

## Overview

CodeGraph transforms source code into intelligent, searchable knowledge graphs that enable advanced code understanding, analysis, and retrieval. Built with performance and scalability in mind, it supports multiple programming languages and provides both REST API access and embedding capabilities.

### Key Features

- **Multi-language Support** - Parse and analyze Rust, Python, JavaScript, TypeScript, Go, and more
- **Graph-based Analysis** - Rich code relationships and dependency tracking
- **Vector Embeddings** - Semantic code search using FAISS vector similarity
- **High Performance** - RocksDB storage with optimized graph operations
- **REST API** - Comprehensive HTTP API for integration
- **Thread-safe Operations** - Concurrent processing with Rust's safety guarantees

## Architecture

CodeGraph uses a modular workspace structure with specialized crates:

```
crates/
├── codegraph-core/     # Core types, traits, and shared functionality
├── codegraph-graph/    # Graph data structures and RocksDB storage  
├── codegraph-parser/   # Tree-sitter based code parsing
├── codegraph-vector/   # Vector embeddings and FAISS search
└── codegraph-api/      # REST API server using Axum
```

## Quick Start

### Prerequisites

- Rust 1.70+ with Cargo
- System dependencies: clang, cmake (for native dependencies)

### Building

```bash
# Clone the repository
git clone <repository-url>
cd codegraph

# Build all crates
cargo build

# Run tests
cargo test

# Development with watching
make dev
```

### Running the API Server

```bash
# Start the REST API server
cargo run --bin codegraph-api

# The server will be available at http://localhost:8080
```

### Basic Usage

```rust
use codegraph_core::*;
use codegraph_parser::Parser;
use codegraph_graph::GraphStorage;

// Parse a source file
let parser = Parser::new();
let nodes = parser.parse_file("src/main.rs")?;

// Store in graph
let mut graph = GraphStorage::new("./graph_db")?;
graph.add_nodes(nodes)?;

// Query relationships
let deps = graph.find_dependencies(node_id)?;
```

## Documentation

Comprehensive documentation is organized in the `docs/` directory:

### 📚 [Documentation Hub](docs/index.md)
Central hub for all project documentation

### 🏗️ Architecture
- [CodeGraph RAG Architecture](docs/architecture/CODEGRAPH_RAG_ARCHITECTURE.md)
- [Unified Architecture Specification](docs/architecture/UNIFIED_ARCHITECTURE_SPECIFICATION.md)
- [REST API Architecture](docs/architecture/REST_API_ARCHITECTURE.md)

### 🔧 API Documentation
- [MCP Server Specification](docs/api/codegraph-mcp-spec.md)

### 📖 Guides
- [Getting Started](docs/guides/startup.md)
- [CI/CD Setup](docs/guides/CI_CD_README.md)

### 📋 Specifications
- [RAG Integration Specifications](docs/specifications/RAG_INTEGRATION_SPECIFICATIONS.md)
- [Implementation Plan](docs/specifications/IMPLEMENTATION_PLAN.md)
- [Phase 1 Roadmap](docs/specifications/PHASE_1_IMPLEMENTATION_ROADMAP.md)
- [Feature Inventory](docs/specifications/FEATURE_INVENTORY.md)
- [Technical Implementation](docs/specifications/CodeGraph-Technical-Implementation.md)
- [Performance Benchmarks](docs/specifications/performance_benchmarks.md)
- [And more...](docs/specifications/)

## Development

### Build Commands

```bash
# Quick development check
make quick         # Format and lint only
make dev          # Full development check (format, lint, test)

# Individual commands
cargo fmt         # Format code
cargo clippy      # Lint code
cargo test        # Run tests
cargo bench       # Run benchmarks
```

### Watch Mode

```bash
# Watch for changes and recompile
cargo watch -c -x check

# Watch and run tests
cargo watch -c -x test

# Development watch mode
make watch
```

### Docker

```bash
# Build Docker image
docker build -t codegraph .

# Run with Docker Compose
docker-compose up -d
```

## Contributing

1. **Code Standards**: Follow the guidelines in [CLAUDE.md](CLAUDE.md)
2. **Testing**: Add tests for new features and ensure all tests pass
3. **Documentation**: Update documentation for API changes
4. **Performance**: Maintain performance standards as documented

### Pull Request Process

1. Run `make dev` to ensure code quality
2. Add/update tests for changes
3. Update relevant documentation
4. Submit PR with clear description

## Performance

CodeGraph is designed for high performance:

- **Concurrent Operations**: Thread-safe graph operations using DashMap and parking_lot
- **Optimized Storage**: RocksDB with appropriate column families and caching
- **Efficient Parsing**: Tree-sitter based parsing with visitor patterns
- **Vector Search**: FAISS integration for fast similarity search
- **Batch Processing**: Bulk operations for improved throughput

See [performance benchmarks](docs/specifications/performance_benchmarks.md) for detailed metrics.

## License

[License information]

## Support

- **Issues**: Please report bugs and feature requests via GitHub Issues
- **Documentation**: See the [docs/](docs/) directory for detailed information
- **Development**: Follow the development setup in [CLAUDE.md](CLAUDE.md)

---

Built with ❤️ in Rust for advanced code understanding and analysis.