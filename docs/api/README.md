---
pdf-engine: lualatex
mainfont: "DejaVu Serif"
monofont: "DejaVu Sans Mono"
header-includes: |
  \usepackage{fontspec}
  \directlua{
    luaotfload.add_fallback("emojifallback", {"NotoColorEmoji:mode=harf;"})
  }
  \setmainfont[
    RawFeature={fallback=emojifallback}
  ]{DejaVu Serif}
---

# CodeGraph API Documentation

**Production-ready code analysis and embedding system APIs**

## Quick Links

- [GraphQL API](./graphql.md) - Schema, queries, mutations & subscriptions
- [REST API](./rest.md) - HTTP endpoints with OpenAPI specs  
- [WebSocket API](./websocket.md) - Real-time subscriptions
- [Authentication](./authentication.md) - JWT, API keys & authorization
- [Rate Limiting](./rate-limiting.md) - Throttling & caching policies
- [Error Handling](./errors.md) - Status codes & error formats

## API Overview

CodeGraph provides three primary API interfaces:

| Interface | Description | Use Case |
|-----------|-------------|----------|
| **GraphQL** | Flexible, type-safe queries | Complex data retrieval, real-time updates |
| **REST** | Traditional HTTP endpoints | Simple CRUD operations, webhooks |
| **WebSocket** | Real-time bidirectional | Live updates, streaming data |

## Authentication Methods

- **API Keys** - For service-to-service communication
- **JWT Tokens** - For user sessions and temporary access
- **OAuth 2.0** - For third-party integrations (planned)

## Rate Limiting

All APIs implement intelligent rate limiting:
- **Burst**: 100 requests per minute
- **Sustained**: 1000 requests per hour  
- **WebSocket**: 50 concurrent connections per client

## Response Formats

All APIs return structured, consistent responses:

```json
{
  "data": { /* actual response data */ },
  "meta": {
    "timestamp": "2024-09-10T12:00:00Z",
    "request_id": "req_1234567890",
    "version": "1.0"
  }
}
```

## Error Handling

Errors follow RFC 7807 Problem Details format:

```json
{
  "type": "https://api.codegraph.dev/errors/validation",
  "title": "Validation Failed",
  "status": 400,
  "detail": "Invalid project configuration",
  "instance": "/api/v1/projects/create",
  "request_id": "req_1234567890"
}
```

## SDK Support

Official SDKs available for:
- **Rust** - `codegraph-client` crate
- **Python** - `codegraph-py` package
- **TypeScript/Node.js** - `@codegraph/client` npm package
- **Go** - `go-codegraph` module (community)

## Getting Started

1. [Obtain API credentials](./authentication.md#obtaining-credentials)
2. [Make your first request](./quickstart.md)
3. [Explore the interactive playground](https://api.codegraph.dev/playground)

## Need Help?

- **Documentation Issues**: [GitHub Issues](https://github.com/codegraph/embedding-system/issues)
- **API Support**: [Discord Community](https://discord.gg/codegraph)
- **Enterprise Support**: enterprise@codegraph.dev