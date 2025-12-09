# CodeGraph Parser (`codegraph-parser`)

## Overview
`codegraph-parser` handles the transformation of raw source code into structured `ExtractionResult`s. It leverages `tree-sitter` for robust, multi-language parsing.

## Key Features

### Unified Extraction
The core innovation is the `parse_content_with_unified_extraction` function.
- **Single Pass**: It traverses the AST once to extract both structural nodes (classes, functions) and their relationships (calls, inheritance).
- **Efficiency**: Avoids multiple walks of the syntax tree.

### Language Support
Supports a wide array of languages via `tree-sitter` grammars:
- Rust, Python, JavaScript, TypeScript, Go, Java, C++, and more.

### Architecture
- **Input**: Raw file content + File Path.
- **Output**: `ExtractionResult` (defined in `codegraph-core`).
- **Isolation**: This crate knows *how* to parse but doesn't know *where* the data goes (DB, Network, etc.).

## Usage
```rust
let parser = TreeSitterParser::new();
let result = parser.parse_content_with_unified_extraction("fn main() {}", "main.rs", &config)?;
```
