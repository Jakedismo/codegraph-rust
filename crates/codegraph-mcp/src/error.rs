use thiserror::Error;

#[derive(Error, Debug)]
pub enum McpError {
    #[error("WebSocket error: {0}")]
    WebSocket(#[from] tokio_tungstenite::tungstenite::Error),

    #[error("JSON serialization error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Invalid message format: {0}")]
    InvalidMessage(String),

    #[error("Protocol version mismatch: expected {expected}, got {actual}")]
    VersionMismatch { expected: String, actual: String },

    #[error("Connection closed")]
    ConnectionClosed,

    #[error("Connection timeout")]
    ConnectionTimeout,

    #[error("Heartbeat timeout")]
    HeartbeatTimeout,

    #[error("Invalid request ID: {0}")]
    InvalidRequestId(String),

    #[error("Request timeout: {0}")]
    RequestTimeout(String),

    #[error("Protocol error: {0}")]
    Protocol(String),

    #[error("Transport error: {0}")]
    Transport(String),

    #[error("Authentication failed: {0}")]
    Authentication(String),

    #[error("Rate limit exceeded")]
    RateLimit,

    #[error("Invalid URL: {0}")]
    InvalidUrl(#[from] url::ParseError),

    #[error("Task join error: {0}")]
    TaskJoin(#[from] tokio::task::JoinError),

    #[error("Registration error: {0}")]
    Registration(String),

    #[error("Scheduling error: {0}")]
    Scheduling(String),

    #[error("Aggregation error: {0}")]
    Aggregation(String),

    #[error("Conflict resolution error: {0}")]
    Conflict(String),
}

pub type Result<T> = std::result::Result<T, McpError>;
