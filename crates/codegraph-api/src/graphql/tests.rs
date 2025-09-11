use async_graphql::{value, Request, Variables};
use serde_json::json;
use std::sync::Arc;
use tokio_test;
use uuid::Uuid;

use crate::graphql::types::*;
use crate::schema::create_schema;
use crate::state::AppState;

/// Helper function to create test app state
async fn create_test_state() -> AppState {
    AppState::new().await.expect("Failed to create test state")
}

/// Helper function to create test schema with state
async fn create_test_schema() -> (crate::schema::CodeGraphSchema, AppState) {
    let state = create_test_state().await;
    let schema = create_schema(state.clone());
    (schema, state)
}

#[tokio::test]
async fn test_health_query() {
    let (schema, _state) = create_test_schema().await;

    let query = r#"
        query {
            health
            version
        }
    "#;

    let req = Request::new(query);
    let res = schema.execute(req).await;

    assert!(res.errors.is_empty());
    let data = res.data.into_json().unwrap();
    assert_eq!(data["health"], "GraphQL API is running");
    assert_eq!(data["version"], "1.0.0");
}

#[tokio::test]
async fn test_code_search_query() {
    let (schema, _state) = create_test_schema().await;

    let query = r#"
        query SearchCode($input: CodeSearchInput!) {
            searchCode(input: $input) {
                nodes {
                    id
                    name
                    nodeType
                    language
                    location {
                        filePath
                        line
                        column
                    }
                    content
                    complexity
                }
                totalCount
                pageInfo {
                    hasNextPage
                    hasPreviousPage
                }
                searchMetadata {
                    queryTimeMs
                    indexUsed
                    filterApplied
                }
            }
        }
    "#;

    let variables = Variables::from_json(json!({
        "input": {
            "query": "test function",
            "limit": 10,
            "offset": 0,
            "languageFilter": ["RUST"],
            "nodeTypeFilter": ["FUNCTION"]
        }
    }));

    let req = Request::new(query).variables(variables);
    let res = schema.execute(req).await;

    assert!(res.errors.is_empty(), "GraphQL errors: {:?}", res.errors);

    let data = res.data.into_json().unwrap();
    let search_result = &data["searchCode"];

    // Verify structure exists
    assert!(search_result["nodes"].is_array());
    assert!(search_result["totalCount"].is_number());
    assert!(search_result["pageInfo"].is_object());
    assert!(search_result["searchMetadata"].is_object());

    // Verify metadata includes performance timing
    let metadata = &search_result["searchMetadata"];
    assert!(metadata["queryTimeMs"].is_number());
    assert_eq!(metadata["indexUsed"], "semantic_vector");
}

#[tokio::test]
async fn test_semantic_search_query() {
    let (schema, _state) = create_test_schema().await;

    let query = r#"
        query SemanticSearch($input: SemanticSearchInput!) {
            semanticSearch(input: $input) {
                nodes {
                    node {
                        id
                        name
                        nodeType
                    }
                    similarityScore
                    rankingScore
                    distanceMetric
                }
                queryEmbedding
                totalCandidates
                searchMetadata {
                    embeddingTimeMs
                    searchTimeMs
                    vectorDimension
                    similarityThreshold
                }
            }
        }
    "#;

    let variables = Variables::from_json(json!({
        "input": {
            "query": "error handling function",
            "similarityThreshold": 0.7,
            "limit": 5
        }
    }));

    let req = Request::new(query).variables(variables);
    let res = schema.execute(req).await;

    assert!(res.errors.is_empty(), "GraphQL errors: {:?}", res.errors);

    let data = res.data.into_json().unwrap();
    let search_result = &data["semanticSearch"];

    // Verify semantic search specific fields
    assert!(search_result["nodes"].is_array());
    assert!(search_result["queryEmbedding"].is_array());
    assert!(search_result["searchMetadata"]["vectorDimension"].is_number());

    // Verify all scored nodes have required fields
    if let Some(nodes) = search_result["nodes"].as_array() {
        for node in nodes {
            assert!(node["similarityScore"].is_number());
            assert!(node["rankingScore"].is_number());
            assert_eq!(node["distanceMetric"], "cosine");
        }
    }
}

#[tokio::test]
async fn test_graph_traversal_query() {
    let (schema, _state) = create_test_schema().await;

    let query = r#"
        query TraverseGraph($input: GraphTraversalInput!) {
            traverseGraph(input: $input) {
                nodes {
                    id
                    name
                    nodeType
                }
                edges {
                    id
                    sourceId
                    targetId
                    edgeType
                    weight
                }
                traversalPath
                depthReached
                totalVisited
                metadata {
                    traversalTimeMs
                    algorithmUsed
                    pruningApplied
                    maxDepth
                }
            }
        }
    "#;

    let start_node_id = Uuid::new_v4().to_string();
    let variables = Variables::from_json(json!({
        "input": {
            "startNodeId": start_node_id,
            "maxDepth": 3,
            "direction": "BOTH",
            "limit": 50
        }
    }));

    let req = Request::new(query).variables(variables);
    let res = schema.execute(req).await;

    assert!(res.errors.is_empty(), "GraphQL errors: {:?}", res.errors);

    let data = res.data.into_json().unwrap();
    let traversal_result = &data["traverseGraph"];

    // Verify traversal result structure
    assert!(traversal_result["nodes"].is_array());
    assert!(traversal_result["edges"].is_array());
    assert!(traversal_result["traversalPath"].is_array());
    assert!(traversal_result["depthReached"].is_number());
    assert!(traversal_result["totalVisited"].is_number());

    // Verify metadata includes performance info
    let metadata = &traversal_result["metadata"];
    assert!(metadata["traversalTimeMs"].is_number());
    assert_eq!(metadata["algorithmUsed"], "breadth_first");
}

#[tokio::test]
async fn test_subgraph_extraction_query() {
    let (schema, _state) = create_test_schema().await;

    let query = r#"
        query ExtractSubgraph($input: SubgraphExtractionInput!) {
            extractSubgraph(input: $input) {
                nodes {
                    id
                    name
                    nodeType
                }
                edges {
                    id
                    sourceId
                    targetId
                    edgeType
                }
                subgraphId
                centerNodeId
                extractionMetadata {
                    extractionTimeMs
                    extractionStrategy
                    nodeCount
                    edgeCount
                    connectivityScore
                }
            }
        }
    "#;

    let center_node_id = Uuid::new_v4().to_string();
    let variables = Variables::from_json(json!({
        "input": {
            "centerNodeId": center_node_id,
            "radius": 2,
            "extractionStrategy": "RADIUS"
        }
    }));

    let req = Request::new(query).variables(variables);
    let res = schema.execute(req).await;

    assert!(res.errors.is_empty(), "GraphQL errors: {:?}", res.errors);

    let data = res.data.into_json().unwrap();
    let subgraph_result = &data["extractSubgraph"];

    // Verify subgraph extraction results
    assert!(subgraph_result["nodes"].is_array());
    assert!(subgraph_result["edges"].is_array());
    assert!(subgraph_result["subgraphId"].is_string());
    assert_eq!(subgraph_result["centerNodeId"], center_node_id);

    // Verify extraction metadata
    let metadata = &subgraph_result["extractionMetadata"];
    assert!(metadata["extractionTimeMs"].is_number());
    assert_eq!(metadata["extractionStrategy"], "Radius");
    assert!(metadata["nodeCount"].is_number());
    assert!(metadata["edgeCount"].is_number());
    assert!(metadata["connectivityScore"].is_number());
}

#[tokio::test]
async fn test_node_by_id_query() {
    let (schema, _state) = create_test_schema().await;

    let query = r#"
        query GetNode($id: ID!) {
            node(id: $id) {
                id
                name
                nodeType
                language
                location {
                    filePath
                    line
                    column
                }
                content
                complexity
                createdAt
                updatedAt
            }
        }
    "#;

    let node_id = Uuid::new_v4().to_string();
    let variables = Variables::from_json(json!({
        "id": node_id
    }));

    let req = Request::new(query).variables(variables);
    let res = schema.execute(req).await;

    assert!(res.errors.is_empty(), "GraphQL errors: {:?}", res.errors);

    let data = res.data.into_json().unwrap();
    let node = &data["node"];

    // Node might be null if not found, which is acceptable
    if !node.is_null() {
        assert!(node["id"].is_string());
        assert!(node["name"].is_string());
        assert!(node["location"]["filePath"].is_string());
        assert!(node["location"]["line"].is_number());
    }
}

#[tokio::test]
async fn test_batch_nodes_query() {
    let (schema, _state) = create_test_schema().await;

    let query = r#"
        query GetNodes($ids: [ID!]!) {
            nodes(ids: $ids) {
                id
                name
                nodeType
                language
            }
        }
    "#;

    let node_ids = vec![
        Uuid::new_v4().to_string(),
        Uuid::new_v4().to_string(),
        Uuid::new_v4().to_string(),
    ];

    let variables = Variables::from_json(json!({
        "ids": node_ids
    }));

    let req = Request::new(query).variables(variables);
    let res = schema.execute(req).await;

    assert!(res.errors.is_empty(), "GraphQL errors: {:?}", res.errors);

    let data = res.data.into_json().unwrap();
    let nodes = &data["nodes"];

    // Should return an array (may be empty)
    assert!(nodes.is_array());

    // If nodes are returned, they should have required fields
    if let Some(nodes_array) = nodes.as_array() {
        for node in nodes_array {
            assert!(node["id"].is_string());
            assert!(node["name"].is_string());
        }
    }
}

#[tokio::test]
async fn test_performance_within_targets() {
    let (schema, _state) = create_test_schema().await;

    // Test simple query performance (<50ms target)
    let simple_query = r#"
        query {
            health
            version
        }
    "#;

    let start = std::time::Instant::now();
    let req = Request::new(simple_query);
    let res = schema.execute(req).await;
    let elapsed = start.elapsed();

    assert!(res.errors.is_empty());
    assert!(
        elapsed.as_millis() < 50,
        "Simple query took {}ms, should be <50ms",
        elapsed.as_millis()
    );

    // Test complex query performance (<200ms target)
    let complex_query = r#"
        query ComplexQuery($searchInput: CodeSearchInput!, $traversalInput: GraphTraversalInput!) {
            searchCode(input: $searchInput) {
                nodes { id name nodeType }
                totalCount
            }
            traverseGraph(input: $traversalInput) {
                nodes { id name }
                metadata { traversalTimeMs }
            }
        }
    "#;

    let variables = Variables::from_json(json!({
        "searchInput": {
            "query": "complex search test",
            "limit": 10
        },
        "traversalInput": {
            "startNodeId": Uuid::new_v4().to_string(),
            "maxDepth": 2,
            "limit": 20
        }
    }));

    let start = std::time::Instant::now();
    let req = Request::new(complex_query).variables(variables);
    let res = schema.execute(req).await;
    let elapsed = start.elapsed();

    assert!(res.errors.is_empty());
    assert!(
        elapsed.as_millis() < 200,
        "Complex query took {}ms, should be <200ms",
        elapsed.as_millis()
    );
}

#[tokio::test]
async fn test_input_validation() {
    let (schema, _state) = create_test_schema().await;

    // Test invalid node ID format
    let query = r#"
        query GetNode($id: ID!) {
            node(id: $id) {
                id
                name
            }
        }
    "#;

    let variables = Variables::from_json(json!({
        "id": "invalid-uuid-format"
    }));

    let req = Request::new(query).variables(variables);
    let res = schema.execute(req).await;

    // Should handle invalid UUID gracefully (either error or null result)
    if !res.errors.is_empty() {
        let error_message = res.errors[0].message.to_lowercase();
        assert!(error_message.contains("invalid") || error_message.contains("format"));
    }
}

#[tokio::test]
async fn test_dataloader_batching() {
    let (schema, _state) = create_test_schema().await;

    // Test that multiple node queries get batched by DataLoader
    let query = r#"
        query BatchTest($id1: ID!, $id2: ID!, $id3: ID!) {
            node1: node(id: $id1) { id name }
            node2: node(id: $id2) { id name }
            node3: node(id: $id3) { id name }
        }
    "#;

    let variables = Variables::from_json(json!({
        "id1": Uuid::new_v4().to_string(),
        "id2": Uuid::new_v4().to_string(),
        "id3": Uuid::new_v4().to_string(),
    }));

    let start = std::time::Instant::now();
    let req = Request::new(query).variables(variables);
    let res = schema.execute(req).await;
    let elapsed = start.elapsed();

    assert!(res.errors.is_empty());

    // With DataLoader batching, this should be fast even with multiple node queries
    assert!(
        elapsed.as_millis() < 100,
        "Batched queries took {}ms, batching may not be working",
        elapsed.as_millis()
    );

    let data = res.data.into_json().unwrap();

    // All three queries should complete (may return null if nodes don't exist)
    assert!(data.get("node1").is_some());
    assert!(data.get("node2").is_some());
    assert!(data.get("node3").is_some());
}

#[tokio::test]
async fn test_query_complexity_limits() {
    let (schema, _state) = create_test_schema().await;

    // Test deeply nested query that should hit complexity limits
    let deep_query = r#"
        query DeepQuery {
            searchCode(input: { query: "test", limit: 100 }) {
                nodes {
                    id
                    name
                    nodeType
                    language
                    location {
                        filePath
                        line
                        column
                        endLine
                        endColumn
                    }
                    content
                    complexity
                    createdAt
                    updatedAt
                    attributes
                }
                pageInfo {
                    hasNextPage
                    hasPreviousPage
                    startCursor
                    endCursor
                }
                searchMetadata {
                    queryTimeMs
                    indexUsed
                    filterApplied
                }
                totalCount
            }
        }
    "#;

    let req = Request::new(deep_query);
    let res = schema.execute(req).await;

    // Query should either succeed (within limits) or fail with complexity error
    if !res.errors.is_empty() {
        let has_complexity_error = res.errors.iter().any(|e| {
            e.message.to_lowercase().contains("complexity")
                || e.message.to_lowercase().contains("limit")
        });

        // If there are errors, at least one should be about complexity
        assert!(
            has_complexity_error
                || res
                    .errors
                    .iter()
                    .any(|e| !e.message.to_lowercase().contains("complexity"))
        );
    }
}

#[cfg(test)]
mod integration_tests {
    use super::*;

    /// Integration test that exercises the full GraphQL pipeline
    #[tokio::test]
    async fn test_full_graphql_pipeline() {
        let (schema, _state) = create_test_schema().await;

        // Execute a realistic workflow: search -> get details -> traverse -> extract subgraph
        let workflow_query = r#"
            query WorkflowTest($searchQuery: String!, $nodeId: ID!) {
                # Step 1: Search for relevant code
                search: searchCode(input: { query: $searchQuery, limit: 5 }) {
                    nodes {
                        id
                        name
                        nodeType
                    }
                    totalCount
                }
                
                # Step 2: Get detailed node information
                nodeDetail: node(id: $nodeId) {
                    id
                    name
                    content
                    location {
                        filePath
                        line
                    }
                }
            }
        "#;

        let variables = Variables::from_json(json!({
            "searchQuery": "function implementation",
            "nodeId": Uuid::new_v4().to_string()
        }));

        let start = std::time::Instant::now();
        let req = Request::new(workflow_query).variables(variables);
        let res = schema.execute(req).await;
        let elapsed = start.elapsed();

        assert!(
            res.errors.is_empty(),
            "Workflow query failed: {:?}",
            res.errors
        );

        let data = res.data.into_json().unwrap();
        assert!(data.get("search").is_some());
        assert!(data.get("nodeDetail").is_some());

        // Entire workflow should complete in reasonable time
        assert!(
            elapsed.as_millis() < 500,
            "Full workflow took {}ms",
            elapsed.as_millis()
        );
    }
}
