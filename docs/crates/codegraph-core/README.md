# CodeGraph Core (`codegraph-core`)

## Overview
`codegraph-core` is the foundational crate for the CodeGraph system. It defines the core data models, types, and traits that are shared across the entire ecosystem. It allows for a decoupled architecture where parsing, storage, and indexing can evolve independently.

## Key Components

### `CodeNode`
The `CodeNode` struct is the atomic unit of the graph. It represents a file, a class, a function, or any other significant code entity.
- **Deterministic IDs**: `CodeNode` uses a content-addressable or path-based deterministic ID system (`with_deterministic_id`) to ensure that re-indexing the same content yields the same ID, facilitating incremental updates.

### `ExtractionResult`
This struct is the bridge between parsing and indexing.
- **Decoupled Design**: It holds a list of `CodeNode`s and a list of `EdgeRelationship`s.
- **Atomic Unit**: The parser produces a single `ExtractionResult` for a file, which contains everything needed to index that file.

### `EdgeRelationship`
Represents a directed connection between nodes.
- **Unresolved Edges**: Initially, edges might point to a string target (e.g., a function name) rather than a concrete Node ID. The `indexer` resolves these later.
- **Types**: Defines relationship types like `Defines`, `Calls`, `Imports`.

## Connascence & Architecture
This crate has **high afferent coupling** (many crates depend on it) but **low efferent coupling** (it depends on few things). This is by design. Changes here ripple through the system, so strict backward compatibility and careful design of `CodeNode` and `ExtractionResult` are required.
