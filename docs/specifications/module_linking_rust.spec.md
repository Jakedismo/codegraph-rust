ABOUTME: Specifies expected behavior for module linking during Rust indexing.
ABOUTME: Ensures module/linking stage produces module, contains, and import edges.

# Specification: Rust Module Linking

## Intent

When indexing Rust projects, the module linking stage must create:

- module nodes that represent a file/module grouping boundary
- containment edges from module nodes to symbols located in the same file
- import edges between module nodes when an import can be resolved (best-effort)

This avoids the current behavior where module linking reports `modules=0 contains=0 imports=0` for Rust.

## Inputs

- `project_root`: filesystem root used for path normalization
- `nodes`: parsed `CodeNode` list (includes `Location.file_path` and `Language::Rust`)
- `edges`: existing `EdgeRelationship` list (may already contain `Imports`)

## Output

Module linker returns `ModuleLinkerStats`:

- `module_nodes_added > 0` when there are Rust nodes from at least one file
- `contains_edges_added > 0` when module nodes exist
- `module_import_edges_added >= 0` (may remain small if resolution is conservative)

## Contracts

- Module keys are stable across runs for the same file paths within a project.
- For Rust files under `.../src/...`, module keys are derived from crate-relative module paths:
  - `src/lib.rs` / `src/main.rs` map to the crate root module key.
  - `src/foo.rs` and `src/foo/mod.rs` map to the same module key.
- `use crate::foo` should resolve to the corresponding module key when that module exists.

## Acceptance Criteria

1. A unit test covering Rust module linking passes.
2. Indexing a Rust project logs non-zero module linking stats (at least modules + contains).
