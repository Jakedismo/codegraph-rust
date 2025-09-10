## Testing Context Summary

- Project Type: Rust monorepo (workspace) with API server and libraries
- Technology Stack: Rust 1.75+, Tokio, Axum, async-graphql, RocksDB, Tree-sitter, optional FAISS, Criterion
- Key Requirements: Graph-based code analysis, parsing endpoints, GraphQL API, streaming endpoints, vector search, HTTP/2 optimization endpoints, observability via Prometheus metrics
- Testing Constraints: Optional FAISS-backed vector store (feature `faiss`), some endpoints rely on semantic search; prefer fixture-free tests or feature-agnostic endpoints; use `axum-test` for router-level integration tests; load tests via k6 assume a running API

