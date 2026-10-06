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
            host: std::env::var("CODEGRAPH_HTTP_HOST").unwrap_or_else(|_| "127.0.0.1".to_string()),
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

/// Host authorities accepted by MCP 3's HTTP transport. Public deployments can
/// supply a comma-separated allowlist independently of the bind address.
pub fn allowed_http_hosts(bind_host: &str) -> Vec<String> {
    if let Ok(hosts) = std::env::var("CODEGRAPH_HTTP_ALLOWED_HOSTS") {
        return hosts
            .split(',')
            .map(str::trim)
            .filter(|host| !host.is_empty())
            .map(str::to_owned)
            .collect();
    }
    let mut hosts = vec![
        "localhost".to_owned(),
        "127.0.0.1".to_owned(),
        "::1".to_owned(),
    ];
    if !hosts.iter().any(|host| host == bind_host) {
        hosts.push(bind_host.to_owned());
    }
    hosts
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
        if !test_env::run(
            concat!(module_path!(), "::test_from_env_with_valid_values"),
            &[
                ("CODEGRAPH_HTTP_HOST", Some("0.0.0.0")),
                ("CODEGRAPH_HTTP_PORT", Some("8080")),
                ("CODEGRAPH_HTTP_KEEP_ALIVE", Some("30")),
            ],
        ) {
            return;
        }

        let config = HttpServerConfig::from_env();
        assert_eq!(config.host, "0.0.0.0");
        assert_eq!(config.port, 8080);
        assert_eq!(config.keep_alive_seconds, 30);

        // Cleanup
    }

    #[test]
    #[serial]
    fn test_from_env_with_invalid_port() {
        if !test_env::run(
            concat!(module_path!(), "::test_from_env_with_invalid_port"),
            &[
                ("CODEGRAPH_HTTP_HOST", Some("localhost")),
                ("CODEGRAPH_HTTP_PORT", Some("not_a_number")),
                ("CODEGRAPH_HTTP_KEEP_ALIVE", Some("invalid")),
            ],
        ) {
            return;
        }

        let config = HttpServerConfig::from_env();
        assert_eq!(config.host, "localhost");
        assert_eq!(config.port, 3000); // Falls back to default
        assert_eq!(config.keep_alive_seconds, 15); // Falls back to default

        // Cleanup
    }

    #[test]
    #[serial]
    fn test_from_env_with_missing_vars() {
        if !test_env::run(
            concat!(module_path!(), "::test_from_env_with_missing_vars"),
            &[
                ("CODEGRAPH_HTTP_HOST", None),
                ("CODEGRAPH_HTTP_PORT", None),
                ("CODEGRAPH_HTTP_KEEP_ALIVE", None),
            ],
        ) {
            return;
        }

        // Ensure vars are not set

        let config = HttpServerConfig::from_env();
        assert_eq!(config.host, "127.0.0.1"); // Default
        assert_eq!(config.port, 3000); // Default
        assert_eq!(config.keep_alive_seconds, 15); // Default
    }
}

#[cfg(test)]
mod test_env {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/support/env.rs"
    ));
}
