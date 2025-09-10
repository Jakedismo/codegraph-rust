# CodeGraph MCP JS/TS SDK

Cross-platform (Browser + Node.js) TypeScript SDK for the CodeGraph Model Context Protocol server.

Features:
- Fully-typed Promise-based API
- Works in Browsers (native WebSocket) and Node (dynamic `ws` import)
- Tree-shaking friendly ESM build (`sideEffects: false`)
- Reconnect with exponential backoff, request timeouts, and simple handshake helpers

## Install

```
npm install @codegraph/mcp-js-sdk @modelcontextprotocol/sdk
```

Node users do not need to install `ws` directly; it is dynamically imported when needed to keep the browser build clean.

## Quick Start

```ts
import MCPClient from '@codegraph/mcp-js-sdk';

const client = new MCPClient({
  url: 'ws://localhost:3001',
  auth: { token: 'dev-secret' },
  agent: { agentId: '00000000-0000-0000-0000-000000000001' },
});

await client.handshake();
console.log('session', client.getSessionId());

// Ping
console.log('latency ms', await client.ping());

// Generic JSON-RPC call
const res = await client.call({ method: 'codegraph/task/distribute', params: {
  taskId: '...optional...',
  targetAgents: [],
  payload: { type: 'noop', data: {} }
}});

console.log(res);

await client.close();
```

## API

- `new MCPClient(options)` – construct client
- `connect()` – connect socket only
- `handshake()` – connect + authenticate + optional agent registration
- `notify(method, params?)` – JSON-RPC notification (no response)
- `request(method, params?)` – JSON-RPC request (returns response)
- `call({ method, params })` – typed helper wrapper for `request`
- `ping()` – measures round-trip latency using server `ping`
- `close(code?, reason?)` – closes the socket
- `on(event, cb)` – subscribe to: `open`, `close`, `error`, `message`, `reconnected`, `session`

## Browser Usage

Use with bundlers (Vite, Webpack, etc.). The SDK auto-uses native `WebSocket` in browsers and avoids bundling `ws`.

## Node Usage

Requires Node 18+. The SDK dynamically imports `ws` at runtime for optimal WebSocket performance and compression.

## Tree-Shaking

The package is ESM-only with `sideEffects: false`. Import only what you need.

