use std::sync::Arc;

use anyhow::{Context as _, Result};
use rb_kafka::{Consumer, ConsumerCfg, TraceContext};
use rb_schemas::{IngestStage, IngestStatus, IngestStatusEvent};
use rb_sse::EventBus;
use serde::Serialize;
use sqlx::PgPool;
use uuid::Uuid;

const TOTAL_PIPELINE_STAGES: i64 = 9;

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
    stage: Option<&'static str>,
    stage_seq: i32,
    ingest_run_id: String,
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

fn status_label(s: IngestStatus) -> &'static str {
    match s {
        IngestStatus::Unspecified => "unspecified",
        IngestStatus::Pending => "pending",
        IngestStatus::Processing => "processing",
        IngestStatus::Done => "done",
        IngestStatus::Failed => "failed",
    }
}

fn stage_label(s: IngestStage) -> Option<&'static str> {
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

/// Returns the `pipeline_stage_runs.status` string and optional error
/// for a given [`IngestStatus`], or `None` if no DB update is warranted.
fn stage_db_params(
    status: IngestStatus,
    error_message: &str,
) -> Option<(&'static str, Option<String>)> {
    match status {
        IngestStatus::Processing => Some(("running", None)),
        IngestStatus::Done => Some(("succeeded", None)),
        IngestStatus::Failed => {
            let err = if error_message.is_empty() {
                None
            } else {
                Some(error_message.to_owned())
            };
            Some(("failed", err))
        }
        IngestStatus::Pending | IngestStatus::Unspecified => None,
    }
}

/// `OTel` [`Extractor`] over a borrowed `&[(String, String)]` header list.
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

fn sse_publish_span(
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

/// Update `pipeline_stage_runs` for the given run + stage transition.
async fn update_stage_run(
    pool: &PgPool,
    ingest_run_id: &str,
    stage: &str,
    db_status: &str,
    error: Option<String>,
) -> Result<()> {
    let run_id: Uuid = ingest_run_id
        .parse()
        .context("invalid ingest_run_id UUID")?;

    match db_status {
        "running" => {
            sqlx::query(
                "UPDATE control.pipeline_stage_runs \
                 SET status = 'running', started_at = now() \
                 WHERE ingestion_run_id = $1 AND stage = $2",
            )
            .bind(run_id)
            .bind(stage)
            .execute(pool)
            .await
            .context("failed to update pipeline_stage_runs to running")?;
        }
        "succeeded" => {
            sqlx::query(
                "UPDATE control.pipeline_stage_runs \
                 SET status = 'succeeded', finished_at = now() \
                 WHERE ingestion_run_id = $1 AND stage = $2",
            )
            .bind(run_id)
            .bind(stage)
            .execute(pool)
            .await
            .context("failed to update pipeline_stage_runs to succeeded")?;
        }
        "failed" => {
            sqlx::query(
                "UPDATE control.pipeline_stage_runs \
                 SET status = 'failed', finished_at = now(), error = $3 \
                 WHERE ingestion_run_id = $1 AND stage = $2",
            )
            .bind(run_id)
            .bind(stage)
            .bind(error.as_deref())
            .execute(pool)
            .await
            .context("failed to update pipeline_stage_runs to failed")?;
        }
        other => {
            tracing::warn!(db_status = other, "unknown stage db_status — skipping");
        }
    }

    Ok(())
}

/// Transition `ingestion_runs` when a stage reports `Processing` (first signal
/// that work has started: move from `queued` → `running`).
async fn maybe_start_run(pool: &PgPool, ingest_run_id: &str) -> Result<()> {
    let run_id: Uuid = ingest_run_id
        .parse()
        .context("invalid ingest_run_id UUID")?;

    sqlx::query(
        "UPDATE control.ingestion_runs \
         SET status = 'running', started_at = COALESCE(started_at, now()) \
         WHERE id = $1 AND status = 'queued'",
    )
    .bind(run_id)
    .execute(pool)
    .await
    .context("failed to transition ingestion_run to running")?;

    Ok(())
}

/// If all pipeline stages have succeeded, mark the run `succeeded`.
async fn maybe_complete_run(pool: &PgPool, ingest_run_id: &str) -> Result<()> {
    let run_id: Uuid = ingest_run_id
        .parse()
        .context("invalid ingest_run_id UUID")?;

    let succeeded: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM control.pipeline_stage_runs \
         WHERE ingestion_run_id = $1 AND status = 'succeeded'",
    )
    .bind(run_id)
    .fetch_one(pool)
    .await
    .context("failed to count succeeded stages")?;

    if succeeded >= TOTAL_PIPELINE_STAGES {
        sqlx::query(
            "UPDATE control.ingestion_runs \
             SET status = 'succeeded', finished_at = now() \
             WHERE id = $1 AND status = 'running'",
        )
        .bind(run_id)
        .execute(pool)
        .await
        .context("failed to mark ingestion_run succeeded")?;
    }

    Ok(())
}

/// Mark `ingestion_runs` as `failed` on any stage failure.
async fn fail_run(pool: &PgPool, ingest_run_id: &str, error_message: &str) -> Result<()> {
    let run_id: Uuid = ingest_run_id
        .parse()
        .context("invalid ingest_run_id UUID")?;

    let error = if error_message.is_empty() {
        None
    } else {
        Some(error_message)
    };

    sqlx::query(
        "UPDATE control.ingestion_runs \
         SET status = 'failed', finished_at = now(), error = $2 \
         WHERE id = $1 AND status IN ('queued', 'running')",
    )
    .bind(run_id)
    .bind(error)
    .execute(pool)
    .await
    .context("failed to mark ingestion_run failed")?;

    Ok(())
}

/// Apply all DB updates for one [`IngestStatusEvent`].
async fn handle_db_updates(pool: &PgPool, ev: &IngestStatusEvent) -> Result<()> {
    let status = IngestStatus::try_from(ev.status).unwrap_or(IngestStatus::Unspecified);
    let stage_opt = IngestStage::try_from(ev.stage).ok().and_then(stage_label);

    if let Some(stage_str) = stage_opt {
        if let Some((db_status, error)) = stage_db_params(status, &ev.error_message) {
            update_stage_run(pool, &ev.ingest_run_id, stage_str, db_status, error).await?;
        }
    }

    match status {
        IngestStatus::Processing => {
            maybe_start_run(pool, &ev.ingest_run_id).await?;
        }
        IngestStatus::Done => {
            maybe_complete_run(pool, &ev.ingest_run_id).await?;
        }
        IngestStatus::Failed => {
            fail_run(pool, &ev.ingest_run_id, &ev.error_message).await?;
        }
        IngestStatus::Pending | IngestStatus::Unspecified => {}
    }

    Ok(())
}

/// Spawn the long-running Kafka consumer task that subscribes to
/// `rb.projector.events`, persists status transitions to Postgres, and fans
/// events out through the SSE bus.
///
/// Returns the [`tokio::task::JoinHandle`] so the caller can abort on shutdown.
///
/// # Errors
///
/// Returns an error if the Kafka consumer cannot be created or subscribed.
pub fn spawn(
    cfg: &ConsumerCfg,
    sse_bus: Arc<EventBus>,
    pool: Arc<PgPool>,
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
                    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                }
                Some(Ok(envelope)) => {
                    let ev = &envelope.payload;
                    let tenant_id = envelope.tenant_id;

                    // Persist to DB before broadcasting; skip commit on error so
                    // the message is redelivered on restart.
                    if let Err(e) = handle_db_updates(&pool, ev).await {
                        tracing::error!(
                            ingest_run_id = %ev.ingest_run_id,
                            "ingest_consumer: DB update failed: {e}"
                        );
                        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                        continue;
                    }

                    let json_ev = IngestStatusEventJson::from(ev);
                    match serde_json::to_string(&json_ev) {
                        Ok(data) => {
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

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // ── status_label ─────────────────────────────────────────────────────────

    #[test]
    fn status_label_unspecified() {
        assert_eq!(status_label(IngestStatus::Unspecified), "unspecified");
    }

    #[test]
    fn status_label_pending() {
        assert_eq!(status_label(IngestStatus::Pending), "pending");
    }

    #[test]
    fn status_label_processing() {
        assert_eq!(status_label(IngestStatus::Processing), "processing");
    }

    #[test]
    fn status_label_done() {
        assert_eq!(status_label(IngestStatus::Done), "done");
    }

    #[test]
    fn status_label_failed() {
        assert_eq!(status_label(IngestStatus::Failed), "failed");
    }

    // ── stage_label ──────────────────────────────────────────────────────────

    #[test]
    fn stage_label_unspecified_is_none() {
        assert!(stage_label(IngestStage::Unspecified).is_none());
    }

    #[test]
    fn stage_label_clone() {
        assert_eq!(stage_label(IngestStage::Clone), Some("clone"));
    }

    #[test]
    fn stage_label_expand() {
        assert_eq!(stage_label(IngestStage::Expand), Some("expand"));
    }

    #[test]
    fn stage_label_parse() {
        assert_eq!(stage_label(IngestStage::Parse), Some("parse"));
    }

    #[test]
    fn stage_label_typecheck() {
        assert_eq!(stage_label(IngestStage::Typecheck), Some("typecheck"));
    }

    #[test]
    fn stage_label_extract() {
        assert_eq!(stage_label(IngestStage::Extract), Some("extract"));
    }

    #[test]
    fn stage_label_embed() {
        assert_eq!(stage_label(IngestStage::Embed), Some("embed"));
    }

    #[test]
    fn stage_label_project_pg() {
        assert_eq!(stage_label(IngestStage::ProjectPg), Some("project_pg"));
    }

    #[test]
    fn stage_label_project_neo4j() {
        assert_eq!(
            stage_label(IngestStage::ProjectNeo4j),
            Some("project_neo4j")
        );
    }

    #[test]
    fn stage_label_project_qdrant() {
        assert_eq!(
            stage_label(IngestStage::ProjectQdrant),
            Some("project_qdrant")
        );
    }

    // ── stage_db_params ──────────────────────────────────────────────────────

    #[test]
    fn stage_db_params_processing() {
        let (status, err) = stage_db_params(IngestStatus::Processing, "").unwrap();
        assert_eq!(status, "running");
        assert!(err.is_none());
    }

    #[test]
    fn stage_db_params_done() {
        let (status, err) = stage_db_params(IngestStatus::Done, "").unwrap();
        assert_eq!(status, "succeeded");
        assert!(err.is_none());
    }

    #[test]
    fn stage_db_params_failed_with_message() {
        let (status, err) = stage_db_params(IngestStatus::Failed, "timeout").unwrap();
        assert_eq!(status, "failed");
        assert_eq!(err, Some("timeout".to_owned()));
    }

    #[test]
    fn stage_db_params_failed_empty_message() {
        let (status, err) = stage_db_params(IngestStatus::Failed, "").unwrap();
        assert_eq!(status, "failed");
        assert!(err.is_none());
    }

    #[test]
    fn stage_db_params_pending_is_none() {
        assert!(stage_db_params(IngestStatus::Pending, "").is_none());
    }

    #[test]
    fn stage_db_params_unspecified_is_none() {
        assert!(stage_db_params(IngestStatus::Unspecified, "").is_none());
    }

    // ── IngestStatusEventJson::from ───────────────────────────────────────────

    fn make_event(status: i32, stage: i32, error: &str) -> IngestStatusEvent {
        IngestStatusEvent {
            ingest_request_id: "req-1".to_owned(),
            tenant_id: "tenant-1".to_owned(),
            status,
            error_message: error.to_owned(),
            occurred_at_ms: 1_700_000_000_000,
            stage,
            stage_seq: 1,
            ingest_run_id: "run-1".to_owned(),
            attempt: 1,
        }
    }

    #[test]
    fn json_ev_processing_clone() {
        let ev = make_event(
            IngestStatus::Processing as i32,
            IngestStage::Clone as i32,
            "",
        );
        let j = IngestStatusEventJson::from(&ev);
        assert_eq!(j.status, "processing");
        assert_eq!(j.stage, Some("clone"));
        assert_eq!(j.ingest_run_id, "run-1");
        assert_eq!(j.stage_seq, 1);
    }

    #[test]
    fn json_ev_done_embed() {
        let ev = make_event(IngestStatus::Done as i32, IngestStage::Embed as i32, "");
        let j = IngestStatusEventJson::from(&ev);
        assert_eq!(j.status, "done");
        assert_eq!(j.stage, Some("embed"));
    }

    #[test]
    fn json_ev_failed_has_error() {
        let ev = make_event(IngestStatus::Failed as i32, IngestStage::Parse as i32, "oom");
        let j = IngestStatusEventJson::from(&ev);
        assert_eq!(j.status, "failed");
        assert_eq!(j.error_message, "oom");
    }

    #[test]
    fn json_ev_unknown_status_label() {
        let ev = make_event(999, IngestStage::Clone as i32, "");
        let j = IngestStatusEventJson::from(&ev);
        assert_eq!(j.status, "unknown");
    }

    #[test]
    fn json_ev_unspecified_stage_is_none() {
        let ev = make_event(
            IngestStatus::Processing as i32,
            IngestStage::Unspecified as i32,
            "",
        );
        let j = IngestStatusEventJson::from(&ev);
        assert!(j.stage.is_none());
    }

    #[test]
    fn json_ev_serialises_to_valid_json() {
        let ev = make_event(
            IngestStatus::Done as i32,
            IngestStage::ProjectPg as i32,
            "",
        );
        let j = IngestStatusEventJson::from(&ev);
        let s = serde_json::to_string(&j).expect("should serialise");
        let v: serde_json::Value = serde_json::from_str(&s).expect("should parse");
        assert_eq!(v["status"], "done");
        assert_eq!(v["stage"], "project_pg");
        assert_eq!(v["ingest_run_id"], "run-1");
    }

    // ── total stages constant ────────────────────────────────────────────────

    #[test]
    fn total_pipeline_stages_is_9() {
        assert_eq!(TOTAL_PIPELINE_STAGES, 9);
    }

    // ── UUID parsing edge cases ──────────────────────────────────────────────

    #[test]
    fn valid_uuid_parses_ok() {
        let id = Uuid::new_v4().to_string();
        assert!(id.parse::<Uuid>().is_ok());
    }

    #[test]
    fn invalid_uuid_errors() {
        assert!("not-a-uuid".parse::<Uuid>().is_err());
    }

    // ── stage_db_params: all terminal states produce consistent output ────────

    #[test]
    fn all_non_terminal_statuses_produce_no_db_params() {
        for status in [IngestStatus::Pending, IngestStatus::Unspecified] {
            assert!(
                stage_db_params(status, "any error").is_none(),
                "{status:?} should not produce a DB update"
            );
        }
    }

    #[test]
    fn all_terminal_statuses_produce_db_params() {
        for status in [
            IngestStatus::Processing,
            IngestStatus::Done,
            IngestStatus::Failed,
        ] {
            assert!(
                stage_db_params(status, "").is_some(),
                "{status:?} should produce a DB update"
            );
        }
    }

    // ── all 9 stage labels are distinct and non-empty ────────────────────────

    #[test]
    fn all_named_stages_have_distinct_labels() {
        let stages = [
            IngestStage::Clone,
            IngestStage::Expand,
            IngestStage::Parse,
            IngestStage::Typecheck,
            IngestStage::Extract,
            IngestStage::Embed,
            IngestStage::ProjectPg,
            IngestStage::ProjectNeo4j,
            IngestStage::ProjectQdrant,
        ];

        let labels: Vec<&'static str> = stages.iter().filter_map(|s| stage_label(*s)).collect();
        assert_eq!(labels.len(), 9, "all 9 stages should have labels");

        let mut sorted = labels.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), 9, "stage labels must be unique");

        for label in &labels {
            assert!(!label.is_empty());
        }
    }
}
