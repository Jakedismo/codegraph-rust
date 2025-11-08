// ABOUTME: Application state management with hot-reload support
// ABOUTME: RwLock-based state for configuration updates without restart

use codegraph_api::state::AppState;
use codegraph_core::ConfigManager;
use napi::Result;
use std::sync::Arc;
use tokio::sync::{OnceCell, RwLock};

#[cfg(feature = "cloud-surrealdb")]
use codegraph_graph::GraphFunctions;

use crate::errors::to_napi_error;

/// NAPI application state with hot-reload support
pub struct NapiAppState {
    pub app_state: AppState,
    pub config_manager: Arc<ConfigManager>,
    pub cloud_enabled: bool,
    #[cfg(feature = "cloud-surrealdb")]
    pub graph_functions: Option<Arc<GraphFunctions>>,
}

impl NapiAppState {
    /// Initialize application state from environment variables
    pub async fn new() -> Result<Self> {
        let config_manager = Arc::new(
            ConfigManager::load()
                .map_err(|e| to_napi_error(format!("Failed to load configuration: {}", e)))?,
        );

        let cloud_enabled = Self::is_cloud_enabled(&config_manager);

        let app_state = AppState::new(Arc::clone(&config_manager))
            .await
            .map_err(to_napi_error)?;

        #[cfg(feature = "cloud-surrealdb")]
        let graph_functions = if let Ok(surrealdb_url) = std::env::var("SURREALDB_CONNECTION") {
            match Self::init_graph_functions(&surrealdb_url).await {
                Ok(gf) => Some(Arc::new(gf)),
                Err(e) => {
                    tracing::warn!("Failed to initialize GraphFunctions: {}", e);
                    None
                }
            }
        } else {
            None
        };

        Ok(Self {
            app_state,
            config_manager,
            cloud_enabled,
            #[cfg(feature = "cloud-surrealdb")]
            graph_functions,
        })
    }

    /// Check if cloud features are enabled (Jina or SurrealDB configured)
    fn is_cloud_enabled(config: &ConfigManager) -> bool {
        // Check for Jina API key
        if config.config().embedding.jina_api_key.is_some() {
            return true;
        }

        // Check for SurrealDB connection via environment variable
        if std::env::var("SURREALDB_CONNECTION").is_ok() {
            return true;
        }

        false
    }

    /// Hot-reload configuration from disk/environment
    pub async fn reload_config(&mut self) -> Result<()> {
        let new_config = Arc::new(
            ConfigManager::load()
                .map_err(|e| to_napi_error(format!("Failed to reload configuration: {}", e)))?,
        );

        self.cloud_enabled = Self::is_cloud_enabled(&new_config);

        // Rebuild AppState with new config
        let new_app_state = AppState::new(Arc::clone(&new_config))
            .await
            .map_err(to_napi_error)?;

        self.app_state = new_app_state;
        self.config_manager = new_config;

        Ok(())
    }

    /// Initialize GraphFunctions from SurrealDB connection string
    #[cfg(feature = "cloud-surrealdb")]
    async fn init_graph_functions(url: &str) -> Result<GraphFunctions> {
        use codegraph_graph::{SurrealDbConfig, SurrealDbStorage};

        let config = SurrealDbConfig {
            connection: url.to_string(),
            namespace: "codegraph".to_string(),
            database: "codegraph".to_string(),
            username: Some("root".to_string()),
            password: Some("root".to_string()),
            strict_mode: false,
            auto_migrate: true,
            cache_enabled: true,
        };

        let storage = SurrealDbStorage::new(config)
            .await
            .map_err(|e| to_napi_error(format!("SurrealDB connection failed: {}", e)))?;

        Ok(GraphFunctions::new(storage.db()))
    }
}

/// Global application state
static STATE: OnceCell<Arc<RwLock<NapiAppState>>> = OnceCell::const_new();

/// Get or initialize the global application state
pub async fn get_or_init_state() -> Result<Arc<RwLock<NapiAppState>>> {
    STATE
        .get_or_try_init(|| async {
            let state = NapiAppState::new().await?;
            Ok(Arc::new(RwLock::new(state)))
        })
        .await
        .map(Arc::clone)
}

/// Execute function with read access to state
pub async fn with_state<F, R>(f: F) -> Result<R>
where
    F: FnOnce(&NapiAppState) -> Result<R>,
{
    let state = get_or_init_state().await?;
    let guard = state.read().await;
    f(&guard)
}

/// Execute function with write access to state
#[allow(dead_code)]
pub async fn with_state_mut<F, R>(f: F) -> Result<R>
where
    F: FnOnce(&mut NapiAppState) -> Result<R>,
{
    let state = get_or_init_state().await?;
    let mut guard = state.write().await;
    f(&mut guard)
}
