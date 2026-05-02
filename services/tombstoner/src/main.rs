use std::sync::Arc;

use anyhow::{Context as _, Result};
use rb_storage_neo4j::TenantGraph;
use rb_storage_pg::TenantPool;

mod consumer;
mod delete;

#[tokio::main]
async fn main() -> Result<()> {
    let _guard = rb_tracing::init("tombstoner")?;

    let database_url = std::env::var("DATABASE_URL")
        .context("DATABASE_URL is required")?;

    let pg = sqlx::postgres::PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await
        .context("failed to connect to PostgreSQL")?;
    let pool = Arc::new(TenantPool::new(pg));

    let neo4j_uri = std::env::var("NEO4J_URI")
        .unwrap_or_else(|_| "bolt://neo4j:7687".to_owned());
    let neo4j_user = std::env::var("NEO4J_USER").unwrap_or_else(|_| "neo4j".to_owned());
    let neo4j_password =
        std::env::var("NEO4J_PASSWORD").context("NEO4J_PASSWORD is required")?;

    let graph = TenantGraph::connect(&neo4j_uri, &neo4j_user, &neo4j_password)
        .await
        .context("failed to connect to Neo4j")?;
    let graph = Arc::new(graph);

    // Optional Qdrant REST endpoint; tombstoner skips Qdrant deletion when unset.
    let qdrant_url = std::env::var("QDRANT_URL").ok();

    tracing::info!("tombstoner starting");

    let handle = consumer::spawn(pool, graph, qdrant_url)?;

    shutdown_signal().await;
    tracing::info!("shutdown signal received — stopping consumer");
    handle.abort();

    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install CTRL+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = ctrl_c => {},
        () = terminate => {},
    }
}
