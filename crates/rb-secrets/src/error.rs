use thiserror::Error;

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum SecretError {
    #[error("secret not found: {key}")]
    NotFound { key: String },
    #[error("failed to read secret file for key '{key}': {source}")]
    Io {
        key: String,
        #[source]
        source: std::io::Error,
    },
}
