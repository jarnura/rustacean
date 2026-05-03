#!/bin/sh
# Applies tenant-schema migrations idempotently to all existing tenant schemas.
# Run by the rb-migrations init container after migrate-control.sh completes.
# Discovers schemas matching tenant_[0-9a-f]{24} and applies /migrations/tenant/*.sql.
set -e

echo "rb-migrations: scanning for tenant schemas to migrate..."

schemas=$(psql "$DATABASE_URL" -t -A \
  -c "SELECT nspname FROM pg_catalog.pg_namespace \
      WHERE nspname ~ '^tenant_[0-9a-f]{24}$' ORDER BY nspname;")

if [ -z "$schemas" ]; then
    echo "rb-migrations: no tenant schemas found, skipping tenant migrations."
    exit 0
fi

for schema in $schemas; do
    echo "rb-migrations: migrating schema $schema ..."

    psql "$DATABASE_URL" -v ON_ERROR_STOP=1 <<SQL
CREATE SCHEMA IF NOT EXISTS "${schema}";
CREATE TABLE IF NOT EXISTS "${schema}".schema_migrations (
    version     INTEGER     PRIMARY KEY,
    description TEXT        NOT NULL,
    checksum    TEXT        NOT NULL,
    applied_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);
SQL

    for f in $(ls /migrations/tenant/*.sql 2>/dev/null | sort); do
        ver=$(basename "$f" | sed 's/^\([0-9]*\)_.*/\1/' | sed 's/^0*//')
        : "${ver:=0}"

        applied=$(psql "$DATABASE_URL" -t -A \
          -c "SELECT count(*) FROM \"${schema}\".schema_migrations WHERE version = ${ver};")

        if [ "$applied" = "1" ]; then
            printf "  %s v%03d already applied, skipping\n" "$schema" "$ver"
            continue
        fi

        printf "  %s applying v%03d: %s\n" "$schema" "$ver" "$(basename "$f")"
        cksum=$(sha256sum "$f" | awk '{print $1}')
        desc=$(basename "$f" | sed 's/^[0-9]*_//' | sed 's/\.sql$//' | tr '_' ' ')

        PGOPTIONS="-c search_path=${schema},public" \
          psql "$DATABASE_URL" -v ON_ERROR_STOP=1 -f "$f"

        psql "$DATABASE_URL" \
          -c "INSERT INTO \"${schema}\".schema_migrations (version, description, checksum)
              VALUES (${ver}, \$\$${desc}\$\$, \$\$${cksum}\$\$)
              ON CONFLICT (version) DO NOTHING;"
    done
done

echo "rb-migrations: all tenant migrations applied."
