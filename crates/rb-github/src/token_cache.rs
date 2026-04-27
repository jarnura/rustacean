// Installation token cache — implemented in RUSAA-49 (REQ-GH-05).
//
// This stub defines the public types so rb-github compiles and other
// modules can reference them before RUSAA-49 lands.

use crate::secret::Secret;

/// A GitHub installation access token with its expiry timestamp.
#[derive(Debug)]
pub struct CachedToken {
    pub token: Secret<String>,
    pub expires_at: chrono::DateTime<chrono::Utc>,
}
