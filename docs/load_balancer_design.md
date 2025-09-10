**Architecture Understanding**
- Components: `codegraph-lb` library integrated by API or gateway
- Integration points: Axum/Tower middlewares, Prometheus metrics, Governor rate limiter
- Data flows: Request selection → upstream execution → metrics/health update
- Deployment: Package into `codegraph-api` or a dedicated proxy service

**Deployment Strategy Decision**
- Selected: Rolling updates for stateless services; Canary via weighted endpoints
- Rationale:
  - Alignment: Stateless selection and health checks suit rolling
  - Risk: Canary via weights enables gradual rollout
  - Rollback: Set weight back to 0 for new canary, immediate fallback

**Failover Strategy**
- Mark endpoint unhealthy after threshold, route to next healthy
- Passive failures increment counters; recovery requires consecutive successes

**Monitoring**
- Metrics: `lb_requests_total`, `lb_failures_total`, `lb_active_connections`, `lb_upstream_latency_seconds`
- Alerts: Elevated failure rate, no healthy endpoints, high p95 latency

**Traffic Shaping**
- Per-route rate limits via `TrafficShaper` with prefix/method matching

**Next Steps**
- Optional: Implement Tower `Service` proxy Layer for full reverse-proxy
- Optional: Add EWMA-based adaptive weights, circuit breakers with open/half-open states

