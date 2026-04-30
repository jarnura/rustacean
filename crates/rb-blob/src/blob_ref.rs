use uuid::Uuid;

use crate::error::BlobError;

/// A handle identifying a single content-addressed blob.
///
/// URI form: `rb-blob://tenant_<tenant_id>/<sha256>`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlobRef {
    pub tenant_id: Uuid,
    /// Hex-encoded SHA-256 of the blob content.
    pub sha256: String,
    pub content_type: String,
    pub size: u64,
}

impl BlobRef {
    pub fn new(
        tenant_id: Uuid,
        sha256: impl Into<String>,
        content_type: impl Into<String>,
        size: u64,
    ) -> Self {
        Self {
            tenant_id,
            sha256: sha256.into(),
            content_type: content_type.into(),
            size,
        }
    }

    /// Canonical URI: `rb-blob://tenant_<id>/<sha256>`
    #[must_use]
    pub fn to_uri(&self) -> String {
        format!("rb-blob://tenant_{}/{}", self.tenant_id, self.sha256)
    }

    /// Parse a URI back to a partial `BlobRef` (`content_type` = "", size = 0).
    ///
    /// # Errors
    ///
    /// Returns [`BlobError::InvalidUri`] if the URI does not match the expected format.
    pub fn from_uri(uri: &str) -> Result<Self, BlobError> {
        let rest = uri
            .strip_prefix("rb-blob://tenant_")
            .ok_or_else(|| BlobError::InvalidUri(uri.to_string()))?;
        let (tenant_str, sha256) = rest
            .split_once('/')
            .ok_or_else(|| BlobError::InvalidUri(uri.to_string()))?;
        let tenant_id = Uuid::parse_str(tenant_str)
            .map_err(|_| BlobError::InvalidUri(uri.to_string()))?;
        Ok(Self {
            tenant_id,
            sha256: sha256.to_string(),
            content_type: String::new(),
            size: 0,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uri_roundtrip() {
        let tenant_id = Uuid::new_v4();
        let original = BlobRef::new(tenant_id, "deadbeef".repeat(8), "application/octet-stream", 42);
        let uri = original.to_uri();
        let parsed = BlobRef::from_uri(&uri).expect("valid uri");
        assert_eq!(parsed.tenant_id, original.tenant_id);
        assert_eq!(parsed.sha256, original.sha256);
    }

    #[test]
    fn invalid_uri_rejected() {
        assert!(BlobRef::from_uri("https://example.com/blob").is_err());
        assert!(BlobRef::from_uri("rb-blob://tenant_notauuid/sha").is_err());
    }
}
