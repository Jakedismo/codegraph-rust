#!/usr/bin/env bash
set -euo pipefail

BASELINE_NAME="${BASELINE_NAME:-baseline}"
THRESHOLD="${THRESHOLD:-0.10}"

echo "Running performance regression vs baseline=${BASELINE_NAME} threshold=${THRESHOLD}"

make bench

python3 scripts/compare_bench.py --baseline "${BASELINE_NAME}" --threshold "${THRESHOLD}"

echo "Performance regression check completed."

