#[cfg(test)]
mod tests {
    use super::*;
    use tokio_test;

    #[tokio::test]
    async fn test_server_creation() {
        let config = CoreRagServerConfig::default();
        let result = CoreRagMcpServer::new(config);
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_config_validation() {
        let mut config = CoreRagServerConfig::default();
        assert!(config.validate().is_ok());

        // Test invalid config
        config.vector_config.max_results = 0;
        assert!(config.validate().is_err());
    }

    #[tokio::test]
    async fn test_rag_tools_creation() {
        let config = CoreRagServerConfig::default();
        let result = RagTools::new(config);
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_search_functionality() {
        let config = CoreRagServerConfig::default();
        let rag_tools = RagTools::new(config).unwrap();

        let results = rag_tools.search_code("test", 10, 0.5).await;
        assert!(results.is_ok());

        let results = results.unwrap();
        // The mock should return some results
        assert!(!results.is_empty());
    }

    #[tokio::test]
    async fn test_semantic_search() {
        let config = CoreRagServerConfig::default();
        let rag_tools = RagTools::new(config).unwrap();

        let results = rag_tools.semantic_search("vector embeddings", 5).await;
        assert!(results.is_ok());

        let results = results.unwrap();
        assert!(!results.is_empty());
    }

    #[tokio::test]
    async fn test_repo_stats() {
        let config = CoreRagServerConfig::default();
        let rag_tools = RagTools::new(config).unwrap();

        let stats = rag_tools.get_repo_stats().await;
        assert!(stats.is_ok());

        let stats = stats.unwrap();
        assert!(stats.total_nodes > 0);
        assert!(!stats.languages.is_empty());
    }
}
