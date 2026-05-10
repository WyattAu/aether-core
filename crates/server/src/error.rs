#![deny(unsafe_code)]

use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Serialize;

#[derive(Debug, Serialize)]
/// Standard error response body returned by the API.
pub struct ErrorResponse {
    /// Machine-readable error code.
    pub error: String,
    /// Human-readable error message.
    pub message: String,
}

#[derive(Debug)]
/// Errors that can occur while handling API requests.
pub enum ApiError {
    /// The requested resource was not found.
    NotFound(String),
    /// The request was malformed or invalid.
    BadRequest(String),
    /// An unexpected internal error occurred.
    InternalError(String),
    /// The requested endpoint is not yet implemented.
    NotImplemented(String),
}

impl ApiError {
    /// Creates a `NotImplemented` error for the given endpoint.
    pub fn not_implemented(endpoint: &str) -> Self {
        Self::NotImplemented(format!("{endpoint} is not yet implemented"))
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, error, message) = match self {
            ApiError::NotFound(msg) => (StatusCode::NOT_FOUND, "not_found", msg),
            ApiError::BadRequest(msg) => (StatusCode::BAD_REQUEST, "bad_request", msg),
            ApiError::InternalError(msg) => {
                (StatusCode::INTERNAL_SERVER_ERROR, "internal_error", msg)
            }
            ApiError::NotImplemented(msg) => (StatusCode::NOT_IMPLEMENTED, "not_implemented", msg),
        };
        (
            status,
            Json(ErrorResponse {
                error: error.to_owned(),
                message,
            }),
        )
            .into_response()
    }
}
