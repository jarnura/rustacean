// Typed webhook event enums — fully implemented in RUSAA-50 (REQ-GH-06).
//
// Stubs only here so the module tree compiles. The route handler and
// dispatcher are in RUSAA-50.

use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum InstallationEvent {
    Created(InstallationPayload),
    Deleted(InstallationPayload),
    Suspend(InstallationPayload),
    Unsuspend(InstallationPayload),
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Deserialize)]
pub struct InstallationPayload {
    pub installation: Installation,
}

#[derive(Debug, Deserialize)]
pub struct Installation {
    pub id: i64,
    pub account: Account,
}

#[derive(Debug, Deserialize)]
pub struct Account {
    pub login: String,
    #[serde(rename = "type")]
    pub account_type: String,
    pub id: i64,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum InstallationRepositoriesEvent {
    Added(InstallationReposPayload),
    Removed(InstallationReposPayload),
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Deserialize)]
pub struct InstallationReposPayload {
    pub installation: Installation,
    pub repositories_added: Vec<RepoRef>,
    pub repositories_removed: Vec<RepoRef>,
}

#[derive(Debug, Deserialize)]
pub struct RepoRef {
    pub id: i64,
    pub full_name: String,
}
