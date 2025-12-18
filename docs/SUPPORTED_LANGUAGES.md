# Supported languages

This document describes which languages CodeGraph can index today, and what “support” means (AST extraction vs analyzer enrichment).

## What counts as “supported”

CodeGraph’s indexing pipeline has multiple layers:

1. **AST extraction** (Tree-sitter): parses files and emits initial nodes/edges.
2. **FastML** enhancement: fast heuristics that can improve recall.
3. **Analyzers** (optional but enabled by default): build context, module linking, LSP enrichment, docs/contracts, architecture, dataflow (Rust-local).

So a language can be:

- **AST-supported**: Tree-sitter parsing + baseline node extraction works.
- **Analyzer-supported**: additional enrichers run (module linker, LSP, etc.).

## AST-supported languages (Tree-sitter)

These languages are registered in `crates/codegraph-parser/src/language.rs`:

| Language | File extensions |
|---|---|
| Rust | `rs` |
| TypeScript | `ts`, `tsx` |
| JavaScript | `js`, `jsx` |
| Python | `py`, `pyi` |
| Go | `go` |
| Java | `java` |
| C/C++ | `c`, `h`, `cc`, `cpp`, `cxx`, `hpp`, `hxx` |
| Swift | `swift` |
| C# | `cs` |
| Ruby | `rb`, `rake`, `gemspec` |
| PHP | `php`, `phtml`, `php3`, `php4`, `php5` |

## Analyzer support by language

### LSP-backed enrichment

CodeGraph can optionally enrich nodes/edges using Language Server Protocol tooling. The LSP server selection is defined in `crates/codegraph-mcp/src/analyzers/mod.rs`.

| Language | LSP tool | Notes |
|---|---|---|
| Rust | `rust-analyzer` | Adds qualified names and resolves references |
| TypeScript | `typescript-language-server` (+ `node`) | Requires Node + TS LS |
| JavaScript | `typescript-language-server` (+ `node`) | Same server as TS |
| Python | `pyright-langserver` (+ `node`) | Requires Node + Pyright LS |
| Go | `gopls` | |
| Java | `jdtls` | |
| C/C++ | `clangd` | |

You can disable analyzers entirely (or tool requirements) via:

- `CODEGRAPH_ANALYZERS=false`
- `CODEGRAPH_ANALYZERS_REQUIRE_TOOLS=false`

### Module linking

Module/import linking currently targets:

- TypeScript
- JavaScript
- Python
- Go

### Rust-specific analyzers

Some analyzers are Rust-only today:

- Build context (Cargo workspace graph + feature edges)
- Rustdoc/API surface enrichment
- Local dataflow enrichment

## Kotlin and Dart status

CodeGraph’s core language enum includes Kotlin and Dart, and file-extension detection exists in parts of the pipeline, but Tree-sitter parsing for these languages is currently disabled due to Tree-sitter version conflicts:

- `tree-sitter-kotlin` and `tree-sitter-dart` are not enabled in `crates/codegraph-parser/Cargo.toml`

Contributions to restore Kotlin/Dart parsing support are welcome (see `CONTRIBUTING.md`).
