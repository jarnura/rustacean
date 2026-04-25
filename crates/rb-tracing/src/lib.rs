use tracing_subscriber::{fmt, layer::SubscriberExt as _, util::SubscriberInitExt as _, EnvFilter};

#[derive(Debug, thiserror::Error)]
pub enum TracingError {
    #[error("failed to set global subscriber: {0}")]
    Subscriber(String),
}

/// Flushes any pending spans/logs when dropped. Hold in `main()`.
pub struct TracingGuard;

/// Initialize structured logging. `RB_LOG_FORMAT=pretty` for dev terminals; default is JSON.
/// Full OTLP export wired in RUSAA-23.
///
/// # Errors
///
/// Returns [`TracingError::Subscriber`] if a global tracing subscriber is already installed.
pub fn init(_service_name: &str) -> Result<TracingGuard, TracingError> {
    let log_format = std::env::var("RB_LOG_FORMAT").unwrap_or_else(|_| "json".to_string());

    if log_format == "json" {
        tracing_subscriber::registry()
            .with(EnvFilter::from_default_env())
            .with(fmt::layer().json())
            .try_init()
            .map_err(|e| TracingError::Subscriber(e.to_string()))?;
    } else {
        tracing_subscriber::registry()
            .with(EnvFilter::from_default_env())
            .with(fmt::layer().pretty())
            .try_init()
            .map_err(|e| TracingError::Subscriber(e.to_string()))?;
    }

    Ok(TracingGuard)
}
