# compose/ — Docker Compose stacks

## Which Postgres does the compose stack use?

**The compose stack runs its own Postgres** (`rustbrain-dev-postgres-1`, image `postgres:16-alpine`).
It does **not** connect to any pre-existing `rustbrain-postgres` container or the host Postgres on port 5432.

| Stack | DB host (inside compose network) | Host port |
|-------|----------------------------------|-----------|
| `dev.yml` (default) | `postgres:5432` | 5432 (or `$POSTGRES_HOST_PORT`) |
| `dev.yml` + `tailscale.env` (mars) | `postgres:5432` | 15432 |

Connection string used by `control-api` inside the compose network:
```
postgres://rustbrain:rustbrain@postgres:5432/rustbrain
```

## How migrations work

A `rb-migrations` init container runs **before** `control-api` starts:

1. Waits for the `postgres` service to be healthy.
2. Creates the `control` schema and a `control.schema_migrations` tracking table if they do not exist.
3. Applies each `.sql` file under `migrations/control/` in version order (idempotent — already-applied versions are skipped).
4. Exits 0 on success. `control-api` starts only after `rb-migrations` exits successfully
   (`depends_on: rb-migrations: condition: service_completed_successfully`).

The script is `compose/scripts/migrate-control.sh`. It mirrors the logic of the
`services/migrate` Rust binary so checksums match and future `migrate` invocations
recognise already-applied versions.

Adding a new control migration: drop a `.sql` file in `migrations/control/` with the next
version prefix (`003_...`). The next `docker compose up` will pick it up automatically.

## Start the stack

```bash
# Local dev (default ports)
docker compose -f compose/dev.yml up -d

# mars / Tailscale (remapped ports, see docs/PORT_MAP.md)
docker compose --env-file compose/tailscale.env \
  -f compose/dev.yml -f compose/tailscale.yml up -d

# Infra only — skip control-api (e.g. running it with `cargo run`)
docker compose -f compose/dev.yml up -d \
  postgres kafka neo4j qdrant otel-collector tempo prometheus grafana ollama
```

## Smoke test after `docker compose up -d control-api`

Run these to confirm the stack is healthy and migrations applied correctly.

```bash
# 1. Health check — expects {"status":"ok"}
PORT=${CONTROL_API_HOST_PORT:-8080}
curl -fsS http://localhost:${PORT}/health | jq .

# 2. Login endpoint — expects 401 (Unauthorized), NOT 500
#    A 401 proves the handler reached the DB and found no matching user.
#    A 500 with "relation does not exist" means migrations did not apply.
curl -s -o /dev/null -w "%{http_code}" \
  -X POST http://localhost:${PORT}/v1/auth/login \
  -H 'Content-Type: application/json' \
  -d '{"email":"smoke@test.local","password":"irrelevant"}'
# → 401

# 3. Check logs for migration errors
docker logs rustbrain-dev-control-api-1 2>&1 | grep -i "relation\|error" | head -20
# → (no output expected)

# 4. Verify migration tracking table
docker compose -f compose/dev.yml exec postgres \
  psql -U rustbrain rustbrain \
  -c "SELECT version, description, applied_at FROM control.schema_migrations ORDER BY version;"
```

## Port reference

See `docs/PORT_MAP.md` for the full port table, including Tailscale remapping for mars.

## Tear down

```bash
# Stop (preserves volumes — migrations stay applied)
docker compose -f compose/dev.yml down

# Full reset (drops volumes — migrations will re-run on next up)
docker compose -f compose/dev.yml down -v
```
