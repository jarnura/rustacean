//! POST /v1/repos/{repo_id}/ingestions — Manual ingestion trigger (REQ-IN-01).
//!
//! Inserts an `ingestion_runs` row and 9 `pipeline_stage_runs` rows in a single
//! transaction, then emits an `IngestRequest` to `rb.ingest.clone.commands`.
//! Returns 202 Accepted with `{ ingest_run_id }` within 200ms.
//! Returns 409 if a run is already queued or running for this repo.

use axum::{Json, extract::State, http::StatusCode, response::IntoResponse};
use rb_kafka::EventEnvelope;
use rb_schemas::{IngestRequest, TenantId};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{
    error::AppError,
    middleware::auth::{AuthContext, require_verified_session},
    state::AppState,
};

const CLONE_COMMANDS_TOPIC: &str = "rb.ingest.clone.commands";

/// Stage names that match `pipeline_stage_runs.stage` CHECK constraint and
/// the `IngestStage` proto enum (ADR-007 §3.1).
const PIPELINE_STAGES: &[&str] = &[
    "clone",
    "expand",
    "parse",
    "typecheck",
    "extract",
    "embed",
    "project_pg",
    "project_neo4j",
    "project_qdrant",
];

#[derive(Debug, Deserialize, ToSchema)]
pub struct TriggerIngestionRequest {
    /// Target commit SHA. Optional; if omitted the clone stage resolves the
    /// branch HEAD.
    pub commit_sha: Option<String>,
    /// Target branch name. Optional; clone stage falls back to the repo's
    /// `default_branch` when both `commit_sha` and `branch` are absent.
    pub branch: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct TriggerIngestionResponse {
    pub ingest_run_id: Uuid,
}

/// Trigger a manual ingestion run for a connected repository.
///
/// Creates an `ingestion_runs` row (status `queued`) and one
/// `pipeline_stage_runs` row per stage (status `pending`), then publishes an
/// `IngestRequest` to `rb.ingest.clone.commands`.
///
/// Returns 409 if an ingestion is already queued or running for this repo.
/// Returns 503 if the Kafka producer is unavailable.
#[utoipa::path(
    post,
    path = "/v1/repos/{repo_id}/ingestions",
    params(
        ("repo_id" = Uuid, Path, description = "Repository UUID")
    ),
    request_body = TriggerIngestionRequest,
    responses(
        (status = 202, description = "Ingestion run queued", body = TriggerIngestionResponse),
        (status = 401, description = "Not authenticated or session expired"),
        (status = 403, description = "Email not verified"),
        (status = 404, description = "Repository not found or belongs to another tenant"),
        (status = 409, description = "Ingestion already in-flight (ingest_run_already_in_flight)"),
        (status = 503, description = "Kafka producer not available (kafka_not_configured)"),
    ),
    tag = "ingestions"
)]
pub async fn trigger_ingestion(
    State(state): State<AppState>,
    auth: AuthContext,
    axum::extract::Path(repo_id): axum::extract::Path<Uuid>,
    Json(body): Json<TriggerIngestionRequest>,
) -> Result<impl IntoResponse, AppError> {
    let session = require_verified_session(auth)?;

    let producer = state
        .ingest_producer
        .as_ref()
        .ok_or(AppError::KafkaNotConfigured)?;

    // 1. Verify the repo exists and belongs to this tenant.
    let exists: Option<(Uuid,)> = sqlx::query_as(
        "SELECT id FROM control.repos \
         WHERE id = $1 AND tenant_id = $2 AND archived_at IS NULL",
    )
    .bind(repo_id)
    .bind(session.tenant_id)
    .fetch_optional(&state.pool)
    .await?;
    exists.ok_or(AppError::NotFound)?;

    // 2. Reject if a run is already queued or running for this repo.
    let in_flight: Option<(Uuid,)> = sqlx::query_as(
        "SELECT id FROM control.ingestion_runs \
         WHERE repo_id = $1 AND tenant_id = $2 AND status IN ('queued', 'running') LIMIT 1",
    )
    .bind(repo_id)
    .bind(session.tenant_id)
    .fetch_optional(&state.pool)
    .await?;
    if in_flight.is_some() {
        return Err(AppError::IngestRunAlreadyInFlight);
    }

    let run_id = Uuid::new_v4();
    let event_id = Uuid::new_v4();

    // 3. Insert ingestion_run + pipeline_stage_runs in a single transaction so
    //    we never have a partial set of rows visible to the status fan-out.
    let mut txn = state.pool.begin().await?;

    sqlx::query(
        "INSERT INTO control.ingestion_runs \
         (id, tenant_id, repo_id, status, requested_by) \
         VALUES ($1, $2, $3, 'queued', $4)",
    )
    .bind(run_id)
    .bind(session.tenant_id)
    .bind(repo_id)
    .bind(session.user_id)
    .execute(&mut *txn)
    .await?;

    for stage in PIPELINE_STAGES {
        sqlx::query(
            "INSERT INTO control.pipeline_stage_runs \
             (id, ingestion_run_id, stage) \
             VALUES ($1, $2, $3)",
        )
        .bind(Uuid::new_v4())
        .bind(run_id)
        .bind(*stage)
        .execute(&mut *txn)
        .await?;
    }

    txn.commit().await?;

    // 4. Publish IngestRequest to rb.ingest.clone.commands.
    //    If publish fails, mark the run failed so the 409 check doesn't block
    //    future attempts.
    let ingest_req = IngestRequest {
        tenant_id: session.tenant_id.to_string(),
        event_id: event_id.to_string(),
        source: "api".to_string(),
        payload: vec![],
        created_at_ms: chrono::Utc::now().timestamp_millis(),
        repo_id: repo_id.to_string(),
        ingest_run_id: run_id.to_string(),
        commit_sha: body.commit_sha.unwrap_or_default(),
        branch: body.branch.unwrap_or_default(),
    };

    let envelope =
        EventEnvelope::new(TenantId::from(session.tenant_id), ingest_req).with_event_id(event_id);

    let partition_key = format!("{}.{}", session.tenant_id, repo_id);

    if let Err(e) = producer
        .publish(CLONE_COMMANDS_TOPIC, partition_key.as_bytes(), envelope)
        .await
    {
        // Rollback: mark run failed so future triggers aren't blocked.
        let _ = sqlx::query(
            "UPDATE control.ingestion_runs SET status = 'failed', \
             error = 'kafka publish failed at trigger' \
             WHERE id = $1",
        )
        .bind(run_id)
        .execute(&state.pool)
        .await;

        return Err(AppError::KafkaPublish(e));
    }

    tracing::info!(
        %run_id,
        %repo_id,
        tenant_id = %session.tenant_id,
        "ingestion run queued and dispatched to clone stage"
    );

    Ok((
        StatusCode::ACCEPTED,
        Json(TriggerIngestionResponse { ingest_run_id: run_id }),
    ))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::middleware::auth::{ApiKeyInfo, Scope, SessionInfo};

    fn verified_session() -> SessionInfo {
        SessionInfo {
            session_id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            tenant_id: Uuid::new_v4(),
            email_verified: true,
        }
    }

    #[test]
    fn pipeline_stages_count() {
        assert_eq!(PIPELINE_STAGES.len(), 9, "nine stages per IngestStage enum");
    }

    #[test]
    fn pipeline_stages_no_duplicates() {
        let mut seen = std::collections::HashSet::new();
        for s in PIPELINE_STAGES {
            assert!(seen.insert(*s), "duplicate stage: {s}");
        }
    }

    #[test]
    fn trigger_ingestion_response_serializes() {
        let run_id = Uuid::new_v4();
        let resp = TriggerIngestionResponse { ingest_run_id: run_id };
        let val = serde_json::to_value(&resp).unwrap();
        assert!(val.get("ingest_run_id").is_some());
    }

    #[test]
    fn anonymous_auth_rejected() {
        let result = require_verified_session(AuthContext::Anonymous);
        assert!(matches!(result, Err(AppError::Unauthorized)));
    }

    #[test]
    fn unverified_email_rejected() {
        let mut info = verified_session();
        info.email_verified = false;
        let result = require_verified_session(AuthContext::Session(info));
        assert!(matches!(result, Err(AppError::EmailNotVerified)));
    }

    #[test]
    fn api_key_auth_rejected() {
        let key = ApiKeyInfo {
            key_id: Uuid::new_v4(),
            tenant_id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            scopes: vec![Scope::Write],
        };
        let result = require_verified_session(AuthContext::ApiKey(key));
        assert!(matches!(result, Err(AppError::Unauthorized)));
    }

    #[test]
    fn kafka_not_configured_returns_503() {
        let err = AppError::KafkaNotConfigured;
        let resp = err.into_response();
        assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
    }

    #[test]
    fn ingest_run_already_in_flight_returns_409() {
        let err = AppError::IngestRunAlreadyInFlight;
        let resp = err.into_response();
        assert_eq!(resp.status(), StatusCode::CONFLICT);
    }
}
