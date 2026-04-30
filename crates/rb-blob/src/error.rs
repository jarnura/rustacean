use thiserror::Error;
use uuid::Uuid;

/// Typed S3 error kinds — avoids stringly-typed error matching in callers
/// (e.g. Wave-5 retry logic that needs to distinguish transient vs permanent).
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg(feature = "s3")]
pub enum S3ErrorKind {
    /// Object or bucket not found (HTTP 404).
    NotFound,
    /// Request throttled / rate-limited (HTTP 429 / `SlowDown`).
    Throttled,
    /// Authentication or authorization failure (HTTP 401/403).
    Auth,
    /// Network-level or connection error.
    Network,
    /// Any other S3 / SDK error — the raw message is preserved.
    Other(String),
}

#[cfg(feature = "s3")]
impl std::fmt::Display for S3ErrorKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound => write!(f, "not found"),
            Self::Throttled => write!(f, "throttled"),
            Self::Auth => write!(f, "auth error"),
            Self::Network => write!(f, "network error"),
            Self::Other(msg) => write!(f, "{msg}"),
        }
    }
}

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum BlobError {
    #[error("blob not found: tenant={tenant_id} sha256={sha256}")]
    NotFound { tenant_id: Uuid, sha256: String },

    #[error("tenant mismatch: blob belongs to a different tenant")]
    TenantMismatch,

    #[error("sha256 mismatch: expected={expected} got={got}")]
    Sha256Mismatch { expected: String, got: String },

    #[error("size mismatch: expected={expected} got={got}")]
    SizeMismatch { expected: u64, got: u64 },

    #[error("invalid sha256 hex: {0}")]
    InvalidSha256(String),

    #[error("invalid blob URI: {0}")]
    InvalidUri(String),

    #[error("unknown backend: {0}")]
    UnknownBackend(String),

    /// Construction-time misconfiguration (e.g. inaccessible S3 bucket).
    #[error("configuration error: {0}")]
    Configuration(String),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[cfg(feature = "s3")]
    #[error("S3 error: {0}")]
    S3(S3ErrorKind),
}
