#!/usr/bin/env bash
# Local dev PostgreSQL as a rootless podman container, owned by the current user.
# Zero impact on system packages or other users' databases. Bound to localhost only.
#
#   ./deploy/dev-db.sh up      # create/start the container
#   ./deploy/dev-db.sh down    # stop & remove (keeps the data volume)
#   ./deploy/dev-db.sh nuke    # stop, remove, and delete the data volume
#   ./deploy/dev-db.sh url     # print the DATABASE_URL
set -euo pipefail

NAME=wp-tt-postgres
VOL=wp-tt-pgdata
IMAGE=docker.io/library/postgres:17
PORT=5433
USER=wp
PASS=wp
DB=wp_tt
URL="postgresql://${USER}:${PASS}@127.0.0.1:${PORT}/${DB}"

case "${1:-up}" in
  up)
    if podman container exists "$NAME"; then
      podman start "$NAME" >/dev/null
      echo "started existing $NAME"
    else
      podman run -d --name "$NAME" \
        -e POSTGRES_USER="$USER" -e POSTGRES_PASSWORD="$PASS" -e POSTGRES_DB="$DB" \
        -p 127.0.0.1:${PORT}:5432 \
        -v "${VOL}:/var/lib/postgresql/data" \
        "$IMAGE" >/dev/null
      echo "created $NAME"
    fi
    echo "waiting for readiness..."
    for _ in $(seq 1 30); do
      if podman exec "$NAME" pg_isready -U "$USER" -d "$DB" >/dev/null 2>&1; then
        echo "ready: $URL"; exit 0
      fi
      sleep 1
    done
    echo "timed out waiting for postgres" >&2; exit 1
    ;;
  down) podman stop "$NAME" >/dev/null 2>&1 || true; podman rm "$NAME" >/dev/null 2>&1 || true; echo "removed $NAME (volume kept)";;
  nuke) podman stop "$NAME" >/dev/null 2>&1 || true; podman rm "$NAME" >/dev/null 2>&1 || true; podman volume rm "$VOL" >/dev/null 2>&1 || true; echo "removed $NAME and volume $VOL";;
  url) echo "$URL";;
  *) echo "usage: $0 {up|down|nuke|url}" >&2; exit 2;;
esac
