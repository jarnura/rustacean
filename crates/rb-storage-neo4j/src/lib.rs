//! `rb-storage-neo4j` — the **only** approved Neo4j write path for the platform.
//!
//! All callers MUST use [`execute`] or [`run`] instead of `neo4rs::Graph` directly.
//! The CI job `forbid-direct-neo4j-writes` enforces this at build time.

mod error;
mod injector;
mod label;

pub use error::CypherError;
pub use injector::inject_tenant_label;

use rb_schemas::TenantId;
use std::collections::HashMap;

/// Neo4j query parameters. Keys are parameter names (without the leading `$`).
pub type Params = HashMap<String, neo4rs::BoltType>;

/// Result type for this crate.
pub type Result<T> = std::result::Result<T, CypherError>;

/// Executes `cypher` against `graph` with tenant-label isolation enforced.
///
/// Before execution:
/// 1. Rejects multi-statement Cypher (bare `;`) with [`CypherError::MultiStatement`].
/// 2. Injects the tenant label onto every node pattern in
///    `MATCH` / `MERGE` / `CREATE` / `OPTIONAL MATCH` clauses.
///
/// # Errors
///
/// - [`CypherError::MultiStatement`]
/// - [`CypherError::UnclosedNodePattern`]
/// - [`CypherError::Neo4j`]
pub async fn execute(
    graph: &neo4rs::Graph,
    cypher: &str,
    tenant: &TenantId,
    params: Params,
) -> Result<Vec<neo4rs::Row>> {
    let lbl = label::tenant_label(tenant);
    let safe_cypher = inject_tenant_label(cypher, &lbl)?;
    let q = params
        .into_iter()
        .fold(neo4rs::query(&safe_cypher), |q, (k, v)| q.param(&k, v));
    let mut stream = graph.execute(q).await.map_err(CypherError::Neo4j)?;
    let mut rows = Vec::new();
    loop {
        match stream.next().await {
            Ok(Some(row)) => rows.push(row),
            Ok(None) => break,
            Err(e) => return Err(CypherError::Neo4j(e)),
        }
    }
    Ok(rows)
}

/// Fire-and-forget variant of [`execute`] for writes that return no rows.
///
/// # Errors
///
/// Same as [`execute`].
pub async fn run(
    graph: &neo4rs::Graph,
    cypher: &str,
    tenant: &TenantId,
    params: Params,
) -> Result<()> {
    let lbl = label::tenant_label(tenant);
    let safe_cypher = inject_tenant_label(cypher, &lbl)?;
    let q = params
        .into_iter()
        .fold(neo4rs::query(&safe_cypher), |q, (k, v)| q.param(&k, v));
    Ok(graph.run(q).await?)
}
