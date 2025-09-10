**Load Balancing Overview**
- Algorithms: Round Robin, Smooth Weighted RR, Least Connections, Power-of-Two Choices (EWMA), HRW Hashing (sticky)
- Health: Active HTTP checks with thresholds, passive failure tracking
- Failover: Next-healthy fallback on failure
- Shaping: Per-route rate limits via `governor`
- Metrics: Prometheus counters and histograms

**Quick Start**
- Add dependency: `codegraph-lb` (already a workspace member)
- Initialize pool and balancer:
  - Create `PoolConfig` and `EndpointPool::from_config`
  - Choose `RoundRobin`/`SmoothWeightedRR`/`LeastConnections`/`PowerOfTwoChoicesEwma`/`HrwHashing`
  - Start active health checks with `start_active_http_checks`

Example (inside an async context):

```rust
use codegraph_lb::{PoolConfig, EndpointConfig, EndpointPool, RoundRobin, start_active_http_checks};
use std::sync::Arc;

let cfg = PoolConfig {
    endpoints: vec![
        EndpointConfig { uri: "http://127.0.0.1:3001".into(), weight: 2, health_check_path: Some("/health".into()) },
        EndpointConfig { uri: "http://127.0.0.1:3002".into(), weight: 1, health_check_path: Some("/health".into()) },
    ],
};
let pool = Arc::new(EndpointPool::from_config(&cfg)?);
let bal = RoundRobin::new();

// Health checks
let eps = pool.endpoints.clone();
tokio::spawn(async move {
    codegraph_lb::start_active_http_checks(eps, Default::default()).await;
});

// Pick target
if let Some(ep) = bal.pick(&pool, None) {
    println!("routing to {}", ep.base_uri);
}
```

**Traffic Shaping**
- Create `TrafficShaper` with `RouteRule { prefix, methods, limit_per_second }`
- Call `allow(&Request)` before executing a route to enforce rate limit.

**Metrics**
- Register via `codegraph_lb::register(&prometheus::default_registry())` to expose LB metrics.

**Notes**
- The crate currently provides primitives and selection logic. A full reverse-proxy `Service` can be layered into axum/tower in a next iteration.

