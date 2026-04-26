use axum::{Json, extract::State, response::IntoResponse};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{
    error::AppError,
    middleware::auth::{AuthContext, require_verified_session},
    state::AppState,
};

// ---------------------------------------------------------------------------
// POST /v1/me/switch-tenant
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, ToSchema)]
pub struct SwitchTenantRequest {
    /// UUID of the tenant to switch to.
    pub tenant_id: Uuid,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct CurrentTenant {
    pub id: Uuid,
    pub name: String,
    pub slug: String,
    pub role: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct SwitchTenantResponse {
    pub current_tenant: CurrentTenant,
}

/// Switch the active tenant for the current session.
///
/// The caller must already be a member of the target tenant. The session's
/// `tenant_id` is updated in place and a `tenant_switched` auth event is
/// written. Returns the new active tenant with the caller's role.
/// Requires: verified session.
#[utoipa::path(
    post,
    path = "/v1/me/switch-tenant",
    request_body = SwitchTenantRequest,
    responses(
        (status = 200, description = "Tenant switched", body = SwitchTenantResponse),
        (status = 401, description = "Not authenticated"),
        (status = 403, description = "Email not verified (email_not_verified) or not a member (not_a_member)"),
        (status = 404, description = "Target tenant not found or inactive"),
    ),
    tag = "me"
)]
pub async fn switch_tenant(
    State(state): State<AppState>,
    auth: AuthContext,
    Json(body): Json<SwitchTenantRequest>,
) -> Result<impl IntoResponse, AppError> {
    let session = require_verified_session(auth)?;

    let mut tx = state.pool.begin().await?;

    // FOR SHARE prevents a concurrent revocation from deleting/updating this row
    // between our membership check and the session UPDATE that follows.
    let member: Option<(String,)> = sqlx::query_as(
        "SELECT role FROM control.tenant_members \
         WHERE tenant_id = $1 AND user_id = $2 \
         FOR SHARE",
    )
    .bind(body.tenant_id)
    .bind(session.user_id)
    .fetch_optional(&mut *tx)
    .await?;

    let (role,) = member.ok_or(AppError::NotAMember)?;

    // FOR SHARE prevents the tenant from being deactivated between check and commit.
    let tenant: Option<(String, String)> = sqlx::query_as(
        "SELECT name, slug FROM control.tenants \
         WHERE id = $1 AND status = 'active' \
         FOR SHARE",
    )
    .bind(body.tenant_id)
    .fetch_optional(&mut *tx)
    .await?;

    let (name, slug) = tenant.ok_or(AppError::NotFound)?;

    sqlx::query("UPDATE control.sessions SET tenant_id = $1 WHERE id = $2")
        .bind(body.tenant_id)
        .bind(session.session_id)
        .execute(&mut *tx)
        .await?;

    sqlx::query(
        "INSERT INTO control.auth_events (user_id, tenant_id, event, metadata) \
         VALUES ($1, $2, 'tenant_switched', $3)",
    )
    .bind(session.user_id)
    .bind(body.tenant_id)
    .bind(serde_json::json!({
        "from_tenant": session.tenant_id,
        "to_tenant": body.tenant_id,
        "session_id": session.session_id,
    }))
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    Ok(Json(SwitchTenantResponse {
        current_tenant: CurrentTenant { id: body.tenant_id, name, slug, role },
    }))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::middleware::auth::{AuthContext, SessionInfo};

    fn make_session(email_verified: bool) -> SessionInfo {
        SessionInfo {
            session_id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            tenant_id: Uuid::new_v4(),
            email_verified,
        }
    }

    #[test]
    fn switch_tenant_request_deserializes() {
        let json = r#"{"tenant_id":"550e8400-e29b-41d4-a716-446655440000"}"#;
        let req: SwitchTenantRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.tenant_id.to_string(), "550e8400-e29b-41d4-a716-446655440000");
    }

    #[test]
    fn switch_tenant_response_serializes_all_fields() {
        let resp = SwitchTenantResponse {
            current_tenant: CurrentTenant {
                id: Uuid::new_v4(),
                name: "Test Corp".to_owned(),
                slug: "test-corp-abc123".to_owned(),
                role: "admin".to_owned(),
            },
        };
        let json = serde_json::to_value(&resp).unwrap();
        let ct = &json["current_tenant"];
        assert!(ct.get("id").is_some());
        assert_eq!(ct["name"], "Test Corp");
        assert_eq!(ct["slug"], "test-corp-abc123");
        assert_eq!(ct["role"], "admin");
    }

    #[test]
    fn anonymous_auth_returns_unauthorized() {
        assert!(matches!(
            require_verified_session(AuthContext::Anonymous),
            Err(AppError::Unauthorized)
        ));
    }

    #[test]
    fn unverified_session_returns_email_not_verified() {
        let auth = AuthContext::Session(make_session(false));
        assert!(matches!(
            require_verified_session(auth),
            Err(AppError::EmailNotVerified)
        ));
    }

    #[test]
    fn verified_session_returns_info() {
        let info = make_session(true);
        let auth = AuthContext::Session(info.clone());
        let result = require_verified_session(auth).unwrap();
        assert_eq!(result.user_id, info.user_id);
        assert_eq!(result.session_id, info.session_id);
    }

    #[test]
    fn not_a_member_error_message() {
        assert_eq!(AppError::NotAMember.to_string(), "user is not a member of this tenant");
    }

    #[test]
    fn email_not_verified_error_message() {
        assert_eq!(AppError::EmailNotVerified.to_string(), "email address not yet verified");
    }
}
