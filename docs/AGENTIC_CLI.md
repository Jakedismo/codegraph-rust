# Agentic CLI and code-agent hooks

CodeGraph exposes the same four public agentic tools through MCP and one-shot CLI
commands. The built-in agents gather graph evidence, reason over it, and synthesize
answers. The CLI does not expose their internal graph analysis tools.

## Build and prerequisites

```sh
cargo build -p codegraph-mcp-server --bin codegraph --features full
./target/debug/codegraph agent --help
```

`ai-enhanced` is the required feature for agentic execution; `full` also includes
embedding providers, HTTP transport, and all agent backends. Select the embedding
features appropriate to your existing setup if using a smaller build. Plain builds
can show help/instructions and install hooks; executing an agentic command fails
with an actionable feature error, and the hook emitter supplies no context.

Use the same SurrealDB schema, indexed project, embedding provider, LLM settings,
and `CODEGRAPH_AGENT_ARCHITECTURE` as for MCP. See [installation](INSTALLATION_GUIDE.md)
and [provider configuration](AI_PROVIDERS.md). No MCP server or transport is started
by an agentic CLI call. Each call runs a fresh agent workflow and may incur the
configured provider's costs.

## Public commands

| MCP tool | CLI command | Accepted `--focus` | Default |
| --- | --- | --- | --- |
| `agentic_context` | `codegraph agent context` | `search`, `builder`, `question` | `builder` |
| `agentic_impact` | `codegraph agent impact` | `dependencies`, `call_chain` | `dependencies` |
| `agentic_architecture` | `codegraph agent architecture` | `structure`, `api_surface` | `structure` |
| `agentic_quality` | `codegraph agent quality` | `complexity`, `coupling`, `hotspots` | quality workflow |

These defaults mirror the MCP implementation. Quality focuses are accepted aliases
for the same quality workflow, as they are over MCP. CLI focuses are validated
against their tool; an invalid focus is rejected before configuration or networking.
The MCP request's existing `limit` field is unused by these workflows and has no
CLI counterpart.

```sh
codegraph agent context "Where is configuration loaded?" --focus search
codegraph agent context "How does provider selection work?" --focus question
codegraph agent impact "What breaks if configuration precedence changes?" --focus call_chain
codegraph agent architecture "Describe the indexing interfaces" --focus api_surface
codegraph agent quality "Coupling risks in the parser" --focus coupling
```

Run from the indexed project root, or select it explicitly:

```sh
codegraph agent context "Understand the parser" --project /path/to/project
codegraph agent impact "Changing parser output" --project /path/to/project --project-id custom-id
codegraph --config /path/to/providers.toml agent context "Find embedding setup"
codegraph agent context --query-file question.txt --timeout-secs 600
printf '%s\n' 'Explain configuration loading' | codegraph agent context --query-file -
```

`--project` changes the working directory before loading `.env` and `.codegraph.toml`.
It retains an existing `CODEGRAPH_PROJECT_ID`; use `--project-id` to override it when
needed. Without a configured identifier, the shared workflow uses the canonical
working directory, matching indexing. Query-file and explicit config paths are
resolved relative to the invoking directory. `--config` selects a TOML file via
`CODEGRAPH_CONFIG_PATH`, ahead of the project/user TOML defaults; environment
variables still override TOML fields. Guidance and hook commands do not load either
provider configuration or `.env`.

## Output and failures

Default stdout is a single JSON object containing the same response data as the MCP
tool's text content: `analysis_type`, `query`, `tier`, `framework`, `answer`,
`findings`, `steps_taken`, `tool_use_count`, and `structured_output` when available.
Logs and diagnostics go to stderr; `--verbose` increases log detail. Source locations
and confidence depend on the evidence returned by the agent, just as over MCP.

`--format text` prints only `answer`. JSON is preferable for agents because it
preserves metadata and partial-result warnings. An internal backend timeout may
return a partial answer with warnings and exit successfully, preserving MCP behavior.
Check `findings` and the answer before treating a result as complete.

| Exit | Meaning |
| --- | --- |
| `0` | Workflow returned a response, possibly partial as described above |
| `1` | Execution, configuration, input-file, or deadline failure |
| `2` | CLI argument error, such as an unsupported focus |

JSON-mode execution failures print `{"error":{"tool":"agentic_context","message":"..."}}`
and a diagnostic on stderr. Text-mode failures leave stdout empty. Clap argument
errors use stderr. `--timeout-secs` sets a whole-workflow deadline (default 600
seconds / 10 minutes), including provider/database setup, graph calls and model
responses. It expires with exit 1, without claiming a complete answer.

## Testing the CLI

After indexing finishes, run the same eight questions as `test_http_mcp.py` through
the CLI:

```sh
python3 test_cli_agentic.py
python3 test_cli_agentic.py --binary /path/to/codegraph --project /path/to/indexed/project
# Run only the three context questions, or one numbered question:
python3 test_cli_agentic.py --tool context
python3 test_cli_agentic.py --case 5 --timeout-secs 600
# Review already saved answers without rerunning queries (latest run):
python3 test_cli_agentic.py --replay test_output_cli
python3 test_cli_agentic.py --replay test_output_cli --case 5
# Inspect cases and commands without contacting providers:
python3 test_cli_agentic.py --list
python3 test_cli_agentic.py --dry-run
```

The runner needs only Python 3.8+ and an existing agent-enabled binary (`full` or
`ai-enhanced`). It defaults to `codegraph` on PATH; `CODEGRAPH_BIN` or `--binary`
selects another executable. `--project-id` and `--config` match the CLI's overrides.
Each CLI process loads the selected project's environment; the Python script does
not load this repository's `.env` or start an MCP server. Use your existing model
configuration; agent queries can incur provider costs. The embedded database accepts
one process at a time, so finish indexing and stop an MCP server holding that store
before running the tests. Cases run sequentially.

Questions, focuses and 600-second deadlines are shared in `agentic_test_cases.py`
so HTTP and CLI inputs stay identical. `--tool` and `--case` can be repeated;
combined filters select their intersection. `--timeout-secs` overrides each deadline.
A process watchdog allows five extra seconds for startup/shutdown, then terminates
a stalled command.
The HTTP test's SSE read budget allows the full case deadline plus five seconds;
the case's own deadline still bounds how long it waits for the agent's answer.

Full agent answers and structured evidence are printed after each case completes,
so you can judge their reasoning and source references directly in the terminal.
JSON answers are pretty-printed; `--summary-only` keeps terminal output compact
without removing saved responses. `--replay PATH` displays a saved case JSON, all
cases in a run directory, or the latest run under a results directory. It also
works before a running suite writes its final summary and supports `--tool`/`--case`
filters. Replay launches no commands, writes no files, and displays saved failures
with their original status; successful inspection exits 0.

Each timestamped run under ignored `test_output_cli/` contains per-case JSON and
readable logs with the complete response, stdout/stderr, command, status, timing,
step/tool counts, warnings and unique structured file locations. `summary.json`
collects the results; `--output-dir` changes the parent directory. Nonzero CLI exits,
error payloads, invalid/empty JSON answers, timeouts and reported partial results
make the runner exit 1; it still runs later cases. Invalid arguments or a missing
binary/config file exit 2; writing failures exit 1.
`OK` means a valid answer was returned, not that its factual accuracy was scored.
Source counts can vary with extraction tier, model and available index evidence;
review saved answers when comparing runs.

Offline runner regression tests use temporary mock executables:

```sh
python3 -m unittest discover -s scripts/tests -p 'test_*.py'
```

## Project initialization

```sh
codegraph init /path/to/project
codegraph init /path/to/project --hooks both --index-tier balanced --workers 4
codegraph init /path/to/project --hooks none --no-index
```

Init runs these steps in order:

1. Offer project-local guidance hooks for **Claude Code, Codex, both, or none**.
   Enter a number or name; Enter defaults to none. If either harness already has
   CodeGraph's session and subagent hooks, init skips this prompt. Use
   `--hooks claude|codex|both|none` to choose explicitly or add the other harness. Noninteractive
   runs require this flag when CodeGraph hooks are absent. None leaves existing hooks
   untouched; unrelated hooks do not count as CodeGraph integration.
2. Add or refresh a `# codegraph` section in both project `AGENTS.md` and `CLAUDE.md`,
   even with no hooks selected. The guidance starts exploration with specific agent
   queries, recommends impact checks before editing, and covers source verification,
   output, project selection and fallbacks. Other content and custom CodeGraph rules
   are preserved; only the block between `<!-- codegraph:begin -->` and
   `<!-- codegraph:end -->` is owned by init. Both files are created if absent.
3. Load the selected project's environment/configuration and recursively index it
   using the existing indexing pipeline and tier/inference policies. Explicit
   `--config` paths remain relative to the invoking directory. Indexing failures
   return nonzero while leaving completed setup available for the next attempt.

`--no-index` finishes after setup without loading application/provider configuration,
creating a database or contacting services. It works in plain builds too. Init is
separate from `codegraph config init`, which creates global application configuration.
The default indexing tier is fast; select balanced/full only after satisfying their
language-server prerequisites. Model providers must already be configured for any
enabled inference stages.

Repeating the same setup leaves files unchanged. Selected hook settings and both
guidance files are validated before writing; malformed settings, broken ownership
markers, unclosed Markdown fences and symlinks fail before setup writes begin.
Writes replace individual files atomically and retain existing file permissions.
User-level harness settings are never modified. Reload the harness to load updated
instructions, and review/trust hooks through its normal controls.

## Harness guidance and hooks

`codegraph agent instructions` prints standalone guidance for any harness or a
manually maintained instruction file. It explains tool selection, focus options,
project selection, output, costs, evidence verification, and fallbacks. It works
without database access or provider credentials.

For Claude Code and Codex, preview and then install project-local hooks:

```sh
codegraph hooks install --harness both --project /path/to/project --dry-run
codegraph hooks install --harness both --project /path/to/project
# Or select one harness: --harness claude / --harness codex
```

The installer merges into `.claude/settings.json` and `.codex/hooks.json`, preserving
existing hooks, permissions, and unrelated settings. Repeating installation leaves
the files unchanged. Invalid settings abort before either file is written. Symlinked
settings/directories and the user home directory are rejected so installation stays
project-local. No user configuration, shared `AGENTS.md`, or binaries are installed.
`--dry-run` prints proposed settings and writes nothing.

Hooks invoke `codegraph hooks emit` only if `codegraph` is on the harness's PATH.
They use a POSIX shell, suitable for macOS/Linux and Claude's Bash environment on
Windows; Windows-native harnesses need an equivalent shell command. Ensure the
built binary is available through your existing installation or PATH. Reload/restart
the harness after installing, and review/trust hooks through the harness's normal
controls. Codex loads both `.codex/hooks.json` and inline hooks in
`.codex/config.toml` and may warn when both are present. The installer reuses existing
CodeGraph handlers from either source, adds only missing lifecycle events, and leaves
the TOML file unchanged.

- `SessionStart` on startup, resume, clear, and compact restores the full CLI guide.
- `SubagentStart` supplies the same guide to newly started code agents.
- No per-tool or per-prompt hook is installed, avoiding repeated context injection.

The emitter reads the event JSON from stdin and writes
`hookSpecificOutput.hookEventName` plus `additionalContext`. It finds the nearest
Git project root from event `cwd`, including Git worktrees, and adds an explicit
project-selection reminder for sessions started in subdirectories. Malformed,
oversized, or unsupported events return `{}`. Missing binaries emit nothing. Hooks
never block tool use, grant permissions, inspect transcripts, index code, contact
providers, or run an agentic query.

To remove the integration, delete handlers containing `codegraph hooks emit` from
the two project files, preserving other handlers. For other harnesses, invoke
`codegraph agent instructions` at the lifecycle points where context is restored,
or adapt the emitter's JSON to that harness's context protocol.

Hook formats follow the official [Claude Code hooks reference](https://code.claude.com/docs/en/hooks)
and [Codex hooks documentation](https://learn.chatgpt.com/docs/hooks).

## Implementation boundary

`AgenticTool` owns the common focus-to-workflow selection. MCP handlers and CLI
commands both call `CodeGraphMCPServer`'s shared agentic workflow; only MCP attaches
peer-based progress notifications. The workflow produces a transport-independent
JSON value, which MCP wraps in tool text and the CLI writes directly. Configuration,
backend selection, prompts, graph evidence gathering, and result synthesis are
therefore shared. The hooks and instructions are separate from execution and work
without loading application configuration.
