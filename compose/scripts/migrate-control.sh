#!/bin/sh
# Applies control-plane migrations idempotently via psql.
# Run by the rb-migrations init container in compose/dev.yml before control-api starts.
# Mirrors the logic of services/migrate (Runner::bootstrap + Runner::apply_all).
set -e

echo "rb-migrations: waiting for postgres..."
until psql "$DATABASE_URL" -c '\q' 2>/dev/null; do
  sleep 1
done

echo "rb-migrations: bootstrapping control schema..."
psql "$DATABASE_URL" -v ON_ERROR_STOP=1 <<'SQL'
CREATE SCHEMA IF NOT EXISTS control;
CREATE TABLE IF NOT EXISTS control.schema_migrations (
    version     INTEGER     PRIMARY KEY,
    description TEXT        NOT NULL,
    checksum    TEXT        NOT NULL,
    applied_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);
SQL

for f in $(ls /migrations/control/*.sql 2>/dev/null | sort); do
    # Strip leading zeros so the value works as a SQL integer literal.
    ver=$(basename "$f" | sed 's/^\([0-9]*\)_.*/\1/' | sed 's/^0*//')
    : "${ver:=0}"

    applied=$(psql "$DATABASE_URL" -t -A \
      -c "SELECT count(*) FROM control.schema_migrations WHERE version = ${ver};")

    if [ "$applied" = "1" ]; then
        printf "  v%03d already applied, skipping\n" "$ver"
        continue
    fi

    printf "  applying v%03d: %s\n" "$ver" "$(basename "$f")"
    cksum=$(sha256sum "$f" | awk '{print $1}')
    desc=$(basename "$f" | sed 's/^[0-9]*_//' | sed 's/\.sql$//' | tr '_' ' ')

    # Apply with search_path=control,public so SQL in the file lands in the
    # right schema without needing explicit schema prefixes.
    PGOPTIONS='-c search_path=control,public' \
      psql "$DATABASE_URL" -v ON_ERROR_STOP=1 -f "$f"

    # Record using dollar-quoting to avoid single-quote escaping issues.
    psql "$DATABASE_URL" \
      -c "INSERT INTO control.schema_migrations (version, description, checksum)
          VALUES (${ver}, \$\$${desc}\$\$, \$\$${cksum}\$\$)
          ON CONFLICT (version) DO NOTHING;"
done

echo "rb-migrations: all control migrations applied."
