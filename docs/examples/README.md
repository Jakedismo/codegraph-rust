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

# CodeGraph Examples

This directory contains practical examples demonstrating how to use CodeGraph for various code analysis tasks.

## 📚 Example Categories

### Basic Usage
- **[Simple Code Analysis](basic-analysis.md)** - Parse and analyze a single file
- **[Graph Operations](graph-operations.md)** - Working with the code graph
- **[Multi-language Support](multi-language.md)** - Parsing different programming languages

### Advanced Features
- **[Vector Embeddings](vector-embeddings.md)** - Semantic code search with FAISS
- **[Batch Processing](batch-processing.md)** - Analyzing entire codebases
- **[Custom Parsers](custom-parsers.md)** - Adding support for new languages

### API Integration
- **[REST API Usage](rest-api.md)** - HTTP API examples
- **[CLI Tools](cli-tools.md)** - Command-line interface examples
- **[Library Integration](library-integration.md)** - Using CodeGraph as a library

### Performance Optimization
- **[Large Codebase Analysis](large-codebase.md)** - Handling big projects efficiently
- **[Memory Optimization](memory-optimization.md)** - Managing memory usage
- **[Parallel Processing](parallel-processing.md)** - Concurrent analysis strategies

## 🚀 Quick Start Examples

### 1. Analyze a Single File

```rust
use codegraph_core::*;
use codegraph_parser::Parser;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let parser = Parser::new();
    let nodes = parser.parse_file("src/main.rs")?;
    
    println!("Found {} nodes", nodes.len());
    for node in &nodes {
        println!("- {}: {}", node.node_type, node.name);
    }
    
    Ok(())
}
```

### 2. Build a Code Graph

```rust
use codegraph_graph::GraphStorage;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut graph = GraphStorage::new("./code_graph")?;
    
    // Add nodes from parsing
    let nodes = vec![/* parsed nodes */];
    graph.add_nodes(nodes)?;
    
    // Query relationships
    let node_id = "function_main";
    let dependencies = graph.find_dependencies(node_id)?;
    let dependents = graph.find_dependents(node_id)?;
    
    println!("Dependencies: {}", dependencies.len());
    println!("Dependents: {}", dependents.len());
    
    Ok(())
}
```

### 3. REST API Query

```bash
# Start the API server
cargo run --bin codegraph-api &

# Parse a project
curl -X POST http://localhost:8080/api/v1/parse \
  -H "Content-Type: application/json" \
  -d '{"project_path": "/path/to/project"}'

# Query nodes
curl http://localhost:8080/api/v1/graph/nodes?type=function

# Search for specific patterns
curl "http://localhost:8080/api/v1/search?q=error+handling"
```

## 📖 Detailed Examples

Each example includes:
- **Purpose and use case**
- **Complete, runnable code**
- **Expected output**
- **Explanation of key concepts**
- **Common variations and extensions**

## 🔧 Running the Examples

### Prerequisites
Ensure you have CodeGraph built and ready:

```bash
cd /path/to/codegraph
cargo build
```

### Running Rust Examples

```bash
# Copy example code to a new file
cp docs/examples/basic-analysis.rs examples/
cd examples/

# Run the example
cargo run --bin basic-analysis
```

### Running API Examples

```bash
# Start the API server
cargo run --bin codegraph-api &

# Run the example scripts
bash docs/examples/rest-api-examples.sh
```

## 🎯 Use Case Examples

### Code Review and Analysis
- Finding unused functions
- Detecting circular dependencies
- Analyzing code complexity
- Identifying refactoring opportunities

### Documentation Generation
- Extracting API signatures
- Generating dependency graphs
- Creating architecture diagrams
- Building knowledge bases

### Migration and Refactoring
- Finding all usages of deprecated APIs
- Tracking dependency changes
- Analyzing impact of modifications
- Planning incremental migrations

### Code Search and Discovery
- Semantic code search
- Finding similar code patterns
- Locating implementation examples
- Cross-referencing related code

## 📁 Example Data

Some examples use sample projects located in:
- `examples/sample-rust-project/` - A small Rust project for testing
- `examples/sample-python-project/` - Python code examples
- `examples/sample-js-project/` - JavaScript/TypeScript examples

## 🤝 Contributing Examples

When adding new examples:

1. **Follow the template structure**
2. **Include complete, runnable code**
3. **Add clear explanations and comments**
4. **Test examples thoroughly**
5. **Update this README with new examples**

### Example Template

```markdown
# Example Title

## Purpose
Brief description of what this example demonstrates.

## Code
[Complete example code]

## Expected Output
[What the user should see when running the example]

## Explanation
[Detailed explanation of key concepts and techniques]

## Variations
[Common modifications and extensions]
```

## 📞 Support

If you have questions about the examples:
- Check the main [documentation](../index.md)
- Review [troubleshooting guide](../troubleshooting/)
- Open an issue on GitHub

---

**Navigation**: [Documentation Hub](../index.md) | [Getting Started](../guides/getting-started.md) | [API Reference](../api/)