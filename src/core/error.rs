use std::fmt;
use thiserror::Error;

pub type CodeGraphResult<T> = Result<T, CodeGraphError>;

#[derive(Error, Debug)]
pub enum CodeGraphError {
    #[error("Parse error: {message} at {location:?}")]
    Parse { 
        message: String, 
        location: Option<SourceLocation> 
    },

    #[error("Graph store error: {0}")]
    GraphStore(#[from] GraphStoreError),

    #[error("Vector index error: {0}")]
    VectorIndex(#[from] VectorIndexError),

    #[error("Embedding error: {0}")]
    Embedding(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Configuration error: {message}")]
    Config { message: String },

    #[error("Resource exhausted: {resource} - {details}")]
    ResourceExhausted { 
        resource: String, 
        details: String 
    },

    #[error("Timeout exceeded: {operation} took longer than {timeout_ms}ms")]
    Timeout { 
        operation: String, 
        timeout_ms: u64 
    },

    #[error("Concurrent access violation: {details}")]
    ConcurrentAccess { details: String },
}

#[derive(Error, Debug)]
pub enum GraphStoreError {
    #[error("Transaction failed: {reason}")]
    TransactionFailed { reason: String },

    #[error("Node not found: {node_id}")]
    NodeNotFound { node_id: String },

    #[error("Edge not found: from {from} to {to}")]
    EdgeNotFound { from: String, to: String },

    #[error("Schema validation failed: {details}")]
    SchemaValidation { details: String },

    #[error("Connection failed: {reason}")]
    ConnectionFailed { reason: String },

    #[error("Query execution failed: {query} - {reason}")]
    QueryFailed { query: String, reason: String },

    #[error("Constraint violation: {constraint} - {details}")]
    ConstraintViolation { constraint: String, details: String },
}

#[derive(Error, Debug)]
pub enum VectorIndexError {
    #[error("Index not found: {index_name}")]
    IndexNotFound { index_name: String },

    #[error("Dimension mismatch: expected {expected}, got {actual}")]
    DimensionMismatch { expected: usize, actual: usize },

    #[error("Search failed: {reason}")]
    SearchFailed { reason: String },

    #[error("Index build failed: {reason}")]
    IndexBuildFailed { reason: String },

    #[error("Vector operation failed: {operation} - {reason}")]
    VectorOpFailed { operation: String, reason: String },

    #[error("Similarity computation failed: {reason}")]
    SimilarityFailed { reason: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceLocation {
    pub file: String,
    pub line: u32,
    pub column: u32,
    pub offset: usize,
}

impl fmt::Display for SourceLocation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}:{}", self.file, self.line, self.column)
    }
}

impl CodeGraphError {
    pub fn parse_error(message: impl Into<String>, location: Option<SourceLocation>) -> Self {
        Self::Parse {
            message: message.into(),
            location,
        }
    }

    pub fn config_error(message: impl Into<String>) -> Self {
        Self::Config {
            message: message.into(),
        }
    }

    pub fn resource_exhausted(resource: impl Into<String>, details: impl Into<String>) -> Self {
        Self::ResourceExhausted {
            resource: resource.into(),
            details: details.into(),
        }
    }

    pub fn timeout(operation: impl Into<String>, timeout_ms: u64) -> Self {
        Self::Timeout {
            operation: operation.into(),
            timeout_ms,
        }
    }

    pub fn concurrent_access(details: impl Into<String>) -> Self {
        Self::ConcurrentAccess {
            details: details.into(),
        }
    }
}

impl GraphStoreError {
    pub fn transaction_failed(reason: impl Into<String>) -> Self {
        Self::TransactionFailed {
            reason: reason.into(),
        }
    }

    pub fn node_not_found(node_id: impl Into<String>) -> Self {
        Self::NodeNotFound {
            node_id: node_id.into(),
        }
    }

    pub fn edge_not_found(from: impl Into<String>, to: impl Into<String>) -> Self {
        Self::EdgeNotFound {
            from: from.into(),
            to: to.into(),
        }
    }

    pub fn schema_validation(details: impl Into<String>) -> Self {
        Self::SchemaValidation {
            details: details.into(),
        }
    }

    pub fn connection_failed(reason: impl Into<String>) -> Self {
        Self::ConnectionFailed {
            reason: reason.into(),
        }
    }

    pub fn query_failed(query: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::QueryFailed {
            query: query.into(),
            reason: reason.into(),
        }
    }

    pub fn constraint_violation(constraint: impl Into<String>, details: impl Into<String>) -> Self {
        Self::ConstraintViolation {
            constraint: constraint.into(),
            details: details.into(),
        }
    }
}

impl VectorIndexError {
    pub fn index_not_found(index_name: impl Into<String>) -> Self {
        Self::IndexNotFound {
            index_name: index_name.into(),
        }
    }

    pub fn dimension_mismatch(expected: usize, actual: usize) -> Self {
        Self::DimensionMismatch { expected, actual }
    }

    pub fn search_failed(reason: impl Into<String>) -> Self {
        Self::SearchFailed {
            reason: reason.into(),
        }
    }

    pub fn index_build_failed(reason: impl Into<String>) -> Self {
        Self::IndexBuildFailed {
            reason: reason.into(),
        }
    }

    pub fn vector_op_failed(operation: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::VectorOpFailed {
            operation: operation.into(),
            reason: reason.into(),
        }
    }

    pub fn similarity_failed(reason: impl Into<String>) -> Self {
        Self::SimilarityFailed {
            reason: reason.into(),
        }
    }
}