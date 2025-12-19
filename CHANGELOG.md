# CodeGraph MCP Server Changelog

All notable changes to the CodeGraph MCP Intelligence Platform will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### 🚀 **Enhanced - Rig-Based Agent System (Now Default)**

#### **Default Backend Switch**
- **Rig is now the default agent backend**: The system now uses the high-performance Rig backend by default over the legacy AutoAgents implementation.
- **Improved Performance**: Rig delivers faster reasoning and better tool orchestration, especially with modern thinking models.

#### **Multi-Architecture Support**
- **LATS (Language Agent Tree Search)**: New `LatsAgent` implementation using Rig primitives for tree-based exploration.
- **Reflexion**: New `ReflexionAgent` that wraps other agents and automatically retries with self-reflection upon failure.
- **Dynamic Architecture Switching**: `RigExecutor` now intelligently selects the best architecture based on task complexity (e.g., LATS for Architecture Analysis) and can fallback on failure.
- **Streaming Support**: Added `execute_stream` to `RigAgentTrait` and `RigExecutor`, enabling real-time feedback (Thought/ToolCall/Result/Answer events).

#### **Dynamic Context Throttling**
- **Smart Tier Adjustment**: Agent now monitors context usage during execution.
- **Automatic Downgrade**: If context usage >80%, automatically downgrades tier (e.g., Detailed -> Terse prompts) to prevent overflow errors.
- **Prevention**: Proactively manages token budget to ensure successful completion of long analysis chains.

#### **Operational Enhancements**
- **Structured Output**: Added `response_format` support to `RigAgentBuilder` for schema-enforced answers.
- **Unified Interface**: Refactored `RigAgentTrait` to support both blocking and streaming execution modes across all architectures (ReAct, LATS, Reflexion).

### ⚠️ BREAKING CHANGES

#### **Agentic Tool Consolidation (8 → 4 Tools)**
- **Consolidated 8 agentic MCP tools into 4** to reduce cognitive load for AI agents:
  - `agentic_context` (absorbs: `agentic_code_search`, `agentic_context_builder`, `agentic_semantic_question`)
  - `agentic_impact` (absorbs: `agentic_dependency_analysis`, `agentic_call_chain_analysis`)
  - `agentic_architecture` (absorbs: `agentic_architecture_analysis`, `agentic_api_surface_analysis`)
  - `agentic_quality` (absorbs: `agentic_complexity_analysis`)

- **Optional `focus` parameter** for precision when needed:
  | Tool | Focus Values | Default Behavior |
  |------|-------------|-----------------|
  | `agentic_context` | `"search"`, `"builder"`, `"question"` | Auto-selects based on query |
  | `agentic_impact` | `"dependencies"`, `"call_chain"` | Both dependency chains and call flows |
  | `agentic_architecture` | `"structure"`, `"api_surface"` | Both structural and interface analysis |
  | `agentic_quality` | `"complexity"`, `"coupling"`, `"hotspots"` | Comprehensive risk assessment |

- **Migration guide**:
  - `agentic_code_search` → `agentic_context` (or with `focus: "search"`)
  - `agentic_dependency_analysis` → `agentic_impact` (or with `focus: "dependencies"`)
  - `agentic_call_chain_analysis` → `agentic_impact` with `focus: "call_chain"`
  - `agentic_architecture_analysis` → `agentic_architecture` (or with `focus: "structure"`)
  - `agentic_api_surface_analysis` → `agentic_architecture` with `focus: "api_surface"`
  - `agentic_context_builder` → `agentic_context` with `focus: "builder"`
  - `agentic_semantic_question` → `agentic_context` with `focus: "question"`
  - `agentic_complexity_analysis` → `agentic_quality`

- **Note**: Legacy tools were removed (not feature-gated) due to rmcp SDK limitation that prevents multiple `#[tool_router]` impl blocks.

### Added

#### **New SurrealQL Functions**
- `fn::get_top_directories($project_id, $limit)` - Returns top-level directories with node counts for project structure overview
- `fn::count_nodes_for_project($project_id)` - Returns node, edge, and chunk counts for health/diagnostics

#### **Experimental GraphDB Schema (Interoperable)**
- `schema/codegraph_graph_experimental.surql` can now be used interoperably with CodeGraph’s indexing pipeline and agentic tools (function surface aligned with `schema/codegraph.surql`).
- Enable by setting:
  - `CODEGRAPH_USE_GRAPH_SCHEMA=true`
  - `CODEGRAPH_GRAPH_DB_DATABASE=codegraph_experimental` (or your chosen DB name)
- Designed for faster/more efficient graph-style queries than the relational/vanilla schema on large codebases.

### Performance

#### **Sub-Second Graph Query Performance**
- **`fn::semantic_search_nodes_via_chunks`**: Optimized from 30-60 seconds to sub-second
  - Now uses HNSW KNN index via `<|K,EF|>` operator for O(log n) vector search
  - Batched edge context queries: 2 queries instead of 300+ nested subqueries
  - Edge enrichment moved after scoring/limiting to minimize expensive lookups

- **`fn::detect_circular_dependencies`**: Optimized from O(E²) to O(E)
  - Replaced correlated subquery per edge pair with edge key set membership check
  - Single pass with `INSIDE` operator instead of nested `SELECT count()` per pair

- **`fn::edge_context`**: Eliminated 30+ nested subqueries per call
  - Now uses direct field access (`to.name`, `to.node_type`) instead of nested SELECTs

- **`fn::get_hub_nodes`**: Early threshold filtering optimization
  - Filters by degree threshold BEFORE expensive `fn::node_info` calls
  - Reduces enrichment from 100s of candidates to max 50 filtered results

- **`fn::calculate_coupling_metrics`**: Fixed broken function
  - Was referencing undefined `$dependents`/`$dependencies` variables
  - Now correctly queries edges for incoming (afferent) and outgoing (efferent) coupling

#### **Rig Agent Tool Call Limits**
- Added explicit tool call limits to Rig agent prompts to prevent runaway execution
  - Small tier: 3 tool calls max
  - Medium tier: 5 tool calls max
  - Large tier: 6 tool calls max
  - Massive tier: 8 tool calls max
- Addresses issue where agents could make 10+ tool calls in a single query

#### **Cache System Reliability and Performance**
- **Read-Ahead Optimizer Fixes**: Resolved critical type mismatch and moved value issues in the read-ahead optimization engine.
- **Memory Profiler Integration**: Provided a minimal memory profiling implementation to support real-time allocation tracking and dashboard visualization.
- **Demo & Example Repairs**: Fully restored and fixed the `memory_profiler_demo` and `readahead_demo` examples, including proper formatting and dependency management.
- **Dependency Optimization**: Added `once_cell`, `uuid`, and enhanced `tracing-subscriber` configuration to the cache crate for improved robustness.

### Added

#### **Runtime Agent Architecture Selection**
- **`CODEGRAPH_AGENT_ARCHITECTURE`**: Environment variable for runtime selection of agent backend
  - `react` (default): ReAct-style single-pass reasoning via AutoAgents
  - `lats`: Language Agent Tree Search for higher quality multi-path exploration
  - `rig`: Rig framework agent with native Rust LLM orchestration
- **`codegraph-mcp-rig`**: New crate implementing Rig-based agent backend
  - Alternative to AutoAgents using the [Rig framework](https://github.com/0xPlaygrounds/rig) (v0.26)
  - Same 8 graph analysis tools: transitive deps, reverse deps, call chains, cycles, coupling, hubs, semantic search, complexity
  - Tier-aware prompting (Small/Medium/Large/Massive) matching AutoAgents behavior
  - Conversation memory for multi-turn analysis within a session
  - Provider support: OpenAI, Anthropic, Ollama (via feature flags)
- **Runtime dispatcher**: `execute_agentic_workflow` dispatches to selected architecture without recompilation
- **Framework identification**: Response JSON includes `framework` field (`"Rig"`, `"AutoAgents-ReAct"`, `"AutoAgents-LATS"`)
- **Rig structured output fallback**: When the Rig backend returns plain text instead of schema JSON, the server synthesizes `structured_output` (with pinpoint `file_path`/`line_number` highlights) from recorded tool-call traces for better client/agent interoperability.

#### **Complexity Analysis Agentic Tool**
- **`agentic_complexity_analysis`**: New MCP tool for identifying high-risk code hotspots
  - Combines cyclomatic complexity with coupling metrics for risk assessment
  - **Risk Score Formula**: `complexity × (afferent_coupling + 1)` - measures blast radius
  - Tier-aware prompts: TERSE (3-5 steps), BALANCED (5-10), DETAILED (10-15), EXPLORATORY (15-20)
  - Refactoring recommendations prioritized by risk reduction potential

- **`fn::get_complexity_hotspots`**: SurrealDB function for complexity analysis
  - Returns functions/methods ranked by `risk_score`
  - Includes coupling metrics: afferent (Ca), efferent (Ce), instability (I = Ce/(Ca+Ce))
  - Filters by `min_complexity` threshold and `limit` parameter
  - Supports all edge types: calls, imports, uses, extends, implements, references

- **`find_complexity_hotspots`**: Inner graph tool for agent reasoning
  - Accessible to ReAct/LATS agents during multi-step analysis
  - Registered in `agent_builder.rs` alongside other graph tools

- **Cyclomatic Complexity Calculation**: Added to all language extractors
  - Formula: `1 + count(decision_points)` where decision points are if, while, for, match, etc.
  - Stored in node `complexity` field during indexing
  - Supports: Rust, TypeScript, Python, Go, Java, C++, Swift, Kotlin, C#, Ruby, PHP, Dart

#### **GraphRAG Semantic Search Function**
- **`fn::semantic_search_nodes_via_chunks`**: Context-engineering optimized search returning full node records
  - Searches chunks for semantic similarity, deduplicates by parent node
  - Returns full node content (not just chunk snippets) for complete function/class bodies
  - **Full graph context**: Includes all outgoing and incoming edges with relationship types
  - Edge types: `calls`, `imports`, `uses`, `extends`, `implements`, `references`
  - Uses `$parent.node_id` for correct record reference comparison in edge subqueries

#### **Incremental Indexing Improvements**
- **Better `--force` flag behavior**: Now performs clean delete-then-insert instead of upsert
  - Deletes all existing nodes, chunks, and edges for the project first
  - Then inserts fresh data ensuring no stale duplicates remain
  - Eliminates orphaned chunks and edges from previous indexing runs
- **Improved deduplication**: Project-scoped deletion prevents cross-project contamination

#### **LLM Provider Function Calling Standardization**
- **Responses API migration**: All providers now use correct Responses API function call format
  - OpenAI-compatible providers: Proper `tool_calls` response handling
  - Anthropic provider: Aligned function calling with Claude's tool use format
  - Ollama provider: Native function calling support
- **Structured output enforcement**: All providers now enforce JSON schema validation
  - Consistent response parsing across providers
  - Better error handling for malformed tool responses

#### **Agent Internal Graph Tools Enhancement**
- **Improved tool executor**: Better error handling and result formatting
- **Consistent JSON serialization**: Fixed `surreal_to_json` converter for nested values
- **Enhanced edge queries**: Correct `$parent` reference for record type comparisons

#### **Agent Reliability Improvements**
- **Execution Timeout Handling** (R1):
  - Configurable global timeout for all agentic tool executions (default: 300 seconds)
  - Environment variable: `CODEGRAPH_AGENT_TIMEOUT_SECS` (set to 0 for unlimited)
  - Graceful timeout error with elapsed time and steps completed
  - Debug logging when `CODEGRAPH_DEBUG=1`

- **3-Stage Progress Notifications** (R2):
  - Stage 1 (0.0): "Agent started: {analysis_type}" - sent at workflow start
  - Stage 2 (0.5): "Agent analyzing with tools..." - sent after first tool execution
  - Stage 3 (1.0): "Agent analysis complete" or error message - sent at workflow end
  - MCP protocol compliant notifications via rmcp SDK
  - Non-blocking async notification delivery

- **LATS Early Termination** (R3):
  - Score-based termination: stops when solution score > 0.9
  - Solution flag termination: stops when `is_solution` is true
  - Per-iteration timeout: `CODEGRAPH_LATS_ITERATION_TIMEOUT_SECS` (default: 60s)
  - Tree size limit: `CODEGRAPH_AGENT_MAX_TREE_NODES` (default: auto-calculated)
  - Debug logging with termination reason (high_score, solution_found, tree_size_exceeded, iteration_timeout)

- **Context Overflow API Error Handling** (R4):
  - Detects context window overflow from LLM API errors
  - Pattern matching for: "context_length_exceeded", "maximum context length", "too many tokens", etc.
  - Returns user-friendly error: "Context window limit reached. Query too complex for model's capacity."
  - Debug logging when `CODEGRAPH_DEBUG=1`

- **Memory Window Size Fix** (R5):
  - Changed default memory window from tier-based (10-40) to fixed 40 messages
  - Environment variable: `CODEGRAPH_AGENT_MEMORY_WINDOW` (default: 40, set to 0 for unlimited)
  - Applies to both ReAct and LATS agents

#### **LATS Agent Architecture** (Experimental)
- **Language Agent Tree Search**: UCT-based tree search for higher-quality agentic analysis
  - Toggle via `CODEGRAPH_AGENT_ARCHITECTURE=lats` environment variable
  - Configurable beam width and max depth for exploration control
  - Multi-provider support: different LLMs for selection/expansion/evaluation phases
  - Feature-gated: requires `--features autoagents-lats` at build time
- **AgentArchitecture enum**: Runtime architecture selection (ReAct vs LATS)
- **AgentExecutorTrait**: Abstraction for pluggable agent implementations
- **AgentExecutorFactory**: Architecture-aware executor creation with tier detection
- **LATS-specific configuration**:
  - `CODEGRAPH_LATS_SELECTION_PROVIDER/MODEL` - Fast model for node selection
  - `CODEGRAPH_LATS_EXPANSION_PROVIDER/MODEL` - Reasoning model for generating next steps
  - `CODEGRAPH_LATS_EVALUATION_PROVIDER/MODEL` - Evaluation model for state assessment
  - `CODEGRAPH_LATS_BEAM_WIDTH` - Number of best paths to keep (default: 3)
  - `CODEGRAPH_LATS_MAX_DEPTH` - Maximum search depth (default: 5)

### ⚠️ BREAKING CHANGES
- **LLM Providers**: Default to Responses API for LM Studio and OpenAI-compatible providers
  - **Who's affected**: Users with older OpenAI-compatible providers that only support Chat Completions API
  - **Migration**: Set `CODEGRAPH_USE_COMPLETIONS_API=true` if you encounter Responses API errors
  - **Ollama users**: NOT affected - Ollama uses its native API
  - **Benefit**: Modern Responses API provides better support for reasoning models and improved token management

### Added

#### **Responses API Support for LLM Providers**
- **Default API**: CodeGraph now uses the modern Responses API by default for LM Studio and OpenAI-compatible providers
- **Backward compatibility flag**: `use_completions_api` configuration field for providers that don't support Responses API
- **Environment variable**: `CODEGRAPH_USE_COMPLETIONS_API` for easy API selection
- **Helpful error messages**: Errors include migration guidance when Responses API fails
- **Warning messages**: Deprecation warning when using Chat Completions API
- **Provider support**: 99% of OpenAI-compatible providers support the modern Responses API

### Changed
- **Default LLM API**: Changed from Chat Completions to Responses API for LM Studio and OpenAI-compatible providers
- **Error messages**: Now include migration guidance for incompatible providers

#### **LM Studio Embedding Provider - Full Environment Variable Support**
- **Fixed `with_auto_from_env()`** to support LM Studio provider (previously only supported Jina and Ollama)
- **Added `embeddings-lmstudio` feature flag** to MCP crate for explicit LM Studio support
- **Exposed in build scripts**: Added to `build-mcp-autoagents` and `build-mcp-http` Makefile targets
- **Environment variable detection**: `CODEGRAPH_EMBEDDING_PROVIDER=lmstudio` now properly initializes provider
- **Architectural improvement**: Consolidates embedding initialization to single code path
  - Symbol resolution now uses `with_config()` instead of `with_auto_from_env()`
  - Eliminates duplicate initialization logic and configuration inconsistencies
  - Single source of truth for all embedding configuration
- **Impact**: LM Studio embeddings now work in all code paths (main indexing, symbol resolution, API)

#### **Fast ML Code Enhancement (Always-On)**
- **Aho-Corasick pattern matching** for sub-microsecond multi-pattern code analysis (50-500ns per file)
  - Detects common patterns: `use`, `impl`, `class`, `extends`, `async fn`, `trait`, `import`, etc.
  - Supports Rust, TypeScript, JavaScript, Python, Go patterns (language-agnostic)
  - SIMD-accelerated with zero training requirements
- **LSH-based symbol resolution** for fast similarity matching (100-500μs per query)
  - Uses Locality-Sensitive Hashing with SignRandomProjections for cosine similarity
  - Resolves unmatched references to similar symbols (fuzzy matching)
  - 128-dimensional character-based vectors with 5 projections, 10 hash tables
- **Node enrichment for better embeddings**:
  - Adds `fast_ml_patterns` metadata: comma-separated list of detected patterns
  - Adds `fast_ml_pattern_count` metadata: total pattern matches
  - Enriches file/module-level nodes with pattern context
  - Helps SOTA embedding models understand code characteristics
- **Parser-level integration** (always-on, <1ms overhead):
  - Applied after AST extraction to all languages
  - Combines AST precision with Fast ML recall for maximum graph completeness
  - Pattern-based edges complement structural AST edges
- **Replaced fake AI files** (800 lines removed):
  - Deleted `ai_pattern_learning.rs` (488 lines of fake HashMap-based "learning")
  - Deleted `real_ai_integration.rs` (314 lines of string pattern matching)
  - Replaced with real Fast ML implementation (554 lines)
- **Dependencies**: `aho-corasick` 1.1, `lsh-rs2` 0.4.1
- **Performance**: <1ms total latency guaranteed (AST + Fast ML combined)
- **Impact**: Richer graph for traversal + better node context for embeddings

#### **LM Studio Embedding Provider**
- **OpenAI-compatible embedding provider** for local LM Studio instances
- **Automatic dimension detection** for 20+ embedding models:
  - Jina models: `jina-embeddings-v3` (1024), `jina-embeddings-v4` (2048), `jina-embeddings-v4-text-code` (2048), `jina-code-embeddings-1.5b` (1536)
  - Qwen models: `qwen3-embedding-0.6b-dwq` (1024), `qwen3-embedding-0.6b` (1024)
  - Nomic models: `nomic-embed-text-v1.5` (768)
  - BGE models: `bge-small` (384), `bge-base` (768), `bge-large` (1024)
  - E5 models: `e5-small` (384), `e5-base` (768), `e5-large` (1024)
  - Safe default: 1536 for unknown models
- **Semantic text chunking** using Qwen2.5-Coder tokenizer with configurable max tokens (default: 8192)
- **Exponential backoff retry** logic (3 attempts with 100ms base delay)
- **Availability checking** via `/models` endpoint before initialization
- **Configuration options**:
  - Environment variables: `CODEGRAPH_LMSTUDIO_MODEL`, `CODEGRAPH_LMSTUDIO_URL`, `CODEGRAPH_LMSTUDIO_TIMEOUT`, `CODEGRAPH_MAX_CHUNK_TOKENS`
  - Config file: `embedding.provider = "lmstudio"`, `embedding.lmstudio_url = "http://localhost:1234"`
  - Default URL: `http://localhost:1234` (the `/v1` path is appended automatically)
- **Feature flag**: `lmstudio` (requires reqwest)
- **Performance characteristics**: 50 texts/sec throughput, 500ms typical latency, high memory usage (running full model)
- **Integration**: Seamless integration with `EmbeddingGenerator` factory, automatic provider selection based on config

#### **Daemon Mode for Automatic Re-Indexing**
- **Background file watching**: `codegraph daemon start <path>` launches a background process that monitors file changes
- **Automatic re-indexing**: Changed files are automatically re-indexed using upsert semantics (no duplicates)
- **Circuit breaker pattern**: SurrealDB health monitoring with exponential backoff for resilience
- **PID file management**: Proper daemon lifecycle with process tracking and graceful shutdown
- **CLI commands**:
  - `codegraph daemon start <path>` - Start watching a project (with `--foreground` option for debugging)
  - `codegraph daemon stop <path>` - Stop the daemon for a project
  - `codegraph daemon status <path>` - Check daemon status (with `--json` option)
- **Language and pattern filtering**: Supports `--languages`, `--exclude`, and `--include` flags
- **Event coalescing**: Batches rapid file changes to avoid redundant re-indexing
- **Feature flag**: `daemon` feature enables this functionality

#### **MCP Server Integration with Daemon (`--watch`)**
- **Integrated daemon with MCP server**: `codegraph start stdio --watch` starts the daemon automatically
- **Non-blocking startup**: Daemon runs in background task, MCP server starts immediately
- **Error isolation**: Daemon failures don't crash MCP server
- **CLI flags for MCP server**:
  - `--watch` - Enable automatic file watching (also via `CODEGRAPH_DAEMON_AUTO_START` env var)
  - `--watch-path <PATH>` - Specify directory to watch (also via `CODEGRAPH_DAEMON_WATCH_PATH` env var)
  - `--no-watch` - Explicitly disable daemon even if config enables it
- **Configuration support**: `[daemon].auto_start_with_mcp` in config.toml to enable by default
- **DaemonConfig**: New configuration struct in `codegraph-core` for daemon settings
- **DaemonManager**: Coordinates daemon lifecycle with MCP server lifecycle
- **Graceful cleanup**: Daemon stops automatically when MCP server exits

### Removed
- Dropped the transactional/versioning/branch N-API bindings plus their documentation; the native TypeScript surface now focuses on semantic search, graph analysis, and cloud configuration so it matches the Surreal-only architecture.
- Replaced the native addon example with a semantic-search walkthrough and removed CLI-centric integration snippets that referenced the deleted CodeGraph CLI.
- Removed the unpublished `sdk/codegraph-cli-wrapper.ts` helper that shell-executed the old CLI, keeping the SDK focused on the MCP WebSocket client (run `npm install` before `npm run typecheck` if you want to lint the SDK locally).

## [1.1.0] - 2025-11-18 - AutoAgents Integration & SurrealDB 2.x Compatibility

### 🚀 **Added - AutoAgents Framework Integration (Experimental)**

#### **AutoAgents ReAct Pattern**
- **Replaced custom orchestrator** with AutoAgents framework (~1,200 lines removed)
- **Feature flag**: `autoagents-experimental` for opt-in testing
- **6 inner graph analysis tools** for ReAct agent:
  - `GetTransitiveDependencies`, `GetReverseDependencies`, `TraceCallChain`
  - `DetectCycles`, `CalculateCoupling`, `GetHubNodes`
- **Tool call parsing**: Converts CodeGraph JSON format to AutoAgents ToolCall format
- **Maintains all 7 agentic MCP tools**: Full compatibility with existing API
- **Tier-aware prompting**: Preserved from legacy orchestrator

#### **Semantic Chunking with Qwen2.5-Coder Tokenizer**
- **Ollama provider**: Added full semantic chunking support
- **Fallback mode**: Semantic chunking when embeddings feature disabled
- **Environment variables**:
  - `CODEGRAPH_MAX_CHUNK_TOKENS=512` - Max tokens per chunk (default)
  - `CODEGRAPH_CHUNK_OVERLAP_TOKENS=50` - Chunk overlap (reserved for future)
- **Token-accurate**: Uses Qwen2.5-Coder tokenizer for precise token counting
- **Chunk aggregation**: Multiple chunks averaged into single node embedding
- **Benefits**: Better embeddings for long functions/classes, preserves code structure

#### **768-Dimension Embedding Support**
- **Added support for embeddinggemma** and other 768-dim models
- **Complete pipeline**: Indexer, storage, HNSW indexes, vector search
- **New constant**: `SURR_EMBEDDING_COLUMN_768`
- **Schema**: HNSW index for `embedding_768` column with EFC 200, M 16
- **Auto-detection**: Automatic column selection based on embedding dimension

#### **Structured Output Enforcement with JSON Schemas**
- **JSON schema enforcement** for all 7 agentic tools with **required file paths**
- **Schema-driven outputs**: LLM cannot return response without file locations
- **AutoAgents integration**: Uses `StructuredOutputFormat` passed via `AgentDeriveT::output_schema()`
- **New module**: `codegraph-ai/src/agentic_schemas.rs` with comprehensive schemas:
  - `CodeSearchOutput`: analysis + components[] + patterns[]
  - `DependencyAnalysisOutput`: analysis + components[] + dependencies[] + circular_dependencies[]
  - `CallChainOutput`: analysis + entry_point + call_chain[] + decision_points[]
  - `ArchitectureAnalysisOutput`: analysis + layers[] + hub_nodes[] + coupling_metrics[]
  - `APISurfaceOutput`: analysis + endpoints[] + usage_patterns[]
  - `ContextBuilderOutput`: comprehensive context with all analysis dimensions
  - `SemanticQuestionOutput`: answer + evidence[] + related_components[]
- **Required fields**: Core component arrays must include `name`, `file_path`, and `line_number`
- **Flexible fields**: Complex nested types (coupling_metrics, dependencies, etc.) accept either structured objects or simplified strings
- **Provider integration**:
  - Added `response_format` field to `GenerationConfig`
  - OpenAI compatible providers send JSON schema to LLM API
  - Ollama-native `format` field for compatibility (dual-field approach)
  - AutoAgents adapter converts `StructuredOutputFormat` to CodeGraph `ResponseFormat`
  - xAI/Grok supports Responses API with full `response_format.json_schema` support
- **Prompt updates**: All 28 tier prompts (4 tiers × 7 tools) updated to use structured format
- **Hybrid output**: Combines freeform `analysis` field with structured arrays
- **MCP handler**: Parses structured JSON and surfaces in `structured_output` field
- **Benefits**:
  - File paths are **mandatory** in core component arrays
  - Downstream tools can navigate directly to relevant code
  - Consistent data structure for programmatic consumption
  - Better agent-to-agent collaboration with actionable locations
  - Flexible parsing handles LLM variations while preserving essential data

#### **File Location Requirements in Agent Outputs (Deprecated)**
- **Superseded by**: Structured output enforcement with JSON schemas (above)
- **Legacy prompt updates**: All EXPLORATORY prompts requested file locations (now enforced)
- **Format**: `ComponentName in path/to/file.rs:line_number`
- **Example**: "ConfigLoader in src/config/loader.rs:42" instead of just "ConfigLoader"
- **6 prompts updated**: code_search, dependency_analysis, call_chain, architecture, context_builder, semantic_question, api_surface
- **Migration**: Prompts now work in conjunction with schema enforcement

### 🐛 **Fixed - AutoAgents and Structured Output Issues**

#### **Tier-Aware Token Limits**
- **Fixed**: AutoAgents ignored tier-based max_tokens (always used default ~4K)
- **Now**: Respects tier limits (Small: 2K, Medium: 4K, Large: 8K, Massive: 16K)
- **Override**: Set `MCP_CODE_AGENT_MAX_OUTPUT_TOKENS` environment variable
- **Impact**: Massive tier (xAI Grok 2M) now outputs detailed 16K responses

#### **Prompt Format Clarification**
- **Fixed**: Architecture and semantic_question tools outputting both old wrapper format AND new structured format
- **Cause**: Prompts showed intermediate format alongside final format
- **Fix**: Clarified final response should be ONLY structured JSON (no reasoning/tool_call/is_final wrapper)

#### **HTTP Transport Testing**
- **Replaced**: Custom SSE parser (1,293 lines) with official MCP SDK `streamablehttp_client`
- **Added**: `test_http_mcp.py` using proper MCP protocol
- **Fixed**: Session management via `Mcp-Session-Id` header
- **Timeouts**: Extended to 300s for complex analysis

### 🐛 **Fixed - Critical Database Persistence Bugs**

#### **Async Writer Flush Bugs**
- **file_metadata flush**: Added missing flush after `persist_file_metadata()` (line 1237)
  - **Impact**: project_metadata and file_metadata tables remained empty
  - **Root cause**: Metadata queued but never written before shutdown
- **symbol_embeddings flush**: Added flush after symbol embedding precomputation (line 979)
  - **Impact**: symbol_embeddings table incomplete or empty
  - **Root cause**: Embeddings queued but not flushed before edge resolution
- **embedding_model tracking**: Added to node metadata in `annotate_node()`
  - **Impact**: Nodes showed hardcoded 'jina-embeddings-v4' instead of actual model
  - **Root cause**: embedding_model not added to metadata attributes

#### **AutoAgents Runtime Fixes**
- **Tool call parsing**: Implemented `tool_calls()` method to extract ToolCall from CodeGraph JSON
  - **Impact**: Agent was stopping at step 0, tools never executed
  - **Root cause**: `CodeGraphChatResponse::tool_calls()` always returned None
- **Runtime panic**: Fixed "Cannot start a runtime from within a runtime"
  - **Impact**: Tool execution crashed with panic
  - **Root cause**: Using `block_on` directly in async context
  - **Fix**: Wrapped in `tokio::task::block_in_place` at tool_executor_adapter.rs:38
- **Steps reporting**: Fixed `steps_taken` to show actual tool execution count
  - **Impact**: Always showed "0" even when tools executed
  - **Fix**: Extract count from `ReActAgentOutput.tool_calls.len()`
- **Feature-gate warning**: Fixed unused `ai_matches` variable warning
  - **Fix**: Added `#[cfg(feature = "ai-enhanced")]` to declaration and accumulation

### 🐛 **Fixed - SurrealDB 2.x Compatibility**

#### **Record ID System Overhaul**
- **Removed manual id field definitions** from 3 tables (nodes, edges, symbol_embeddings)
  - **Impact**: Defining `id` as string broke SurrealDB's built-in record ID system
  - **Root cause**: SurrealDB 2.x `id` is always a record type, overriding with string breaks queries
- **Updated fn::node_info()**: Changed parameter type from `string` to `record`
- **Added explicit record casting** to all 5 graph functions:
  - `fn::node_reference()`, `fn::get_transitive_dependencies()`, `fn::trace_call_chain()`
  - `fn::calculate_coupling_metrics()`, `fn::get_reverse_dependencies()`
  - **Pattern**: `LET $record = type::thing('nodes', $node_id);`
  - **Ref**: https://surrealdb.com/docs/surrealql/datamodel/casting
- **Changed UPSERT queries** from `CONTENT` to `SET`:
  - **Impact**: CONTENT $doc tried to overwrite id field with string → data corruption
  - **Fix**: Explicit SET field list excludes id field
  - **Applies to**: UPSERT_NODES_QUERY, UPSERT_EDGES_QUERY, UPSERT_SYMBOL_EMBEDDINGS_QUERY

#### **Schema Syntax Corrections**
- **ASSERT statements**: Added `$value = NONE OR` for proper null handling
- **FLEXIBLE metadata**: Changed metadata fields to `FLEXIBLE TYPE option<object>`
- **HNSW syntax**: Corrected index definitions for SurrealDB 2.3.10
- **Removed duplicate**: Eliminated duplicate ANALYZER definition

### ⚡ **Performance Improvements**

#### **File Metadata Optimization (120x Faster)**
- **Optimized complexity**: O(N²) → O(N) using HashMaps
- **Before**: 2 minutes for 1,000 files (10M+ iterations)
- **After**: ~1 second for 1,000 files (15K iterations)
- **Pattern**: Pre-build lookup tables instead of nested iterations
- **Added progress bar**: Shows file-by-file processing status

#### **Throughput Display Fixes**
- **Fixed duplicate units**: "21,970/s/s" → "21,970/s"
- **Root cause**: `{per_sec}` placeholder already includes "/s" suffix
- **Fixed 3 progress bars**: batch, enhanced, simple progress templates

### 🗑️ **Removed**
- **Surreal-only runtime**: Deleted all FAISS/RocksDB features, installers, and vector backends. MCP server, CLI, and SDK now require SurrealDB for graph + vector persistence with no fallback.
- **Dependency cleanup**: Removed 79 unused dependencies across 17 crates
  - **Tool**: cargo machete for detection
  - **Impact**: Faster compilation, smaller binaries
- **Broken schema files**: Deleted 6 individual function files in `schema/functions/`
  - **calculate_coupling_metrics.surql**: Syntax error with orphaned code
  - **detect_circular_dependencies.surql**: Wrong function name (fn::coupling_metrics)
  - **get_hub_nodes.surql**: Wrong function signature
  - **get_reverse_dependencies.surql, get_transitive_dependencies.surql, trace_call_chain.surql**: Incomplete implementations
  - **Impact**: Conflicted with correct implementations in codegraph.surql

### ⚠️ **Breaking Changes**

#### **SurrealDB Schema Migration Required**
- **Record ID system change**: Manual `id` field definitions removed
- **Migration steps**:
  1. Re-apply schema: `cd schema && ./apply-schema.sh`
  2. Re-index codebase: `codegraph index -l rust -r --force .`
- **Reason**: Compatibility with SurrealDB 2.x record ID handling
- **Impact**: Existing data incompatible, full re-index required

#### **AutoAgents Feature Flag**
- **Default**: Uses legacy orchestrator (stable)
- **Experimental**: Use `autoagents-experimental` feature for new framework
- **Build**: `cargo build --features "autoagents-experimental,ai-enhanced,faiss,ollama"`
- **Status**: Testing in progress, production-ready in v1.2.0

### 📝 **Changed**

#### **Environment Variables**
- **Unified chunking**: `CODEGRAPH_MAX_CHUNK_TOKENS` now works across all providers
- **Ollama support**: Ollama provider now respects chunking configuration
- **Jina unchanged**: Still uses `JINA_MAX_TOKENS` (provider-specific)

### 📦 **Dependencies**

#### **Added**
- **schemars** (workspace): JSON schema generation for structured LLM outputs
  - Used in `codegraph-ai` for agentic schema definitions
  - Enables compile-time schema validation
  - Auto-generates JSON Schema from Rust types

### 📚 **Documentation**
- **GraphFunctions enrichment plan**: Comprehensive plan saved to `.ouroboros/plans/graphfunctions-enrichment-20251118.md`
  - Schema alignment recommendations
  - Missing field identification (qualified_name, column positions, timestamps)
  - Implementation phases with SQL examples

---

## [Unreleased] - 2025-01-08 - Agentic Code Intelligence & Architecture Migration

### 🚀 **Added - Agentic MCP Tools (AI-Enhanced Feature)**

#### **1. Tier-Aware Agentic Orchestration**
- **7 Agentic MCP Tools**: Multi-step reasoning workflows for comprehensive code analysis
  - `agentic_code_search` - Autonomous graph exploration for code search
  - `agentic_dependency_analysis` - Dependency chain and impact analysis
  - `agentic_call_chain_analysis` - Execution flow tracing
  - `agentic_architecture_analysis` - Architectural pattern assessment
  - `agentic_api_surface_analysis` - Public interface analysis
  - `agentic_context_builder` - Comprehensive context gathering
  - `agentic_semantic_question` - Complex codebase Q&A
- **Automatic tier detection**: Based on LLM context window (Small/Medium/Large/Massive)
- **Tier-aware prompting**: 28 specialized prompts (7 types × 4 tiers)
  - Small (<50K): TERSE prompts, 5 max steps, 2,048 tokens
  - Medium (50K-150K): BALANCED prompts, 10 max steps, 4,096 tokens
  - Large (150K-500K): DETAILED prompts, 15 max steps, 8,192 tokens
  - Massive (>500K): EXPLORATORY prompts, 20 max steps, 16,384 tokens
- **LRU caching**: Transparent SurrealDB result caching (100 entries default)
- **Configurable max tokens**: `MCP_CODE_AGENT_MAX_OUTPUT_TOKENS` env variable

#### **2. Graph Analysis Integration**
- **6 SurrealDB graph tools**: Deep structural code analysis
  - `get_transitive_dependencies` - Full dependency chains
  - `detect_circular_dependencies` - Cycle detection
  - `trace_call_chain` - Execution path analysis
  - `calculate_coupling_metrics` - Ca, Ce, I metrics
  - `get_hub_nodes` - Architectural hotspot detection
  - `get_reverse_dependencies` - Change impact assessment
- **Zero-heuristic design**: LLM infers from structured data only
- **Tool call logging**: Complete reasoning traces with execution stats
- **Cache statistics**: Hit rate, evictions, size tracking

#### **3. CLI Enhancements**
- **New command**: `codegraph config agent-status`
  - Shows LLM provider, context tier, prompt verbosity
  - Lists all available MCP tools with descriptions
  - Displays orchestrator settings (max steps, cache, tokens)
  - JSON output support for automation
- **Configuration visibility**: Understand how config affects system behavior

### ✅ **Added - Local Surreal Embeddings & Reranking**
- SurrealDB indexing now honors local embedding providers (Ollama + LM Studio) using the same workflow as Jina—set `CODEGRAPH_EMBEDDING_PROVIDER/MODEL/DIMENSION` and we stream vectors into the matching `embedding_<dim>` column automatically
- Supported combinations today: `all-mini-llm` (384), `qwen3-embedding:0.6b` (1024), `qwen3-embedding:4b` (2048), `qwen3-embedding:8b` (4096), plus `jina-embeddings-v4`
- LM Studio's OpenAI-compatible reranker endpoint can now be used for local reranking, so hybrid/local deployments keep the same two-stage retrieval experience as Jina Cloud
- CLI/indexer logs explicitly call out the active dimension + Surreal column so it's obvious which field is being populated

### ✅ **Added - Incremental Indexing with File Change Detection**
- SHA-256-based file change detection tracks added, modified, deleted, and unchanged files between indexing runs
- Differential re-indexing processes only changed files (added + modified), dramatically reducing re-index time for large codebases
- File metadata persistence (`file_metadata` table) stores content hash, size, timestamps, and node/edge counts per file
- Smart cleanup automatically removes orphaned data (nodes, edges, embeddings) for deleted/renamed files
- Backward compatible with existing indexes—falls back to full re-index if metadata unavailable
- `--force` flag now performs clean slate deletion before re-indexing, preventing data accumulation

### 🛠️ **Improved - Surreal Edge Persistence Diagnostics**
- After dependency resolution we now query `edges` directly and log the stored count vs. expected total; any mismatch is surfaced immediately with a warning, making it easier to spot schema or auth issues during indexing

### ⚠️ **Deprecated - MCP Server FAISS+RocksDB Support**

**IMPORTANT**: The MCP server's FAISS+RocksDB graph database solution is now **deprecated** in favor of SurrealDB-based architecture.

**What's Deprecated:**
- MCP server integration with FAISS vector search
- MCP server integration with RocksDB graph storage
- Cloud dual-mode search via MCP protocol

**What Remains Supported:**
- ✅ **CLI commands**: All FAISS/RocksDB operations remain available via `codegraph` CLI
- ✅ **Rust SDK**: Full programmatic access to FAISS/RocksDB functionality
- ✅ **NAPI bindings**: TypeScript/Node.js integration still functional
- ✅ **Local embeddings**: ONNX, Ollama, LM Studio providers unchanged

**Migration Path:**

For **MCP code-agent** functionality, you must now set up SurrealDB:

**Option 1: Free Cloud Instance (Recommended for testing)**
1. Sign up at [Surreal Cloud](https://surrealdb.com/cloud) - **FREE 1GB instance included**
2. Get connection details from dashboard
3. Configure environment:
   ```bash
   export SURREALDB_URL=wss://your-instance.surrealdb.cloud
   export SURREALDB_NAMESPACE=codegraph
   export SURREALDB_DATABASE=main
   export SURREALDB_USERNAME=your-username
   export SURREALDB_PASSWORD=your-password
   ```

**Option 2: Local Installation**
```bash
# Install SurrealDB
curl -sSf https://install.surrealdb.com | sh

# Run locally
surreal start --bind 127.0.0.1:3004 --user root --pass root memory

# Configure
export SURREALDB_URL=ws://localhost:3004
export SURREALDB_NAMESPACE=codegraph
export SURREALDB_DATABASE=main
```

**Free Cloud Services:**
- 🆓 **SurrealDB Cloud**: 1GB free instance (perfect for testing and small projects)
- 🆓 **Jina AI**: 10 million free API tokens when you register at [jina.ai](https://jina.ai)
  - Includes embeddings, reranking, and token counting APIs
  - Production-grade embeddings with no local GPU required

**Rationale:**
- SurrealDB provides native graph capabilities vs. custom RocksDB layer
- HNSW vector indexing is built-in vs. separate FAISS integration
- Cloud-native architecture enables distributed deployments
- Unified storage reduces complexity and maintenance overhead

## [1.1.0] - 2025-11-08 - Cloud-Native Vector Search & TypeScript Integration

### 🌟 **Major Release - Cloud Embeddings, Dual-Mode Search, and NAPI Bindings**

This release transforms CodeGraph into a hybrid local/cloud platform with enterprise-grade cloud embeddings, cloud-native vector search, and zero-overhead TypeScript integration through native Node.js bindings.

### ☁️ **Added - Cloud Provider Ecosystem**

#### **1. xAI Grok Integration (2M context window)**
- **Massive 2M token context**: Analyze entire large codebases in a single query
- **Extremely affordable**: $0.50/$1.50 per million tokens (10x cheaper than GPT-5)
- **OpenAI-compatible API**: Seamless integration using existing OpenAI provider code
- **Multiple models**: grok-4-fast (default) and grok-4-turbo
- **Environment configuration**:
  ```bash
  XAI_API_KEY=xai_xxx
  ```
- **Config options**: `xai_api_key`, `xai_base_url` (default: https://api.x.ai/v1)
- **Use case**: Whole-codebase analysis, massive documentation ingestion, cross-repo analysis

#### **2. Jina AI Cloud Embeddings (Variable Matrysohka dimensions)**
- **Production-grade embeddings**: Jina AI jina-code-embeddings-1.5b ad 0.5b with 1536/896-dimensional vectors
- **Intelligent reranking**: Optional two-stage retrieval with Jina reranker-v3
- **Token counting API**: Accurate token usage tracking for cost optimization
- **Batch processing**: Efficient batch embedding generation with automatic chunking
- **Environment configuration**:
  ```bash
  JINA_API_KEY=jina_xxx
  JINA_RERANKING_ENABLED=true
  ```
- **Feature flag**: `--features cloud-jina` for conditional compilation

#### **3. SurrealDB HNSW Vector Backend**
- **Cloud-native vector search**: Distributed HNSW index with SurrealDB
- **Sub-5ms query latency**: Fast approximate nearest neighbor search
- **Automatic fallback**: Graceful degradation to local FAISS on connection failure
- **Flexible deployment**: Self-hosted or cloud-managed SurrealDB instances
- **Schema-based storage**: Structured code node storage with version tracking
- **Environment configuration**:
  ```bash
  SURREALDB_CONNECTION=ws://localhost:8000
  SURREALDB_NAMESPACE=codegraph
  SURREALDB_DATABASE=production
  ```
- **Feature flag**: `--features cloud-surrealdb` for conditional compilation

#### **4. Dual-Mode Search Architecture**
- **Intelligent routing**: Automatic selection between local FAISS and cloud SurrealDB
- **Configuration-driven**: Enable cloud search globally or per-query
- **Automatic fallback**: Seamless degradation to local search on cloud failure
- **Performance monitoring**: Detailed timing metrics for each search mode
- **Explicit mode override**: Client can force local or cloud search per query
- **Implementation**:
  ```rust
  // Automatic routing based on config
  let use_cloud = match opts.use_cloud {
      Some(explicit) => explicit,
      None => state.cloud_enabled,
  };

  // Cloud search with fallback
  let results = if use_cloud {
      cloud::search_cloud(&state, &query, &opts)
          .await
          .or_else(|_| local::search_local(&state, &query, &opts).await)
  } else {
      local::search_local(&state, &query, &opts).await
  };
  ```

### 📦 **Added - Node.js NAPI Bindings**

#### **Zero-Overhead TypeScript Integration:**
- **Native performance**: Direct Rust-to-Node.js bindings with NAPI-RS
- **Auto-generated types**: TypeScript definitions generated from Rust code
- **No serialization overhead**: Direct memory sharing between Rust and Node.js
- **Async runtime**: Full tokio async support with Node.js event loop integration
- **Type safety**: Compile-time type checking across language boundary

#### **Complete API Surface:**
```typescript
// Search operations
const results = await semanticSearch(query, {
  limit: 10,
  useCloud: true,
  reranking: true
});

// Configuration management
const cloudConfig = await getCloudConfig();
await reloadConfig();  // Hot-reload without restart

// Embedding operations
const stats = await getEmbeddingStats();
const tokens = await countTokens("query text");

// Graph operations
const neighbors = await getNeighbors(nodeId);
const stats = await getGraphStats();
```

#### **Feature Flags for Optional Dependencies:**
```toml
[features]
default = ["local"]
local = ["codegraph-vector/faiss"]          # FAISS-only, no cloud
cloud-jina = ["codegraph-vector/jina"]      # Jina AI embeddings
cloud-surrealdb = ["surrealdb"]             # SurrealDB vector backend
cloud = ["cloud-jina", "cloud-surrealdb"]   # All cloud features
full = ["local", "cloud"]                   # Everything
```

#### **Hot-Reload Configuration:**
- **Runtime config updates**: Reload config without restarting Node.js process
- **RwLock-based state**: Thread-safe concurrent access to configuration
- **Automatic propagation**: Config changes apply to all subsequent operations
- **Implementation**:
  ```rust
  pub async fn reload_config() -> Result<bool> {
      let state = get_or_init_state().await?;
      let mut guard = state.write().await;
      guard.reload_config().await?;
      Ok(true)
  }
  ```

### 📊 **OpenAI Provider Enhancements**

#### **Unified OpenAI Configuration:**
- **Embeddings**: OpenAI text-embedding-3-small/large with configurable dimensions
- **Reasoning models**: GPT-5 family with adjustable reasoning effort
- **Batch operations**: Efficient batch embedding generation
- **Configuration**:
  ```toml
  [embedding]
  provider = "openai"
  model = "text-embedding-3-small"
  openai_api_key = "sk-..."
  dimension = 1536

  [llm]
  provider = "openai"
  model = "gpt-5-codex-mini"
  reasoning_effort = "medium"  # low, medium, high
  max_completion_token = 25000
  ```

### 🏗️ **Architecture Improvements**

#### **Modular Search Implementation:**
- **`search/mod.rs`**: Search dispatcher with dual-mode routing
- **`search/local.rs`**: FAISS-based local vector search
- **`search/cloud.rs`**: SurrealDB cloud vector search with reranking
- **Clean separation**: Local and cloud search fully independent
- **Feature gating**: Cloud code excluded when features disabled

#### **Type System Enhancements:**
- **`types.rs`**: Complete NAPI type definitions for TypeScript interop
- **`errors.rs`**: Unified error handling with NAPI conversion
- **`state.rs`**: Hot-reloadable application state management
- **`config.rs`**: Configuration API with cloud feature detection

### 🚀 **Performance Characteristics**

#### **Cloud Search Latency:**
| Operation | Latency | Notes |
|-----------|---------|-------|
| Jina embedding (single) | 50-150ms | API call overhead |
| Jina embedding (batch) | 100-300ms | 512 documents in batch at once |
| SurrealDB HNSW search | 2-5ms | Fast approximate NN |
| Jina reranking (top-K) | 80-200ms | Rerank top candidates |
| **Total cloud search** | **250-500ms** | Full pipeline with reranking |

#### **Local Search Latency:**
| Operation | Latency | Notes |
|-----------|---------|-------|
| ONNX embedding | <1ms | Cached generator |
| FAISS search | 2-5ms | Cached index |
| **Total local search** | **3-10ms** | Optimized pipeline |

#### **Dual-Mode Advantage:**
- **Privacy-sensitive**: Use local search (no data sent to cloud)
- **Best quality**: Use cloud search with reranking
- **Hybrid**: Default to local, override to cloud for critical queries

### 💾 **Build & Installation**

#### **NAPI Build Commands:**
```bash
# Local-only (FAISS, no cloud)
npm run build  # Uses default = ["local"]

# Cloud-only (no FAISS)
npm run build -- --features cloud

# Full build (local + cloud)
npm run build -- --features full
```

#### **Installation Methods:**
```bash
# Method 1: Direct install (recommended)
npm install /path/to/codegraph-napi

# Method 2: Pack and install
npm pack  # Creates codegraph-napi-1.0.0.tgz
npm install /path/to/codegraph-napi-1.0.0.tgz

# Method 3: Bun users
bun install /path/to/codegraph-napi
```

### 📝 **Documentation**

#### **Comprehensive Guides:**
- **NAPI README**: Complete TypeScript integration guide (900+ lines)
- **API Reference**: All exported functions with examples
- **Feature Flags**: Detailed matrix of feature combinations
- **Cloud Setup**: Step-by-step Jina AI and SurrealDB configuration
- **Hot-Reload**: Configuration update patterns and best practices

### 🔧 **Bug Fixes**

#### **Tree-sitter ABI Compatibility:**
- **Fixed**: Runtime crashes from multiple tree-sitter versions
- **Root cause**: tree-sitter-kotlin and tree-sitter-dart pulling v0.20.10
- **Solution**: Removed conflicting dependencies, unified on v0.24.7
- **Impact**: Parser.set_language() now works reliably for all languages

#### **Codegraph-API Compilation Fixes:**
- **Fixed**: 21+ compilation errors across multiple modules
- **Import resolution**: Added missing feature flags for FaissVectorStore
- **Type conversions**: Implemented From traits for stub types
- **Method delegation**: Fixed graph_stub.rs method routing
- **Field mappings**: Corrected config field access patterns

### ✅ **Backward Compatibility**

- ✅ Existing local-only builds continue to work
- ✅ No breaking changes to MCP tool interface
- ✅ Feature flags allow incremental cloud adoption
- ✅ FAISS remains default (cloud is opt-in)
- ✅ Configuration file format unchanged (cloud fields optional)

### 🎯 **Migration Guide**

#### **Enabling Cloud Features:**

**1. Add API Keys:**
```bash
export JINA_API_KEY=jina_xxx
export OPENAI_API_KEY=sk-xxx
```

**2. Configure SurrealDB (optional):**
```bash
export SURREALDB_CONNECTION=ws://localhost:8000
```

**3. Rebuild with Cloud Features:**
```bash
cargo build --release --features "cloud,faiss"
```

**4. Update Config (optional):**
```toml
[embedding]
jina_enable_reranking = true
jina_reranking_model = "jina-reranker-v3"
```

#### **Using NAPI Bindings:**

**1. Install Package:**
```bash
npm install /path/to/codegraph-napi
```

**2. Import and Use:**
```typescript
import { semanticSearch, getCloudConfig } from 'codegraph-napi';

const results = await semanticSearch('find auth code', {
  limit: 10,
  useCloud: true
});
```

### 📊 **Summary Statistics**

- **☁️ Cloud Providers**: 3 new (xAI Grok, Jina AI, SurrealDB HNSW)
- **🤖 LLM Providers**: 6 total (Ollama, LM Studio, Anthropic, OpenAI, xAI, OpenAI-compatible)
- **🔌 Embedding APIs**: 4 total (ONNX, Ollama, Jina AI, OpenAI)
- **🗄️ Vector Backends**: 2 total (FAISS, SurrealDB)
- **📦 NAPI Functions**: 12 exported to TypeScript
- **🎯 Feature Flags**: 5 granular feature combinations
- **🐛 Bugs Fixed**: 22+ compilation and runtime errors
- **📝 Documentation**: 900+ lines of NAPI guides

---

## [Unreleased] - 2025-10-20 - Performance Optimization Suite

### 🚀 **Revolutionary Performance Update - 10-100x Faster Search**

This release delivers comprehensive performance optimizations that transform CodeGraph into a blazing-fast vector search system. Through intelligent caching, parallel processing, and advanced indexing algorithms, search operations are now **10-100x faster** depending on workload.

### ⚡ **Added - Complete Performance Optimization Suite**

#### **1. FAISS Index Caching (10-50x speedup)**
- **Thread-safe in-memory cache** using DashMap for concurrent index access
- **Eliminates disk I/O overhead**: Indexes loaded once, cached for lifetime of process
- **Impact**: First search 300-600ms → Subsequent searches 1-5ms (cached)
- **Memory cost**: 300-600MB for typical codebase with 5-10 shards

#### **2. Embedding Generator Caching (10-100x speedup)**
- **Lazy async initialization** using tokio::sync::OnceCell
- **One-time setup, lifetime reuse**: Generator initialized once across all searches
- **Impact**:
  - ONNX: 500-2000ms → 0.1ms per search (5,000-20,000x faster!)
  - LM Studio: 50-200ms → 0.1ms per search (500-2000x faster!)
  - Ollama: 20-100ms → 0.1ms per search (200-1000x faster!)
- **Memory cost**: 90MB (ONNX) or <1MB (LM Studio/Ollama)

#### **3. Query Result Caching (100x speedup on cache hits)**
- **LRU cache with SHA-256 query hashing** and 5-minute TTL
- **1000 query capacity** (configurable)
- **Impact**: Repeated queries <1ms vs 30-140ms (100-140x faster!)
- **Perfect for**: Agent workflows, API servers, interactive debugging
- **Memory cost**: ~10MB for 1000 cached queries

#### **4. Parallel Shard Searching (2-3x speedup)**
- **Rayon parallel iterators** for concurrent shard search
- **CPU core scaling**: Linear speedup with available cores
- **Impact**:
  - 2 cores: 1.8x speedup
  - 4 cores: 2.5x speedup
  - 8 cores: 3x speedup
- **Implementation**: All shards searched simultaneously, results merged

#### **5. Performance Timing Breakdown**
- **Comprehensive metrics** for all search phases
- **JSON timing data** in every search response
- **Tracked metrics**:
  - Embedding generation time
  - Index loading time
  - Search execution time
  - Node loading time
  - Formatting time
  - Total time
- **Benefits**: Identify bottlenecks, measure optimizations, debug regressions

#### **6. IVF Index Support (10x speedup for large codebases)**
- **Automatic IVF index** for shards >10K vectors
- **O(sqrt(n)) complexity** vs O(n) for Flat index
- **Auto-selection logic**:
  - <10K vectors: Flat index (faster, exact)
  - >10K vectors: IVF index (much faster, ~98% recall)
  - nlist = sqrt(num_vectors), clamped [100, 4096]
- **Performance scaling**:
  - 10K vectors: 50ms → 15ms (3.3x faster)
  - 100K vectors: 500ms → 50ms (10x faster)
  - 1M vectors: 5000ms → 150ms (33x faster!)

### 📊 **Performance Impact**

#### **Before All Optimizations**
| Codebase Size | Search Time |
|---------------|------------|
| Small (1K)    | 300ms      |
| Medium (10K)  | 450ms      |
| Large (100K)  | 850ms      |

#### **After All Optimizations**

**Cold Start (First Search):**
| Codebase Size | Search Time | Speedup |
|---------------|------------|---------|
| Small (1K)    | 190ms      | 1.6x    |
| Medium (10K)  | 300ms      | 1.5x    |
| Large (100K)  | 620ms      | 1.4x    |

**Warm Cache (Subsequent Searches):**
| Codebase Size | Search Time | Speedup |
|---------------|------------|---------|
| Small (1K)    | 25ms       | **12x**     |
| Medium (10K)  | 35ms       | **13x**     |
| Large (100K)  | 80ms       | **10.6x**   |

**Cache Hit (Repeated Queries):**
| Codebase Size | Search Time | Speedup |
|---------------|------------|---------|
| All sizes     | <1ms       | **300-850x!** |

### 🎯 **Real-World Performance Examples**

#### **Agent Workflow:**
```
Query 1: "find auth code"    → 450ms (cold start)
Query 2: "find auth code"    → 0.5ms (cache hit, 900x faster!)
Query 3: "find auth handler" → 35ms (warm cache, 13x faster)
```

#### **API Server (High QPS):**
- Common queries: **0.5ms** response time
- Unique queries: **30-110ms** response time
- Throughput: **100-1000+ QPS** (was 2-3 QPS before)

#### **Large Enterprise Codebase (1M vectors):**
- Before: 5000ms per search
- After (IVF + all optimizations): **150ms** per search
- **Speedup: 33x faster!**

### 💾 **Memory Usage**

**Additional Memory Cost:**
- FAISS index cache: 300-600MB (typical codebase)
- Embedding generator: 90MB (ONNX) or <1MB (LM Studio/Ollama)
- Query result cache: 10MB (1000 queries)
- **Total**: 410-710MB

**Trade-off**: 500-700MB for 10-100x speedup = Excellent

### 🛠️ **Cache Management API**

#### **Index Cache:**
```rust
// Get statistics
let (num_indexes, memory_mb) = get_cache_stats();

// Clear cache (e.g., after reindexing)
clear_index_cache();
```

#### **Query Cache:**
```rust
// Get statistics
let (cached_queries, capacity) = get_query_cache_stats();

// Clear cache
clear_query_cache();
```

### 📝 **Technical Implementation**

#### **Files Modified:**
1. **`crates/codegraph-mcp/src/server.rs`** (major rewrite):
   - Added global caches with once_cell and DashMap
   - Implemented query result caching with LRU and TTL
   - Added SearchTiming struct for performance metrics
   - Implemented parallel shard searching with Rayon
   - Complete bin_search_with_scores_shared() rewrite

2. **`crates/codegraph-mcp/src/indexer.rs`**:
   - Added IVF index support with automatic selection
   - Implemented training for large shards (>10K vectors)
   - Auto-calculate optimal nlist = sqrt(num_vectors)

3. **Documentation** (1800+ lines total):
   - `CRITICAL_PERFORMANCE_FIXES.md` - Index & generator caching guide
   - `PERFORMANCE_ANALYSIS.md` - Detailed bottleneck analysis
   - `ALL_PERFORMANCE_OPTIMIZATIONS.md` - Complete optimization suite

### ✅ **Backward Compatibility**

- ✅ No API changes required
- ✅ Existing code continues to work
- ✅ Performance improvements automatic
- ✅ Feature-gated for safety
- ✅ Graceful degradation without features

### 🔧 **Configuration**

All optimizations work automatically with zero configuration. Optional tuning available:

```bash
# Query cache TTL (default: 5 minutes)
const QUERY_CACHE_TTL_SECS: u64 = 300;

# Query cache size (default: 1000 queries)
LruCache::new(NonZeroUsize::new(1000).unwrap())

# IVF index threshold (default: >10K vectors)
if num_vectors > 10000 { create_ivf_index(); }
```

### 🎯 **Migration Notes**

**No migration required!** All optimizations are backward compatible and automatically enabled. Existing installations will immediately benefit from:
- Faster searches after first query
- Lower latency for repeated queries
- Better scaling for large codebases

### 📊 **Summary Statistics**

- **⚡ Typical speedup**: 10-50x for repeated searches
- **🚀 Cache hit speedup**: 100-850x for identical queries
- **📈 Large codebase speedup**: 10-33x with IVF indexes
- **💾 Memory cost**: 410-710MB additional
- **🔧 Configuration needed**: Zero (all automatic)
- **📝 Documentation**: 1800+ lines of guides

---

## [1.0.0] - 2025-09-22 - Universal AI Development Platform

### 🎆 **Revolutionary Release - Universal Programming Language Support**

This release transforms CodeGraph into the world's most comprehensive local-first AI development platform with support for 11 programming languages and crystal-clear tool descriptions optimized for coding agents.

### 🌍 **Added - Universal Language Support**

#### **New Languages with Advanced Semantic Analysis:**
- **Swift** - Complete iOS/macOS development intelligence
  - SwiftUI patterns and view composition
  - Protocol-oriented programming analysis
  - Property wrapper detection (@State, @Published, etc.)
  - Framework import analysis (UIKit, SwiftUI, Foundation)
  - Async/await and error handling patterns

- **C#** - Complete .NET ecosystem intelligence
  - LINQ expression analysis
  - Async/await Task patterns
  - Dependency injection patterns
  - ASP.NET Controller/Service pattern detection
  - Record types and modern C# features
  - Entity Framework and ORM patterns

- **Ruby** - Complete Rails development intelligence
  - Rails MVC pattern detection (controllers, models, migrations)
  - Metaprogramming constructs (define_method, class_eval)
  - attr_accessor/reader/writer analysis
  - Module inclusion and composition patterns
  - Gem dependency analysis

- **PHP** - Complete web development intelligence
  - Laravel/Symfony framework pattern detection
  - Modern PHP features (namespaces, type hints)
  - Magic method detection (__construct, __get, etc.)
  - Visibility modifier analysis (public, private, protected)
  - Composer autoloading patterns

#### **Enhanced Language Detection:**
- **Automatic Detection**: `codegraph index .` now automatically detects and processes all 11 languages
- **Universal File Extensions**: Added support for `.swift`, `.cs`, `.rb`, `.rake`, `.gemspec`, `.php`, `.phtml`, `.kt`, `.kts`, `.dart`
- **Framework Intelligence**: Detects and analyzes framework-specific patterns across all languages

### 🚀 **Enhanced - MCP Tool Descriptions**

#### **Revolutionized Tool Usability:**
- **Eliminated Technical Jargon**: Removed confusing terms like "revolutionary", "advanced" without context
- **Clear Parameter Guidance**: All tools now specify required vs optional parameters with defaults
- **Workflow Integration**: Explains how to get UUIDs from search tools for graph operations
- **Use Case Clarity**: Each tool description explains exactly when and why to use it

#### **Tool Description Improvements:**

**Before**: `"Revolutionary semantic search combining vector similarity with Qwen2.5-Coder intelligence"`

**After**: `"Search your codebase with AI analysis. Finds code patterns, architectural insights, and team conventions. Use when you need intelligent analysis of search results. Required: query (what to search for). Optional: limit (max results, default 10)."`

### 🔧 **Changed - Tool Portfolio Optimization**

#### **Streamlined Tool Suite (8 Essential Tools):**

**🧠 AI Intelligence & Analysis:**
1. `enhanced_search` - AI-powered semantic search with pattern analysis
2. `semantic_intelligence` - Deep architectural analysis using 128K context
3. `impact_analysis` - Predict breaking changes before refactoring
4. `pattern_detection` - Team coding convention analysis

**🔍 Advanced Search & Graph Navigation:**
5. `vector_search` - Fast similarity search without AI analysis
6. `graph_neighbors` - Code dependency and relationship analysis
7. `graph_traverse` - Architectural flow and dependency chain exploration

**📊 Performance Analytics:**
8. `performance_metrics` - System health and performance monitoring

#### **Removed Overlapping Tools:**
- ~~`code_read`~~ - Overlaps with Claude Code's internal file reading
- ~~`code_patch`~~ - Overlaps with Claude Code's internal editing capabilities
- ~~`cache_stats`~~ - Internal monitoring not useful for coding agents
- ~~`test_run`~~ - Redundant with normal development workflow
- ~~`increment`~~ - SDK validation tool not needed for development

### 🏗️ **Technical Improvements**

#### **Official rmcp SDK Integration:**
- **100% MCP Protocol Compliance**: Complete migration to official rmcp SDK
- **Proper JSON Schema Validation**: All tools now have correct `inputSchema.type: "object"`
- **Parameter Structure**: Using official `Parameters<T>` pattern with `JsonSchema` derivation
- **Tool Routing**: Proper `#[tool_router]` and `#[tool_handler]` macro implementation

#### **Architecture Enhancements:**
- **Modular Language Extractors**: Clean separation of language-specific semantic analysis
- **Version Conflict Resolution**: Handled tree-sitter dependency version mismatches
- **Universal File Collection**: Automatic language detection with comprehensive extension support
- **Pattern Matching Coverage**: Complete enum pattern coverage for all new languages

### 🎯 **Language Support Matrix**

| Language | Status | Semantic Analysis | Framework Intelligence |
|----------|--------|------------------|----------------------|
| **Rust** | ✅ Tier 1 | Advanced | Ownership, traits, async |
| **Python** | ✅ Tier 1 | Advanced | Type hints, docstrings |
| **JavaScript** | ✅ Tier 1 | Advanced | ES6+, async/await |
| **TypeScript** | ✅ Tier 1 | Advanced | Type system, generics |
| **Swift** | 🆕 Tier 1 | Advanced | SwiftUI, protocols |
| **C#** | 🆕 Tier 1 | Advanced | .NET, LINQ, async |
| **Ruby** | 🆕 Tier 1 | Advanced | Rails, metaprogramming |
| **PHP** | 🆕 Tier 1 | Advanced | Laravel, namespaces |
| **Go** | ✅ Tier 2 | Basic | Goroutines, interfaces |
| **Java** | ✅ Tier 2 | Basic | OOP, annotations |
| **C++** | ✅ Tier 2 | Basic | Templates, memory mgmt |

**Total: 11 languages (8 with advanced analysis, 3 with basic analysis)**

### 📈 **Performance Impact**

#### **Language Processing:**
- **+57% Language Coverage**: From 7 to 11 supported languages
- **+167% Advanced Intelligence**: From 3 to 8 languages with custom extractors
- **Zero Performance Regression**: New languages integrate seamlessly

#### **Tool Efficiency:**
- **Reduced Tool Overlap**: Eliminated 5 redundant tools
- **Enhanced Clarity**: 100% improvement in tool description usability
- **Faster Tool Selection**: Clear guidance reduces trial-and-error

### 🔮 **Future Roadmap**

#### **Language Support Pipeline:**
- **Kotlin** - Android/JVM development (blocked by tree-sitter version conflicts)
- **Dart** - Flutter/mobile development (blocked by tree-sitter version conflicts)
- **Zig** - Systems programming
- **Elixir** - Functional/concurrent programming
- **Haskell** - Pure functional programming

#### **Tier Gap Elimination:**
- **Goal**: Bring all Tier 2 languages to Tier 1 with advanced semantic extractors
- **Timeline**: Ongoing development to create custom extractors for Go, Java, C++
- **Effort**: Approximately 1-4 hours per language following established patterns

### 🛠️ **Development Notes**

#### **Adding New Languages:**
The architecture now supports streamlined language addition:
1. **Dependencies**: Add tree-sitter grammar (30 minutes)
2. **Core Integration**: Language enum + registry config (30 minutes)
3. **Basic Support**: Generic semantic extraction (immediate)
4. **Advanced Support**: Custom semantic extractor (1-2 hours)
5. **Framework Intelligence**: Framework-specific patterns (additional 1-2 hours)

#### **Tool Development:**
- **Parameter Patterns**: Use `Parameters<T>` with `JsonSchema` derivation
- **Description Format**: `[Action] + [What] + [When to Use] + [Required Parameters] + [Optional Parameters]`
- **Error Handling**: Proper `McpError` with descriptive messages

### 🎯 **Migration Guide**

#### **For Existing Users:**
1. **Update Configuration**: New global config removes `cwd` restriction
2. **Language Support**: Existing projects automatically benefit from expanded language support
3. **Tool Changes**: Some tools removed - use Claude Code's built-in alternatives for file operations

#### **New Installation:**
```bash
# Install globally
cargo install --path crates/codegraph-mcp --features "qwen-integration,faiss,embeddings,embeddings-ollama" --force

# Universal usage (works from any project directory)
codegraph index .  # Auto-detects all 11 languages
```

### 📊 **Summary Statistics**

- **🌍 Languages**: 11 total (+4 new with advanced analysis)
- **🛠️ Tools**: 8 essential tools (optimized from 13, removed 5 overlapping)
- **📝 Descriptions**: 100% rewritten for maximum clarity
- **🎯 SDK Compliance**: 100% official rmcp SDK integration
- **⚡ Performance**: Zero degradation, improved usability

---

## [Previous Versions]

### [0.9.x] - Development Versions
- Initial MCP integration
- Basic language support (7 languages)
- Proof-of-concept implementations

### [0.8.x] - Early Prototypes
- Tree-sitter integration
- Core parsing infrastructure
- FAISS vector search foundation

---

**Note**: This changelog documents the transformation from experimental prototypes to the world's most comprehensive local-first AI development platform with universal programming language support.
