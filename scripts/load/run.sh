#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

export BASE_URL="${BASE_URL:-http://localhost:3000}"
export API_KEY="${API_KEY:-test-api-key}"

if ! command -v k6 >/dev/null 2>&1; then
  echo "k6 not found. Please install k6: https://k6.io/docs/get-started/installation/"
  exit 1
fi

echo "Running k6 against ${BASE_URL}"
k6 run "${SCRIPT_DIR}/k6_api_e2e.js"

