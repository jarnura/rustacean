-- Control schema: Phase 2 — GitHub App integration and repo connection
-- All four tables form a referential-integrity unit and ship together.
-- See ADR-005 for design rationale.

CREATE TABLE github_installations (
    id                     UUID        PRIMARY KEY,
    tenant_id              UUID        NOT NULL REFERENCES tenants(id),
    github_installation_id BIGINT      NOT NULL UNIQUE,
    account_login          TEXT        NOT NULL,
    account_type           TEXT        NOT NULL CHECK (account_type IN ('User','Organization')),
    account_id             BIGINT      NOT NULL,
    created_at             TIMESTAMPTZ NOT NULL DEFAULT now(),
    suspended_at           TIMESTAMPTZ,
    deleted_at             TIMESTAMPTZ
);
CREATE INDEX idx_github_installations_tenant
    ON github_installations (tenant_id) WHERE deleted_at IS NULL;

CREATE TABLE github_install_states (
    token_hash TEXT        PRIMARY KEY,
    tenant_id  UUID        NOT NULL REFERENCES tenants(id),
    user_id    UUID        NOT NULL REFERENCES users(id),
    expires_at TIMESTAMPTZ NOT NULL,
    used_at    TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX idx_github_install_states_active
    ON github_install_states (expires_at) WHERE used_at IS NULL;

CREATE TABLE repos (
    id              UUID        PRIMARY KEY,
    tenant_id       UUID        NOT NULL REFERENCES tenants(id),
    installation_id UUID        NOT NULL REFERENCES github_installations(id),
    github_repo_id  BIGINT      NOT NULL,
    full_name       TEXT        NOT NULL,
    default_branch  TEXT        NOT NULL,
    status          TEXT        NOT NULL DEFAULT 'connected'
                                CHECK (status IN ('connected','ingesting','ready','error')),
    last_error      TEXT,
    connected_by    UUID        NOT NULL REFERENCES users(id),
    connected_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    archived_at     TIMESTAMPTZ,
    UNIQUE (tenant_id, github_repo_id)
);
CREATE INDEX idx_repos_tenant_status
    ON repos (tenant_id, status) WHERE archived_at IS NULL;
CREATE INDEX idx_repos_installation
    ON repos (installation_id) WHERE archived_at IS NULL;

-- Skeleton table — semantics implemented in Wave 5 (REQ-IN-01).
-- Migrated now so the FK from repos exists and the table is testable end-to-end.
CREATE TABLE ingestion_runs (
    id           UUID        PRIMARY KEY,
    tenant_id    UUID        NOT NULL REFERENCES tenants(id),
    repo_id      UUID        NOT NULL REFERENCES repos(id),
    commit_sha   TEXT        NOT NULL,
    status       TEXT        NOT NULL DEFAULT 'queued'
                             CHECK (status IN ('queued','running','succeeded','failed','cancelled')),
    started_at   TIMESTAMPTZ,
    finished_at  TIMESTAMPTZ,
    error        TEXT,
    requested_by UUID        NOT NULL REFERENCES users(id),
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX idx_ingestion_runs_repo_active
    ON ingestion_runs (repo_id, status)
    WHERE status IN ('queued','running');
