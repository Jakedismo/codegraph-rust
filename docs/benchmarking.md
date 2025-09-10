# CodeGraph Benchmarking Suite

This project includes a comprehensive Criterion-based benchmarking suite with baseline comparison and automated report generation.

## Quick Start

- Run all benches and save a baseline (default: `baseline`):
  - `make bench`
  - Environment:
    - `BASELINE_NAME` (default `baseline`) to name the saved baseline
    - `ENABLE_FAISS_BENCH=1` to include FAISS-dependent benches (requires FAISS available)

- Compare current results to a baseline:
  - `make bench-compare` (env: `BASELINE_NAME`, `THRESHOLD` default `0.10`)

- Generate a Markdown report:
  - `make bench-report` (writes to `benchmarks/reports/benchmark_report_latest.md`)

Artifacts are copied under `benchmarks/artifacts/<timestamp>/criterion` and reports under `benchmarks/reports/`.

## Notes

- Criterion baselines are saved via `--save-baseline <name>` and stored in `target/criterion/<bench>/base/<name>/`.
- The comparison script parses `estimates.json` and flags regressions above the threshold (default 10%).
- Vector and API benches may require enabling `codegraph-vector` feature `faiss`. In CI or environments without FAISS, leave `ENABLE_FAISS_BENCH=0` to skip them.

## Adding New Benches

- Prefer per-crate benches under `crates/<crate>/benches/your_bench.rs`.
- Use `criterion_group!` and `criterion_main!` and keep measurement windows modest for CI.
- Use `Throughput` and parameterized inputs for meaningful, comparable metrics.

