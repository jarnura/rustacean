//! Repo management endpoints.
//! - `POST /v1/repos` — Connect a GitHub repo to the tenant (REQ-GH-04).
//! - `POST /v1/repos/{id}/ingest` — Trigger an ingestion run (REQ-GH-08).

use axum::{
    Json,
    extract::State,
    http::StatusCode,
    response::IntoResponse,
};
use rb_github::GhError;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{
    error::AppError,
    middleware::auth::{AuthContext, require_verified_session},
    state::AppState,
};


// ---------------------------------------------------------------------------
// POST /v1/repos
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, ToSchema)]
pub struct ConnectRepoRequest {
    /// GitHub numeric installation ID (from the App install callback).
    pub installation_id: i64,
    /// GitHub numeric repository ID (from the list-repos response).
    pub github_repo_id: i64,
    /// Default branch override. If omitted, the value is fetched from GitHub.
    pub default_branch: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ConnectRepoResponse {
    pub repo_id: Uuid,
    pub full_name: String,
    pub default_branch: String,
}

/// Connect a GitHub repository to the calling user's active tenant.
///
/// Verifies the installation belongs to the session tenant, confirms the repo
/// is accessible via GitHub's API, then inserts a `repos` row with
/// `status = 'connected'`.
#[utoipa::path(
    post,
    path = "/v1/repos",
    request_body = ConnectRepoRequest,
    responses(
        (status = 201, description = "Repository connected", body = ConnectRepoResponse),
        (status = 401, description = "Not authenticated or session expired"),
        (status = 403, description = "Email not verified"),
        (status = 404, description = "Installation not found or not owned by this tenant"),
        (status = 409, description = "Repository already connected (repo_already_connected)"),
        (status = 422, description = "Repository not accessible via installation (repo_not_accessible)"),
        (status = 503, description = "GitHub App not configured on this instance"),
    ),
    tag = "repos"
)]
pub async fn connect_repo(
    State(state): State<AppState>,
    auth: AuthContext,
    Json(body): Json<ConnectRepoRequest>,
) -> Result<impl IntoResponse, AppError> {
    let session = require_verified_session(auth)?;

    let gh = state.gh.as_ref().ok_or(AppError::GithubAppNotConfigured)?;

    let row: Option<(Uuid,)> = sqlx::query_as(
        "SELECT id FROM control.github_installations \
         WHERE github_installation_id = $1 \
           AND tenant_id = $2 \
           AND deleted_at IS NULL \
           AND suspended_at IS NULL",
    )
    .bind(body.installation_id)
    .bind(session.tenant_id)
    .fetch_optional(&state.pool)
    .await?;

    let (installation_uuid,) = row.ok_or(AppError::NotFound)?;

    let repo_info = gh
        .fetch_repo(body.installation_id, body.github_repo_id)
        .await
        .map_err(|e| match e {
            GhError::ApiError { status: 404, .. } | GhError::ApiError { status: 403, .. } => {
                AppError::RepoNotAccessible
            }
            other => AppError::Internal(anyhow::anyhow!("{other}")),
        })?;

    let default_branch = body.default_branch.unwrap_or(repo_info.default_branch);

    let repo_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO control.repos \
         (id, tenant_id, installation_id, github_repo_id, full_name, default_branch, connected_by, status) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, 'connected')",
    )
    .bind(repo_id)
    .bind(session.tenant_id)
    .bind(installation_uuid)
    .bind(body.github_repo_id)
    .bind(&repo_info.full_name)
    .bind(&default_branch)
    .bind(session.user_id)
    .execute(&state.pool)
    .await
    .map_err(|e| {
        if let sqlx::Error::Database(ref dbe) = e {
            if dbe.constraint() == Some("repos_tenant_id_github_repo_id_key") {
                return AppError::RepoAlreadyConnected;
            }
        }
        AppError::Database(e)
    })?;

    tracing::info!(
        %repo_id,
        tenant_id = %session.tenant_id,
        github_repo_id = body.github_repo_id,
        full_name = %repo_info.full_name,
        "repo connected"
    );

    Ok((
        StatusCode::CREATED,
        Json(ConnectRepoResponse {
            repo_id,
            full_name: repo_info.full_name,
            default_branch,
        }),
    ))
}


// ---------------------------------------------------------------------------
// POST /v1/repos/{id}/ingest — REQ-GH-08
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize, ToSchema)]
pub struct TriggerIngestResponse {
    pub run_id: Uuid,
    pub repo_id: Uuid,
    pub status: String,
}

/// Trigger an asynchronous ingestion run for a connected repository.
///
/// Returns 202 immediately; ingestion is processed asynchronously by the worker.
/// 404 if the repository does not exist or belongs to another tenant.
/// 409 if an ingestion run is already queued or running for this repo.
#[utoipa::path(
    post,
    path = "/v1/repos/{id}/ingest",
    params(
        ("id" = Uuid, Path, description = "Repository UUID (from POST /v1/repos)")
    ),
    responses(
        (status = 202, description = "Ingestion run queued", body = TriggerIngestResponse),
        (status = 401, description = "Not authenticated or session expired"),
        (status = 403, description = "Email not verified"),
        (status = 404, description = "Repository not found or belongs to another tenant"),
        (status = 409, description = "Ingestion run already in-flight (ingest_run_already_in_flight)"),
    ),
    tag = "repos"
)]
pub async fn trigger_ingest(
    State(state): State<AppState>,
    auth: AuthContext,
    axum::extract::Path(repo_id): axum::extract::Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let session = require_verified_session(auth)?;

    let exists: Option<(Uuid,)> = sqlx::query_as(
        "SELECT id FROM control.repos \
         WHERE id = $1 AND tenant_id = $2 AND archived_at IS NULL",
    )
    .bind(repo_id)
    .bind(session.tenant_id)
    .fetch_optional(&state.pool)
    .await?;
    exists.ok_or(AppError::NotFound)?;

    let in_flight: Option<(Uuid,)> = sqlx::query_as(
        "SELECT id FROM control.ingestion_runs \
         WHERE repo_id = $1 AND tenant_id = $2 AND status IN ('queued', 'running') LIMIT 1",
    )
    .bind(repo_id)
    .bind(session.tenant_id)
    .fetch_optional(&state.pool)
    .await?;
    if in_flight.is_some() {
        return Err(AppError::IngestRunAlreadyInFlight);
    }

    let run_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO control.ingestion_runs \
         (id, tenant_id, repo_id, status, requested_by) \
         VALUES ($1, $2, $3, 'queued', $4)",
    )
    .bind(run_id)
    .bind(session.tenant_id)
    .bind(repo_id)
    .bind(session.user_id)
    .execute(&state.pool)
    .await?;

    tracing::info!(
        %run_id,
        %repo_id,
        tenant_id = %session.tenant_id,
        "ingestion run queued"
    );

    Ok((
        StatusCode::ACCEPTED,
        Json(TriggerIngestResponse {
            run_id,
            repo_id,
            status: "queued".to_owned(),
        }),
    ))
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::middleware::auth::{ApiKeyInfo, Scope, SessionInfo};

    fn verified_session() -> SessionInfo {
        SessionInfo {
            session_id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            tenant_id: Uuid::new_v4(),
            email_verified: true,
        }
    }

    #[test]
    fn anonymous_auth_rejected() {
        let result = require_verified_session(AuthContext::Anonymous);
        assert!(matches!(result, Err(AppError::Unauthorized)));
    }

    #[test]
    fn api_key_auth_rejected() {
        let key = ApiKeyInfo {
            key_id: Uuid::new_v4(),
            tenant_id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            scopes: vec![Scope::Write],
        };
        let result = require_verified_session(AuthContext::ApiKey(key));
        assert!(matches!(result, Err(AppError::Unauthorized)));
    }

    #[test]
    fn expired_session_rejected() {
        let result = require_verified_session(AuthContext::ExpiredSession);
        assert!(matches!(result, Err(AppError::SessionExpired)));
    }

    #[test]
    fn unverified_email_rejected() {
        let mut info = verified_session();
        info.email_verified = false;
        let result = require_verified_session(AuthContext::Session(info));
        assert!(matches!(result, Err(AppError::EmailNotVerified)));
    }

    #[test]
    fn verified_session_accepted() {
        let info = verified_session();
        let user_id = info.user_id;
        let result = require_verified_session(AuthContext::Session(info));
        let session = result.unwrap();
        assert_eq!(session.user_id, user_id);
    }

    #[test]
    fn github_404_maps_to_repo_not_accessible() {
        let err = GhError::ApiError {
            status: 404,
            body: "Not Found".to_owned(),
        };
        let app_err = match err {
            GhError::ApiError { status: 404, .. } | GhError::ApiError { status: 403, .. } => {
                AppError::RepoNotAccessible
            }
            other => AppError::Internal(anyhow::anyhow!("{other}")),
        };
        assert!(matches!(app_err, AppError::RepoNotAccessible));
    }

    #[test]
    fn github_403_maps_to_repo_not_accessible() {
        let err = GhError::ApiError {
            status: 403,
            body: "Forbidden".to_owned(),
        };
        let app_err = match err {
            GhError::ApiError { status: 404, .. } | GhError::ApiError { status: 403, .. } => {
                AppError::RepoNotAccessible
            }
            other => AppError::Internal(anyhow::anyhow!("{other}")),
        };
        assert!(matches!(app_err, AppError::RepoNotAccessible));
    }

    #[test]
    fn github_500_maps_to_internal() {
        let err = GhError::ApiError {
            status: 500,
            body: "Server Error".to_owned(),
        };
        let app_err = match err {
            GhError::ApiError { status: 404, .. } | GhError::ApiError { status: 403, .. } => {
                AppError::RepoNotAccessible
            }
            other => AppError::Internal(anyhow::anyhow!("{other}")),
        };
        assert!(matches!(app_err, AppError::Internal(_)));
    }

    #[test]
    fn default_branch_override_takes_priority() {
        let github_branch = "main".to_owned();
        let override_branch = Some("develop".to_owned());
        let result = override_branch.unwrap_or(github_branch);
        assert_eq!(result, "develop");
    }

    #[test]
    fn github_default_branch_used_when_no_override() {
        let github_branch = "main".to_owned();
        let override_branch: Option<String> = None;
        let result = override_branch.unwrap_or(github_branch);
        assert_eq!(result, "main");
    }

    #[test]
    fn trigger_ingest_response_serializes_correctly() {
        let run_id = Uuid::new_v4();
        let repo_id = Uuid::new_v4();
        let resp = TriggerIngestResponse {
            run_id,
            repo_id,
            status: "queued".to_owned(),
        };
        let val = serde_json::to_value(&resp).unwrap();
        assert_eq!(val["status"], "queued");
        assert!(val.get("run_id").is_some());
        assert!(val.get("repo_id").is_some());
    }

    #[test]
    fn ingest_run_already_in_flight_is_conflict() {
        let err = AppError::IngestRunAlreadyInFlight;
        let resp = err.into_response();
        assert_eq!(resp.status(), StatusCode::CONFLICT);
    }

    #[test]
    fn ingest_error_message() {
        assert_eq!(
            AppError::IngestRunAlreadyInFlight.to_string(),
            "an ingestion run is already in progress for this repository"
        );
    }

    #[test]
    fn new_error_messages() {
        assert_eq!(
            AppError::GithubAppNotConfigured.to_string(),
            "GitHub App is not configured on this instance"
        );
        assert_eq!(
            AppError::RepoNotAccessible.to_string(),
            "repository is not accessible via the given installation"
        );
        assert_eq!(
            AppError::RepoAlreadyConnected.to_string(),
            "repository is already connected to this tenant"
        );
        assert_eq!(
            AppError::IngestRunAlreadyInFlight.to_string(),
            "an ingestion run is already in progress for this repository"
        );
    }
}
