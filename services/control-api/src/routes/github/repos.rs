//! `GET /v1/github/installations/{id}/available-repos` (REQ-GH-03).
//!
//! Lists repositories accessible to the given GitHub App installation using
//! the cached installation token — not the caller's OAuth identity.
//!
//! Archived repos are excluded by default; pass `?include_archived=true`
//! to include them. Note: `total_count` always reflects GitHub's raw count
//! (which includes archived repos), so visible items may be fewer than
//! `per_page` when archived filtering is active.

use axum::{
    extract::{Path, Query, State},
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    error::AppError,
    middleware::auth::{require_verified_session, AuthContext},
    state::AppState,
};

#[derive(Deserialize)]
pub struct QueryParams {
    #[serde(default = "default_page")]
    page: u32,
    #[serde(default = "default_per_page")]
    per_page: u32,
    #[serde(default)]
    include_archived: bool,
}

fn default_page() -> u32 {
    1
}
fn default_per_page() -> u32 {
    30
}

#[derive(Serialize)]
pub struct RepoItemResponse {
    pub id: i64,
    pub name: String,
    pub full_name: String,
    pub private: bool,
    pub archived: bool,
    pub default_branch: String,
    pub html_url: String,
}

#[derive(Serialize)]
pub struct ListReposResponse {
    pub total_count: u64,
    pub page: u32,
    pub per_page: u32,
    pub repositories: Vec<RepoItemResponse>,
}

/// `GET /v1/github/installations/{id}/available-repos`
///
/// `id` is the internal UUID from `github_installations`. The handler
/// verifies it belongs to the session's tenant before hitting GitHub.
pub async fn list_available_repos(
    State(state): State<AppState>,
    auth: AuthContext,
    Path(installation_id): Path<Uuid>,
    Query(params): Query<QueryParams>,
) -> Result<impl IntoResponse, AppError> {
    let session = require_verified_session(auth)?;

    let per_page = params.per_page.clamp(1, 100);
    let page = params.page.max(1);

    let Some(gh) = state.gh.clone() else {
        return Err(AppError::GitHubAppNotConfigured);
    };

    // Verify the installation belongs to the session's tenant and is active.
    let row: Option<(i64,)> = sqlx::query_as(
        "SELECT github_installation_id \
         FROM control.github_installations \
         WHERE id = $1 AND tenant_id = $2 \
           AND deleted_at IS NULL AND suspended_at IS NULL",
    )
    .bind(installation_id)
    .bind(session.tenant_id)
    .fetch_optional(&state.pool)
    .await?;

    let Some((github_installation_id,)) = row else {
        return Err(AppError::NotFound);
    };

    let page_data = gh
        .list_installation_repos(github_installation_id, page, per_page)
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("GitHub API error: {e}")))?;

    let repositories: Vec<RepoItemResponse> = page_data
        .repositories
        .into_iter()
        .filter(|r| params.include_archived || !r.archived)
        .map(|r| RepoItemResponse {
            id: r.id,
            name: r.name,
            full_name: r.full_name,
            private: r.private,
            archived: r.archived,
            default_branch: r.default_branch,
            html_url: r.html_url,
        })
        .collect();

    Ok(Json(ListReposResponse {
        total_count: page_data.total_count,
        page,
        per_page,
        repositories,
    }))
}
