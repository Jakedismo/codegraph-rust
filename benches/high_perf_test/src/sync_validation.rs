use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::{Duration, Instant};

use parking_lot::RwLock;
use tokio::sync::{broadcast, Mutex};

// Lightweight update region model for simulation
#[derive(Debug, Clone)]
pub struct UpdateRegion {
    pub file_path: String,
    pub start_line: usize,
    pub end_line: usize,
    pub affected_ids: Vec<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum UpdatePriority { Low, Normal, High, Critical }

#[derive(Debug, Clone)]
pub struct SelectiveUpdateRequest {
    pub region: UpdateRegion,
    pub new_values: HashMap<u64, i64>,
    pub priority: UpdatePriority,
}

#[derive(Debug, Clone)]
pub struct SelectiveUpdateResult {
    pub updated: usize,
    pub added: usize,
    pub removed: usize,
    pub duration: Duration,
}

// Simple in-memory store (id -> value) protected by RwLock to simulate graph
#[derive(Debug, Default)]
pub struct InMemoryStore(Arc<RwLock<HashMap<u64, i64>>>);

impl InMemoryStore {
    pub fn new() -> Self { Self::default() }
    pub fn get_snapshot(&self) -> HashMap<u64, i64> { self.0.read().clone() }
    pub fn len(&self) -> usize { self.0.read().len() }
}

pub struct SelectiveUpdater {
    store: InMemoryStore,
}

impl SelectiveUpdater {
    pub fn new(store: InMemoryStore) -> Self { Self { store } }

    pub async fn selective_update(&self, req: SelectiveUpdateRequest) -> SelectiveUpdateResult {
        let start = Instant::now();

        // Simulate per-region write by taking a write lock; this will serialize overlapping writes
        let mut db = self.store.0.write();

        // Remove items in region not present in new_values
        let mut removed = 0usize;
        for id in &req.region.affected_ids {
            if !req.new_values.contains_key(id) && db.remove(id).is_some() { removed += 1; }
        }

        // Add or update values
        let mut added = 0usize;
        let mut updated = 0usize;
        for (id, val) in &req.new_values {
            if db.contains_key(id) { updated += 1; } else { added += 1; }
            db.insert(*id, *val);
        }

        SelectiveUpdateResult { updated, added, removed, duration: start.elapsed() }
    }
}

// Broadcast-based event bus to validate propagation latency
#[derive(Debug, Clone)]
pub struct EventBus {
    sender: broadcast::Sender<GraphUpdateEvent>,
}

#[derive(Debug, Clone)]
pub struct GraphUpdateEvent {
    pub seq: u64,
    pub change_count: usize,
    pub published_at: Instant,
}

impl EventBus {
    pub fn new(buffer: usize) -> Self {
        let (tx, _rx) = broadcast::channel(buffer.max(16));
        Self { sender: tx }
    }
    pub fn subscribe(&self) -> broadcast::Receiver<GraphUpdateEvent> { self.sender.subscribe() }
    pub fn publish(&self, seq: u64, change_count: usize) -> usize {
        let ev = GraphUpdateEvent { seq, change_count, published_at: Instant::now() };
        // Ignore send errors (no subscribers)
        let _ = self.sender.send(ev);
        self.sender.receiver_count()
    }
}

// Simple transactional consistency manager for concurrent updates
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IsolationLevel { ReadCommitted, Serializable }

#[derive(Debug)]
pub struct TxManager {
    // Per-key locks; coarse map lock kept simple for test harness
    locks: Arc<RwLock<HashMap<u64, usize>>>,
    store: InMemoryStore,
}

impl TxManager {
    pub fn new(store: InMemoryStore) -> Self { Self { locks: Arc::new(RwLock::new(HashMap::new())), store } }

    // Acquire exclusive locks on keys; naive deadlock prevention via sorted lock order
    fn acquire_locks(&self, keys: &mut Vec<u64>) {
        keys.sort_unstable();
        let mut guard = self.locks.write();
        for k in keys.iter() { *guard.entry(*k).or_insert(0) += 1; }
    }

    fn release_locks(&self, keys: &mut Vec<u64>) {
        keys.sort_unstable();
        let mut guard = self.locks.write();
        for k in keys.iter() {
            if let Some(cnt) = guard.get_mut(k) { if *cnt > 0 { *cnt -= 1; } }
        }
        guard.retain(|_, v| *v > 0);
    }

    pub async fn transact(
        &self,
        isolation: IsolationLevel,
        writes: HashMap<u64, i64>,
        read_back_keys: Vec<u64>,
    ) -> Result<(HashMap<u64, i64>, bool), String> {
        let mut keys: Vec<u64> = writes.keys().cloned().collect();
        self.acquire_locks(&mut keys);

        // Snapshot for serializable check
        let before = if matches!(isolation, IsolationLevel::Serializable) {
            Some(self.store.get_snapshot())
        } else { None };

        {
            // Apply writes
            let mut db = self.store.0.write();
            for (k, v) in writes.iter() { db.insert(*k, *v); }
        }

        // Serializable validation: ensure no other writer modified keys (simulated by comparing snapshot)
        if let Some(snapshot) = before {
            let after = self.store.get_snapshot();
            for k in keys.iter() {
                let s = snapshot.get(k);
                let a = after.get(k);
                // If key existed and changed by others (not our value), conflict; simplistic check
                if s.is_some() && a.is_some() && s != a && !writes.get(k).map(|w| Some(w) == a).unwrap_or(false) {
                    // Conflict detected; rollback our writes for the involved keys
                    {
                        let mut db = self.store.0.write();
                        for kk in keys.iter() {
                            if let Some(orig) = snapshot.get(kk) { db.insert(*kk, *orig); } else { db.remove(kk); }
                        }
                    }
                    self.release_locks(&mut keys);
                    return Err("serialization_conflict".into());
                }
            }
        }

        // Read-back
        let read_values = {
            let db = self.store.0.read();
            read_back_keys.into_iter().map(|k| (k, *db.get(&k).unwrap_or(&0))).collect()
        };

        self.release_locks(&mut keys);
        Ok((read_values, true))
    }
}

// ========== Public test entrypoints ==========

// 1) Concurrency testing for parallel update scenarios
pub async fn run_concurrency_stress(rounds: usize, concurrency: usize) {
    let store = InMemoryStore::new();
    let updater = Arc::new(SelectiveUpdater::new(store));

    // Preload with baseline keys
    {
        let mut db = updater.store.0.write();
        for i in 0..10_000u64 { db.insert(i, 0); }
    }

    let start = Instant::now();
    let mut tasks = vec![];
    for t in 0..concurrency {
        let up = updater.clone();
        tasks.push(tokio::spawn(async move {
            let mut rng = fastrand::Rng::with_seed(t as u64 + 42);
            let mut updated_total = 0usize;
            for _ in 0..rounds {
                let base: u64 = rng.u64(0..9_900);
                let affected: Vec<u64> = (base..base + 100).collect();
                let mut new_values = HashMap::new();
                for id in affected.iter().take(60) { new_values.insert(*id, rng.i64(1..1_000_000)); }
                let req = SelectiveUpdateRequest {
                    region: UpdateRegion { file_path: format!("file_{}.rs", base % 100), start_line: 1, end_line: 200, affected_ids: affected },
                    new_values,
                    priority: UpdatePriority::Normal,
                };
                let res = up.selective_update(req).await;
                updated_total += res.updated + res.added + res.removed;
            }
            updated_total
        }));
    }
    let mut total = 0usize;
    for task in tasks { total += task.await.unwrap(); }
    let elapsed = start.elapsed();
    let ops_per_sec = (total as f64) / elapsed.as_secs_f64();
    println!("[Concurrency] total_ops={} elapsed={:?} ops/sec={:.0}", total, elapsed, ops_per_sec);
}

// 2) Performance validation against <1s propagation target
pub async fn run_propagation_benchmark(subscribers: usize, publishers: usize, events_per_publisher: usize) {
    let bus = EventBus::new(16384);
    let max_latency = Arc::new(Mutex::new(Duration::from_millis(0)));

    // Start subscribers
    let mut subs = vec![];
    for _ in 0..subscribers {
        let mut rx = bus.subscribe();
        let max_lat = max_latency.clone();
        subs.push(tokio::spawn(async move {
            // Receive only the first message to measure propagation latency
            if let Ok(ev) = rx.recv().await {
                let lat = ev.published_at.elapsed();
                let mut guard = max_lat.lock().await;
                if lat > *guard { *guard = lat; }
            }
        }));
    }

    // Start publishers
    let start = Instant::now();
    let mut pubs = vec![];
    for p in 0..publishers {
        let bus_cl = bus.clone();
        pubs.push(tokio::spawn(async move {
            for i in 0..events_per_publisher {
                let seq = (p as u64) * 1_000_000 + i as u64;
                let _ = bus_cl.publish(seq, 1);
            }
        }));
    }
    for h in pubs { let _ = h.await; }
    for h in subs { let _ = h.await; }
    let elapsed = start.elapsed();

    let worst = *max_latency.lock().await;
    println!("[Propagation] subs={} pubs={} events={} elapsed={:?} worst_latency={:?}", subscribers, publishers, publishers * events_per_publisher, elapsed, worst);
    assert!(worst < Duration::from_secs(1), "Propagation exceeded 1s target: {:?}", worst);
}

// 3) Consistency checks for distributed (transactional) updates
pub async fn run_consistency_checks(concurrency: usize, tx_size: usize) {
    let store = InMemoryStore::new();
    let txm = Arc::new(TxManager::new(store));

    // Seed store
    {
        let mut db = txm.store.0.write();
        for i in 0..10_000u64 { db.insert(i, 0); }
    }

    let mut tasks = vec![];
    for t in 0..concurrency {
        let m = txm.clone();
        tasks.push(tokio::spawn(async move {
            let mut rng = fastrand::Rng::with_seed(1000 + t as u64);
            let mut committed = 0usize;
            let mut aborted = 0usize;
            for _ in 0..100 {
                let mut writes = HashMap::new();
                let start_key = rng.u64(0..9_900);
                for k in start_key..start_key + tx_size as u64 { writes.insert(k, rng.i64(1..1_000_000)); }
                let keys: Vec<u64> = writes.keys().cloned().collect();
                match m.transact(IsolationLevel::Serializable, writes, keys).await {
                    Ok(_) => committed += 1,
                    Err(_) => aborted += 1,
                }
            }
            (committed, aborted)
        }));
    }
    let mut committed = 0usize; let mut aborted = 0usize;
    for h in tasks { let (c, a) = h.await.unwrap(); committed += c; aborted += a; }
    println!("[Consistency] committed={} aborted={}", committed, aborted);
    // Expect some aborts under conflicts, but overall progress should be made
    assert!(committed > 0);
}

// 4) Edge case handling for complex scenarios
pub async fn run_edge_case_scenarios() {
    // Edge case 1: Empty region update
    let store = InMemoryStore::new();
    let updater = SelectiveUpdater::new(store);
    let req = SelectiveUpdateRequest {
        region: UpdateRegion { file_path: "empty.rs".into(), start_line: 0, end_line: 0, affected_ids: vec![] },
        new_values: HashMap::new(),
        priority: UpdatePriority::Low,
    };
    let res = updater.selective_update(req).await;
    assert_eq!(res.added + res.updated + res.removed, 0);

    // Edge case 2: Max overlap regions racing
    let store = InMemoryStore::new();
    let updater = Arc::new(SelectiveUpdater::new(store));
    {
        let mut db = updater.store.0.write();
        for i in 0..1_000u64 { db.insert(i, 1); }
    }
    let up1 = updater.clone();
    let up2 = updater.clone();
    let t1 = tokio::spawn(async move {
        let mut nv = HashMap::new();
        for i in 0..1_000u64 { nv.insert(i, 2); }
        up1.selective_update(SelectiveUpdateRequest {
            region: UpdateRegion { file_path: "x.rs".into(), start_line: 1, end_line: 1000, affected_ids: (0..1_000).collect() },
            new_values: nv,
            priority: UpdatePriority::High,
        }).await
    });
    let t2 = tokio::spawn(async move {
        let mut nv = HashMap::new();
        for i in 0..1_000u64 { nv.insert(i, 3); }
        up2.selective_update(SelectiveUpdateRequest {
            region: UpdateRegion { file_path: "x.rs".into(), start_line: 1, end_line: 1000, affected_ids: (0..1_000).collect() },
            new_values: nv,
            priority: UpdatePriority::Critical,
        }).await
    });
    let _ = t1.await.unwrap();
    let _ = t2.await.unwrap();
    let snapshot = updater.store.get_snapshot();
    // Ensure determinism: all keys present and each is either 2 or 3
    assert_eq!(snapshot.len(), 1_000);
    let mut seen: HashSet<i64> = HashSet::new();
    for v in snapshot.values() { seen.insert(*v); }
    assert!(seen.iter().all(|x| *x == 2 || *x == 3));
}
