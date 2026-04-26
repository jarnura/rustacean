use axum::{
    extract::FromRequestParts,
    http::{header, request::Parts, StatusCode},
};
use rb_auth::sha256_hex;
use sqlx::PgPool;
use uuid::Uuid;

use crate::{error::AppError, state::AppState};

// ---------------------------------------------------------------------------
// Identity types
// ---------------------------------------------------------------------------

#[allow(dead_code, clippy::struct_field_names)]
#[derive(Debug, Clone)]
pub struct SessionInfo {
    pub session_id: Uuid,
    pub user_id: Uuid,
    pub tenant_id: Uuid,
    /// `true` when `users.email_verified_at IS NOT NULL`.
    pub email_verified: bool,
}

// ---------------------------------------------------------------------------
// AuthContext
// ---------------------------------------------------------------------------

/// Identity attached to every inbound request.
///
/// - `Session` — resolved from `Cookie: rb_session=<token>`
/// - `Anonymous` — no valid credential present
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

// ---------------------------------------------------------------------------
// Helpers for request credential extraction
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Database lookups
// ---------------------------------------------------------------------------

async fn lookup_session(pool: &PgPool, token: &str) -> Option<SessionInfo> {
    let token_hash = sha256_hex(token);
    let row: Option<(Uuid, Uuid, Uuid, bool)> = sqlx::query_as(
        "SELECT s.id, s.user_id, s.tenant_id, \
                (u.email_verified_at IS NOT NULL) \
         FROM control.sessions s \
         JOIN control.users u ON u.id = s.user_id \
         WHERE s.token_hash = $1 AND s.revoked_at IS NULL AND s.expires_at > now()",
    )
    .bind(&token_hash)
    .fetch_optional(pool)
    .await
    .ok()?;
    row.map(|(session_id, user_id, tenant_id, email_verified)| SessionInfo {
        session_id,
        user_id,
        tenant_id,
        email_verified,
    })
}

// ---------------------------------------------------------------------------
// Auth helpers
// ---------------------------------------------------------------------------

/// Require a valid session whose owner has confirmed their email address.
///
/// Returns `Unauthorized` for non-session callers and `EmailNotVerified` for
/// sessions where `email_verified_at` is NULL.
pub fn require_verified_session(auth: AuthContext) -> Result<SessionInfo, AppError> {
    match auth {
        AuthContext::Session(info) if info.email_verified => Ok(info),
        AuthContext::Session(_) => Err(AppError::EmailNotVerified),
        AuthContext::Anonymous => Err(AppError::Unauthorized),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::{HeaderMap, HeaderValue};

    fn parts_with_cookie(cookie: &str) -> Parts {
        let mut headers = HeaderMap::new();
        headers.insert(header::COOKIE, HeaderValue::from_str(cookie).unwrap());
        let mut req = axum::http::Request::builder().body(()).unwrap();
        *req.headers_mut() = headers;
        req.into_parts().0
    }

    fn make_session(email_verified: bool) -> SessionInfo {
        SessionInfo {
            session_id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            tenant_id: Uuid::new_v4(),
            email_verified,
        }
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
        assert!(extract_session_cookie(&parts).is_some());
    }

    #[test]
    fn require_verified_session_accepts_verified() {
        let info = make_session(true);
        let auth = AuthContext::Session(info.clone());
        let result = require_verified_session(auth).unwrap();
        assert_eq!(result.user_id, info.user_id);
    }

    #[test]
    fn require_verified_session_rejects_unverified_session() {
        let auth = AuthContext::Session(make_session(false));
        assert!(matches!(
            require_verified_session(auth),
            Err(AppError::EmailNotVerified)
        ));
    }

    #[test]
    fn require_verified_session_rejects_anonymous() {
        assert!(matches!(
            require_verified_session(AuthContext::Anonymous),
            Err(AppError::Unauthorized)
        ));
    }
}
