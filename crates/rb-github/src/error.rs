use thiserror::Error;

#[derive(Debug, Error)]
pub enum GhError {
    #[error("JWT mint failed: {0}")]
    JwtMint(#[from] jsonwebtoken::errors::Error),

    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("GitHub API error {status}: {body}")]
    ApiError { status: u16, body: String },

    #[error("base64 decode error: {0}")]
    Base64(#[from] base64::DecodeError),

    #[error("PEM key invalid: {0}")]
    InvalidKey(String),

    #[error("webhook signature format invalid")]
    BadSignatureFormat,

    #[error("webhook signature mismatch")]
    SignatureMismatch,

    #[error("webhook delivery already seen (replay)")]
    Replay,
}
