# CodeGraph Concurrent (`codegraph-concurrent`)

## Overview
Provides concurrency primitives and structures to ensure safe, high-performance parallel processing.

## Features
- **Graph Primitives**: Thread-safe graph structures for in-memory operations.
- **Queues**: MPMC (Multi-Producer Multi-Consumer) and SPSC (Single-Producer Single-Consumer) channel implementations optimized for the indexing workload.
- **Usage**: heavily used by `codegraph-mcp` during the parallel indexing phase.
