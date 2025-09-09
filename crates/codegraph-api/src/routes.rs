use crate::{handlers, vector_handlers, versioning_handlers, AppState};
use axum::{
    routing::{get, post},
    Router,
};
use tower_http::{
    cors::CorsLayer,
    trace::TraceLayer,
};

pub fn create_router(state: AppState) -> Router {
    Router::new()
        // Health check
        .route("/health", get(handlers::health))
        
        // Node operations
        .route("/nodes/:id", get(handlers::get_node))
        .route("/nodes/:id/similar", get(handlers::find_similar))
        
        // Parsing
        .route("/parse", post(handlers::parse_file))
        
        // Search
        .route("/search", get(handlers::search_nodes))
        
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
        
        // Add state
        .with_state(state)
        
        // Add middleware
        .layer(
            CorsLayer::new()
                .allow_origin(tower_http::cors::Any)
                .allow_methods(tower_http::cors::Any)
                .allow_headers(tower_http::cors::Any)
        )
        .layer(TraceLayer::new_for_http())
}