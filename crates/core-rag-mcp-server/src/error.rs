use thiserror::Error;

pub type Result<T> = std::result::Result<T, CoreRagError>;

#[derive(Error, Debug)]
pub enum CoreRagError {
    #[error("Configuration error: {0}")]
    Config(String),

    #[error("CodeGraph error: {0}")]
    CodeGraph(#[from] codegraph_core::Error),

    #[error("Vector search error: {0}")]
    VectorSearch(String),

    #[error("Database error: {0}")]
    Database(String),

    #[error("Parser error: {0}")]
    Parser(String),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Invalid input: {0}")]
    InvalidInput(String),

    #[error("Service unavailable: {0}")]
    ServiceUnavailable(String),
}

impl CoreRagError {
    pub fn config(msg: impl Into<String>) -> Self {
        Self::Config(msg.into())
    }

    pub fn vector_search(msg: impl Into<String>) -> Self {
        Self::VectorSearch(msg.into())
    }

    pub fn database(msg: impl Into<String>) -> Self {
        Self::Database(msg.into())
    }

    pub fn parser(msg: impl Into<String>) -> Self {
        Self::Parser(msg.into())
    }

    pub fn not_found(msg: impl Into<String>) -> Self {
        Self::NotFound(msg.into())
    }

    pub fn invalid_input(msg: impl Into<String>) -> Self {
        Self::InvalidInput(msg.into())
    }

    pub fn service_unavailable(msg: impl Into<String>) -> Self {
        Self::ServiceUnavailable(msg.into())
    }
}

impl From<CoreRagError> for rmcp::ErrorData {
    fn from(err: CoreRagError) -> Self {
        match err {
            CoreRagError::NotFound(msg) => rmcp::ErrorData::internal_error(msg),
            CoreRagError::InvalidInput(msg) => rmcp::ErrorData::internal_error(msg),
            CoreRagError::ServiceUnavailable(msg) => rmcp::ErrorData::internal_error(msg),
            _ => rmcp::ErrorData::internal_error(err.to_string()),
        }
    }
}