-- Wave 5 (ADR-007 §4): per-stage progress rows, one row per (ingestion_run, stage).
-- Created atomically with the ingestion_runs row by POST /v1/repos/{id}/ingestions.
-- Written by ingest-status consumer (RUSAA-69); read by GET /v1/ingest/runs/{id}.

CREATE TABLE pipeline_stage_runs (
    id               UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    ingestion_run_id UUID        NOT NULL REFERENCES ingestion_runs(id) ON DELETE CASCADE,
    stage            TEXT        NOT NULL
                                 CHECK (stage IN (
                                   'clone', 'expand', 'parse', 'typecheck',
                                   'extract', 'embed',
                                   'project_pg', 'project_neo4j', 'project_qdrant'
                                 )),
    status           TEXT        NOT NULL DEFAULT 'pending'
                                 CHECK (status IN ('pending', 'running', 'succeeded', 'failed')),
    started_at       TIMESTAMPTZ,
    finished_at      TIMESTAMPTZ,
    error            TEXT,
    created_at       TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (ingestion_run_id, stage)
);

CREATE INDEX idx_pipeline_stage_runs_run
    ON pipeline_stage_runs (ingestion_run_id, status);
