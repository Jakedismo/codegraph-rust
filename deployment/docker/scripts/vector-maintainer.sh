#!/usr/bin/env sh
set -eu

API_BASE="${API_BASE:-http://api:8080}"
PERIOD_SECONDS="${PERIOD_SECONDS:-21600}"

echo "[vector-maintainer] API: ${API_BASE} period: ${PERIOD_SECONDS}s"

while true; do
  echo "[vector-maintainer] Triggering vector index rebuild..."
  if wget -qO- --timeout=10 --tries=1 "${API_BASE}/vector/index/rebuild" >/dev/null 2>&1; then
    echo "[vector-maintainer] Rebuild request sent."
  else
    echo "[vector-maintainer] WARN: could not contact API at ${API_BASE}" >&2
  fi
  sleep "${PERIOD_SECONDS}"
done

