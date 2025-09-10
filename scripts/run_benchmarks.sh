#!/usr/bin/env bash
set -euo pipefail

# Configuration
BASELINE_NAME=${BASELINE_NAME:-baseline}
ENABLE_FAISS_BENCH=${ENABLE_FAISS_BENCH:-0}
TIMESTAMP=$(date +%Y%m%d_%H%M%S)

echo "[bench] Compiling benches (no run)"
cargo bench --no-run --workspace

echo "[bench] Running default workspace benches, saving baseline: ${BASELINE_NAME}"
cargo bench --workspace -- --save-baseline "${BASELINE_NAME}"

if [[ "${ENABLE_FAISS_BENCH}" == "1" ]]; then
  echo "[bench] Running FAISS-dependent benches (vector, api)"
  # Vector crate benches with faiss
  cargo bench -p codegraph-vector -F codegraph-vector/faiss -- --save-baseline "${BASELINE_NAME}"
  # API crate benches wired to benchmarks.rs, also requires faiss feature on vector
  cargo bench -p codegraph-api -F codegraph-vector/faiss --bench api_benchmarks -- --save-baseline "${BASELINE_NAME}"
else
  echo "[bench] Skipping FAISS-dependent benches (set ENABLE_FAISS_BENCH=1 to enable)"
fi

mkdir -p benchmarks/artifacts/${TIMESTAMP}
cp -R target/criterion benchmarks/artifacts/${TIMESTAMP}/criterion || true

echo "[bench] Comparing against baseline and generating reports"
python3 scripts/compare_bench.py --baseline "${BASELINE_NAME}" --threshold 0.10 || true
python3 scripts/generate_bench_report.py --output "benchmarks/reports/benchmark_report_${TIMESTAMP}.md" || true

echo "[bench] Done. Artifacts under benchmarks/artifacts/${TIMESTAMP} and reports under benchmarks/reports/."

