# Indexing benchmarks

Build the benchmark separately so compilation does not contaminate wall time or RSS:

```sh
cargo build -p codegraph-mcp --example indexing_benchmark --release
python3 scripts/benchmarks/indexing-speed.py \
  --binary <cargo-target-dir>/release/examples/indexing_benchmark \
  --output /tmp/indexing-speed.json --files 1000 --repeats 3
```

The temporary corpus and embedded in-memory database isolate project state. Samples cover
cold parsing, forced warm reconciliation, no change, one-file edit, cross-file rename,
delete, docs/manifest changes and Balanced/Full tier upgrades. Assertions check graph
identity against forced indexing, stale-definition removal, AST reuse and zero writer/
inference work on no-change runs. Startup is reported separately. The wrapper records
binary identity, platform, process wall time and the largest process peak RSS (not the
sum of simultaneous process memory). Debug results verify behavior; use release results
for speed comparisons. Keep corpus, workers, provider, model revision and database mode
fixed between comparisons. Report median and tail samples, not just the fastest run.

Default offline runs disable inference and external analyzers explicitly. `--analyzers`
enables installed language servers/build tools; missing tools fail. Those runs cover
analyzer tier costs. They do not measure remote provider throughput or persistent-disk
write cost. For a real repository/database/provider, use the CLI's `--stats-json` output
and separate cold/model-warm/source-warm states. Run against a disposable project database.

The final debug run on macOS ARM64 used 100 one-line Rust files, three repeats, in-memory
SurrealDB and disabled inference/analyzers. Observed median/max indexing wall times:

| Scenario | Median (ms) | Maximum of three (ms) |
| --- | ---: | ---: |
| Cold | 362.32 | 572.81 |
| No change | 9.50 | 13.14 |
| Forced warm full | 396.75 | 616.80 |
| Single-file edit | 94.58 | 123.50 |
| Cross-file rename | 235.97 | 242.59 |
| Delete | 142.07 | 147.09 |
| Documentation change | 64.86 | 65.05 |
| Manifest change | 65.06 | 68.07 |
| Balanced upgrade | 74.23 | 78.57 |
| Full upgrade | 78.05 | 116.52 |

Every no-change sample reused all ASTs and acknowledged zero writer rows/inference
texts. Single-file edits reused 99 ASTs and acknowledged three rows; docs/manifest
changes acknowledged zero rows with analyzers disabled. Startup median was 89.24 ms;
the largest child process peak RSS was 161,103,872 bytes. These samples were not collected
on an otherwise idle machine.
Forced runs deliberately rewrite the catalog and are not an incremental speed target.
Three samples are insufficient for percentile estimates. This is a harness baseline,
not a comparison against the original implementation or a production throughput claim.

## Same-model runtime and precision probes

Prepare a labeled corpus with short complete texts (each within the 512-token probe
budget). Identity values must describe the actual weights and task:

```json
{
  "identity": {"model": "same-model", "revision": "weights-sha256", "task": "prepared-text"},
  "documents": [{"id": "a", "text": "fn load_config() { /* read settings */ }"}],
  "queries": [{"id": "q", "text": "read configuration", "expected": ["a"]}]
}
```

Run only with explicitly selected, compatible model weights. Local paths reuse existing
models; repository IDs can cause the provider to download weights. No models are loaded
by the offline test suite.

```sh
cargo run -p codegraph-vector --example embedding_probe --features local-embeddings \
  -- corpus.json candle.json local /path/to/model
cargo run -p codegraph-vector --example embedding_probe --features onnx-coreml \
  -- corpus.json onnx.json onnx /path/to/same-model-onnx
python3 scripts/benchmarks/retrieval-quality.py candle.json onnx.json --k 10
```

Exports record startup/inference durations, tokenizer/corpus fingerprints, runtime knobs
and real vectors. The comparison rejects changed model/revision/task/tokenizer/corpus
identities, invalid vectors and missing relevance labels. It reports recall, top-k overlap
and cosine drift; default gates permit at most 0.01 recall loss and require 0.95 overlap.
Use enough labeled queries to justify a quality decision. These probes measure prepared
text with the same task on both backends; production query prompts/tasks require their
own aligned exports and labels.

Compare CoreML/CPU with `CODEGRAPH_ONNX_EP`, cold/warm compiled caches with
`CODEGRAPH_COREML_CACHE_DIR`, Candle F32/F16/BF16 with `CODEGRAPH_LOCAL_DTYPE`,
and quantized ONNX artifacts with `CODEGRAPH_ONNX_MODEL_FILE`. Repeat processes to include
startup, and separately repeat inference in a warm engine. Reduced precision or different
splitters stay opt-in until quality gates pass. No speedup is implied by their availability.
