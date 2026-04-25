// Auth extraction stub — full implementation lands in RUSAA-29 (rb-auth).
// This extractor always produces Anonymous so that health and OpenAPI
// endpoints work before auth crate exists.

use axum::{
    extract::FromRequestParts,
    http::{request::Parts, StatusCode},
};

/// Identity attached to every inbound request.
///
/// Populated by parsing `Cookie: rb_session=<token>` or
/// `Authorization: Bearer <key>`. Currently a stub that always returns
/// [`AuthContext::Anonymous`]; full extraction is wired in RUSAA-29.
#[derive(Debug, Clone)]
#[non_exhaustive]
#[allow(dead_code)]
pub enum AuthContext {
    Anonymous,
}

impl<S: Send + Sync> FromRequestParts<S> for AuthContext {
    type Rejection = StatusCode;

    async fn from_request_parts(
        _parts: &mut Parts,
        _state: &S,
    ) -> Result<Self, Self::Rejection> {
        Ok(AuthContext::Anonymous)
    }
}
