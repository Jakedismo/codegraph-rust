## Testing Strategy

### Unit Testing
- Framework: Rust built-in tests with `tokio-test`
- Mock Strategy: Prefer trait-based abstraction and local stubs

### Integration Testing
- Scope: Router-level API validation for core endpoints (health, GraphQL, HTTP/2, parsing)
- Tools: `axum-test` with in-process router, temporary files where needed

### E2E Testing
- Framework: k6 for HTTP-level smoke, ramp, and simple soak
- Key Workflows: health, GraphQL basic query, HTTP/2 health/config/tune

### Performance Testing
- Tools: Criterion benches (existing), benchmark comparison scripts
- Metrics: p95/p99 latency in benches, threshold-based regression detection

## How To Run
- Integration: `make e2e`
- Load (requires running API): `make load-test BASE_URL=http://localhost:3000`
- Deployment Validation: `make deploy-validate BASE_URL=http://host:port`
- Performance Regression: `make perf-regression BASELINE_NAME=baseline THRESHOLD=0.10`

