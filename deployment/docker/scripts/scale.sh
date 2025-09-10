#!/usr/bin/env bash
set -euo pipefail

if [[ $# -lt 1 ]]; then
  echo "Usage: $0 <replicas>" >&2
  exit 2
fi

REPLICAS="$1"
ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")"/../.. && pwd)"
COMPOSE_FILE="$ROOT_DIR/deployment/docker/docker-compose.yml"

echo "[codegraph] Scaling api to ${REPLICAS} replicas..."
docker compose -f "$COMPOSE_FILE" up -d --scale api="$REPLICAS"
docker compose -f "$COMPOSE_FILE" ps

