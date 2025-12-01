ABOUTME: Symbol resolution quality investigation (edge %)
ABOUTME: Why dependency resolution dropped to ~51% and how to fix it

Context
-------
- Recent indexing run logged: `Dependencies: 32969 extracted | 17010 stored (resolved 51.6%)`.
- The `symbol_embeddings` table is empty after indexing with the new architecture.
- Resolution pipeline: build symbol_map from CodeNode names → generate known & unresolved symbol embeddings (ai-enhanced) → resolve edges via exact/pattern/AI → store only resolved edges.

Likely root causes
------------------
1) ai-enhanced not active in the binary that ran indexing  
   - The symbol-embedding path is fully gated by `#[cfg(feature = "ai-enhanced")]`. If the installed `codegraph` binary wasn’t built with `ai-enhanced`, no embeddings are generated, resolution falls back to simple name heuristics, and unresolved edges remain high.
2) Symbol embeddings never persisted (or failed silently)  
   - Even with ai-enhanced, `persist_symbol_embedding_records` relies on the Surreal writer queue. If the feature is off or Surreal errors are swallowed, `symbol_embeddings` stays empty, removing the AI fallback from later runs.
3) Symbol map doesn’t cover the edge target shapes emitted by fast_ml/AST extraction  
   - `EdgeRelationship.to` often contains fully-qualified or decorated names (generics, params, macro bangs). The resolver only strips `()`/`!` and splits on `::` once. Common patterns that miss today:  
     - `crate::module::Type::method(args)` (args/generics not stripped)  
     - `self::` / `super::` / `crate::` prefixes  
     - Generic suffixes (`<T>`) and trait-qualified calls (`Type as Trait::method`)  
     - Method receivers encoded in the edge metadata but not in the string key  
   - Result: large fraction of edges never match symbol_map, so even before AI they stay unresolved.
4) Embedding generation mismatch vs table schema  
   - Symbol embedding record maps dims {384, 768, 1024, 2048, 2560, 4096}. If the configured dimension is 1536/3072, records fall into the 2048 branch, producing rows with the wrong column; Surreal indexes for 1536/3072 exist, so queries won’t hit those vectors. This can zero-out effective AI matches.

Impact
------
- Without stored symbol embeddings, AI semantic matching does not kick in, capping resolution to naive heuristics (~50% in the observed run).
- Even with embeddings, normalization gaps leave many edges unresolved, degrading graph quality and downstream agentic tools.

Recommended fixes (priority order)
----------------------------------
1) Build/ship binaries with ai-enhanced enabled by default (or verify install path uses `--all-features`), and add a runtime warning when `CODEGRAPH_DEBUG=1` but `ai-enhanced` is absent.
2) Add durability checks and counters: log the number of symbol embeddings generated and successfully upserted; treat Surreal write failures as hard errors during `--force` indexing.
3) Improve target normalization before lookup: strip args/parentheses, generics `<...>`, leading `self::/super::/crate::`, macro bangs, and trait-qualification (` as Trait`). Keep both fully-qualified and short variants in the symbol_map.
4) Align embedding columns with configured dimension: extend `SymbolEmbeddingRecord` mapping to 1536/3072 (and any other supported dims) so vectors land in matching Surreal fields and indexes.
5) Add a debug sample dump (when CODEGRAPH_DEBUG=1) of the top unresolved targets with their originating files to verify whether they’re external deps vs normalization misses.

Normalization crate options to avoid bespoke heuristics
------------------------------------------------------
- **`symbolic-demangle`** (from the `symbolic` suite) — can demangle Rust/C++ symbols and strip argument type noise. Good fit when `EdgeRelationship.to` contains mangled names coming from compiled artifacts.  
  - Pros: battle-tested in Sentry stack, handles Rust v0 and legacy; simple API (`Name::try_demangle` with `Language::Rust`).  
  - Cons: Works on mangled symbols; does not strip generic params on already-demangled paths.
- **`rustc-demangle`** — lighter-weight demangler for Rust symbols.  
  - Pros: small dependency; focused on Rust.  
  - Cons: same limitation: only helps if inputs are mangled.
- **`syn` + `desynt`** — parse identifiers/paths and strip raw prefixes plus generic params safely.  
  - Suggested pipeline: `syn::parse_str::<syn::Path>(target)` → use `desynt::StripRaw` to drop `r#` → traverse segments, drop `PathArguments`, rebuild `Path` string.  
  - Pros: Rust-grammar-aware; handles trait-qualified paths (`Type as Trait::method`) and generics without regex hacks.  
  - Cons: Heavier compile time; needs `syn` feature flags (`full`) to parse full paths.
- **Fuzzy helpers** (matching, not normalization): `nucleo-matcher` or `code-fuzzy-match` if we decide to fall back to fuzzy matching when normalization still misses; they don’t normalize but can reduce unresolved edges by tolerant matching.

Recommendation: use `syn` + `desynt` for structural stripping of generics/trait-quals/raw idents, and optionally apply `symbolic-demangle`/`rustc-demangle` first when inputs are mangled. Keep fuzzy matcher optional for last-resort matching.

Language-specific parser/normalizer crates (ready-made options)
--------------------------------------------------------------
- **Java:**  
  - `cafebabe` parses `.class` files up to Java 21 and yields class/member names already decoded from the constant pool, which we can reuse for canonical FQNs. citeturn0search0  
  - `java_class_parser` provides higher-level access to class names/fields/methods without manual constant-pool lookups. citeturn0search4  
  - `jdescriptor` parses method/field descriptors; useful for stripping/normalizing signature noise. citeturn0search2
- **JavaScript/TypeScript:**  
  - `biome_js_parser` (Rome/biome) is a fast, lossless JS/TS parser; we can traverse its AST to reconstruct normalized identifier paths (modules/exports/members) instead of regexing strings. citeturn0search8
- **Python:**  
  - `python_parser` (nom-based) builds a Python 3.7+ AST; identifiers can be read from the AST nodes and re-rendered without call-site arguments/qualifiers. citeturn0search10

These crates let us canonicalize identifiers per language by walking their ASTs (strip args/generics, capture module/class/function scopes) rather than expanding our heuristic string cleaner.

Quick verification steps (no rebuild required)
----------------------------------------------
- Run `codegraph` with `--version` and `cargo tree -e features -p codegraph-mcp-server` to confirm `ai-enhanced` is in the binary you’re using.
- After an index run with `CODEGRAPH_DEBUG=1`, check `.codegraph/debug/*` for the symbol embedding generation logs and Surreal write warnings.
- Query Surreal: `select count() from symbol_embeddings;` and `select embedding_2048, embedding_1536 from symbol_embeddings limit 5;` to see whether vectors landed in the correct columns.
