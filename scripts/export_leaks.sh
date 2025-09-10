#!/usr/bin/env bash
set -euo pipefail

HOST="${HOST:-localhost}"
PORT="${PORT:-3000}"

echo "Exporting leak report from http://${HOST}:${PORT}/memory/leaks"
curl -sSf "http://${HOST}:${PORT}/memory/leaks" | jq .

