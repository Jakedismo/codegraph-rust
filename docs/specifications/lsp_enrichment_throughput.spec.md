ABOUTME: Specifies performance and behavioral contracts for the LSP enrichment phase.
ABOUTME: Focuses on throughput and bounded work per file to keep indexing responsive.

# Specification: LSP Enrichment Throughput

## Intent

The LSP enrichment phase should provide symbol/name enrichment and best-effort definition resolution without making indexing feel “stuck”.

In particular, LSP enrichment must avoid work that scales with `O(files * total_edges)` when it can be organized as `O(files + total_edges)` by pre-grouping edges per file.

## Context Boundary

Inputs:
- `project_root`: filesystem root for URI generation
- `files`: paths passed to the LSP analyzer for a language
- `nodes`: extracted nodes to enrich (qualified names)
- `edges`: extracted edges to optionally retarget via definition resolution

Outputs:
- Mutated `nodes` metadata (qualified names + provenance)
- Mutated `edges` `to` + metadata (when a definition location resolves to a known node)
- `LspEnrichmentStats`

## Contracts

1. **Bounded per-file edge processing**
   - Only edges whose `from` node belongs to the currently processed file are considered for definition resolution.
   - The implementation MUST NOT scan all edges for every file when sufficient information exists to pre-group edges by file.

2. **Deterministic position mapping**
   - Mapping from byte offsets to LSP UTF-16 positions must be correct for Unicode input.
   - The mapping should avoid repeatedly scanning the entire file for each lookup.

3. **Observability**
   - Progress logging must continue to report processed files and enrichment counts at least every 10 seconds.

## Acceptance Criteria

1. Unit tests cover:
   - UTF-16 position mapping remains correct for representative Unicode cases.
   - Per-file edge grouping selects only edges for the matching file key(s).
2. `cargo test -p codegraph-mcp -q` passes with pristine output.

