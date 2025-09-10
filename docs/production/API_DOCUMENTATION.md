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

## Table of Contents

1. [Overview](#overview)
2. [Authentication](#authentication)
3. [Base URLs and Versioning](#base-urls-and-versioning)
4. [Request/Response Format](#requestresponse-format)
5. [Rate Limiting](#rate-limiting)
6. [Error Handling](#error-handling)
7. [Core API Endpoints](#core-api-endpoints)
8. [GraphQL API](#graphql-api)
9. [Streaming API](#streaming-api)
10. [Vector Search API](#vector-search-api)
11. [Version Management API](#version-management-api)
12. [HTTP/2 Optimization API](#http2-optimization-api)
13. [Monitoring and Metrics](#monitoring-and-metrics)
14. [SDKs and Client Libraries](#sdks-and-client-libraries)

## Overview

The CodeGraph API provides comprehensive access to code analysis, graph-based search, vector similarity operations, and advanced versioning capabilities. Built on Rust with Axum, it offers both REST and GraphQL interfaces with high-performance streaming capabilities.

### Key Features

- **Graph-based Code Analysis**: Parse and analyze source code relationships
- **Vector Search**: Semantic similarity search using FAISS
- **Version Management**: Git-like versioning with transaction support
- **Real-time Streaming**: Efficient data streaming for large datasets
- **HTTP/2 Optimization**: Advanced connection management and server push
- **GraphQL Support**: Flexible query language with subscriptions

### Supported Programming Languages

- Rust
- Python
- JavaScript/TypeScript
- Go
- Java
- C/C++

## Authentication

### API Key Authentication

All API requests require authentication via API key in the Authorization header:

```http
Authorization: Bearer your-api-key-here
```

### JWT Token Authentication

For user-based access, JWT tokens are supported:

```http
Authorization: Bearer jwt-token-here
```

### Example Authentication

```bash
curl -H "Authorization: Bearer your-api-key" \
     https://api.codegraph.example.com/health
```

## Base URLs and Versioning

### Production Environment
```
https://api.codegraph.example.com
```

### Development Environment
```
https://api-dev.codegraph.example.com
```

### API Versioning

The API follows semantic versioning. Current version: `v1`

All endpoints are prefixed with `/api/v1/` (when versioned endpoints are implemented).

## Request/Response Format

### Content Types

- **Request**: `application/json`
- **Response**: `application/json`
- **Streaming**: `application/x-ndjson` or `text/event-stream`

### Common Response Structure

```json
{
  "success": true,
  "data": { ... },
  "metadata": {
    "timestamp": "2025-01-10T12:00:00Z",
    "request_id": "uuid-here",
    "processing_time_ms": 42
  }
}
```

### Error Response Structure

```json
{
  "success": false,
  "error": {
    "code": "INVALID_REQUEST",
    "message": "Detailed error description",
    "details": { ... }
  },
  "metadata": {
    "timestamp": "2025-01-10T12:00:00Z",
    "request_id": "uuid-here"
  }
}
```

## Rate Limiting

### Default Limits

- **Requests per second**: 1000
- **Concurrent connections**: 512
- **Request timeout**: 30 seconds

### Rate Limit Headers

```http
X-RateLimit-Limit: 1000
X-RateLimit-Remaining: 999
X-RateLimit-Reset: 1641812400
```

### Rate Limit Response

When rate limited, the API returns `429 Too Many Requests`:

```json
{
  "success": false,
  "error": {
    "code": "RATE_LIMIT_EXCEEDED",
    "message": "Too many requests. Please try again later.",
    "retry_after": 60
  }
}
```

## Error Handling

### HTTP Status Codes

| Code | Description |
|------|-------------|
| 200  | Success |
| 201  | Created |
| 400  | Bad Request |
| 401  | Unauthorized |
| 403  | Forbidden |
| 404  | Not Found |
| 409  | Conflict |
| 422  | Unprocessable Entity |
| 429  | Too Many Requests |
| 500  | Internal Server Error |
| 503  | Service Unavailable |

### Error Codes

| Code | Description |
|------|-------------|
| `INVALID_REQUEST` | Request validation failed |
| `AUTHENTICATION_FAILED` | Invalid or missing authentication |
| `RESOURCE_NOT_FOUND` | Requested resource does not exist |
| `RATE_LIMIT_EXCEEDED` | Too many requests |
| `INTERNAL_ERROR` | Server-side processing error |
| `SERVICE_UNAVAILABLE` | Service temporarily unavailable |

## Core API Endpoints

### Health Check

#### GET `/health`

Check service health and status.

**Response:**
```json
{
  "status": "healthy",
  "version": "1.0.0"
}
```

**Example:**
```bash
curl https://api.codegraph.example.com/health
```

---

### Node Operations

#### GET `/nodes/{id}`

Retrieve a specific code graph node by ID.

**Parameters:**
- `id` (path): Node identifier (UUID)

**Response:**
```json
{
  "success": true,
  "data": {
    "id": "uuid-here",
    "node_type": "function",
    "name": "parse_file",
    "file_path": "/src/parser.rs",
    "line_number": 42,
    "metadata": {
      "visibility": "public",
      "parameters": ["file_path: String"],
      "return_type": "Result<Vec<Node>, Error>"
    },
    "relationships": [
      {
        "type": "calls",
        "target_id": "uuid-other",
        "target_name": "read_file"
      }
    ]
  }
}
```

**Example:**
```bash
curl -H "Authorization: Bearer your-api-key" \
     https://api.codegraph.example.com/nodes/123e4567-e89b-12d3-a456-426614174000
```

#### GET `/nodes/{id}/similar`

Find nodes similar to the specified node using vector similarity.

**Parameters:**
- `id` (path): Node identifier
- `limit` (query): Maximum number of results (default: 10)
- `threshold` (query): Similarity threshold (0.0-1.0, default: 0.8)

**Response:**
```json
{
  "success": true,
  "data": {
    "results": [
      {
        "node": { ... },
        "similarity_score": 0.95,
        "distance": 0.05
      }
    ],
    "total": 5
  }
}
```

---

### Parsing Operations

#### POST `/parse`

Parse a source code file and extract graph nodes.

**Request Body:**
```json
{
  "file_path": "/path/to/source/file.rs",
  "language": "rust",
  "options": {
    "include_comments": true,
    "extract_docs": true,
    "max_depth": 10
  }
}
```

**Response:**
```json
{
  "success": true,
  "data": {
    "nodes_created": 42,
    "message": "Successfully parsed file",
    "nodes": [
      {
        "id": "uuid-here",
        "node_type": "function",
        "name": "main",
        "line_number": 1
      }
    ]
  }
}
```

**Example:**
```bash
curl -X POST \
     -H "Authorization: Bearer your-api-key" \
     -H "Content-Type: application/json" \
     -d '{"file_path": "/src/main.rs"}' \
     https://api.codegraph.example.com/parse
```

---

### Search Operations

#### GET `/search`

Search code graph nodes using text queries.

**Parameters:**
- `query` (query): Search query string
- `limit` (query): Maximum results (default: 20)
- `offset` (query): Result offset for pagination (default: 0)
- `filters` (query): JSON filters object

**Response:**
```json
{
  "success": true,
  "data": {
    "results": [
      {
        "id": "uuid-here",
        "name": "function_name",
        "node_type": "function",
        "file_path": "/src/lib.rs",
        "score": 0.95,
        "highlights": ["function_name", "parameter_type"]
      }
    ],
    "total": 150,
    "pagination": {
      "limit": 20,
      "offset": 0,
      "has_more": true
    }
  }
}
```

**Example:**
```bash
curl -G \
     -H "Authorization: Bearer your-api-key" \
     -d "query=async function" \
     -d "limit=10" \
     https://api.codegraph.example.com/search
```

## GraphQL API

### Endpoint

```
POST /graphql
```

### GraphiQL IDE

Access the interactive GraphQL IDE at:
```
GET /graphiql
```

### WebSocket Subscriptions

Real-time subscriptions via WebSocket:
```
WS /graphql/ws
```

### Schema Overview

#### Types

```graphql
type Node {
  id: ID!
  nodeType: NodeType!
  name: String!
  filePath: String!
  lineNumber: Int!
  metadata: JSON
  relationships: [Relationship!]!
}

type Relationship {
  type: RelationshipType!
  targetId: ID!
  targetName: String!
  metadata: JSON
}

enum NodeType {
  FUNCTION
  CLASS
  INTERFACE
  VARIABLE
  IMPORT
  MODULE
}

enum RelationshipType {
  CALLS
  IMPLEMENTS
  EXTENDS
  IMPORTS
  DEFINES
}
```

#### Queries

```graphql
type Query {
  node(id: ID!): Node
  nodes(
    filter: NodeFilter
    limit: Int = 20
    offset: Int = 0
  ): NodesConnection!
  
  search(
    query: String!
    limit: Int = 20
    offset: Int = 0
  ): SearchConnection!
  
  similarNodes(
    nodeId: ID!
    limit: Int = 10
    threshold: Float = 0.8
  ): [SimilarNode!]!
}
```

#### Mutations

```graphql
type Mutation {
  parseFile(input: ParseFileInput!): ParseResult!
  createNode(input: CreateNodeInput!): Node!
  updateNode(id: ID!, input: UpdateNodeInput!): Node!
  deleteNode(id: ID!): Boolean!
}
```

#### Subscriptions

```graphql
type Subscription {
  nodeCreated: Node!
  nodeUpdated: Node!
  nodeDeleted: ID!
  parseProgress(taskId: ID!): ParseProgress!
}
```

### Example Queries

#### Basic Node Query

```graphql
query GetNode($id: ID!) {
  node(id: $id) {
    id
    name
    nodeType
    filePath
    lineNumber
    relationships {
      type
      targetId
      targetName
    }
  }
}
```

#### Search Query

```graphql
query SearchNodes($query: String!, $limit: Int) {
  search(query: $query, limit: $limit) {
    edges {
      node {
        id
        name
        nodeType
        filePath
      }
      score
    }
    totalCount
  }
}
```

#### Parse File Mutation

```graphql
mutation ParseFile($input: ParseFileInput!) {
  parseFile(input: $input) {
    nodesCreated
    message
    nodes {
      id
      name
      nodeType
    }
  }
}
```

## Streaming API

### Stream Search Results

#### GET `/stream/search`

Stream search results for large result sets.

**Parameters:**
- `query` (query): Search query
- `batch_size` (query): Results per batch (default: 100)
- `format` (query): `ndjson` or `sse` (default: ndjson)

**Response (NDJSON):**
```
{"type":"metadata","total":5000,"estimated_time":30}
{"type":"result","data":{"id":"uuid1","name":"func1"}}
{"type":"result","data":{"id":"uuid2","name":"func2"}}
{"type":"complete","processed":5000,"elapsed_ms":28500}
```

**Example:**
```bash
curl -N -H "Authorization: Bearer your-api-key" \
     "https://api.codegraph.example.com/stream/search?query=async&format=ndjson"
```

### Stream Large Dataset

#### GET `/stream/dataset`

Stream large datasets with flow control.

**Parameters:**
- `dataset_id` (query): Dataset identifier
- `chunk_size` (query): Chunk size in bytes
- `compression` (query): `gzip`, `zstd`, or `none`

### Stream CSV Results

#### GET `/stream/csv`

Export search results as CSV stream.

**Parameters:**
- `query` (query): Search query
- `fields` (query): Comma-separated field list
- `delimiter` (query): CSV delimiter (default: comma)

**Example:**
```bash
curl -N -H "Authorization: Bearer your-api-key" \
     "https://api.codegraph.example.com/stream/csv?query=functions&fields=id,name,file_path"
```

## Vector Search API

### Vector Search

#### POST `/vector/search`

Perform semantic similarity search using vector embeddings.

**Request Body:**
```json
{
  "vector": [0.1, 0.2, 0.3, ...],
  "k": 10,
  "threshold": 0.8,
  "filters": {
    "node_type": "function",
    "language": "rust"
  },
  "include_metadata": true
}
```

**Response:**
```json
{
  "success": true,
  "data": {
    "results": [
      {
        "node_id": "uuid-here",
        "score": 0.95,
        "distance": 0.05,
        "metadata": { ... }
      }
    ],
    "search_time_ms": 23,
    "total_candidates": 1000000
  }
}
```

### Batch Vector Search

#### POST `/vector/batch-search`

Perform multiple vector searches in a single request.

**Request Body:**
```json
{
  "searches": [
    {
      "id": "search1",
      "vector": [0.1, 0.2, ...],
      "k": 10
    },
    {
      "id": "search2",
      "vector": [0.3, 0.4, ...],
      "k": 5
    }
  ],
  "global_filters": {
    "language": "rust"
  }
}
```

### Vector Index Management

#### GET `/vector/index/stats`

Get vector index statistics.

**Response:**
```json
{
  "success": true,
  "data": {
    "index_type": "IVF",
    "total_vectors": 1000000,
    "dimension": 768,
    "index_size_mb": 2048,
    "memory_usage_mb": 1024,
    "build_time_ms": 45000,
    "last_updated": "2025-01-10T12:00:00Z"
  }
}
```

#### POST `/vector/index/rebuild`

Rebuild the vector index with new parameters.

**Request Body:**
```json
{
  "index_type": "HNSW",
  "parameters": {
    "M": 16,
    "efConstruction": 200,
    "efSearch": 100
  },
  "force": false
}
```

## Version Management API

### Transaction Management

#### POST `/transactions`

Begin a new transaction.

**Request Body:**
```json
{
  "isolation_level": "ReadCommitted",
  "timeout_seconds": 300,
  "description": "Bulk node update operation"
}
```

**Response:**
```json
{
  "success": true,
  "data": {
    "transaction_id": "txn-uuid-here",
    "isolation_level": "ReadCommitted",
    "created_at": "2025-01-10T12:00:00Z",
    "expires_at": "2025-01-10T12:05:00Z"
  }
}
```

#### POST `/transactions/{id}/commit`

Commit a transaction.

**Parameters:**
- `id` (path): Transaction ID

#### POST `/transactions/{id}/rollback`

Rollback a transaction.

**Parameters:**
- `id` (path): Transaction ID

### Version Management

#### POST `/versions`

Create a new version snapshot.

**Request Body:**
```json
{
  "name": "v1.2.0",
  "description": "Major feature release",
  "tag": "release",
  "metadata": {
    "author": "developer@example.com",
    "build_number": "123"
  }
}
```

#### GET `/versions`

List all versions.

**Parameters:**
- `limit` (query): Maximum results
- `offset` (query): Pagination offset
- `tag` (query): Filter by tag

#### GET `/versions/{from}/compare/{to}`

Compare two versions.

**Response:**
```json
{
  "success": true,
  "data": {
    "from_version": "v1.1.0",
    "to_version": "v1.2.0",
    "changes": {
      "added_nodes": 42,
      "modified_nodes": 15,
      "deleted_nodes": 3
    },
    "diff": [
      {
        "type": "added",
        "node_id": "uuid-here",
        "node_name": "new_function"
      }
    ]
  }
}
```

### Branch Management

#### POST `/branches`

Create a new branch.

**Request Body:**
```json
{
  "name": "feature/new-parser",
  "base_version": "v1.1.0",
  "description": "New language parser implementation"
}
```

#### POST `/merge`

Merge branches.

**Request Body:**
```json
{
  "source_branch": "feature/new-parser",
  "target_branch": "main",
  "strategy": "merge",
  "message": "Merge new parser feature"
}
```

## HTTP/2 Optimization API

### HTTP/2 Metrics

#### GET `/http2/metrics`

Get HTTP/2 connection and performance metrics.

**Response:**
```json
{
  "success": true,
  "data": {
    "active_streams": 42,
    "total_connections": 128,
    "push_promises_sent": 1500,
    "push_promises_accepted": 1425,
    "flow_control": {
      "window_size": 65535,
      "bytes_pending": 1024
    },
    "performance": {
      "avg_response_time_ms": 45,
      "throughput_mbps": 125.5
    }
  }
}
```

### Server Push Registration

#### POST `/http2/push/register`

Register resources for HTTP/2 server push.

**Request Body:**
```json
{
  "resources": [
    {
      "path": "/api/nodes/{id}",
      "triggers": ["/api/search"],
      "priority": "high"
    }
  ]
}
```

## Monitoring and Metrics

### Metrics Endpoint

#### GET `/metrics`

Prometheus-compatible metrics endpoint.

**Response:**
```
# HELP http_requests_total Total HTTP requests
# TYPE http_requests_total counter
http_requests_total{method="GET",status="200"} 1234

# HELP http_request_duration_seconds HTTP request duration
# TYPE http_request_duration_seconds histogram
http_request_duration_seconds_bucket{le="0.1"} 100
http_request_duration_seconds_bucket{le="0.5"} 200
```

### Application Statistics

#### GET `/stats/transactions`

Get transaction statistics.

#### GET `/stats/recovery`

Get recovery and backup statistics.

#### POST `/integrity/check`

Run database integrity check.

## SDKs and Client Libraries

### Rust SDK

```rust
use codegraph_client::{CodeGraphClient, Config};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::new("https://api.codegraph.example.com")
        .with_api_key("your-api-key");
    
    let client = CodeGraphClient::new(config).await?;
    
    // Parse a file
    let result = client.parse_file("/path/to/file.rs").await?;
    println!("Created {} nodes", result.nodes_created);
    
    // Search nodes
    let results = client.search("async function").await?;
    for node in results.nodes {
        println!("Found: {} in {}", node.name, node.file_path);
    }
    
    Ok(())
}
```

### Python SDK

```python
import asyncio
from codegraph import CodeGraphClient, Config

async def main():
    config = Config("https://api.codegraph.example.com", api_key="your-api-key")
    client = CodeGraphClient(config)
    
    # Parse a file
    result = await client.parse_file("/path/to/file.py")
    print(f"Created {result.nodes_created} nodes")
    
    # Search nodes
    results = await client.search("async function")
    for node in results.nodes:
        print(f"Found: {node.name} in {node.file_path}")

if __name__ == "__main__":
    asyncio.run(main())
```

### JavaScript/TypeScript SDK

```typescript
import { CodeGraphClient, Config } from '@codegraph/client';

const config = new Config('https://api.codegraph.example.com', {
  apiKey: 'your-api-key'
});

const client = new CodeGraphClient(config);

// Parse a file
const result = await client.parseFile('/path/to/file.ts');
console.log(`Created ${result.nodesCreated} nodes`);

// Search nodes
const results = await client.search('async function');
results.nodes.forEach(node => {
  console.log(`Found: ${node.name} in ${node.filePath}`);
});
```

### CLI Tool

```bash
# Install CLI
cargo install codegraph-cli

# Configure
codegraph config set endpoint https://api.codegraph.example.com
codegraph config set api-key your-api-key

# Parse files
codegraph parse /path/to/project

# Search
codegraph search "async function"

# Stream results
codegraph stream search "function" --format csv > results.csv
```

## Best Practices

### Performance Optimization

1. **Use Batch Operations**: For multiple similar requests, use batch endpoints
2. **Implement Caching**: Cache frequently accessed nodes and search results
3. **Use Streaming**: For large datasets, use streaming endpoints
4. **Optimize Queries**: Use specific filters to reduce result sets
5. **Connection Pooling**: Reuse HTTP connections for better performance

### Security Guidelines

1. **API Key Security**: Never expose API keys in client-side code
2. **Rate Limiting**: Implement client-side rate limiting
3. **Input Validation**: Validate all inputs before sending to API
4. **HTTPS Only**: Always use HTTPS in production
5. **Token Rotation**: Regularly rotate API keys and JWT tokens

### Error Handling

1. **Retry Logic**: Implement exponential backoff for transient errors
2. **Circuit Breaker**: Use circuit breaker pattern for resilience
3. **Logging**: Log all API interactions for debugging
4. **Monitoring**: Monitor API usage and error rates
5. **Graceful Degradation**: Handle API unavailability gracefully

For additional information, see the [Operations Runbook](OPERATIONS_RUNBOOK.md) and [Troubleshooting Guide](TROUBLESHOOTING_GUIDE.md).