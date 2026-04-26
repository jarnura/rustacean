use axum::{
    extract::FromRequestParts,
    http::{header, request::Parts, StatusCode},
};
use rb_auth::sha256_hex;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{error::AppError, state::AppState};

// ---------------------------------------------------------------------------
// Scope
// ---------------------------------------------------------------------------

/// Access scope for an API key.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum Scope {
    Read,
    Write,
    Admin,
}

impl Scope {
    pub(crate) fn from_str(s: &str) -> Option<Self> {
        match s {
            "read" => Some(Scope::Read),
            "write" => Some(Scope::Write),
            "admin" => Some(Scope::Admin),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// Identity types
// ---------------------------------------------------------------------------

// Fields used by session-gated handlers (login, switch-tenant, etc.)
#[allow(dead_code, clippy::struct_field_names)]
#[derive(Debug, Clone)]
pub struct SessionInfo {
    pub session_id: Uuid,
    pub user_id: Uuid,
    pub tenant_id: Uuid,
}

/// Identity extracted from a valid API key in the `Authorization: Bearer` header.
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct ApiKeyInfo {
    pub key_id: Uuid,
    pub tenant_id: Uuid,
    pub user_id: Uuid,
    pub scopes: Vec<Scope>,
}

// ---------------------------------------------------------------------------
// AuthContext
// ---------------------------------------------------------------------------

/// Identity attached to every inbound request.
///
/// - `Session` — resolved from `Cookie: rb_session=<token>`
/// - `ApiKey`  — resolved from `Authorization: Bearer rb_live_<hex>`
/// - `Anonymous` — no valid credential present
#[allow(dead_code)]
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum AuthContext {
    Session(SessionInfo),
    ApiKey(ApiKeyInfo),
    Anonymous,
}

impl FromRequestParts<AppState> for AuthContext {
    type Rejection = StatusCode;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        // API key via Bearer header takes precedence over session cookie.
        if let Some(token) = extract_bearer_token(parts) {
            if token.starts_with("rb_live_") {
                if let Some(info) = lookup_api_key(&state.pool, &token).await {
                    return Ok(AuthContext::ApiKey(info));
                }
                // Token looks like an API key but failed lookup → stay Anonymous
                // (don't fall through to cookie — the caller intended key auth).
                return Ok(AuthContext::Anonymous);
            }
        }
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

fn extract_bearer_token(parts: &Parts) -> Option<String> {
    let value = parts.headers.get(header::AUTHORIZATION)?.to_str().ok()?;
    value.strip_prefix("Bearer ").map(|t| t.trim().to_owned()).filter(|t| !t.is_empty())
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

// ---------------------------------------------------------------------------
// Database lookups
// ---------------------------------------------------------------------------

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

async fn lookup_api_key(pool: &PgPool, token: &str) -> Option<ApiKeyInfo> {
    let key_hash = sha256_hex(token);
    let row: Option<(Uuid, Uuid, Uuid, serde_json::Value)> = sqlx::query_as(
        "SELECT id, tenant_id, created_by_user_id, scopes \
         FROM control.api_keys \
         WHERE key_hash = $1 AND revoked_at IS NULL",
    )
    .bind(&key_hash)
    .fetch_optional(pool)
    .await
    .ok()?;

    let (key_id, tenant_id, user_id, scopes_json) = row?;
    let scopes = parse_scopes(&scopes_json);

    // Fire-and-forget: update last_used_at without blocking the hot path.
    let pool = pool.clone();
    tokio::spawn(async move {
        let _ = sqlx::query(
            "UPDATE control.api_keys SET last_used_at = now() WHERE id = $1",
        )
        .bind(key_id)
        .execute(&pool)
        .await;
    });

    Some(ApiKeyInfo { key_id, tenant_id, user_id, scopes })
}

fn parse_scopes(value: &serde_json::Value) -> Vec<Scope> {
    value
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().and_then(Scope::from_str))
                .collect()
        })
        .unwrap_or_default()
}

// ---------------------------------------------------------------------------
// Scope-check helper
// ---------------------------------------------------------------------------

/// Require that the caller authenticated via API key and holds the given scope.
///
/// Returns `Unauthorized` for non-API-key callers and `InsufficientScope`
/// when the key lacks the required scope.
#[allow(dead_code)]
pub fn require_scope<'a>(auth: &'a AuthContext, required: &Scope) -> Result<&'a ApiKeyInfo, AppError> {
    match auth {
        AuthContext::ApiKey(info) => {
            if info.scopes.contains(required) {
                Ok(info)
            } else {
                Err(AppError::InsufficientScope)
            }
        }
        _ => Err(AppError::Unauthorized),
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

    fn parts_with_bearer(token: &str) -> Parts {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {token}")).unwrap(),
        );
        let mut req = axum::http::Request::builder().body(()).unwrap();
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
    fn extract_bearer_token_parses_valid_header() {
        let parts = parts_with_bearer("rb_live_abc123def456");
        assert_eq!(extract_bearer_token(&parts).as_deref(), Some("rb_live_abc123def456"));
    }

    #[test]
    fn extract_bearer_token_returns_none_when_absent() {
        let parts = parts_with_cookie("rb_session=tok");
        assert!(extract_bearer_token(&parts).is_none());
    }

    #[test]
    fn extract_bearer_token_trims_whitespace() {
        let parts = parts_with_bearer("  rb_live_abc  ");
        assert_eq!(extract_bearer_token(&parts).as_deref(), Some("rb_live_abc"));
    }

    #[test]
    fn parse_scopes_extracts_known_values() {
        let json = serde_json::json!(["read", "write"]);
        let scopes = parse_scopes(&json);
        assert_eq!(scopes, vec![Scope::Read, Scope::Write]);
    }

    #[test]
    fn parse_scopes_ignores_unknown_values() {
        let json = serde_json::json!(["read", "superpower"]);
        let scopes = parse_scopes(&json);
        assert_eq!(scopes, vec![Scope::Read]);
    }

    #[test]
    fn parse_scopes_returns_empty_for_non_array() {
        let json = serde_json::json!("read");
        let scopes = parse_scopes(&json);
        assert!(scopes.is_empty());
    }

    #[test]
    fn scope_roundtrips_via_serde() {
        for scope in [Scope::Read, Scope::Write, Scope::Admin] {
            let s = serde_json::to_string(&scope).unwrap();
            let parsed: Scope = serde_json::from_str(&s).unwrap();
            assert_eq!(scope, parsed);
        }
    }

    #[test]
    fn scope_from_str_all_variants() {
        assert_eq!(Scope::from_str("read"), Some(Scope::Read));
        assert_eq!(Scope::from_str("write"), Some(Scope::Write));
        assert_eq!(Scope::from_str("admin"), Some(Scope::Admin));
        assert_eq!(Scope::from_str("unknown"), None);
    }

    #[test]
    fn require_scope_rejects_anonymous() {
        let auth = AuthContext::Anonymous;
        assert!(matches!(require_scope(&auth, &Scope::Read), Err(AppError::Unauthorized)));
    }

    #[test]
    fn require_scope_rejects_session_auth() {
        let auth = AuthContext::Session(SessionInfo {
            session_id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            tenant_id: Uuid::new_v4(),
        });
        assert!(matches!(require_scope(&auth, &Scope::Read), Err(AppError::Unauthorized)));
    }

    #[test]
    fn require_scope_accepts_matching_scope() {
        let info = ApiKeyInfo {
            key_id: Uuid::new_v4(),
            tenant_id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            scopes: vec![Scope::Read, Scope::Write],
        };
        let auth = AuthContext::ApiKey(info);
        assert!(require_scope(&auth, &Scope::Read).is_ok());
        assert!(require_scope(&auth, &Scope::Write).is_ok());
    }

    #[test]
    fn require_scope_rejects_missing_scope() {
        let info = ApiKeyInfo {
            key_id: Uuid::new_v4(),
            tenant_id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            scopes: vec![Scope::Read],
        };
        let auth = AuthContext::ApiKey(info);
        assert!(matches!(
            require_scope(&auth, &Scope::Write),
            Err(AppError::InsufficientScope)
        ));
    }
}
