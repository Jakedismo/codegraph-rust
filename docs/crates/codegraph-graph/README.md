# CodeGraph Graph (`codegraph-graph`)

## Overview
`codegraph-graph` serves as the Data Access Layer (DAL) for the system, specifically targeting SurrealDB as the persistent store.

## Key Components

### `SurrealDbStorage`
The main client struct for database interactions.
- **Async Operations**: Optimized for high-throughput, concurrent writes.
- **Graph Operations**: Methods to insert nodes, create edges, and query the graph.

### Schema & Models
Defines the storage-optimized versions of core types.
- **`CodeEdge`**: Represents a fully resolved edge in the database.
- **`NodeEmbeddingRecord`**: storage for vector embeddings.

## Migration
Includes utilities for applying SurrealDB schema files (`.surql`) to ensure the database structure matches the code expectations.
