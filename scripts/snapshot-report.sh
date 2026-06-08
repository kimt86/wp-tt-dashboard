#!/usr/bin/env bash
# Generate a static, self-contained HTML KPI snapshot from the real L0 data in
# PostgreSQL. Opens in any browser — no server, no Node, no build.
#   DATABASE_URL=... ./scripts/snapshot-report.sh 2026-06-04
set -euo pipefail

DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DATE="${1:-$(date -d yesterday +%Y-%m-%d)}"
: "${DATABASE_URL:?set DATABASE_URL}"
OUT="$DIR/../docs/snapshot-${DATE}.html"

JSON="$(psql "$DATABASE_URL" -tA -v d="$DATE" -f "$DIR/snapshot.sql")"
if [ -z "$JSON" ]; then echo "no data for $DATE" >&2; exit 1; fi

# inject JSON into the template's __DATA__ marker (awk avoids sed escaping issues)
awk -v data="$JSON" '{ if ($0 ~ /__DATA__/) { sub(/__DATA__/, data); print } else print }' \
  "$DIR/snapshot-template.html" > "$OUT"

echo "wrote $OUT"
