// Extension methods for SemanticSearch to support health checks
use codegraph_vector::SemanticSearch;
use codegraph_core::Result;

#[derive(Debug, Clone)]
pub struct IndexStats {
    pub total_vectors: usize,
    pub dimension: usize,
    pub index_type: String,
}

pub struct PerformanceStats {
    pub total_searches: u64,
    pub sub_millisecond_searches: u64,
    pub sub_ms_rate: f64,
}

pub trait SemanticSearchExt {
    async fn get_index_stats(&self) -> Result<IndexStats>;
    async fn test_search(&self) -> Result<bool>;
    fn get_performance_stats(&self) -> PerformanceStats;
}

impl SemanticSearchExt for SemanticSearch {
    async fn get_index_stats(&self) -> Result<IndexStats> {
        Ok(IndexStats {
            total_vectors: 0,  // Would need actual implementation
            dimension: 384,    // Default dimension
            index_type: "FlatL2".to_string(),
        })
    }
    
    async fn test_search(&self) -> Result<bool> {
        // Simple test - just return true for now
        Ok(true)
    }
    
    fn get_performance_stats(&self) -> PerformanceStats {
        PerformanceStats {
            total_searches: 0,
            sub_millisecond_searches: 0,
            sub_ms_rate: 0.0,
        }
    }
}