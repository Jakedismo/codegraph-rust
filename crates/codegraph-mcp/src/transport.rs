use crate::{heartbeat::HeartbeatManager, message::JsonRpcMessage, McpError, Result};
use futures::{stream::SplitSink, SinkExt, StreamExt};
use std::{sync::Arc, time::Duration};
use tokio::sync::{broadcast, RwLock};
use tokio_tungstenite::{connect_async, tungstenite::Message as WsMessage, WebSocketStream};
use tracing::{error, warn};
use url::Url;

/// Abstraction over the underlying transport for MCP
#[async_trait::async_trait]
pub trait Transport: Send + Sync {
    async fn send_text(&self, text: &str) -> Result<()>;
    async fn send_json(&self, msg: &JsonRpcMessage) -> Result<()> {
        let text = serde_json::to_string(msg)?;
        self.send_text(&text).await
    }
    async fn close(&self) -> Result<()>;
}

/// Handle to receive frames from the server
pub type Incoming = broadcast::Receiver<IncomingFrame>;

#[derive(Debug, Clone)]
pub enum IncomingFrame {
    Text(String),
    Binary(Vec<u8>),
    Pong(Vec<u8>),
    Close(Option<(u16, String)>),
}

/// WebSocket transport implementation using tokio-tungstenite
pub struct WebSocketTransport {
    writer: Arc<
        RwLock<
            SplitSink<
                WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>,
                WsMessage,
            >,
        >,
    >,
    #[allow(dead_code)]
    incoming: Incoming,
    #[allow(dead_code)]
    heartbeat: HeartbeatManager,
}

impl WebSocketTransport {
    pub async fn connect(
        url: &Url,
        heartbeat: Option<HeartbeatManager>,
    ) -> Result<(Arc<Self>, Incoming)> {
        let (stream, _resp) = connect_async(url.as_str()).await.map_err(McpError::from)?;
        let (sink, mut stream) = stream.split();

        let (tx, rx) = broadcast::channel::<IncomingFrame>(128);

        // If not provided, create a disabled heartbeat manager
        let mut hb = heartbeat.unwrap_or_default();
        let hb_clone = hb.clone();
        let writer = Arc::new(RwLock::new(sink));
        let writer_clone = writer.clone();

        // Reader task
        tokio::spawn(async move {
            while let Some(frame) = stream.next().await {
                match frame {
                    Ok(WsMessage::Text(txt)) => {
                        let _ = tx.send(IncomingFrame::Text(
                            String::from_utf8_lossy(txt.as_bytes()).to_string(),
                        ));
                    }
                    Ok(WsMessage::Binary(b)) => {
                        let _ = tx.send(IncomingFrame::Binary(b.to_vec()));
                    }
                    Ok(WsMessage::Pong(payload)) => {
                        // Notify heartbeat manager (sequence encoded as u64 ASCII if present)
                        if let Ok(s) = std::str::from_utf8(&payload) {
                            if let Ok(seq) = s.parse::<u64>() {
                                hb_clone.on_pong(seq).await;
                            }
                        }
                        let _ = tx.send(IncomingFrame::Pong(payload.to_vec()));
                    }
                    Ok(WsMessage::Ping(payload)) => {
                        // Respond to ping immediately
                        if let Err(e) = writer_clone
                            .write()
                            .await
                            .send(WsMessage::Pong(payload))
                            .await
                        {
                            error!(?e, "Failed to send PONG");
                            break;
                        }
                    }
                    Ok(WsMessage::Close(frame)) => {
                        let _ = tx.send(IncomingFrame::Close(
                            frame.map(|f| (f.code.into(), f.reason.to_string())),
                        ));
                        break;
                    }
                    Err(e) => {
                        error!(error = %e, "WebSocket read error");
                        break;
                    }
                    _ => {}
                }
            }
        });

        // If heartbeat is enabled, start ping loop using websocket-level ping frames
        if hb.is_enabled() {
            let writer_clone = writer.clone();
            hb.start(move |seq| {
                let writer_clone = writer_clone.clone();
                tokio::spawn(async move {
                    // Use websocket Ping with ASCII-encoded sequence number
                    let payload = seq.to_string().into_bytes();
                    if let Err(e) = writer_clone
                        .write()
                        .await
                        .send(WsMessage::Ping(payload.into()))
                        .await
                    {
                        warn!(?e, "Failed to send heartbeat ping");
                    }
                })
            })
            .await?
        }

        let transport = Arc::new(Self {
            writer,
            incoming: rx.resubscribe(),
            heartbeat: hb,
        });
        Ok((transport, rx.resubscribe()))
    }
}

#[async_trait::async_trait]
impl Transport for WebSocketTransport {
    async fn send_text(&self, text: &str) -> Result<()> {
        self.writer
            .write()
            .await
            .send(WsMessage::Text(text.to_string().into()))
            .await
            .map_err(McpError::from)
    }

    async fn close(&self) -> Result<()> {
        self.writer
            .write()
            .await
            .send(WsMessage::Close(None))
            .await
            .map_err(McpError::from)
    }
}

/// Backoff helper for reconnect logic
pub fn backoff_durations(max_retries: usize) -> impl Iterator<Item = Duration> {
    // Exponential with jitter
    (0..max_retries).map(|i| {
        let base = 2u64.saturating_pow(i.min(6) as u32); // cap exponent
        let jitter = fastrand::u64(0..(base * 50 + 1));
        Duration::from_millis(200 * base + jitter)
    })
}
