use async_graphql::{Request, Variables};
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;
use tokio::runtime::Runtime;
use uuid::Uuid;

use crate::schema::create_schema;
use crate::state::AppState;

/// Benchmark configuration
struct BenchConfig {
    pub simple_query_target_ms: u64,  // 50ms target
    pub complex_query_target_ms: u64, // 200ms target
    pub batch_size_variants: Vec<usize>,
    pub depth_variants: Vec<i32>,
    pub dataset_sizes: Vec<usize>,
}

impl Default for BenchConfig {
    fn default() -> Self {
        Self {
            simple_query_target_ms: 50,
            complex_query_target_ms: 200,
            batch_size_variants: vec![1, 5, 10, 20, 50, 100],
            depth_variants: vec![1, 2, 3, 5, 8],
            dataset_sizes: vec![100, 500, 1000, 5000],
        }
    }
}

/// Create test runtime and schema for benchmarks
async fn create_bench_setup() -> (crate::schema::CodeGraphSchema, AppState) {
    let state = AppState::new().await.expect("Failed to create test state");
    let schema = create_schema(state.clone());
    (schema, state)
}

/// Benchmark simple health queries (target: <50ms)
fn bench_simple_queries(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let (schema, _state) = rt.block_on(create_bench_setup());

    let mut group = c.benchmark_group("simple_queries");
    group.measurement_time(Duration::from_secs(10));

    // Health query benchmark
    group.bench_function("health_query", |b| {
        let query = r#"
            query {
                health
                version
            }
        "#;

        b.to_async(&rt).iter(|| async {
            let req = Request::new(query);
            let res = schema.execute(black_box(req)).await;
            black_box(res)
        })
    });

    // Single node query benchmark
    group.bench_function("single_node_query", |b| {
        let query = r#"
            query GetNode($id: ID!) {
                node(id: $id) {
                    id
                    name
                    nodeType
                }
            }
        "#;

        b.to_async(&rt).iter(|| async {
            let variables = Variables::from_json(json!({
                "id": Uuid::new_v4().to_string()
            }));

            let req = Request::new(query).variables(variables);
            let res = schema.execute(black_box(req)).await;
            black_box(res)
        })
    });

    group.finish();
}

/// Benchmark code search queries with different parameters
fn bench_code_search(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let (schema, _state) = rt.block_on(create_bench_setup());
    let config = BenchConfig::default();

    let mut group = c.benchmark_group("code_search");
    group.measurement_time(Duration::from_secs(15));

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
                }
            }
        }
    "#;

    // Benchmark different result limits
    for limit in &config.batch_size_variants {
        group.bench_with_input(
            BenchmarkId::new("search_by_limit", limit),
            limit,
            |b, &limit| {
                b.to_async(&rt).iter(|| async {
                    let variables = Variables::from_json(json!({
                        "input": {
                            "query": "function test implementation",
                            "limit": limit,
                            "offset": 0,
                            "languageFilter": ["RUST"],
                            "nodeTypeFilter": ["FUNCTION"]
                        }
                    }));

                    let req = Request::new(query).variables(variables);
                    let res = schema.execute(black_box(req)).await;
                    black_box(res)
                })
            },
        );
    }

    // Benchmark different query complexities
    let search_queries = vec![
        ("simple", "test"),
        ("medium", "function implementation error handling"),
        (
            "complex",
            "async function with error handling and logging implementation pattern",
        ),
    ];

    for (complexity, search_query) in search_queries {
        group.bench_with_input(
            BenchmarkId::new("search_by_complexity", complexity),
            search_query,
            |b, &search_query| {
                b.to_async(&rt).iter(|| async {
                    let variables = Variables::from_json(json!({
                        "input": {
                            "query": search_query,
                            "limit": 20,
                            "offset": 0
                        }
                    }));

                    let req = Request::new(query).variables(variables);
                    let res = schema.execute(black_box(req)).await;
                    black_box(res)
                })
            },
        );
    }

    group.finish();
}

/// Benchmark semantic search performance
fn bench_semantic_search(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let (schema, _state) = rt.block_on(create_bench_setup());
    let config = BenchConfig::default();

    let mut group = c.benchmark_group("semantic_search");
    group.measurement_time(Duration::from_secs(20));

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
                }
                queryEmbedding
                searchMetadata {
                    embeddingTimeMs
                    searchTimeMs
                    vectorDimension
                }
            }
        }
    "#;

    // Benchmark different similarity thresholds
    let thresholds = vec![0.5, 0.7, 0.8, 0.9];
    for threshold in thresholds {
        group.bench_with_input(
            BenchmarkId::new("semantic_by_threshold", (threshold * 100.0) as i32),
            &threshold,
            |b, &threshold| {
                b.to_async(&rt).iter(|| async {
                    let variables = Variables::from_json(json!({
                        "input": {
                            "query": "error handling and logging pattern",
                            "similarityThreshold": threshold,
                            "limit": 10
                        }
                    }));

                    let req = Request::new(query).variables(variables);
                    let res = schema.execute(black_box(req)).await;
                    black_box(res)
                })
            },
        );
    }

    // Benchmark different result limits for semantic search
    for limit in &[5, 10, 20, 50] {
        group.bench_with_input(
            BenchmarkId::new("semantic_by_limit", limit),
            limit,
            |b, &limit| {
                b.to_async(&rt).iter(|| async {
                    let variables = Variables::from_json(json!({
                        "input": {
                            "query": "database connection pool management",
                            "similarityThreshold": 0.7,
                            "limit": limit
                        }
                    }));

                    let req = Request::new(query).variables(variables);
                    let res = schema.execute(black_box(req)).await;
                    black_box(res)
                })
            },
        );
    }

    group.finish();
}

/// Benchmark graph traversal with different depths and strategies
fn bench_graph_traversal(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let (schema, _state) = rt.block_on(create_bench_setup());
    let config = BenchConfig::default();

    let mut group = c.benchmark_group("graph_traversal");
    group.measurement_time(Duration::from_secs(25));

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
                }
                traversalPath
                depthReached
                metadata {
                    traversalTimeMs
                    algorithmUsed
                }
            }
        }
    "#;

    // Benchmark different traversal depths
    for depth in &config.depth_variants {
        group.bench_with_input(
            BenchmarkId::new("traversal_by_depth", depth),
            depth,
            |b, &depth| {
                b.to_async(&rt).iter(|| async {
                    let variables = Variables::from_json(json!({
                        "input": {
                            "startNodeId": Uuid::new_v4().to_string(),
                            "maxDepth": depth,
                            "direction": "BOTH",
                            "limit": 100
                        }
                    }));

                    let req = Request::new(query).variables(variables);
                    let res = schema.execute(black_box(req)).await;
                    black_box(res)
                })
            },
        );
    }

    // Benchmark different traversal directions
    let directions = vec!["OUTGOING", "INCOMING", "BOTH"];
    for direction in directions {
        group.bench_with_input(
            BenchmarkId::new("traversal_by_direction", direction),
            &direction,
            |b, &direction| {
                b.to_async(&rt).iter(|| async {
                    let variables = Variables::from_json(json!({
                        "input": {
                            "startNodeId": Uuid::new_v4().to_string(),
                            "maxDepth": 3,
                            "direction": direction,
                            "limit": 50
                        }
                    }));

                    let req = Request::new(query).variables(variables);
                    let res = schema.execute(black_box(req)).await;
                    black_box(res)
                })
            },
        );
    }

    group.finish();
}

/// Benchmark subgraph extraction with different radii
fn bench_subgraph_extraction(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let (schema, _state) = rt.block_on(create_bench_setup());

    let mut group = c.benchmark_group("subgraph_extraction");
    group.measurement_time(Duration::from_secs(20));

    let query = r#"
        query ExtractSubgraph($input: SubgraphExtractionInput!) {
            extractSubgraph(input: $input) {
                nodes {
                    id
                    name
                }
                edges {
                    id
                    sourceId
                    targetId
                }
                extractionMetadata {
                    extractionTimeMs
                    nodeCount
                    edgeCount
                    connectivityScore
                }
            }
        }
    "#;

    // Benchmark different extraction radii
    let radii = vec![1, 2, 3, 4, 5];
    for radius in radii {
        group.bench_with_input(
            BenchmarkId::new("extraction_by_radius", radius),
            &radius,
            |b, &radius| {
                b.to_async(&rt).iter(|| async {
                    let variables = Variables::from_json(json!({
                        "input": {
                            "centerNodeId": Uuid::new_v4().to_string(),
                            "radius": radius,
                            "extractionStrategy": "RADIUS"
                        }
                    }));

                    let req = Request::new(query).variables(variables);
                    let res = schema.execute(black_box(req)).await;
                    black_box(res)
                })
            },
        );
    }

    // Benchmark different extraction strategies
    let strategies = vec!["RADIUS", "CONNECTED", "SEMANTIC", "DEPENDENCY"];
    for strategy in strategies {
        group.bench_with_input(
            BenchmarkId::new("extraction_by_strategy", strategy),
            &strategy,
            |b, &strategy| {
                b.to_async(&rt).iter(|| async {
                    let variables = Variables::from_json(json!({
                        "input": {
                            "centerNodeId": Uuid::new_v4().to_string(),
                            "radius": 2,
                            "extractionStrategy": strategy
                        }
                    }));

                    let req = Request::new(query).variables(variables);
                    let res = schema.execute(black_box(req)).await;
                    black_box(res)
                })
            },
        );
    }

    group.finish();
}

/// Benchmark DataLoader batching efficiency
fn bench_dataloader_batching(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let (schema, _state) = rt.block_on(create_bench_setup());
    let config = BenchConfig::default();

    let mut group = c.benchmark_group("dataloader_batching");
    group.measurement_time(Duration::from_secs(15));

    // Benchmark batch node loading
    for batch_size in &config.batch_size_variants {
        let mut query_parts = vec!["query BatchTest(".to_string()];
        let mut query_body_parts = vec![];
        let mut variables = serde_json::Map::new();

        // Generate parameterized query for different batch sizes
        for i in 0..*batch_size {
            query_parts.push(format!("$id{}: ID!", i));
            query_body_parts.push(format!("node{}: node(id: $id{}) {{ id name }}", i, i));
            variables.insert(format!("id{}", i), json!(Uuid::new_v4().to_string()));

            if i < batch_size - 1 {
                query_parts.push(", ".to_string());
            }
        }

        let query = format!(
            "{}) {{ {} }}",
            query_parts.join(""),
            query_body_parts.join(" ")
        );

        group.bench_with_input(
            BenchmarkId::new("batch_nodes", batch_size),
            &(query.clone(), variables.clone()),
            |b, (query, variables)| {
                b.to_async(&rt).iter(|| async {
                    let vars = Variables::from_json(json!(variables));
                    let req = Request::new(query).variables(vars);
                    let res = schema.execute(black_box(req)).await;
                    black_box(res)
                })
            },
        );
    }

    // Benchmark sequential vs batched queries
    group.bench_function("sequential_queries", |b| {
        b.to_async(&rt).iter(|| async {
            let mut results = Vec::new();
            for _ in 0..10 {
                let query = r#"query($id: ID!) { node(id: $id) { id name } }"#;
                let variables = Variables::from_json(json!({
                    "id": Uuid::new_v4().to_string()
                }));

                let req = Request::new(query).variables(variables);
                let res = schema.execute(req).await;
                results.push(res);
            }
            black_box(results)
        })
    });

    group.finish();
}

/// Benchmark complex multi-operation queries
fn bench_complex_queries(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let (schema, _state) = rt.block_on(create_bench_setup());

    let mut group = c.benchmark_group("complex_queries");
    group.measurement_time(Duration::from_secs(30));

    // Complex query combining multiple operations
    let complex_query = r#"
        query ComplexWorkflow($searchQuery: String!, $nodeId: ID!, $traversalInput: GraphTraversalInput!) {
            # Multi-operation query testing resolver coordination
            search: searchCode(input: { 
                query: $searchQuery, 
                limit: 20,
                languageFilter: [RUST, PYTHON],
                nodeTypeFilter: [FUNCTION, CLASS]
            }) {
                nodes {
                    id
                    name
                    nodeType
                    complexity
                }
                totalCount
                searchMetadata {
                    queryTimeMs
                }
            }
            
            nodeDetail: node(id: $nodeId) {
                id
                name
                content
                location {
                    filePath
                    line
                }
            }
            
            traversal: traverseGraph(input: $traversalInput) {
                nodes {
                    id
                    name
                }
                metadata {
                    traversalTimeMs
                }
            }
            
            semantic: semanticSearch(input: {
                query: $searchQuery,
                limit: 10,
                similarityThreshold: 0.8
            }) {
                nodes {
                    similarityScore
                    node {
                        id
                        name
                    }
                }
            }
        }
    "#;

    group.bench_function("multi_operation_workflow", |b| {
        b.to_async(&rt).iter(|| async {
            let variables = Variables::from_json(json!({
                "searchQuery": "async function error handling database transaction",
                "nodeId": Uuid::new_v4().to_string(),
                "traversalInput": {
                    "startNodeId": Uuid::new_v4().to_string(),
                    "maxDepth": 2,
                    "direction": "BOTH",
                    "limit": 30
                }
            }));

            let req = Request::new(complex_query).variables(variables);
            let res = schema.execute(black_box(req)).await;
            black_box(res)
        })
    });

    // Deeply nested query
    let nested_query = r#"
        query DeepNested($searchQuery: String!) {
            searchCode(input: { query: $searchQuery, limit: 5 }) {
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

    group.bench_function("deeply_nested", |b| {
        b.to_async(&rt).iter(|| async {
            let variables = Variables::from_json(json!({
                "searchQuery": "complex nested data structure operations"
            }));

            let req = Request::new(nested_query).variables(variables);
            let res = schema.execute(black_box(req)).await;
            black_box(res)
        })
    });

    group.finish();
}

/// Benchmark query performance under different loads
fn bench_concurrent_queries(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let (schema, _state) = rt.block_on(create_bench_setup());
    let schema = Arc::new(schema);

    let mut group = c.benchmark_group("concurrent_queries");
    group.measurement_time(Duration::from_secs(20));

    let query = r#"
        query ConcurrentTest($input: CodeSearchInput!) {
            searchCode(input: $input) {
                nodes { id name nodeType }
                totalCount
            }
        }
    "#;

    // Test concurrent query execution
    let concurrency_levels = vec![1, 2, 4, 8, 16];
    for concurrency in concurrency_levels {
        group.bench_with_input(
            BenchmarkId::new("concurrent_search", concurrency),
            &concurrency,
            |b, &concurrency| {
                b.to_async(&rt).iter(|| async {
                    let schema = schema.clone();
                    let tasks: Vec<_> = (0..concurrency)
                        .map(|i| {
                            let schema = schema.clone();
                            tokio::spawn(async move {
                                let variables = Variables::from_json(json!({
                                    "input": {
                                        "query": format!("concurrent test query {}", i),
                                        "limit": 10
                                    }
                                }));

                                let req = Request::new(query).variables(variables);
                                schema.execute(req).await
                            })
                        })
                        .collect();

                    let results = futures::future::join_all(tasks).await;
                    black_box(results)
                })
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_simple_queries,
    bench_code_search,
    bench_semantic_search,
    bench_graph_traversal,
    bench_subgraph_extraction,
    bench_dataloader_batching,
    bench_complex_queries,
    bench_concurrent_queries
);

criterion_main!(benches);

#[cfg(test)]
mod benchmark_tests {
    use super::*;

    #[test]
    fn test_benchmark_config() {
        let config = BenchConfig::default();
        assert_eq!(config.simple_query_target_ms, 50);
        assert_eq!(config.complex_query_target_ms, 200);
        assert!(!config.batch_size_variants.is_empty());
        assert!(!config.depth_variants.is_empty());
    }

    #[tokio::test]
    async fn test_bench_setup() {
        let (schema, state) = create_bench_setup().await;

        // Verify schema is functional
        let query = "query { health }";
        let req = Request::new(query);
        let res = schema.execute(req).await;

        assert!(res.errors.is_empty());
    }

    #[tokio::test]
    async fn test_performance_targets() {
        let (schema, _state) = create_bench_setup().await;
        let config = BenchConfig::default();

        // Test simple query performance
        let simple_query = "query { health version }";
        let start = std::time::Instant::now();
        let req = Request::new(simple_query);
        let res = schema.execute(req).await;
        let elapsed = start.elapsed();

        assert!(res.errors.is_empty());
        assert!(
            elapsed.as_millis() < config.simple_query_target_ms as u128,
            "Simple query took {}ms, target is {}ms",
            elapsed.as_millis(),
            config.simple_query_target_ms
        );

        // Test complex query performance
        let complex_query = r#"
            query ComplexTest($input: CodeSearchInput!, $nodeId: ID!) {
                searchCode(input: $input) {
                    nodes { id name nodeType }
                    totalCount
                }
                node(id: $nodeId) { id name }
            }
        "#;

        let variables = Variables::from_json(json!({
            "input": {
                "query": "complex performance test query",
                "limit": 50,
                "languageFilter": ["RUST"],
                "nodeTypeFilter": ["FUNCTION", "CLASS"]
            },
            "nodeId": Uuid::new_v4().to_string()
        }));

        let start = std::time::Instant::now();
        let req = Request::new(complex_query).variables(variables);
        let res = schema.execute(req).await;
        let elapsed = start.elapsed();

        assert!(res.errors.is_empty());
        assert!(
            elapsed.as_millis() < config.complex_query_target_ms as u128,
            "Complex query took {}ms, target is {}ms",
            elapsed.as_millis(),
            config.complex_query_target_ms
        );
    }
}
