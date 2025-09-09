# CodeGraph Project - Claude Code Instructions

## Project Overview

CodeGraph is a sophisticated code analysis and embedding system built in Rust. It provides graph-based code representation, vector search capabilities, and a comprehensive API for code understanding and analysis. The project uses a workspace structure with multiple specialized crates.

## Workspace Structure

```
crates/
├── codegraph-core/     # Core types, traits, and shared functionality
├── codegraph-graph/    # Graph data structures and RocksDB storage
├── codegraph-parser/   # Tree-sitter based code parsing
├── codegraph-vector/   # Vector embeddings and FAISS search
└── codegraph-api/      # REST API server using Axum
```

## Development Workflow

### Build Commands
- `cargo build` - Build all crates
- `cargo check` - Quick compilation check
- `cargo test` - Run all tests
- `make dev` - Full development check (format, lint, test)
- `make quick` - Quick check (format, lint only)

### Quality Tools
- **Formatting**: Run `cargo fmt` or `make fmt`
- **Linting**: Run `cargo clippy` or `make lint`
- **Testing**: Run `cargo test` or `make test`

### Watch Mode
- `cargo watch -c -x check` - Watch for changes and check
- `cargo watch -c -x test` - Watch for changes and test
- `make watch` - Development watch mode

## Crate-Specific Guidelines

### codegraph-core
- Contains shared types, traits, and error handling
- All other crates depend on this
- Keep interfaces stable and well-documented
- Use `#[derive(Debug, Clone, Serialize, Deserialize)]` for data types

### codegraph-graph
- RocksDB-based graph storage
- Thread-safe operations using DashMap and parking_lot
- Implement proper error handling for database operations
- Use async/await for I/O operations

### codegraph-parser
- Tree-sitter based parsing for multiple languages
- Support for Rust, Python, JavaScript, TypeScript, Go
- Visitor pattern for AST traversal
- Language-agnostic node representation

### codegraph-vector
- FAISS integration for vector similarity search
- Optional FAISS support via features
- Efficient embedding storage and retrieval
- Thread-safe vector operations

### codegraph-api
- Axum-based REST API server
- Structured error responses
- Request/response validation
- Comprehensive logging with tracing

## Code Standards

### Error Handling
- Use `thiserror` for custom error types
- Propagate errors with `?` operator
- Provide context with `anyhow` when appropriate
- Log errors at appropriate levels

### Async/Await
- Use `tokio` for async runtime
- Prefer `async-trait` for trait definitions
- Use `Arc<T>` for shared state in async contexts
- Avoid blocking operations in async functions

### Testing
- Unit tests in each crate
- Integration tests in `tests/` directories  
- Use `tokio-test` for async tests
- Mock external dependencies

### Documentation
- Document all public APIs with `///` comments
- Include examples in documentation
- Keep README files up to date
- Use `cargo doc` to generate documentation

## Dependencies Management

### Key Dependencies
- **tokio**: Async runtime with full features
- **serde**: Serialization with derive feature
- **tracing**: Structured logging
- **anyhow/thiserror**: Error handling
- **rocksdb**: Graph persistence
- **tree-sitter**: Code parsing
- **axum**: Web framework
- **faiss**: Vector search (optional)

### Adding Dependencies
1. Add to workspace `Cargo.toml` under `[workspace.dependencies]`
2. Reference in crate `Cargo.toml` with `{ workspace = true }`
3. Update version numbers in workspace file only
4. Document breaking changes in CHANGELOG.md

## Build Configuration

### Profiles
- **dev**: Fast compilation, debug symbols
- **release**: Optimized builds with LTO
- **bench**: Maximum optimization for benchmarks
- **test**: Balanced optimization for tests

### Features
- Default features are minimal for fast builds
- Optional features for heavy dependencies (e.g., FAISS)
- Use `--all-features` for complete functionality testing

## API Development

### Request/Response Patterns
- Use structured JSON for all endpoints
- Validate input with serde and custom validators
- Return consistent error formats
- Include request IDs for tracing

### State Management
- Share state via `Arc<AppState>`
- Use `tokio::sync::RwLock` for read-heavy data
- Use `parking_lot::Mutex` for short-lived locks
- Avoid global state where possible

## Performance Considerations

### Graph Operations
- Use batch operations for bulk inserts
- Implement connection pooling for RocksDB
- Cache frequently accessed data
- Use appropriate RocksDB column families

### Vector Operations
- Batch vector operations when possible
- Use appropriate FAISS index types
- Consider memory vs. accuracy trade-offs
- Monitor index build performance

### API Performance  
- Use connection pooling
- Implement request throttling
- Add appropriate caching headers
- Monitor response times with metrics

## Security Guidelines

### Input Validation
- Validate all API inputs
- Sanitize file paths and queries
- Use type-safe deserialization
- Implement rate limiting

### Error Information
- Don't expose internal errors to clients
- Log security-relevant events
- Use structured error codes
- Implement audit logging

## Environment Configuration

### Development
- Use `.env` files for local configuration
- Set `RUST_LOG=debug` for verbose logging
- Use `cargo watch` for development workflow
- Enable all clippy warnings

### Production
- Use environment variables for configuration
- Set appropriate log levels
- Enable security features
- Use release builds

## Docker Deployment

### Build
- Multi-stage builds for smaller images
- Install only runtime dependencies
- Use non-root user for security
- Implement proper health checks

### Configuration
- Use bind mounts for data persistence
- Configure resource limits
- Set up proper networking
- Use secrets for sensitive data

## Troubleshooting

### Build Issues
- Check Rust version compatibility
- Verify system dependencies (clang, cmake)
- Clear `target/` directory if needed
- Check Cargo.lock for version conflicts

### Runtime Issues
- Check database permissions and paths
- Verify network connectivity
- Monitor memory usage for large graphs
- Check log files for error details

### Performance Issues
- Profile with `cargo flamegraph`
- Monitor database performance
- Check vector index efficiency
- Use async profiling tools

## Git Workflow

### Branches
- `main`: Stable production code
- `develop`: Integration branch
- Feature branches: `feature/description`
- Hotfix branches: `hotfix/issue`

### Commits
- Use conventional commit messages
- Include issue references
- Keep changes atomic
- Write descriptive commit messages

### Pull Requests
- Run full CI checks locally first
- Include tests for new features
- Update documentation as needed
- Request appropriate reviews

## CI/CD Pipeline

### Checks
- Format verification with `cargo fmt --check`
- Linting with `cargo clippy`
- All tests pass across Rust versions
- Security audit with `cargo audit`

### Deployment
- Automated testing in multiple environments
- Docker image building and scanning
- Performance regression testing
- Automated documentation updates