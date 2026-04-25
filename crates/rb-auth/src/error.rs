use thiserror::Error;

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum AuthError {
    #[error("argon2 error: {0}")]
    Argon2(String),
    #[error("database error: {0}")]
    Sqlx(#[from] sqlx::Error),
    #[error("rate limited: retry after {retry_after_secs}s")]
    RateLimited { retry_after_secs: u64 },
}
