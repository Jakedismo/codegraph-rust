// Inline modules to avoid path issues
mod types {
    use chrono::{DateTime, Utc};
    use serde::{Deserialize, Serialize};
    use serde_json::Value;
    use std::collections::HashMap;
    use uuid::Uuid;

    pub type AgentId = Uuid;
    pub type TaskId = Uuid;

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
    pub enum AgentStatus { Online, Degraded, Offline, Blocked }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct Capability {
        pub name: String,
        pub version: Option<String>,
        #[serde(default)] pub score: Option<f32>,
        #[serde(default)] pub attributes: HashMap<String, Value>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct AgentHealth {
        pub status: AgentStatus,
        pub avg_latency_ms: f64,
        pub success_count: u64,
        pub failure_count: u64,
        pub consecutive_failures: u32,
        pub circuit_state: CircuitState,
        pub updated_at: DateTime<Utc>,
    }

    #[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
    pub enum CircuitState { Closed, Open, HalfOpen }

    impl Default for AgentHealth {
        fn default() -> Self {
            Self { status: AgentStatus::Online, avg_latency_ms: 0.0, success_count: 0, failure_count: 0, consecutive_failures: 0, circuit_state: CircuitState::Closed, updated_at: Utc::now() }
        }
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct AgentInfo {
        pub id: AgentId,
        pub name: String,
        pub endpoint: Option<String>,
        pub capabilities: Vec<Capability>,
        pub capacity: u32,
        #[serde(default)] pub tags: Vec<String>,
        pub registered_at: DateTime<Utc>,
        pub last_seen: DateTime<Utc>,
        #[serde(default)] pub health: AgentHealth,
        #[serde(default)] pub in_flight: u32,
    }

    impl AgentInfo {
        pub fn supports_all(&self, caps: &[String]) -> bool {
            if caps.is_empty() { return true; }
            let have: std::collections::HashSet<_> = self.capabilities.iter().map(|c| c.name.as_str()).collect();
            caps.iter().all(|c| have.contains(c.as_str()))
        }
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub enum TaskPriority { Low, Normal, High, Critical }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub enum AggregationStrategy {
        FirstSuccess,
        MajorityVote,
        WeightedConfidence,
        MergeJson { policy: JsonConflictPolicy },
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub enum JsonConflictPolicy { PreferFirst, PreferLast, PreferHighestConfidence, RaiseConflict, MergeArraysUnique }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct TaskSpec {
        pub id: TaskId,
        pub kind: String,
        pub payload: Value,
        #[serde(default)] pub required_capabilities: Vec<String>,
        pub priority: TaskPriority,
        #[serde(default)] pub soft_affinity_tags: Vec<String>,
        #[serde(default)] pub hard_affinity_agents: Vec<AgentId>,
        #[serde(default)] pub timeout_ms: Option<u64>,
        #[serde(default = "TaskSpec::default_max_retries")] pub max_retries: u32,
        #[serde(default = "TaskSpec::default_replicas")] pub requested_replicas: u32,
        #[serde(default = "TaskSpec::default_aggregation")] pub aggregation_strategy: AggregationStrategy,
    }
    impl TaskSpec {
        fn default_replicas() -> u32 { 1 }
        fn default_max_retries() -> u32 { 2 }
        fn default_aggregation() -> AggregationStrategy { AggregationStrategy::FirstSuccess }
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct TaskAssignment {
        pub task_id: TaskId,
        pub agent_id: AgentId,
        pub attempt: u32,
        pub assigned_at: DateTime<Utc>,
        pub deadline_at: Option<DateTime<Utc>>,
    }
    impl TaskAssignment { pub fn new(task_id: TaskId, agent_id: AgentId, attempt: u32) -> Self { Self { task_id, agent_id, attempt, assigned_at: Utc::now(), deadline_at: None } } }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct TaskResult {
        pub task_id: TaskId,
        pub agent_id: AgentId,
        pub success: bool,
        #[serde(default)] pub result: Option<serde_json::Value>,
        #[serde(default)] pub confidence: Option<f32>,
        #[serde(default)] pub errors: Option<Vec<String>>,
        #[serde(default)] pub duration_ms: Option<u64>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct AggregatedResult {
        pub task_id: TaskId,
        pub success: bool,
        pub result: Option<serde_json::Value>,
        #[serde(default)] pub details: HashMap<String, serde_json::Value>,
    }
}

mod conflict {
    use serde::{Deserialize, Serialize};
    use serde_json::Value;

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub enum ConflictResolutionStrategy { LastWriteWins, FirstWriteWins, ThreeWayMerge, Custom(String) }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct ConflictContext { pub base: Option<Value>, pub a: Value, pub b: Value }

    pub fn resolve_value(ctx: ConflictContext, strategy: &ConflictResolutionStrategy) -> crate::Result<Value> {
        match strategy {
            ConflictResolutionStrategy::LastWriteWins => Ok(ctx.b),
            ConflictResolutionStrategy::FirstWriteWins => Ok(ctx.a),
            ConflictResolutionStrategy::ThreeWayMerge => Ok(three_way_merge(ctx.base, ctx.a, ctx.b)),
            ConflictResolutionStrategy::Custom(_name) => Ok(ctx.b),
        }
    }

    fn three_way_merge(base: Option<Value>, a: Value, b: Value) -> Value {
        match (base, a, b) {
            (Some(Value::Object(mut base)), Value::Object(a), Value::Object(b)) => {
                for (k, bv) in base.clone() {
                    let av = a.get(&k).cloned();
                    let bv2 = b.get(&k).cloned();
                    if let (Some(av), Some(bv2)) = (av, bv2) {
                        base.insert(k.clone(), three_way_merge(Some(bv), av, bv2));
                    }
                }
                for (k, v) in a.iter() { if !base.contains_key(k) { base.insert(k.clone(), v.clone()); } }
                for (k, v) in b.iter() { if !base.contains_key(k) { base.insert(k.clone(), v.clone()); } }
                Value::Object(base)
            }
            (_, _, b) => b,
        }
    }
}

mod registry {
    use super::types::*;
    use crate::Result;
    use chrono::Utc;
    use dashmap::DashMap;
    use std::sync::Arc;
    use tokio::sync::RwLock;

    #[derive(Default)]
    pub struct AgentRegistry { pub(super) agents: DashMap<AgentId, Arc<RwLock<AgentInfo>>> }

    impl AgentRegistry {
        pub fn new() -> Self { Self { agents: DashMap::new() } }
        pub async fn register(&self, mut info: AgentInfo) -> Result<AgentId> {
            if info.id == uuid::Uuid::nil() { info.id = uuid::Uuid::new_v4(); }
            info.registered_at = Utc::now(); info.last_seen = Utc::now();
            self.agents.insert(info.id, Arc::new(RwLock::new(info))); Ok(info.id)
        }
        pub async fn deregister(&self, id: AgentId) -> Result<()> { self.agents.remove(&id); Ok(()) }
        pub async fn get(&self, id: AgentId) -> Option<Arc<RwLock<AgentInfo>>> { self.agents.get(&id).map(|e| e.value().clone()) }
        pub async fn touch(&self, id: AgentId) { if let Some(entry) = self.agents.get(&id) { let mut info = entry.value().write().await; info.last_seen = Utc::now(); info.health.updated_at = Utc::now(); } }
        pub async fn update_capabilities(&self, id: AgentId, caps: Vec<Capability>) -> Result<()> { if let Some(entry) = self.agents.get(&id) { let mut info = entry.value().write().await; info.capabilities = caps; info.last_seen = Utc::now(); } Ok(()) }
        pub async fn set_capacity(&self, id: AgentId, capacity: u32) -> Result<()> { if let Some(entry) = self.agents.get(&id) { let mut info = entry.value().write().await; info.capacity = capacity; } Ok(()) }
        pub async fn record_assignment(&self, assignment: &TaskAssignment) -> Result<()> { if let Some(entry) = self.agents.get(&assignment.agent_id) { let mut info = entry.value().write().await; info.in_flight = info.in_flight.saturating_add(1); } Ok(()) }
        pub async fn record_result(&self, assignment: &TaskAssignment, result: &TaskResult) -> Result<()> {
            if let Some(entry) = self.agents.get(&assignment.agent_id) {
                let mut info = entry.value().write().await; info.in_flight = info.in_flight.saturating_sub(1);
                let latency = result.duration_ms.unwrap_or(0) as f64;
                info.health.success_count += if result.success { 1 } else { 0 };
                info.health.failure_count += if result.success { 0 } else { 1 };
                if result.success {
                    info.health.consecutive_failures = 0;
                    if info.health.avg_latency_ms == 0.0 { info.health.avg_latency_ms = latency; } else { info.health.avg_latency_ms = (info.health.avg_latency_ms * 0.8) + (latency * 0.2); }
                    if info.health.circuit_state == CircuitState::HalfOpen { info.health.circuit_state = CircuitState::Closed; }
                    info.health.status = AgentStatus::Online;
                } else { self.record_failure(assignment.agent_id).await?; }
                info.last_seen = Utc::now();
            }
            Ok(())
        }
        pub async fn record_failure(&self, id: AgentId) -> Result<()> { if let Some(entry) = self.agents.get(&id) { let mut info = entry.value().write().await; info.health.consecutive_failures = info.health.consecutive_failures.saturating_add(1); info.health.failure_count += 1; if info.health.consecutive_failures >= 3 { info.health.circuit_state = CircuitState::Open; info.health.status = AgentStatus::Degraded; } info.last_seen = Utc::now(); } Ok(()) }
        pub async fn half_open(&self, id: AgentId) { if let Some(entry) = self.agents.get(&id) { let mut info = entry.value().write().await; if info.health.circuit_state == CircuitState::Open { info.health.circuit_state = CircuitState::HalfOpen; } } }
        pub async fn healthy_candidates(&self) -> Vec<AgentInfo> {
            self.agents.iter().filter_map(|e| { let info = futures::executor::block_on(async { e.value().read().await.clone() }); match (info.health.status, info.health.circuit_state) { (AgentStatus::Online | AgentStatus::Degraded, CircuitState::Closed | CircuitState::HalfOpen) => Some(info), _ => None } }).collect()
        }
        pub async fn find_candidates(&self, spec: &TaskSpec) -> Result<Vec<AgentInfo>> {
            let mut list = self.healthy_candidates().await; list.retain(|a| a.supports_all(&spec.required_capabilities)); if !spec.hard_affinity_agents.is_empty() { let set: std::collections::HashSet<_> = spec.hard_affinity_agents.iter().copied().collect(); list.retain(|a| set.contains(&a.id)); } Ok(list)
        }
    }
}

mod scheduler {
    use super::registry::AgentRegistry;
    use super::types::*;
    use async_trait::async_trait;

    #[async_trait]
    pub trait Scheduler {
        async fn select_agents(&self, task: &TaskSpec, registry: &AgentRegistry, desired: usize) -> Vec<AgentId>;
    }
    pub struct RoundRobinScheduler { counters: dashmap::DashMap<String, usize> }
    impl RoundRobinScheduler { pub fn new() -> Self { Self { counters: dashmap::DashMap::new() } } }
    #[async_trait]
    impl Scheduler for RoundRobinScheduler {
        async fn select_agents(&self, task: &TaskSpec, registry: &AgentRegistry, desired: usize) -> Vec<AgentId> {
            let key = task.required_capabilities.join("+");
            let mut candidates = registry.find_candidates(task).await.unwrap_or_default();
            if candidates.is_empty() { return vec![]; }
            candidates.sort_by_key(|a| a.id);
            let idx = self.counters.fetch_add(key.clone(), 1).unwrap_or(0) % candidates.len();
            (0..desired.min(candidates.len())).map(|i| candidates[(idx + i) % candidates.len()].id).collect()
        }
    }
    pub struct LeastLoadedScheduler;
    #[async_trait]
    impl Scheduler for LeastLoadedScheduler {
        async fn select_agents(&self, task: &TaskSpec, registry: &AgentRegistry, desired: usize) -> Vec<AgentId> {
            let mut candidates = registry.find_candidates(task).await.unwrap_or_default();
            candidates.sort_by(|a, b| ((a.in_flight as i32) - (a.capacity as i32)).cmp(&((b.in_flight as i32) - (b.capacity as i32))));
            candidates.into_iter().take(desired).map(|a| a.id).collect()
        }
    }
    pub struct HybridScheduler;
    #[async_trait]
    impl Scheduler for HybridScheduler {
        async fn select_agents(&self, task: &TaskSpec, registry: &AgentRegistry, desired: usize) -> Vec<AgentId> {
            let mut candidates = registry.find_candidates(task).await.unwrap_or_default();
            let mut scored: Vec<(f64, AgentInfo)> = candidates.drain(..).map(|a| {
                let affinity = affinity_score(&a, task);
                let load_penalty = if a.capacity > 0 { (a.in_flight as f64) / (a.capacity as f64) } else { 1.0 };
                let latency_penalty = (a.health.avg_latency_ms / 1000.0).min(1.0);
                let score = affinity - (0.4 * load_penalty) - (0.2 * latency_penalty);
                (score, a)
            }).collect();
            scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());
            scored.into_iter().take(desired).map(|(_, a)| a.id).collect()
        }
    }
    fn affinity_score(agent: &AgentInfo, task: &TaskSpec) -> f64 {
        let mut score = 0.0;
        for cap in &agent.capabilities { if task.required_capabilities.iter().any(|r| r == &cap.name) { score += cap.score.unwrap_or(0.8) as f64; } }
        if !task.soft_affinity_tags.is_empty() {
            let tags: std::collections::HashSet<_> = agent.tags.iter().collect();
            for tag in &task.soft_affinity_tags { if tags.contains(tag) { score += 0.1; } }
        }
        score
    }
    trait CounterExt { fn fetch_add(&self, key: String, by: usize) -> Option<usize>; }
    impl CounterExt for dashmap::DashMap<String, usize> {
        fn fetch_add(&self, key: String, by: usize) -> Option<usize> {
            use dashmap::mapref::entry::Entry;
            match self.entry(key) { Entry::Occupied(mut e) => { let v = e.get_mut(); let old = *v; *v = old.saturating_add(by); Some(old) }, Entry::Vacant(e) => { e.insert(by); Some(0) } }
        }
    }
}

mod aggregator {
    use super::types::*;
    use async_trait::async_trait;
    use serde_json::{json, Value};
    use std::collections::HashMap;

    #[async_trait]
    pub trait Aggregator { async fn aggregate(&self, spec: &TaskSpec, results: Vec<TaskResult>) -> crate::Result<AggregatedResult>; }
    pub struct DefaultAggregator;
    #[async_trait]
    impl Aggregator for DefaultAggregator {
        async fn aggregate(&self, spec: &TaskSpec, results: Vec<TaskResult>) -> crate::Result<AggregatedResult> {
            match spec.aggregation_strategy {
                AggregationStrategy::FirstSuccess => first_success(spec, results),
                AggregationStrategy::MajorityVote => majority_vote(spec, results),
                AggregationStrategy::WeightedConfidence => weighted_confidence(spec, results),
                AggregationStrategy::MergeJson { policy } => merge_json(spec, results, policy),
            }
        }
    }
    fn first_success(spec: &TaskSpec, results: Vec<TaskResult>) -> crate::Result<AggregatedResult> {
        let ok = results.iter().find(|r| r.success && r.result.is_some());
        let result = ok.and_then(|r| r.result.clone());
        Ok(AggregatedResult { task_id: spec.id, success: result.is_some(), result, details: Default::default() })
    }
    fn majority_vote(spec: &TaskSpec, results: Vec<TaskResult>) -> crate::Result<AggregatedResult> {
        let mut counts: HashMap<String, (usize, f32)> = HashMap::new();
        for r in results.into_iter().filter(|r| r.success) {
            if let Some(val) = r.result { let key = if val.is_string() { val.as_str().unwrap().to_string() } else { val.to_string() }; let conf = r.confidence.unwrap_or(1.0); let entry = counts.entry(key).or_insert((0, 0.0)); entry.0 += 1; entry.1 += conf; }
        }
        let best = counts.into_iter().max_by(|a, b| a.1.0.cmp(&b.1.0).then(a.1.1.partial_cmp(&b.1.1).unwrap())).map(|(k, _)| k);
        let result = best.map(|s| serde_json::from_str::<Value>(&format!("\"{}\"", s)).unwrap_or(Value::String(s)));
        Ok(AggregatedResult { task_id: spec.id, success: result.is_some(), result, details: Default::default() })
    }
    fn weighted_confidence(spec: &TaskSpec, results: Vec<TaskResult>) -> crate::Result<AggregatedResult> {
        let mut best: Option<(f32, Value)> = None;
        for r in results.into_iter().filter(|r| r.success) { if let Some(val) = r.result.clone() { let conf = r.confidence.unwrap_or(0.5); match &best { None => best = Some((conf, val)), Some((b, _)) if conf > *b => best = Some((conf, val)), _ => {} } } }
        let result = best.map(|(_, v)| v);
        Ok(AggregatedResult { task_id: spec.id, success: result.is_some(), result, details: Default::default() })
    }
    fn merge_json(spec: &TaskSpec, results: Vec<TaskResult>, policy: JsonConflictPolicy) -> crate::Result<AggregatedResult> {
        let mut acc = Value::Null; let mut success = false;
        for r in results.into_iter().filter(|r| r.success) { if let Some(val) = r.result { acc = json_merge(acc, val, &policy); success = true; } }
        Ok(AggregatedResult { task_id: spec.id, success, result: if success { Some(acc) } else { None }, details: Default::default() })
    }
    fn json_merge(base: Value, incoming: Value, policy: &JsonConflictPolicy) -> Value {
        match (base, incoming) {
            (Value::Object(mut a), Value::Object(b)) => { for (k, v) in b.into_iter() { let nv = match a.remove(&k) { None => v, Some(prev) => json_merge_conflict(prev, v, policy) }; a.insert(k, nv); } Value::Object(a) }
            (Value::Array(mut a), Value::Array(b)) => match policy { JsonConflictPolicy::MergeArraysUnique => { let mut set = std::collections::BTreeSet::new(); for v in a.iter().chain(b.iter()) { set.insert(v.clone()); } Value::Array(set.into_iter().collect()) } _ => Value::Array({ let mut out = a; out.extend(b); out }) },
            (a, b) => json_merge_conflict(a, b, policy),
        }
    }
    fn json_merge_conflict(a: Value, b: Value, policy: &JsonConflictPolicy) -> Value {
        match policy { JsonConflictPolicy::PreferFirst => a, JsonConflictPolicy::PreferLast => b, JsonConflictPolicy::PreferHighestConfidence => b, JsonConflictPolicy::RaiseConflict => serde_json::json!({"conflict": {"a": a, "b": b}}), JsonConflictPolicy::MergeArraysUnique => json_merge(a, b, policy) }
    }
}

pub use types::*;
pub use registry::*;
pub use scheduler::*;
pub use aggregator::*;
pub use conflict::*;

use crate::Result;
use dashmap::DashMap;
use std::{sync::Arc, time::Duration};
use tokio::sync::RwLock;
use tokio::time::timeout;
use tracing::{debug, error, info, warn};

/// Abstraction for sending tasks to agents. Implement this to integrate with your transport.
#[async_trait::async_trait]
pub trait AgentCommunicator: Send + Sync + 'static {
    async fn dispatch(&self, assignment: TaskAssignment, spec: TaskSpec) -> Result<TaskResult>;
}

/// CoordinationEngine orchestrates agent selection, dispatching, aggregation, and fault tolerance.
pub struct CoordinationEngine<C: AgentCommunicator> {
    pub registry: Arc<AgentRegistry>,
    pub communicator: Arc<C>,
    pub scheduler: Arc<dyn Scheduler + Send + Sync>,
    pub aggregator: Arc<dyn Aggregator + Send + Sync>,
    in_flight: Arc<DashMap<TaskId, Arc<RwLock<TaskState>>>>,
}

#[derive(Debug, Default, Clone)]
pub struct CoordinationConfig {
    pub default_timeout: Duration,
    pub max_parallel_assignments: usize,
}

impl Default for CoordinationEngineConfigInternal {
    fn default() -> Self {
        Self {
            default_timeout: Duration::from_secs(30),
            max_parallel_assignments: 4,
        }
    }
}

#[derive(Debug, Clone)]
struct CoordinationEngineConfigInternal {
    default_timeout: Duration,
    max_parallel_assignments: usize,
}

#[derive(Debug, Default)]
struct TaskState {
    pub assignments: Vec<TaskAssignment>,
    pub results: Vec<TaskResult>,
}

impl<C: AgentCommunicator> CoordinationEngine<C> {
    pub fn new(
        registry: Arc<AgentRegistry>,
        communicator: Arc<C>,
        scheduler: Arc<dyn Scheduler + Send + Sync>,
        aggregator: Arc<dyn Aggregator + Send + Sync>,
    ) -> Self {
        Self {
            registry,
            communicator,
            scheduler,
            aggregator,
            in_flight: Arc::new(DashMap::new()),
        }
    }

    /// Submit a task and wait for aggregated result.
    pub async fn submit_and_await(&self, mut spec: TaskSpec) -> Result<AggregatedResult> {
        let replicas = spec.requested_replicas.max(1) as usize;
        let available = self
            .registry
            .find_candidates(&spec)
            .await
            .unwrap_or_default();

        if available.is_empty() {
            return Err(crate::McpError::Scheduling(
                "no healthy agents available for task".into(),
            ));
        }

        let chosen = self
            .scheduler
            .select_agents(&spec, &self.registry, replicas)
            .await;

        if chosen.is_empty() {
            return Err(crate::McpError::Scheduling(
                "scheduler returned no agents".into(),
            ));
        }

        let task_id = spec.id;
        let state = Arc::new(RwLock::new(TaskState::default()));
        self.in_flight.insert(task_id, state.clone());

        let timeout_ms = spec.timeout_ms.unwrap_or(30_000);

        // Spawn dispatches in parallel
        let mut handles = Vec::with_capacity(chosen.len());
        for (i, agent_id) in chosen.into_iter().enumerate() {
            let communicator = Arc::clone(&self.communicator);
            let registry = Arc::clone(&self.registry);
            let state_clone = Arc::clone(&state);
            let spec_clone = spec.clone();

            let handle = tokio::spawn(async move {
                let assignment = TaskAssignment::new(task_id, agent_id, i as u32);
                registry.record_assignment(&assignment).await.ok();
                let result = timeout(
                    Duration::from_millis(timeout_ms as u64),
                    communicator.dispatch(assignment.clone(), spec_clone),
                )
                .await
                .map_err(|_| crate::McpError::RequestTimeout(format!(
                    "task {} timed out on agent {}",
                    assignment.task_id, assignment.agent_id
                )))?;

                match result {
                    Ok(res) => {
                        registry.record_result(&assignment, &res).await.ok();
                        let mut lock = state_clone.write().await;
                        lock.assignments.push(assignment);
                        lock.results.push(res);
                        Ok(())
                    }
                    Err(e) => {
                        registry.record_failure(assignment.agent_id).await.ok();
                        Err(e)
                    }
                }
            });
            handles.push(handle);
        }

        // Wait for either first-success if strategy allows or for all handles
        let aggregated = match spec.aggregation_strategy {
            AggregationStrategy::FirstSuccess => {
                // As soon as one returns success, aggregate
                let mut collected = Vec::new();
                for h in handles {
                    match h.await {
                        Ok(Ok(())) => {
                            let lock = state.read().await;
                            collected = lock.results.clone();
                            break;
                        }
                        Ok(Err(_e)) => continue,
                        Err(_join) => continue,
                    }
                }
                self.aggregator.aggregate(&spec, collected).await
            }
            _ => {
                // Wait all
                for h in handles {
                    let _ = h.await;
                }
                let lock = state.read().await;
                self.aggregator.aggregate(&spec, lock.results.clone()).await
            }
        }?;

        self.in_flight.remove(&task_id);
        Ok(aggregated)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::coordination::aggregator::DefaultAggregator;
    use crate::coordination::registry::AgentRegistry;
    use crate::coordination::scheduler::LeastLoadedScheduler;
    use chrono::Utc;
    use serde_json::json;
    use uuid::Uuid;

    struct MockComm;
    #[async_trait::async_trait]
    impl AgentCommunicator for MockComm {
        async fn dispatch(&self, assignment: TaskAssignment, _spec: TaskSpec) -> Result<TaskResult> {
            Ok(TaskResult {
                task_id: assignment.task_id,
                agent_id: assignment.agent_id,
                success: true,
                result: Some(json!({"ok": true})),
                confidence: Some(0.9),
                errors: None,
                duration_ms: Some(5),
            })
        }
    }

    fn mk_agent(id: AgentId) -> AgentInfo {
        AgentInfo {
            id,
            name: format!("agent-{}", id),
            endpoint: None,
            capabilities: vec![Capability { name: "work".into(), version: None, score: Some(1.0), attributes: Default::default() }],
            capacity: 4,
            tags: vec![],
            registered_at: Utc::now(),
            last_seen: Utc::now(),
            health: AgentHealth::default(),
            in_flight: 0,
        }
    }

    #[tokio::test]
    async fn test_engine_submit_and_await() {
        let registry = Arc::new(AgentRegistry::new());
        registry.register(mk_agent(Uuid::new_v4())).await.unwrap();
        registry.register(mk_agent(Uuid::new_v4())).await.unwrap();

        let engine = CoordinationEngine::new(
            registry,
            Arc::new(MockComm),
            Arc::new(LeastLoadedScheduler),
            Arc::new(DefaultAggregator),
        );

        let spec = TaskSpec {
            id: Uuid::new_v4(),
            kind: "k".into(),
            payload: json!({}),
            required_capabilities: vec!["work".into()],
            priority: TaskPriority::Normal,
            soft_affinity_tags: vec![],
            hard_affinity_agents: vec![],
            timeout_ms: Some(1000),
            max_retries: 0,
            requested_replicas: 2,
            aggregation_strategy: AggregationStrategy::FirstSuccess,
        };

        let out = engine.submit_and_await(spec).await.unwrap();
        assert!(out.success);
        assert!(out.result.is_some());
    }
}
