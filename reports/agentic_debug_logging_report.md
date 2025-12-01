# Agentic Debug Logging Investigation (2025-12-01)

## Observed behavior
- With `CODEGRAPH_DEBUG=1` enabled, the JSONL log only includes `tool_call_start`/`tool_call_finish` entries for underlying graph tools when they return `Ok`. Intermediate reasoning from AutoAgents and failure paths are absent, so the debug view shows only successful `agentic_*` results without the thinking trail.

## Findings
1. **DebugLogger is only partially wired**: The helper provides `log_agent_start`, `log_agent_finish`, and `log_reasoning_step` but the only callers are `GraphToolExecutor::execute` for `tool_start`/`tool_finish`. No agentic code path invokes the other hooks (`crates/codegraph-mcp/src/debug_logger.rs`, `crates/codegraph-mcp/src/graph_tool_executor.rs`).
2. **Agentic workflow skips debug hooks**: The AutoAgents stack (`CodeGraphExecutor`, `CodeGraphAgentBuilder`, `CodeGraphChatResponse`) uses `tracing` but never sends events to `DebugLogger`, so reasoning turns and high-level agent start/finish are never written (`crates/codegraph-mcp/src/autoagents/*.rs`, `crates/codegraph-mcp/src/official_server.rs`).
3. **Errors are dropped from the log**: `GraphToolExecutor::execute` logs a start event before validation but only logs a finish event on success. Any `Err` (unknown tool, validation failure, SurrealDB error) returns early without a matching log entry. There is no `tool_call_error` event, so failures stay invisible in debug output.
4. **Viewer expects events that never occur**: `DEBUG_LOGGING.md` and `tools/view_debug_logs.py` describe/format `reasoning_step` and agent lifecycle events, but the runtime never emits them, causing gaps between expectation and reality.
5. **Init tied to CLI main**: `DebugLogger::init()` is called in `crates/codegraph-mcp/src/bin/codegraph.rs` only. If the server is embedded or started differently, debug logging will never activate even with `CODEGRAPH_DEBUG=1`.

## Recommended changes to get full debugging
1. **Lifecycle logging for agentic tools**
   - In `CodeGraphExecutor::execute` (or `execute_agentic_workflow` in `official_server.rs`), call `DebugLogger::log_agent_start(query, analysis_type, tier)` before invoking AutoAgents and `DebugLogger::log_agent_finish(success, output_json, error_message)` in a `match`/`drop_guard` so it runs on both `Ok` and `Err` paths.

2. **Reasoning step logging**
   - In `CodeGraphChatResponse::tool_calls` (where each LLM turn is parsed), emit `DebugLogger::log_reasoning_step(step_counter, reasoning_text, planned_action)` using the parsed `reasoning` and `tool_call.tool_name`. Track `step_counter` per agent run (store an `AtomicU64` inside `CodeGraphChatAdapter` or attach a counter to `AgentHandle`).
   - If the LLM marks `is_final=true`, log a final `reasoning_step` with `action="final_answer"` so the tail of the trace is captured even without a tool call.

3. **Tool error visibility**
   - Extend `DebugLogger` with `log_tool_error(tool_name, parameters, error)` and call it from `GraphToolExecutor::execute` whenever an error is about to be returned (unknown tool, validation errors, downstream failures). Also wrap `execute_sync` in `GraphToolExecutorAdapter` to catch panics/errors and forward them to the new logger.
   - Consider logging `tool_call_finish` with a `success` flag instead of a separate event if you prefer a single schema, but make sure failures reach the log.

4. **Streaming order guarantees**
   - After each write, keep the existing flush to disk. When adding new logging sites, ensure writes follow execution order (start → reasoning → tool start/finish/error → agent finish) to match the spec in `docs/specifications/agentic_debug_logging.spec.md`.

5. **Tests to lock behavior**
   - Add focused tests per the spec: a happy-path agentic request that asserts presence/order of lifecycle + reasoning + tool events; a failing request that asserts `tool_call_error` and `agent_execution_finish.success=false`; and a cache-hit case to ensure tool events still log when returning cached data. Use a temp debug dir to keep test output deterministic.

6. **Documentation alignment**
   - Update `DEBUG_LOGGING.md` to mention the new error event (or success flag) and to note that agentic reasoning is captured via AutoAgents turns. Keep `tools/view_debug_logs.py` in sync so it renders the added event.

## Quick file map for implementation
- Lifecycle hooks: `crates/codegraph-mcp/src/official_server.rs`, `crates/codegraph-mcp/src/autoagents/executor.rs`.
- Reasoning hook: `crates/codegraph-mcp/src/autoagents/agent_builder.rs` (`CodeGraphChatResponse::tool_calls`).
- Tool error hook: `crates/codegraph-mcp/src/graph_tool_executor.rs` and `crates/codegraph-mcp/src/autoagents/tools/tool_executor_adapter.rs`.
- Logger API: `crates/codegraph-mcp/src/debug_logger.rs` (add `tool_call_error` or `success` flag).
- Spec/tests: `docs/specifications/agentic_debug_logging.spec.md` and new Rust/Python tests exercising debug output.
