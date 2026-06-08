#!/usr/bin/env bash
# Apply all migrations (in order) then the seed, against $DATABASE_URL.
# Idempotent-ish: migrations are plain CREATEs (run once on a fresh DB); the seed
# uses UPSERT so it can be re-run. For a clean re-apply, drop & recreate the DB.
set -euo pipefail

DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
: "${DATABASE_URL:?set DATABASE_URL, e.g. postgresql:///wp_tt}"

echo "Applying migrations to $DATABASE_URL"
for f in "$DIR"/migrations/[0-9]*.sql; do
  echo "  -> $(basename "$f")"
  psql "$DATABASE_URL" -v ON_ERROR_STOP=1 -q -f "$f"
done

echo "Applying seed"
psql "$DATABASE_URL" -v ON_ERROR_STOP=1 -q -f "$DIR/seed/kpi_target.sql"

echo "Done. Tables:"
psql "$DATABASE_URL" -c "\dt"
