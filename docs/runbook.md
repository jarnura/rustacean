# Runbook

Operational reference for the rust-brain dev stack. This document covers start/stop procedures, log access, health verification, common failure modes, and database migrations.

---

## Docker Compose stacks

There are two compose files and an optional env file:

| File | Purpose |
|------|---------|
| `compose/dev.yml` | Full dev stack — all services with default local ports |
| `compose/tailscale.yml` | Overlay — adds `restart: unless-stopped` to all services for remote deployment |
| `compose/tailscale.env` | Port remapping for the mars host (Tailscale IP: `100.87.157.74`) |

### Local development

```bash
# Start everything
docker compose -f compose/dev.yml up -d

# Start only infrastructure (skip control-api if running it with cargo)
docker compose -f compose/dev.yml up -d postgres kafka neo4j qdrant \
  otel-collector tempo prometheus grafana ollama

# Stop everything (preserves volumes)
docker compose -f compose/dev.yml down

# Stop and delete all volumes (full reset)
docker compose -f compose/dev.yml down -v
```

### Remote deployment (mars / Tailscale)

```bash
# Start (or restart after code changes)
docker compose --env-file compose/tailscale.env \
  -f compose/dev.yml -f compose/tailscale.yml up -d

# Restart a single service after updating its image
docker compose --env-file compose/tailscale.env \
  -f compose/dev.yml -f compose/tailscale.yml up -d --no-deps control-api

# Pull latest images and restart
docker compose --env-file compose/tailscale.env \
  -f compose/dev.yml -f compose/tailscale.yml pull
docker compose --env-file compose/tailscale.env \
  -f compose/dev.yml -f compose/tailscale.yml up -d
```

---

## Port reference

### Local (default ports)

| Service | Port | Protocol | URL |
|---------|------|----------|-----|
| control-api | 8080 | HTTP | http://localhost:8080 |
| postgres | 5432 | TCP | — |
| kafka | 9094 | TCP (external) | localhost:9094 |
| grafana | 3000 | HTTP | http://localhost:3000 |
| prometheus | 9090 | HTTP | http://localhost:9090 |
| pgweb | 8081 | HTTP | http://localhost:8081 |
| kafka-ui | 8082 | HTTP | http://localhost:8082 |
| caddy | 80/443 | HTTP/S | http://localhost |
| ollama | 11434 | HTTP | http://localhost:11434 |

### Remote — mars (tailscale.env remapping)

| Service | Port | URL |
|---------|------|-----|
| control-api | 18080 | http://100.87.157.74:18080 |
| postgres | 15432 | — |
| kafka | 19094 | 100.87.157.74:19094 |
| grafana | 13000 | http://100.87.157.74:13000 |
| prometheus | 19090 | http://100.87.157.74:19090 |
| pgweb | 18081 | http://100.87.157.74:18081 |
| kafka-ui | 18082 | http://100.87.157.74:18082 |
| ollama | 21434 | http://100.87.157.74:21434 |

Full authoritative table: [PORT_MAP.md](PORT_MAP.md).

---

## Health checks

### control-api

```bash
# Liveness — always 200 while the process is running
curl -s http://localhost:8080/health | jq .
# → {"status":"ok"}

# Readiness — 200 when ready to serve traffic (DB connected)
curl -s http://localhost:8080/ready | jq .
# → {"status":"ok"}
```

### PostgreSQL

```bash
docker compose -f compose/dev.yml exec postgres pg_isready -U rustbrain
# → /var/run/postgresql:5432 - accepting connections
```

### Kafka

```bash
docker compose -f compose/dev.yml exec kafka \
  kafka-topics.sh --bootstrap-server localhost:9092 --list
# prints topic names like rb.ingest.clone.commands, rb.projector.events, ...
```

### All service health at once

```bash
docker compose -f compose/dev.yml ps
# STATUS column shows: healthy / running / starting / exited
```

---

## Logs

```bash
# Stream all logs
docker compose -f compose/dev.yml logs -f

# Stream a specific service
docker compose -f compose/dev.yml logs -f control-api

# Last 100 lines from control-api
docker compose -f compose/dev.yml logs --tail=100 control-api

# Filter for errors
docker compose -f compose/dev.yml logs control-api 2>&1 | grep -i error

# Find verification email token in dev (email_transport=console)
docker compose -f compose/dev.yml logs control-api 2>&1 | grep "verify-email"
```

The control-api emits structured JSON logs. To pretty-print them:

```bash
docker compose -f compose/dev.yml logs -f control-api | jq .
```

---

## Database migrations

Migrations are managed by the `migrate` binary in `services/migrate/`. There is no automatic migration on API startup — run migrations explicitly.

### Running migrations

```bash
# Against local Docker postgres
RB_DATABASE_URL=postgres://rustbrain:rustbrain@localhost:5432/rustbrain \
  cargo run -p migrate -- up

# Against remote (mars) postgres through Tailscale.
# Replace <user>:<password> with the credentials from compose/tailscale.env
# (defaults match the local Docker compose values).
RB_DATABASE_URL=postgres://<user>:<password>@100.87.157.74:15432/rustbrain \
  cargo run -p migrate -- up
```

### Checking migration status

```bash
RB_DATABASE_URL=postgres://rustbrain:rustbrain@localhost:5432/rustbrain \
  cargo run -p migrate -- status
```

### Kafka topic creation

The `kafka-init` container in `dev.yml` creates all required topics on first boot. To re-create topics manually (e.g. after a full `down -v`):

```bash
docker compose -f compose/dev.yml up kafka-init
```

Required topics: `rb.ingest.clone.commands`, `rb.ingest.expand.commands`, `rb.ingest.parse.commands`, `rb.ingest.typecheck.commands`, `rb.ingest.graph.commands`, `rb.ingest.embed.commands`, `rb.projector.events`, `rb.audit.events`.

---

## Browsing the database

**pgweb** is a read-only web-based database browser included in the compose stack.

- Local: http://localhost:8081
- Remote: http://100.87.157.74:18081

Useful queries:

```sql
-- List all tenants
SELECT id, slug, name, status, created_at FROM control.tenants;

-- List all users with verification status
SELECT id, email, email_verified_at IS NOT NULL AS verified, status, created_at
FROM control.users;

-- Active sessions
SELECT id, user_id, tenant_id, expires_at FROM control.sessions
WHERE revoked_at IS NULL AND expires_at > now();

-- API keys for a tenant
SELECT id, name, scopes, last_used_at, revoked_at FROM control.api_keys
WHERE tenant_id = '<uuid>';

-- Auth event log (last 50)
SELECT event, user_id, tenant_id, occurred_at FROM control.auth_events
ORDER BY occurred_at DESC LIMIT 50;
```

---

## Building and deploying the control-api image

The API has a multi-stage Dockerfile at `docker/control-api/Dockerfile`.

```bash
# Build the image
docker build -f docker/control-api/Dockerfile -t rustbrain/control-api:dev .

# Force rebuild in compose
docker compose -f compose/dev.yml build control-api
docker compose -f compose/dev.yml up -d control-api
```

---

## Common failure modes

### control-api exits immediately on start

**Symptom**: `docker compose ps` shows `control-api` as `exited (1)`.

**Diagnosis**:
```bash
docker compose -f compose/dev.yml logs control-api
```

Most common cause: `RB_DATABASE_URL` is wrong or postgres is not yet healthy.

**Fix**: Wait for postgres to be healthy, then restart:
```bash
docker compose -f compose/dev.yml up -d control-api
```

---

### `cargo run -p migrate` fails with "connection refused"

**Cause**: postgres container is not running or the port mapping differs.

**Fix**:
```bash
docker compose -f compose/dev.yml up -d postgres
# Wait for healthy, then re-run migrate
```

---

### Kafka topics already exist error from kafka-init

**Cause**: kafka-init uses `--if-not-exists`; this error should not occur unless the container is run in an unexpected mode.

**Fix**: Check kafka logs, then run `kafka-init` again:
```bash
docker compose -f compose/dev.yml up kafka-init
```

---

### Login returns 429 (rate limited)

**Cause**: ≥ 5 failed login attempts for the same email within 10 minutes.

**Fix**: Wait 15 minutes, or restart the control-api to clear the in-memory rate limiter:
```bash
docker compose -f compose/dev.yml restart control-api
```

---

### Email verification link expired

**Cause**: Email tokens expire after 1 hour. Password reset tokens expire after 15 minutes.

**Fix**: Repeat the signup or forgot-password flow to get a fresh token. In dev, the link appears in the API logs:
```bash
docker compose -f compose/dev.yml logs control-api | grep -E "verify-email|reset-password"
```

---

### OpenAPI schema drift (CI failure)

**Symptom**: CI job `openapi-sync` fails.

**Cause**: A handler was changed without regenerating `openapi.json`.

**Fix**:
```bash
cargo run -p control-api -- print-openapi > openapi.json
git add openapi.json
git commit -m "docs: regenerate openapi.json"
```

---

## Observability

### Grafana dashboards

- Local: http://localhost:3000 (no login required — anonymous access enabled)
- Remote: http://100.87.157.74:13000

Pre-provisioned data sources: Prometheus, Tempo.

### Prometheus metrics

- Local: http://localhost:9090
- Remote: http://100.87.157.74:19090

### Distributed traces (Tempo)

Traces are emitted by the control-api via OTLP gRPC to `otel-collector:4317`, which forwards them to Tempo. View traces in Grafana → Explore → Tempo data source.

### Kafka monitoring

- Local: http://localhost:8082
- Remote: http://100.87.157.74:18082
