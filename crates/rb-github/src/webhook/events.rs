//! Typed payload structs for the GitHub webhook events the receiver dispatches
//! on. The receiver chooses which struct to deserialize into based on the
//! `X-GitHub-Event` header — there is intentionally no "any event" enum.
//!
//! Only the fields the receiver acts on are typed; unknown fields are
//! ignored (`serde` defaults to dropping them).

use serde::Deserialize;

// ---------------------------------------------------------------------------
// `installation` event
// ---------------------------------------------------------------------------

/// `X-GitHub-Event: installation`
///
/// We dispatch on the `action` discriminant; payloads are otherwise identical
/// across the four actions we care about.
#[derive(Debug, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum InstallationEvent {
    Created(InstallationPayload),
    Deleted(InstallationPayload),
    Suspend(InstallationPayload),
    Unsuspend(InstallationPayload),
    /// Any other action (`new_permissions_accepted`, etc.) — receiver
    /// acknowledges with 202 and does nothing.
    #[serde(other)]
    Other,
}

#[derive(Debug, Deserialize)]
pub struct InstallationPayload {
    pub installation: Installation,
}

#[derive(Debug, Deserialize)]
pub struct Installation {
    /// The numeric installation id GitHub assigns when an org/user installs
    /// the App. Stable across the install's lifetime.
    pub id: i64,
    pub account: Account,
}

#[derive(Debug, Deserialize)]
pub struct Account {
    pub login: String,
    /// `User` or `Organization`. The DB constraint enforces this; values
    /// outside that set will fail the SQL on insert paths.
    #[serde(rename = "type")]
    pub kind: String,
    pub id: i64,
}

// ---------------------------------------------------------------------------
// `installation_repositories` event
// ---------------------------------------------------------------------------

/// `X-GitHub-Event: installation_repositories`
#[derive(Debug, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum InstallationRepositoriesEvent {
    Added(InstallationReposPayload),
    Removed(InstallationReposPayload),
    #[serde(other)]
    Other,
}

#[derive(Debug, Deserialize)]
pub struct InstallationReposPayload {
    pub installation: Installation,
    #[serde(default)]
    pub repositories_added: Vec<RepoRef>,
    #[serde(default)]
    pub repositories_removed: Vec<RepoRef>,
}

#[derive(Debug, Deserialize)]
pub struct RepoRef {
    pub id: i64,
    pub full_name: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_installation_created() {
        let json = serde_json::json!({
            "action": "created",
            "installation": {
                "id": 12345,
                "account": { "login": "octo", "type": "Organization", "id": 99 }
            }
        });
        let evt: InstallationEvent = serde_json::from_value(json).unwrap();
        let InstallationEvent::Created(p) = evt else {
            panic!("expected Created variant");
        };
        assert_eq!(p.installation.id, 12345);
        assert_eq!(p.installation.account.login, "octo");
        assert_eq!(p.installation.account.kind, "Organization");
        assert_eq!(p.installation.account.id, 99);
    }

    #[test]
    fn parses_installation_deleted_suspend_unsuspend() {
        for action in ["deleted", "suspend", "unsuspend"] {
            let json = serde_json::json!({
                "action": action,
                "installation": {
                    "id": 1,
                    "account": { "login": "x", "type": "User", "id": 2 }
                }
            });
            let evt: InstallationEvent = serde_json::from_value(json).unwrap();
            match (action, evt) {
                ("deleted", InstallationEvent::Deleted(_))
                | ("suspend", InstallationEvent::Suspend(_))
                | ("unsuspend", InstallationEvent::Unsuspend(_)) => {}
                (a, other) => panic!("action {a} parsed as {other:?}"),
            }
        }
    }

    #[test]
    fn parses_installation_unknown_action_as_other() {
        let json = serde_json::json!({
            "action": "new_permissions_accepted",
            "installation": {
                "id": 1,
                "account": { "login": "x", "type": "User", "id": 2 }
            }
        });
        let evt: InstallationEvent = serde_json::from_value(json).unwrap();
        assert!(matches!(evt, InstallationEvent::Other));
    }

    #[test]
    fn parses_installation_repositories_added() {
        let json = serde_json::json!({
            "action": "added",
            "installation": {
                "id": 1,
                "account": { "login": "x", "type": "User", "id": 2 }
            },
            "repositories_added": [
                { "id": 10, "full_name": "x/repo-a" },
                { "id": 11, "full_name": "x/repo-b" }
            ]
        });
        let evt: InstallationRepositoriesEvent = serde_json::from_value(json).unwrap();
        let InstallationRepositoriesEvent::Added(p) = evt else {
            panic!("expected Added");
        };
        assert_eq!(p.repositories_added.len(), 2);
        assert!(p.repositories_removed.is_empty());
    }

    #[test]
    fn parses_installation_repositories_removed() {
        let json = serde_json::json!({
            "action": "removed",
            "installation": {
                "id": 1,
                "account": { "login": "x", "type": "User", "id": 2 }
            },
            "repositories_removed": [
                { "id": 100, "full_name": "x/repo-c" }
            ]
        });
        let evt: InstallationRepositoriesEvent = serde_json::from_value(json).unwrap();
        let InstallationRepositoriesEvent::Removed(p) = evt else {
            panic!("expected Removed");
        };
        assert_eq!(p.repositories_removed.len(), 1);
        assert_eq!(p.repositories_removed[0].id, 100);
    }

    #[test]
    fn ignores_unknown_fields() {
        // Real GitHub payloads carry `repository`, `sender`, `requester`, …
        // None of those should fail deserialization.
        let json = serde_json::json!({
            "action": "deleted",
            "installation": {
                "id": 1,
                "account": { "login": "x", "type": "User", "id": 2 }
            },
            "sender": { "login": "octocat" },
            "extra_field_we_do_not_care_about": [1, 2, 3]
        });
        let evt: InstallationEvent = serde_json::from_value(json).unwrap();
        assert!(matches!(evt, InstallationEvent::Deleted(_)));
    }
}
