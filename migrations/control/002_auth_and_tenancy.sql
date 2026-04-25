-- Control schema: Phase 1 — Auth and Tenancy tables
-- Creates all control-plane tables for auth, sessions, tenant management, and API keys.
-- citext provides case-insensitive text equality for emails and tenant slugs.
CREATE EXTENSION IF NOT EXISTS citext;

CREATE TABLE tenants (
    id             UUID        PRIMARY KEY,
    slug           CITEXT      NOT NULL UNIQUE,
    name           TEXT        NOT NULL,
    schema_name    TEXT        NOT NULL UNIQUE,
    schema_version INT         NOT NULL DEFAULT 0,
    status         TEXT        NOT NULL DEFAULT 'active'
                               CHECK (status IN ('active', 'suspended', 'deleting', 'deleted')),
    created_at     TIMESTAMPTZ NOT NULL DEFAULT now(),
    deleted_at     TIMESTAMPTZ
);
CREATE INDEX idx_tenants_status ON tenants (status, deleted_at);

CREATE TABLE users (
    id                UUID        PRIMARY KEY,
    email             CITEXT      NOT NULL UNIQUE,
    password_hash     TEXT        NOT NULL,
    email_verified_at TIMESTAMPTZ,
    mfa_secret        BYTEA,
    status            TEXT        NOT NULL DEFAULT 'active'
                                  CHECK (status IN ('active', 'suspended')),
    created_at        TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE tenant_members (
    tenant_id  UUID        NOT NULL REFERENCES tenants(id),
    user_id    UUID        NOT NULL REFERENCES users(id),
    role       TEXT        NOT NULL CHECK (role IN ('owner', 'admin', 'member')),
    invited_by UUID                 REFERENCES users(id),
    invited_at TIMESTAMPTZ,
    joined_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (tenant_id, user_id)
);

CREATE TABLE sessions (
    id           UUID        PRIMARY KEY,
    user_id      UUID        NOT NULL REFERENCES users(id),
    tenant_id    UUID        NOT NULL REFERENCES tenants(id),
    token_hash   TEXT        NOT NULL UNIQUE,
    ip           INET,
    user_agent   TEXT,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    last_seen_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    expires_at   TIMESTAMPTZ NOT NULL,
    revoked_at   TIMESTAMPTZ
);
CREATE INDEX idx_sessions_user_active
    ON sessions (user_id, expires_at) WHERE revoked_at IS NULL;

CREATE TABLE email_tokens (
    token_hash TEXT        PRIMARY KEY,
    user_id    UUID        NOT NULL REFERENCES users(id),
    kind       TEXT        NOT NULL
                           CHECK (kind IN ('verify', 'reset', 'invite', 'bootstrap')),
    expires_at TIMESTAMPTZ NOT NULL,
    used_at    TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE auth_events (
    id         BIGINT      GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    user_id    UUID                 REFERENCES users(id),
    tenant_id  UUID                 REFERENCES tenants(id),
    event      TEXT        NOT NULL,
    ip         INET,
    user_agent TEXT,
    metadata   JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX idx_auth_events_user   ON auth_events (user_id,   created_at DESC);
CREATE INDEX idx_auth_events_tenant ON auth_events (tenant_id, created_at DESC);

CREATE TABLE api_keys (
    id                 UUID        PRIMARY KEY,
    tenant_id          UUID        NOT NULL REFERENCES tenants(id),
    key_hash           TEXT        NOT NULL UNIQUE,
    name               TEXT        NOT NULL,
    scopes             JSONB       NOT NULL,
    created_by_user_id UUID        NOT NULL REFERENCES users(id),
    last_used_at       TIMESTAMPTZ,
    revoked_at         TIMESTAMPTZ,
    created_at         TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX idx_api_keys_active ON api_keys (key_hash) WHERE revoked_at IS NULL;
