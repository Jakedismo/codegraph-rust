use crate::{handlers, health, service_registry, vector_handlers, versioning_handlers, streaming_handlers, http2_handlers, AppState, auth_middleware, create_schema, RateLimitManager};
use axum::{
    routing::{get, post},
    Router,
    middleware,
};
use tower_http::{
    cors::CorsLayer,
    trace::TraceLayer,
    set_header::SetResponseHeaderLayer,
    compression::CompressionLayer,
};
use tower::{
    buffer::BufferLayer,
    limit::ConcurrencyLimitLayer,
    timeout::TimeoutLayer,
    load_shed::LoadShedLayer,
};
use http::header::{HeaderName, HeaderValue, CONNECTION};
use std::time::Duration;
use async_graphql_axum::{GraphQL, GraphQLSubscription};
use async_graphql::http::GraphiQLSource;
use axum::response::Html;

pub fn create_router(state: AppState) -> Router {
    let schema = create_schema(state.clone());

    let mut app = Router::new()
        // Health and readiness checks
        .route("/health", get(health::comprehensive_health_check))
        .route("/health/live", get(health::liveness_check))
        .route("/health/ready", get(health::readiness_check))
        .route("/metrics", get(handlers::metrics_handler))
        
        // GraphQL HTTP endpoint and GraphiQL IDE
        .route("/graphql", post(GraphQL::new(schema.clone())))
        .route("/graphiql", get(|| async {
            Html(GraphiQLSource::build()
                .endpoint("/graphql")
                .subscription_endpoint("/graphql/ws")
                .finish())
        }))
        // GraphQL WebSocket subscriptions (graphql-transport-ws)
        .route("/graphql/ws", get(GraphQLSubscription::new(schema.clone())))
        .route_layer(middleware::from_fn_with_state(state.clone(), auth_middleware))
        .layer(RateLimitManager::new())

        // Node operations
        .route("/nodes/:id", get(handlers::get_node))
        .route("/nodes/:id/similar", get(handlers::find_similar))
        
        // Parsing
        .route("/parse", post(handlers::parse_file))
        
        // Search
        .route("/search", get(handlers::search_nodes))
        
        // Streaming endpoints for large datasets
        .route("/stream/search", get(streaming_handlers::stream_search_results))
        .route("/stream/dataset", get(streaming_handlers::stream_large_dataset))
        .route("/stream/csv", get(streaming_handlers::stream_csv_results))
        .route("/stream/:id/metadata", get(streaming_handlers::get_stream_metadata))
        .route("/stream/stats", get(streaming_handlers::get_flow_control_stats))
        
        // Advanced vector search
        .route("/vector/search", post(vector_handlers::vector_search))
        .route("/vector/batch-search", post(vector_handlers::batch_vector_search))
        .route("/vector/index/stats", get(vector_handlers::get_index_stats))
        .route("/vector/index/config", get(vector_handlers::get_index_config))
        .route("/vector/index/rebuild", post(vector_handlers::rebuild_index))
        .route("/vector/performance", get(vector_handlers::get_search_performance))
        
        // Batch operations
        .route("/batch/operations", post(vector_handlers::submit_batch_operations))
        .route("/batch/status", get(vector_handlers::get_batch_status))
        
        // Transaction Management
        .route("/transactions", post(versioning_handlers::begin_transaction))
        .route("/transactions/:id/commit", post(versioning_handlers::commit_transaction))
        .route("/transactions/:id/rollback", post(versioning_handlers::rollback_transaction))
        
        // Version Management
        .route("/versions", post(versioning_handlers::create_version))
        .route("/versions", get(versioning_handlers::list_versions))
        .route("/versions/:id", get(versioning_handlers::get_version))
        .route("/versions/:id/tag", post(versioning_handlers::tag_version))
        .route("/versions/:from/compare/:to", get(versioning_handlers::compare_versions))
        
        // Branch Management
        .route("/branches", post(versioning_handlers::create_branch))
        .route("/branches", get(versioning_handlers::list_branches))
        .route("/branches/:name", get(versioning_handlers::get_branch))
        .route("/branches/:name", axum::routing::delete(versioning_handlers::delete_branch))
        
        // Merge Operations
        .route("/merge", post(versioning_handlers::merge_branches))
        .route("/merge/:id/resolve", post(versioning_handlers::resolve_conflicts))
        
        // Snapshot Management
        .route("/snapshots/:transaction_id", post(versioning_handlers::create_snapshot))
        .route("/snapshots/:id", get(versioning_handlers::get_snapshot))
        
        // Statistics and Monitoring
        .route("/stats/transactions", get(versioning_handlers::get_transaction_stats))
        .route("/stats/recovery", get(versioning_handlers::get_recovery_stats))
        .route("/integrity/check", post(versioning_handlers::run_integrity_check))
        
        // Backup and Recovery
        .route("/backup", post(versioning_handlers::create_backup))
        .route("/backup/:id/restore", post(versioning_handlers::restore_from_backup))
        
        // HTTP/2 Optimization and Metrics
        .route("/http2/metrics", get(http2_handlers::get_http2_metrics))
        .route("/http2/config", get(http2_handlers::get_http2_config))
        .route("/http2/config", post(http2_handlers::update_http2_config))
        .route("/http2/push/register", post(http2_handlers::register_push_resources))
        .route("/http2/health", get(http2_handlers::get_http2_health))
        .route("/http2/analytics", get(http2_handlers::get_stream_analytics))
        .route("/http2/performance", get(http2_handlers::get_performance_metrics))
        .route("/http2/tune", post(http2_handlers::tune_http2_optimization))
        
        // Service Discovery and Registration
        .route("/services", post(service_registry::register_service_handler))
        .route("/services", get(service_registry::list_services_handler))
        .route("/services/discover", get(service_registry::discover_services_handler))
        .route("/services/:id", get(service_registry::get_service_handler))
        .route("/services/:id", axum::routing::delete(service_registry::deregister_service_handler))
        .route("/services/heartbeat", post(service_registry::heartbeat_handler))
        
        // Add state
        .with_state(state)
        
        // Add middleware (order matters)
        .layer(middleware::from_fn(crate::metrics::http_metrics_middleware))
        .layer(
            CorsLayer::new()
                .allow_origin(tower_http::cors::Any)
                .allow_methods(tower_http::cors::Any)
                .allow_headers(tower_http::cors::Any)
        )
        // Adaptive response compression (gzip, deflate, brotli, zstd)
        .layer(CompressionLayer::new())
        // Set keep-alive headers to encourage connection reuse by clients
        .layer({
            let connection_value = HeaderValue::from_static("keep-alive");
            SetResponseHeaderLayer::if_not_present(CONNECTION, connection_value)
        })
        .layer({
            let keep_alive = HeaderName::from_static("keep-alive");
            let keep_alive_value = HeaderValue::from_static("timeout=60, max=1000");
            SetResponseHeaderLayer::if_not_present(keep_alive, keep_alive_value)
        })
        // Apply backpressure and timeouts
        .layer(BufferLayer::new(1024))
        .layer(ConcurrencyLimitLayer::new(512))
        .layer(LoadShedLayer::new())
        .layer(TimeoutLayer::new(Duration::from_secs(30)))
        .layer(TraceLayer::new_for_http());

    // Memory leak detection routes (only when feature enabled)
    #[cfg(feature = "leak-detect")]
    {
        let leak_routes = Router::new()
            .route("/memory/stats", get(handlers::memory_stats))
            .route("/memory/leaks", get(handlers::export_leak_report));
        app = app.merge(leak_routes);
    }

    app
}
