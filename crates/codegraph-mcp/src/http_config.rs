// ABOUTME: HTTP server configuration for CodeGraph MCP server
// ABOUTME: Handles host, port, and SSE keep-alive settings for session-based HTTP transport

use serde::{Deserialize, Serialize};

/// Configuration for HTTP server transport
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpServerConfig {
    /// Host address to bind to (default: "127.0.0.1")
    pub host: String,
    /// Port to listen on (default: 3000)
    pub port: u16,
    /// SSE keep-alive interval in seconds (default: 15)
    pub keep_alive_seconds: u64,
}

impl Default for HttpServerConfig {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port: 3000,
            keep_alive_seconds: 15,
        }
    }
}

impl HttpServerConfig {
    /// Get the bind address as host:port string
    pub fn bind_address(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }

    /// Parse from environment variables with CODEGRAPH_HTTP_ prefix
    pub fn from_env() -> Self {
        Self {
            host: std::env::var("CODEGRAPH_HTTP_HOST")
                .unwrap_or_else(|_| "127.0.0.1".to_string()),
            port: std::env::var("CODEGRAPH_HTTP_PORT")
                .ok()
                .and_then(|p| p.parse().ok())
                .unwrap_or(3000),
            keep_alive_seconds: std::env::var("CODEGRAPH_HTTP_KEEP_ALIVE")
                .ok()
                .and_then(|k| k.parse().ok())
                .unwrap_or(15),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    #[test]
    fn test_default_http_config() {
        let config = HttpServerConfig::default();
        assert_eq!(config.host, "127.0.0.1");
        assert_eq!(config.port, 3000);
        assert_eq!(config.keep_alive_seconds, 15);
    }

    #[test]
    fn test_bind_address() {
        let config = HttpServerConfig {
            host: "0.0.0.0".to_string(),
            port: 8080,
            keep_alive_seconds: 30,
        };
        assert_eq!(config.bind_address(), "0.0.0.0:8080");
    }

    #[test]
    #[serial]
    fn test_from_env_with_valid_values() {
        std::env::set_var("CODEGRAPH_HTTP_HOST", "0.0.0.0");
        std::env::set_var("CODEGRAPH_HTTP_PORT", "8080");
        std::env::set_var("CODEGRAPH_HTTP_KEEP_ALIVE", "30");

        let config = HttpServerConfig::from_env();
        assert_eq!(config.host, "0.0.0.0");
        assert_eq!(config.port, 8080);
        assert_eq!(config.keep_alive_seconds, 30);

        // Cleanup
        std::env::remove_var("CODEGRAPH_HTTP_HOST");
        std::env::remove_var("CODEGRAPH_HTTP_PORT");
        std::env::remove_var("CODEGRAPH_HTTP_KEEP_ALIVE");
    }

    #[test]
    #[serial]
    fn test_from_env_with_invalid_port() {
        std::env::set_var("CODEGRAPH_HTTP_HOST", "localhost");
        std::env::set_var("CODEGRAPH_HTTP_PORT", "not_a_number");
        std::env::set_var("CODEGRAPH_HTTP_KEEP_ALIVE", "invalid");

        let config = HttpServerConfig::from_env();
        assert_eq!(config.host, "localhost");
        assert_eq!(config.port, 3000); // Falls back to default
        assert_eq!(config.keep_alive_seconds, 15); // Falls back to default

        // Cleanup
        std::env::remove_var("CODEGRAPH_HTTP_HOST");
        std::env::remove_var("CODEGRAPH_HTTP_PORT");
        std::env::remove_var("CODEGRAPH_HTTP_KEEP_ALIVE");
    }

    #[test]
    #[serial]
    fn test_from_env_with_missing_vars() {
        // Ensure vars are not set
        std::env::remove_var("CODEGRAPH_HTTP_HOST");
        std::env::remove_var("CODEGRAPH_HTTP_PORT");
        std::env::remove_var("CODEGRAPH_HTTP_KEEP_ALIVE");

        let config = HttpServerConfig::from_env();
        assert_eq!(config.host, "127.0.0.1"); // Default
        assert_eq!(config.port, 3000); // Default
        assert_eq!(config.keep_alive_seconds, 15); // Default
    }
}
