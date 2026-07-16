# Colby's CodeGraph Dockerized

This repository provides a frictionless, containerized setup for [Colby McHenry's CodeGraph](https://github.com/colbymchenry/codegraph). It packages the Node-based `codegraph` CLI together with `supergateway` to expose the MCP server over an HTTP Server-Sent Events (SSE) endpoint.

This means your AI agents (Claude Code, Cursor, Codex, etc.) can connect to a live-syncing CodeGraph without you having to install Node.js, Rust, or any local binaries on your machine.

## Quick Start

1. **Clone this repository** (or copy `docker-compose.yml` to your project).
2. **Start the container**:
   ```bash
   # Run this from the root of the project you want to index
   docker-compose up -d
   ```
   *Note: This will index the current directory `.` by mounting it to `/repo` inside the container.*

3. **Wire your agents**:
   Use the included script to automatically inject the MCP server configuration into your agents' global settings.
   ```bash
   ./wire-clients.sh
   ```

## How It Works

- The `Dockerfile` installs `@colbymchenry/codegraph` and `supergateway`.
- The `ENTRYPOINT` starts `supergateway` on port `8045` and proxies traffic to `codegraph serve --mcp`.
- The `docker-compose.yml` mounts your code to `/repo` and persists the `.codegraph` index so it doesn't have to rebuild from scratch if you restart the container.

## Supported Auto-Wired Agents

The `wire-clients.sh` script automatically configures:
- Cursor
- VSCode native MCP
- Codex
- Gemini
- Kimi
- OpenCode
- Cline (VSCode extension)
- Roo Code (VSCode extension)
- Claude Code (Project-scoped)
