use crate::heartbeat::HeartbeatManager;
use crate::message::*;
use crate::protocol::{handshake, parse_response_typed, McpProtocol};
use crate::transport::{backoff_durations, IncomingFrame, Transport, WebSocketTransport};
use crate::version::{ProtocolVersion, VersionNegotiator, DEFAULT_VERSION};
use crate::{McpError, Result};
use dashmap::DashMap;
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::{json, Value};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{broadcast, oneshot, RwLock};
use tokio::time::timeout;
use tracing::warn;
use url::Url;

#[derive(Debug, Clone)]
pub struct McpClientConfig {
    pub url: Url,
    pub client_name: String,
    pub client_version: String,
    pub request_timeout: Duration,
    pub connect_max_retries: usize,
    pub heartbeat: Option<HeartbeatManager>,
}

impl McpClientConfig {
    pub fn new(url: Url) -> Self {
        Self {
            url,
            client_name: "codegraph-mcp-rs".into(),
            client_version: env!("CARGO_PKG_VERSION").into(),
            request_timeout: Duration::from_secs(30),
            connect_max_retries: 5,
            heartbeat: None,
        }
    }

    pub fn with_heartbeat(mut self, hb: HeartbeatManager) -> Self {
        self.heartbeat = Some(hb);
        self
    }
}

/// Core MCP connection supporting JSON-RPC 2.0 and MCP handshake
pub struct McpConnection {
    url: Url,
    writer: Arc<dyn Transport>,
    incoming: broadcast::Receiver<IncomingFrame>,
    negotiator: VersionNegotiator,
    protocol: RwLock<McpProtocol>,
    pending: DashMap<String, oneshot::Sender<JsonRpcMessage>>, // request_id -> tx
    in_flight: AtomicU64,
    notify_handler: RwLock<Option<Arc<dyn Fn(JsonRpcNotification) + Send + Sync>>>,
}

impl McpConnection {
    pub async fn connect(cfg: &McpClientConfig) -> Result<Arc<Self>> {
        // Retry with backoff
        let mut last_err: Option<McpError> = None;
        for d in backoff_durations(cfg.connect_max_retries) {
            match WebSocketTransport::connect(&cfg.url, cfg.heartbeat.clone()).await {
                Ok((t, rx)) => {
                    let conn = Arc::new(Self::new_inner(cfg.url.clone(), t, rx));
                    conn.spawn_reader();
                    conn.initialize(&cfg.client_name, &cfg.client_version).await?;
                    return Ok(conn);
                }
                Err(e) => {
                    last_err = Some(e);
                    warn!(delay_ms = d.as_millis() as u64, "Connect failed, retrying");
                    tokio::time::sleep(d).await;
                }
            }
        }
        Err(last_err.unwrap_or(McpError::Transport("connect failed".into())))
    }

    fn new_inner(url: Url, writer: Arc<dyn Transport>, incoming: broadcast::Receiver<IncomingFrame>) -> Self {
        Self {
            url,
            writer,
            incoming,
            negotiator: VersionNegotiator::new(),
            protocol: RwLock::new(McpProtocol::default()),
            pending: DashMap::new(),
            in_flight: AtomicU64::new(0),
            notify_handler: RwLock::new(None),
        }
    }

    fn spawn_reader(self: &Arc<Self>) {
        let this = Arc::clone(self);
        tokio::spawn(async move {
            let mut rx = this.incoming.resubscribe();
            while let Ok(frame) = rx.recv().await {
                match frame {
                    IncomingFrame::Text(txt) => {
                        match serde_json::from_str::<JsonRpcMessage>(&txt) {
                            Ok(msg) => this.on_message(msg).await,
                            Err(e) => warn!(%e, "Failed to parse incoming JSON-RPC"),
                        }
                    }
                    IncomingFrame::Close(code_reason) => {
                        warn!(?code_reason, "Connection closed by server");
                        break;
                    }
                    _ => {}
                }
            }
        });
    }

    async fn on_message(&self, msg: JsonRpcMessage) {
        match msg.clone() {
            JsonRpcMessage::V2(JsonRpcV2Message::Response(res)) => {
                let id_str = match &res.id {
                    Value::String(s) => s.clone(),
                    v => v.to_string(),
                };
                if let Some((_, tx)) = self.pending.remove(&id_str) {
                    let _ = tx.send(JsonRpcMessage::V2(JsonRpcV2Message::Response(res)));
                } else {
                    warn!(id = id_str, "Response with unknown id");
                }
            }
            JsonRpcMessage::V2(JsonRpcV2Message::Notification(notif)) => {
                if let Some(handler) = self.notify_handler.read().await.as_ref() {
                    handler(notif)
                }
            }
            JsonRpcMessage::V2(JsonRpcV2Message::Request(_req)) => {
                // Server initiated request – not supported for now
                warn!("Server-initiated request received; ignoring for now");
            }
        }
    }

    pub async fn set_notification_handler<F>(&self, f: F)
    where
        F: Fn(JsonRpcNotification) + Send + Sync + 'static,
    {
        *self.notify_handler.write().await = Some(Arc::new(f));
    }

    async fn initialize(&self, client_name: &str, client_version: &str) -> Result<()> {
        let req = handshake::build_initialize_request(&self.negotiator, Some(DEFAULT_VERSION), client_name, client_version, None).await?;
        let resp: McpInitializeResult = self.send_request_typed("initialize", &req.params.unwrap_or(json!({}))).await?;
        let negotiated = ProtocolVersion::new(resp.protocol_version)?;
        *self.protocol.write().await = McpProtocol::new(negotiated);
        Ok(())
    }

    pub fn inflight(&self) -> u64 {
        self.in_flight.load(Ordering::Relaxed)
    }

    pub async fn send_notification<T: Serialize>(&self, method: &str, params: &T) -> Result<()> {
        let p = self.protocol.read().await.clone();
        let notif = p.build_notification(method, params)?;
        let msg = JsonRpcMessage::V2(JsonRpcV2Message::Notification(notif));
        let text = serde_json::to_string(&msg)?;
        self.writer.send_text(&text).await
    }

    pub async fn send_request_raw(&self, method: &str, params: Value, timeout_dur: Duration) -> Result<JsonRpcMessage> {
        let id = uuid::Uuid::new_v4().to_string();
        let req = JsonRpcRequest::new(json!(id.clone()), method.to_string(), Some(params));
        let msg = JsonRpcMessage::V2(JsonRpcV2Message::Request(req));
        let text = serde_json::to_string(&msg)?;

        let (tx, rx) = oneshot::channel();
        self.pending.insert(id.clone(), tx);
        self.in_flight.fetch_add(1, Ordering::SeqCst);
        let send_res = self.writer.send_text(&text).await;
        if let Err(e) = send_res {
            self.pending.remove(&id);
            self.in_flight.fetch_sub(1, Ordering::SeqCst);
            return Err(e);
        }

        let res = timeout(timeout_dur, rx).await;
        self.in_flight.fetch_sub(1, Ordering::SeqCst);
        match res {
            Ok(Ok(msg)) => Ok(msg),
            Ok(Err(_canceled)) => Err(McpError::ConnectionClosed),
            Err(_elapsed) => {
                self.pending.remove(&id);
                Err(McpError::RequestTimeout(method.to_string()))
            }
        }
    }

    pub async fn send_request_typed<P, R>(&self, method: &str, params: &P) -> Result<R>
    where
        P: Serialize,
        R: DeserializeOwned,
    {
        let val = serde_json::to_value(params)?;
        let msg = self
            .send_request_raw(method, val, Duration::from_secs(30))
            .await?;
        parse_response_typed::<R>(&msg)
    }

    pub async fn close(&self) -> Result<()> {
        self.writer.close().await
    }
}

/// Simple connection pool for multiplexed MCP clients
pub struct McpClientPool {
    clients: Vec<Arc<McpConnection>>, // shared connections
}

impl McpClientPool {
    pub async fn connect(url: Url, size: usize) -> Result<Self> {
        let mut clients = Vec::with_capacity(size);
        for _ in 0..size.max(1) {
            let cfg = McpClientConfig::new(url.clone());
            clients.push(McpConnection::connect(&cfg).await?);
        }
        Ok(Self { clients })
    }

    /// Get the least busy connection (based on in-flight requests)
    pub fn acquire(&self) -> Arc<McpConnection> {
        let mut best = None;
        let mut best_load = u64::MAX;
        for c in &self.clients {
            let load = c.inflight();
            if load < best_load {
                best = Some(Arc::clone(c));
                best_load = load;
            }
        }
        best.expect("pool should contain at least one client")
    }
}

