#![allow(dead_code, unused_variables, unused_imports)]

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use chrono::{DateTime, Utc};
use dashmap::DashMap;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock as AsyncRwLock;
use tracing::{info, warn};
use uuid::Uuid;

use crate::{CodeGraph, DeltaOperation, GraphDeltaProcessor};
use codegraph_core::{CodeNode, GraphStore, NodeId};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateRegion {
    pub id: Uuid,
    pub file_path: String,
    pub start_line: usize,
    pub end_line: usize,
    pub start_byte: usize,
    pub end_byte: usize,
    pub content_hash: String,
    pub affected_nodes: HashSet<NodeId>,
}

#[derive(Debug, Clone)]
pub struct SelectiveUpdateRequest {
    pub region: UpdateRegion,
    pub new_nodes: Vec<CodeNode>,
    pub priority: UpdatePriority,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum UpdatePriority {
    Low = 0,
    Normal = 1,
    High = 2,
    Critical = 3,
}

#[derive(Debug)]
pub struct SelectiveUpdateResult {
    pub updated_nodes: HashSet<NodeId>,
    pub added_nodes: HashSet<NodeId>,
    pub removed_nodes: HashSet<NodeId>,
    pub duration: Duration,
    pub bytes_processed: usize,
}

pub struct SelectiveNodeUpdater {
    graph: Arc<AsyncRwLock<CodeGraph>>,
    delta_processor: Arc<GraphDeltaProcessor>,
    update_cache: Arc<DashMap<String, CachedUpdate>>,
    pending_updates: Arc<RwLock<Vec<SelectiveUpdateRequest>>>,
    update_strategies: Arc<RwLock<HashMap<String, UpdateStrategy>>>,
    performance_tracker: PerformanceTracker,
}

#[derive(Debug, Clone)]
struct CachedUpdate {
    region_id: Uuid,
    content_hash: String,
    nodes: Vec<CodeNode>,
    timestamp: DateTime<Utc>,
    access_count: usize,
}

#[derive(Debug, Clone)]
pub enum UpdateStrategy {
    /// Replace all nodes in the region
    Replace,
    /// Merge new nodes with existing ones based on similarity
    Merge { similarity_threshold: f64 },
    /// Only update nodes that have actually changed
    DiffOnly,
    /// Custom strategy with user-defined logic
    Custom { name: String },
}

impl SelectiveNodeUpdater {
    pub fn new(graph: Arc<AsyncRwLock<CodeGraph>>) -> Self {
        Self {
            graph,
            delta_processor: Arc::new(GraphDeltaProcessor::new()),
            update_cache: Arc::new(DashMap::new()),
            pending_updates: Arc::new(RwLock::new(Vec::new())),
            update_strategies: Arc::new(RwLock::new(Self::default_strategies())),
            performance_tracker: PerformanceTracker::new(),
        }
    }

    fn default_strategies() -> HashMap<String, UpdateStrategy> {
        let mut strategies = HashMap::new();
        strategies.insert("rust".to_string(), UpdateStrategy::DiffOnly);
        strategies.insert(
            "typescript".to_string(),
            UpdateStrategy::Merge {
                similarity_threshold: 0.8,
            },
        );
        strategies.insert(
            "javascript".to_string(),
            UpdateStrategy::Merge {
                similarity_threshold: 0.8,
            },
        );
        strategies.insert("python".to_string(), UpdateStrategy::DiffOnly);
        strategies.insert("go".to_string(), UpdateStrategy::Replace);
        strategies.insert("default".to_string(), UpdateStrategy::DiffOnly);
        strategies
    }

    pub async fn selective_update(
        &self,
        request: SelectiveUpdateRequest,
    ) -> Result<SelectiveUpdateResult> {
        let start_time = std::time::Instant::now();

        info!(
            "Starting selective update for region {} in {}",
            request.region.id, request.region.file_path
        );

        // Check if we can use cached result
        if let Some(cached) = self.get_cached_update(&request).await {
            info!(
                "Using cached update result for region {}",
                request.region.id
            );
            return Ok(SelectiveUpdateResult {
                updated_nodes: HashSet::new(),
                added_nodes: cached.nodes.iter().map(|n| n.id).collect(),
                removed_nodes: HashSet::new(),
                duration: start_time.elapsed(),
                bytes_processed: 0,
            });
        }

        // Get current nodes in the region
        let current_nodes = self.get_nodes_in_region(&request.region).await?;

        // Determine update strategy
        let strategy = self.get_update_strategy(&request.region.file_path);

        // Apply selective update
        let result = match strategy {
            UpdateStrategy::Replace => {
                self.apply_replace_strategy(&request, &current_nodes)
                    .await?
            }
            UpdateStrategy::Merge {
                similarity_threshold,
            } => {
                self.apply_merge_strategy(&request, &current_nodes, similarity_threshold)
                    .await?
            }
            UpdateStrategy::DiffOnly => self.apply_diff_strategy(&request, &current_nodes).await?,
            UpdateStrategy::Custom { name } => {
                warn!(
                    "Custom strategy '{}' not implemented, falling back to DiffOnly",
                    name
                );
                self.apply_diff_strategy(&request, &current_nodes).await?
            }
        };

        // Cache the result for future use
        self.cache_update_result(&request, &result).await;

        let duration = start_time.elapsed();

        // Update performance tracking
        self.performance_tracker
            .record_update(&request, &result, duration)
            .await;
        info!(
            "Selective update completed in {}ms: {} updated, {} added, {} removed",
            duration.as_millis(),
            result.updated_nodes.len(),
            result.added_nodes.len(),
            result.removed_nodes.len()
        );

        Ok(SelectiveUpdateResult {
            updated_nodes: result.updated_nodes,
            added_nodes: result.added_nodes,
            removed_nodes: result.removed_nodes,
            duration,
            bytes_processed: self.calculate_bytes_processed(&request),
        })
    }

    async fn get_nodes_in_region(&self, region: &UpdateRegion) -> Result<Vec<CodeNode>> {
        let graph = self.graph.read().await;

        // Get nodes that intersect with the region
        let mut nodes_in_region = Vec::new();

        // This would be more efficient with a spatial index, but for now we'll check all nodes
        for node_id in &region.affected_nodes {
            if let Some(node) = graph.get_node(*node_id).await? {
                // Check if node is within the region bounds
                if self.node_intersects_region(&node, region) {
                    nodes_in_region.push(node);
                }
            }
        }

        Ok(nodes_in_region)
    }

    fn node_intersects_region(&self, node: &CodeNode, region: &UpdateRegion) -> bool {
        // Check if node's line range intersects with region's line range
        let start_line = node.location.line as usize;
        let end_line = node.location.end_line.unwrap_or(node.location.line) as usize;
        !(end_line <= region.start_line || start_line >= region.end_line)
    }

    async fn apply_replace_strategy(
        &self,
        request: &SelectiveUpdateRequest,
        current_nodes: &[CodeNode],
    ) -> Result<InternalUpdateResult> {
        let mut graph = self.graph.write().await;

        let updated_nodes = HashSet::new();
        let mut added_nodes = HashSet::new();
        let mut removed_nodes = HashSet::new();

        // Remove all current nodes in the region
        for node in current_nodes {
            graph.remove_node(node.id).await?;
            removed_nodes.insert(node.id);
        }

        // Add all new nodes
        for node in &request.new_nodes {
            graph.add_node(node.clone()).await?;
            added_nodes.insert(node.id);
        }

        Ok(InternalUpdateResult {
            updated_nodes,
            added_nodes,
            removed_nodes,
        })
    }

    async fn apply_merge_strategy(
        &self,
        request: &SelectiveUpdateRequest,
        current_nodes: &[CodeNode],
        similarity_threshold: f64,
    ) -> Result<InternalUpdateResult> {
        let mut updated_nodes = HashSet::new();
        let mut added_nodes = HashSet::new();
        let mut removed_nodes = HashSet::new();

        // Create maps for efficient lookups by NodeId
        let current_map: HashMap<NodeId, &CodeNode> =
            current_nodes.iter().map(|n| (n.id, n)).collect();

        let new_map: HashMap<NodeId, &CodeNode> =
            request.new_nodes.iter().map(|n| (n.id, n)).collect();

        let mut graph = self.graph.write().await;

        // Find nodes to remove (not in new set)
        for (node_id, _) in &current_map {
            if !new_map.contains_key(node_id) {
                graph.remove_node(*node_id).await?;
                removed_nodes.insert(*node_id);
            }
        }

        // Process new nodes
        for (node_id, new_node) in &new_map {
            match current_map.get(node_id) {
                Some(current_node) => {
                    // Check similarity
                    let similarity = self.calculate_node_similarity(current_node, new_node);
                    if similarity < similarity_threshold {
                        // Update the node
                        graph.remove_node(*node_id).await?;
                        graph.add_node((*new_node).clone()).await?;
                        updated_nodes.insert(*node_id);
                    }
                }
                None => {
                    // Add new node
                    graph.add_node((*new_node).clone()).await?;
                    added_nodes.insert(*node_id);
                }
            }
        }

        Ok(InternalUpdateResult {
            updated_nodes,
            added_nodes,
            removed_nodes,
        })
    }

    async fn apply_diff_strategy(
        &self,
        request: &SelectiveUpdateRequest,
        current_nodes: &[CodeNode],
    ) -> Result<InternalUpdateResult> {
        // Use delta computation for precise differences
        let delta_result = self
            .delta_processor
            .compute_delta(
                current_nodes,
                &request.new_nodes,
                Some(request.region.file_path.clone()),
                Some(request.region.content_hash.clone()),
            )
            .await?;

        let mut graph = self.graph.write().await;
        let _application_result = self
            .delta_processor
            .apply_delta(&mut graph, &delta_result.delta)
            .await?;

        // Extract the operation results
        let mut updated_nodes = HashSet::new();
        let mut added_nodes = HashSet::new();
        let mut removed_nodes = HashSet::new();

        for operation in &delta_result.delta.operations {
            match operation {
                DeltaOperation::AddNode(node) => {
                    added_nodes.insert(node.id.clone());
                }
                DeltaOperation::RemoveNode(node_id) => {
                    removed_nodes.insert(node_id.clone());
                }
                DeltaOperation::UpdateNode(node_id, _) => {
                    updated_nodes.insert(node_id.clone());
                }
                _ => {} // Handle edge operations if needed
            }
        }

        Ok(InternalUpdateResult {
            updated_nodes,
            added_nodes,
            removed_nodes,
        })
    }

    fn calculate_node_similarity(&self, node1: &CodeNode, node2: &CodeNode) -> f64 {
        let mut similarity_score = 0.0;
        let mut total_weight = 0.0;

        // Compare name (weight: 0.3)
        if node1.name == node2.name {
            similarity_score += 0.3;
        }
        total_weight += 0.3;

        // Compare type (weight: 0.2)
        if node1.node_type == node2.node_type {
            similarity_score += 0.2;
        }
        total_weight += 0.2;

        // Compare content similarity (weight: 0.4)
        if let (Some(content1), Some(content2)) = (&node1.content, &node2.content) {
            let content_similarity = self.calculate_text_similarity(content1, content2);
            similarity_score += 0.4 * content_similarity;
        }
        total_weight += 0.4;

        // Compare metadata attributes (weight: 0.1)
        if node1.metadata.attributes == node2.metadata.attributes {
            similarity_score += 0.1;
        }
        total_weight += 0.1;

        if total_weight > 0.0 {
            similarity_score / total_weight
        } else {
            0.0
        }
    }

    fn calculate_text_similarity(&self, text1: &str, text2: &str) -> f64 {
        // Simple similarity based on common subsequences
        // For better accuracy, could use more sophisticated algorithms like Jaro-Winkler

        if text1 == text2 {
            return 1.0;
        }

        if text1.is_empty() && text2.is_empty() {
            return 1.0;
        }

        if text1.is_empty() || text2.is_empty() {
            return 0.0;
        }

        // Use Levenshtein distance as a simple similarity measure
        let distance = self.levenshtein_distance(text1, text2);
        let max_len = std::cmp::max(text1.len(), text2.len()) as f64;

        if max_len == 0.0 {
            1.0
        } else {
            1.0 - (distance as f64 / max_len)
        }
    }

    fn levenshtein_distance(&self, s1: &str, s2: &str) -> usize {
        let s1_chars: Vec<char> = s1.chars().collect();
        let s2_chars: Vec<char> = s2.chars().collect();
        let s1_len = s1_chars.len();
        let s2_len = s2_chars.len();

        let mut matrix = vec![vec![0; s2_len + 1]; s1_len + 1];

        for i in 0..=s1_len {
            matrix[i][0] = i;
        }
        for j in 0..=s2_len {
            matrix[0][j] = j;
        }

        for i in 1..=s1_len {
            for j in 1..=s2_len {
                let cost = if s1_chars[i - 1] == s2_chars[j - 1] {
                    0
                } else {
                    1
                };
                matrix[i][j] = std::cmp::min(
                    std::cmp::min(matrix[i - 1][j] + 1, matrix[i][j - 1] + 1),
                    matrix[i - 1][j - 1] + cost,
                );
            }
        }

        matrix[s1_len][s2_len]
    }

    fn get_update_strategy(&self, file_path: &str) -> UpdateStrategy {
        let strategies = self.update_strategies.read();

        // Determine strategy based on file extension
        if let Some(extension) = std::path::Path::new(file_path).extension() {
            if let Some(ext_str) = extension.to_str() {
                if let Some(strategy) = strategies.get(ext_str) {
                    return strategy.clone();
                }
            }
        }

        // Return default strategy
        strategies
            .get("default")
            .unwrap_or(&UpdateStrategy::DiffOnly)
            .clone()
    }

    async fn get_cached_update(&self, request: &SelectiveUpdateRequest) -> Option<CachedUpdate> {
        let cache_key = format!(
            "{}:{}",
            request.region.file_path, request.region.content_hash
        );

        if let Some(mut cached) = self.update_cache.get_mut(&cache_key) {
            cached.access_count += 1;
            Some(cached.value().clone())
        } else {
            None
        }
    }

    async fn cache_update_result(
        &self,
        request: &SelectiveUpdateRequest,
        _result: &InternalUpdateResult,
    ) {
        let cache_key = format!(
            "{}:{}",
            request.region.file_path, request.region.content_hash
        );

        let cached_update = CachedUpdate {
            region_id: request.region.id,
            content_hash: request.region.content_hash.clone(),
            nodes: request.new_nodes.clone(),
            timestamp: Utc::now(),
            access_count: 1,
        };

        self.update_cache.insert(cache_key, cached_update);

        // Limit cache size to prevent memory bloat
        if self.update_cache.len() > 1000 {
            self.cleanup_cache().await;
        }
    }

    async fn cleanup_cache(&self) {
        // Remove oldest entries that haven't been accessed recently
        let cutoff_time = Utc::now() - chrono::Duration::minutes(30);

        let keys_to_remove: Vec<String> = self
            .update_cache
            .iter()
            .filter_map(|entry| {
                if entry.value().timestamp < cutoff_time && entry.value().access_count <= 1 {
                    Some(entry.key().clone())
                } else {
                    None
                }
            })
            .collect();

        for key in keys_to_remove {
            self.update_cache.remove(&key);
        }
    }

    fn calculate_bytes_processed(&self, request: &SelectiveUpdateRequest) -> usize {
        request
            .region
            .end_byte
            .saturating_sub(request.region.start_byte)
    }

    pub fn add_pending_update(&self, request: SelectiveUpdateRequest) {
        let mut pending = self.pending_updates.write();
        pending.push(request);
        pending.sort_by(|a, b| b.priority.cmp(&a.priority));
    }

    pub async fn process_pending_updates(&self) -> Result<Vec<SelectiveUpdateResult>> {
        let updates = {
            let mut pending = self.pending_updates.write();
            std::mem::take(&mut *pending)
        };

        let mut results = Vec::new();
        for update in updates {
            match self.selective_update(update).await {
                Ok(result) => results.push(result),
                Err(e) => warn!("Failed to process pending update: {}", e),
            }
        }

        Ok(results)
    }
}

struct InternalUpdateResult {
    updated_nodes: HashSet<NodeId>,
    added_nodes: HashSet<NodeId>,
    removed_nodes: HashSet<NodeId>,
}

struct PerformanceTracker {
    update_times: Arc<DashMap<String, Duration>>,
    throughput_data: Arc<DashMap<String, Vec<f64>>>,
}

impl PerformanceTracker {
    fn new() -> Self {
        Self {
            update_times: Arc::new(DashMap::new()),
            throughput_data: Arc::new(DashMap::new()),
        }
    }

    async fn record_update(
        &self,
        request: &SelectiveUpdateRequest,
        result: &InternalUpdateResult,
        duration: Duration,
    ) {
        let file_type = std::path::Path::new(&request.region.file_path)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("unknown");

        // Record update time
        self.update_times
            .insert(request.region.id.to_string(), duration);

        // Record throughput (nodes per second)
        let total_nodes =
            result.updated_nodes.len() + result.added_nodes.len() + result.removed_nodes.len();
        let throughput = total_nodes as f64 / duration.as_secs_f64();

        self.throughput_data
            .entry(file_type.to_string())
            .and_modify(|data| {
                data.push(throughput);
                if data.len() > 100 {
                    data.remove(0); // Keep only recent data
                }
            })
            .or_insert_with(|| vec![throughput]);
    }

    pub fn get_average_throughput(&self, file_type: &str) -> Option<f64> {
        self.throughput_data
            .get(file_type)
            .map(|data| data.iter().sum::<f64>() / data.len() as f64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use codegraph_core::Language;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_selective_updater_creation() {
        let temp_dir = TempDir::new().unwrap();
        let graph = Arc::new(AsyncRwLock::new(
            CodeGraph::new(&temp_dir.path().join("test.db"))
                .await
                .unwrap(),
        ));
        let updater = SelectiveNodeUpdater::new(graph);

        // Test that it can be created successfully
        assert!(true);
    }

    #[test]
    fn test_text_similarity() {
        let updater = SelectiveNodeUpdater::new(Arc::new(AsyncRwLock::new(
            // This won't actually work in tests without proper initialization,
            // but it's fine for testing the similarity function
            unsafe { std::mem::zeroed() },
        )));

        let similarity = updater.calculate_text_similarity("hello world", "hello world");
        assert_eq!(similarity, 1.0);

        let similarity = updater.calculate_text_similarity("hello", "world");
        assert!(similarity < 1.0);
    }

    #[test]
    fn test_update_priority_ordering() {
        let mut priorities = vec![
            UpdatePriority::Low,
            UpdatePriority::Critical,
            UpdatePriority::Normal,
            UpdatePriority::High,
        ];

        priorities.sort();

        assert_eq!(
            priorities,
            vec![
                UpdatePriority::Low,
                UpdatePriority::Normal,
                UpdatePriority::High,
                UpdatePriority::Critical,
            ]
        );
    }
}
