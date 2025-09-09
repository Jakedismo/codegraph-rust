use thiserror::Error;

#[derive(Error, Debug)]
pub enum CodeGraphError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Parse error: {0}")]
    Parse(String),

    #[error("Graph error: {0}")]
    Graph(String),

    #[error("Vector error: {0}")]
    Vector(String),

    #[error("Database error: {0}")]
    Database(String),

    #[error("Node not found: {0}")]
    NodeNotFound(String),

    #[error("Invalid operation: {0}")]
    InvalidOperation(String),

    #[error("Transaction error: {0}")]
    Transaction(String),

    #[error("Version error: {0}")]
    Version(String),

    #[error("Conflict error: {0}")]
    Conflict(String),

    #[error("Recovery error: {0}")]
    Recovery(String),

    #[error("Configuration error: {0}")]
    Configuration(String),

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("Not found: {0}")]
    NotFound(String),
}

pub type Result<T> = std::result::Result<T, CodeGraphError>;