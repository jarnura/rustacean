#[derive(Debug, thiserror::Error)]
pub enum EmailError {
    #[error("SMTP transport error: {0}")]
    Smtp(String),
    #[error("template rendering failed: {0}")]
    Template(String),
    #[error("unknown email transport '{0}': expected smtp, console, or noop")]
    UnknownTransport(String),
}
