use opentelemetry::{global, trace::TracerProvider as _};
use opentelemetry_otlp::{SpanExporter, WithExportConfig as _};
use opentelemetry_sdk::{propagation::TraceContextPropagator, trace::SdkTracerProvider, Resource};
use tracing_opentelemetry::OpenTelemetryLayer;
use tracing_subscriber::{fmt, layer::SubscriberExt as _, util::SubscriberInitExt as _, EnvFilter};

#[derive(Debug, thiserror::Error)]
pub enum TracingError {
    #[error("failed to initialize OTLP exporter: {0}")]
    OtlpInit(String),
    #[error("failed to set global subscriber: {0}")]
    Subscriber(String),
}

/// Flushes pending spans on drop. Hold for the process lifetime inside `main()`.
pub struct TracingGuard {
    provider: SdkTracerProvider,
}

impl Drop for TracingGuard {
    fn drop(&mut self) {
        if let Err(e) = self.provider.shutdown() {
            eprintln!("tracer provider shutdown failed: {e}");
        }
    }
}

/// Initialize OTLP tracing and structured logging.
///
/// Installs a W3C `TraceContext` propagator, a batched OTLP gRPC span exporter,
/// and a `tracing-subscriber` registry with JSON (production) or `pretty`
/// (dev) formatting. Call once at binary startup before any `tracing` macros.
///
/// Configuration via env vars:
/// - `OTEL_EXPORTER_OTLP_ENDPOINT` — gRPC endpoint (default: `http://localhost:4317`)
/// - `RUST_LOG`                     — log filter (default: `info`)
/// - `RB_LOG_FORMAT`                — `json` (default) or `pretty`
///
/// # Errors
///
/// Returns [`TracingError::OtlpInit`] if the OTLP exporter fails to build.
/// Returns [`TracingError::Subscriber`] if a global subscriber is already set.
pub fn init(service_name: &str) -> Result<TracingGuard, TracingError> {
    global::set_text_map_propagator(TraceContextPropagator::new());

    let endpoint = std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT")
        .unwrap_or_else(|_| "http://localhost:4317".to_string());

    let exporter = SpanExporter::builder()
        .with_tonic()
        .with_endpoint(endpoint)
        .build()
        .map_err(|e| TracingError::OtlpInit(e.to_string()))?;

    let resource = Resource::builder()
        .with_service_name(service_name.to_owned())
        .build();

    let provider = SdkTracerProvider::builder()
        .with_batch_exporter(exporter)
        .with_resource(resource)
        .build();

    global::set_tracer_provider(provider.clone());

    let log_format = std::env::var("RB_LOG_FORMAT").unwrap_or_else(|_| "json".to_string());

    if log_format == "pretty" {
        tracing_subscriber::registry()
            .with(EnvFilter::from_default_env())
            .with(fmt::layer().pretty())
            .with(OpenTelemetryLayer::new(provider.tracer("rb-tracing")))
            .try_init()
            .map_err(|e| TracingError::Subscriber(e.to_string()))?;
    } else {
        tracing_subscriber::registry()
            .with(EnvFilter::from_default_env())
            .with(fmt::layer().json())
            .with(OpenTelemetryLayer::new(provider.tracer("rb-tracing")))
            .try_init()
            .map_err(|e| TracingError::Subscriber(e.to_string()))?;
    }

    Ok(TracingGuard { provider })
}
