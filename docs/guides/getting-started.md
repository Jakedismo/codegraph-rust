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

# Getting Started with CodeGraph

This guide will help you set up CodeGraph and start analyzing your first codebase.

## Prerequisites

Before installing CodeGraph, ensure you have:

- **Rust 1.70+** with Cargo
- **System dependencies**:
  - `clang` (for native dependencies)
  - `cmake` (for building native libraries)
  - `git` (for version control)

### Installing Prerequisites

#### macOS
```bash
# Install Rust via rustup
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Install system dependencies via Homebrew
brew install cmake clang
```

#### Ubuntu/Debian
```bash
# Install Rust via rustup
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Install system dependencies
sudo apt update
sudo apt install build-essential cmake clang pkg-config
```

#### Windows
```powershell
# Install Rust via rustup
# Download and run rustup-init.exe from https://rustup.rs/

# Install Visual Studio Build Tools with C++ support
# Or install full Visual Studio with Desktop development with C++
```

## Installation

### 1. Clone the Repository

```bash
git clone <repository-url>
cd codegraph
```

### 2. Build the Project

```bash
# Build all crates
cargo build

# Or build in release mode for production
cargo build --release
```

### 3. Run Tests

Verify everything is working correctly:

```bash
# Run all tests
cargo test

# Run tests with output
cargo test -- --nocapture
```

### 4. Quick Development Check

```bash
# Run the full development pipeline
make dev

# Or run individual steps
make fmt    # Format code
make lint   # Run clippy lints
make test   # Run tests
```

## Your First CodeGraph Analysis

### 1. Start the API Server

```bash
# Start the REST API server
cargo run --bin codegraph-api

# The server will be available at http://localhost:8080
```

### 2. Basic Code Analysis

Create a simple Rust file to analyze:

```rust
// example.rs
fn main() {
    println!("Hello, CodeGraph!");
}

fn add_numbers(a: i32, b: i32) -> i32 {
    a + b
}

struct Calculator {
    name: String,
}

impl Calculator {
    fn new(name: String) -> Self {
        Calculator { name }
    }
    
    fn calculate(&self, x: i32, y: i32) -> i32 {
        add_numbers(x, y)
    }
}
```

### 3. Analyze with CodeGraph

```rust
use codegraph_core::*;
use codegraph_parser::Parser;
use codegraph_graph::GraphStorage;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize the parser
    let parser = Parser::new();
    
    // Parse the source file
    let nodes = parser.parse_file("example.rs")?;
    
    // Create graph storage
    let mut graph = GraphStorage::new("./example_graph")?;
    
    // Add parsed nodes to the graph
    graph.add_nodes(nodes)?;
    
    // Query for function definitions
    let functions = graph.find_nodes_by_type("function")?;
    println!("Found {} functions", functions.len());
    
    // Find dependencies for a specific node
    if let Some(main_fn) = functions.iter().find(|n| n.name == "main") {
        let deps = graph.find_dependencies(main_fn.id)?;
        println!("Main function depends on {} other nodes", deps.len());
    }
    
    Ok(())
}
```

### 4. Using the REST API

With the API server running, you can interact via HTTP:

```bash
# Health check
curl http://localhost:8080/health

# Parse a file (if endpoint exists)
curl -X POST http://localhost:8080/api/v1/parse \
  -H "Content-Type: application/json" \
  -d '{"file_path": "example.rs"}'

# Query the graph
curl http://localhost:8080/api/v1/graph/nodes
```

## Development Workflow

### 1. Watch Mode for Development

```bash
# Watch for changes and recompile
cargo watch -c -x check

# Watch and run tests
cargo watch -c -x test

# Development watch mode (if configured)
make watch
```

### 2. Running Benchmarks

```bash
# Run performance benchmarks
cargo bench

# Run specific benchmark
cargo bench graph_operations
```

### 3. Docker Development

```bash
# Build Docker image
docker build -t codegraph .

# Run with Docker Compose
docker-compose up -d

# View logs
docker-compose logs -f
```

## Common Operations

### Adding a New Language Parser

1. Add language support in `codegraph-parser`
2. Update tree-sitter grammar dependencies
3. Implement language-specific parsing rules
4. Add tests for the new language

### Configuring Vector Embeddings

1. Enable FAISS feature: `cargo build --features faiss`
2. Configure embedding model in settings
3. Initialize vector storage
4. Index your codebase for semantic search

### Performance Tuning

1. Adjust RocksDB settings in configuration
2. Tune batch sizes for bulk operations
3. Configure appropriate caching strategies
4. Monitor memory usage and optimize accordingly

## Next Steps

- **Read the [Architecture Overview](../architecture/UNIFIED_ARCHITECTURE_SPECIFICATION.md)** to understand system design
- **Check out [Tutorials](../tutorials/)** for specific use cases
- **Review [API Documentation](../api/)** for integration details
- **See [Performance Benchmarks](../specifications/performance_benchmarks.md)** for optimization tips

## Troubleshooting

### Build Issues

**Problem**: `cargo build` fails with linking errors
**Solution**: Ensure you have the required system dependencies (clang, cmake)

**Problem**: Tests fail with permission errors  
**Solution**: Check file permissions and ensure write access to test directories

**Problem**: API server won't start
**Solution**: Check if port 8080 is available, or configure a different port

### Performance Issues

**Problem**: Parsing is slow for large codebases
**Solution**: Enable parallel parsing and adjust batch sizes

**Problem**: High memory usage
**Solution**: Configure RocksDB cache limits and enable compression

For more detailed troubleshooting, see the [Troubleshooting Guide](../troubleshooting/).

## Getting Help

- **Documentation**: Browse the [docs directory](../index.md)
- **Issues**: Report bugs on GitHub Issues
- **Development**: Follow guidelines in [CLAUDE.md](../../CLAUDE.md)

---

**Next**: [CI/CD Setup](CI_CD_README.md) | **Up**: [Documentation Hub](../index.md)