pub mod auth;
pub mod connection_pool;
pub mod enhanced_health;
pub mod error;
pub mod event_bus;
pub mod graph_stub;
#[cfg(feature = "graphql")]
pub mod graphql;
pub mod handlers;
pub mod health;
#[cfg(feature = "http2")]
pub mod http2_handlers;
#[cfg(feature = "http2")]
pub mod http2_optimizer;
#[cfg(feature = "lb")]
pub mod lb_proxy;
pub mod leak_guard;
pub mod metrics;
// Disabled legacy middleware module; use `auth` for request auth instead
// pub mod middleware;
// Legacy mutations module is disabled to avoid conflicts; mutations live in GraphQL resolvers
// pub mod mutations;
pub mod performance;
#[cfg(feature = "graphql")]
pub mod queries;
pub mod rate_limit;
pub mod rest;
pub mod routes;
#[cfg(feature = "graphql")]
pub mod schema;
pub mod server;
pub mod parser_ext;
pub mod semantic_search_ext;
pub mod vector_store_ext;
pub mod service_registry;
pub mod state;
pub mod streaming_handlers;
#[cfg(feature = "graphql")]
pub mod subscriptions;
pub mod vector_handlers;
pub mod versioning_handlers;

#[cfg(test)]
pub mod test_helpers;

pub use auth::*;
pub use connection_pool::*;
pub use error::*;
pub use event_bus::*;
pub use handlers::*;
pub use health::*;
#[cfg(feature = "http2")]
pub use http2_handlers::*;
#[cfg(feature = "http2")]
pub use http2_optimizer::*;
#[cfg(feature = "lb")]
pub use lb_proxy::*;
pub use metrics::*;
// pub use middleware::*;
// pub use mutations::*;
pub use performance::*;
#[cfg(feature = "graphql")]
pub use queries::*;
pub use rate_limit::RateLimitManager;
pub use routes::*;
#[cfg(feature = "graphql")]
pub use schema::*;
pub use server::*;
pub use service_registry::*;
pub use state::*;
pub use streaming_handlers::*;
#[cfg(feature = "graphql")]
pub use subscriptions::*;
pub use vector_handlers::*;
pub use versioning_handlers::*;
