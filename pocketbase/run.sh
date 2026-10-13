#!/usr/bin/env bash
# Run (or restart) the local PocketBase test instance for Brillante.
#
#   ./pocketbase/run.sh            -> start detached on http://127.0.0.1:8090
#   ./pocketbase/run.sh --logs     -> start and follow logs
#   ./pocketbase/run.sh --stop     -> stop and remove the container
#
# Data persists in ./pb_data (gitignored). Admin API: http://127.0.0.1:8090/_/
# Create the first superuser once with:
#   podman exec brillante-pocketbase pocketbase superuser upsert EMAIL PASS/pb_data
set -euo pipefail

NAME=brillante-pocketbase
IMAGE=brillante-pb:0.40.4
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PORT="${PB_PORT:-8090}"

# Load secrets (AUTH0_*, PB_SUPERUSER_*) from pocketbase/.env regardless of
# the caller's working directory.
if [ -f "$HERE/.env" ]; then
  set -a
  source "$HERE/.env"
  set +a
fi

if [[ "${1:-}" == "--stop" ]]; then
  podman rm -f "$NAME" 2>/dev/null || true
  echo "stopped $NAME"
  exit 0
fi

podman rm -f "$NAME" 2>/dev/null || true
mkdir -p "$HERE/pb_data" "$HERE/pb_hooks" "$HERE/pb_migrations"

# Forward optional OAuth2 broker configuration (see pb_hooks/auth0.pb.js).
PODMAN_ARGS=()
for var in AUTH0_DOMAIN AUTH0_CLIENT_ID AUTH0_CLIENT_SECRET; do
  if [[ -n "${!var:-}" ]]; then
    echo "$var=${!var}"
    PODMAN_ARGS+=( -e "$var=${!var}" )
  fi
done

podman run -d --name "$NAME" \
  -p "127.0.0.1:${PORT}:8090" \
  -v "$HERE:/pb" \
  -e PB_SUPERUSER_EMAIL="${PB_SUPERUSER_EMAIL:-}" \
  -e PB_SUPERUSER_PASSWORD="${PB_SUPERUSER_PASSWORD:-}" \
  "${PODMAN_ARGS[@]}" \
  "$IMAGE" >/dev/null

for _ in $(seq 1 30); do
  if curl -fsS "http://127.0.0.1:${PORT}/api/health" >/dev/null 2>&1; then
    echo "pocketbase up: http://127.0.0.1:${PORT} (admin: http://127.0.0.1:${PORT}/_/)"
    if [[ "${1:-}" == "--logs" ]]; then
      podman logs -f "$NAME"
    fi
    exit 0
  fi
  sleep 0.3
done

echo "error: pocketbase did not become healthy on port ${PORT}" >&2
podman logs "$NAME" 2>&1 | tail -20 >&2
exit 1
