use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use codegraph_core::CodeGraphError;
use serde_json::json;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ApiError {
    #[error("CodeGraph error: {0}")]
    CodeGraph(#[from] CodeGraphError),

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Internal server error: {0}")]
    Internal(String),

    #[error("Bad request: {0}")]
    BadRequest(String),

    #[error("Service unavailable: {0}")]
    ServiceUnavailable(String),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, error_message) = match self {
            ApiError::CodeGraph(ref err) => match err {
                CodeGraphError::NodeNotFound(_) => (StatusCode::NOT_FOUND, self.to_string()),
                CodeGraphError::InvalidOperation(_) => (StatusCode::BAD_REQUEST, self.to_string()),
                _ => (StatusCode::INTERNAL_SERVER_ERROR, self.to_string()),
            },
            ApiError::Validation(_) => (StatusCode::BAD_REQUEST, self.to_string()),
            ApiError::NotFound(_) => (StatusCode::NOT_FOUND, self.to_string()),
            ApiError::BadRequest(_) => (StatusCode::BAD_REQUEST, self.to_string()),
            ApiError::Internal(_) => (StatusCode::INTERNAL_SERVER_ERROR, self.to_string()),
            ApiError::ServiceUnavailable(_) => (StatusCode::SERVICE_UNAVAILABLE, self.to_string()),
        };

        let body = Json(json!({
            "error": error_message,
            "status": status.as_u16()
        }));

        (status, body).into_response()
    }
}

pub type ApiResult<T> = Result<T, ApiError>;
