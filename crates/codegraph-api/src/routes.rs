use crate::{handlers, AppState};
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