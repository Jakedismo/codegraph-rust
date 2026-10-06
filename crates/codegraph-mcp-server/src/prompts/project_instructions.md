# codegraph

CodeGraph is a CLI tool available through Bash as the `codegraph` command. Run the
commands below using your Bash or shell execution tool. Check that CodeGraph is
on PATH with `command -v codegraph`.

When CodeGraph is available and the project is indexed, start code exploration
with its CLI agent commands. Ask a specific question about the task, relevant
symbols or paths instead of starting with broad grep/rg searches:

- `codegraph agent context "Find the implementation and callers for <task>" --focus search`
  locates code; use `--focus builder` to gather implementation context or `question`
  to explain behavior.
- `codegraph agent impact "What depends on <symbol> and what would <change> affect?"`
  checks dependencies before editing; `--focus call_chain` follows call flows.
- `codegraph agent architecture "Describe <area> and its interfaces"`
  maps structure; `--focus api_surface` inspects public interfaces.
- `codegraph agent quality "Assess coupling, complexity and risks in <area>"`
  supports refactoring decisions and targeted follow-up checks.

Run from the indexed project root, or append `--project /path/to/project`; retain
the indexed `--project-id` if one was configured. Prefer the default JSON output:
inspect source locations, findings and partial-result warnings, then read the
specific files/lines before editing. Reuse useful findings and narrow follow-up
questions rather than repeating broad queries. After changes, verify against
current source and run relevant tests; the index may lag uncommitted work.

If CodeGraph is unavailable, the project is not indexed, a command fails or returns
insufficient evidence, fall back to targeted source reads and rg/grep. Known file
locations and exact-string verification also warrant direct reads/searches.
Do not install CodeGraph, download models or reindex solely to satisfy these
instructions. Agent queries use the configured model and may incur provider costs.
Use the four public agent commands; internal graph tools belong to CodeGraph's
built-in agents. Reload usage details with `codegraph agent instructions`.
