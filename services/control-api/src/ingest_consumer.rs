use std::sync::Arc;

use rb_kafka::{Consumer, ConsumerCfg, TraceContext};
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

/// `OTel` [`Extractor`] over a borrowed `&[(String, String)]` header list.
/// Scoped to this module — mirrors `KafkaHeaderExtractor` from `rb-kafka` without
/// requiring it to be re-exported.
///
/// [`Extractor`]: opentelemetry::propagation::Extractor
struct HeaderExtractor<'a>(&'a [(String, String)]);

impl opentelemetry::propagation::Extractor for HeaderExtractor<'_> {
    fn get(&self, key: &str) -> Option<&str> {
        self.0
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(key))
            .map(|(_, v)| v.as_str())
    }

    fn keys(&self) -> Vec<&str> {
        self.0.iter().map(|(k, _)| k.as_str()).collect()
    }
}

/// Build a `sse.publish` span that is a child of the upstream producer's trace.
///
/// We re-extract the W3C trace context from the envelope so the span shares the
/// same trace id as `kafka.produce` and `kafka.consume`.  If no trace context is
/// present the span is still emitted but without a remote parent.
///
/// The guard returned by `attach()` is intentionally scoped to this helper —
/// it is dropped before any `.await` point in the caller (ADR-006 §9.3).
fn sse_publish_span(
    tenant_id: &rb_schemas::TenantId,
    tc: Option<&TraceContext>,
) -> tracing::Span {
    // Scope the context guard to span construction only.
    // The guard is dropped when this block exits; the parent relationship is
    // already captured inside the Span at creation time.
    let _cx_guard = tc.map(|tc| {
        let headers: Vec<(String, String)> = vec![
            ("traceparent".to_owned(), tc.traceparent.clone()),
            ("tracestate".to_owned(), tc.tracestate.clone()),
        ];
        opentelemetry::global::get_text_map_propagator(|prop| {
            prop.extract(&HeaderExtractor(&headers))
        })
        .attach()
    });

    tracing::info_span!(
        "sse.publish",
        "otel.kind" = "PRODUCER",
        "messaging.system" = "sse",
        "messaging.destination" = "ingest.events",
        "rb.tenant_id" = %tenant_id,
    )
    // _cx_guard dropped here — parent relationship already captured
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
                        Ok(data) => {
                            // Build a sse.publish span in the same OTel trace as the producer.
                            // in_scope ensures the entry guard never crosses an .await boundary.
                            let span =
                                sse_publish_span(&tenant_id, envelope.trace_context.as_ref());
                            span.in_scope(|| {
                                sse_bus.publish_raw(&tenant_id, "ingest.status", data);
                            });
                        }
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
