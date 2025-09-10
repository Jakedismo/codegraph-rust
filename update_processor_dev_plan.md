## Phase 3: Architecture and Design

With the research complete, this phase focuses on the high-level architecture and design of the update processor.

### Core Components

1.  **`FileWatcher`**: A component that uses the `notify` crate to listen for file system events. It will filter and batch these events before sending them to the `UpdateScheduler`.

2.  **`UpdateScheduler`**: This component will receive file change events and schedule the processing work. It will be responsible for:
    -   Prioritizing updates.
    -   Batching changes to the same file.
    -   Distributing work to the `DeltaProcessor`.

3.  **`DeltaProcessor`**: This component will run in a `rayon` thread pool. It will take a file path and its new content, compute the delta from the version stored in the graph, and identify the changed parts of the file.

4.  **`GraphUpdater`**: This component will receive the computed deltas and apply them to the `codegraph-graph`. It will be responsible for:
    -   Updating the content of the changed nodes.
    -   Traversing the dependency graph to find and invalidate affected nodes.
    -   Emitting events for other parts of the system to consume (e.g., for re-indexing in `codegraph-vector`).

5.  **`ProgressTracker`**: A component to track the progress of updates in real-time. It will expose metrics such as:
    -   Number of pending updates.
    -   Processing time per file.
    -   Graph update latency.

### Data Flow

```
+-------------+
| FileWatcher |
+-------------+
      |
      v
+-----------------+
| UpdateScheduler |
+-----------------+
      |
      v
+----------------+
| DeltaProcessor |
+----------------+
      |
      v
+--------------+
| GraphUpdater |
+--------------+
      |
      v
+-----------------+
| ProgressTracker |
+-----------------+
```

### Error Handling

-   Errors in each stage of the pipeline will be propagated and logged.
-   A mechanism for retrying failed updates will be implemented.
-   The system will be designed to be resilient to failures in individual components.

## Phase 2: Best Practices Research

This phase focuses on identifying and applying current best practices for building a high-performance, concurrent system in Rust.

-   **`rocksdb` Performance**: Research will focus on:
    -   **Column Families**: Using column families to separate different types of data (e.g., nodes, edges, metadata) to optimize access patterns.
    -   **Write-Ahead Log (WAL)**: Configuring the WAL for optimal write performance and durability trade-offs.
    -   **Caching**: Utilizing RocksDB's built-in block cache to keep frequently accessed data in memory.

-   **`tokio` and `rayon` Integration**:
    -   **`spawn_blocking`**: For CPU-bound work originating from an async context (like parsing or diffing), `tokio::task::spawn_blocking` will be used to move the work to a blocking thread pool, preventing it from starving the async runtime.
    -   **Bridging Async and Sync**: We will use channels to communicate between the async `tokio` tasks and the sync `rayon` thread pools.

-   **Dependency Graph Updates**:
    -   **Incremental Updates**: Instead of re-calculating the entire graph, we will implement a strategy to only update the affected nodes and their dependencies.
    -   **Lock-Free Data Structures**: Where possible, we will use lock-free data structures to avoid contention and improve parallelism.

-   **Inspiration from Existing Systems**: We will draw inspiration from existing build systems like `bazel` and `buck`, and language servers that have solved similar problems of incremental computation and dependency tracking.

## Phase 1: Library Research & Planning

Based on the dependencies in `Cargo.toml`, the following libraries are key for implementing the update processor. This section outlines their roles and how they will be used.

-   **`tokio`**: As the core async runtime, `tokio` will be used to manage the parallel processing of file updates. We will use `tokio::spawn` to create lightweight asynchronous tasks for each file, allowing for high concurrency.

-   **`rocksdb`**: This will be used for the persistent graph storage. We will leverage its performance characteristics for fast reads and writes. The graph structure will be designed to allow for efficient querying of dependencies.

-   **`rayon`**: For CPU-bound tasks like delta computation (diffing) and dependency analysis, `rayon` will be used to parallelize the work across multiple CPU cores. This will be crucial for achieving the <1s performance target.

-   **`crossbeam-channel`**: To coordinate between the different stages of the pipeline (e.g., file watching, delta computation, graph update), we will use `crossbeam` channels for fast and efficient message passing.

-   **`dashmap`**: For in-memory caching of frequently accessed graph nodes or dependencies, `dashmap` will provide a thread-safe concurrent hash map. This will reduce the need to query RocksDB for every operation.

-   **`parking_lot`**: Where fine-grained locking is required for shared data structures, `parking_lot`'s `Mutex` and `RwLock` will be used for their performance benefits over the standard library's equivalents.

-   **`notify`**: To detect changes in the file system, the `notify` crate will be used to watch for file creation, modification, and deletion events. This will be the entry point to the update processing pipeline.

-   **`similar`**: For computing the delta between file versions, the `similar` crate will be used to generate a compact and efficient diff. This will be more efficient than re-parsing the entire file.

## Development Context

- **Feature**: An efficient change processing pipeline for large codebases.
- **Core Deliverables**:
    1.  **Delta Computation**: Calculate incremental updates to avoid full reprocessing.
    2.  **Dependency Invalidation**: Accurately invalidate dependent nodes in a large-scale code graph.
    3.  **Parallel Processing**: Process updates for multiple files concurrently.
    4.  **Progress Tracking**: Provide real-time metrics on the update process.
- **Technical Stack**:
    -   **Language**: Rust
    -   **Core Libraries**: `tokio` for asynchronous operations, `rocksdb` for persistent graph storage, `dashmap` and `parking_lot` for concurrent data structures.
    -   **Relevant Crates**: The implementation will likely reside primarily in `codegraph-graph` for graph manipulation and `codegraph-core` for shared types. It will be used by `codegraph-api` to expose the functionality.
- **Constraints**:
    -   Must handle repositories with over 100,000 files.
    -   The system is built on a Rust workspace architecture.
- **Success Criteria**:
    -   **Performance**: Update propagation time must be less than 1 second for complex changes in large repositories.
    -   **Accuracy**: Dependency invalidation must be precise to ensure graph consistency.
    -   **Scalability**: The solution must scale horizontally with the number of files and changes.
