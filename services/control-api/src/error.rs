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
    #[error("email already registered")]
    EmailTaken,
    #[error("password must be at least 12 characters")]
    WeakPassword,
    #[error("invalid email address")]
    InvalidEmail,
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
    #[error("auth error: {0}")]
    Auth(#[from] rb_auth::AuthError),
    #[error("email error: {0}")]
    Email(#[from] rb_email::EmailError),
    #[error("internal server error")]
    Internal(#[from] anyhow::Error),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, code, message) = match &self {
            AppError::NotFound => (StatusCode::NOT_FOUND, "not_found", self.to_string()),
            AppError::EmailTaken => (StatusCode::CONFLICT, "email_taken", self.to_string()),
            AppError::WeakPassword => {
                (StatusCode::BAD_REQUEST, "weak_password", self.to_string())
            }
            AppError::InvalidEmail => {
                (StatusCode::UNPROCESSABLE_ENTITY, "invalid_email", self.to_string())
            }
            AppError::Database(e) => {
                tracing::error!(error = %e, "database error");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "internal_error",
                    "internal server error".to_owned(),
                )
            }
            AppError::Auth(rb_auth::AuthError::RateLimited { retry_after_secs }) => (
                StatusCode::TOO_MANY_REQUESTS,
                "rate_limited",
                format!("too many requests, retry after {retry_after_secs}s"),
            ),
            AppError::Auth(e) => {
                tracing::error!(error = %e, "auth error");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "internal_error",
                    "internal server error".to_owned(),
                )
            }
            AppError::Email(e) => {
                tracing::warn!(error = %e, "email delivery error");
                // Non-fatal — signup succeeds even if email fails
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "internal_error",
                    "internal server error".to_owned(),
                )
            }
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
