use std::sync::Arc;

use anyhow::{Context as _, Result};
use rb_blob::store_from_env;
use rb_kafka::{Consumer, ConsumerCfg, Producer, ProducerCfg};
use rb_schemas::{GraphRelationEvent, IngestStatusEvent, TypecheckedItemEvent};

mod consumer;
mod extractor;

#[tokio::main]
async fn main() -> Result<()> {
    let _guard = rb_tracing::init("ingest-graph")?;

    let blob_store = store_from_env().await.context("failed to init blob store")?;

    let item_consumer: Consumer<TypecheckedItemEvent> =
        Consumer::new(&ConsumerCfg::new("ingest-graph"))?;
    item_consumer.subscribe(&[consumer::TOPIC_TYPECHECKED_ITEMS])?;

    let relation_producer =
        Arc::new(Producer::<GraphRelationEvent>::new(&ProducerCfg::default())?);
    let status_producer =
        Arc::new(Producer::<IngestStatusEvent>::new(&ProducerCfg::default())?);

    tracing::info!("ingest-graph starting");

    let handle = tokio::spawn(consumer::run(
        item_consumer,
        blob_store,
        relation_producer,
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
