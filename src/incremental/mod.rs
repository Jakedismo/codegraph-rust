pub mod cache;
pub mod dependency;
pub mod update;

pub use cache::{IncrementalEmbeddingCache, CachedEmbedding};
pub use dependency::{DependencyGraph, DependencyTracker, Symbol};
pub use update::{UpdateRequest, ChangeType, UpdateProcessor, InvalidationTracker};

use std::path::PathBuf;
use std::time::SystemTime;
use std::collections::HashSet;
use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct ContentHash(pub u64);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateBatch {
    pub requests: Vec<UpdateRequest>,
    pub timestamp: SystemTime,
    pub batch_id: String,
}

#[derive(Debug, Clone)]
pub struct InvalidationResult {
    pub invalidated_files: HashSet<PathBuf>,
    pub affected_symbols: HashSet<Symbol>,
    pub cascade_depth: usize,
}

pub trait IncrementalUpdateStrategy {
    fn should_invalidate(&self, change: &UpdateRequest, dependencies: &DependencyGraph) -> InvalidationResult;
    fn compute_priority(&self, request: &UpdateRequest) -> u8;
    fn batch_compatible(&self, req1: &UpdateRequest, req2: &UpdateRequest) -> bool;
}