use reqwest::Client;
use serde::Deserialize;

use crate::{app_jwt::mint_app_jwt, error::GhError, GhApp};

/// Response from `GET https://api.github.com/app`.
#[derive(Debug, Deserialize)]
pub struct AppIdentity {
    pub id: i64,
    pub slug: String,
    pub owner: AppOwner,
}

#[derive(Debug, Deserialize)]
pub struct AppOwner {
    pub login: String,
}

/// Response from `GET https://api.github.com/app/installations/{id}`.
#[derive(Debug, Deserialize)]
pub struct InstallationInfo {
    pub id: i64,
    pub account: InstallationAccount,
}

#[derive(Debug, Deserialize)]
pub struct InstallationAccount {
    pub login: String,
    /// `User` or `Organization` — matches the DB constraint in github_installations.
    #[serde(rename = "type")]
    pub kind: String,
    pub id: i64,
}

impl Clone for InstallationInfo {
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            account: InstallationAccount {
                login: self.account.login.clone(),
                kind: self.account.kind.clone(),
                id: self.account.id,
            },
        }
    }
}

/// Fetches the GitHub App's own identity by calling `GET /app` with an App JWT.
///
/// Used by `GET /v1/health/github-app` to confirm the private key is valid
/// and the App ID matches what GitHub knows.
pub async fn fetch_app_identity(
    app: &GhApp,
    http: &Client,
) -> Result<AppIdentity, GhError> {
    let jwt = mint_app_jwt(app.app_id, &app.encoding_key)?;
    let resp = http
        .get("https://api.github.com/app")
        .header("Authorization", format!("Bearer {jwt}"))
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .header("User-Agent", "rust-brain/1.0")
        .send()
        .await?;

    let status = resp.status().as_u16();
    if !resp.status().is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(GhError::ApiError { status, body });
    }

    Ok(resp.json::<AppIdentity>().await?)
}

/// Fetches metadata for a specific GitHub App installation using the App JWT.
///
/// Called by `GET /v1/github/callback` (REQ-GH-02) to retrieve `account_login`,
/// `account_type`, and `account_id` before writing the `github_installations` row.
pub async fn fetch_installation(
    app: &GhApp,
    http: &Client,
    installation_id: i64,
) -> Result<InstallationInfo, GhError> {
    let jwt = mint_app_jwt(app.app_id, &app.encoding_key)?;
    let resp = http
        .get(format!("https://api.github.com/app/installations/{installation_id}"))
        .header("Authorization", format!("Bearer {jwt}"))
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .header("User-Agent", "rust-brain/1.0")
        .send()
        .await?;

    let status = resp.status().as_u16();
    if !resp.status().is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(GhError::ApiError { status, body });
    }
    Ok(resp.json::<InstallationInfo>().await?)
}
