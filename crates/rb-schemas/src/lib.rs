use uuid::Uuid;

mod ingest {
    #![allow(clippy::all, clippy::pedantic)]
    include!(concat!(env!("OUT_DIR"), "/rust_brain.v1.rs"));
}

pub use ingest::{IngestRequest, IngestStatus, IngestStatusEvent};

/// Newtype over [`Uuid`] representing a tenant identifier.
/// Prost-generated event types are re-exported from this crate (see [`IngestRequest`] etc.).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(transparent)]
pub struct TenantId(Uuid);

impl TenantId {
    #[must_use]
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    #[must_use]
    pub fn as_uuid(&self) -> Uuid {
        self.0
    }
}

impl From<Uuid> for TenantId {
    fn from(uuid: Uuid) -> Self {
        Self(uuid)
    }
}

impl std::str::FromStr for TenantId {
    type Err = uuid::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(s.parse::<Uuid>()?))
    }
}

impl Default for TenantId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for TenantId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tenant_id_roundtrip_json() {
        let id = TenantId::new();
        let json = serde_json::to_string(&id).unwrap();
        let decoded: TenantId = serde_json::from_str(&json).unwrap();
        assert_eq!(id, decoded);
    }

    #[test]
    fn tenant_id_display_matches_uuid() {
        let id = TenantId::new();
        assert_eq!(id.to_string(), id.as_uuid().to_string());
    }

    #[test]
    fn ingest_request_fields_accessible() {
        let req = IngestRequest {
            tenant_id: "tenant-123".to_string(),
            event_id: "evt-456".to_string(),
            source: "github".to_string(),
            payload: vec![1, 2, 3],
            created_at_ms: 1_700_000_000_000,
        };
        assert_eq!(req.source, "github");
        assert_eq!(req.payload, vec![1u8, 2, 3]);
    }

    #[test]
    fn ingest_status_roundtrip() {
        let status = IngestStatus::Done;
        let as_i32 = status as i32;
        let back = IngestStatus::try_from(as_i32).unwrap();
        assert_eq!(status, back);
    }

    #[test]
    fn ingest_status_unspecified_is_zero() {
        assert_eq!(IngestStatus::Unspecified as i32, 0);
    }

    #[test]
    fn ingest_status_event_fields_accessible() {
        let ev = IngestStatusEvent {
            ingest_request_id: "req-1".to_string(),
            tenant_id: "tenant-1".to_string(),
            status: IngestStatus::Processing as i32,
            error_message: String::new(),
            occurred_at_ms: 1_700_000_001_000,
        };
        assert_eq!(ev.status, IngestStatus::Processing as i32);
        assert!(ev.error_message.is_empty());
    }
}
