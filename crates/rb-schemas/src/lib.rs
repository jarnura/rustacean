use uuid::Uuid;

/// Newtype over [`Uuid`] representing a tenant identifier.
/// Prost-generated event types added in RUSAA-28.
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
}
