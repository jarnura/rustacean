//! SSE event formatting and OpenTelemetry tracing helpers.

use rb_kafka::TraceContext;
use rb_schemas::{IngestStage, IngestStatus, IngestStatusEvent};
use serde::Serialize;

/// JSON-serialisable mirror of [`IngestStatusEvent`] for SSE wire format.
///
/// `IngestStatusEvent` is prost-generated and does not implement [`Serialize`];
/// this newtype bridges the gap without modifying generated code.
#[derive(Debug, Serialize)]
pub(crate) struct IngestStatusEventJson {
    pub(crate) ingest_request_id: String,
    pub(crate) tenant_id: String,
    pub(crate) status: &'static str,
    pub(crate) error_message: String,
    pub(crate) occurred_at_ms: i64,
    pub(crate) stage: Option<&'static str>,
    pub(crate) stage_seq: i32,
    pub(crate) ingest_run_id: String,
}

impl From<&IngestStatusEvent> for IngestStatusEventJson {
    fn from(ev: &IngestStatusEvent) -> Self {
        let status = IngestStatus::try_from(ev.status).map_or("unknown", status_label);
        let stage = IngestStage::try_from(ev.stage).ok().and_then(stage_label);
        Self {
            ingest_request_id: ev.ingest_request_id.clone(),
            tenant_id: ev.tenant_id.clone(),
            status,
            error_message: ev.error_message.clone(),
            occurred_at_ms: ev.occurred_at_ms,
            stage,
            stage_seq: ev.stage_seq,
            ingest_run_id: ev.ingest_run_id.clone(),
        }
    }
}

pub(crate) fn status_label(s: IngestStatus) -> &'static str {
    match s {
        IngestStatus::Unspecified => "unspecified",
        IngestStatus::Pending => "pending",
        IngestStatus::Processing => "processing",
        IngestStatus::Done => "done",
        IngestStatus::Failed => "failed",
    }
}

pub(crate) fn stage_label(s: IngestStage) -> Option<&'static str> {
    match s {
        IngestStage::Unspecified => None,
        IngestStage::Clone => Some("clone"),
        IngestStage::Expand => Some("expand"),
        IngestStage::Parse => Some("parse"),
        IngestStage::Typecheck => Some("typecheck"),
        IngestStage::Extract => Some("extract"),
        IngestStage::Embed => Some("embed"),
        IngestStage::ProjectPg => Some("project_pg"),
        IngestStage::ProjectNeo4j => Some("project_neo4j"),
        IngestStage::ProjectQdrant => Some("project_qdrant"),
    }
}

/// `OTel` [`Extractor`] over a borrowed `&[(String, String)]` header list.
pub(crate) struct HeaderExtractor<'a>(pub(crate) &'a [(String, String)]);

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

pub(crate) fn sse_publish_span(
    tenant_id: &rb_schemas::TenantId,
    tc: Option<&TraceContext>,
) -> tracing::Span {
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
}
