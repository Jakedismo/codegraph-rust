#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")"/../.. && pwd)"
COMPOSE_FILE="$ROOT_DIR/deployment/docker/docker-compose.yml"

PROFILE_MONITORING=${PROFILE_MONITORING:-1}

echo "[codegraph] Starting containers..."
if [[ "${PROFILE_MONITORING}" == "1" ]]; then
  docker compose -f "$COMPOSE_FILE" up -d --build
else
  docker compose -f "$COMPOSE_FILE" up -d --build api vector-maintainer graph-backup
fi

echo "[codegraph] Waiting for API health (30s timeout)..."
set +e
for i in {1..30}; do
  if curl -fsS http://127.0.0.1:8080/health/ready >/dev/null 2>&1; then
    echo "[codegraph] API healthy."
    exit 0
  fi
  sleep 1
done
set -e
echo "[codegraph] API did not become healthy within 30s." >&2
exit 1

