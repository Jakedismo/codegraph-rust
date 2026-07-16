#!/usr/bin/env bash
#
# wire-clients.sh
# A utility script to automatically configure popular AI coding agents
# to use the local CodeGraph MCP SSE server provided by this container.
#
# Usage: ./wire-clients.sh [SSE_URL] [PROJECT_DIR]
# Example: ./wire-clients.sh http://localhost:8045/sse /path/to/my/project

set -euo pipefail

SSE_URL="${1:-http://localhost:8045/sse}"
REPO_ROOT="${2:-$PWD}"

inject_sse() {
  local target="$1"
  local agent="$2"
  
  if [ ! -f "$target" ]; then
    mkdir -p "$(dirname "$target")"
    echo '{"mcpServers": {}}' > "$target"
  fi
  
  if ! jq . "$target" >/dev/null 2>&1; then
    echo "WARNING: $target is not valid JSON, skipping $agent wiring" >&2
    return
  fi

  jq --arg url "$SSE_URL" '.mcpServers.codegraph = {"type": "sse", "url": $url}' "$target" > "$target.tmp" && mv "$target.tmp" "$target"
  echo "✅ Wired CodeGraph SSE for $agent -> $target"
}

echo "Wiring Colby's CodeGraph MCP SSE URL: $SSE_URL"
echo ""

# 1. Claude Code (Project-scoped)
inject_sse "$REPO_ROOT/.claude.json" "Claude Code"

# 2. Codex (Global)
inject_sse "$HOME/.codex/mcp.json" "Codex"

# 3. Gemini (Global)
inject_sse "$HOME/.gemini/config/mcp.json" "Gemini"

# 4. Kimi (Global)
inject_sse "$HOME/.kimi-code/mcp.json" "Kimi"

# 5. OpenCode (Global)
inject_sse "$HOME/.opencode/mcp.json" "OpenCode"

# 6. Cursor (Global)
inject_sse "$HOME/.cursor/mcp.json" "Cursor"

# 7. VSCode native MCP (Global)
inject_sse "$HOME/.vscode/mcp.json" "VSCode"

# 8. Cline (VSCode extension)
CLINE_PATH="$HOME/.config/Code/User/globalStorage/saoudrizwan.claude-dev/settings/cline_mcp_settings.json"
[ "$(uname)" = "Darwin" ] && CLINE_PATH="$HOME/Library/Application Support/Code/User/globalStorage/saoudrizwan.claude-dev/settings/cline_mcp_settings.json"
inject_sse "$CLINE_PATH" "Cline"

# 9. Roo (VSCode extension)
ROO_PATH="$HOME/.config/Code/User/globalStorage/rooveterinaryinc.roo-cline/settings/mcp_settings.json"
[ "$(uname)" = "Darwin" ] && ROO_PATH="$HOME/Library/Application Support/Code/User/globalStorage/rooveterinaryinc.roo-cline/settings/mcp_settings.json"
inject_sse "$ROO_PATH" "Roo Code"

echo ""
echo "Done! Restart your agents to apply the new configuration."
