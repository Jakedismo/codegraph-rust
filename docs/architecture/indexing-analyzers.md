ABOUTME: Describes how `codegraph index` builds the code graph today
ABOUTME: Proposes custom analyzers that extend beyond AST + FastML extraction

# Indexing pipeline and analyzer roadmap

This document has two goals:

1. Describe the current `codegraph index` indexing process as implemented in this repository.
2. Outline higher-leverage custom analyzers that can produce a richer, more accurate graph than the current **Tree-sitter AST extraction + FastML** enhancement approach.

## Current `codegraph index` process (as implemented)

### CLI entrypoint

The `codegraph index` command is implemented in `crates/codegraph-mcp-server/src/bin/codegraph.rs`:

- `Commands::Index { ... }` is parsed by `clap`
- `handle_index(...)` builds an `IndexerConfig` and runs `ProjectIndexer::index_project(...)`

### Project indexing orchestration

`ProjectIndexer::index_project` lives in `crates/codegraph-mcp/src/indexer.rs` and coordinates the full pipeline.

At a high level:

1. **Load file collection config**
   - Builds `FileCollectionConfig` from `IndexerConfig`
   - Collects source files via `codegraph_parser::file_collect::collect_source_files_with_config`

2. **Decide incremental vs clean-slate**
   - If `--force`, calls `clean_project_data(project_id)` in storage (clean slate)
   - If the project is already indexed and file metadata exists:
     - Detects added/modified/deleted/unchanged files
     - Deletes persisted data for deleted/changed files (delete-then-insert semantics)
     - Indexes only the changed files
   - Otherwise indexes all files

3. **Parse + extract nodes and edges (unified pass)**
   - Calls `parse_files_with_unified_extraction(...)` (delegates to a shared helper in `crates/codegraph-mcp/src/estimation.rs`)
   - Per file: `TreeSitterParser::parse_file_with_edges(...)`

4. **Normalize node identifiers**
   - Generates deterministic node IDs (project-scoped)
   - Updates edges so `from` IDs remain consistent after re-identification

5. **Chunking + embeddings (optional; feature-gated)**
   - Builds a chunk plan from nodes + file sources
   - Generates embeddings for chunks and persists them into SurrealDB

6. **Persist nodes**
   - Writes nodes in batches through a SurrealDB writer queue

7. **Resolve and persist edges**
   - Resolves `EdgeRelationship.to` (string symbols) into concrete node IDs using:
     - exact and normalized matching against a symbol index
     - optional semantic matching when `ai-enhanced` is enabled
   - Persists resolved edges (with project_id)

8. **Persist metadata**
   - Writes project metadata summary and per-file metadata for future incremental runs

9. **Watch mode (optional)**
   - If `--watch`, monitors for changes and reindexes

## Current analyzers: AST + FastML

### AST extraction (Tree-sitter)

Per file, `TreeSitterParser::parse_file_with_edges(...)`:

1. Detects language from the path
2. Parses content with Tree-sitter (with tolerant cleanup for error trees)
3. Dispatches to a language extractor in `crates/codegraph-parser/src/languages/*` which returns an `ExtractionResult`:
   - `nodes`: semantic nodes (functions, classes/structs, modules, etc.)
   - `edges`: relationship edges (calls/imports/dependencies, depending on language extractor)

### FastML enhancement (pattern + symbol heuristics)

After the language extractor runs, the parser applies:

- `crate::fast_ml::enhance_extraction(ast_result, content)`

FastML currently combines:

- Pattern-based edge creation (fast textual patterns over file content)
- Local symbol indexing + lightweight resolution heuristics

This improves recall, especially for edges that are hard to extract purely from Tree-sitter patterns (or where the extractor is conservative).

## Analyzer pipeline in this repository (implemented)

In addition to AST + FastML extraction, the indexing pipeline runs analyzer stages that enrich the graph by default:

- **Build context**: Cargo workspace packages, features, and dependency edges (`depends_on`, `enables`)
- **LSP resolution**: enriches qualified names and resolves reference targets via language servers
- **Rustdoc + API surface**: attaches doc comments, marks API visibility, emits `exports`/`reexports`, and links feature-gated items
- **Module linker**: adds module nodes plus module-level import/containment edges
- **Dataflow (Rust-local)**: emits conservative def-use + propagation edges (`defines`, `uses`, `flows_to`, `returns`, `mutates`)
- **Docs/contracts**: creates document/spec nodes and links backticked symbols (`documents`, `specifies`)
- **Architecture/boundaries**: counts package dependency cycles and (optionally) emits `violates_boundary` edges from configured rules

### Project-level resolution (cross-file)

Even after FastML, many extracted edges still have `to: String` rather than a resolved node ID. The indexer builds a project-wide symbol index and attempts to resolve edge targets across the whole project, with optional semantic matching behind the `ai-enhanced` feature flag.

## Why go beyond AST + FastML

Tree-sitter + heuristics is fast and broad, but it has structural blind spots:

- **Missing type information**: overload resolution, trait impl dispatch, generic specialization, dynamic language inference
- **Weak cross-file resolution** without a real module/type system
- **Macro / codegen opacity** (Rust macros, TS decorators, generated code, build scripts)
- **Dataflow is largely absent** (def-use chains, taint/propagation, aliasing)
- **Build-context blindness** (features/targets/conditional compilation) can produce incorrect edges

The highest-leverage analyzers fix accuracy at the source: they produce fewer ambiguous edges and more stable, fully-qualified identifiers.

## Analyzer extension points in the current pipeline

The current implementation has three natural places to plug in richer analyzers:

1. **File-level post-AST enhancement**
   - Right after `languages::extract_for_language(...)` and before returning an `ExtractionResult`
   - Best for: doc extraction, local imports/exports, simple structural facts

2. **Project-level analysis after parse**
   - After all file `ExtractionResult`s have been merged (global visibility)
   - Best for: module graphs, cross-file linking, build-aware symbol tables, deduplication

3. **Post-persistence enrichment**
   - After nodes/edges exist in storage (graph queries become cheap)
   - Best for: centrality metrics, component detection, architectural constraint checks, “impact” precomputations

## Recommended analyzer output contract

To make analyzers composable (and keep later tools honest), treat analyzer outputs as first-class data with:

- **Provenance**: analyzer name/version, run mode (full vs incremental), and inputs (files/build config).
- **Confidence**: per node/edge confidence + method (AST, heuristic, LSP/compiler, semantic similarity).
- **Evidence**: source span references (file + byte/line ranges) and optional stable hashes of evidence snippets.
- **Determinism**: prefer fully-qualified names and stable IDs so incremental runs converge instead of drifting.

## Custom analyzers that can outperform the current approach

The suggestions below focus on analyzers that meaningfully improve graph **precision**, **completeness**, or **actionability** for agentic tooling.

### 1) Type-aware symbol and call resolution (Language Server / compiler-backed)

**What it is**

Use language-native tooling to resolve identifiers and call targets with type information:

- Rust: `rust-analyzer` (or `cargo check` + compiler metadata) for fully-qualified item paths, trait impls, and macro expansion boundaries
- TypeScript/JavaScript: `tsserver` / TypeScript language service for imports, types, and call target resolution
- Python: `pyright` for module resolution and inferred types (as available)
- Java: JDT language server (or build tool integration) for classpath-aware resolution

**What it produces**

- Stable, fully qualified symbol identities (module/crate/package scoped)
- High-confidence edges:
  - calls → resolved callee
  - implements/overrides → resolved target
  - imports/exports → resolved module symbol
  - references → resolved definition
- Rich node metadata:
  - signature (params/return), visibility, generic params, receiver type, namespace/module path

**Why it is more powerful than AST + FastML**

It eliminates ambiguity that heuristics cannot reliably solve (overloads, trait dispatch, reexports, conditional compilation).

**Where it plugs in**

Project-level analysis after parse is the best fit, because it needs workspace-wide context and build settings.

**Pragmatic scope control**

Start by implementing it for one language (Rust is typically the highest ROI in this repo) and limit outputs to:

- “resolved” edges where confidence is high
- metadata needed for later matching (fully-qualified names), even when edges cannot be resolved

### 2) Build-context analyzer (workspace/package graph + conditional compilation)

**What it is**

Index build manifests and configuration so the analyzer understands what code is “active” and how modules relate:

- Rust: `Cargo.toml`, `Cargo.lock`, `cargo metadata`, feature flags, target cfgs
- Node: `package.json`, lockfiles, tsconfig, workspace boundaries
- Go: `go.mod`, module graph
- Java: `pom.xml`/Gradle files, dependency graph

**What it produces**

- Nodes representing packages/crates/modules and configuration entities (features/targets)
- Edges:
  - depends-on (crate/package/module)
  - enables (feature → module/file sets)
  - generates (build script/codegen → generated artifacts, when discoverable)

**Why it is more powerful**

It reduces incorrect edges caused by analyzing code without the context that compilers use.

**Where it plugs in**

Project-level analysis after parse, and it can also inform file collection (what to include/exclude).

### 3) Cross-file import/export linker (fully-qualified naming without full type checking)

**What it is**

For languages where full type-check is heavy, a middle ground is a **module-aware linker**:

- Parse import/export statements and build a module graph
- Build symbol tables per module/file and resolve references using module scoping rules

**What it produces**

- Fully-qualified names (good for deterministic IDs and stable search)
- More resolved edges for imports/exports and static references

**Why it is more powerful**

It substantially improves cross-file precision at lower cost than full compiler integration.

**Where it plugs in**

Project-level analysis after parse, operating over the aggregated node set.

### 4) Dataflow analyzer (def-use, propagation, and “impact” edges)

**What it is**

Compute a simplified dataflow representation per function/module:

- definitions → uses
- assignment/return propagation
- parameter flow into callees (coarse-grained)

**What it produces**

- New edge kinds: `Defines`, `Uses`, `FlowsTo`, `Returns`, `Captures`, `Mutates`
- “Impact” views: what could change if this symbol changes?

**Why it is more powerful**

It enables higher-quality automated reasoning (“what breaks if…”, “who consumes this value…”) than pure call graphs.

**Where it plugs in**

File-level post-AST enhancement for local def-use; project-level for interprocedural linking (if desired).

**Scope control**

Start with local def-use edges and a conservative model (no alias analysis), then iterate.

### 5) API surface analyzer (public exports, stability, and semver boundaries)

**What it is**

Detect and persist the public API surface for each package/module:

- Rust: `pub` items, reexports, feature-gated API
- TS: exported symbols, declaration emission boundaries

**What it produces**

- Nodes and metadata describing API visibility and ownership (which package exports what)
- Edges:
  - exports (module → symbol)
  - reexports (module → symbol)

**Why it is more powerful**

It makes “what is the supported surface?” explicit, which improves agentic refactors and change planning.

**Where it plugs in**

Project-level analysis after parse; build-context data improves correctness.

### 6) Documentation and contract analyzer (docs → code binding)

**What it is**

Link human-facing contracts to code:

- doc comments (Rustdoc/JSDoc)
- READMEs and design docs
- schemas (OpenAPI/GraphQL/SQL schema files) where applicable

**What it produces**

- Nodes representing documents/contracts
- Edges:
  - documents (doc → symbol)
  - specifies (spec → component)

**Why it is more powerful**

It improves retrieval quality for “why” questions and reduces hallucinations by anchoring to explicit contracts.

**Where it plugs in**

Post-persistence enrichment is convenient (store docs as nodes + link by symbol names), but file-level extraction also works.

### 7) Architecture and boundary analyzer (component detection + dependency constraints)

**What it is**

Infer components (crates/modules/subsystems) and track dependency directionality and cycles.

**What it produces**

- Component nodes (crate/module, optionally folder-based)
- Edges:
  - depends-on (component-level)
  - violates-boundary (when a dependency crosses a configured rule)
- Metrics: centrality, fan-in/out, cyclic dependencies

**Why it is more powerful**

It converts a raw graph into actionable architecture signals: hotspots, cycles, boundary violations, high-impact symbols.

**Where it plugs in**

Post-persistence enrichment (graph queries make this cheap), with optional hints from build-context analyzer.

## Summary: recommended analyzer roadmap

If the goal is “more powerful than AST + FastML” with the smallest number of high-impact additions, prioritize:

1. **Type-aware resolver for Rust** (compiler/LSP-backed) to materially improve edge resolution precision.
2. **Build-context analyzer** to make results correct under features/targets and to model package/crate dependencies explicitly.
3. **Cross-file import/export linker** for languages where full type-check is heavier, to improve qualified naming and basic linking.
4. **Dataflow (local def-use)** to unlock impact analysis beyond call graphs.
5. **Architecture/boundary enrichment** post-persistence to expose cycles and dependency direction problems.
