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
}

pub type Result<T> = std::result::Result<T, CodeGraphError>;