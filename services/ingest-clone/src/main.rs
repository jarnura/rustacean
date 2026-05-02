use std::sync::Arc;

use anyhow::{Context as _, Result};
use jsonwebtoken::EncodingKey;
use rb_blob::store_from_env;
use rb_github::GhApp;
use rb_kafka::{Consumer, ConsumerCfg, Producer, ProducerCfg};
use rb_schemas::{IngestRequest, IngestStatusEvent, SourceFileEvent};
use sqlx::postgres::PgPoolOptions;
use tokio::task::JoinHandle;

mod consumer;

#[tokio::main]
async fn main() -> Result<()> {
    let _guard = rb_tracing::init("ingest-clone")?;

    let database_url = std::env::var("DATABASE_URL")
        .context("DATABASE_URL is required")?;
    let pool = Arc::new(
        PgPoolOptions::new()
            .max_connections(5)
            .connect(&database_url)
            .await
            .context("failed to connect to Postgres")?,
    );

    let gh_app = build_gh_app()?;
    let gh_app = Arc::new(gh_app);

    let blob_store = store_from_env().await.context("failed to init blob store")?;

    let consumer: Consumer<IngestRequest> =
        Consumer::new(&ConsumerCfg::new("ingest-clone-worker"))?;
    consumer.subscribe(&[consumer::TOPIC_CLONE_COMMANDS])?;

    let source_producer = Arc::new(Producer::<SourceFileEvent>::new(&ProducerCfg::default())?);
    let expand_producer = Arc::new(Producer::<IngestRequest>::new(&ProducerCfg::default())?);
    let status_producer = Arc::new(Producer::<IngestStatusEvent>::new(&ProducerCfg::default())?);

    tracing::info!("ingest-clone starting");

    let handle: JoinHandle<()> = tokio::spawn(consumer::run(
        consumer,
        pool,
        gh_app,
        blob_store,
        source_producer,
        expand_producer,
        status_producer,
    ));

    shutdown_signal().await;
    tracing::info!("shutdown signal received — stopping consumer");
    handle.abort();

    Ok(())
}

fn build_gh_app() -> Result<GhApp> {
    let app_id: i64 = std::env::var("GITHUB_APP_ID")
        .context("GITHUB_APP_ID is required")?
        .parse()
        .context("GITHUB_APP_ID must be a number")?;

    let private_key_pem = std::env::var("GITHUB_APP_PRIVATE_KEY_PEM")
        .context("GITHUB_APP_PRIVATE_KEY_PEM is required")?;

    let encoding_key = EncodingKey::from_rsa_pem(private_key_pem.as_bytes())
        .context("invalid GITHUB_APP_PRIVATE_KEY_PEM")?;

    let webhook_secret_raw = std::env::var("GITHUB_WEBHOOK_SECRET")
        .unwrap_or_default()
        .into_bytes();

    Ok(GhApp::new(
        app_id,
        encoding_key,
        rb_github::Secret::new(webhook_secret_raw),
    ))
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
