ABOUTME: Ensures `codegraph-parser` builds when experimental features are enabled
ABOUTME: Prevents regressions in incremental parsing and semantic analysis modules

# Parser experimental feature build integrity

## Intent

`codegraph-parser` has optional experimental modules (`diff`, `semantic`, watcher experiments). These modules must compile cleanly when enabled so downstream crates can safely opt in to `--features experimental` or `--all-features`.

## Scope

- `crates/codegraph-parser` with feature flags enabled:
  - `experimental`
  - `watcher-experimental`

## Behavioral contract

Given a Rust toolchain capable of building this workspace:

1. `cargo check -p codegraph-parser --all-features` succeeds (exit code `0`).
2. The compilation does not fail due to stale API usage across crate boundaries (e.g. outdated `CodeNode`/`Location`/`Span` field names).

## Rationale

Experimental modules are still part of the repository surface area. Keeping them buildable avoids bit rot and reduces surprises when enabling feature flags for development or CI.
