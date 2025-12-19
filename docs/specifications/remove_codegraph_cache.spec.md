ABOUTME: Specification for removing the unused `codegraph-cache` crate from the workspace.
ABOUTME: Ensures workspace builds cleanly and no stale references remain.

# Specification: Remove `codegraph-cache`

## Intent

Remove the `crates/codegraph-cache` crate from the repository because it is not used by production code paths and duplicates caching functionality already present elsewhere (e.g. `codegraph-vector`).

## In Scope

- Delete `crates/codegraph-cache/` from the repository.
- Remove all workspace and dependency references to `codegraph-cache`.
- Remove any feature flags whose only purpose was to expose `codegraph-cache` (e.g. `codegraph-vector`’s `cache` feature).

## Out of Scope

- Introducing a replacement caching layer.
- Changing indexing behavior or persistence strategy.
- Refactoring `codegraph-vector`’s existing caches.

## Behavioral Contracts

- `cargo check --workspace` succeeds without requiring the removed crate.
- `cargo test --workspace` succeeds (or, at minimum, crates that depended on the removed crate compile and their tests run).
- Repository-wide references to `codegraph-cache` are eliminated (except for historical git history).

## Acceptance Criteria

1. No `codegraph-cache` workspace member exists in `Cargo.toml`.
2. No `codegraph-cache` entry exists in `[workspace.dependencies]`.
3. `crates/codegraph-vector/Cargo.toml` no longer exposes a `cache` feature that depended on `codegraph-cache`.
4. The `crates/codegraph-cache/` directory is removed from the working tree.
