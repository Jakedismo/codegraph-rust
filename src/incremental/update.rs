use super::{InvalidationResult, DependencyGraph, IncrementalUpdateStrategy};
use crate::embedding::EmbeddingError;

use std::collections::HashSet;
use std::path::PathBuf;
use std::time::SystemTime;
use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateRequest {
    pub file_path: PathBuf,
    pub change_type: ChangeType,
    pub affected_symbols: Vec<super::Symbol>,
    pub timestamp: SystemTime,
    pub priority: UpdatePriority,
    pub metadata: UpdateMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ChangeType {
    Added,
    Modified,
    Deleted,
    Moved(PathBuf),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum UpdatePriority {
    Low = 0,
    Normal = 1,
    High = 2,
    Critical = 3,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateMetadata {
    pub change_size: ChangeSize,
    pub file_type: FileType,
    pub is_test_file: bool,
    pub is_generated: bool,
    pub last_build_time: Option<SystemTime>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ChangeSize {
    Small,      // < 100 lines changed
    Medium,     // 100-1000 lines changed
    Large,      // > 1000 lines changed
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FileType {
    Source,
    Header,
    Test,
    Documentation,
    Configuration,
    Build,
}

pub struct UpdateProcessor {
    strategy: Box<dyn IncrementalUpdateStrategy + Send + Sync>,
    batch_size: usize,
    max_cascade_depth: usize,
}

impl UpdateProcessor {
    pub fn new() -> Self {
        Self {
            strategy: Box::new(SmartInvalidationStrategy::new()),
            batch_size: 50,
            max_cascade_depth: 10,
        }
    }

    pub fn with_strategy(strategy: Box<dyn IncrementalUpdateStrategy + Send + Sync>) -> Self {
        Self {
            strategy,
            batch_size: 50,
            max_cascade_depth: 10,
        }
    }

    pub fn process_update(&self, request: &UpdateRequest, dependencies: &DependencyGraph) -> Result<InvalidationResult, EmbeddingError> {
        let result = self.strategy.should_invalidate(request, dependencies);
        
        // Limit cascade depth to prevent excessive invalidation
        if result.cascade_depth > self.max_cascade_depth {
            return Ok(InvalidationResult {
                invalidated_files: HashSet::new(),
                affected_symbols: HashSet::new(),
                cascade_depth: 0,
            });
        }

        Ok(result)
    }

    pub fn process_batch(&self, requests: Vec<UpdateRequest>, dependencies: &DependencyGraph) -> Result<Vec<InvalidationResult>, EmbeddingError> {
        let mut results = Vec::with_capacity(requests.len());
        let mut processed_files = HashSet::new();

        // Sort by priority first
        let mut sorted_requests = requests;
        sorted_requests.sort_by(|a, b| b.priority.cmp(&a.priority));

        for request in sorted_requests {
            // Skip if we've already processed this file in this batch
            if processed_files.contains(&request.file_path) {
                continue;
            }

            let result = self.process_update(&request, dependencies)?;
            
            // Track processed files to avoid duplicate processing
            processed_files.insert(request.file_path.clone());
            for file in &result.invalidated_files {
                processed_files.insert(file.clone());
            }

            results.push(result);
        }

        Ok(results)
    }
}

pub struct InvalidationTracker {
    pending_invalidations: HashSet<PathBuf>,
    completed_invalidations: HashSet<PathBuf>,
    invalidation_history: Vec<InvalidationEvent>,
    max_history_size: usize,
}

#[derive(Debug, Clone)]
pub struct InvalidationEvent {
    pub file_path: PathBuf,
    pub timestamp: SystemTime,
    pub trigger: InvalidationTrigger,
    pub cascade_size: usize,
}

#[derive(Debug, Clone)]
pub enum InvalidationTrigger {
    DirectChange,
    DependencyChange(PathBuf),
    BatchInvalidation,
}

impl InvalidationTracker {
    pub fn new() -> Self {
        Self {
            pending_invalidations: HashSet::new(),
            completed_invalidations: HashSet::new(),
            invalidation_history: Vec::new(),
            max_history_size: 1000,
        }
    }

    pub fn add_pending(&mut self, files: &[PathBuf], trigger: InvalidationTrigger) {
        for file in files {
            self.pending_invalidations.insert(file.clone());
            
            let event = InvalidationEvent {
                file_path: file.clone(),
                timestamp: SystemTime::now(),
                trigger: trigger.clone(),
                cascade_size: files.len(),
            };
            
            self.add_to_history(event);
        }
    }

    pub fn mark_completed(&mut self, file: &PathBuf) -> bool {
        if self.pending_invalidations.remove(file) {
            self.completed_invalidations.insert(file.clone());
            true
        } else {
            false
        }
    }

    pub fn get_pending(&self) -> &HashSet<PathBuf> {
        &self.pending_invalidations
    }

    pub fn get_completed(&self) -> &HashSet<PathBuf> {
        &self.completed_invalidations
    }

    pub fn get_recent_history(&self, limit: usize) -> &[InvalidationEvent] {
        let start = if self.invalidation_history.len() > limit {
            self.invalidation_history.len() - limit
        } else {
            0
        };
        
        &self.invalidation_history[start..]
    }

    pub fn clear_completed(&mut self) {
        self.completed_invalidations.clear();
    }

    fn add_to_history(&mut self, event: InvalidationEvent) {
        self.invalidation_history.push(event);
        
        if self.invalidation_history.len() > self.max_history_size {
            self.invalidation_history.remove(0);
        }
    }
}

pub struct SmartInvalidationStrategy {
    file_type_weights: std::collections::HashMap<FileType, f32>,
    dependency_strength_multipliers: std::collections::HashMap<super::DependencyStrength, f32>,
}

impl SmartInvalidationStrategy {
    pub fn new() -> Self {
        let mut file_type_weights = std::collections::HashMap::new();
        file_type_weights.insert(FileType::Source, 1.0);
        file_type_weights.insert(FileType::Header, 1.5);
        file_type_weights.insert(FileType::Test, 0.3);
        file_type_weights.insert(FileType::Documentation, 0.1);
        file_type_weights.insert(FileType::Configuration, 0.8);
        file_type_weights.insert(FileType::Build, 0.9);

        let mut dependency_strength_multipliers = std::collections::HashMap::new();
        dependency_strength_multipliers.insert(super::DependencyStrength::Weak, 0.2);
        dependency_strength_multipliers.insert(super::DependencyStrength::Medium, 0.6);
        dependency_strength_multipliers.insert(super::DependencyStrength::Strong, 1.0);
        dependency_strength_multipliers.insert(super::DependencyStrength::Critical, 2.0);

        Self {
            file_type_weights,
            dependency_strength_multipliers,
        }
    }
}

impl IncrementalUpdateStrategy for SmartInvalidationStrategy {
    fn should_invalidate(&self, change: &UpdateRequest, dependencies: &DependencyGraph) -> InvalidationResult {
        let mut invalidated_files = HashSet::new();
        let mut affected_symbols = HashSet::new();
        
        // Always invalidate the changed file itself
        invalidated_files.insert(change.file_path.clone());
        
        // Get base invalidation weight
        let base_weight = self.file_type_weights
            .get(&change.metadata.file_type)
            .copied()
            .unwrap_or(1.0);

        // Apply change size multiplier
        let size_multiplier = match change.metadata.change_size {
            ChangeSize::Small => 0.5,
            ChangeSize::Medium => 1.0,
            ChangeSize::Large => 2.0,
        };

        let invalidation_threshold = base_weight * size_multiplier;

        // Analyze dependencies for affected symbols
        for symbol in &change.affected_symbols {
            let dependents = dependencies.compute_transitive_dependents(symbol, 3);
            
            for dependent in dependents {
                affected_symbols.insert(dependent.clone());
                invalidated_files.insert(dependent.location.file_path.clone());
            }
        }

        // Calculate cascade depth
        let cascade_depth = if invalidated_files.len() <= 1 {
            0
        } else if invalidated_files.len() <= 5 {
            1
        } else if invalidated_files.len() <= 20 {
            2
        } else {
            3
        };

        // Limit invalidation for large changes
        if invalidation_threshold > 1.5 && invalidated_files.len() > 50 {
            // Only invalidate direct dependencies
            invalidated_files.retain(|file| {
                file == &change.file_path || 
                change.affected_symbols.iter().any(|s| s.location.file_path == *file)
            });
        }

        InvalidationResult {
            invalidated_files,
            affected_symbols,
            cascade_depth,
        }
    }

    fn compute_priority(&self, request: &UpdateRequest) -> u8 {
        let mut priority_score = match request.priority {
            UpdatePriority::Critical => 100,
            UpdatePriority::High => 75,
            UpdatePriority::Normal => 50,
            UpdatePriority::Low => 25,
        };

        // Boost priority for certain file types
        match request.metadata.file_type {
            FileType::Header => priority_score += 20,
            FileType::Build | FileType::Configuration => priority_score += 15,
            FileType::Test => priority_score -= 10,
            FileType::Documentation => priority_score -= 20,
            _ => {}
        }

        // Adjust for change size
        match request.metadata.change_size {
            ChangeSize::Large => priority_score += 10,
            ChangeSize::Small => priority_score -= 5,
            _ => {}
        }

        priority_score.min(255) as u8
    }

    fn batch_compatible(&self, req1: &UpdateRequest, req2: &UpdateRequest) -> bool {
        // Compatible if same file type and similar priority
        req1.metadata.file_type == req2.metadata.file_type &&
        (req1.priority as u8).abs_diff(req2.priority as u8) <= 1
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::{Symbol, SymbolType, SymbolLocation};

    #[test]
    fn test_update_priority_ordering() {
        let critical = UpdatePriority::Critical;
        let high = UpdatePriority::High;
        let normal = UpdatePriority::Normal;
        let low = UpdatePriority::Low;

        assert!(critical > high);
        assert!(high > normal);
        assert!(normal > low);
    }

    #[test]
    fn test_smart_invalidation_strategy() {
        let strategy = SmartInvalidationStrategy::new();
        let dependencies = DependencyGraph::new();
        
        let request = UpdateRequest {
            file_path: PathBuf::from("test.rs"),
            change_type: ChangeType::Modified,
            affected_symbols: vec![],
            timestamp: SystemTime::now(),
            priority: UpdatePriority::Normal,
            metadata: UpdateMetadata {
                change_size: ChangeSize::Small,
                file_type: FileType::Source,
                is_test_file: false,
                is_generated: false,
                last_build_time: None,
            },
        };

        let result = strategy.should_invalidate(&request, &dependencies);
        assert!(!result.invalidated_files.is_empty());
        assert!(result.invalidated_files.contains(&PathBuf::from("test.rs")));
    }

    #[test]
    fn test_invalidation_tracker() {
        let mut tracker = InvalidationTracker::new();
        let files = vec![PathBuf::from("test1.rs"), PathBuf::from("test2.rs")];
        
        tracker.add_pending(&files, InvalidationTrigger::DirectChange);
        assert_eq!(tracker.get_pending().len(), 2);
        
        tracker.mark_completed(&files[0]);
        assert_eq!(tracker.get_pending().len(), 1);
        assert_eq!(tracker.get_completed().len(), 1);
    }
}

impl Default for UpdateProcessor {
    fn default() -> Self {
        Self::new()
    }
}

impl Default for InvalidationTracker {
    fn default() -> Self {
        Self::new()
    }
}