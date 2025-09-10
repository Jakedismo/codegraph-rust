use super::*;
use crate::{AppState, ApiError};
use axum::{
    body::Body,
    extract::{Query, State},
    http::{Request, StatusCode},
    Router,
};
use axum_test::TestServer;
use serde_json;
use tokio::time::{timeout, Duration};
use std::collections::HashMap;

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_stream_search_results_basic() {
        let state = create_test_app_state().await;
        
        let params = StreamQuery {
            query: "test".to_string(),
            limit: Some(10),
            batch_size: Some(5),
            throttle_ms: Some(1),
        };

        let result = stream_search_results(State(state), Query(params)).await;
        assert!(result.is_ok(), "Stream search should succeed");
    }

    #[tokio::test]
    async fn test_stream_large_dataset() {
        let state = create_test_app_state().await;
        
        let mut params = HashMap::new();
        params.insert("batch_size".to_string(), "10".to_string());
        params.insert("throttle_ms".to_string(), "1".to_string());

        let result = stream_large_dataset(State(state), Query(params)).await;
        assert!(result.is_ok(), "Large dataset stream should succeed");
    }

    #[tokio::test]
    async fn test_stream_csv_results() {
        let state = create_test_app_state().await;
        
        let params = StreamQuery {
            query: "test".to_string(),
            limit: Some(5),
            batch_size: Some(2),
            throttle_ms: Some(1),
        };

        let result = stream_csv_results(State(state), Query(params)).await;
        assert!(result.is_ok(), "CSV stream should succeed");
    }

    #[tokio::test]
    async fn test_get_stream_metadata() {
        let state = create_test_app_state().await;
        let stream_id = "test-stream-123".to_string();

        let result = get_stream_metadata(State(state), Path(stream_id.clone())).await;
        assert!(result.is_ok(), "Stream metadata should be retrievable");
        
        let metadata = result.unwrap().0;
        assert_eq!(metadata.stream_id, stream_id);
        assert!(metadata.total_results > 0);
        assert!(metadata.batch_size > 0);
    }

    #[tokio::test]
    async fn test_get_flow_control_stats() {
        let state = create_test_app_state().await;

        let result = get_flow_control_stats(State(state)).await;
        assert!(result.is_ok(), "Flow control stats should be retrievable");
        
        let stats = result.unwrap().0;
        assert!(stats.active_streams < 1000); // reasonable upper bound
    }

    #[tokio::test]
    async fn test_backpressure_stream_creation() {
        let mock_results = vec![
            codegraph_vector::SearchResult {
                node_id: uuid::Uuid::new_v4(),
                score: 0.9,
            },
            codegraph_vector::SearchResult {
                node_id: uuid::Uuid::new_v4(),
                score: 0.8,
            },
        ];

        // Create a minimal mock graph read guard
        let graph = create_mock_graph_guard();
        
        let stream = create_backpressure_stream(
            mock_results,
            graph,
            2,
            Duration::from_millis(1),
            "test-stream".to_string(),
        );

        // Test that we can collect from the stream without hanging
        let timeout_duration = Duration::from_secs(5);
        let items: Result<Vec<_>, _> = timeout(timeout_duration, stream.collect()).await;
        
        assert!(items.is_ok(), "Stream should complete within timeout");
        let collected_items = items.unwrap();
        assert_eq!(collected_items.len(), 2, "Should get all items from stream");
    }

    #[tokio::test]
    async fn test_large_dataset_stream_performance() {
        let batch_size = 100;
        let throttle_ms = 1;
        
        let stream = create_large_dataset_stream(batch_size, throttle_ms);
        
        let start = std::time::Instant::now();
        let timeout_duration = Duration::from_secs(10);
        
        // Take only first 500 items to test performance
        let items: Result<Vec<_>, _> = timeout(
            timeout_duration,
            stream.take(500).collect()
        ).await;
        
        let duration = start.elapsed();
        
        assert!(items.is_ok(), "Stream should complete within timeout");
        let collected_items = items.unwrap();
        assert_eq!(collected_items.len(), 500, "Should get exactly 500 items");
        
        // Performance assertion: should complete in reasonable time
        assert!(duration.as_secs() < 5, "Stream should be reasonably fast");
    }

    #[tokio::test]
    async fn test_stream_error_handling() {
        // Test with invalid parameters
        let state = create_test_app_state().await;
        
        let params = StreamQuery {
            query: "".to_string(), // Empty query
            limit: Some(0),
            batch_size: Some(0),
            throttle_ms: Some(0),
        };

        let result = stream_search_results(State(state), Query(params)).await;
        // Should handle gracefully (in this case, it will work with empty results)
        assert!(result.is_ok(), "Should handle edge cases gracefully");
    }

    #[tokio::test] 
    async fn test_csv_stream_format() {
        let mock_results = vec![
            codegraph_vector::SearchResult {
                node_id: uuid::Uuid::new_v4(),
                score: 0.9,
            }
        ];

        let stream = create_csv_stream(mock_results, 1);
        let items: Vec<_> = stream.collect().await;
        
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].node_type, "CsvData");
        assert!(items[0].file_path.contains(".csv"));
    }

    // Helper functions for testing

    async fn create_test_app_state() -> AppState {
        // This would need to be implemented based on your actual AppState structure
        // For now, we'll create a minimal mock
        AppState::new_for_testing().await
    }

    fn create_mock_graph_guard() -> tokio::sync::RwLockReadGuard<'static, Box<dyn codegraph_core::GraphStore + Send + Sync>> {
        // This is a simplified mock - in real tests you'd create a proper mock
        use std::sync::Arc;
        use tokio::sync::RwLock;
        
        // Create a static reference for testing
        static MOCK_GRAPH: std::sync::OnceLock<Arc<RwLock<Box<dyn codegraph_core::GraphStore + Send + Sync>>>> = std::sync::OnceLock::new();
        
        let graph = MOCK_GRAPH.get_or_init(|| {
            Arc::new(RwLock::new(Box::new(MockGraphStore) as Box<dyn codegraph_core::GraphStore + Send + Sync>))
        });
        
        // This is unsafe in real code - using for test simplification
        unsafe {
            std::mem::transmute(graph.blocking_read())
        }
    }

    // Mock GraphStore implementation for testing
    struct MockGraphStore;

    #[async_trait::async_trait]
    impl codegraph_core::GraphStore for MockGraphStore {
        async fn add_node(&mut self, _node: codegraph_core::Node) -> Result<(), codegraph_core::CodeGraphError> {
            Ok(())
        }

        async fn get_node(&self, _id: codegraph_core::NodeId) -> Result<Option<codegraph_core::Node>, codegraph_core::CodeGraphError> {
            Ok(None)
        }

        async fn update_node(&mut self, _node: codegraph_core::Node) -> Result<(), codegraph_core::CodeGraphError> {
            Ok(())
        }

        async fn delete_node(&mut self, _id: codegraph_core::NodeId) -> Result<bool, codegraph_core::CodeGraphError> {
            Ok(false)
        }

        async fn get_nodes_by_file(&self, _file_path: &str) -> Result<Vec<codegraph_core::Node>, codegraph_core::CodeGraphError> {
            Ok(vec![])
        }

        async fn get_all_nodes(&self) -> Result<Vec<codegraph_core::Node>, codegraph_core::CodeGraphError> {
            Ok(vec![])
        }

        async fn clear(&mut self) -> Result<(), codegraph_core::CodeGraphError> {
            Ok(())
        }
    }
}

// Integration tests
#[cfg(test)]
mod integration_tests {
    use super::*;
    use axum_test::TestServer;

    #[tokio::test]
    async fn test_streaming_endpoints_integration() {
        let app = create_test_router().await;
        let server = TestServer::new(app).unwrap();

        // Test streaming search endpoint
        let response = server
            .get("/stream/search")
            .add_query_param("query", "test")
            .add_query_param("limit", "10")
            .add_query_param("batch_size", "5")
            .await;

        assert_eq!(response.status_code(), StatusCode::OK);
        
        // Test that response has appropriate content-type for streaming
        let content_type = response.headers().get("content-type").unwrap();
        assert!(content_type.to_str().unwrap().contains("application/json"));
    }

    #[tokio::test]
    async fn test_large_dataset_endpoint() {
        let app = create_test_router().await;
        let server = TestServer::new(app).unwrap();

        let response = server
            .get("/stream/dataset")
            .add_query_param("batch_size", "10")
            .add_query_param("throttle_ms", "1")
            .await;

        assert_eq!(response.status_code(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_csv_streaming_endpoint() {
        let app = create_test_router().await;
        let server = TestServer::new(app).unwrap();

        let response = server
            .get("/stream/csv")
            .add_query_param("query", "test")
            .add_query_param("limit", "5")
            .await;

        assert_eq!(response.status_code(), StatusCode::OK);
        
        // CSV endpoints should return CSV content type
        let content_type = response.headers().get("content-type").unwrap();
        assert!(content_type.to_str().unwrap().contains("text/csv"));
    }

    #[tokio::test]
    async fn test_stream_metadata_endpoint() {
        let app = create_test_router().await;
        let server = TestServer::new(app).unwrap();

        let response = server
            .get("/stream/test-stream-123/metadata")
            .await;

        assert_eq!(response.status_code(), StatusCode::OK);
        
        let metadata: StreamingMetadata = response.json();
        assert_eq!(metadata.stream_id, "test-stream-123");
    }

    #[tokio::test]
    async fn test_flow_control_stats_endpoint() {
        let app = create_test_router().await;
        let server = TestServer::new(app).unwrap();

        let response = server
            .get("/stream/stats")
            .await;

        assert_eq!(response.status_code(), StatusCode::OK);
        
        let stats: FlowControlStats = response.json();
        assert!(stats.active_streams < 1000);
    }

    async fn create_test_router() -> Router {
        let state = AppState::new_for_testing().await;
        
        Router::new()
            .route("/stream/search", axum::routing::get(stream_search_results))
            .route("/stream/dataset", axum::routing::get(stream_large_dataset))
            .route("/stream/csv", axum::routing::get(stream_csv_results))
            .route("/stream/:id/metadata", axum::routing::get(get_stream_metadata))
            .route("/stream/stats", axum::routing::get(get_flow_control_stats))
            .with_state(state)
    }
}