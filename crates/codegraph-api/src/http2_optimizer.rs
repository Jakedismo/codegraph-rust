#![cfg(feature = "http2")]

use axum::{
    body::Body,
    extract::{Path, Query, Request},
    http::{HeaderMap, HeaderName, HeaderValue, StatusCode},
    response::Response,
};
use hyper::Version;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};
use tokio::sync::{RwLock, Semaphore};
use tracing::{debug, info, instrument, warn};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Http2OptimizerConfig {
    pub max_concurrent_streams: usize,
    pub initial_window_size: u32,
    pub max_frame_size: u32,
    pub header_table_size: u32,
    pub enable_server_push: bool,
    pub push_timeout_ms: u64,
    pub stream_timeout_ms: u64,
    pub enable_adaptive_window: bool,
    pub max_header_list_size: u32,
}

impl Default for Http2OptimizerConfig {
    fn default() -> Self {
        Self {
            max_concurrent_streams: 100,
            initial_window_size: 65535,
            max_frame_size: 16384,
            header_table_size: 4096,
            enable_server_push: true,
            push_timeout_ms: 5000,
            stream_timeout_ms: 30000,
            enable_adaptive_window: true,
            max_header_list_size: 8192,
        }
    }
}

#[derive(Debug, Clone)]
pub struct StreamMetrics {
    pub stream_id: u32,
    pub start_time: Instant,
    pub bytes_sent: AtomicU64,
    pub bytes_received: AtomicU64,
    pub frame_count: AtomicU64,
}

impl StreamMetrics {
    pub fn new(stream_id: u32) -> Self {
        Self {
            stream_id,
            start_time: Instant::now(),
            bytes_sent: AtomicU64::new(0),
            bytes_received: AtomicU64::new(0),
            frame_count: AtomicU64::new(0),
        }
    }

    pub fn record_bytes_sent(&self, bytes: u64) {
        self.bytes_sent.fetch_add(bytes, Ordering::Relaxed);
    }

    pub fn record_bytes_received(&self, bytes: u64) {
        self.bytes_received.fetch_add(bytes, Ordering::Relaxed);
    }

    pub fn record_frame(&self) {
        self.frame_count.fetch_add(1, Ordering::Relaxed);
    }
}

#[derive(Debug)]
pub struct StreamMultiplexer {
    config: Http2OptimizerConfig,
    active_streams: Arc<RwLock<HashMap<u32, Arc<StreamMetrics>>>>,
    stream_semaphore: Arc<Semaphore>,
    next_stream_id: AtomicU64,
    total_streams: AtomicU64,
    connection_metrics: Arc<ConnectionMetrics>,
}

#[derive(Debug, Default)]
pub struct ConnectionMetrics {
    pub total_bytes_sent: AtomicU64,
    pub total_bytes_received: AtomicU64,
    pub total_frames: AtomicU64,
    pub push_promises_sent: AtomicU64,
    pub streams_created: AtomicU64,
    pub window_updates_sent: AtomicU64,
}

impl StreamMultiplexer {
    pub fn new(config: Http2OptimizerConfig) -> Self {
        let stream_semaphore = Arc::new(Semaphore::new(config.max_concurrent_streams));

        Self {
            config,
            active_streams: Arc::new(RwLock::new(HashMap::new())),
            stream_semaphore,
            next_stream_id: AtomicU64::new(1),
            total_streams: AtomicU64::new(0),
            connection_metrics: Arc::new(ConnectionMetrics::default()),
        }
    }

    #[instrument(skip(self))]
    pub async fn acquire_stream(&self) -> Result<StreamHandle, Http2Error> {
        let permit = self
            .stream_semaphore
            .acquire()
            .await
            .map_err(|_| Http2Error::StreamLimitExceeded)?;

        let stream_id = self.next_stream_id.fetch_add(2, Ordering::Relaxed) as u32;
        let metrics = Arc::new(StreamMetrics::new(stream_id));

        {
            let mut streams = self.active_streams.write().await;
            streams.insert(stream_id, metrics.clone());
        }

        self.total_streams.fetch_add(1, Ordering::Relaxed);
        self.connection_metrics
            .streams_created
            .fetch_add(1, Ordering::Relaxed);

        info!(
            "Acquired stream {}, total active: {}",
            stream_id,
            self.active_streams.read().await.len()
        );

        Ok(StreamHandle {
            stream_id,
            metrics,
            _permit: permit,
            multiplexer: self.clone(),
        })
    }

    pub async fn release_stream(&self, stream_id: u32) {
        let mut streams = self.active_streams.write().await;
        if let Some(metrics) = streams.remove(&stream_id) {
            let duration = metrics.start_time.elapsed();
            let bytes_sent = metrics.bytes_sent.load(Ordering::Relaxed);
            let bytes_received = metrics.bytes_received.load(Ordering::Relaxed);

            debug!(
                "Released stream {} after {:?}, sent: {} bytes, received: {} bytes",
                stream_id, duration, bytes_sent, bytes_received
            );
        }
    }

    pub async fn get_stream_count(&self) -> usize {
        self.active_streams.read().await.len()
    }

    pub fn get_metrics(&self) -> Arc<ConnectionMetrics> {
        self.connection_metrics.clone()
    }
}

impl Clone for StreamMultiplexer {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            active_streams: self.active_streams.clone(),
            stream_semaphore: self.stream_semaphore.clone(),
            next_stream_id: AtomicU64::new(self.next_stream_id.load(Ordering::Relaxed)),
            total_streams: AtomicU64::new(self.total_streams.load(Ordering::Relaxed)),
            connection_metrics: self.connection_metrics.clone(),
        }
    }
}

pub struct StreamHandle {
    pub stream_id: u32,
    pub metrics: Arc<StreamMetrics>,
    _permit: tokio::sync::SemaphorePermit<'static>,
    multiplexer: StreamMultiplexer,
}

impl Drop for StreamHandle {
    fn drop(&mut self) {
        let stream_id = self.stream_id;
        let multiplexer = self.multiplexer.clone();
        tokio::spawn(async move {
            multiplexer.release_stream(stream_id).await;
        });
    }
}

#[derive(Debug, Clone)]
pub struct ServerPushStrategy {
    config: Http2OptimizerConfig,
    push_cache: Arc<RwLock<HashMap<String, Vec<PushResource>>>>,
    push_metrics: Arc<AtomicU64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PushResource {
    pub path: String,
    pub headers: HashMap<String, String>,
    pub priority: u8,
    pub max_age: Duration,
    pub created_at: Instant,
}

impl ServerPushStrategy {
    pub fn new(config: Http2OptimizerConfig) -> Self {
        Self {
            config,
            push_cache: Arc::new(RwLock::new(HashMap::new())),
            push_metrics: Arc::new(AtomicU64::new(0)),
        }
    }

    #[instrument(skip(self))]
    pub async fn register_push_resources(&self, base_path: &str, resources: Vec<PushResource>) {
        let mut cache = self.push_cache.write().await;
        cache.insert(base_path.to_string(), resources);
        debug!("Registered push resources for path: {}", base_path);
    }

    #[instrument(skip(self, request))]
    pub async fn get_push_resources(&self, request: &Request<Body>) -> Vec<PushResource> {
        if !self.config.enable_server_push {
            return vec![];
        }

        let path = request.uri().path();
        let cache = self.push_cache.read().await;

        if let Some(resources) = cache.get(path) {
            let now = Instant::now();
            let valid_resources: Vec<PushResource> = resources
                .iter()
                .filter(|resource| now.duration_since(resource.created_at) < resource.max_age)
                .cloned()
                .collect();

            if !valid_resources.is_empty() {
                self.push_metrics
                    .fetch_add(valid_resources.len() as u64, Ordering::Relaxed);
                info!(
                    "Found {} push resources for path: {}",
                    valid_resources.len(),
                    path
                );
            }

            valid_resources
        } else {
            vec![]
        }
    }

    pub async fn create_push_headers(&self, resources: &[PushResource]) -> HeaderMap {
        let mut headers = HeaderMap::new();

        for resource in resources {
            let link_value = format!("<{}>; rel=preload", resource.path);
            if let Ok(header_value) = HeaderValue::from_str(&link_value) {
                headers.append("link", header_value);
            }
        }

        headers
    }

    pub fn get_push_count(&self) -> u64 {
        self.push_metrics.load(Ordering::Relaxed)
    }
}

#[derive(Debug, Clone)]
pub struct HpackCompressor {
    config: Http2OptimizerConfig,
    compression_stats: Arc<CompressionStats>,
}

#[derive(Debug, Default)]
pub struct CompressionStats {
    pub headers_compressed: AtomicU64,
    pub bytes_saved: AtomicU64,
    pub compression_ratio: AtomicU64, // Stored as percentage * 100
}

impl HpackCompressor {
    pub fn new(config: Http2OptimizerConfig) -> Self {
        Self {
            config,
            compression_stats: Arc::new(CompressionStats::default()),
        }
    }

    #[instrument(skip(self, headers))]
    pub fn optimize_headers(&self, headers: &mut HeaderMap) -> Result<(), Http2Error> {
        let original_size = self.calculate_header_size(headers);

        // Remove redundant headers that HTTP/2 handles automatically
        headers.remove("connection");
        headers.remove("upgrade");
        headers.remove("proxy-connection");
        headers.remove("transfer-encoding");

        // Optimize common headers by lowercasing (HPACK prefers lowercase)
        let headers_to_optimize = [
            "content-type",
            "content-length",
            "user-agent",
            "accept",
            "accept-encoding",
            "accept-language",
            "cache-control",
        ];

        for header_name in &headers_to_optimize {
            if let Some(value) = headers.remove(*header_name) {
                if let Ok(name) = HeaderName::from_str(&header_name.to_lowercase()) {
                    headers.insert(name, value);
                }
            }
        }

        // Add HTTP/2 specific optimizations
        if !headers.contains_key("cache-control") {
            headers.insert(
                "cache-control",
                HeaderValue::from_static("public, max-age=3600"),
            );
        }

        let optimized_size = self.calculate_header_size(headers);
        let bytes_saved = original_size.saturating_sub(optimized_size);

        self.compression_stats
            .headers_compressed
            .fetch_add(1, Ordering::Relaxed);
        self.compression_stats
            .bytes_saved
            .fetch_add(bytes_saved as u64, Ordering::Relaxed);

        if original_size > 0 {
            let ratio = ((optimized_size * 10000) / original_size) as u64;
            self.compression_stats
                .compression_ratio
                .store(ratio, Ordering::Relaxed);
        }

        debug!(
            "Header optimization: {} -> {} bytes (saved: {})",
            original_size, optimized_size, bytes_saved
        );

        Ok(())
    }

    fn calculate_header_size(&self, headers: &HeaderMap) -> usize {
        headers
            .iter()
            .map(|(name, value)| name.as_str().len() + value.len() + 4) // +4 for ": " and "\r\n"
            .sum()
    }

    pub fn get_stats(&self) -> &CompressionStats {
        &self.compression_stats
    }
}

#[derive(Debug, Clone)]
pub struct FlowControlOptimizer {
    config: Http2OptimizerConfig,
    window_size: Arc<AtomicU64>,
    flow_stats: Arc<FlowControlStats>,
}

#[derive(Debug, Default)]
pub struct FlowControlStats {
    pub window_updates_sent: AtomicU64,
    pub window_updates_received: AtomicU64,
    pub bytes_throttled: AtomicU64,
    pub avg_window_size: AtomicU64,
}

impl FlowControlOptimizer {
    pub fn new(config: Http2OptimizerConfig) -> Self {
        Self {
            window_size: Arc::new(AtomicU64::new(config.initial_window_size as u64)),
            config,
            flow_stats: Arc::new(FlowControlStats::default()),
        }
    }

    #[instrument(skip(self))]
    pub fn calculate_optimal_window_size(&self, bytes_pending: u64, rtt_ms: u64) -> u32 {
        if !self.config.enable_adaptive_window {
            return self.config.initial_window_size;
        }

        // Adaptive window sizing based on bandwidth-delay product
        let bandwidth_estimate = bytes_pending * 1000 / rtt_ms.max(1);
        let bdp = bandwidth_estimate * rtt_ms / 1000;

        // Use 2x BDP as window size, clamped to reasonable bounds
        let optimal_size = (bdp * 2).clamp(
            self.config.initial_window_size as u64,
            1024 * 1024, // 1MB max
        );

        self.window_size.store(optimal_size, Ordering::Relaxed);
        self.flow_stats
            .avg_window_size
            .store(optimal_size, Ordering::Relaxed);

        debug!(
            "Calculated optimal window size: {} bytes (BDP: {}, RTT: {}ms)",
            optimal_size, bdp, rtt_ms
        );

        optimal_size as u32
    }

    pub fn should_send_window_update(&self, consumed_bytes: u64) -> bool {
        let current_window = self.window_size.load(Ordering::Relaxed);
        let threshold = current_window / 2;

        if consumed_bytes >= threshold {
            self.flow_stats
                .window_updates_sent
                .fetch_add(1, Ordering::Relaxed);
            true
        } else {
            false
        }
    }

    pub fn get_current_window_size(&self) -> u64 {
        self.window_size.load(Ordering::Relaxed)
    }

    pub fn get_stats(&self) -> &FlowControlStats {
        &self.flow_stats
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Http2Error {
    #[error("Stream limit exceeded")]
    StreamLimitExceeded,
    #[error("Push not supported")]
    PushNotSupported,
    #[error("Header compression failed: {0}")]
    CompressionFailed(String),
    #[error("Flow control error: {0}")]
    FlowControlError(String),
}

#[derive(Debug, Clone)]
pub struct Http2Optimizer {
    multiplexer: StreamMultiplexer,
    push_strategy: ServerPushStrategy,
    hpack_compressor: HpackCompressor,
    flow_control: FlowControlOptimizer,
    config: Http2OptimizerConfig,
}

impl Http2Optimizer {
    pub fn new(config: Http2OptimizerConfig) -> Self {
        Self {
            multiplexer: StreamMultiplexer::new(config.clone()),
            push_strategy: ServerPushStrategy::new(config.clone()),
            hpack_compressor: HpackCompressor::new(config.clone()),
            flow_control: FlowControlOptimizer::new(config.clone()),
            config,
        }
    }

    #[instrument(skip(self, request))]
    pub async fn optimize_request(
        &self,
        mut request: Request<Body>,
    ) -> Result<Request<Body>, Http2Error> {
        // Only optimize HTTP/2 requests
        if request.version() != Version::HTTP_2 {
            return Ok(request);
        }

        // Acquire stream for multiplexing
        let _stream_handle = self.multiplexer.acquire_stream().await?;

        // Optimize headers with HPACK
        let headers = request.headers_mut();
        self.hpack_compressor.optimize_headers(headers)?;

        // Add flow control information
        let window_size = self.flow_control.get_current_window_size();
        if let Ok(window_header) = HeaderValue::from_str(&window_size.to_string()) {
            headers.insert("x-http2-window-size", window_header);
        }

        Ok(request)
    }

    #[instrument(skip(self, response))]
    pub async fn optimize_response(
        &self,
        mut response: Response<Body>,
        request: &Request<Body>,
    ) -> Result<Response<Body>, Http2Error> {
        // Only optimize HTTP/2 responses
        if request.version() != Version::HTTP_2 {
            return Ok(response);
        }

        // Optimize response headers
        let headers = response.headers_mut();
        self.hpack_compressor.optimize_headers(headers)?;

        // Add server push headers if applicable
        let push_resources = self.push_strategy.get_push_resources(request).await;
        if !push_resources.is_empty() {
            let push_headers = self
                .push_strategy
                .create_push_headers(&push_resources)
                .await;
            for (name, value) in push_headers {
                if let Some(name) = name {
                    headers.insert(name, value);
                }
            }
        }

        // Add HTTP/2 performance headers
        headers.insert("x-http2-stream-id", HeaderValue::from_static("optimized"));
        headers.insert("x-http2-multiplexed", HeaderValue::from_static("true"));

        Ok(response)
    }

    pub async fn register_push_resources(&self, path: &str, resources: Vec<PushResource>) {
        self.push_strategy
            .register_push_resources(path, resources)
            .await;
    }

    pub async fn get_connection_metrics(&self) -> Http2Metrics {
        Http2Metrics {
            active_streams: self.multiplexer.get_stream_count().await,
            total_streams: self.multiplexer.total_streams.load(Ordering::Relaxed),
            push_promises_sent: self.push_strategy.get_push_count(),
            headers_compressed: self
                .hpack_compressor
                .get_stats()
                .headers_compressed
                .load(Ordering::Relaxed),
            bytes_saved: self
                .hpack_compressor
                .get_stats()
                .bytes_saved
                .load(Ordering::Relaxed),
            window_updates_sent: self
                .flow_control
                .get_stats()
                .window_updates_sent
                .load(Ordering::Relaxed),
            current_window_size: self.flow_control.get_current_window_size(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Http2Metrics {
    pub active_streams: usize,
    pub total_streams: u64,
    pub push_promises_sent: u64,
    pub headers_compressed: u64,
    pub bytes_saved: u64,
    pub window_updates_sent: u64,
    pub current_window_size: u64,
}

// HTTP/2 optimization middleware can be applied at the service level
// For now, optimization is handled through the AppState and handlers

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::{Method, Uri};

    #[tokio::test]
    async fn test_stream_multiplexer() {
        let config = Http2OptimizerConfig::default();
        let multiplexer = StreamMultiplexer::new(config);

        let handle1 = multiplexer.acquire_stream().await.unwrap();
        let handle2 = multiplexer.acquire_stream().await.unwrap();

        assert_eq!(multiplexer.get_stream_count().await, 2);
        assert_ne!(handle1.stream_id, handle2.stream_id);

        drop(handle1);
        // Stream count should decrease after handle is dropped
        tokio::time::sleep(Duration::from_millis(10)).await;
        assert_eq!(multiplexer.get_stream_count().await, 1);
    }

    #[tokio::test]
    async fn test_hpack_compression() {
        let config = Http2OptimizerConfig::default();
        let compressor = HpackCompressor::new(config);

        let mut headers = HeaderMap::new();
        headers.insert("Connection", HeaderValue::from_static("keep-alive"));
        headers.insert("Content-Type", HeaderValue::from_static("application/json"));
        headers.insert("User-Agent", HeaderValue::from_static("test"));

        let original_size = headers.len();
        compressor.optimize_headers(&mut headers).unwrap();

        // Connection header should be removed
        assert!(!headers.contains_key("connection"));
        assert!(headers.contains_key("content-type"));
        assert!(headers.contains_key("user-agent"));
    }

    #[tokio::test]
    async fn test_server_push_strategy() {
        let config = Http2OptimizerConfig::default();
        let push_strategy = ServerPushStrategy::new(config);

        let resources = vec![PushResource {
            path: "/style.css".to_string(),
            headers: HashMap::new(),
            priority: 1,
            max_age: Duration::from_secs(3600),
            created_at: Instant::now(),
        }];

        push_strategy
            .register_push_resources("/index.html", resources)
            .await;

        let request = Request::builder()
            .method(Method::GET)
            .uri("/index.html")
            .body(Body::empty())
            .unwrap();

        let push_resources = push_strategy.get_push_resources(&request).await;
        assert_eq!(push_resources.len(), 1);
        assert_eq!(push_resources[0].path, "/style.css");
    }

    #[tokio::test]
    async fn test_flow_control_optimizer() {
        let config = Http2OptimizerConfig::default();
        let flow_control = FlowControlOptimizer::new(config);

        let optimal_size = flow_control.calculate_optimal_window_size(1000, 100);
        assert!(optimal_size >= config.initial_window_size);

        let should_update =
            flow_control.should_send_window_update(flow_control.get_current_window_size() / 2 + 1);
        assert!(should_update);
    }
}
