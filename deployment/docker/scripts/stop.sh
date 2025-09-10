#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")"/../.. && pwd)"
COMPOSE_FILE="$ROOT_DIR/deployment/docker/docker-compose.yml"

PRUNE_VOLUMES=${PRUNE_VOLUMES:-0}

echo "[codegraph] Stopping containers..."
if [[ "$PRUNE_VOLUMES" == "1" ]]; then
  docker compose -f "$COMPOSE_FILE" down -v --remove-orphans
else
  docker compose -f "$COMPOSE_FILE" down --remove-orphans
fi

echo "[codegraph] Stopped."

