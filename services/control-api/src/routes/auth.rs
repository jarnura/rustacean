use axum::{Json, extract::State, http::StatusCode, response::IntoResponse};
use chrono::{DateTime, Duration, Utc};
use rb_auth::{EmailToken, SessionToken, sha256_hex};
use rb_email::EmailTemplate;
use rb_schemas::TenantId;
use rb_tenant::TenantCtx;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{error::AppError, state::AppState};

#[derive(Debug, Deserialize, ToSchema)]
pub struct SignupRequest {
    /// RFC 5322 email address.
    pub email: String,
    /// Plaintext password, minimum 12 characters.
    pub password: String,
    /// Display name for the new tenant workspace.
    pub tenant_name: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct SignupResponse {
    /// Always `true` on signup — email verification required before tenant access.
    pub email_verification_required: bool,
    pub user_id: Uuid,
}

struct SignupTransactionResult {
    user_id: Uuid,
    session_token: SessionToken,
    email_token: EmailToken,
}

/// Register a new user and create their first tenant workspace.
#[utoipa::path(
    post,
    path = "/v1/auth/signup",
    request_body = SignupRequest,
    responses(
        (status = 201, description = "User and tenant created", body = SignupResponse),
        (status = 400, description = "Weak password (weak_password) or invalid email (invalid_email)"),
        (status = 409, description = "Email already registered (email_taken)"),
    ),
    tag = "auth"
)]
pub async fn signup(
    State(state): State<AppState>,
    Json(body): Json<SignupRequest>,
) -> Result<impl IntoResponse, AppError> {
    validate_email(&body.email)?;
    if body.password.len() < 12 {
        return Err(AppError::WeakPassword);
    }
    let password_hash = state.hasher.hash(&body.password)?;

    let mut tx = state.pool.begin().await?;
    let result =
        execute_signup_transaction(&mut tx, &body, &password_hash, state.config.session_ttl_days)
            .await?;
    tx.commit().await?;

    let verify_link = format!(
        "{}/auth/verify-email?token={}",
        state.config.base_url,
        result.email_token.as_str()
    );
    let email = EmailTemplate::VerifyEmail { link: verify_link }.to_email(&body.email)?;
    if let Err(e) = state.email_sender.send(email).await {
        tracing::warn!(user_id = %result.user_id, error = %e, "verification email delivery failed");
    }

    let cookie = format!(
        "rb_session={}; HttpOnly; SameSite=Lax; Path=/",
        result.session_token.as_str()
    );
    Ok((
        StatusCode::CREATED,
        [("Set-Cookie", cookie)],
        Json(SignupResponse { email_verification_required: true, user_id: result.user_id }),
    ))
}

async fn execute_signup_transaction(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    body: &SignupRequest,
    password_hash: &str,
    session_ttl_days: i64,
) -> Result<SignupTransactionResult, AppError> {
    let email_exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM control.users WHERE email = $1)",
    )
    .bind(&body.email)
    .fetch_one(&mut **tx)
    .await?;
    if email_exists {
        return Err(AppError::EmailTaken);
    }

    let tenant_id_typed = TenantId::new();
    let tenant_uuid = tenant_id_typed.as_uuid();
    let tenant_ctx = TenantCtx::new(tenant_id_typed);
    let slug = derive_slug(&body.tenant_name, tenant_uuid);

    sqlx::query(
        "INSERT INTO control.tenants (id, slug, name, schema_name) VALUES ($1, $2, $3, $4)",
    )
    .bind(tenant_uuid)
    .bind(&slug)
    .bind(&body.tenant_name)
    .bind(tenant_ctx.schema_name())
    .execute(&mut **tx)
    .await?;

    let schema = tenant_ctx.schema_name();
    sqlx::query(&format!(r#"CREATE SCHEMA IF NOT EXISTS "{schema}""#))
        .execute(&mut **tx)
        .await?;

    let user_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO control.users (id, email, password_hash) VALUES ($1, $2, $3)",
    )
    .bind(user_id)
    .bind(&body.email)
    .bind(password_hash)
    .execute(&mut **tx)
    .await?;

    sqlx::query(
        "INSERT INTO control.tenant_members (tenant_id, user_id, role) VALUES ($1, $2, 'owner')",
    )
    .bind(tenant_uuid)
    .bind(user_id)
    .execute(&mut **tx)
    .await?;

    let email_token = EmailToken::generate();
    let expires_at = Utc::now() + Duration::hours(1);
    sqlx::query(
        "INSERT INTO control.email_tokens (token_hash, user_id, kind, expires_at) \
         VALUES ($1, $2, 'verify', $3)",
    )
    .bind(email_token.hash())
    .bind(user_id)
    .bind(expires_at)
    .execute(&mut **tx)
    .await?;

    sqlx::query(
        "INSERT INTO control.auth_events (user_id, tenant_id, event) VALUES ($1, $2, 'signup')",
    )
    .bind(user_id)
    .bind(tenant_uuid)
    .execute(&mut **tx)
    .await?;

    let session_id = Uuid::new_v4();
    let session_token = SessionToken::generate();
    let session_expires_at = Utc::now() + Duration::days(session_ttl_days);
    sqlx::query(
        "INSERT INTO control.sessions (id, user_id, tenant_id, token_hash, expires_at) \
         VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(session_id)
    .bind(user_id)
    .bind(tenant_uuid)
    .bind(session_token.hash())
    .bind(session_expires_at)
    .execute(&mut **tx)
    .await?;

    Ok(SignupTransactionResult { user_id, session_token, email_token })
}

fn validate_email(email: &str) -> Result<(), AppError> {
    let Some((local, domain)) = email.split_once('@') else {
        return Err(AppError::InvalidEmail);
    };
    if local.is_empty() || domain.is_empty() || !domain.contains('.') {
        return Err(AppError::InvalidEmail);
    }
    Ok(())
}

fn derive_slug(name: &str, id: Uuid) -> String {
    let base: String = name
        .to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    let base = if base.is_empty() { "workspace".to_owned() } else { base };
    let suffix = &id.simple().to_string()[..6];
    format!("{base}-{suffix}")
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ForgotPasswordRequest {
    /// Email address for the account to recover.
    pub email: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ResetPasswordRequest {
    /// Plaintext reset token from the emailed link.
    pub token: String,
    /// New password; minimum 12 characters.
    pub new_password: String,
}

/// Request a password-reset email.
///
/// Always returns 200 OK regardless of whether the address is registered,
/// preventing email enumeration. When found, a reset link with a 15-minute
/// expiry is emailed. When not found, a dummy argon2id hash is performed to
/// keep the response time within ±50ms of a real lookup.
#[utoipa::path(
    post,
    path = "/v1/auth/forgot-password",
    request_body = ForgotPasswordRequest,
    responses(
        (status = 200, description = "Reset email sent or silently skipped"),
    ),
    tag = "auth"
)]
pub async fn forgot_password(
    State(state): State<AppState>,
    Json(body): Json<ForgotPasswordRequest>,
) -> Result<impl IntoResponse, AppError> {
    let row: Option<(Uuid,)> =
        sqlx::query_as("SELECT id FROM control.users WHERE email = $1")
            .bind(&body.email)
            .fetch_optional(&state.pool)
            .await?;

    match row {
        Some((user_id,)) => {
            let reset_token = EmailToken::generate();
            let expires_at = Utc::now() + Duration::minutes(15);

            let mut tx = state.pool.begin().await?;

            sqlx::query(
                "INSERT INTO control.email_tokens (token_hash, user_id, kind, expires_at) \
                 VALUES ($1, $2, 'reset', $3)",
            )
            .bind(reset_token.hash())
            .bind(user_id)
            .bind(expires_at)
            .execute(&mut *tx)
            .await?;

            sqlx::query(
                "INSERT INTO control.auth_events (user_id, event) \
                 VALUES ($1, 'password_reset_requested')",
            )
            .bind(user_id)
            .execute(&mut *tx)
            .await?;

            tx.commit().await?;

            let reset_link = format!(
                "{}/auth/reset-password?token={}",
                state.config.base_url,
                reset_token.as_str()
            );
            let email =
                EmailTemplate::ResetPassword { link: reset_link }.to_email(&body.email)?;
            if let Err(e) = state.email_sender.send(email).await {
                tracing::warn!(
                    user_id = %user_id,
                    error = %e,
                    "reset email delivery failed"
                );
            }
        }
        None => {
            // Dummy hash keeps response time indistinguishable from the found path.
            let _ = state.hasher.hash("dummy-timing-equalizer-password-xx");
        }
    }

    Ok(StatusCode::OK)
}

/// Consume a reset token and set a new password.
///
/// Marks the token used, updates the password hash, and revokes **all** active
/// sessions for the user. The caller must re-authenticate after resetting.
#[utoipa::path(
    post,
    path = "/v1/auth/reset-password",
    request_body = ResetPasswordRequest,
    responses(
        (status = 204, description = "Password updated and all sessions revoked"),
        (status = 400, description = "Expired/used token (invalid_token) or short password (weak_password)"),
    ),
    tag = "auth"
)]
pub async fn reset_password(
    State(state): State<AppState>,
    Json(body): Json<ResetPasswordRequest>,
) -> Result<impl IntoResponse, AppError> {
    if body.new_password.len() < 12 {
        return Err(AppError::WeakPassword);
    }

    let token_hash = sha256_hex(&body.token);
    // Hash the new password before acquiring the DB transaction so the
    // CPU-bound work doesn't hold a transaction slot open.
    let new_password_hash = state.hasher.hash(&body.new_password)?;

    let mut tx = state.pool.begin().await?;

    // SELECT FOR UPDATE serialises concurrent reset attempts for the same token.
    let row: Option<(Uuid, Option<DateTime<Utc>>, DateTime<Utc>)> = sqlx::query_as(
        "SELECT user_id, used_at, expires_at \
         FROM control.email_tokens \
         WHERE token_hash = $1 AND kind = 'reset' \
         FOR UPDATE",
    )
    .bind(&token_hash)
    .fetch_optional(&mut *tx)
    .await?;

    let Some((user_id, used_at, expires_at)) = row else {
        return Err(AppError::InvalidToken);
    };

    if used_at.is_some() || expires_at < Utc::now() {
        return Err(AppError::InvalidToken);
    }

    sqlx::query("UPDATE control.users SET password_hash = $1 WHERE id = $2")
        .bind(&new_password_hash)
        .bind(user_id)
        .execute(&mut *tx)
        .await?;

    sqlx::query("UPDATE control.email_tokens SET used_at = now() WHERE token_hash = $1")
        .bind(&token_hash)
        .execute(&mut *tx)
        .await?;

    sqlx::query(
        "UPDATE control.sessions SET revoked_at = now() \
         WHERE user_id = $1 AND revoked_at IS NULL",
    )
    .bind(user_id)
    .execute(&mut *tx)
    .await?;

    sqlx::query(
        "INSERT INTO control.auth_events (user_id, event) VALUES ($1, 'password_reset')",
    )
    .bind(user_id)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_email_accepts_valid() {
        assert!(validate_email("user@example.com").is_ok());
        assert!(validate_email("a+b@sub.domain.io").is_ok());
    }

    #[test]
    fn validate_email_rejects_no_at() {
        assert!(matches!(validate_email("nodomain"), Err(AppError::InvalidEmail)));
    }

    #[test]
    fn validate_email_rejects_empty_local() {
        assert!(matches!(validate_email("@example.com"), Err(AppError::InvalidEmail)));
    }

    #[test]
    fn validate_email_rejects_no_dot_in_domain() {
        assert!(matches!(validate_email("user@localhost"), Err(AppError::InvalidEmail)));
    }

    #[test]
    fn derive_slug_lowercases_and_hyphenates() {
        let id = Uuid::new_v4();
        let slug = derive_slug("Acme Corp", id);
        assert!(slug.starts_with("acme-corp-"));
        assert!(slug.chars().all(|c| c.is_ascii_alphanumeric() || c == '-'));
    }

    #[test]
    fn derive_slug_collapses_multiple_separators() {
        let id = Uuid::new_v4();
        let slug = derive_slug("Hello   World!!!", id);
        assert!(slug.starts_with("hello-world-"));
    }

    #[test]
    fn derive_slug_empty_name_uses_fallback() {
        let id = Uuid::new_v4();
        let slug = derive_slug("---", id);
        assert!(slug.starts_with("workspace-"));
    }

    #[test]
    fn derive_slug_includes_uuid_suffix() {
        let id = Uuid::new_v4();
        let slug = derive_slug("MyTenant", id);
        let suffix = &id.simple().to_string()[..6];
        assert!(slug.ends_with(suffix));
    }
}
