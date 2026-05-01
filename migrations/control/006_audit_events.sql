-- Control schema: Wave 5 — Audit service (REQ-OB-04)
-- Creates the shared audit schema, audit_events table, and the INSERT-only role.
-- See ADR-007 §7.2 for the non-tenant-isolated table rationale.
-- IF NOT EXISTS guards allow parallel migration with 005 (which also creates the audit schema).

CREATE SCHEMA IF NOT EXISTS audit;

CREATE TABLE IF NOT EXISTS audit.audit_events (
    id               UUID        NOT NULL PRIMARY KEY DEFAULT gen_random_uuid(),
    schema_version   TEXT        NOT NULL DEFAULT 'rust_brain.v1',
    event_id         UUID        NOT NULL,
    tenant_id        UUID        NOT NULL,
    ingestion_run_id UUID,
    repo_id          UUID,
    stage            TEXT,
    stage_seq        INT,
    actor_kind       TEXT        NOT NULL DEFAULT 'system',
    actor_user_id    UUID,
    action           TEXT        NOT NULL,
    outcome          TEXT        NOT NULL,
    occurred_at      TIMESTAMPTZ NOT NULL,
    payload          JSONB       NOT NULL DEFAULT '{}',
    recorded_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Per-tenant lookup (most common query pattern).
CREATE INDEX IF NOT EXISTS audit_events_tenant_occurred_idx
    ON audit.audit_events (tenant_id, occurred_at DESC);

-- Action-type filter for compliance queries.
CREATE INDEX IF NOT EXISTS audit_events_action_idx
    ON audit.audit_events (action);

-- Idempotency guard: unique event_id per tenant to prevent double-write.
CREATE UNIQUE INDEX IF NOT EXISTS audit_events_tenant_event_id_uidx
    ON audit.audit_events (tenant_id, event_id);

-- rb_audit_writer role: INSERT + SELECT only.
-- No UPDATE or DELETE is granted; the audit log is immutable by design.
-- CI verifies UPDATE/DELETE return "permission denied" for this role (ADR-007 §7.2).
DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'rb_audit_writer') THEN
        CREATE ROLE rb_audit_writer NOLOGIN;
    END IF;
END
$$;

GRANT USAGE ON SCHEMA audit TO rb_audit_writer;
GRANT INSERT, SELECT ON audit.audit_events TO rb_audit_writer;
-- Explicitly deny mutation on the table for all non-owner roles.
REVOKE UPDATE, DELETE ON audit.audit_events FROM PUBLIC;
