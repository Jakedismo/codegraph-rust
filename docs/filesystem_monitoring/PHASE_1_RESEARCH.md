## Library Research Summary

Targets: `notify` (cross-platform FS events), `globset` (include globs), `ignore` (.gitignore semantics).

- notify (6.x):
  - Provides `RecommendedWatcher` with OS backends (inotify/FSEvents/ReadDirectoryChangesW).
  - Event model: `Event { kind: EventKind, paths: Vec<PathBuf>, ... }`.
  - Rename events surface via `EventKind::Modify(ModifyKind::Name(RenameMode::{From,To,Both}))` and often include two paths.
  - Best practice: do light work in the callback and hand off to a channel; use debounce/coalescing to avoid floods.

- globset:
  - Compile many glob patterns into a `GlobSet` for efficient matching.
  - Useful for fast include filtering like `**/*.rs`.

- ignore:
  - `GitignoreBuilder` compiles .gitignore-compatible matchers anchored to a base directory.
  - `Gitignore::matched_path_or_any_parents(path, is_dir)` to honor root patterns and parent directories.

Patterns and Anti-patterns:
- Do not hash file contents on the watcher callback thread; offload work.
- Coalesce multiple modify events into one (first old, last new).
- Handle rename pairs atomically; treat delete+create as modify when appropriate.

Performance Notes:
- Keep debounce small (20–40ms) to hit <100ms latency while still batching.
- Use non-blocking channels; avoid long critical sections.

