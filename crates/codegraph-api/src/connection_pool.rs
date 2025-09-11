use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use reqwest::{Client, ClientBuilder, Method, RequestBuilder, Url};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

#[derive(Debug, Error)]
pub enum PoolError {
    #[error("invalid base url: {0}")]
    InvalidBaseUrl(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionPoolConfig {
    pub pool_max_idle_per_host: usize,
    pub pool_idle_timeout_secs: u64,
    pub connect_timeout_secs: u64,
    pub tcp_keepalive_secs: Option<u64>,
    pub http2_keep_alive_interval_secs: Option<u64>,
    pub http2_keep_alive_timeout_secs: Option<u64>,
}

impl Default for ConnectionPoolConfig {
    fn default() -> Self {
        Self {
            pool_max_idle_per_host: 32,
            pool_idle_timeout_secs: 90,
            connect_timeout_secs: 10,
            tcp_keepalive_secs: Some(60),
            http2_keep_alive_interval_secs: Some(30),
            http2_keep_alive_timeout_secs: Some(10),
        }
    }
}

impl ConnectionPoolConfig {
    pub fn from_env() -> Self {
        let get_u64 = |key: &str| std::env::var(key).ok().and_then(|v| v.parse().ok());
        let get_usize = |key: &str| std::env::var(key).ok().and_then(|v| v.parse().ok());

        let mut cfg = Self::default();
        if let Some(v) = get_usize("HTTP_POOL_MAX_IDLE_PER_HOST") {
            cfg.pool_max_idle_per_host = v;
        }
        if let Some(v) = get_u64("HTTP_POOL_IDLE_TIMEOUT_SECS") {
            cfg.pool_idle_timeout_secs = v;
        }
        if let Some(v) = get_u64("HTTP_CONNECT_TIMEOUT_SECS") {
            cfg.connect_timeout_secs = v;
        }
        cfg.tcp_keepalive_secs = get_u64("HTTP_TCP_KEEPALIVE_SECS");
        cfg.http2_keep_alive_interval_secs = get_u64("HTTP2_KEEP_ALIVE_INTERVAL_SECS");
        cfg.http2_keep_alive_timeout_secs = get_u64("HTTP2_KEEP_ALIVE_TIMEOUT_SECS");
        cfg
    }
}

#[derive(Debug)]
pub struct LoadBalancedEndpoints {
    endpoints: Vec<Url>,
    index: AtomicUsize,
}

impl LoadBalancedEndpoints {
    pub fn new(endpoints: Vec<Url>) -> Self {
        Self {
            endpoints,
            index: AtomicUsize::new(0),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.endpoints.is_empty()
    }

    pub fn next(&self) -> Option<Url> {
        if self.endpoints.is_empty() {
            return None;
        }
        let i = self.index.fetch_add(1, Ordering::Relaxed);
        Some(self.endpoints[i % self.endpoints.len()].clone())
    }
}

#[derive(Clone)]
pub struct HttpClientPool {
    client: Client,
    lb: Arc<LoadBalancedEndpoints>,
    config: ConnectionPoolConfig,
    /// For dynamic reconfiguration if needed later
    _reload_guard: Arc<RwLock<()>>,
}

impl std::fmt::Debug for HttpClientPool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HttpClientPool")
            .field(
                "pool_max_idle_per_host",
                &self.config.pool_max_idle_per_host,
            )
            .field(
                "pool_idle_timeout_secs",
                &self.config.pool_idle_timeout_secs,
            )
            .field("connect_timeout_secs", &self.config.connect_timeout_secs)
            .finish()
    }
}

impl HttpClientPool {
    pub fn new(config: ConnectionPoolConfig, base_urls: Vec<String>) -> Result<Self, PoolError> {
        let lb_urls: Result<Vec<Url>, PoolError> = base_urls
            .into_iter()
            .map(|s| Url::parse(&s).map_err(|_| PoolError::InvalidBaseUrl(s)))
            .collect();

        let lb = Arc::new(LoadBalancedEndpoints::new(lb_urls?));

        // Build client with keep-alive and pooling tuned
        let mut builder = ClientBuilder::new()
            .pool_max_idle_per_host(config.pool_max_idle_per_host)
            .pool_idle_timeout(Duration::from_secs(config.pool_idle_timeout_secs))
            .connect_timeout(Duration::from_secs(config.connect_timeout_secs))
            .http1_title_case_headers();

        if let Some(ka) = config.tcp_keepalive_secs.map(Duration::from_secs) {
            builder = builder.tcp_keepalive(ka);
        }
        if let Some(interval) = config
            .http2_keep_alive_interval_secs
            .map(Duration::from_secs)
        {
            builder = builder
                .http2_keep_alive_interval(interval)
                .http2_keep_alive_while_idle(true);
        }
        if let Some(timeout) = config
            .http2_keep_alive_timeout_secs
            .map(Duration::from_secs)
        {
            builder = builder.http2_keep_alive_timeout(timeout);
        }

        let client = builder.build().expect("failed to build reqwest client");

        Ok(Self {
            client,
            lb,
            config,
            _reload_guard: Arc::new(RwLock::new(())),
        })
    }

    pub fn client(&self) -> &Client {
        &self.client
    }

    pub fn close_idle(&self) {
        self.client.close_idle_connections();
    }

    pub fn request(&self, method: Method, url: &str) -> RequestBuilder {
        self.client.request(method, url)
    }

    pub fn request_with_base(&self, method: Method, path: &str) -> RequestBuilder {
        if let Some(mut base) = self.lb.next() {
            // Join path to base; tolerate leading slash
            let joined = if path.is_empty() {
                base
            } else {
                if path.starts_with('/') {
                    base.set_path(path);
                    base
                } else {
                    // Append relative
                    base.join(path).unwrap_or(base)
                }
            };
            self.client.request(method, joined)
        } else {
            warn!("request_with_base called with no configured endpoints; using raw path");
            self.client.request(method, path)
        }
    }

    pub fn get_with_base(&self, path: &str) -> RequestBuilder {
        self.request_with_base(Method::GET, path)
    }

    pub fn post_with_base(&self, path: &str) -> RequestBuilder {
        self.request_with_base(Method::POST, path)
    }
}

/// Utility to parse comma-separated base URLs from env var OUTBOUND_BASE_URLS
pub fn load_base_urls_from_env() -> Vec<String> {
    std::env::var("OUTBOUND_BASE_URLS")
        .ok()
        .map(|s| {
            s.split(',')
                .map(|v| v.trim().to_string())
                .filter(|v| !v.is_empty())
                .collect()
        })
        .unwrap_or_else(Vec::new)
}
