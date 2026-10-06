# Balanced indexing: CLI agent evaluation, 2026-10-06

The refreshed `gpt-6-luna` run passed eight response checks. Hub ranking, coupling
ratio arithmetic and the wrapper-caller explanation now work. Completeness gaps
remain, and the public-API command took 587.5 seconds. This is not an eight-out-of-eight
factual accuracy score.

The [15:19 UTC rerun](#refreshed-balanced-run-1519-utc) and its
[eight full answers](#refreshed-full-agent-answers-1519-utc) are recorded here.
The [original 04:19 UTC run](#original-balanced-run-0419-utc), findings and answers
remain historical evidence.

## Refreshed balanced run (15:19 UTC)

The project owner re-indexed with `balanced` and authorized this rerun with the
same eight questions and an explicit `gpt-6-luna` request model. All eight commands
returned `OK`: **1,053.334 seconds**, **131 tool calls** and **177 summed per-case
locations**. These response checks do not measure factual accuracy.

The hub query now succeeds and the seven reported non-zero instability ratios in
case 6 match the intended formula. Case 7 correctly describes the
`CountingExecutor` indirection that the original balanced answer misclassified.
However, prompt, cache and call-chain answers are less complete than the original
balanced answers, the public-API answer omits one method, and that command takes
587.5 seconds. The evidence supports specific fixes, not an overall accuracy gain
or a conclusion that agent latency is resolved.

### Refreshed run provenance

| Setting | Recorded value |
| --- | --- |
| Run ID | `20261006_151920_418410` |
| Started | 2026-10-06 15:19:20 UTC |
| Transport | Sequential one-shot CLI commands; no MCP server started |
| Questions/focuses | All eight shared cases in [agentic_test_cases.py](../../agentic_test_cases.py), unchanged |
| Indexing tier | `balanced`, confirmed by the project owner; indexing command not captured by the runner |
| Request model | `gpt-6-luna`, explicitly exported as `CODEGRAPH_LLM_MODEL` |
| Agent provider/architecture | Configured OpenAI provider, Rig ReAct; Reflexion recovery in cases 2, 3 and 5 |
| Agent context/output configuration | 1,000,000-token configured window, response `tier: Massive`, output limit 58,000; configured budgets, not verified model capabilities |
| Embeddings | Jina `jina-embeddings-v5-text-small`, 1,024 dimensions |
| Runtime embedding request | `task=retrieval.query`, `truncate=true`, `normalized=true`, `late_chunking=false` |
| Resolved embedding input policy | 512-token chunk input limit including prefixes/special tokens, AST splitting enabled, 64-token overlap, chunking enabled |
| Reranker | Jina `jina-reranker-v3`; successful requests recorded |
| Persisted reconciliation catalog | 238 files, 90,812 lines, 21,832 nodes, 43,266 edges, 23,387 chunks |
| Stored completion | Graph complete, vector index ready, embedding/semantic stages ready; catalog written at 15:17:04 UTC |
| Analyzer evidence | 2,157 LSP-enriched nodes, build context/enrichment/module/docs output; no dataflow additions |
| Checkout throughout run | `df2bab8e7ed75fd1afefeca65033a12c33050a5d` |
| Installed CLI | `codegraph 0.1.0`; SHA-256 `3b6d7b57d1ccbcccd24eb067938e300dd8e9d9381667d5814d4aaf92742b8027`, unchanged after the run |
| Deadline | 600 seconds per case, plus the runner's five-second process allowance |
| Tool-result limits | 200,000 bytes per result, 2,000-character content snippets, 600,000-byte aggregate budget |
| Response checks | 8 `OK`; zero reported partial results, errors or timeouts at command/response level |
| Total command duration | 1,053.334 seconds; includes per-command setup |
| Tool calls / locations | 131 calls; 177 summed per-case locations, not distinct files |

Catalog counts and stage flags were read from the latest persisted reconciliation
artifact without opening the database separately. All eight CLI logs independently
report 21,832 indexed nodes; edge/chunk counts were not independently queried from
the live database. The indexing command and binary build revision were not
recorded: the checkout above identifies reviewed source, not a build attestation.
The request-model label does not attest provider-side routing. No reindexing,
installation or Rust changes were performed for this evaluation.

```sh
CODEGRAPH_LLM_MODEL=gpt-6-luna python3 -u test_cli_agentic.py \
  --project . --output-dir test_output_cli/balanced-gpt-6-luna-refresh \
  --summary-only
```

`--summary-only` suppresses console answer printing; the runner still saves full
answers, structured evidence and diagnostics. Raw files and additional provenance,
diagnostic and arithmetic-review snapshots remain in the ignored directory
`test_output_cli/balanced-gpt-6-luna-refresh/20261006_151920_418410/`.
Replay them without contacting providers:

```sh
python3 test_cli_agentic.py --replay \
  test_output_cli/balanced-gpt-6-luna-refresh/20261006_151920_418410
```

### Refreshed case review

These are qualitative source checks, not numerical accuracy grades. Graph counts
and extracted complexity were not independently reconstructed.

| Case | Question area | Response | Seconds | Tool calls | Source review |
| --- | --- | --- | ---: | ---: | --- |
| 1 | Configuration loading | `OK` | 74.731 | 14 | Useful current-loader inventory with accurate dotenv/TOML precedence; caller inventory remains incomplete (handle_start is omitted). |
| 2 | Tier-aware prompts | `OK` | 98.165 | 32 | Correct thresholds and retrieval limits, but omits the actual 3/5/6/8 default round budgets, prompt task/depth/format details and builder initialization; recovered after MaxTurnsError and later search results were budget-limited. |
| 3 | LRU cache | `OK` | 95.213 | 21 | Correct project-scoped key, lookup and capacity; incomplete about successful-result insertion, post-truncation storage, eviction and clearing. Explicitly says the retrieved excerpt does not expose insertion. |
| 4 | PromptSelector dependencies | `OK` | 44.647 | 12 | Correct absence caveat and a relevant get_tier_system_prompt substitute. Helpers match source; the ContextTier import is at line 5, not the reported line 3. Transitive helper details remain unexamined. |
| 5 | Agent-to-graph call chain | `OK` | 54.923 | 11 | Correct setup and feature branch, but stops at RigExecutor::new; misses RigExecutor::execute, builder/factory, CountingExecutor and graph dispatch. Two traversal outputs were size-truncated, and recovery used the remaining result budget. |
| 6 | Server architecture/hubs | `OK` | 82.877 | 22 | Hub ranking now returns successfully. All seven reported instability ratios match Ce/(Ca+Ce) after rounding, and global/external hubs are qualified correctly. A package coupling result was replaced by a budget note; graph counts and cycle semantics still have coverage limits. |
| 7 | Executor public API | `OK` | 587.500 | 18 | Correct CountingExecutor indirection and visibility/re-export/use-site details, but lists nine of the ten public inherent methods: graph_functions (line 241) is missing. Returned OK only after 587.500 seconds, with a 510.6-second gap between the first completed search/next model round and the next tool call. |
| 8 | Complexity/risk hotspots | `OK` | 15.276 | 1 | All twenty risk scores match complexity times (incoming dependency-edge rows plus one); declarations at all reported starts exist. The answer does not explain candidate preselection or the formula, and calls incoming edge rows dependents without qualifying distinct caller counts. |

### What the refreshed answers establish

1. **Configuration:** The main loader, dotenv/TOML precedence, explicit `[llm]`
   overlay and the advanced/MCP-core/ML loaders match source. The caller inventory
   remains incomplete: [handle_start](../../crates/codegraph-mcp-server/src/bin/codegraph.rs#L740)
   also loads configuration but is omitted. The answer describes the callers it
   found; it does not prove exhaustive coverage.
2. **Prompts:** Tier thresholds and retrieval/over-retrieval limits agree with
   [ContextTier](../../crates/codegraph-mcp-core/src/context_aware_limits.rs#L30).
   The answer omits the actual default **3/5/6/8** round budgets from
   [get_max_turns](../../crates/codegraph-mcp-rig/src/prompts/tier_prompts.rs#L146),
   the task/depth/answer-format prompt helpers and
   [builder initialization](../../crates/codegraph-mcp-rig/src/agent/builder.rs#L63).
   It explicitly declines to establish underlying prompt content. The current
   source uses shared context resolution; the original run's separate-resolution
   finding is historical.
3. **Cache:** Lookup, project-scoped keys and capacities match
   [execute](../../crates/codegraph-mcp-tools/src/graph_tool_executor.rs#L345) and
   [cache_key](../../crates/codegraph-mcp-tools/src/graph_tool_executor.rs#L332).
   The answer says its retrieved excerpt does not establish insertion or the order
   of truncation. Source does: successful results are shortened/truncated before
   [cache.put](../../crates/codegraph-mcp-tools/src/graph_tool_executor.rs#L426).
   Eviction, [clear_cache](../../crates/codegraph-mcp-tools/src/graph_tool_executor.rs#L246)
   and absence of a TTL in this executor are also omitted. These were covered by
   the original balanced answer. The new answer is cautious but incomplete.
4. **Absent target:** `PromptSelector` still has no Rust definition. The answer
   appropriately states that absence and selects the relevant
   [get_tier_system_prompt](../../crates/codegraph-mcp-rig/src/prompts/tier_prompts.rs#L8)
   wrapper as a substitute. The direct helper names are correct. One location is
   wrong: `ContextTier` is imported at line **5**, not line 3. Transitive helper
   coverage is limited, as the answer acknowledges.
5. **Call chain:** The AI-enabled setup and feature-disabled stub are correctly
   distinguished, but the answer stops at `RigExecutor::new`. Source then calls
   [RigExecutor::execute](../../crates/codegraph-mcp-server/src/official_server.rs#L702),
   which uses the builder/tool factory; Rig adapters call
   [CountingExecutor::execute](../../crates/codegraph-mcp-rig/src/tools/counting_executor.rs#L118),
   which delegates to `GraphToolExecutor::execute` and graph dispatch. The original
   balanced answer followed that path. Two oversized traversal results were
   truncated in this rerun; the observed answer does not establish the later path.
6. **Architecture:** `get_hub_nodes` succeeds in **0.627 seconds**, returning a
   ranking rather than an `array::concat` error. The answer distinguishes external
   dependencies (`web-sys`, `windows`) from project hubs. All **seven** displayed
   instability ratios satisfy **Ce/(Ca+Ce)** after rounding: for example the server
   is Ca=19/Ce=85/I=0.817, and `GraphToolExecutor::execute` is 1/19/0.95.
   This verifies the reported arithmetic against the corrected
   [formula](../../schema/codegraph_v2.surql#L469), not every underlying graph edge.
   One package coupling result was replaced by a budget note. Hub candidate limits,
   macro/type coverage and cycle interpretation still constrain conclusions.
7. **Public API:** The answer now correctly identifies the
   [CountingExecutor wrapper](../../crates/codegraph-mcp-rig/src/tools/counting_executor.rs#L120)
   instead of classifying Rig adapters as direct executor callers. Constructors,
   exports, private fields and checked use sites match source. It names **nine of
   ten** public inherent methods, omitting
   [graph_functions](../../crates/codegraph-mcp-tools/src/graph_tool_executor.rs#L241).
   Zero struct-level coupling is qualified using visible consumers, appropriately.
   `OK` therefore does not establish a complete public-API inventory.
8. **Hotspots:** The declarations exist at all 20 reported starts; all 20 reported
   risk scores satisfy **complexity × (incoming dependency-edge rows + 1)**.
   `reconcile_project` leads at 108 × 13 = **1,404**. However, the answer does not
   explain the formula or the [query's candidate preselection](../../schema/codegraph_v2.surql#L555):
   it ranks up to twice the requested limit of high-complexity candidates, not all
   functions globally by risk. Its references to incoming "dependents" need the
   edge-row qualification; these counts need not represent distinct callers. Risk
   is a heuristic, not a calibrated failure probability.

### Refreshed diagnostics and remaining latency

There are **zero logged graph-tool failures**, no `ERROR` lines and no Jina 422
validation errors in these eight commands. The successful hub call in case 6
exercises the formerly failing query; case 7 does not call that tool in this run.
No malformed-id failure was observed, which does not independently exercise every
malformed-id recovery branch.

| Case | Tool-result bytes returned, cumulative | Results reduced by aggregate budget | Reflexion recovery |
| --- | ---: | ---: | --- |
| 1 | 585,647 | 0 | No |
| 2 | 599,560 | 3 | Yes |
| 3 | 495,499 | 0 | Yes |
| 4 | 337,367 | 0 | No |
| 5 | 599,496 | 2 | Yes |
| 6 | 567,969 | 1 | No |
| 7 | 451,524 | 0 | No |
| 8 | 6,428 | 0 | No |

Cases 2, 3 and 5 reach the primary eight-round limit and recover through Reflexion
rather than failing the command. Aggregate-budget reductions occur in cases 2, 5
and 6. These include budget-exhaustion notes replacing results; they are not
database/query failures. Case 5 also logs the run's only two `WARN` lines: traversal
results of 261,931 and 281,842 bytes exceed the 200,000-byte per-result ceiling and
are truncated. Snippet and result limits bound context, but the answers still need
review for the information they omit.

**Latency remains unresolved.** Case 7 finishes in **587.500 seconds**, only 12.5
seconds inside the 600-second whole-agent deadline. Its first search finishes and
the next model round begins at **15:26:54.328 UTC**; the next tool call starts at
**15:35:24.881 UTC**, a **510.6-second** gap with no graph tool running in the log.
This places the observed delay between tool calls, during the agent/model turn,
rather than in a long-running graph query. The provider-side cause is not
established. No timeout happened in this run, but the earlier latency problem
cannot be declared generally resolved.

All eight answers are prose rather than typed-schema JSON. The server synthesizes
`structured_output` highlights from tool traces. The current parser logs that
fallback at `DEBUG`, so absence of the original typed-answer `WARN` messages is
not evidence of typed-answer success.

### Refreshed comparison limits

This run changes the installed binary/checkout, indexed files, embedding provider,
input/chunk policy, result limits and deadline relative to the original balanced
run. In particular it uses Jina v5 with a 512-token chunk policy rather than Ollama
Qwen3 with the previous 32,768-token policy. Both request `gpt-6-luna`, but these
are not controlled tier-only or before/after accuracy comparisons.

**Earlier evaluation answers are in the searchable index.** Structured locations
in cases 1–4 include the original balanced and/or full evaluation documents, which
contain saved answers to these same questions. That contamination means this is
an operational smoke test with source review, not a held-out accuracy benchmark.
A future controlled accuracy comparison should exclude saved evaluation answers
from its index. This run did not change the owner's index or configuration.

The original run and its full answers are preserved below. All eight new answers
are also preserved [verbatim](#refreshed-full-agent-answers-1519-utc). A complete
full-tier rerun has not been performed as part of this balanced evaluation.

## Original balanced run (04:19 UTC)

Eight CLI response checks passed, but this is not an eight-out-of-eight factual
accuracy score. Source review found useful implementation explanations, a mistaken
direct-caller classification, missing hub results and metric limitations. The full
agent answers are preserved below so the findings can be assessed independently.

## Run provenance

| Setting | Recorded value |
| --- | --- |
| Run ID | `20261006_041913_884481` |
| Started | 2026-10-06 04:19:13 UTC |
| Transport | Sequential one-shot CLI commands; no MCP server started |
| Questions/focuses | All eight shared cases in [agentic_test_cases.py](../../agentic_test_cases.py), unchanged |
| Indexing tier | `balanced`, reported by the indexing session; the runner does not capture the indexing command |
| Request model | `gpt-6-luna`, explicitly exported as `CODEGRAPH_LLM_MODEL` for this run |
| Agent provider/architecture | Configured OpenAI provider, Rig ReAct |
| Agent context/output configuration | 1,000,000-token configured window, response `tier: Massive`, output limit 58,000; these are configured budgets, not independently verified model capabilities |
| Embeddings | Ollama `qwen3-embedding:0.6b`, 1,024 dimensions |
| Resolved embedding input policy | Model/serving limit 32,768 tokens, publisher tokenizer, retrieval query prefix, unprefixed documents, truncation disabled |
| Stored catalog | 250 files, 94,173 lines, 22,496 nodes, 48,224 edges, 22,496 chunks |
| Stored completion | Graph complete, vector index ready, embedding/semantic stages ready; catalog written at 04:12:02 UTC |
| Analyzer evidence | 2,256 LSP-enriched nodes; build context and enrichment present; no dataflow additions |
| Checkout at explicit-run start | `48738bf` (documentation-only commit after `165efd0`) |
| Installed CLI | `codegraph 0.1.0`; SHA-256 `85dfd4b2296be0f0d58c20476ca638054d3c360fbc31ff3c40962a7e45c2682c` |
| Deadline | 300 seconds per case, plus the runner's five-second process allowance |
| Response checks | 8 `OK`, zero reported partial results, errors or timeouts at the command/response level |
| Total command duration | 462.820 seconds; includes per-command setup |
| Tool calls | 125; these are calls, not tool-loop rounds |
| Structured locations | 181 summed per-case locations, not 181 distinct files across the run |

The index's build revision and the installed binary's build revision were not
recorded by the indexing command/runner. The checkout revision above identifies
the source reviewed, not a verified build attestation. The script does not capture
the provider's returned model identity, so the request-model label does not attest
provider-side routing. No reindexing, installation or Rust changes were performed
for this evaluation.

Run command, from the project root:

```sh
CODEGRAPH_LLM_MODEL=gpt-6-luna python3 -u test_cli_agentic.py \
  --project . --output-dir test_output_cli/balanced-gpt-6-luna
```

Complete JSON responses, structured evidence, stdout/stderr, timings and logs remain
in the ignored local directory
`test_output_cli/balanced-gpt-6-luna/20261006_041913_884481/`.
Inspect them without contacting providers:

```sh
python3 test_cli_agentic.py --replay \
  test_output_cli/balanced-gpt-6-luna/20261006_041913_884481
```

## Case review

These are qualitative judgments of observed answers, not numerical grades or
exhaustive proofs of graph coverage. Locations, conditional implementations,
method visibility, wrapper types and query formulas were checked against source.
All graph connectivity counts were not independently reconstructed.

| Case | Question area | Response | Seconds | Tool calls | Source review |
| --- | --- | --- | ---: | ---: | --- |
| 1 | Configuration loading | `OK` | 56.413 | 11 | Useful, source-aligned coverage of multiple loading paths and runtime-use caveats |
| 2 | Tier-aware prompts | `OK` | 72.279 | 24 | Correct builder/tier flow and config-fallback mismatch; budget and numerical-metric caveats |
| 3 | LRU cache | `OK` | 38.917 | 8 | Source-aligned hits/misses, successful-result insertion, truncation, eviction, clearing and lack of TTL |
| 4 | `PromptSelector` dependencies | `OK` | 52.651 | 12 | Appropriate missing-symbol caveat and explicitly stated alternate target |
| 5 | Agent-to-graph call chain | `OK` | 122.686 | 40 | Correct active feature branch, wrapper/dispatch flow and architectural branches |
| 6 | Server architecture/hubs | `OK` | 56.850 | 19 | Useful structure; incomplete hub/cycle evidence, with failures and metric inconsistencies disclosed |
| 7 | Executor public API | `OK` | 40.892 | 10 | All ten public method names, but indirect adapters mislabeled as direct callers |
| 8 | Complexity/risk hotspots | `OK` | 22.132 | 1 | Useful reported ranking; heuristic risk, candidate scope and edge-count limitations |

### Source checks and qualifications

1. **Configuration:** The [main manager](../../crates/codegraph-core/src/config_manager.rs#L481)
   loads TOML/defaults, applies environment overrides and validates. Dotenv
   [initialization](../../crates/codegraph-core/src/config_manager.rs#L524) is separate.
   The [layered Settings loader](../../crates/codegraph-core/src/config.rs#L376),
   [advanced-config loader](../../crates/codegraph-core/src/advanced_config.rs#L364)
   and [MCP-core loader](../../crates/codegraph-mcp-core/src/config_manager.rs#L165)
   also exist. The answer appropriately distinguishes available APIs from established
   production callers. Its observation about `~/.codegraph.env` versus the config
   init command's `~/.codegraph/.env` paths is supported by source.
2. **Prompts:** [Builder initialization](../../crates/codegraph-mcp-rig/src/agent/builder.rs#L63)
   and [system_prompt](../../crates/codegraph-mcp-rig/src/agent/builder.rs#L126)
   use the real shared prompt builder. Server and Rig context resolution are
   separate, as the answer says. The eight-round cap applies to the default
   [budget selector](../../crates/codegraph-mcp-rig/src/prompts/tier_prompts.rs#L144);
   the public [max_turns override](../../crates/codegraph-mcp-rig/src/agent/builder.rs#L104)
   does not enforce that cap. The reported Ca=14/Ce=5/instability=0.0 tuple also
   needs caution: it is inconsistent with the intended Ce/(Ca+Ce) ratio. The
   evaluation does not establish whether that originates in query arithmetic,
   graph data or answer synthesis.
3. **Cache:** The [key](../../crates/codegraph-mcp-tools/src/graph_tool_executor.rs#L275),
   [execution](../../crates/codegraph-mcp-tools/src/graph_tool_executor.rs#L288) and
   [clear_cache](../../crates/codegraph-mcp-tools/src/graph_tool_executor.rs#L189)
   implementations support the explanation. Successful post-truncation JSON is
   cached; propagated execution errors do not reach insertion. No TTL appears in
   this executor's cache. This does not describe every other cache in the project.
4. **Missing target:** No Rust definition named `PromptSelector` exists in the
   reviewed checkout. The alternate wrapper really calls `build_system_prompt`
   and `get_max_turns`; the production builder calls `build_system_prompt` directly.
   The question remains an absent-symbol test, not an ordinary dependency-coverage
   measurement for a live `PromptSelector` implementation.
5. **Call chain:** The [AI-enabled workflow](../../crates/codegraph-mcp-server/src/official_server.rs#L574)
   creates the executor and calls Rig. The disabled-feature stub is separate.
   [CountingExecutor](../../crates/codegraph-mcp-rig/src/tools/counting_executor.rs#L39),
   the dispatch branches and [GraphFunctions::trace_call_chain](../../crates/codegraph-graph/src/graph_functions.rs#L260)
   support the described flow. ReAct tool selection is dynamic; LATS does not
   register these tools. This answer correctly avoids the diagnostic run's
   conclusion that the active workflow only returns an error.
6. **Architecture:** The [HTTP adapter](../../crates/codegraph-mcp-server/src/http_server.rs#L15)
   wraps the shared server, and the server/Rig integration uses the shared executor.
   Hub queries failed four times in this case. The answer names that gap and
   declines to conclude cycle presence/absence from unclear returned records.
   Its reported server metrics Ca=19/Ce=66/instability=0.0 are inconsistent with
   the intended [coupling ratio](../../schema/codegraph_v2.surql#L443): 66/85 is
   approximately 0.776, not zero. Zero struct/impl counts likewise do not establish
   absence of consumers; source references show those consumers exist.
7. **API:** Source has exactly the ten listed public inherent method names:
   `new`, `with_context_window`, `with_limits`, `with_cache`, `cache_stats`,
   `graph_functions`, `clear_cache`, `execute`, `get_tool_schemas` and
   `get_tool_names`. The answer's **direct-caller statement is incorrect**.
   [Rig adapters hold CountingExecutor](../../crates/codegraph-mcp-rig/src/tools/graph_tools.rs#L138)
   and call its `execute`; that wrapper calls the inner `GraphToolExecutor::execute`
   at [line 41](../../crates/codegraph-mcp-rig/src/tools/counting_executor.rs#L41).
   The adapters are indirect consumers. One hub query also failed in this case.
8. **Hotspots:** The named functions and locations exist, and all 20 displayed risk
   scores satisfy complexity × (reported afferent count + 1), including
   108 × 13 = 1,404. The [query](../../schema/codegraph_v2.surql#L529) first takes
   at most twice the requested limit of functions ordered by complexity, then
   ranks that candidate set by risk. This is not a guaranteed global risk ranking.
   Its afferent counts aggregate dependency-edge rows rather than distinct source
   nodes; repeated relationships need not represent different callers. The exact
   extracted complexity/coupling values were not independently recomputed, and
   this risk formula is not a calibrated prediction of failures.

## Diagnostics and comparison limits

There were **five `get_hub_nodes` errors**, across cases 6 and 7:

```text
Incorrect arguments for function array::concat(). Output must not exceed 1048576 bytes.
```

Each of the eight cases also logged a typed-answer parse warning. The agent returned
prose; [the server](../../crates/codegraph-mcp-server/src/official_server.rs#L748)
fell back to synthesizing structured highlights from tool traces. Those are actual
CLI fallback results, not successful typed-schema answers. The runner's `OK`
classification checks the command and returned response, not stderr tool failures,
complete task coverage or factual accuracy. The warning/error payloads are retained
in the ignored raw artifacts; credentials and endpoint configuration are not copied
into this report.

The first balanced diagnostic run, `20261006_041408_086869`, completed eight response
checks in 265.352 seconds. It used the same index but did not set the Rig model
override. The project `.env` set `CODEGRAPH_MODEL=gpt-6-luna`, while
[Rig model selection](../../crates/codegraph-mcp-rig/src/adapter/llm_adapter.rs#L77)
reads `CODEGRAPH_LLM_MODEL`, then `CODEGRAPH_AGENT_MODEL`, and otherwise defaults to
requesting `gpt-4o` for OpenAI. Neither Rig override was set in that diagnostic
process. The configured request name is inferred from this source/environment;
the provider's actual served model was not captured. Its saved answers selected
the feature-disabled stub in case 5 and incorrectly suggested that the executor
was isolated in case 7.

That run remains under `test_output_cli/balanced/20261006_041408_086869/`; it is not
merged into the explicit-model result. The historical fast run's `gpt-6-luna`
label came from the session and cannot be verified from the runner artifacts.
That fast index also predates the current tokenizer/context/chunk policy. Model
selection, input policy, corpus/graph changes, cache warmth and nondeterministic
reasoning prevent attributing differences to indexing tier alone. No overall
accuracy percentage or tier speedup is assigned. Full-tier evaluation is pending.

## Full agent answers

These are the returned `answer` strings, not rewritten summaries. Any absolute
local repository/home prefixes are replaced with `.`/`$HOME` for portability;
other answer text, including mistakes, is preserved. The raw structured evidence
and diagnostics can be inspected using the replay command above.

### Case 1: agentic_context (default)

How is configuration loaded in this codebase? Find all config loading mechanisms.

<details>
<summary>Full agent answer</summary>

Configuration is **not loaded through one unified path**: the graph shows several separate managers and component-specific environment readers. For the main CLI/server configuration, `codegraph-core::config_manager::ConfigManager::load()` reads a TOML file or defaults, applies environment overrides, then validates; a separate `ConfigManager` in `config.rs` assembles layered settings and can watch for changes.

### Main CLI/server configuration

In `crates/codegraph-core/src/config_manager.rs:481-520`, `ConfigManager::load()` invokes file discovery, environment overrides, and validation. `load_config_file()` at `:554-580` chooses the first available source: `CODEGRAPH_CONFIG_PATH`, `./.codegraph.toml`, `~/.codegraph/config.toml`, or built-in defaults. The chosen TOML is read and parsed by `read_toml_file()` at `:583-591`. `apply_env_overrides()` at `:594-752` then overrides embedding, reranking, LLM, indexing, logging, and daemon settings from environment variables. So, for this path, the effective order is **file/defaults, then environment overrides**.

This is used by the CLI’s `run_cli()` (`crates/codegraph-mcp-server/src/bin/codegraph.rs:619-738`), `handle_agent_status()` (`:2004-2222`), and `handle_daemon_start()` (`:2354-2437`). The MCP server also loads it in `detect_context_tier()` (`crates/codegraph-mcp-server/src/official_server.rs:525-545`) and `execute_agentic_workflow()` (`:574-892`). The daemon startup paths in `handle_start()` also load it (`crates/codegraph-mcp-server/src/bin/codegraph.rs:740-1129`).

Before those environment reads, `ConfigManager::initialize_environment()` in `crates/codegraph-core/src/config_manager.rs:524-546` can import variables from the working directory’s `.env`, or—if that is absent—from `~/.codegraph.env`. The graph shows calls from the binary `main()` and `prepare_environment()` in `crates/codegraph-mcp-server/src/agent_cli.rs`. This is separate from `load()`: its documentation says `load()` does not mutate the process environment. Note that the CLI’s `config init` code documents and creates `~/.codegraph/.env` (`crates/codegraph-mcp-server/src/bin/codegraph.rs:1759-2002`), while `initialize_environment()` searches for `~/.codegraph.env`; those are different paths.

### Layered settings manager and reload

`crates/codegraph-core/src/config.rs:376-412` defines another mechanism, `ConfigManager::load_from_sources()`. It uses the `config` builder to add optional `default` files in TOML/YAML/JSON, environment-named files in those formats, `local.toml`, and `CODEGRAPH__*` environment variables; it then optionally adds decrypted secrets and deserializes `Settings`. The config directory is selected by `default_config_dir()` (`:284-307`): `~/.codegraph`, then `./config`, then the current directory. The environment name comes from `APP_ENV`, then `RUST_ENV`, then defaults to `development` (`:208-213`).

`new_watching()` (`crates/codegraph-core/src/config.rs:265-282`) loads and validates those settings, then starts `spawn_watcher()` (`:414-440`), which reloads them when files in the config directory change. `load_from_sources()` is also called directly by that watcher. The graph did not establish an external production caller for `new_watching()`, so its availability is clear but its runtime use is not.

Encrypted secrets are a source within this mechanism, not a separate settings loader: `try_load_encrypted_secrets()` (`crates/codegraph-core/src/config.rs:442-448`) checks for `secrets.enc`, `secrets.toml.enc`, or `secrets.enc.toml`; `decrypt_to_temp_toml()` (`:450-475`) uses `CONFIG_ENC_KEY` and writes a temporary TOML file for the settings builder.

### Advanced configuration and other file loaders

`AdvancedConfig::from_file()` in `crates/codegraph-core/src/advanced_config.rs:364-372` reads TOML directly. `ConfigurationManager::from_file()` (`:408-414`) wraps that loader, and the configuration CLI uses it from `ConfigCliHandler::new()` (`crates/codegraph-core/src/cli_config.rs:160-173`), defaulting to `codegraph.toml` when no path is provided and to `AdvancedConfig::default()` if the file is absent. `AdvancedConfig` also has JSON string deserialization via `from_json()` (`crates/codegraph-core/src/advanced_config.rs:382-390`). Its `apply_environment_overrides()` method (`:282-351`) is a separate override facility; the graph does not establish that it is automatically applied by `from_file()`.

For advanced-config hot reload, `ConfigurationManager::start_hot_reload()` (`crates/codegraph-core/src/advanced_config.rs:461-502`) periodically calls `AdvancedConfig::from_file()` when enabled and when a file path is present. The graph found this as an available method but did not establish a caller that starts it.

There are also narrower file-loading APIs. `MLPipeline::load_config()` in `crates/codegraph-vector/src/ml/pipeline.rs:473-485` reads JSON into `MLPipelineConfig`; the indexed caller is a serialization test in the same file (`:740-754`). `codegraph-mcp-core` has a separate `ConfigManager::load_config()` at `crates/codegraph-mcp-core/src/config_manager.rs:165-191`, which accepts an optional path, reads TOML, applies server/cache environment overrides, and validates; its TOML parser is `load_from_file()` (`:194-202`). The graph found no callers of that loader, so it appears to be a separate library API rather than the main CLI/server path.

### Component-specific environment configuration

Several components read environment variables directly instead of going through those managers. These are narrower configuration mechanisms, not additional loads of the whole application config:

- LLM provider selection is handled by `RigProvider::from_env()` in `crates/codegraph-mcp-rig/src/adapter/llm_adapter.rs:26-52`: an explicit `CODEGRAPH_LLM_PROVIDER` takes priority, followed by provider/API-key detection. The OpenAI and Anthropic provider `from_env()` methods construct from their respective `Default` configurations (`crates/codegraph-ai/src/openai_llm_provider.rs:71-73`; `crates/codegraph-ai/src/anthropic_provider.rs:64-66`).
- Embedding provider selection can happen in `EmbeddingGenerator::with_auto_from_env()` (`crates/codegraph-vector/src/embedding.rs:301-502`). It reads `CODEGRAPH_EMBEDDING_PROVIDER` and related model/batch variables to construct a provider. Jina also reads provider-specific variables in `JinaConfig::default()` (`crates/codegraph-vector/src/jina_provider.rs:46-101`); OpenAI embedding defaults read `OPENAI_API_KEY` in `crates/codegraph-vector/src/openai_provider.rs:35-45`.
- Reranker selection has `create_reranker_from_env()` in `crates/codegraph-vector/src/reranking/factory.rs:53-75`, which reads `CODEGRAPH_RERANK_PROVIDER` and falls back to the supplied `RerankConfig`.
- HTTP server settings have `HttpServerConfig::from_env()` in `crates/codegraph-mcp-server/src/http_config.rs:34-46`, reading `CODEGRAPH_HTTP_*`. Indexer behavior also has environment readers such as `AnalyzerSettings::from_env()` (`crates/codegraph-mcp/src/analyzers/mod.rs:64-74`) and `InferencePolicies::from_env()` (`crates/codegraph-mcp/src/policy.rs:44-55`).

The main practical distinction is whether a change belongs to the **core `CodeGraphConfig` loading path**, the **layered `Settings` manager**, the **advanced-config CLI**, or a **component-specific environment reader**. The graph does not show a single source that coordinates all of them, and it did not establish runtime callers for the layered watcher, advanced hot reload, or the `codegraph-mcp-core` loader.

</details>

### Case 2: agentic_context (builder)

Gather comprehensive context about the tier-aware prompt selection system

<details>
<summary>Full agent answer</summary>

The system builds one system-prompt template, selecting its task text from `AnalysisType` and its investigation guidance, response format, and tool-round budget from `ContextTier`; it does not keep a separate prompt template for every tier. The main implementation is `crates/codegraph-mcp-rig/src/prompts/tier_prompts.rs` (lines 8–153), wired into agent construction by `crates/codegraph-mcp-rig/src/agent/builder.rs` (lines 63–128). One integration detail merits attention: the server computes a tier for reporting, but the workflow shown does not pass it to the Rig executor, whose builder independently derives its tier.

**Prompt construction and selectors.** In `crates/codegraph-mcp-rig/src/prompts/tier_prompts.rs` (lines 8–10), `get_tier_system_prompt` combines `build_system_prompt` with the tier’s default turn budget. The core builder at lines 21–68 formats a shared identity/instructions/example structure with task text, tier-specific depth guidance, the supplied turn limit, and a tier-specific answer format. `get_task` at lines 71–98 maps the analysis type to the task instructions; `get_depth_guidance` at lines 101–116 and `get_answer_format` at lines 119–134 provide the tier-dependent portions. The eight analysis variants—code search, dependency analysis, call-chain analysis, architecture analysis, API-surface analysis, context building, semantic question, and complexity analysis—are declared in `crates/codegraph-mcp-core/src/analysis.rs` (lines 5–14).

`crates/codegraph-mcp-rig/src/prompts/mod.rs` (lines 1–7) declares the tier-prompts module and re-exports `build_system_prompt`, `detect_tier`, `get_max_turns`, and `get_tier_system_prompt`. In the production builder path, `RigAgentBuilder::system_prompt` calls `build_system_prompt` directly (`crates/codegraph-mcp-rig/src/agent/builder.rs`, lines 126–128); the tier-specific wrapper is used by tests in the indexed graph. That builder defaults its analysis type to `SemanticQuestion` and exposes `.analysis_type(...)`, `.tier(...)`, and `.max_turns(...)` overrides (same file, lines 63–107).

**Tier classification and resulting behavior.** The shared `ContextTier` enum and thresholds live in `crates/codegraph-mcp-core/src/context_aware_limits.rs` (lines 19–39): Small covers 0–50,000 tokens, Medium 50,001–150,000, Large 150,001–500,000, and Massive anything above that. Prompt turn budgets in `tier_prompts.rs` (lines 144–153) are respectively 3, 5, 6, and 8; the hard cap is 8. The same tier also affects retrieval and output limits in `context_aware_limits.rs` (lines 42–80): base retrieval limits are 10/25/50/100, local over-retrieval multipliers 5/8/10/15, cloud multipliers 3/4/5/8, and output-token defaults 2,048/4,096/8,192/16,384. These are adjacent tier behavior, not additional prompt templates.

**How the prompt reaches execution.** `RigAgentBuilder::new` obtains a context-window value, classifies it, and sets the initial turn budget; `.tier(...)` and `.max_turns(...)` can override those values, and `system_prompt()` uses the resulting configuration (`crates/codegraph-mcp-rig/src/agent/builder.rs`, lines 63–128). Provider-specific ReAct builders use that prompt as the preamble and attach the graph tools (`builder.rs`, lines 258–427). The agent implementations enforce the configured turn count by passing `self.max_turns` to the Rig prompt loop—for example, the OpenAI implementation in `crates/codegraph-mcp-rig/src/agent/react.rs` (lines 64–105) and the Anthropic implementation (lines 118–158). This is why prompt budget changes should be checked against both builder propagation and execution enforcement, not just prompt text.

The server workflow in `crates/codegraph-mcp-server/src/official_server.rs` (lines 574–892) determines a tier, logs it, then constructs `RigExecutor` and calls `execute(query, analysis_type)` without passing that tier. The server’s detector (same file, lines 525–545) checks `CODEGRAPH_CONTEXT_WINDOW`, then falls back to `ConfigManager`’s `llm.context_window`. By contrast, `RigAgentBuilder::new` gets its value from `get_context_window` in `crates/codegraph-mcp-rig/src/adapter/llm_adapter.rs` (lines 108–114): it checks `CODEGRAPH_CONTEXT_WINDOW`, then `CODEGRAPH_LLM_CONTEXT_WINDOW`, and defaults to 128,000. `RigExecutor::execute` constructs the builder without a tier override, except that it forces Small when estimated history usage exceeds 80%; an explicit-tier path exists as `execute_with_tier` (`crates/codegraph-mcp-rig/src/agent/executor.rs`, lines 48–157 and 181–219). Therefore, when the server falls back to config rather than the shared environment variable, its reported tier can differ from the tier used to build the actual prompt. The config’s own context-window default is 32,000 (`crates/codegraph-core/src/config_manager.rs`, lines 417–419), while the Rig adapter default is 128,000, so the no-override path is especially worth checking.

**Neighboring tool and test code.** Prompt instructions deliberately do not enumerate tool semantics: the builder attaches tools through `GraphToolFactory` in `crates/codegraph-mcp-rig/src/agent/builder.rs` (lines 258–427), and the factory exposes semantic search, dependency traversal, call-chain, circular-dependency, coupling, hub, reverse-dependency, and complexity tools in `crates/codegraph-mcp-rig/src/tools/factory.rs` (lines 14–81). The prompt builder’s documented contract points tool descriptions to `crates/codegraph-mcp-rig/src/tools/graph_tools.rs`; that file is the neighboring place to account for if changing tool semantics.

The prompt tests in `tier_prompts.rs` (lines 161–217) check that every analysis type has a prompt at each tier, that Small and Massive prompts differ, that rendered budgets match `get_max_turns`, that an explicit turn override appears in the prompt, and that turn budgets increase with tier. Those tests cover prompt generation and its text, while the ReAct execution code is the evidence for enforcement.

**Coupling and change considerations.** The graph measured `build_system_prompt` at afferent coupling 3, efferent coupling 0, instability 0.0; that indicates a small, stable prompt-generation surface in this graph. It measured `RigAgentBuilder::new` at afferent coupling 14, efferent coupling 5, instability 0.0; it has more neighboring uses, so changing its default tier derivation has broader propagation implications. These metrics describe graph-recorded relationships, not every external or runtime consumer.

For a tier-selection change, keep the prompt guidance and answer-format tests aligned with the tier mapping, and verify that the same tier drives the prompt, the enforced turn count, and server reporting. The clearest follow-up is to reconcile the server’s config fallback with the Rig builder’s environment/default path—potentially by passing the detected tier through `execute_with_tier` or unifying context-window resolution—and add coverage for config-only tier selection.

</details>

### Case 3: agentic_context (question)

How does the LRU cache work in GraphToolExecutor? What gets cached and when?

<details>
<summary>Full agent answer</summary>

`GraphToolExecutor` caches **successful tool-call JSON results**, keyed by the executor’s project ID, tool name, and the serialized input parameters. A hit returns the cached value before the tool runs; a miss runs the tool, truncates an oversized result if needed, then caches that post-truncation result. The cache is an in-memory, mutex-protected LRU with a default capacity of 100, and caching can be disabled or configured through the constructors.

**Key and cached value.** In `crates/codegraph-mcp-tools/src/graph_tool_executor.rs`, `cache_key` builds `"{project_id}:{tool_name}:{parameters}"` from the project ID, tool name, and `parameters.to_string()` (lines 275–278). `execute` validates the tool, checks the cache, and returns immediately on a hit; on a miss it dispatches to one of the eight implemented tools, truncates the result if needed, and stores a clone of it (lines 288–391). The stored value is the returned JSON result—including its tool/parameters/result wrapper—not an intermediate database response. Since the cache lookup is before dispatch, a hit also skips work such as semantic-search embedding, graph enrichment, and reranking.

**When entries are added and evicted.** In that same `execute` method, insertion occurs only after dispatch and result truncation complete successfully; errors propagated from tool execution return without reaching the insertion code (lines 288–391). The cache is declared as `LruCache<String, JsonValue>` behind `Arc<Mutex<...>>` (lines 51–68); a cache `get` serves hits and `put` adds results. At capacity, the LRU policy evicts an entry, and the executor updates its eviction and size statistics around insertion (lines 288–391). There is no time-based expiry shown; `clear_cache` explicitly clears entries and resets hit, miss, eviction, and current-size counters (lines 189–197).

**Capacity and observability.** `with_context_window` enables caching with capacity 100 by default, while `with_limits` accepts an enable flag and custom capacity; a zero capacity is replaced with 100 for the actual LRU (lines 89–158). `CacheStats` exposes hits, misses, evictions, current size, and maximum size (lines 24–35). The code’s key uses the raw serialized input parameters, so calls with different parameter JSON can have different entries even if the tool interprets them equivalently.

</details>

### Case 4: agentic_impact (dependencies)

Analyze the dependency chain for the PromptSelector. What does it depend on?

<details>
<summary>Full agent answer</summary>

The indexed codebase contains no symbol named `PromptSelector`. Assuming you mean the tier-based system-prompt selector, `get_tier_system_prompt` in `crates/codegraph-mcp-rig/src/prompts/tier_prompts.rs:8` directly calls `build_system_prompt` and `get_max_turns`, both in that file. The graph shows no call cycle involving this prompt-selection path.

**Dependencies.** `get_tier_system_prompt` passes the requested `AnalysisType` and `ContextTier` to `build_system_prompt`, and gets the tool-call budget from `get_max_turns` (`tier_prompts.rs:8–10`). `get_max_turns` maps the tier to a budget—3, 5, 6, or 8 turns—and caps it at 8 (`tier_prompts.rs:144–153`). `build_system_prompt` assembles the prompt from the analysis type, tier, and budget (`tier_prompts.rs:21–68`). Its source also consults prompt-building helpers; the graph’s call-dependency traversal did not resolve further linked dependencies for that function. The relevant input types are `AnalysisType` (`crates/codegraph-mcp-core/src/analysis.rs:5`) and `ContextTier` (`crates/codegraph-mcp-core/src/context_aware_limits.rs:19`).

**Dependents and wider path.** The graph identifies direct callers of `get_tier_system_prompt` only in its tests: `test_all_analysis_types_have_prompts` (`tier_prompts.rs:172–180`), `test_tier_affects_requested_depth` (`:183–189`), and `test_prompt_states_enforced_budget` (`:192–201`). The builder path is separate: `RigAgentBuilder::system_prompt` calls `build_system_prompt` directly (`crates/codegraph-mcp-rig/src/agent/builder.rs:126–128`), rather than calling `get_tier_system_prompt`. So changes to the shared prompt-building function may affect both paths, while changes specific to the tier wrapper’s delegation or budget selection are not shown as affecting production callers.

The cycle check found no cycle involving these prompt functions; it did find unrelated call cycles elsewhere in the repository. The exact-name `PromptSelector` could not be established from the graph, so the analysis above treats `get_tier_system_prompt` as the likely intended target.

</details>

### Case 5: agentic_impact (call_chain)

Trace the call chain from execute_agentic_workflow to the graph analysis tools

<details>
<summary>Full agent answer</summary>

With `ai-enhanced` enabled, `execute_agentic_workflow` builds a shared `GraphToolExecutor`, then hands the query to `RigExecutor`, which chooses an agent architecture; with the default ReAct path, the LLM can call the registered graph tools as needed. Each call flows through the matching Rig tool, `CountingExecutor`, and `GraphToolExecutor` dispatch branch to a `GraphFunctions` method that queries SurrealDB. **This is a branching, agent-driven flow—not a fixed sequence of all tools.**

**Setup and agent selection.** In `crates/codegraph-mcp-server/src/official_server.rs:574–892`, the AI-enabled `execute_agentic_workflow` initializes `GraphFunctions`, the embedding generator, and `GraphToolExecutor` (around lines 637–708), then calls `RigExecutor::execute` (around lines 712–724). The separate implementation at `official_server.rs:896–909` is a stub that returns an error when `ai-enhanced` is disabled.

`crates/codegraph-mcp-rig/src/agent/executor.rs:48–157` builds the agent and runs it; if execution fails, it **branches** to a Reflexion retry path. Agent construction branches in `crates/codegraph-mcp-rig/src/agent/builder.rs:158–167`: ReAct/Rig builds a tool-calling agent, LATS follows a reasoning-only path and does not call graph tools, and Reflexion wraps ReAct. With ReAct, provider-specific builders register the graph tools in `builder.rs:254–428`.

**Tool-call path and dispatch.** On ReAct, an individual tool call enters its corresponding implementation in `crates/codegraph-mcp-rig/src/tools/graph_tools.rs`—for example, `TraceCallChain::call` at lines 230–257 converts arguments to JSON and calls `CountingExecutor::execute`. `crates/codegraph-mcp-rig/src/tools/counting_executor.rs:39–65` increments the call count, delegates to the shared `GraphToolExecutor`, and records the result or error trace.

`crates/codegraph-mcp-tools/src/graph_tool_executor.rs:288–391` validates the tool name, checks the result cache, **branches by tool name**, then applies result-size limiting and caches the result. Its eight dispatch branches are `get_transitive_dependencies`, `detect_circular_dependencies`, `trace_call_chain`, `calculate_coupling_metrics`, `get_hub_nodes`, `get_reverse_dependencies`, `semantic_code_search`, and `find_complexity_hotspots`. These are alternatives selected per LLM tool call; the agent may invoke multiple branches over its run.

**Graph-analysis endpoints.** The dispatch handlers in `graph_tool_executor.rs` forward to the corresponding `GraphFunctions` methods in `crates/codegraph-graph/src/graph_functions.rs` (the `GraphFunctions` implementation begins at line 70). For example, the `trace_call_chain` handler at lines 447–470 forwards to `GraphFunctions::trace_call_chain` at lines 260–288, which issues `fn::trace_call_chain` to SurrealDB. Other evidenced endpoints include transitive dependencies (`graph_functions.rs:183–215`), circular dependencies (`224–250`), and reverse dependencies (`370–400`); the remaining dispatch handlers forward coupling metrics, hubs, semantic search, and complexity hotspots to their matching `GraphFunctions` methods.

The semantic-search branch has an additional **conditional** path: `execute_semantic_code_search` at `graph_tool_executor.rs:548–606` generates an embedding, calls `semantic_search_with_context`, then applies reranking if configured (`630–664`). `GraphFunctions::semantic_search_with_context` at `graph_functions.rs:511–529` delegates to `semantic_search_nodes_via_chunks`.

**Return path.** Tool results flow back through `CountingExecutor`, where they are captured in traces, then to the Rig agent for further reasoning or a final answer. `execute_agentic_workflow` receives the answer and traces and parses or synthesizes structured output before returning JSON (`official_server.rs:724–892`).

</details>

### Case 6: agentic_architecture (structure)

Analyze the architecture of the MCP server. Find coupling metrics and hub nodes.

<details>
<summary>Full agent answer</summary>

The MCP server is layered around a shared server implementation: the CLI and HTTP transport expose `CodeGraphMCPServer`, which owns the MCP-facing tools and delegates graph analysis to `GraphToolExecutor`; the tool executor then calls graph and search services. The strongest measured coupling is in the shared server crate and the executor’s dispatch function, but the hub-node query failed, so I cannot give a verified ranked hub list.

**Components and dependency direction.** `codegraph-mcp-server` contains the entrypoint, server implementation, HTTP transport/configuration, and agent-related modules. The binary imports `CodeGraphMCPServer` in `crates/codegraph-mcp-server/src/bin/codegraph.rs:12`; the public library exposes its modules and re-exports in `crates/codegraph-mcp-server/src/lib.rs:5-20`. The core server type and tool router are defined in `crates/codegraph-mcp-server/src/official_server.rs:240-245`. Its tool methods delegate to a common workflow, while its `ServerHandler` implementation supplies server information and prompts at `crates/codegraph-mcp-server/src/official_server.rs:913-960`.

HTTP is an adapter around that same server: `build_http_app` accepts `CodeGraphMCPServer` and `HttpServerConfig`, constructs the streamable HTTP service, and mounts it alongside a health route in `crates/codegraph-mcp-server/src/http_server.rs:15-40`. `start_http_server` builds the app and binds/serves it in `crates/codegraph-mcp-server/src/http_server.rs:43-74`; host rules and configuration helpers are in `crates/codegraph-mcp-server/src/http_config.rs:18-69`. Thus the transport layer depends on the server implementation and config, not the reverse in the graph results examined.

The MCP-facing server also imports `GraphToolExecutor` from `codegraph-mcp-tools` at `crates/codegraph-mcp-server/src/official_server.rs:39`. That executor centralizes graph-tool dispatch, schema validation, caching, and result-size handling; it holds graph functions, config, an embedding generator, cache, and optional reranker in `crates/codegraph-mcp-tools/src/graph_tool_executor.rs:51-68`, with its implementation spanning `:73-692`. The dispatch method routes named tools to the individual handlers at `crates/codegraph-mcp-tools/src/graph_tool_executor.rs:288-391`. The direction is therefore server/agent callers → shared tool executor → graph-function and embedding/search services.

The same executor is also consumed outside the server: `codegraph-mcp-rig` imports it in `crates/codegraph-mcp-rig/src/tools/factory.rs:6`, and its factory creates tool wrappers around the executor in `crates/codegraph-mcp-rig/src/tools/factory.rs:14-81`. The server’s agent workflow also creates a `GraphToolExecutor` and passes it to the Rig backend in `crates/codegraph-mcp-server/src/official_server.rs:392-909`. This makes the executor a shared boundary between the server and the agent/tool layer, rather than transport-specific logic.

`codegraph-mcp-core` provides shared protocol types: `McpProtocol` is declared in `crates/codegraph-mcp-core/src/protocol.rs:13-15`, and the initialize-result type is in `crates/codegraph-mcp-core/src/message.rs:84-90`. The indexed connection code imports core protocol/version/message types from that crate in `crates/codegraph-mcp/src/connection.rs:3-8`, consistent with protocol concerns living below transports and clients.

**Coupling and connectivity.** The reported package-level metrics for `codegraph-mcp-server` are Ca=19, Ce=66, total coupling=85, and a reported category of “stable” (`crates/codegraph-mcp-server/Cargo.toml`). The counts indicate a broad package boundary with many recorded dependencies and dependents; the result’s reported instability is 0.0 despite those nonzero counts, so I would not treat that value/category as a reliable signal without validating the graph metric calculation.

At function level, `GraphToolExecutor::execute` has Ca=9 and Ce=18 (total=27) in `crates/codegraph-mcp-tools/src/graph_tool_executor.rs:288-391`. Its dependents include multiple Rig tool `call` functions in `crates/codegraph-mcp-rig/src/tools/graph_tools.rs:164-175, 206-215, 246-256, 287-296, 327-336, 367-378, 409-420, 451-461`; its dependencies include the executor’s individual handlers and schema/cache/logging helpers. This is the clearest measured high-connectivity seam: changes to its dispatch contract or behavior can affect several consumers and touch many internal operations.

`build_http_app` reports Ca=6 and Ce=8 (total=14) in `crates/codegraph-mcp-server/src/http_server.rs:15-40`, with dependencies including the server type, router/configuration, and host-policy helper. By contrast, coupling measurements for the `CodeGraphMCPServer` struct (`crates/codegraph-mcp-server/src/official_server.rs:240-245`), its handler impl (`:913-960`), its inherent impl (`:248-381`), and the `GraphToolExecutor` struct (`crates/codegraph-mcp-tools/src/graph_tool_executor.rs:51-68`) all returned zero. Given the concrete cross-component references above, those zeroes appear to reflect limitations of the graph’s struct/impl-level coupling extraction rather than proof those components are unconnected.

**Hubs and cycles.** The whole-codebase hub query failed on repeated attempts, so no ranked hub nodes or degree values can be confirmed. From the available coupling data, `GraphToolExecutor::execute` is the best evidenced connectivity hotspot, but that is not a substitute for a successful hub ranking. The import-cycle query returned a few import-edge records, including records involving MCP-core and other crates, but did not provide clear cycle paths or a server-specific cycle report. I therefore cannot conclude either that the MCP server has an import cycle or that it is cycle-free.

The main follow-up is to rerun hub and cycle analysis after the graph tools are available, and to validate coupling results at module/package granularity: the reported zeroes for central structs and impls conflict with the recorded dependency structure, while the server package’s “stable” label conflicts with its high reported Ce.

</details>

### Case 7: agentic_architecture (api_surface)

What is the public API surface of the GraphToolExecutor?

<details>
<summary>Full agent answer</summary>

Assuming “public API surface” means the Rust-visible `GraphToolExecutor` type, its public inherent methods, and the crate paths that expose it: the API is centered on construction, tool execution, cache/graph access, and listing tool schemas or names. **`execute` is the most broadly used operational entry point:** eight Rig tool adapters call it. The graph’s coupling result for the struct itself is zero, however, so it does not reliably capture the usage visible in the indexed imports and call sites.

**Type and exports.** `GraphToolExecutor` is a public struct in `crates/codegraph-mcp-tools/src/graph_tool_executor.rs:51–68`; its fields are private. The tools crate exposes the module at `crates/codegraph-mcp-tools/src/lib.rs:4` and glob-reexports its public contents at line 7. The `codegraph-mcp` crate also reexports `GraphToolExecutor` at `crates/codegraph-mcp/src/lib.rs:27` (under the `embeddings` feature). The indexed code shows imports in the server (`crates/codegraph-mcp-server/src/official_server.rs:39`) and Rig integration code, including `crates/codegraph-mcp-rig/src/tools/factory.rs:6`, `crates/codegraph-mcp-rig/src/agent/executor.rs:10`, and `crates/codegraph-mcp-rig/src/tools/counting_executor.rs:5`.

**Public methods.** The public inherent API is implemented in `crates/codegraph-mcp-tools/src/graph_tool_executor.rs` (impl begins at line 73):
- Constructors: `new` (uses `CODEGRAPH_CONTEXT_WINDOW` with a default), `with_context_window` (line 92), `with_limits` (line 113), and `with_cache` (line 161; documented as legacy compatibility). `new` delegates through the context-window constructor; the alternate constructors allow explicit result-size and cache configuration.
- `execute` (line 288) accepts a tool name and JSON parameters and returns a JSON result asynchronously. It dispatches the supported graph tools, applies result-size limiting, and uses the cache when enabled.
- `cache_stats`, `graph_functions`, and `clear_cache` expose cache statistics, a cloned `Arc<GraphFunctions>`, and cache reset behavior, respectively.
- `get_tool_schemas` and `get_tool_names` expose the available tool schemas and names. The implementation’s dispatch includes eight tool names: transitive dependencies, circular dependencies, call-chain tracing, coupling metrics, hub nodes, reverse dependencies, semantic search, and complexity hotspots.

**Callers and change risk.** The eight direct callers of `execute` are the `Tool::call` implementations in `crates/codegraph-mcp-rig/src/tools/graph_tools.rs`: `GetTransitiveDependencies` (lines 164–175), `DetectCircularDependencies` (206–215), `TraceCallChain` (246–256), `CalculateCouplingMetrics` (287–296), `GetHubNodes` (327–336), `GetReverseDependencies` (367–378), `SemanticCodeSearch` (409–420), and `FindComplexityHotspots` (451–461). Because each funnels a different public-facing tool through `execute`, changes to its signature or common behavior have the widest evidenced impact.

The executor is also wrapped by `CountingExecutor` (`crates/codegraph-mcp-rig/src/tools/counting_executor.rs:22–36`) and accepted by `GraphToolFactory::new` (`crates/codegraph-mcp-rig/src/tools/factory.rs:14–20`). That makes `execute` and the constructor/type contract the riskiest surface to change; the schema/name accessors are narrower listing APIs. The measured struct coupling was **afferent 0, efferent 0, instability 0.0**. Treat that as an indexing gap rather than evidence of no dependents: the graph also shows the exports/imports above and the eight concrete calls to `execute`.

</details>

### Case 8: agentic_quality (default)

Find the highest complexity hotspots in the codebase. Which functions have the highest risk scores?

<details>
<summary>Full agent answer</summary>

`reconcile_project` has the highest measured risk score (1404), while `walk` in the Rust parser has the most afferent coupling (15)—the largest count of other nodes depending on a hotspot. `reconcile_project` is also the most complex function by a wide margin (108). Ranked below by the tool’s risk score, these are the 20 returned hotspots with complexity at least 5.

| Rank | Function and location | Risk score | Complexity | Afferent / efferent coupling |
|---:|---|---:|---:|---:|
| 1 | `reconcile_project` — `./crates/codegraph-mcp/src/indexer.rs:632–2221` | 1404 | 108 | 12 / 317 |
| 2 | `walk` — `./crates/codegraph-parser/src/languages/rust.rs:109–513` | 496 | 31 | 15 / 97 |
| 3 | `scan_and_emit` — `./crates/codegraph-core/src/watch/mod.rs:246–478` | 440 | 44 | 9 / 0 |
| 4 | `walk` — `./crates/codegraph-parser/src/languages/java.rs:109–457` | 246 | 41 | 5 / 74 |
| 5 | `extract_symbols` — `./crates/codegraph-core/src/watch/mod.rs:1001–1134` | 231 | 33 | 6 / 35 |
| 6 | `new` — `./crates/codegraph-vector/src/embeddings/generator.rs:301–483` | 220 | 22 | 9 / 4 |
| 7 | `enrich_async` — `./crates/codegraph-mcp/src/analyzers/lsp.rs:589–924` | 205 | 41 | 4 / 83 |
| 8 | `walk` — `./crates/codegraph-parser/src/languages/ruby.rs:122–394` | 198 | 33 | 5 / 45 |
| 9 | `walk` — `./crates/codegraph-parser/src/languages/php.rs:130–428` | 192 | 32 | 5 / 61 |
| 10 | `apply_env_overrides` — `./crates/codegraph-core/src/config_manager.rs:594–752` | 184 | 46 | 3 / 0 |
| 11 | `walk` — `./crates/codegraph-parser/src/languages/swift.rs:108–387` | 180 | 30 | 5 / 55 |
| 12 | `walk` — `./crates/codegraph-parser/src/languages/cpp.rs:105–394` | 174 | 29 | 5 / 57 |
| 13 | `walk` — `./crates/codegraph-parser/src/languages/csharp.rs:109–394` | 168 | 28 | 5 / 53 |
| 14 | `extract_imports` — `./crates/codegraph-core/src/watch/mod.rs:1162–1239` | 154 | 22 | 6 / 20 |
| 15 | `collect_source_files_with_config` — `./crates/codegraph-parser/src/file_collect.rs:29–164` | 144 | 18 | 7 / 31 |
| 16 | `process_path_event` — `./crates/codegraph-core/src/watch/mod.rs:534–727` | 140 | 35 | 3 / 53 |
| 17 | `walk` — `./crates/codegraph-parser/src/languages/go.rs:104–296` | 138 | 23 | 5 / 39 |
| 18 | `strip_comments_c_like` — `./crates/codegraph-core/src/watch/mod.rs:887–935` | 112 | 16 | 6 / 2 |
| 19 | `syntax_boundaries` — `./crates/codegraph-parser/src/chunk_boundaries.rs:13–60` | 108 | 18 | 5 / 11 |
| 20 | `parse_batch_scores` — `./crates/codegraph-vector/src/reranking/ollama.rs:117–171` | 90 | 18 | 4 / 8 |

The highest raw complexity and highest risk coincide at `reconcile_project`; its 12 afferent connections also make it a high-impact hotspot. The Rust parser’s `walk` ranks second by risk and has the most dependents in this set, so changes there also warrant particular care. By contrast, `apply_env_overrides` has the second-highest raw complexity (46) but ranks tenth by risk, with only 3 afferent connections.

</details>

## Refreshed full agent answers (15:19 UTC)

Answers below are reproduced exactly from the saved `response.answer` strings;
source-review corrections are kept above, outside the agent's prose.

### Refreshed case 1: agentic_context (default)

> How is configuration loaded in this codebase? Find all config loading mechanisms.

`OK` · 74.731 seconds · 14 tool calls · 25 structured locations

<!-- balanced-refresh:20261006_151920_418410:case:1:begin -->
There isn’t one universal configuration loader: the main application uses `codegraph-core::ConfigManager`, while the CLI’s advanced settings, the MCP-core API, the ML pipeline, and several focused components each have their own loaders. For the main loader, `.env` files are loaded separately before configuration is read; the loader then applies environment overrides to file settings or defaults. I’m treating “all” as the runtime configuration mechanisms surfaced by the code graph, including scoped loaders—not every individual `env::var` use.

**Main application configuration.** `ConfigManager::load()` in `crates/codegraph-core/src/config_manager.rs:534–569` gets a config from `load_config_file()`, applies environment overrides, validates it, and returns it. `config_file_path()` at `crates/codegraph-core/src/config_manager.rs:609–620` selects `CODEGRAPH_CONFIG_PATH`, then `./.codegraph.toml`, then `~/.codegraph/config.toml`; if none is found, `load_config_file()` at `crates/codegraph-core/src/config_manager.rs:622–633` uses `CodeGraphConfig::default()`. TOML reading and parsing happen in `read_toml_file()` at `crates/codegraph-core/src/config_manager.rs:670–678`. Although the `load()` documentation describes environment variables above the file, the implementation’s sequence is file/default first, then environment overrides.

The environment setup is separate: `ConfigManager::initialize_environment()` at `crates/codegraph-core/src/config_manager.rs:577–599` loads a project `.env`, or, if absent, `~/.codegraph.env`. The code documents that `load()` itself does not mutate the environment. The CLI agent’s `prepare_environment()` calls this setup and passes an explicit `--config` path through `CODEGRAPH_CONFIG_PATH` at `crates/codegraph-mcp-server/src/agent_cli.rs:166–191`. The graph lists `load()` calls from `run_cli`, `handle_config`, `handle_daemon_start`, and `handle_agent_status` in `crates/codegraph-mcp-server/src/bin/codegraph.rs` (the `load()` node is at `crates/codegraph-core/src/config_manager.rs:534–569`).

**Other file-based loaders.**

- The advanced configuration API reads TOML directly: `AdvancedConfig::from_file()` at `crates/codegraph-core/src/advanced_config.rs:364–372`. The CLI’s `ConfigCliHandler::new()` at `crates/codegraph-core/src/cli_config.rs:160–173` loads the supplied path—or `codegraph.toml` by default—when it exists, otherwise using `AdvancedConfig::default()`. `ConfigurationManager::from_file()` wraps that loader at `crates/codegraph-core/src/advanced_config.rs:408–413`. Its `start_hot_reload()` periodically rereads the same file and validates it at `crates/codegraph-core/src/advanced_config.rs:461–502`.
- `AdvancedConfig::from_json()` deserializes a JSON string at `crates/codegraph-core/src/advanced_config.rs:387–389`. The graph shows serialization-test callers at `crates/codegraph-core/src/advanced_config.rs:530–538` and `crates/codegraph-core/tests/config_integration_test.rs:57–70`; this is a parsing API, rather than a file-path discovery mechanism.
- The ML pipeline has an independent JSON-file loader: `MLPipeline::load_config()` reads a path and deserializes `MLPipelineConfig` at `crates/codegraph-vector/src/ml/pipeline.rs:473–485`. The graph shows a serialization test calling the save/load path at `crates/codegraph-vector/src/ml/pipeline.rs:740–754`.
- `codegraph-mcp-core` contains another `ConfigManager::load_config(config_file)` at `crates/codegraph-mcp-core/src/config_manager.rs:165–191`. It starts from defaults, optionally reads the caller-supplied TOML path via `load_from_file()` at `crates/codegraph-mcp-core/src/config_manager.rs:194–202`, applies its own environment overrides, and validates. Unlike the main loader, this API does not discover project/global config paths itself; if the supplied path is missing, it retains defaults. It is a distinct API and config type, not the same loader as `codegraph-core::ConfigManager`.

**LLM-specific settings.** The LLM adapter independently reads only explicitly supplied `[llm]` keys. `ConfigManager::explicit_llm_settings()` resolves the config path and reads the file at `crates/codegraph-core/src/config_manager.rs:641–656`; `ExplicitLlmSettings::from_toml_str()` extracts the supported keys at `crates/codegraph-core/src/config_manager.rs:439–472`. The adapter combines those settings with environment values in `Sources` at `crates/codegraph-mcp-rig/src/adapter/llm_adapter.rs:19–102`; `RigProvider::from_env()` enters that resolution at `crates/codegraph-mcp-rig/src/adapter/llm_adapter.rs:152–163`. The adapter’s provider selection prioritizes the environment, then an explicit file provider, then API-key detection.

**Scoped environment loaders and overrides.** In addition to the main `ConfigManager` overrides at `crates/codegraph-core/src/config_manager.rs:681–843`, the graph surfaces separate environment-backed settings: `AdvancedConfig::apply_environment_overrides()` at `crates/codegraph-core/src/advanced_config.rs:283–336`; `HttpServerConfig::from_env()` at `crates/codegraph-mcp-server/src/http_config.rs:34–46`; `AnalyzerSettings::from_env()` at `crates/codegraph-mcp/src/analyzers/mod.rs:64–74`; and `InferencePolicies::from_env()` at `crates/codegraph-mcp/src/policy.rs:44–55`. Embedding provider adapters also add provider-specific environment fallbacks: `OllamaEmbeddingConfig::from()` at `crates/codegraph-vector/src/ollama_embedding_provider.rs:50–87` and `LmStudioEmbeddingConfig::from()` at `crates/codegraph-vector/src/lmstudio_embedding_provider.rs:45–85`. These are focused settings, not replacements for the main config loader. The graph search establishes these mechanisms but does not prove that every direct environment-variable read in the codebase has been identified.
<!-- balanced-refresh:20261006_151920_418410:case:1:end -->

### Refreshed case 2: agentic_context (builder)

> Gather comprehensive context about the tier-aware prompt selection system

`OK` · 98.165 seconds · 32 tool calls · 22 structured locations

<!-- balanced-refresh:20261006_151920_418410:case:2:begin -->
Tier-aware prompt selection is split across a shared `ContextTier` policy, tier- and analysis-specific prompt construction, and the agent/server code that supplies the configured tier. The configured context window determines one of four tiers; that tier affects prompt depth and tool-round budget, and also selects retrieval and over-retrieval limits elsewhere in the system.

**Tier policy.** `ContextTier::from_context_window` in `crates/codegraph-mcp-core/src/context_aware_limits.rs:30–39` maps windows up to 50,000 to Small, 50,001–150,000 to Medium, 150,001–500,000 to Large, and larger windows to Massive. The same policy provides tier-specific retrieval settings in `crates/codegraph-mcp-core/src/context_aware_limits.rs:30–80`: base limits are 10, 25, 50, and 100; local-search over-retrieval multipliers are 5×, 8×, 10×, and 15×; cloud-search multipliers are 3×, 4×, 5×, and 8×, respectively. Thus, changing tier thresholds or policy values can affect both the prompt and retrieval behavior.

**Prompt construction.** `get_tier_system_prompt(analysis_type, tier)` in `crates/codegraph-mcp-rig/src/prompts/tier_prompts.rs:8–10` delegates to `build_system_prompt` and obtains the default round count from `get_max_turns`. `detect_tier` in that file at lines 158–160 delegates to the shared `ContextTier::from_context_window`, rather than defining a separate mapping. The prompt module re-exports `build_system_prompt`, `detect_tier`, `get_max_turns`, and `get_tier_system_prompt` from `crates/codegraph-mcp-rig/src/prompts/mod.rs:4–7`.

The indexed prompt-tier documentation (`docs/AGENT_PROMPT_TIERS.md`, document begins at line 1) describes a shared prompt layout with task text selected by analysis type, and tier-dependent investigation depth, answer detail, and stated tool-round budget. It also says tier selection uses the `CODEGRAPH_CONTEXT_WINDOW` environment variable ahead of `llm.context_window` loaded via `ConfigManager::load()`. The graph results do not expose the underlying prompt text or config-loading implementation sufficiently to verify those details beyond the documentation.

**Agent integration and callers.** `RigAgentBuilder::system_prompt` in `crates/codegraph-mcp-rig/src/agent/builder.rs:126–128` calls `build_system_prompt` with the builder’s analysis type, tier, and max-turn value. The graph connects this method to provider-specific agent-building functions in the same file, so changes to prompt inputs or construction should be checked alongside those builders. On the server side, `detect_context_tier` in `crates/codegraph-mcp-server/src/official_server.rs:526–530` gets the configured context window through `ConfigManager::agent_context_window()` and maps it through `ContextTier::from_context_window`. The CLI’s `handle_agent_status` in `crates/codegraph-mcp-server/src/bin/codegraph.rs:2004` also obtains that configured window and derives a tier; its status reporting then maps tiers to verbosity labels and tier-specific parameters in the function body. These paths show that the prompt tier is part of a broader context-aware configuration, not an isolated prompt switch.

**Tests and change considerations.** Tests in `crates/codegraph-mcp-rig/src/prompts/tier_prompts.rs:174–219` check that every analysis type produces a prompt for each tier, that Small and Massive prompts differ, that prompts state their round budget (including an explicit override), that maximum rounds increase with tier, and that tier detection maps representative context-window sizes as expected. The shared policy also has tier-detection tests in `crates/codegraph-mcp-core/src/context_aware_limits.rs:173–187`.

If changing selection behavior, keep the shared threshold mapping, prompt construction/default rounds, builder-provided overrides, and server-side tier consumers consistent. The graph evidence does not establish a complete list of prompt call sites or the exact prompt variants, and coupling or complexity metrics were not available; those are worth checking if the change spans beyond the Rig builder path.
<!-- balanced-refresh:20261006_151920_418410:case:2:end -->

### Refreshed case 3: agentic_context (question)

> How does the LRU cache work in GraphToolExecutor? What gets cached and when?

`OK` · 95.213 seconds · 21 tool calls · 20 structured locations

<!-- balanced-refresh:20261006_151920_418410:case:3:begin -->
`GraphToolExecutor` keeps a bounded, mutex-protected LRU of JSON tool results. When caching is enabled, `execute` checks it after validating the tool name but before dispatching the tool; a hit returns a clone of the cached result immediately, while a miss is counted and proceeds to execution. The cache key combines the project ID, tool name, and serialized parameters, so different projects or inputs do not share an entry.

In `crates/codegraph-mcp-tools/src/graph_tool_executor.rs`, the `GraphToolExecutor` fields document the cache as holding tool results and define it as `Arc<Mutex<LruCache<String, JsonValue>>>` (lines 51–68). The `execute` implementation validates the tool, looks up that key when `cache_enabled` is true, clones and returns a hit, and records a miss before dispatch (lines 345–roughly 400). The key format is `project_id:tool_name:serialized_parameters` (lines 332–335).

Capacity is set during construction: `with_context_window` enables caching with a capacity of 100, while `with_limits` accepts an enabled flag and custom size; a zero size falls back to an LRU capacity of 100 (lines 145–168 and 170–215). The cache and its statistics are shared through `Arc<Mutex<…>>`, and `CacheStats` tracks hits, misses, evictions, current size, and maximum size (lines 24–35 and 51–68).

The indexed excerpt shows the lookup and miss path, but not the later cache-write branch. So it establishes that results are looked up before execution and that the cache is intended for tool results, but does not let me verify precisely which completed results are inserted or whether truncation happens before insertion.
<!-- balanced-refresh:20261006_151920_418410:case:3:end -->

### Refreshed case 4: agentic_impact (dependencies)

> Analyze the dependency chain for the PromptSelector. What does it depend on?

`OK` · 44.647 seconds · 12 tool calls · 18 structured locations

<!-- balanced-refresh:20261006_151920_418410:case:4:begin -->
The indexed codebase contains no symbol named `PromptSelector`. Assuming you mean the closest prompt-selection function, `get_tier_system_prompt`, its direct dependencies are `get_max_turns` and `build_system_prompt`; it also takes `AnalysisType` and `ContextTier` as inputs. The graph reports no call cycle involving this proxy function, though that is not evidence about an unindexed `PromptSelector`.

In `crates/codegraph-mcp-rig/src/prompts/tier_prompts.rs:8–10`, `get_tier_system_prompt(analysis_type, tier)` calls `get_max_turns(tier)` and passes that result, along with both inputs, to `build_system_prompt`. These are its **direct call dependencies**. The file imports `AnalysisType` at line 4 and `ContextTier` at line 3; `AnalysisType` is defined in `crates/codegraph-mcp-core/src/analysis.rs:5–14`. The graph did not establish the `ContextTier` definition.

The resulting call chain is:

- `get_tier_system_prompt` → `get_max_turns` (`tier_prompts.rs:146–155`), which maps a context tier to a turn budget.
- `get_tier_system_prompt` → `build_system_prompt` (`tier_prompts.rs:21–70`), which constructs the prompt from the analysis type, tier, and budget.

No further **transitive call dependencies** were returned beneath either helper. That means the graph did not expose additional calls in the chain; it does not establish that prompt construction uses no other inputs or logic.

The graph shows test functions that call `get_tier_system_prompt` directly in `tier_prompts.rs:174–203` (`test_all_analysis_types_have_prompts`, `test_tier_affects_requested_depth`, and `test_prompt_states_enforced_budget`). It returned no production callers for this function. A call-cycle scan found cycles elsewhere in the codebase, but none involving this function or its two reported helper dependencies.
<!-- balanced-refresh:20261006_151920_418410:case:4:end -->

### Refreshed case 5: agentic_impact (call_chain)

> Trace the call chain from execute_agentic_workflow to the graph analysis tools

`OK` · 54.923 seconds · 11 tool calls · 25 structured locations

<!-- balanced-refresh:20261006_151920_418410:case:5:begin -->
`execute_agentic_workflow` is the entry point ( `crates/codegraph-mcp-server/src/official_server.rs:559–901`, with the feature-disabled fallback at `:905–917`). In the `ai-enhanced` path, it sets up project-scoped graph access and constructs the graph-tool executor, then constructs the Rig executor that shares it. **The indexed call-chain evidence does not identify the later dispatch from the Rig executor to a particular graph-analysis tool**, so I can trace the setup to the tool boundary, but not which tool method runs for a given request.

The setup path and its branches are:

- **Choose the database configuration:** `execute_agentic_workflow` calls `SurrealDbConfig::for_project` (`crates/codegraph-graph/src/surrealdb_storage.rs:143–148`). That selects remote configuration when `CODEGRAPH_SURREALDB_URL` is set, or embedded configuration otherwise.
- **Open the store:** it calls `SurrealDbStorage::new` (`crates/codegraph-graph/src/surrealdb_storage.rs:306–340`). Within that constructor, the connection path branches between embedded and remote storage; persistent embedded stores can also take the schema-setup path.
- **Create graph access:** `GraphFunctions::new_with_project_id` (`crates/codegraph-graph/src/graph_functions.rs:114–119`) is called from the workflow and provides project-scoped graph operations.
- **Build the graph-tool layer:** `GraphToolExecutor::new` (`crates/codegraph-mcp-tools/src/graph_tool_executor.rs:134–142`) receives `Arc<GraphFunctions>`, configuration, and the embedding generator. It delegates to `with_context_window` (`:147–167`), which configures result limits and continues to `with_limits`; the executor is thus the boundary between the workflow and graph-analysis tools.
- **Hand the tool executor to the agent executor:** `RigExecutor::new` (`crates/codegraph-mcp-rig/src/agent/executor.rs:40–45`) stores a shared `Arc<GraphToolExecutor>`. That is the last supported step toward tool invocation in the available call-chain evidence.

There are two other visible branches to keep in mind: the database configuration branch between remote and embedded storage, and the compile-time feature branch where the fallback workflow returns an “Agentic tools require the `ai-enhanced` feature to be enabled” error (`official_server.rs:905–917`). The exact runtime route from `RigExecutor` through tool selection/dispatch to individual graph-analysis methods remains unestablished; tracing that dispatch is the follow-up needed to name the analysis tool calls and their branches.
<!-- balanced-refresh:20261006_151920_418410:case:5:end -->

### Refreshed case 6: agentic_architecture (structure)

> Analyze the architecture of the MCP server. Find coupling metrics and hub nodes.

`OK` · 82.877 seconds · 22 tool calls · 22 structured locations

<!-- balanced-refresh:20261006_151920_418410:case:6:begin -->
Assuming “MCP server” means the server crate plus the MCP support crates it builds on, the architecture is layered: transport and process startup feed the server implementation, which delegates graph-tool execution to a separate tools crate; protocol types and configuration sit in shared core crates, while the daemon and Rig agent backend add background indexing and agentic workflows. The clearest coupling concern is that the server-facing packages have many outgoing dependencies and high graph-measured instability; the cycle results exist, but the reported examples do not establish a transport-to-tools architectural loop.

**Components and dependency direction.** In `crates/codegraph-mcp-server/src/http_server.rs`, `build_http_app` builds an Axum router and mounts the streamable MCP HTTP service at `/mcp` (lines 15–40); `start_http_server` builds that app, binds the listener, and serves it (lines 43–74). The MCP server implementation is represented by `CodeGraphMCPServer` in `crates/codegraph-mcp-server/src/official_server.rs` (struct at lines 240–245). The server imports `GraphToolExecutor` from `codegraph-mcp-tools` (`official_server.rs`, line 39), establishing the primary request/tool dependency as **server → tools**.

The tools layer defines schemas and graph operations: `GraphToolSchemas` includes operations such as `get_transitive_dependencies`, `trace_call_chain`, `get_hub_nodes`, and `find_complexity_hotspots` (`crates/codegraph-mcp-tools/src/graph_tool_schemas.rs`, lines 34–65, 89–115, 139–158, 227–254). `GraphToolExecutor::execute` validates the requested tool name, checks its cache, and dispatches by tool name (`crates/codegraph-mcp-tools/src/graph_tool_executor.rs`, lines 345–457). Its downstream dependencies include graph functions and shared MCP-core logging/protocol helpers. The core crate also provides shared MCP message and protocol types, process management, configuration, and progress reporting; examples include `message.rs` and `protocol.rs` under `crates/codegraph-mcp-core/src/` and process management in `crates/codegraph-mcp-core/src/process.rs` (lines 44–96, 168–212).

The adjacent packages extend rather than replace that path. The graph-family documentation describes `codegraph-mcp-daemon` as the background file-watching and long-running state component, `codegraph-mcp-rig` as the agent backend, and `codegraph-mcp-tools` as the search/traversal tools crate (`docs/crates/codegraph-mcp-server/README.md`, line 1). In code, `execute_agentic_workflow` in `crates/codegraph-mcp-server/src/official_server.rs` (lines 559–901) invokes configuration, graph storage, progress, and Rig components; `codegraph-mcp-rig` also imports `GraphToolExecutor` (for example, `crates/codegraph-mcp-rig/src/agent/executor.rs`, line 10). Thus the agent path builds on the tools/graph layer, while the server owns the client-facing integration.

**Measured coupling.** The coupling values below are graph counts, not just Cargo crate-to-crate edges; they include the graph’s represented symbols and dependencies. The calculated package metrics are:

- `codegraph-mcp-server` (`crates/codegraph-mcp-server/Cargo.toml`, line 1): Ca 19, Ce 85, instability 0.817. Its outgoing coupling substantially exceeds its incoming coupling.
- `codegraph-mcp-core` (`crates/codegraph-mcp-core/Cargo.toml`, line 1): Ca 5, Ce 138, instability 0.965.
- `codegraph-mcp-tools` (`crates/codegraph-mcp-tools/Cargo.toml`, line 1): Ca 5, Ce 39, instability 0.886.
- `codegraph-mcp-daemon` (`crates/codegraph-mcp-daemon/Cargo.toml`, line 1): Ca 1, Ce 68, instability 0.986.
- `codegraph-mcp-rig` (`crates/codegraph-mcp-rig/Cargo.toml`, line 1): Ca 6, Ce 125, instability 0.954.

All five are classified as “unstable” by the metric. In particular, the server and core packages are dependency-heavy, so changes that reach beyond a narrow interface may have broad effects. Treat the precise ratios cautiously: the graph’s package nodes aggregate many internal symbols and external dependencies, so these values are not a direct measure of runtime or source-level cohesion.

At function level, `GraphToolExecutor::execute` has Ca 1, Ce 19, instability 0.95 (`crates/codegraph-mcp-tools/src/graph_tool_executor.rs`, lines 345–457). `execute_agentic_workflow` has Ca 0, Ce 33, instability 1.0 (`crates/codegraph-mcp-server/src/official_server.rs`, lines 559–901). The latter’s zero incoming count should not be read as proof that nothing uses it: the graph does not represent every framework or macro-dispatched call path, so this is a useful metric signal but an incomplete call map.

**Hub nodes.** The whole-codebase hub ranking includes several external dependency manifests at the very top—`web-sys` has degree 1,737 and `windows` has degree 701—so they are not MCP architectural hubs. Among the project’s MCP-relevant package hubs, `codegraph-mcp` has degree 286 (`crates/codegraph-mcp/Cargo.toml`, line 1), `codegraph-mcp-core` 147, `codegraph-mcp-rig` 139, `codegraph-mcp-server` 107, and `codegraph-mcp-daemon` 73 (each manifest at line 1). The broader project hubs include `codegraph-vector` at 590 and `codegraph-core` at 492. These rankings reinforce that shared graph/core functionality is central, while the server package connects many internal pieces. The hub tool’s degree includes edge types such as exports, enables, and dependency links, so interpret it as graph centrality rather than pure call frequency.

**Cycles and follow-up.** The import-cycle detector returned a result involving `nix::sys::signal` imports in `crates/codegraph-mcp-core/src/process.rs` (line 4) and `crates/codegraph-mcp-server/src/bin/codegraph.rs` (line 2540). Because those are import-symbol nodes rather than a clearly demonstrated crate/module dependency loop, this result alone does not establish an architectural cycle. The call-cycle detector also returned cycles, including recursive helper patterns; the results do not isolate a confirmed MCP-layer feedback loop. The graph therefore raises a cycle-analysis flag, but it does not support claiming that the transport, server, tools, core, and daemon layers depend on one another cyclically.

The attempted coupling calculation for the `codegraph-mcp` package did not return a metric, so its degree-286 hub ranking should not be conflated with measured Ca/Ce. A focused follow-up on its package dependencies and on the `ai-enhanced`/feature-gated call paths would clarify how its centrality and the agent workflow’s zero incoming-call count relate to the actual composition.
<!-- balanced-refresh:20261006_151920_418410:case:6:end -->

### Refreshed case 7: agentic_architecture (api_surface)

> What is the public API surface of the GraphToolExecutor?

`OK` · 587.500 seconds · 18 tool calls · 25 structured locations

<!-- balanced-refresh:20261006_151920_418410:case:7:begin -->
`GraphToolExecutor` is a public, opaque executor type: its fields are private, and its API consists of constructors, tool dispatch, cache controls, and tool-schema/name listing. The central behavior is `execute`; changing its contract or the executor type is riskiest because the Rig tool layer forwards calls through it and the Rig/server layers use the type, though the graph’s coupling counts understate those uses.

**Export and type.** The type is declared at `crates/codegraph-mcp-tools/src/graph_tool_executor.rs:51` and its module and contents are publicly re-exported from `crates/codegraph-mcp-tools/src/lib.rs:4,7`. The `codegraph-mcp` crate also re-exports `GraphToolExecutor` at `crates/codegraph-mcp/src/lib.rs:27` (the indexed result marks that re-export as gated by the `embeddings` feature). Its fields are private, so callers use methods rather than accessing its internals.

**Construction and execution.** The public constructors are `new` (`graph_tool_executor.rs:134`), `with_context_window` (`:147`), `with_limits` (`:170`), and `with_cache` (`:218`). They form a configuration chain: `new` uses the configured context window, while the explicit options delegate to `with_limits`. `GraphToolFactory::new` accepts `Arc<GraphToolExecutor>` and wraps it in `CountingExecutor` (`crates/codegraph-mcp-rig/src/tools/factory.rs:16–20`); the Rig agent builder registers the resulting graph tools in its provider-specific build paths (`crates/codegraph-mcp-rig/src/agent/builder.rs:258–284, 316–340, 343–369`). A test constructs the executor directly at `crates/codegraph-mcp-rig/src/agent/react.rs:278–369`. The server workflow also appears as a caller/use site in the graph at `crates/codegraph-mcp-server/src/official_server.rs:559–901`.

The main operational entry point is `execute(&self, tool_name, parameters)` at `graph_tool_executor.rs:345`. It validates the tool name and dispatches to the corresponding implementation. In the Rig layer, `CountingExecutor::execute` forwards calls to the underlying executor; that adapter is in `crates/codegraph-mcp-rig/src/tools/counting_executor.rs:97–187`. This makes `execute` the most behavior-sensitive method to change: its contract sits between the executor and all registered Rig tool adapters.

**Supporting public methods.** `cache_stats` (`graph_tool_executor.rs:236`) returns cache statistics and is forwarded by `RigExecutor::cache_stats` (`crates/codegraph-mcp-rig/src/agent/executor.rs:267–269`). `clear_cache` (`graph_tool_executor.rs:246`) clears the cache and resets its statistics; no caller was established in the indexed results. `get_tool_schemas` (`:750`) and `get_tool_names` (`:755`) expose registration metadata; tests call them at `:844–855`, and the test asserts there are eight schemas/names.

**Dependency and change risk.** The broadest concrete integration is the `GraphToolExecutor` type: it is imported in the Rig factory, counting adapter, builder, and agent executor, and also in the server (`crates/codegraph-mcp-rig/src/tools/factory.rs:6`, `counting_executor.rs:6`, `agent/builder.rs:25`, `agent/executor.rs:10`, and `crates/codegraph-mcp-server/src/official_server.rs:39`). Within its methods, prioritize compatibility of `execute`; constructor changes would also affect code that builds and passes the executor into those layers. The coupling tool reports `GraphToolExecutor` with Ca=0 and Ce=0, and `execute` with Ca=1, Ce=19, instability 0.95. Those measurements conflict with the visible imports and wrapper flow, so they should not be read as a reliable ranking of actual dependents; direct caller coverage for some methods, especially `clear_cache`, remains unestablished.
<!-- balanced-refresh:20261006_151920_418410:case:7:end -->

### Refreshed case 8: agentic_quality (default)

> Find the highest complexity hotspots in the codebase. Which functions have the highest risk scores?

`OK` · 15.276 seconds · 1 tool calls · 20 structured locations

<!-- balanced-refresh:20261006_151920_418410:case:8:begin -->
The highest reported risk score is **`reconcile_project` at 1,404**, followed by Rust’s parser `walk` at **496** and `scan_and_emit` at **440**. `walk` has the most incoming dependents (afferent coupling **15**); `reconcile_project` is also a high-risk change point, with **12**. The ranking and values below are the hotspot analysis results; locations show each function’s start–end lines.

| Rank | Function | File and lines | Complexity | Risk score | Afferent / efferent coupling |
|---:|---|---|---:|---:|---:|
| 1 | `reconcile_project` | `./crates/codegraph-mcp/src/indexer.rs:632–2221` | 108 | 1,404 | 12 / 211 |
| 2 | `walk` | `./crates/codegraph-parser/src/languages/rust.rs:109–513` | 31 | 496 | 15 / 94 |
| 3 | `scan_and_emit` | `./crates/codegraph-core/src/watch/mod.rs:246–478` | 44 | 440 | 9 / 0 |
| 4 | `walk` | `./crates/codegraph-parser/src/languages/java.rs:109–457` | 41 | 246 | 5 / 74 |
| 5 | `extract_symbols` | `./crates/codegraph-core/src/watch/mod.rs:1001–1134` | 33 | 231 | 6 / 19 |
| 6 | `new` | `./crates/codegraph-vector/src/embeddings/generator.rs:301–483` | 22 | 220 | 9 / 4 |
| 7 | `enrich_async` | `./crates/codegraph-mcp/src/analyzers/lsp.rs:683–1018` | 41 | 205 | 4 / 62 |
| 8 | `walk` | `./crates/codegraph-parser/src/languages/ruby.rs:122–394` | 33 | 198 | 5 / 44 |
| 9 | `walk` | `./crates/codegraph-parser/src/languages/php.rs:130–428` | 32 | 192 | 5 / 61 |
| 10 | `walk` | `./crates/codegraph-parser/src/languages/swift.rs:108–387` | 30 | 180 | 5 / 55 |
| 11 | `walk` | `./crates/codegraph-parser/src/languages/cpp.rs:105–394` | 29 | 174 | 5 / 55 |
| 12 | `walk` | `./crates/codegraph-parser/src/languages/csharp.rs:109–394` | 28 | 168 | 5 / 54 |
| 13 | `extract_imports` | `./crates/codegraph-core/src/watch/mod.rs:1162–1239` | 22 | 154 | 6 / 11 |
| 14 | `collect_source_files_with_config` | `./crates/codegraph-parser/src/file_collect.rs:29–164` | 18 | 144 | 7 / 25 |
| 15 | `process_path_event` | `./crates/codegraph-core/src/watch/mod.rs:534–727` | 35 | 140 | 3 / 33 |
| 16 | `apply_env_overrides` | `./crates/codegraph-core/src/config_manager.rs:681–843` | 46 | 138 | 2 / 3 |
| 17 | `walk` | `./crates/codegraph-parser/src/languages/go.rs:104–296` | 23 | 138 | 5 / 38 |
| 18 | `strip_comments_c_like` | `./crates/codegraph-core/src/watch/mod.rs:887–935` | 16 | 112 | 6 / 1 |
| 19 | `syntax_boundaries` | `./crates/codegraph-parser/src/chunk_boundaries.rs:13–60` | 18 | 108 | 5 / 4 |
| 20 | `start` | `./crates/codegraph-mcp/src/analyzers/lsp.rs:252–492` | 17 | 102 | 5 / 37 |

For change-impact risk, the strongest signal here is incoming coupling: `walk` in the Rust parser has the most dependents (15), followed by `reconcile_project` (12). `scan_and_emit` and the embeddings `new` function each have 9 dependents; `collect_source_files_with_config` has 7. The parser `walk` implementations recur across language files, so several parser hotspots are worth considering together. The coupling figures indicate dependency counts, not a direct measure of how difficult a change would be.
<!-- balanced-refresh:20261006_151920_418410:case:8:end -->
