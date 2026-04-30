use chrono::{DateTime, Utc};
use rb_schemas::TenantId;
use uuid::Uuid;

// ── Header key constants (locked per ADR-006 §3.1) ──────────────────────────
pub const HEADER_TENANT_ID: &str = "x-rb-tenant-id";
pub const HEADER_EVENT_ID: &str = "x-rb-event-id";
pub const HEADER_SCHEMA_VERSION: &str = "x-rb-schema-version";
pub const HEADER_BLOB_REF: &str = "x-rb-blob-ref";
pub const HEADER_ATTEMPT: &str = "x-rb-attempt";
pub const HEADER_TRACEPARENT: &str = "traceparent";
pub const HEADER_TRACESTATE: &str = "tracestate";
pub const HEADER_PROCESS_AFTER_MS: &str = "x-rb-process-after-ms";
pub const HEADER_DLQ_REASON: &str = "x-rb-dlq-reason";
pub const HEADER_DLQ_AT: &str = "x-rb-dlq-at";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SchemaVersion {
    V1,
}

impl SchemaVersion {
    pub const V1_STR: &'static str = "rust_brain.v1";

    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            SchemaVersion::V1 => Self::V1_STR,
        }
    }
}

impl std::fmt::Display for SchemaVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for SchemaVersion {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            Self::V1_STR => Ok(SchemaVersion::V1),
            other => Err(format!("unknown schema version: {other}")),
        }
    }
}

/// Trace context extracted from W3C `traceparent`/`tracestate` headers.
#[derive(Debug, Clone, Default)]
pub struct TraceContext {
    pub traceparent: String,
    pub tracestate: String,
}

/// Transport metadata populated by `Consumer::next()`.
/// Callers constructing envelopes for production should leave this as `Default::default()`.
#[doc(hidden)]
#[derive(Debug, Clone, Default)]
pub struct EnvelopeMeta {
    pub topic: String,
    pub partition: i32,
    pub offset: i64,
    pub attempt: u32,
}

/// Typed envelope wrapping a protobuf message with all required transport metadata.
#[derive(Debug, Clone)]
pub struct EventEnvelope<E: prost::Message> {
    pub tenant_id: TenantId,
    pub event_id: Uuid,
    pub schema_version: SchemaVersion,
    pub trace_context: Option<TraceContext>,
    /// Optional URI pointer to a blob-store object (present when payload exceeded inline limit).
    pub blob_ref: Option<String>,
    pub created_at: DateTime<Utc>,
    pub payload: E,
    /// Internal transport metadata; do not inspect in application code.
    #[doc(hidden)]
    pub _meta: EnvelopeMeta,
}

impl<E: prost::Message + Default> EventEnvelope<E> {
    /// Create a new outbound envelope with a fresh `event_id` and `schema_version = V1`.
    #[must_use]
    pub fn new(tenant_id: TenantId, payload: E) -> Self {
        Self {
            tenant_id,
            event_id: Uuid::new_v4(),
            schema_version: SchemaVersion::V1,
            trace_context: None,
            blob_ref: None,
            created_at: Utc::now(),
            payload,
            _meta: EnvelopeMeta::default(),
        }
    }

    /// Override the generated `event_id` (useful for idempotency testing).
    #[must_use]
    pub fn with_event_id(mut self, event_id: Uuid) -> Self {
        self.event_id = event_id;
        self
    }

    /// Attach a W3C trace context.
    #[must_use]
    pub fn with_trace_context(mut self, tc: TraceContext) -> Self {
        self.trace_context = Some(tc);
        self
    }

    /// Attach a blob-store pointer URI.
    #[must_use]
    pub fn with_blob_ref(mut self, blob_ref: impl Into<String>) -> Self {
        self.blob_ref = Some(blob_ref.into());
        self
    }
}

/// Result of a successful produce operation.
#[derive(Debug, Clone)]
pub struct DeliveryReport {
    pub topic: String,
    pub partition: i32,
    pub offset: i64,
}
