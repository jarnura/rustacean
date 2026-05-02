use std::sync::Arc;

use anyhow::{Context as _, Result};
use rb_blob::store_from_env;
use rb_kafka::{Consumer, ConsumerCfg, Producer, ProducerCfg};
use rb_schemas::{IngestRequest, IngestStatusEvent, ParsedItemEvent};

mod consumer;
mod helpers;
mod parse_strategy;

#[tokio::main]
async fn main() -> Result<()> {
    let _guard = rb_tracing::init("parse-worker")?;

    let blob_store = store_from_env().await.context("failed to init blob store")?;

    let cmd_consumer: Consumer<IngestRequest> =
        Consumer::new(&ConsumerCfg::new("parse-worker"))?;
    cmd_consumer.subscribe(&[consumer::TOPIC_PARSE_COMMANDS])?;

    let item_producer = Arc::new(Producer::<ParsedItemEvent>::new(&ProducerCfg::default())?);
    let typecheck_producer = Arc::new(Producer::<IngestRequest>::new(&ProducerCfg::default())?);
    let status_producer = Arc::new(Producer::<IngestStatusEvent>::new(&ProducerCfg::default())?);

    tracing::info!("parse-worker starting");

    let handle = tokio::spawn(consumer::run(
        cmd_consumer,
        blob_store,
        item_producer,
        typecheck_producer,
        status_producer,
    ));

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
