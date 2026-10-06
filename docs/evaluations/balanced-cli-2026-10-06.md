# Balanced indexing: CLI agent evaluation, 2026-10-06

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
