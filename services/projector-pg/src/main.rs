use std::sync::Arc;

use anyhow::{Context as _, Result};
use rb_storage_pg::TenantPool;

use projector_pg::spawn;

#[tokio::main]
async fn main() -> Result<()> {
    let _guard = rb_tracing::init("projector-pg")?;

    let database_url = std::env::var("DATABASE_URL")
        .context("DATABASE_URL is required")?;

    let pg = sqlx::postgres::PgPoolOptions::new()
        .max_connections(10)
        .connect(&database_url)
        .await
        .context("failed to connect to PostgreSQL")?;
    let pool = TenantPool::new(pg);
    let pool = Arc::new(pool);

    tracing::info!("projector-pg starting");

    let handle = spawn(pool)?;

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
