// ABOUTME: Factory for creating reranker instances based on configuration
// ABOUTME: Supports Jina API and Ollama chat-based reranking providers
use super::Reranker;
#[cfg(feature = "jina")]
use super::jina::JinaReranker;
#[cfg(feature = "ollama")]
use super::ollama::OllamaReranker;
#[cfg(any(feature = "jina", feature = "ollama"))]
use anyhow::Context;
use anyhow::Result;
use codegraph_core::{RerankConfig, RerankProvider};
use std::sync::Arc;

/// Create a reranker instance based on the configuration
pub fn create_reranker(config: &RerankConfig) -> Result<Option<Arc<dyn Reranker>>> {
    let config = config.clone().with_provider_defaults();
    config.validate()?;
    match config.provider {
        RerankProvider::None => Ok(None),

        RerankProvider::Jina => {
            #[cfg(feature = "jina")]
            {
                let reranker =
                    JinaReranker::new(&config).context("Failed to create Jina reranker")?;
                Ok(Some(Arc::new(reranker)))
            }
            #[cfg(not(feature = "jina"))]
            {
                anyhow::bail!("RerankProvider::Jina requested but the 'jina' feature is disabled")
            }
        }

        RerankProvider::Ollama => {
            #[cfg(feature = "ollama")]
            {
                let reranker =
                    OllamaReranker::new(&config).context("Failed to create Ollama reranker")?;
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
/// Uses the same provider, model and endpoint overrides as the main config loader.
pub fn create_reranker_from_env(config: &RerankConfig) -> Result<Option<Arc<dyn Reranker>>> {
    create_reranker(&config.clone().with_env_overrides())
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
        if !test_env::run(
            concat!(
                module_path!(),
                "::test_create_jina_reranker_without_api_key"
            ),
            &[("JINA_API_KEY", None)],
        ) {
            return;
        }
        let config = RerankConfig {
            provider: RerankProvider::Jina,
            top_n: 10,
            jina: Some(JinaRerankConfig::default()),
            ollama: None,
        };

        let error = create_reranker(&config)
            .err()
            .expect("missing key must fail");
        #[cfg(feature = "jina")]
        assert!(format!("{error:#}").contains("Jina API key not found"));
        #[cfg(not(feature = "jina"))]
        assert!(error.to_string().contains("feature is disabled"));
    }

    #[cfg(feature = "jina")]
    #[test]
    fn selected_jina_provider_works_without_a_nested_config_block() {
        if !test_env::run(
            concat!(
                module_path!(),
                "::selected_jina_provider_works_without_a_nested_config_block"
            ),
            &[("JINA_API_KEY", Some("test-key"))],
        ) {
            return;
        }
        let config = RerankConfig {
            provider: RerankProvider::Jina,
            ..Default::default()
        };
        let reranker = create_reranker(&config).unwrap().unwrap();
        assert_eq!(reranker.provider_name(), "jina");
        assert_eq!(reranker.model_name(), "jina-reranker-v3");
    }

    #[cfg(feature = "ollama")]
    #[test]
    fn env_factory_selects_ollama_and_supplies_defaults() {
        if !test_env::run(
            concat!(
                module_path!(),
                "::env_factory_selects_ollama_and_supplies_defaults"
            ),
            &[
                ("CODEGRAPH_RERANK_PROVIDER", Some("ollama")),
                ("CODEGRAPH_ENABLE_RERANKING", None),
                ("JINA_ENABLE_RERANKING", Some("true")),
                ("CODEGRAPH_OLLAMA_RERANK_MODEL", Some("fixture-reranker")),
            ],
        ) {
            return;
        }
        let reranker = create_reranker_from_env(&RerankConfig::default())
            .unwrap()
            .unwrap();
        assert_eq!(reranker.provider_name(), "ollama");
        assert_eq!(reranker.model_name(), "fixture-reranker");
    }

    #[cfg(feature = "jina")]
    #[tokio::test]
    async fn env_factory_sends_jina_rerank_requests_and_preserves_source_metadata() {
        use crate::reranking::RerankDocument;
        use serde_json::{Value, json};
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        if !test_env::run(
            concat!(
                module_path!(),
                "::env_factory_sends_jina_rerank_requests_and_preserves_source_metadata"
            ),
            &[
                ("CODEGRAPH_RERANK_PROVIDER", Some("jina")),
                ("CODEGRAPH_ENABLE_RERANKING", None),
                ("JINA_ENABLE_RERANKING", None),
                ("JINA_RERANKING_MODEL", Some("fixture-reranker")),
                ("JINA_API_BASE", None),
                ("JINA_API_KEY", Some("test-key")),
            ],
        ) {
            return;
        }
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}/v1", listener.local_addr().unwrap());
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut bytes = Vec::new();
            let body: Value;
            loop {
                let mut buffer = [0; 4096];
                let count = stream.read(&mut buffer).await.unwrap();
                assert!(count > 0);
                bytes.extend_from_slice(&buffer[..count]);
                if let Some(end) = bytes.windows(4).position(|window| window == b"\r\n\r\n") {
                    let header = String::from_utf8_lossy(&bytes[..end]).to_ascii_lowercase();
                    assert!(header.starts_with("post /v1/rerank "));
                    assert!(header.contains("authorization: bearer test-key"));
                    let length: usize = header
                        .lines()
                        .find_map(|line| line.strip_prefix("content-length: "))
                        .unwrap()
                        .parse()
                        .unwrap();
                    if bytes.len() >= end + 4 + length {
                        body = serde_json::from_slice(&bytes[end + 4..end + 4 + length]).unwrap();
                        break;
                    }
                }
            }
            let response = json!({
                "model": "fixture-reranker",
                "results": [{"index": 1, "relevance_score": 0.95}],
                "usage": {"total_tokens": 12}
            })
            .to_string();
            stream.write_all(format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{response}",
                response.len()
            ).as_bytes()).await.unwrap();
            body
        });
        let config = RerankConfig {
            jina: Some(JinaRerankConfig {
                api_base: url,
                ..Default::default()
            }),
            ..Default::default()
        };
        let reranker = create_reranker_from_env(&config).unwrap().unwrap();
        let documents = vec![
            RerankDocument {
                id: "first".into(),
                text: "unrelated".into(),
                metadata: None,
            },
            RerankDocument {
                id: "second".into(),
                text: "matching code".into(),
                metadata: Some(json!({"file_path": "src/lib.rs", "line": 12})),
            },
        ];
        let results = tokio::time::timeout(
            std::time::Duration::from_secs(5),
            reranker.rerank("find matching code", documents, 1),
        )
        .await
        .unwrap()
        .unwrap();
        let body = server.await.unwrap();
        assert_eq!(body["model"], "fixture-reranker");
        assert_eq!(body["query"], "find matching code");
        assert_eq!(body["documents"], json!(["unrelated", "matching code"]));
        assert_eq!(body["top_n"], 1);
        assert_eq!(body["return_documents"], false);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "second");
        assert_eq!(
            results[0].metadata,
            Some(json!({"file_path": "src/lib.rs", "line": 12}))
        );
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
        #[cfg(feature = "ollama")]
        {
            assert!(result.is_ok());
            assert!(result.unwrap().is_some());
        }
        #[cfg(not(feature = "ollama"))]
        {
            assert!(result.is_err());
        }
    }

    #[test]
    fn test_create_reranker_from_env() {
        if !test_env::run(
            concat!(module_path!(), "::test_create_reranker_from_env"),
            &[
                ("CODEGRAPH_RERANK_PROVIDER", Some("none")),
                ("CODEGRAPH_ENABLE_RERANKING", Some("true")),
                ("JINA_ENABLE_RERANKING", Some("true")),
            ],
        ) {
            return;
        }

        let config = RerankConfig {
            provider: RerankProvider::Jina, // Will be overridden by env var
            top_n: 10,
            jina: Some(JinaRerankConfig::default()),
            ollama: None,
        };

        let result = create_reranker_from_env(&config).unwrap();
        assert!(result.is_none());
    }
}

#[cfg(test)]
mod test_env {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/support/env.rs"
    ));
}
