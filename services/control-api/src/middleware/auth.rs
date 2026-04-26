use axum::{
    extract::FromRequestParts,
    http::{header, request::Parts, StatusCode},
};
use rb_auth::sha256_hex;
use sqlx::PgPool;
use uuid::Uuid;

use crate::state::AppState;

// Fields are part of the public API used by upcoming session endpoints
// (RUSAA-31 login, RUSAA-34 /me, RUSAA-35 switch-tenant).
#[allow(dead_code, clippy::struct_field_names)]
#[derive(Debug, Clone)]
pub struct SessionInfo {
    pub session_id: Uuid,
    pub user_id: Uuid,
    pub tenant_id: Uuid,
}

/// Identity attached to every inbound request.
///
/// Populated by parsing `Cookie: rb_session=<token>`. The `Session` variant
/// is produced when the token resolves to a valid, non-revoked, non-expired
/// session row. All other cases produce `Anonymous`.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum AuthContext {
    Session(SessionInfo),
    Anonymous,
}

impl FromRequestParts<AppState> for AuthContext {
    type Rejection = StatusCode;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        if let Some(token) = extract_session_cookie(parts) {
            if let Some(info) = lookup_session(&state.pool, &token).await {
                return Ok(AuthContext::Session(info));
            }
        }
        Ok(AuthContext::Anonymous)
    }
}

fn extract_session_cookie(parts: &Parts) -> Option<String> {
    let cookie_header = parts.headers.get(header::COOKIE)?.to_str().ok()?;
    for part in cookie_header.split(';') {
        let part = part.trim();
        if let Some(val) = part.strip_prefix("rb_session=") {
            if !val.is_empty() {
                return Some(val.to_owned());
            }
        }
    }
    None
}

async fn lookup_session(pool: &PgPool, token: &str) -> Option<SessionInfo> {
    let token_hash = sha256_hex(token);
    let row: Option<(Uuid, Uuid, Uuid)> = sqlx::query_as(
        "SELECT id, user_id, tenant_id \
         FROM control.sessions \
         WHERE token_hash = $1 AND revoked_at IS NULL AND expires_at > now()",
    )
    .bind(&token_hash)
    .fetch_optional(pool)
    .await
    .ok()?;
    row.map(|(session_id, user_id, tenant_id)| SessionInfo { session_id, user_id, tenant_id })
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::{HeaderMap, HeaderValue};

    fn parts_with_cookie(cookie: &str) -> Parts {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::COOKIE,
            HeaderValue::from_str(cookie).unwrap(),
        );
        let mut req = axum::http::Request::builder()
            .body(())
            .unwrap();
        *req.headers_mut() = headers;
        req.into_parts().0
    }

    #[test]
    fn extract_session_cookie_finds_rb_session() {
        let parts = parts_with_cookie("rb_session=abc123; other=val");
        assert_eq!(extract_session_cookie(&parts).as_deref(), Some("abc123"));
    }

    #[test]
    fn extract_session_cookie_works_when_first() {
        let parts = parts_with_cookie("rb_session=tok42");
        assert_eq!(extract_session_cookie(&parts).as_deref(), Some("tok42"));
    }

    #[test]
    fn extract_session_cookie_returns_none_when_absent() {
        let parts = parts_with_cookie("other=value");
        assert!(extract_session_cookie(&parts).is_none());
    }

    #[test]
    fn extract_session_cookie_ignores_empty_value() {
        let parts = parts_with_cookie("rb_session=");
        assert!(extract_session_cookie(&parts).is_none());
    }

    #[test]
    fn extract_session_cookie_handles_whitespace_around_parts() {
        let parts = parts_with_cookie("first=a;  rb_session=tok99  ;last=b");
        // Trim of each part means "rb_session=tok99  " → value = "tok99  "
        // That's still non-empty so we get it back (trailing space is part of value).
        assert!(extract_session_cookie(&parts).is_some());
    }
}
