pub mod advanced_config;
pub mod buffer_pool;
pub mod cli_config;
pub mod config;
pub mod config_manager; // NEW: Simplified configuration system
pub mod embedding_config;
pub mod error;
pub mod incremental;
pub mod integration;
pub mod memory;
pub mod mmap;
pub mod node;
pub mod optimization_coordinator;
pub mod optimized_types;
pub mod performance_config;
pub mod performance_monitor;
pub mod propagation;
pub mod rerank_config;
pub mod shared;
pub mod traits;
pub mod types;
pub mod versioning;
pub mod watch;

pub use advanced_config::*;
pub use buffer_pool::*;
pub use config::ConfigManager as ServerConfigManager;
pub use config::LoggingConfig as ServerLoggingConfig;
pub use config::{
    crypto, DatabaseBackend, DatabaseConfig, SecretsConfig, SecurityConfig, ServerConfig, Settings,
    SurrealDbConfig, VectorConfig,
};
pub use config_manager::*;
pub use embedding_config::*;
pub use error::*;
pub use incremental::*;
pub use integration::*;
pub use mmap::*;
pub use node::*;
pub use optimization_coordinator::*;
pub use optimized_types::*;
pub use performance_config::*;
pub use performance_monitor::*;
pub use propagation::*;
pub use rerank_config::*;
pub use shared::*;
pub use traits::*;
pub use types::*;
pub use versioning::*;
pub use watch::*;

// Use jemalloc as the global allocator when the feature is enabled
#[cfg(feature = "jemalloc")]
#[global_allocator]
static GLOBAL: tikv_jemallocator::Jemalloc = tikv_jemallocator::Jemalloc;
