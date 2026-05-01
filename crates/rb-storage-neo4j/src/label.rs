use rb_schemas::TenantId;

/// Derives the Neo4j tenant label for `tenant_id`.
///
/// Format: `Tenant_<24 lowercase hex chars>` — identical derivation to `rb-tenant`'s schema
/// name but with a capital-T prefix (Neo4j label convention).
///
/// The 24 hex chars come from bytes 4..16 of the UUID (lower 96 bits), matching the
/// schema-name derivation in `rb-tenant::TenantCtx`.
#[must_use]
pub fn tenant_label(tenant_id: &TenantId) -> String {
    let uuid = tenant_id.as_uuid();
    let bytes = uuid.as_bytes();
    let mut hex = String::with_capacity(24);
    for b in &bytes[4..] {
        use std::fmt::Write as _;
        write!(hex, "{b:02x}").expect("infallible write to String");
    }
    format!("Tenant_{hex}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use rb_schemas::TenantId;

    #[test]
    fn label_starts_with_tenant_prefix() {
        let label = tenant_label(&TenantId::new());
        assert!(label.starts_with("Tenant_"));
    }

    #[test]
    fn label_has_correct_length() {
        let label = tenant_label(&TenantId::new());
        // "Tenant_" (7) + 24 hex = 31 chars
        assert_eq!(label.len(), 31);
    }

    #[test]
    fn label_is_valid_neo4j_identifier() {
        let label = tenant_label(&TenantId::new());
        assert!(label.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'));
    }

    #[test]
    fn label_is_deterministic() {
        let id = TenantId::new();
        assert_eq!(tenant_label(&id), tenant_label(&id));
    }

    #[test]
    fn different_tenants_have_different_labels() {
        let l1 = tenant_label(&TenantId::new());
        let l2 = tenant_label(&TenantId::new());
        assert_ne!(l1, l2);
    }
}
