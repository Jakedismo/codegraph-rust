// ABOUTME: Configuration API for cloud features and hot-reload
// ABOUTME: Exposes cloud config, reload capability, and embedding stats to Node.js

use crate::state::with_state;
use crate::types::{CloudConfig, EmbeddingStats};
use napi::Result;
use napi_derive::napi;

/// Get current cloud configuration
#[napi]
pub async fn get_cloud_config() -> Result<CloudConfig> {
    with_state(|state| {
        let config = state.config_manager.config();

        Ok(CloudConfig {
            jina_enabled: config.embedding.jina_api_key.is_some(),
            jina_model: config
                .embedding
                .model
                .clone()
                .unwrap_or_else(|| "jina-embeddings-v3".to_string()),
            jina_reranking_enabled: config.embedding.jina_enable_reranking,
            surrealdb_enabled: std::env::var("SURREALDB_CONNECTION").is_ok(),
            surrealdb_url: std::env::var("SURREALDB_CONNECTION").ok(),
        })
    })
    .await
}

/// Reload configuration from disk without restarting process
///
/// Returns true if configuration changed, false if unchanged
#[napi]
pub async fn reload_config() -> Result<bool> {
    use crate::state::get_or_init_state;

    let state = get_or_init_state().await?;
    let mut guard = state.write().await;

    let old_cloud_enabled = guard.cloud_enabled;
    let old_jina_model = guard.config_manager.config().embedding.model.clone();

    // Reload configuration
    guard.reload_config().await?;

    let config_changed = old_cloud_enabled != guard.cloud_enabled
        || old_jina_model != guard.config_manager.config().embedding.model;

    if config_changed {
        tracing::info!(
            "🔄 Configuration reloaded: cloud_enabled={}",
            guard.cloud_enabled
        );
    }

    Ok(config_changed)
}

/// Get statistics about embedding provider and cache performance
#[napi]
pub async fn get_embedding_stats() -> Result<EmbeddingStats> {
    with_state(|state| {
        let config = state.config_manager.config();

        // Determine active provider
        let (provider, model) = if config.embedding.jina_api_key.is_some() {
            (
                "jina-ai".to_string(),
                config
                    .embedding
                    .model
                    .clone()
                    .unwrap_or_else(|| "jina-embeddings-v3".to_string()),
            )
        } else {
            (
                "onnx-local".to_string(),
                config
                    .embedding
                    .model
                    .clone()
                    .unwrap_or_else(|| "default".to_string()),
            )
        };

        // Get dimension from config
        let dimension = config.embedding.dimension as u32;

        // Cache stats not yet implemented in AppState
        // Return placeholder values for now
        let cache_stats = (0u32, 0.0f64);

        Ok(EmbeddingStats {
            provider,
            model,
            dimension,
            total_embeddings: cache_stats.0,
            cache_hit_rate: cache_stats.1,
        })
    })
    .await
}

/// Check if cloud features are available
#[napi]
pub async fn is_cloud_available() -> Result<bool> {
    with_state(|state| Ok(state.cloud_enabled)).await
}

/// Get current configuration file path
#[napi]
pub async fn get_config_path() -> Result<String> {
    with_state(|state| {
        Ok(state
            .config_manager
            .config_path()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|| "No config file (using defaults)".to_string()))
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_cloud_config() {
        // This test requires proper initialization
        // Skip for now - will be tested in integration tests
    }
}
