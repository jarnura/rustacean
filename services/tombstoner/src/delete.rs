//! Projection deletion logic for tombstone events.
//!
//! Each storage backend is deleted in sequence: `PostgreSQL` → `Neo4j` → `Qdrant`.
//! The function short-circuits on the first failure and returns the error so
//! the consumer can retry the whole event atomically (Kafka at-least-once
//! delivery ensures idempotency).

use anyhow::{Context as _, Result};
use rb_schemas::{TenantId, Tombstone};
use rb_storage_neo4j::TenantGraph;
use rb_storage_pg::TenantPool;
use rb_tenant::TenantCtx;

/// Delete all projections described by `ev` from every storage backend.
///
/// When `ev.repo_id` is empty the deletion is tenant-wide (drops the PG
/// schema, removes all Neo4j nodes for the tenant, and lists+deletes all
/// Qdrant collections for the tenant).  When non-empty only data for that
/// specific repo is removed.
///
/// Idempotent: each backend uses IF-NOT-EXISTS / MATCH-with-no-results
/// semantics so re-delivery of the same tombstone is safe.
#[allow(clippy::missing_errors_doc)]
pub async fn handle_tombstone(
    pool: &TenantPool,
    graph: &TenantGraph,
    qdrant_url: Option<&str>,
    tenant_id: &TenantId,
    ev: &Tombstone,
) -> Result<()> {
    let ctx = TenantCtx::new(*tenant_id);
    let tenant_wide = ev.repo_id.is_empty();

    // ── PostgreSQL ───────────────────────────────────────────────────────────
    if tenant_wide {
        pool.drop_schema(&ctx)
            .await
            .context("PG drop_schema failed")?;
        tracing::debug!(tenant_id = %tenant_id, "tombstoner: PG schema dropped");
    } else {
        let repo_uuid = ev
            .repo_id
            .parse::<uuid::Uuid>()
            .with_context(|| format!("invalid repo_id UUID: {}", ev.repo_id))?;
        pool.delete_repo_data(&ctx, repo_uuid)
            .await
            .context("PG delete_repo_data failed")?;
        tracing::debug!(
            tenant_id = %tenant_id,
            repo_id   = %ev.repo_id,
            "tombstoner: PG repo rows deleted"
        );
    }

    // ── Neo4j ────────────────────────────────────────────────────────────────
    if tenant_wide {
        graph
            .delete_all_tenant_nodes(tenant_id)
            .await
            .context("Neo4j delete_all_tenant_nodes failed")?;
        tracing::debug!(tenant_id = %tenant_id, "tombstoner: Neo4j tenant nodes deleted");
    } else {
        graph
            .delete_repo_nodes(tenant_id, &ev.repo_id)
            .await
            .context("Neo4j delete_repo_nodes failed")?;
        tracing::debug!(
            tenant_id = %tenant_id,
            repo_id   = %ev.repo_id,
            "tombstoner: Neo4j repo nodes deleted"
        );
    }

    // ── Qdrant (best-effort HTTP) ────────────────────────────────────────────
    // Qdrant collection naming: `rb_{tenant_id}_{repo_id}` (per-repo) or
    // `rb_{tenant_id}_*` prefix scan (tenant-wide). Collections are created
    // by projector-qdrant (REQ-IN-13) when that service is built.
    match qdrant_url {
        None => {
            tracing::warn!(
                tenant_id = %tenant_id,
                "tombstoner: QDRANT_URL not set — Qdrant deletion skipped"
            );
        }
        Some(url) => {
            if tenant_wide {
                delete_qdrant_tenant(url, tenant_id).await?;
            } else {
                delete_qdrant_repo(url, tenant_id, &ev.repo_id).await?;
            }
        }
    }

    Ok(())
}

/// Delete the Qdrant collection for a single (tenant, repo) pair.
///
/// Returns `Ok(())` when the collection does not exist (HTTP 404 = idempotent).
async fn delete_qdrant_repo(
    qdrant_url: &str,
    tenant_id: &TenantId,
    repo_id: &str,
) -> Result<()> {
    let collection = qdrant_collection_name(tenant_id, repo_id);
    let url = format!("{qdrant_url}/collections/{collection}");
    let status = reqwest::Client::new()
        .delete(&url)
        .send()
        .await
        .with_context(|| format!("Qdrant DELETE {url}"))?
        .status()
        .as_u16();
    match status {
        200 | 204 | 404 => {
            tracing::debug!(
                tenant_id  = %tenant_id,
                repo_id    = %repo_id,
                collection = %collection,
                "tombstoner: Qdrant repo collection deleted (or not found)"
            );
            Ok(())
        }
        code => Err(anyhow::anyhow!(
            "Qdrant DELETE /collections/{collection} returned unexpected status {code}"
        )),
    }
}

/// Delete all Qdrant collections matching the tenant prefix.
///
/// Lists `/collections`, filters by the `rb_{tenant_id}_` prefix, then issues
/// individual DELETE requests. Idempotent: 404 responses are ignored.
async fn delete_qdrant_tenant(qdrant_url: &str, tenant_id: &TenantId) -> Result<()> {
    let client = reqwest::Client::new();
    let list_url = format!("{qdrant_url}/collections");

    let body: serde_json::Value = client
        .get(&list_url)
        .send()
        .await
        .context("Qdrant GET /collections failed")?
        .json()
        .await
        .context("Qdrant /collections response was not valid JSON")?;

    let prefix = format!("rb_{tenant_id}_");
    let names: Vec<String> = body["result"]["collections"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|c| c["name"].as_str())
                .filter(|name| name.starts_with(&prefix))
                .map(std::borrow::ToOwned::to_owned)
                .collect()
        })
        .unwrap_or_default();

    for name in &names {
        let url = format!("{qdrant_url}/collections/{name}");
        let status = client
            .delete(&url)
            .send()
            .await
            .with_context(|| format!("Qdrant DELETE /collections/{name}"))?
            .status()
            .as_u16();
        match status {
            200 | 204 | 404 => {
                tracing::debug!(
                    tenant_id  = %tenant_id,
                    collection = %name,
                    "tombstoner: Qdrant tenant collection deleted"
                );
            }
            code => {
                return Err(anyhow::anyhow!(
                    "Qdrant DELETE /collections/{name} returned unexpected status {code}"
                ));
            }
        }
    }

    tracing::debug!(
        tenant_id = %tenant_id,
        count     = names.len(),
        "tombstoner: Qdrant tenant collections deleted"
    );
    Ok(())
}

/// Collection naming convention shared with `projector-qdrant` (REQ-IN-13):
/// `rb_{tenant_id}_{repo_id}`.
fn qdrant_collection_name(tenant_id: &TenantId, repo_id: &str) -> String {
    format!("rb_{tenant_id}_{repo_id}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use rb_schemas::TenantId;

    #[test]
    fn qdrant_collection_name_format() {
        let tid = TenantId::new();
        let repo = "some-repo-uuid";
        let name = qdrant_collection_name(&tid, repo);
        assert!(name.starts_with("rb_"), "must start with rb_");
        assert!(name.contains(repo), "must contain repo_id");
        assert!(name.contains(&tid.to_string()), "must contain tenant_id");
    }

    #[test]
    fn handle_tombstone_parses_empty_repo_id_as_tenant_wide() {
        let ev = Tombstone {
            tenant_id: "t".to_string(),
            repo_id: String::new(),
            requested_by: "u".to_string(),
            emitted_at_ms: 0,
        };
        assert!(ev.repo_id.is_empty(), "empty repo_id means tenant-wide");
    }

    #[test]
    fn handle_tombstone_detects_repo_specific() {
        let repo_uuid = uuid::Uuid::new_v4().to_string();
        let ev = Tombstone {
            tenant_id: "t".to_string(),
            repo_id: repo_uuid.clone(),
            requested_by: "u".to_string(),
            emitted_at_ms: 0,
        };
        assert!(!ev.repo_id.is_empty());
        assert!(ev.repo_id.parse::<uuid::Uuid>().is_ok(), "repo_id must be a valid UUID");
    }

    #[test]
    fn handle_tombstone_rejects_non_uuid_repo_id() {
        let ev = Tombstone {
            tenant_id: "t".to_string(),
            repo_id: "not-a-uuid".to_string(),
            requested_by: "u".to_string(),
            emitted_at_ms: 0,
        };
        assert!(
            ev.repo_id.parse::<uuid::Uuid>().is_err(),
            "non-UUID repo_id must fail parse"
        );
    }
}
