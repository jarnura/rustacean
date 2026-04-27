//! GET /v1/github/install-url  — generate a single-use App install URL (REQ-GH-02)
//! GET /v1/github/callback     — validate state token and create installation row (REQ-GH-02)

use axum::{
    Json,
    extract::{Query, State},
    response::IntoResponse,
};
use rand::RngCore as _;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    error::AppError,
    middleware::auth::{AuthContext, require_verified_session},
    state::AppState,
};

// ---------------------------------------------------------------------------
// GET /v1/github/install-url
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
pub struct InstallUrlResponse {
    /// Full GitHub App install URL to open in the browser.
    pub url: String,
    /// The raw opaque state token embedded in the URL. Included for
    /// client-side logging/debug; clients do not need to store it — GitHub
    /// echoes it back in the callback.
    pub state_token: String,
}

/// Generate a single-use, 10-minute, tenant-bound GitHub App install URL.
///
/// The state token is stored as SHA-256(token) so the raw value never
/// appears in the database. Requires a verified session.
pub async fn github_install_url(
    State(state): State<AppState>,
    auth: AuthContext,
) -> Result<impl IntoResponse, AppError> {
    let session = require_verified_session(auth)?;
    let gh = state.gh.as_ref().ok_or(AppError::GitHubAppNotConfigured)?;

    // App slug comes from the 60-second identity cache — safe per request.
    let identity = gh.check_identity().await.map_err(|e| {
        tracing::error!(error = %e, "install-url: GitHub App identity unavailable");
        AppError::Internal(anyhow::anyhow!("GitHub App identity unavailable"))
    })?;

    // 256-bit cryptographically random state token.
    let mut raw = [0u8; 32];
    rand::rng().fill_bytes(&mut raw);
    let token_hex = hex::encode(raw);
    let token_hash = rb_github::hash_token(&raw);

    sqlx::query(
        "INSERT INTO control.github_install_states \
         (token_hash, tenant_id, user_id, expires_at, created_at) \
         VALUES ($1, $2, $3, now() + interval '10 minutes', now())",
    )
    .bind(&token_hash)
    .bind(session.tenant_id)
    .bind(session.user_id)
    .execute(&state.pool)
    .await?;

    let url = format!(
        "https://github.com/apps/{}/installations/new?state={}",
        identity.slug, token_hex
    );

    tracing::info!(
        tenant_id = %session.tenant_id,
        user_id = %session.user_id,
        "github install-url: state token issued"
    );

    Ok(Json(InstallUrlResponse { url, state_token: token_hex }))
}

// ---------------------------------------------------------------------------
// GET /v1/github/callback
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct CallbackParams {
    /// GitHub's numeric installation ID — present in every callback redirect.
    pub installation_id: i64,
    /// The opaque state token we generated in `install-url`.
    pub state: String,
    /// `install` or `update` — informational only, not acted on here.
    #[serde(default)]
    pub setup_action: Option<String>,
    // `code` is also sent by GitHub but we do not need OAuth exchange for
    // pure App installs; omitting it avoids an unused-field warning.
}

#[derive(Debug, Serialize)]
pub struct CallbackResponse {
    pub installation_id: i64,
    pub account_login: String,
    pub account_type: String,
}

/// Validate the state token and create (or reactivate) the `github_installations` row.
///
/// The state token lookup is atomic — a single `UPDATE ... RETURNING` that
/// validates expiry and single-use constraint together. The installation row
/// upsert handles the race with the `installation.created` webhook (whichever
/// arrives first, the result is correct).
pub async fn github_callback(
    State(state): State<AppState>,
    Query(params): Query<CallbackParams>,
) -> Result<impl IntoResponse, AppError> {
    let gh = state.gh.as_ref().ok_or(AppError::GitHubAppNotConfigured)?;

    // Atomically validate and consume the state token.
    // Decode hex first — install-url hashes raw bytes, so callback must too.
    let raw = hex::decode(&params.state).map_err(|_| AppError::InvalidToken)?;
    let token_hash = rb_github::hash_token(&raw);
    let row: Option<(Uuid, Uuid)> = sqlx::query_as(
        "UPDATE control.github_install_states \
         SET used_at = now() \
         WHERE token_hash = $1 \
           AND used_at IS NULL \
           AND expires_at > now() \
         RETURNING tenant_id, user_id",
    )
    .bind(&token_hash)
    .fetch_optional(&state.pool)
    .await?;

    let (tenant_id, user_id) = row.ok_or(AppError::InvalidToken)?;

    // Fetch account metadata (login, type, numeric id) using the App JWT.
    let info = gh.fetch_installation(params.installation_id).await.map_err(|e| {
        tracing::error!(
            installation_id = params.installation_id,
            error = %e,
            "callback: failed to fetch installation from GitHub"
        );
        AppError::Internal(anyhow::anyhow!("failed to fetch GitHub installation"))
    })?;

    // Upsert the installation row. The webhook.created path may have fired
    // before or after this callback — both paths are idempotent.
    sqlx::query(
        "INSERT INTO control.github_installations \
         (id, tenant_id, github_installation_id, account_login, account_type, account_id) \
         VALUES ($1, $2, $3, $4, $5, $6) \
         ON CONFLICT (github_installation_id) \
         DO UPDATE SET deleted_at = NULL, suspended_at = NULL",
    )
    .bind(Uuid::new_v4())
    .bind(tenant_id)
    .bind(params.installation_id)
    .bind(&info.account.login)
    .bind(&info.account.kind)
    .bind(info.account.id)
    .execute(&state.pool)
    .await?;

    tracing::info!(
        tenant_id = %tenant_id,
        user_id = %user_id,
        installation_id = params.installation_id,
        account = %info.account.login,
        setup_action = ?params.setup_action,
        "github callback: installation upserted"
    );

    Ok(Json(CallbackResponse {
        installation_id: params.installation_id,
        account_login: info.account.login,
        account_type: info.account.kind,
    }))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn install_url_response_serializes() {
        let resp = InstallUrlResponse {
            url: "https://github.com/apps/my-app/installations/new?state=abc".to_owned(),
            state_token: "abc".to_owned(),
        };
        let v = serde_json::to_value(&resp).unwrap();
        assert!(v["url"].as_str().unwrap().contains("state=abc"));
        assert_eq!(v["state_token"], "abc");
    }

    #[test]
    fn callback_params_setup_action_defaults_to_none() {
        let p = CallbackParams {
            installation_id: 1,
            state: "tok".to_owned(),
            setup_action: None,
        };
        assert!(p.setup_action.is_none());
    }

    #[test]
    fn callback_response_serializes_all_fields() {
        let resp = CallbackResponse {
            installation_id: 42,
            account_login: "octo-org".to_owned(),
            account_type: "Organization".to_owned(),
        };
        let v = serde_json::to_value(&resp).unwrap();
        assert_eq!(v["installation_id"], 42);
        assert_eq!(v["account_login"], "octo-org");
        assert_eq!(v["account_type"], "Organization");
    }

    /// Round-trip: install-url generates hex(raw), hashes raw bytes.
    /// Callback receives hex string, decodes to raw, hashes raw bytes.
    /// Both sides must produce the same digest.
    #[test]
    fn state_token_round_trip() {
        let raw = [0xdeu8; 32];
        let token_hex = hex::encode(raw);

        // install-url path
        let hash_at_generation = rb_github::hash_token(&raw);

        // callback path: decode hex then hash
        let decoded = hex::decode(&token_hex).unwrap();
        let hash_at_callback = rb_github::hash_token(&decoded);

        assert_eq!(hash_at_generation, hash_at_callback);
    }

    #[test]
    fn state_token_invalid_hex_is_rejected() {
        assert!(hex::decode("not-valid-hex!").is_err());
    }
}
