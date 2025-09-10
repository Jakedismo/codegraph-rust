## Development Context

- Feature: Git repository integration (hooks, branch tracking, change detection, conflict resolution, history analysis)
- Technical Stack: Rust (libgit2 via `git2` crate), `notify` for file watching, async via `tokio`, error handling with `thiserror`/`anyhow`, logging with `tracing`
- Constraints: Handle large repositories efficiently; thread-safe; non-blocking in async contexts (wrap CPU/IO-heavy operations via `spawn_blocking` when called from async); cross-platform hook scripts; avoid exposing internal errors via API
- Success Criteria: 
  - Hooks install/uninstall functions
  - Branch listing and upstream tracking
  - Change detection and diffs
  - Merge with strategies (ours/theirs/normal) + conflict metrics
  - History analysis with commit/author/churn insights
  - Unit tests for core flows pass

## Library Insights (Context7)

- git2 (libgit2 bindings):
  - Revwalk: `push_head`, `push(oid)`, `simplify_first_parent`, `hide_*` for filtering
  - Diff/Stats: `diff_tree_to_tree`, `Diff::stats()` for insertions/deletions; use `DiffOptions`
  - Merge: `MergeOptions` + `MergeFileFavor::{OURS,THEIRS}`, patience/minimal diff flags as needed
  - Fast-forward: `merge_analysis` + `set_target` on branch ref, `checkout_head`
- notify:
  - `recommended_watcher`, configure recursive watch; debounce for high-churn repositories
  - Ignore `.git/` path to reduce noise

## Current Best Practices (Web search snapshots)

- Merge conflicts: prefer smaller, short-lived branches; frequent rebases/merges to reduce divergence; use tools to visualize conflicts; apply consistent strategies (ours/theirs) only when appropriate
- Watchers: debounce events, ignore `.git` internals, avoid heavy work on watcher thread; queue work to background
- Large repos: limit revwalk scope (max commits/time window); avoid loading full blobs; rely on `Diff::stats()` when possible

## Implementation Notes

- Crate: `crates/codegraph-git`
- Public API:
  - `GitRepository::open/init`, `install_hooks`, `list_branches`, `status_summary`, `diff_between`, `merge_branches`, `analyze_history`
  - `RepoWatcher::start` with `WatchOptions`
  - Types: `MergeStrategy`, `MergeOutcome`, `HistoryOptions`, `HistoryInsights`

