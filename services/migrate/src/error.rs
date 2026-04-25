use thiserror::Error;

#[derive(Debug, Error)]
pub enum MigrateError {
    #[error("database: {0}")]
    Db(#[from] sqlx::Error),

    #[error("io: {0}")]
    Io(#[from] std::io::Error),

    #[error("invalid migration filename '{0}': expected NNN_description.sql")]
    InvalidFilename(String),

    #[error("checksum mismatch for v{version}: stored={stored}, actual={actual}")]
    ChecksumMismatch {
        version: i32,
        stored: String,
        actual: String,
    },

    #[error("advisory lock unavailable for schema '{0}' — another runner is active")]
    LockUnavailable(String),

    #[error("migrations directory not found: {0}")]
    MissingDir(String),
}
