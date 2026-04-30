use std::time::Duration;

use chrono::{DateTime, Utc};
use uuid::Uuid;

/// Opaque monotonic event identifier used for `Last-Event-Id` replay.
///
/// Format: `{timestamp_millis_hex_16}{uuid_simple_32}` — lexicographically
/// sortable by time, globally unique.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct EventId(pub(crate) String);

impl EventId {
    #[must_use]
    pub fn new() -> Self {
        let ts = Utc::now().timestamp_millis() as u64;
        let rand = Uuid::new_v4().simple();
        Self(format!("{ts:016x}{rand}"))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for EventId {
    fn default() -> Self {
        Self::new()
    }
}

impl From<String> for EventId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for EventId {
    fn from(s: &str) -> Self {
        Self(s.to_owned())
    }
}

/// A single SSE event ready to transmit over the wire.
#[derive(Debug, Clone)]
pub struct SseEnvelope {
    pub id: EventId,
    /// SSE `event:` field (e.g. `"ingest.status"`, `"stream-reset"`).
    pub event: String,
    /// SSE `retry:` advisory in milliseconds. Absent means browser default.
    pub retry_ms: Option<u64>,
    /// SSE `data:` payload — JSON-serialised application event.
    pub data: String,
    pub created_at: DateTime<Utc>,
}

impl SseEnvelope {
    #[must_use]
    pub fn new(event: impl Into<String>, data: impl Into<String>) -> Self {
        Self {
            id: EventId::new(),
            event: event.into(),
            retry_ms: Some(5_000),
            data: data.into(),
            created_at: Utc::now(),
        }
    }

    /// Synthetic advisory sent to a lagging client before closing the stream.
    /// The browser will reconnect with `Last-Event-Id` and replay from the ring buffer.
    #[must_use]
    pub fn stream_reset() -> Self {
        Self {
            id: EventId::new(),
            event: "stream-reset".to_owned(),
            retry_ms: Some(5_000),
            data: "{}".to_owned(),
            created_at: Utc::now(),
        }
    }

    /// Convert to an axum SSE wire event.
    pub fn to_axum_event(&self) -> axum::response::sse::Event {
        let mut ev = axum::response::sse::Event::default()
            .id(self.id.as_str())
            .event(self.event.as_str())
            .data(self.data.as_str());
        if let Some(ms) = self.retry_ms {
            ev = ev.retry(Duration::from_millis(ms));
        }
        ev
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_id_has_expected_length() {
        let id = EventId::new();
        // 16 hex (timestamp) + 32 hex (uuid simple) = 48 chars
        assert_eq!(id.as_str().len(), 48);
    }

    #[test]
    fn event_id_is_unique() {
        let a = EventId::new();
        let b = EventId::new();
        assert_ne!(a, b);
    }

    #[test]
    fn event_id_from_string_roundtrip() {
        let s = "0000000000000001deadbeef1234567890abcdefdeadbeef12345678".to_owned();
        let id = EventId::from(s.clone());
        assert_eq!(id.as_str(), s);
    }

    #[test]
    fn sse_envelope_has_retry_field() {
        let env = SseEnvelope::new("test", r#"{"x":1}"#);
        assert_eq!(env.retry_ms, Some(5_000));
    }

    #[test]
    fn stream_reset_envelope_has_correct_event_name() {
        let env = SseEnvelope::stream_reset();
        assert_eq!(env.event, "stream-reset");
    }
}
