use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use thiserror::Error;

/// Top-level application error type.
///
/// Every variant maps to an HTTP status code, a stable machine-readable
/// `error` string, and a human-readable message.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum AppError {
    #[error("not found")]
    NotFound,
    #[error("internal server error")]
    Internal(#[from] anyhow::Error),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, code, message) = match &self {
            AppError::NotFound => (StatusCode::NOT_FOUND, "not_found", self.to_string()),
            AppError::Internal(e) => {
                tracing::error!(error = %e, "unhandled internal error");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "internal_error",
                    "internal server error".to_owned(),
                )
            }
        };
        (
            status,
            Json(json!({ "error": code, "message": message })),
        )
            .into_response()
    }
}
