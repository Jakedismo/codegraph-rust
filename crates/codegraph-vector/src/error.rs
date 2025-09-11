use thiserror::Error;

#[derive(Error, Debug)]
pub enum VectorError {
    #[error("Dimension mismatch: expected {0}, got {1}")]
    DimensionMismatch(usize, usize),

    #[error("Batch size mismatch")]
    BatchSizeMismatch,

    #[error("Vector is empty")]
    EmptyVector,

    #[error("Index out of bounds: {0}")]
    IndexOutOfBounds(usize),

    #[error("Invalid vector operation: {0}")]
    InvalidOperation(String),

    #[error("SIMD operation failed: {0}")]
    SimdError(String),

    #[error("Embedding error: {0}")]
    Embedding(String),

    #[error("Search error: {0}")]
    Search(String),
}

impl From<VectorError> for codegraph_core::CodeGraphError {
    fn from(err: VectorError) -> Self {
        codegraph_core::CodeGraphError::Vector(err.to_string())
    }
}
