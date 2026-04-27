//! GitHub installation repository listing (REQ-GH-03).
//!
//! Calls `GET /installation/repositories` authenticated with an installation
//! access token. Callers obtain the token via [`crate::GhApp::list_installation_repos`],
//! which handles caching internally.

use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::error::GhError;

/// A single repository entry returned by GitHub's accessible-repos API.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct RepoItem {
    pub id: i64,
    pub name: String,
    pub full_name: String,
    pub private: bool,
    pub archived: bool,
    pub default_branch: String,
    pub html_url: String,
}

/// One page of results from `GET /installation/repositories`.
pub struct RepoPage {
    /// GitHub's total count across all pages (includes archived repos).
    pub total_count: u64,
    pub repositories: Vec<RepoItem>,
}

#[derive(Deserialize)]
struct GitHubRepoPage {
    total_count: u64,
    repositories: Vec<RepoItem>,
}

/// Fetches one page of repositories accessible to the installation.
///
/// `token` is the raw installation access token string.
/// `page` ≥ 1 and `per_page` 1–100 are forwarded directly to GitHub.
pub(crate) async fn list_installation_repos(
    token: &str,
    http: &Client,
    page: u32,
    per_page: u32,
) -> Result<RepoPage, GhError> {
    let resp = http
        .get("https://api.github.com/installation/repositories")
        .header("Authorization", format!("token {token}"))
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .header("User-Agent", "rust-brain/1.0")
        .query(&[("page", page), ("per_page", per_page)])
        .send()
        .await?;

    let status = resp.status().as_u16();
    if !resp.status().is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(GhError::ApiError { status, body });
    }

    let raw: GitHubRepoPage = resp.json().await?;
    Ok(RepoPage {
        total_count: raw.total_count,
        repositories: raw.repositories,
    })
}
