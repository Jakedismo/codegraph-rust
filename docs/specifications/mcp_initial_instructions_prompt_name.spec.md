ABOUTME: Specifies the MCP prompt naming contract for CodeGraph initial instructions.
ABOUTME: Ensures clients can request the prompt using a stable, compatible identifier.

# Specification: MCP Prompt Name for Initial Instructions

## Intent

The CodeGraph MCP server must expose the initial instructions prompt under a name that works across common MCP clients.

## Contract

- The prompt name MUST be exactly `codegraph:initial_instructions`.
- The prompt MUST be retrievable via `get_prompt` using the same name.
- The prompt should not rely on titles/prefix formatting for clients to work.

## Acceptance Criteria

1. `list_prompts` includes a prompt whose `name == "codegraph:initial_instructions"`.
2. `get_prompt(name="codegraph:initial_instructions")` succeeds.
3. Unit test enforces the name to prevent regression.
