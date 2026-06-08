#!/usr/bin/env bash
# Start the full local stack: rootless Postgres (if down), the axum API, and the
# Vite dev server. Ctrl-C stops the API and Vite (Postgres container keeps running).
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
export DATABASE_URL="${DATABASE_URL:-postgresql://wp:wp@127.0.0.1:5433/wp_tt}"

# ensure dev DB is up
"$ROOT/deploy/dev-db.sh" up >/dev/null

# load nvm for node/vite
export NVM_DIR="$HOME/.nvm"; [ -s "$NVM_DIR/nvm.sh" ] && . "$NVM_DIR/nvm.sh"; nvm use default >/dev/null 2>&1 || true

# build + run API
( cd "$ROOT" && cargo build -q -p wp-api )
"$ROOT/target/debug/api" &
API_PID=$!

# run Vite (proxies /api -> :8080)
( cd "$ROOT/web" && npm run dev ) &
VITE_PID=$!

echo "API   : http://127.0.0.1:8080/api/kpis"
echo "WEB   : http://127.0.0.1:5173"
trap 'kill $API_PID $VITE_PID 2>/dev/null' INT TERM
wait
