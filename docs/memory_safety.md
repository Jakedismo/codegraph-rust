## Memory Safety and Leak Detection

This project integrates a feature-gated, runtime memory leak detector with stack traces and Prometheus metrics.

Overview

- Global tracking allocator via `memscope-rs` (feature: `leak-detect`).
- Runtime metrics exposed at `/metrics`:
  - `memscope_active_memory_bytes`
  - `memscope_active_allocations`
  - `memscope_leaked_memory_bytes`
  - `memscope_leaked_allocations`
- On-demand leak report export: `GET /memory/leaks` → JSON file under `target/memory_reports/`.
- Live memory stats: `GET /memory/stats`.
- RAII LeakGuard logs detected leaks on graceful shutdown.

Usage

- Run API with leak detection:
  - `make run-api-leaks`
- View memory stats:
  - `make leak-stats`
- Export leak report JSON:
  - `make leak-report`

Validation Tools

- Miri (UB detector): `make miri` (requires nightly + miri component)
- Address/Leak Sanitizer: `make asan-test` (nightly, platform support required)

Notes

- Leak detection is feature-gated to avoid production overhead. Enable `--features leak-detect` for diagnostics and CI checks.
- Prometheus rules can alert when `memscope_leaked_allocations > 0`. See `prometheus.rules.yml`.

