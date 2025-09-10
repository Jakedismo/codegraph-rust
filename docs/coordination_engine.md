## Development Context

- Feature: Multi-agent Coordination Engine for MCP (agent registry, scheduling, aggregation, conflict resolution) with fault tolerance.
- Technical Stack: Rust (tokio, dashmap, parking_lot, serde, tracing), integrated in `codegraph-mcp` crate.
- Constraints: Async-first, thread-safe (DashMap + RwLock), minimal coupling to transport (via trait), stable public types with serde derives, compile without enabling external crates.
- Success Criteria: 
  - Agent registry managing active agents and capabilities
  - Schedulers for load balancing (least-loaded, round-robin, hybrid)
  - Result aggregation policies (first-success, majority, weighted, JSON-merge)
  - Conflict resolution strategies (LWW, FWW, 3-way)
  - Fault tolerance (timeouts, retries, circuit breaking) hooks

## Architecture Alignment

- Pattern: Coordinator + Registry + Strategy interfaces
- Components:
  - `AgentRegistry`: lifecycle and health tracking, capability filtering
  - `Scheduler` (trait): RoundRobin, LeastLoaded, Hybrid
  - `Aggregator` (trait): Default aggregator with 4 strategies
  - `CoordinationEngine`: orchestrates dispatch → collect → aggregate
  - `AgentCommunicator` (trait): abstract transport for MCP
- Interfaces: serde-friendly types (`AgentInfo`, `TaskSpec`, `TaskResult`, `AggregatedResult`)
- Constraints: Non-blocking; uses `tokio::spawn` and timeouts; circuit breaker modeled in `AgentHealth`

## Usage

```rust
use codegraph_mcp::coordination::{
  AgentRegistry, CoordinationEngine, LeastLoadedScheduler, DefaultAggregator,
  AgentCommunicator, AgentInfo, Capability, TaskSpec, TaskPriority
};
use std::sync::Arc;
use uuid::Uuid;
use serde_json::json;

struct MyComm; // implement your MCP transport here
#[async_trait::async_trait]
impl AgentCommunicator for MyComm {
  async fn dispatch(&self, assignment: TaskAssignment, spec: TaskSpec) -> codegraph_mcp::Result<TaskResult> {
    // send over MCP, await response; map to TaskResult
    unimplemented!()
  }
}

async fn run() -> codegraph_mcp::Result<()> {
  let registry = Arc::new(AgentRegistry::new());
  // Register agents
  registry.register(AgentInfo {
    id: Uuid::new_v4(),
    name: "agent-a".into(),
    endpoint: None,
    capabilities: vec![Capability { name: "work".into(), version: None, score: Some(1.0), attributes: Default::default() }],
    capacity: 4,
    tags: vec![],
    registered_at: chrono::Utc::now(),
    last_seen: chrono::Utc::now(),
    health: Default::default(),
    in_flight: 0,
  }).await?;

  let engine = CoordinationEngine::new(
    registry,
    Arc::new(MyComm),
    Arc::new(LeastLoadedScheduler),
    Arc::new(DefaultAggregator),
  );

  let spec = TaskSpec {
    id: Uuid::new_v4(),
    kind: "example".into(),
    payload: json!({"text": "hello"}),
    required_capabilities: vec!["work".into()],
    priority: TaskPriority::Normal,
    soft_affinity_tags: vec![],
    hard_affinity_agents: vec![],
    timeout_ms: Some(5_000),
    max_retries: 2,
    requested_replicas: 2,
    aggregation_strategy: AggregationStrategy::FirstSuccess,
  };

  let aggregated = engine.submit_and_await(spec).await?;
  println!("aggregated: {:?}", aggregated);
  Ok(())
}
```

## Fault Tolerance Hooks

- Timeouts: per-dispatch `tokio::time::timeout` with configurable `timeout_ms`
- Circuit Breaker: opens on ≥3 consecutive failures; half-open logic supported via `registry.half_open`
- Retries/Reassignment: implement externally by resubmitting with new `requested_replicas` and a different scheduler or filtered candidates

## Testing

- Unit tests included for registry, schedulers, aggregator, and engine happy-path
- To run only mcp crate tests: `cargo test -p codegraph-mcp`

