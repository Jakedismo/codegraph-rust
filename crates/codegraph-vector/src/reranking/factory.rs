// ABOUTME: Factory for creating reranker instances based on configuration
// ABOUTME: Supports Jina API and Ollama chat-based reranking providers
use super::Reranker;
#[cfg(feature = "jina")]
use super::jina::JinaReranker;
#[cfg(feature = "ollama")]
use super::ollama::OllamaReranker;
use anyhow::Result;
#[cfg(any(feature = "jina", feature = "ollama"))]
use anyhow::Context;
use codegraph_core::{RerankConfig, RerankProvider};
use std::sync::Arc;

/// Create a reranker instance based on the configuration
pub fn create_reranker(config: &RerankConfig) -> Result<Option<Arc<dyn Reranker>>> {
    match config.provider {
        RerankProvider::None => Ok(None),

        RerankProvider::Jina => {
            #[cfg(feature = "jina")]
            {
                let reranker = JinaReranker::new(config)
                    .context("Failed to create Jina reranker")?;
                Ok(Some(Arc::new(reranker)))
            }
            #[cfg(not(feature = "jina"))]
            {
                anyhow::bail!(
                    "RerankProvider::Jina requested but the 'jina' feature is disabled"
                )
            }
        }

        RerankProvider::Ollama => {
            #[cfg(feature = "ollama")]
            {
                let reranker = OllamaReranker::new(config)
                    .context("Failed to create Ollama reranker")?;
                Ok(Some(Arc::new(reranker)))
            }
            #[cfg(not(feature = "ollama"))]
            {
                anyhow::bail!(
                    "RerankProvider::Ollama requested but the 'ollama' feature is disabled"
                )
            }
        }
    }
}

/// Create a reranker from environment variable
///
/// Reads CODEGRAPH_RERANK_PROVIDER from environment to determine provider.
/// Falls back to config if not set.
pub fn create_reranker_from_env(config: &RerankConfig) -> Result<Option<Arc<dyn Reranker>>> {
    let provider = std::env::var("CODEGRAPH_RERANK_PROVIDER")
        .ok()
        .and_then(|p| match p.to_lowercase().as_str() {
            "jina" => Some(RerankProvider::Jina),
            "ollama" => Some(RerankProvider::Ollama),
            "none" | "" => Some(RerankProvider::None),
            _ => {
                tracing::warn!("Unknown reranking provider: {}, using config default", p);
                None
            }
        })
        .unwrap_or(config.provider.clone());

    let config_with_provider = RerankConfig {
        provider,
        top_n: config.top_n,
        jina: config.jina.clone(),
        ollama: config.ollama.clone(),
    };

    create_reranker(&config_with_provider)
}

#[cfg(test)]
mod tests {
    use super::*;
    use codegraph_core::{JinaRerankConfig, OllamaRerankConfig};

    #[test]
    fn test_create_none_reranker() {
        let config = RerankConfig {
            provider: RerankProvider::None,
            top_n: 10,
            jina: None,
            ollama: None,
        };

        let result = create_reranker(&config).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_create_jina_reranker_without_api_key() {
        let config = RerankConfig {
            provider: RerankProvider::Jina,
            top_n: 10,
            jina: Some(JinaRerankConfig::default()),
            ollama: None,
        };

        let result = create_reranker(&config);
        // Should fail if JINA_API_KEY is not set
        if std::env::var("JINA_API_KEY").is_err() {
            assert!(result.is_err());
        }
    }

    #[test]
    fn test_create_ollama_reranker() {
        let config = RerankConfig {
            provider: RerankProvider::Ollama,
            top_n: 10,
            jina: None,
            ollama: Some(OllamaRerankConfig::default()),
        };

        let result = create_reranker(&config);
        assert!(result.is_ok());
        assert!(result.unwrap().is_some());
    }

    #[test]
    fn test_create_reranker_from_env() {
        std::env::set_var("CODEGRAPH_RERANK_PROVIDER", "none");

        let config = RerankConfig {
            provider: RerankProvider::Jina, // Will be overridden by env var
            top_n: 10,
            jina: Some(JinaRerankConfig::default()),
            ollama: None,
        };

        let result = create_reranker_from_env(&config).unwrap();
        assert!(result.is_none());

        std::env::remove_var("CODEGRAPH_RERANK_PROVIDER");
    }
}
