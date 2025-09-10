//! Archived data structures for CodeGraph
//!
//! This module defines zero-copy archived versions of core CodeGraph data structures
//! that can be directly accessed from serialized form without deserialization.

use rkyv::{Archive, Deserialize, Serialize};
use std::collections::HashMap;

/// Archived version of a code node with zero-copy access
#[derive(Archive, Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct ArchivedCodeNode {
    pub id: u64,
    pub name: String,
    pub node_type: String,
    pub file_path: String,
    pub start_line: u32,
    pub end_line: u32,
    pub start_column: u32,
    pub end_column: u32,
    pub content: String,
    pub hash: String,
    pub parent_id: Option<u64>,
    pub children: Vec<u64>,
    pub metadata: HashMap<String, String>,
}

/// Archived version of a code edge representing relationships
#[derive(Archive, Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct ArchivedCodeEdge {
    pub id: u64,
    pub source_id: u64,
    pub target_id: u64,
    pub edge_type: String,
    pub weight: f32,
    pub metadata: HashMap<String, String>,
}

/// Archived version of a code graph containing nodes and edges
#[derive(Archive, Serialize, Deserialize, Debug, Clone)]
pub struct ArchivedCodeGraph {
    pub version: u32,
    pub timestamp: u64,
    pub nodes: HashMap<u64, ArchivedCodeNode>,
    pub edges: Vec<ArchivedCodeEdge>,
    pub metadata: HashMap<String, String>,
    pub file_hashes: HashMap<String, String>,
}

/// Archived version of embedding vectors
#[derive(Archive, Serialize, Deserialize, Debug, Clone)]
pub struct ArchivedEmbedding {
    pub id: u64,
    pub model: String,
    pub dimensions: u32,
    pub vector: Vec<f32>,
    pub metadata: HashMap<String, String>,
}

/// Archived version of search results
#[derive(Archive, Serialize, Deserialize, Debug, Clone)]
pub struct ArchivedSearchResult {
    pub query_id: u64,
    pub results: Vec<ArchivedSearchItem>,
    pub total_count: usize,
    pub execution_time_ms: u64,
    pub metadata: HashMap<String, String>,
}

/// Individual search result item
#[derive(Archive, Serialize, Deserialize, Debug, Clone)]
pub struct ArchivedSearchItem {
    pub node_id: u64,
    pub score: f32,
    pub snippet: String,
    pub highlights: Vec<ArchivedHighlight>,
}

/// Text highlight in search results
#[derive(Archive, Serialize, Deserialize, Debug, Clone)]
pub struct ArchivedHighlight {
    pub start: usize,
    pub end: usize,
    pub highlight_type: String,
}

/// Archived cache entry with expiration
#[derive(Archive, Serialize, Deserialize, Debug, Clone)]
pub struct ArchivedCacheEntry<T> {
    pub key: String,
    pub value: T,
    pub created_at: u64,
    pub expires_at: Option<u64>,
    pub access_count: u64,
    pub last_accessed: u64,
}

/// Archived configuration settings
#[derive(Archive, Serialize, Deserialize, Debug, Clone)]
pub struct ArchivedConfig {
    pub version: String,
    pub database_path: String,
    pub cache_size: usize,
    pub max_file_size: usize,
    pub embedding_model: String,
    pub chunk_size: usize,
    pub overlap_size: usize,
    pub settings: HashMap<String, String>,
}

/// Archived metrics and statistics
#[derive(Archive, Serialize, Deserialize, Debug, Clone)]
pub struct ArchivedMetrics {
    pub timestamp: u64,
    pub processing_time: HashMap<String, u64>,
    pub memory_usage: HashMap<String, u64>,
    pub cache_stats: ArchivedCacheStats,
    pub error_counts: HashMap<String, u64>,
    pub counters: HashMap<String, u64>,
}

/// Cache statistics
#[derive(Archive, Serialize, Deserialize, Debug, Clone)]
pub struct ArchivedCacheStats {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
    pub size: usize,
    pub capacity: usize,
}

impl ArchivedCodeNode {
    /// Check if this node is a function
    pub fn is_function(&self) -> bool {
        self.node_type == "function" || self.node_type == "method"
    }
    
    /// Check if this node is a class or struct
    pub fn is_type_definition(&self) -> bool {
        matches!(self.node_type.as_str(), "class" | "struct" | "interface" | "enum")
    }
    
    /// Get the size in lines
    pub fn line_count(&self) -> u32 {
        self.end_line.saturating_sub(self.start_line).saturating_add(1)
    }
    
    /// Check if this node contains the given line
    pub fn contains_line(&self, line: u32) -> bool {
        line >= self.start_line && line <= self.end_line
    }
}

impl ArchivedCodeEdge {
    /// Check if this is a dependency relationship
    pub fn is_dependency(&self) -> bool {
        matches!(self.edge_type.as_str(), "depends_on" | "imports" | "requires")
    }
    
    /// Check if this is a structural relationship
    pub fn is_structural(&self) -> bool {
        matches!(self.edge_type.as_str(), "contains" | "parent_of" | "child_of")
    }
    
    /// Check if this is a call relationship
    pub fn is_call_relationship(&self) -> bool {
        matches!(self.edge_type.as_str(), "calls" | "invokes" | "references")
    }
}

impl ArchivedCodeGraph {
    /// Get a node by ID
    pub fn get_node(&self, id: u64) -> Option<&ArchivedCodeNode> {
        self.nodes.get(&id)
    }
    
    /// Get all edges from a source node
    pub fn get_edges_from(&self, source_id: u64) -> Vec<&ArchivedCodeEdge> {
        self.edges.iter()
            .filter(|edge| edge.source_id == source_id)
            .collect()
    }
    
    /// Get all edges to a target node
    pub fn get_edges_to(&self, target_id: u64) -> Vec<&ArchivedCodeEdge> {
        self.edges.iter()
            .filter(|edge| edge.target_id == target_id)
            .collect()
    }
    
    /// Get the total number of nodes
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }
    
    /// Get the total number of edges
    pub fn edge_count(&self) -> usize {
        self.edges.len()
    }
    
    /// Find nodes by type
    pub fn nodes_by_type(&self, node_type: &str) -> Vec<&ArchivedCodeNode> {
        self.nodes.values()
            .filter(|node| node.node_type == node_type)
            .collect()
    }
    
    /// Find nodes in a specific file
    pub fn nodes_in_file(&self, file_path: &str) -> Vec<&ArchivedCodeNode> {
        self.nodes.values()
            .filter(|node| node.file_path == file_path)
            .collect()
    }
}

impl<T> ArchivedCacheEntry<T> {
    /// Check if the cache entry has expired
    pub fn is_expired(&self, current_time: u64) -> bool {
        match self.expires_at {
            Some(expires_at) => current_time > expires_at,
            None => false,
        }
    }
    
    /// Get the age of the cache entry in milliseconds
    pub fn age(&self, current_time: u64) -> u64 {
        current_time.saturating_sub(self.created_at)
    }
    
    /// Get time since last access in milliseconds
    pub fn time_since_access(&self, current_time: u64) -> u64 {
        current_time.saturating_sub(self.last_accessed)
    }
}

impl ArchivedSearchResult {
    /// Get the top N results
    pub fn top_results(&self, n: usize) -> &[ArchivedSearchItem] {
        let end = n.min(self.results.len());
        &self.results[..end]
    }
    
    /// Check if there are more results than returned
    pub fn has_more_results(&self) -> bool {
        self.total_count > self.results.len()
    }
    
    /// Get results above a certain score threshold
    pub fn results_above_score(&self, threshold: f32) -> Vec<&ArchivedSearchItem> {
        self.results.iter()
            .filter(|item| item.score >= threshold)
            .collect()
    }
}

impl ArchivedMetrics {
    /// Get cache hit rate as percentage
    pub fn cache_hit_rate(&self) -> f64 {
        let total = self.cache_stats.hits + self.cache_stats.misses;
        if total == 0 {
            0.0
        } else {
            (self.cache_stats.hits as f64 / total as f64) * 100.0
        }
    }
    
    /// Get cache utilization as percentage
    pub fn cache_utilization(&self) -> f64 {
        if self.cache_stats.capacity == 0 {
            0.0
        } else {
            (self.cache_stats.size as f64 / self.cache_stats.capacity as f64) * 100.0
        }
    }
    
    /// Get total error count
    pub fn total_errors(&self) -> u64 {
        self.error_counts.values().sum()
    }
    
    /// Get average processing time for an operation
    pub fn average_processing_time(&self, operation: &str) -> Option<u64> {
        self.processing_time.get(operation).copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rkyv::{to_bytes, from_bytes};

    #[test]
    fn test_archived_code_node() {
        let node = ArchivedCodeNode {
            id: 1,
            name: "test_function".to_string(),
            node_type: "function".to_string(),
            file_path: "src/lib.rs".to_string(),
            start_line: 10,
            end_line: 20,
            start_column: 0,
            end_column: 10,
            content: "fn test() {}".to_string(),
            hash: "abc123".to_string(),
            parent_id: Some(2),
            children: vec![3, 4],
            metadata: HashMap::new(),
        };
        
        assert!(node.is_function());
        assert!(!node.is_type_definition());
        assert_eq!(node.line_count(), 11);
        assert!(node.contains_line(15));
        assert!(!node.contains_line(25));
        
        // Test serialization roundtrip
        let bytes = to_bytes::<rkyv::rancor::Error>(&node).unwrap();
        let archived = from_bytes::<ArchivedCodeNode, rkyv::rancor::Error>(&bytes).unwrap();
        
        assert_eq!(archived.id, 1);
        assert_eq!(archived.name, "test_function");
        assert_eq!(archived.node_type, "function");
    }

    #[test]
    fn test_archived_code_graph() {
        let mut nodes = HashMap::new();
        nodes.insert(1, ArchivedCodeNode {
            id: 1,
            name: "main".to_string(),
            node_type: "function".to_string(),
            file_path: "src/main.rs".to_string(),
            start_line: 1,
            end_line: 10,
            start_column: 0,
            end_column: 10,
            content: "fn main() {}".to_string(),
            hash: "hash1".to_string(),
            parent_id: None,
            children: vec![],
            metadata: HashMap::new(),
        });
        
        let edges = vec![ArchivedCodeEdge {
            id: 1,
            source_id: 1,
            target_id: 2,
            edge_type: "calls".to_string(),
            weight: 1.0,
            metadata: HashMap::new(),
        }];
        
        let graph = ArchivedCodeGraph {
            version: 1,
            timestamp: 1234567890,
            nodes,
            edges,
            metadata: HashMap::new(),
            file_hashes: HashMap::new(),
        };
        
        assert_eq!(graph.node_count(), 1);
        assert_eq!(graph.edge_count(), 1);
        
        let node = graph.get_node(1).unwrap();
        assert_eq!(node.name, "main");
        
        let functions = graph.nodes_by_type("function");
        assert_eq!(functions.len(), 1);
        
        // Test serialization
        let bytes = to_bytes::<rkyv::rancor::Error>(&graph).unwrap();
        let archived = from_bytes::<ArchivedCodeGraph, rkyv::rancor::Error>(&bytes).unwrap();
        
        assert_eq!(archived.version, 1);
        assert_eq!(archived.node_count(), 1);
    }

    #[test]
    fn test_archived_cache_entry() {
        let entry = ArchivedCacheEntry {
            key: "test_key".to_string(),
            value: "test_value".to_string(),
            created_at: 1000,
            expires_at: Some(2000),
            access_count: 5,
            last_accessed: 1500,
        };
        
        assert!(!entry.is_expired(1800));
        assert!(entry.is_expired(2100));
        assert_eq!(entry.age(1500), 500);
        assert_eq!(entry.time_since_access(1700), 200);
        
        // Test serialization
        let bytes = to_bytes::<rkyv::rancor::Error>(&entry).unwrap();
        let archived = from_bytes::<ArchivedCacheEntry<String>, rkyv::rancor::Error>(&bytes).unwrap();
        
        assert_eq!(archived.key, "test_key");
        assert_eq!(archived.value, "test_value");
    }

    #[test]
    fn test_archived_metrics() {
        let mut processing_time = HashMap::new();
        processing_time.insert("parse".to_string(), 100);
        processing_time.insert("index".to_string(), 200);
        
        let mut error_counts = HashMap::new();
        error_counts.insert("parse_error".to_string(), 5);
        error_counts.insert("io_error".to_string(), 2);
        
        let metrics = ArchivedMetrics {
            timestamp: 1234567890,
            processing_time,
            memory_usage: HashMap::new(),
            cache_stats: ArchivedCacheStats {
                hits: 80,
                misses: 20,
                evictions: 5,
                size: 100,
                capacity: 200,
            },
            error_counts,
            counters: HashMap::new(),
        };
        
        assert_eq!(metrics.cache_hit_rate(), 80.0);
        assert_eq!(metrics.cache_utilization(), 50.0);
        assert_eq!(metrics.total_errors(), 7);
        assert_eq!(metrics.average_processing_time("parse"), Some(100));
        
        // Test serialization
        let bytes = to_bytes::<rkyv::rancor::Error>(&metrics).unwrap();
        let archived = from_bytes::<ArchivedMetrics, rkyv::rancor::Error>(&bytes).unwrap();
        
        assert_eq!(archived.timestamp, 1234567890);
        assert_eq!(archived.cache_hit_rate(), 80.0);
    }
}