use std::sync::Arc;

use rb_kafka::{Consumer, ConsumerCfg};
use rb_schemas::{IngestStatus, IngestStatusEvent};
use rb_sse::EventBus;
use serde::Serialize;

/// JSON-serialisable mirror of [`IngestStatusEvent`] for SSE wire format.
///
/// `IngestStatusEvent` is prost-generated and does not implement [`Serialize`];
/// this newtype bridges the gap without modifying generated code.
#[derive(Debug, Serialize)]
struct IngestStatusEventJson {
    ingest_request_id: String,
    tenant_id: String,
    status: &'static str,
    error_message: String,
    occurred_at_ms: i64,
}

impl From<&IngestStatusEvent> for IngestStatusEventJson {
    fn from(ev: &IngestStatusEvent) -> Self {
        let status = IngestStatus::try_from(ev.status).map_or("unknown", status_label);
        Self {
            ingest_request_id: ev.ingest_request_id.clone(),
            tenant_id: ev.tenant_id.clone(),
            status,
            error_message: ev.error_message.clone(),
            occurred_at_ms: ev.occurred_at_ms,
        }
    }
}

fn status_label(s: IngestStatus) -> &'static str {
    match s {
        IngestStatus::Unspecified => "unspecified",
        IngestStatus::Pending => "pending",
        IngestStatus::Processing => "processing",
        IngestStatus::Done => "done",
        IngestStatus::Failed => "failed",
    }
}

/// Spawn the long-running Kafka consumer task that subscribes to
/// `rb.projector.events` and fans events out through the SSE bus.
///
/// Returns the [`tokio::task::JoinHandle`] so the caller can abort on shutdown.
/// Consumer errors are logged and retried; the task only exits on an
/// unrecoverable Kafka init failure (returned as `Err`).
///
/// # Errors
///
/// Returns an error if the Kafka consumer cannot be created or subscribed.
pub fn spawn(
    cfg: &ConsumerCfg,
    sse_bus: Arc<EventBus>,
) -> anyhow::Result<tokio::task::JoinHandle<()>> {
    let consumer = Consumer::<IngestStatusEvent>::new(cfg)?;
    consumer.subscribe(&["rb.projector.events"])?;

    let handle = tokio::spawn(async move {
        loop {
            match consumer.next().await {
                None => {
                    tracing::info!("ingest_consumer: stream ended");
                    break;
                }
                Some(Err(e)) => {
                    tracing::error!("ingest_consumer: kafka error: {e}");
                    // Brief back-off before retrying to avoid tight error loops.
                    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                }
                Some(Ok(envelope)) => {
                    let tenant_id = envelope.tenant_id;
                    let json_ev = IngestStatusEventJson::from(&envelope.payload);

                    match serde_json::to_string(&json_ev) {
                        Ok(data) => sse_bus.publish_raw(&tenant_id, "ingest.status", data),
                        Err(e) => {
                            tracing::error!("ingest_consumer: serialise error: {e}");
                        }
                    }

                    if let Err(e) = consumer.commit(&envelope).await {
                        tracing::warn!("ingest_consumer: commit failed: {e}");
                    }
                }
            }
        }
    });

    Ok(handle)
}
