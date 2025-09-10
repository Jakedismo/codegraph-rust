use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap, HashSet, VecDeque};
use std::time::{Duration, Instant};

use parking_lot::RwLock;
use tokio::sync::broadcast;

use crate::{CodeGraphError, Result};

/// Impact level used to derive scheduling priority.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ImpactLevel {
    Low = 1,
    Medium = 2,
    High = 3,
    Critical = 4,
}

/// Relationship strength between files to help weight impact amplification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DependencyStrength {
    Weak = 1,
    Medium = 2,
    Strong = 3,
}

/// Types of inter-file relationships.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FileDependencyKind {
    Imports,
    Exports,
    Uses,
    References,
}

#[derive(Debug, Clone)]
pub struct FileDependency {
    pub from: String,
    pub to: String,
    pub kind: FileDependencyKind,
    pub strength: DependencyStrength,
}

/// Tracks inter-file dependencies using a bidirectional adjacency structure.
#[derive(Default)]
pub struct DependencyGraph {
    /// from_file -> set of dependent files (edges out)
    forward: RwLock<HashMap<String, HashSet<String>>>,
    /// to_file -> set of precedent files (edges in)
    reverse: RwLock<HashMap<String, HashSet<String>>>,
    /// Optional edge weights/kinds
    weights: RwLock<HashMap<(String, String), (FileDependencyKind, DependencyStrength)>>,
}

impl DependencyGraph {
    pub fn new() -> Self { Self::default() }

    /// Replace dependencies for a file atomically.
    pub fn set_file_dependencies(&self, file: &str, deps: &[FileDependency]) {
        let mut fwd = self.forward.write();
        let mut rev = self.reverse.write();
        let mut w = self.weights.write();

        // Remove previous edges for file
        if let Some(old_deps) = fwd.get(file).cloned() {
            for to in old_deps {
                if let Some(rset) = rev.get_mut(&to) { rset.remove(file); }
                w.remove(&(file.to_string(), to));
            }
        }

        // Insert new edges
        let mut out = HashSet::with_capacity(deps.len());
        for d in deps {
            out.insert(d.to.clone());
            rev.entry(d.to.clone()).or_default().insert(file.to_string());
            w.insert((file.to_string(), d.to.clone()), (d.kind, d.strength));
        }
        fwd.insert(file.to_string(), out);
    }

    /// Add a single dependency (idempotent).
    pub fn add_dependency(&self, dep: FileDependency) {
        let mut fwd = self.forward.write();
        let mut rev = self.reverse.write();
        let mut w = self.weights.write();

        fwd.entry(dep.from.clone()).or_default().insert(dep.to.clone());
        rev.entry(dep.to.clone()).or_default().insert(dep.from.clone());
        w.insert((dep.from, dep.to), (dep.kind, dep.strength));
    }

    /// Return files that directly depend on `file` (outgoing neighbors).
    pub fn dependents_of(&self, file: &str) -> Vec<String> {
        self.forward
            .read()
            .get(file)
            .map(|s| s.iter().cloned().collect())
            .unwrap_or_default()
    }

    /// Return files that `file` depends on (incoming neighbors via reverse map).
    pub fn prerequisites_of(&self, file: &str) -> Vec<String> {
        self.reverse
            .read()
            .get(file)
            .map(|s| s.iter().cloned().collect())
            .unwrap_or_default()
    }

    /// Get edge weight/kind if present.
    pub fn edge_info(&self, from: &str, to: &str) -> Option<(FileDependencyKind, DependencyStrength)> {
        self.weights.read().get(&(from.to_string(), to.to_string())).cloned()
    }
}

/// An external change detected for a file.
#[derive(Debug, Clone)]
pub struct FileChange {
    pub file_path: String,
    pub impact: ImpactLevel,
    pub details: Option<String>,
    /// Whether this change affects user-visible surface (e.g., public exports)
    pub user_visible: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Priority {
    Low = 1,
    Medium = 2,
    High = 3,
    Critical = 4,
}

impl From<ImpactLevel> for Priority {
    fn from(i: ImpactLevel) -> Self {
        match i {
            ImpactLevel::Low => Priority::Low,
            ImpactLevel::Medium => Priority::Medium,
            ImpactLevel::High => Priority::High,
            ImpactLevel::Critical => Priority::Critical,
        }
    }
}

#[derive(Debug, Clone)]
pub struct UpdateItem {
    pub file_path: String,
    pub priority: Priority,
    pub impact: ImpactLevel,
    pub reason: String,
}

impl PartialEq for UpdateItem {
    fn eq(&self, other: &Self) -> bool {
        self.file_path == other.file_path && self.priority == other.priority
    }
}

impl Eq for UpdateItem {}

impl PartialOrd for UpdateItem {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for UpdateItem {
    fn cmp(&self, other: &Self) -> Ordering {
        // Higher priority first; tie-breaker by reason length and file name
        self.priority.cmp(&other.priority)
            .then_with(|| self.reason.len().cmp(&other.reason.len()))
            .then_with(|| other.file_path.cmp(&self.file_path))
    }
}

#[derive(Debug, Clone)]
pub struct UpdateBatch {
    pub files: Vec<String>,
    pub priority: Priority,
}

/// Notification payload for API layer subscriptions.
#[derive(Debug, Clone)]
pub struct ChangeEventNotification {
    pub changed_files: Vec<String>,
    pub impacted_files: Vec<String>,
    pub batches: Vec<UpdateBatch>,
}

/// Pluggable notifier to bridge into API subscriptions (e.g., async-graphql broker).
pub trait ChangeNotifier: Send + Sync {
    fn notify(&self, event: ChangeEventNotification);
}

/// Manager composing dependency tracking, impact analysis, scheduling and batching.
pub struct ChangePropagationManager {
    deps: DependencyGraph,
    batch_max: usize,
    /// Optional external notifier
    notifier: Option<std::sync::Arc<dyn ChangeNotifier>>,
    /// Internal broadcast channel for subscribers without coupling to API crate
    tx: broadcast::Sender<ChangeEventNotification>,
}

impl ChangePropagationManager {
    pub fn new(batch_max: usize) -> Self {
        let (tx, _rx) = broadcast::channel(256);
        Self { deps: DependencyGraph::new(), batch_max: batch_max.max(1), notifier: None, tx }
    }

    pub fn graph(&self) -> &DependencyGraph { &self.deps }

    pub fn with_notifier(mut self, notifier: std::sync::Arc<dyn ChangeNotifier>) -> Self {
        self.notifier = Some(notifier);
        self
    }

    /// Subscribe to internal broadcast of change notifications.
    pub fn subscribe(&self) -> broadcast::Receiver<ChangeEventNotification> { self.tx.subscribe() }

    /// Compute impacted files from an initial set of changes, schedule by priority,
    /// and return coalesced batches optimized for downstream writes.
    pub fn analyze_and_schedule(&self, changed: Vec<FileChange>) -> Result<(Vec<UpdateBatch>, Vec<String>)> {
        let start = Instant::now();

        // Impact analysis: BFS over dependents avoiding cycles
        let mut impacted = HashSet::new();
        let mut impact_levels: HashMap<String, ImpactLevel> = HashMap::new();
        let mut queue: VecDeque<(String, ImpactLevel, bool)> = VecDeque::new();
        let mut direct_changed = Vec::with_capacity(changed.len());

        for ch in changed.into_iter() {
            impacted.insert(ch.file_path.clone());
            impact_levels.entry(ch.file_path.clone()).and_modify(|lvl| { *lvl = std::cmp::max(*lvl, ch.impact) }).or_insert(ch.impact);
            queue.push_back((ch.file_path.clone(), ch.impact, ch.user_visible));
            direct_changed.push(ch.file_path);
        }

        while let Some((file, base_impact, user_visible)) = queue.pop_front() {
            for dep in self.deps.dependents_of(&file) {
                // amplify impact based on edge info
                let (kind, strength) = self.deps.edge_info(&file, &dep)
                    .unwrap_or((FileDependencyKind::References, DependencyStrength::Medium));
                let mut level = Self::amplify_impact(base_impact, kind, strength);
                if user_visible { level = std::cmp::max(level, ImpactLevel::High); }

                let first_seen = impacted.insert(dep.clone());
                // Track the strongest impact for each file
                impact_levels
                    .entry(dep.clone())
                    .and_modify(|lvl| { *lvl = std::cmp::max(*lvl, level) })
                    .or_insert(level);

                if first_seen {
                    queue.push_back((dep, level, user_visible));
                }
            }
        }

        // Priority scheduling using a binary heap (max-heap via Ord impl)
        let mut heap = BinaryHeap::new();
        for file in &impacted {
            let base_level = *impact_levels.get(file).unwrap_or(&ImpactLevel::Medium);
            let mut priority = Priority::from(base_level);
            // Compute priority from strongest incoming prerequisite or default
            for pre in self.deps.prerequisites_of(file) {
                if let Some((kind, _)) = self.deps.edge_info(&pre, file) {
                    priority = Self::priority_from_kind(kind, priority);
                }
            }
            heap.push(UpdateItem {
                file_path: file.clone(),
                impact: base_level,
                priority,
                reason: "impact_analysis".to_string(),
            });
        }

        // Batch by priority while preserving priority groups
        let mut batches: Vec<UpdateBatch> = Vec::new();
        let mut by_priority: HashMap<Priority, Vec<String>> = HashMap::new();
        while let Some(item) = heap.pop() {
            by_priority.entry(item.priority).or_default().push(item.file_path);
        }
        for (priority, mut files) in by_priority.into_iter() {
            // Stable order to improve cache behavior
            files.sort();
            for chunk in files.chunks(self.batch_max) {
                batches.push(UpdateBatch { files: chunk.to_vec(), priority });
            }
        }

        // Emit notification
        let impacted_vec: Vec<String> = impacted.into_iter().collect();
        let notif = ChangeEventNotification { changed_files: direct_changed.clone(), impacted_files: impacted_vec.clone(), batches: batches.clone() };
        let _ = self.tx.send(notif.clone());
        if let Some(n) = &self.notifier { n.notify(notif); }

        // Performance guardrail (best-effort): warn if breached
        let _elapsed = start.elapsed();
        // In core we avoid logging dependencies; callers can time this externally.

        Ok((batches, impacted_vec))
    }

    fn amplify_impact(base: ImpactLevel, kind: FileDependencyKind, strength: DependencyStrength) -> ImpactLevel {
        use DependencyStrength::*;
        use FileDependencyKind::*;
        let bump = match (kind, strength) {
            (Exports, Strong) => 2,
            (Imports, Strong) => 1,
            (Uses, Strong) => 1,
            (References, Strong) => 0,
            (_, Medium) => 0,
            (_, Weak) => 0,
        };
        let val = (base as i32 + bump).clamp(ImpactLevel::Low as i32, ImpactLevel::Critical as i32);
        match val {
            1 => ImpactLevel::Low,
            2 => ImpactLevel::Medium,
            3 => ImpactLevel::High,
            _ => ImpactLevel::Critical,
        }
    }

    fn priority_from_kind(kind: FileDependencyKind, current: Priority) -> Priority {
        use FileDependencyKind::*;
        let p = match kind {
            Exports => Priority::Critical,
            Imports => Priority::High,
            Uses => Priority::Medium,
            References => Priority::Low,
        };
        std::cmp::max(p, current)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dep(from: &str, to: &str, kind: FileDependencyKind) -> FileDependency {
        FileDependency { from: from.to_string(), to: to.to_string(), kind, strength: DependencyStrength::Medium }
    }

    #[test]
    fn dependency_graph_basic() {
        let g = DependencyGraph::new();
        g.add_dependency(dep("a.rs", "b.rs", FileDependencyKind::Imports));
        g.add_dependency(dep("b.rs", "c.rs", FileDependencyKind::Uses));

        assert_eq!(g.dependents_of("a.rs"), vec!["b.rs".to_string()]);
        let mut prereq_c = g.prerequisites_of("c.rs"); prereq_c.sort();
        assert_eq!(prereq_c, vec!["b.rs".to_string()]);
    }

    #[test]
    fn impact_analysis_chain() {
        let mgr = ChangePropagationManager::new(10);
        mgr.graph().add_dependency(dep("a", "b", FileDependencyKind::Imports));
        mgr.graph().add_dependency(dep("b", "c", FileDependencyKind::Imports));

        let (batches, impacted) = mgr
            .analyze_and_schedule(vec![FileChange { file_path: "a".into(), impact: ImpactLevel::Medium, details: None, user_visible: false }])
            .unwrap();

        // All should be impacted: a, b, c
        let mut set: HashSet<_> = impacted.into_iter().collect();
        assert!(set.remove("a"));
        assert!(set.remove("b"));
        assert!(set.remove("c"));
        assert!(set.is_empty());

        // Batches grouped by priority exist
        assert!(!batches.is_empty());
        let total: usize = batches.iter().map(|b| b.files.len()).sum();
        assert_eq!(total, 3);
    }

    #[test]
    fn impact_analysis_cycle_no_infinite_loop() {
        let mgr = ChangePropagationManager::new(10);
        mgr.graph().add_dependency(dep("a", "b", FileDependencyKind::Imports));
        mgr.graph().add_dependency(dep("b", "a", FileDependencyKind::Imports));

        let (_batches, impacted) = mgr
            .analyze_and_schedule(vec![FileChange { file_path: "a".into(), impact: ImpactLevel::High, details: None, user_visible: false }])
            .unwrap();

        let mut set: HashSet<_> = impacted.into_iter().collect();
        assert!(set.remove("a"));
        assert!(set.remove("b"));
        assert!(set.is_empty());
    }

    #[test]
    fn priority_ranks_user_visible_exports_higher() {
        let mgr = ChangePropagationManager::new(10);
        mgr.graph().add_dependency(FileDependency { from: "core".into(), to: "api".into(), kind: FileDependencyKind::Exports, strength: DependencyStrength::Strong });

        let (batches, _impacted) = mgr
            .analyze_and_schedule(vec![FileChange { file_path: "core".into(), impact: ImpactLevel::Medium, details: None, user_visible: true }])
            .unwrap();

        assert_eq!(batches.len(), 1);
        assert_eq!(batches[0].priority, Priority::Critical);
        let mut files = batches[0].files.clone(); files.sort();
        assert_eq!(files, vec!["api", "core"]);
    }

    #[test]
    fn batching_reduces_write_pressure() {
        let mgr = ChangePropagationManager::new(5);
        for i in 0..30 { // star: root -> leaf{i}
            let leaf = format!("leaf{}", i);
            mgr.graph().add_dependency(dep("root", &leaf, FileDependencyKind::Imports));
        }

        let (batches, impacted) = mgr
            .analyze_and_schedule(vec![FileChange { file_path: "root".into(), impact: ImpactLevel::Medium, details: None, user_visible: false }])
            .unwrap();

        let naive_writes = impacted.len();
        let batched_writes: usize = batches.len();
        // Expect batching to reduce writes by >= 70%
        // i.e., number_of_batches <= 30% of naive individual writes
        assert!(batched_writes * 10 <= naive_writes * 3, "expected at least 70% reduction: naive={}, batches={}", naive_writes, batched_writes);
    }

    #[tokio::test]
    async fn notifications_are_broadcast() {
        let mgr = ChangePropagationManager::new(3);
        mgr.graph().add_dependency(dep("x", "y", FileDependencyKind::Uses));
        let mut rx = mgr.subscribe();
        let _ = mgr.analyze_and_schedule(vec![FileChange { file_path: "x".into(), impact: ImpactLevel::Low, details: None, user_visible: false }]).unwrap();
        let evt = rx.recv().await.expect("should receive event");
        assert!(evt.changed_files.contains(&"x".to_string()));
        assert!(evt.impacted_files.iter().any(|f| f == "y" || f == "x"));
    }
}
