// ABOUTME: Error type conversions for NAPI boundaries
// ABOUTME: Converts Rust errors to NAPI::Error with user-friendly messages

use codegraph_core::CodeGraphError;
use napi::Error as NapiError;

/// Generic error conversion function for any Display type
pub fn to_napi_error(err: impl std::fmt::Display) -> NapiError {
    NapiError::from_reason(err.to_string())
}

/// Specialized conversion from CodeGraphError to NapiError
#[allow(dead_code)]
pub fn from_codegraph_error(err: CodeGraphError) -> NapiError {
    let message = match err {
        CodeGraphError::NotFound(msg) => {
            format!("Resource not found: {}", msg)
        }
        CodeGraphError::Configuration(msg) => {
            format!("Configuration error: {}", msg)
        }
        CodeGraphError::Network(msg) => {
            format!("Network error: {}", msg)
        }
        // Generic fallback for all other error variants
        other => other.to_string(),
    };
    NapiError::from_reason(message)
}
