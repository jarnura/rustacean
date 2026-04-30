use thiserror::Error;
use uuid::Uuid;

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

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[cfg(feature = "s3")]
    #[error("S3 error: {0}")]
    S3(String),
}
