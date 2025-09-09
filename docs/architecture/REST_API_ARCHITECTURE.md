# CodeGraph REST API Architecture
## High-Performance Code Intelligence System

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

### Document Version: 1.0
### Date: September 2025
### Status: Architecture Specification

---

## Executive Summary

This document defines the REST API architecture for CodeGraph, a high-performance code intelligence system targeting sub-50ms query latency and <1s incremental updates. The API design prioritizes resource-oriented architecture, HTTP/2 streaming for large responses, robust versioning, and comprehensive security patterns.

### Core Performance Targets
- **Query Latency**: <50ms (p99)
- **Update Propagation**: <1s
- **Throughput**: 10,000+ req/s per instance
- **Binary Size**: <50MB
- **Memory Usage**: <500MB (100k LOC)

---

## 1. Resource-Oriented Endpoint Design

### 1.1 Primary Resource Hierarchy

```
/api/v1/
├── projects/                    # Project management
│   ├── {projectId}/
│   │   ├── graph/              # Code graph operations
│   │   ├── files/              # File operations
│   │   ├── entities/           # Code entities
│   │   ├── dependencies/       # Dependency analysis
│   │   ├── search/             # Search operations
│   │   └── analytics/          # Analytics and metrics
├── embeddings/                  # Vector embeddings
├── sessions/                    # Analysis sessions
├── queries/                     # Saved queries
└── system/                      # System operations
```

### 1.2 Resource Definitions

#### Projects Resource
```http
GET    /api/v1/projects                    # List all projects
POST   /api/v1/projects                    # Create new project
GET    /api/v1/projects/{id}              # Get project details
PUT    /api/v1/projects/{id}              # Update project
DELETE /api/v1/projects/{id}              # Delete project
POST   /api/v1/projects/{id}/analyze      # Trigger analysis
GET    /api/v1/projects/{id}/status       # Analysis status
```

#### Graph Resource
```http
GET    /api/v1/projects/{id}/graph                    # Get graph overview
GET    /api/v1/projects/{id}/graph/nodes             # List nodes
POST   /api/v1/projects/{id}/graph/nodes             # Create node
GET    /api/v1/projects/{id}/graph/nodes/{nodeId}    # Get node details
PUT    /api/v1/projects/{id}/graph/nodes/{nodeId}    # Update node
DELETE /api/v1/projects/{id}/graph/nodes/{nodeId}    # Delete node

GET    /api/v1/projects/{id}/graph/edges             # List edges
POST   /api/v1/projects/{id}/graph/edges             # Create edge
GET    /api/v1/projects/{id}/graph/edges/{edgeId}    # Get edge details
DELETE /api/v1/projects/{id}/graph/edges/{edgeId}    # Delete edge

POST   /api/v1/projects/{id}/graph/traverse          # Graph traversal
POST   /api/v1/projects/{id}/graph/paths             # Path finding
POST   /api/v1/projects/{id}/graph/subgraph         # Subgraph extraction
```

#### Files Resource
```http
GET    /api/v1/projects/{id}/files                   # List files
POST   /api/v1/projects/{id}/files                   # Add file
GET    /api/v1/projects/{id}/files/{fileId}          # Get file details
PUT    /api/v1/projects/{id}/files/{fileId}          # Update file
DELETE /api/v1/projects/{id}/files/{fileId}          # Delete file
GET    /api/v1/projects/{id}/files/{fileId}/ast      # Get AST
GET    /api/v1/projects/{id}/files/{fileId}/entities # Get file entities
```

#### Entities Resource
```http
GET    /api/v1/projects/{id}/entities                # List entities
GET    /api/v1/projects/{id}/entities/{entityId}     # Get entity details
GET    /api/v1/projects/{id}/entities/{entityId}/references  # Get references
GET    /api/v1/projects/{id}/entities/{entityId}/dependencies # Get dependencies
POST   /api/v1/projects/{id}/entities/search         # Search entities
```

#### Search Resource
```http
POST   /api/v1/projects/{id}/search/text             # Full-text search
POST   /api/v1/projects/{id}/search/semantic         # Semantic search
POST   /api/v1/projects/{id}/search/code             # Code pattern search
POST   /api/v1/projects/{id}/search/similar          # Similarity search
GET    /api/v1/projects/{id}/search/suggestions      # Search suggestions
```

### 1.3 Resource Schemas

#### Project Schema
```json
{
  "id": "string",
  "name": "string",
  "description": "string",
  "language": "string",
  "rootPath": "string",
  "status": "analyzing|ready|error",
  "createdAt": "2025-09-09T12:00:00Z",
  "updatedAt": "2025-09-09T12:30:00Z",
  "metrics": {
    "fileCount": 1250,
    "nodeCount": 45000,
    "edgeCount": 120000,
    "lastAnalyzed": "2025-09-09T12:25:00Z"
  },
  "config": {
    "ignorePatterns": ["node_modules", ".git"],
    "embeddings": {
      "provider": "local|openai",
      "model": "sentence-transformers/all-MiniLM-L6-v2"
    }
  }
}
```

#### Node Schema
```json
{
  "id": "string",
  "type": "function|class|variable|module|import",
  "name": "string",
  "qualifiedName": "string",
  "file": {
    "id": "string",
    "path": "string",
    "language": "python|javascript|typescript|rust"
  },
  "location": {
    "startLine": 42,
    "endLine": 58,
    "startColumn": 4,
    "endColumn": 1
  },
  "metadata": {
    "visibility": "public|private|protected",
    "isAsync": true,
    "parameters": [],
    "returnType": "string",
    "complexity": 5,
    "docstring": "string"
  },
  "embeddings": {
    "semantic": [0.1, 0.2, ...],  // 384-dim vector
    "structural": [0.3, 0.4, ...]  // 128-dim vector
  },
  "createdAt": "2025-09-09T12:00:00Z",
  "updatedAt": "2025-09-09T12:30:00Z"
}
```

#### Edge Schema
```json
{
  "id": "string",
  "type": "calls|imports|extends|implements|references|contains",
  "source": "string",  // node ID
  "target": "string",  // node ID
  "weight": 1.0,
  "properties": {
    "confidence": 0.95,
    "context": "string",
    "lineNumber": 42
  },
  "createdAt": "2025-09-09T12:00:00Z"
}
```

---

## 2. HTTP/2 Streaming Patterns

### 2.1 Server-Sent Events (SSE) for Real-time Updates

```http
GET /api/v1/projects/{id}/stream
Accept: text/event-stream
Cache-Control: no-cache
```

**Response Stream:**
```
data: {"type":"analysis_started","project":"proj-123","timestamp":"2025-09-09T12:00:00Z"}

data: {"type":"file_analyzed","file":{"id":"file-456","path":"src/main.py","entities":25}}

data: {"type":"graph_updated","stats":{"nodes":45000,"edges":120000,"updated":150}}

data: {"type":"analysis_completed","project":"proj-123","duration":"2.5s","timestamp":"2025-09-09T12:02:30Z"}
```

### 2.2 Chunked Transfer for Large Responses

#### Large Graph Queries
```http
POST /api/v1/projects/{id}/graph/export
Content-Type: application/json
Transfer-Encoding: chunked
Accept: application/x-ndjson

Request:
{
  "format": "json",
  "filters": {
    "nodeTypes": ["function", "class"],
    "files": ["src/**/*.py"]
  },
  "streaming": true,
  "chunkSize": 1000
}
```

**Chunked Response (NDJSON):**
```json
{"type":"metadata","total":45000,"chunks":45}
{"type":"nodes","data":[...1000 nodes...]}
{"type":"nodes","data":[...1000 nodes...]}
{"type":"edges","data":[...1000 edges...]}
{"type":"complete","processed":45000}
```

### 2.3 HTTP/2 Server Push

```http
HTTP/2 200 OK
Link: </api/v1/projects/123/graph/stats>; rel=preload; as=fetch
Link: </api/v1/projects/123/files/recent>; rel=preload; as=fetch

PUSH_PROMISE: /api/v1/projects/123/graph/stats
PUSH_PROMISE: /api/v1/projects/123/files/recent
```

### 2.4 Progressive Loading Endpoints

```http
GET /api/v1/projects/{id}/graph/progressive?depth=2&expand=children
```

**Progressive Response Structure:**
```json
{
  "nodes": [...],
  "edges": [...],
  "pagination": {
    "depth": 2,
    "hasMore": true,
    "nextUrl": "/api/v1/projects/123/graph/progressive?depth=3&cursor=abc123"
  },
  "loadingHints": {
    "preload": [
      "/api/v1/projects/123/entities?filter=referenced",
      "/api/v1/projects/123/graph/stats"
    ]
  }
}
```

---

## 3. API Versioning & Backwards Compatibility

### 3.1 Versioning Strategy

#### URL Path Versioning (Primary)
```http
GET /api/v1/projects
GET /api/v2/projects
GET /api/v1.1/projects  # Minor version for non-breaking changes
```

#### Header-based Versioning (Fallback)
```http
GET /api/projects
API-Version: v1
Accept: application/vnd.codegraph.v1+json
```

#### Query Parameter Versioning (Compatibility)
```http
GET /api/projects?version=v1
```

### 3.2 Version Compatibility Matrix

| Version | Status | Support End | Breaking Changes | Migration Path |
|---------|--------|-------------|------------------|----------------|
| v1.0    | Active | 2026-09-09  | None             | N/A            |
| v1.1    | Active | 2026-12-09  | None             | Optional       |
| v2.0    | Beta   | TBD         | Schema changes   | Migration API  |

### 3.3 Backwards Compatibility Patterns

#### Field Evolution
```json
// v1 Response
{
  "id": "node-123",
  "name": "function_name",
  "type": "function"
}

// v1.1 Response (Backwards Compatible)
{
  "id": "node-123",
  "name": "function_name",
  "type": "function",
  "signature": "def function_name(x: int) -> str"  // New field
}

// v2 Response (Breaking Change)
{
  "id": "node-123",
  "displayName": "function_name",  // Renamed field
  "nodeType": "function",          // Renamed field
  "signature": {                   // Restructured
    "name": "function_name",
    "parameters": [{"name": "x", "type": "int"}],
    "returns": "str"
  }
}
```

#### Deprecation Headers
```http
HTTP/1.1 200 OK
Deprecation: Sun, 01 Jan 2026 00:00:00 GMT
Sunset: Sun, 01 Jul 2026 00:00:00 GMT
Link: </api/v2/projects>; rel="successor-version"
Warning: 299 - "API version v1 is deprecated. Migrate to v2 by July 2026."
```

### 3.4 Migration Support

#### Migration Endpoint
```http
POST /api/v1/migrate
Content-Type: application/json

{
  "targetVersion": "v2",
  "resources": ["projects", "queries"],
  "dryRun": true
}

Response:
{
  "migrationId": "mig-789",
  "status": "planned",
  "changes": [
    {
      "resource": "projects",
      "type": "field_rename",
      "from": "name",
      "to": "displayName"
    }
  ],
  "estimatedDuration": "5m"
}
```

#### Version-aware Client Support
```http
GET /api/versions
Response:
{
  "current": "v1.1",
  "supported": ["v1.0", "v1.1"],
  "beta": ["v2.0"],
  "deprecated": [],
  "clientLibraries": {
    "rust": "0.1.2",
    "python": "0.2.1",
    "javascript": "1.0.5"
  }
}
```

---

## 4. Rate Limiting & Security Patterns

### 4.1 Multi-tier Rate Limiting

#### Global Rate Limiting
```http
HTTP/1.1 429 Too Many Requests
X-RateLimit-Limit: 10000
X-RateLimit-Remaining: 0
X-RateLimit-Reset: 1725890400
X-RateLimit-Retry-After: 60
Retry-After: 60
```

#### Endpoint-specific Limits
```yaml
rate_limits:
  /api/v1/projects:
    GET: 1000/hour
    POST: 100/hour
    PUT: 500/hour
    DELETE: 50/hour
  
  /api/v1/projects/{id}/search:
    POST: 100/minute
  
  /api/v1/projects/{id}/analyze:
    POST: 10/hour
  
  /api/v1/projects/{id}/stream:
    GET: 5/concurrent
```

#### User-tier Based Limits
```json
{
  "tiers": {
    "free": {
      "requests_per_hour": 1000,
      "concurrent_streams": 2,
      "max_project_size": "10MB",
      "features": ["basic_search", "graph_view"]
    },
    "pro": {
      "requests_per_hour": 10000,
      "concurrent_streams": 10,
      "max_project_size": "100MB",
      "features": ["semantic_search", "analytics", "webhooks"]
    },
    "enterprise": {
      "requests_per_hour": 100000,
      "concurrent_streams": 50,
      "max_project_size": "1GB",
      "features": ["custom_models", "priority_support", "sla"]
    }
  }
}
```

### 4.2 Authentication Patterns

#### JWT Bearer Token Authentication
```http
POST /api/v1/auth/login
Content-Type: application/json

{
  "username": "user@example.com",
  "password": "secure_password",
  "mfa_token": "123456"
}

Response:
{
  "access_token": "eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9...",
  "refresh_token": "rt_abc123def456...",
  "expires_in": 3600,
  "token_type": "Bearer",
  "scope": "read:projects write:projects admin:system"
}
```

#### API Key Authentication
```http
GET /api/v1/projects
Authorization: Bearer sk-codegraph-abc123def456...
X-API-Key-Name: production-analyzer
```

#### OAuth 2.0 Integration
```http
GET /api/v1/auth/oauth/github/authorize?
  client_id=codegraph_client&
  redirect_uri=https://app.codegraph.dev/auth/callback&
  scope=read:user,repo&
  state=random_state_string
```

### 4.3 Authorization Patterns

#### Role-Based Access Control (RBAC)
```json
{
  "roles": {
    "viewer": {
      "permissions": ["read:projects", "read:graph", "search:basic"]
    },
    "contributor": {
      "inherits": ["viewer"],
      "permissions": ["write:files", "create:entities"]
    },
    "admin": {
      "inherits": ["contributor"],
      "permissions": ["admin:projects", "delete:projects", "manage:users"]
    }
  },
  "project_permissions": {
    "owner": "all",
    "collaborator": ["read", "write"],
    "viewer": ["read"]
  }
}
```

#### Resource-based Permissions
```http
GET /api/v1/projects/123/graph
Authorization: Bearer jwt_token

# JWT payload:
{
  "sub": "user-456",
  "permissions": {
    "projects": {
      "123": ["read", "write"],
      "456": ["read"]
    },
    "features": ["semantic_search", "analytics"]
  }
}
```

### 4.4 Security Headers & Policies

#### Security Headers
```http
HTTP/2 200 OK
Strict-Transport-Security: max-age=31536000; includeSubDomains; preload
Content-Security-Policy: default-src 'self'; script-src 'self' 'unsafe-inline'
X-Frame-Options: DENY
X-Content-Type-Options: nosniff
X-XSS-Protection: 1; mode=block
Referrer-Policy: strict-origin-when-cross-origin
Permissions-Policy: geolocation=(), microphone=(), camera=()
```

#### CORS Configuration
```yaml
cors:
  allowed_origins:
    - https://app.codegraph.dev
    - https://dashboard.codegraph.dev
  allowed_methods: [GET, POST, PUT, DELETE, OPTIONS]
  allowed_headers:
    - Authorization
    - Content-Type
    - X-API-Key-Name
    - X-Request-ID
  exposed_headers:
    - X-RateLimit-*
    - X-Request-ID
  max_age: 86400
  allow_credentials: true
```

### 4.5 Input Validation & Sanitization

#### Request Validation Schema
```json
{
  "endpoints": {
    "POST /api/v1/projects": {
      "body": {
        "type": "object",
        "required": ["name", "rootPath"],
        "properties": {
          "name": {
            "type": "string",
            "minLength": 1,
            "maxLength": 100,
            "pattern": "^[a-zA-Z0-9_-]+$"
          },
          "rootPath": {
            "type": "string",
            "format": "path"
          },
          "language": {
            "type": "string",
            "enum": ["python", "javascript", "typescript", "rust", "auto"]
          }
        }
      }
    }
  }
}
```

#### Path Parameter Validation
```rust
// Example validation patterns
path_params: {
  "projectId": r"^[a-zA-Z0-9_-]{8,64}$",
  "nodeId": r"^node-[a-f0-9]{8}-[a-f0-9]{4}-4[a-f0-9]{3}-[89aAbB][a-f0-9]{3}-[a-f0-9]{12}$",
  "fileId": r"^file-[a-zA-Z0-9_-]{16,}$"
}
```

---

## 5. Comprehensive API Specification

### 5.1 OpenAPI 3.0 Specification Header

```yaml
openapi: 3.0.3
info:
  title: CodeGraph API
  description: High-performance code intelligence and analysis platform
  version: 1.0.0
  contact:
    name: CodeGraph API Support
    url: https://docs.codegraph.dev
    email: api-support@codegraph.dev
  license:
    name: MIT
    url: https://opensource.org/licenses/MIT

servers:
  - url: https://api.codegraph.dev/v1
    description: Production server
  - url: https://staging-api.codegraph.dev/v1  
    description: Staging server
  - url: http://localhost:8080/v1
    description: Development server

security:
  - BearerAuth: []
  - ApiKeyAuth: []

components:
  securitySchemes:
    BearerAuth:
      type: http
      scheme: bearer
      bearerFormat: JWT
    ApiKeyAuth:
      type: apiKey
      in: header
      name: X-API-Key
```

### 5.2 Core Endpoint Specifications

#### Project Management Endpoints

```yaml
paths:
  /projects:
    get:
      summary: List projects
      description: Retrieve a paginated list of projects accessible to the authenticated user
      parameters:
        - name: limit
          in: query
          schema:
            type: integer
            minimum: 1
            maximum: 100
            default: 20
        - name: offset
          in: query  
          schema:
            type: integer
            minimum: 0
            default: 0
        - name: sort
          in: query
          schema:
            type: string
            enum: [name, created_at, updated_at, size]
            default: updated_at
        - name: order
          in: query
          schema:
            type: string
            enum: [asc, desc]
            default: desc
        - name: status
          in: query
          schema:
            type: string
            enum: [analyzing, ready, error]
        - name: language
          in: query
          schema:
            type: string
            enum: [python, javascript, typescript, rust]
      responses:
        200:
          description: Successfully retrieved projects
          headers:
            X-Total-Count:
              schema:
                type: integer
              description: Total number of projects
            X-RateLimit-Remaining:
              schema:
                type: integer
          content:
            application/json:
              schema:
                type: object
                properties:
                  projects:
                    type: array
                    items:
                      $ref: '#/components/schemas/Project'
                  pagination:
                    $ref: '#/components/schemas/Pagination'
        401:
          $ref: '#/components/responses/Unauthorized'
        429:
          $ref: '#/components/responses/RateLimited'

    post:
      summary: Create project
      description: Create a new project for code analysis
      requestBody:
        required: true
        content:
          application/json:
            schema:
              $ref: '#/components/schemas/ProjectCreate'
      responses:
        201:
          description: Project created successfully
          content:
            application/json:
              schema:
                $ref: '#/components/schemas/Project'
        400:
          $ref: '#/components/responses/BadRequest'
        409:
          description: Project with this name already exists
        422:
          $ref: '#/components/responses/ValidationError'

  /projects/{projectId}:
    parameters:
      - name: projectId
        in: path
        required: true
        schema:
          type: string
          pattern: '^[a-zA-Z0-9_-]{8,64}$'
    
    get:
      summary: Get project details
      responses:
        200:
          description: Project details retrieved
          content:
            application/json:
              schema:
                $ref: '#/components/schemas/ProjectDetailed'
        404:
          $ref: '#/components/responses/NotFound'
    
    put:
      summary: Update project
      requestBody:
        content:
          application/json:
            schema:
              $ref: '#/components/schemas/ProjectUpdate'
      responses:
        200:
          description: Project updated successfully
          content:
            application/json:
              schema:
                $ref: '#/components/schemas/Project'
    
    delete:
      summary: Delete project
      responses:
        204:
          description: Project deleted successfully
        404:
          $ref: '#/components/responses/NotFound'
        409:
          description: Cannot delete project with active analysis

  /projects/{projectId}/analyze:
    post:
      summary: Start project analysis
      description: Trigger a full or incremental analysis of the project
      requestBody:
        content:
          application/json:
            schema:
              type: object
              properties:
                type:
                  type: string
                  enum: [full, incremental]
                  default: incremental
                force:
                  type: boolean
                  default: false
                  description: Force analysis even if project is already being analyzed
                options:
                  type: object
                  properties:
                    includeTests:
                      type: boolean
                      default: false
                    includeComments:
                      type: boolean
                      default: true
                    generateEmbeddings:
                      type: boolean
                      default: true
      responses:
        202:
          description: Analysis started
          content:
            application/json:
              schema:
                type: object
                properties:
                  analysisId:
                    type: string
                  status:
                    type: string
                    enum: [queued, running]
                  estimatedDuration:
                    type: string
                    example: "2m30s"
        409:
          description: Project is already being analyzed
```

#### Graph Operations Endpoints

```yaml
  /projects/{projectId}/graph/traverse:
    post:
      summary: Traverse graph
      description: Perform graph traversal with filtering and pagination
      requestBody:
        content:
          application/json:
            schema:
              type: object
              required: [startNodes, direction]
              properties:
                startNodes:
                  type: array
                  items:
                    type: string
                  minItems: 1
                  maxItems: 100
                direction:
                  type: string
                  enum: [outbound, inbound, bidirectional]
                maxDepth:
                  type: integer
                  minimum: 1
                  maximum: 10
                  default: 3
                edgeTypes:
                  type: array
                  items:
                    type: string
                    enum: [calls, imports, extends, implements, references, contains]
                nodeTypes:
                  type: array
                  items:
                    type: string
                    enum: [function, class, variable, module, import]
                filters:
                  type: object
                  properties:
                    minConfidence:
                      type: number
                      minimum: 0
                      maximum: 1
                    languages:
                      type: array
                      items:
                        type: string
                    pathPatterns:
                      type: array
                      items:
                        type: string
                limit:
                  type: integer
                  minimum: 1
                  maximum: 10000
                  default: 1000
      responses:
        200:
          description: Traversal results
          content:
            application/json:
              schema:
                type: object
                properties:
                  nodes:
                    type: array
                    items:
                      $ref: '#/components/schemas/Node'
                  edges:
                    type: array
                    items:
                      $ref: '#/components/schemas/Edge'
                  traversalStats:
                    type: object
                    properties:
                      totalNodes:
                        type: integer
                      totalEdges:
                        type: integer
                      maxDepthReached:
                        type: integer
                      executionTime:
                        type: string
                        example: "45ms"

  /projects/{projectId}/search/semantic:
    post:
      summary: Semantic search
      description: Perform semantic search using vector embeddings
      requestBody:
        content:
          application/json:
            schema:
              type: object
              required: [query]
              properties:
                query:
                  type: string
                  minLength: 1
                  maxLength: 1000
                limit:
                  type: integer
                  minimum: 1
                  maximum: 100
                  default: 20
                similarityThreshold:
                  type: number
                  minimum: 0
                  maximum: 1
                  default: 0.7
                filters:
                  type: object
                  properties:
                    nodeTypes:
                      type: array
                      items:
                        type: string
                    languages:
                      type: array
                      items:
                        type: string
                    filePatterns:
                      type: array
                      items:
                        type: string
                includeCode:
                  type: boolean
                  default: true
                includeComments:
                  type: boolean
                  default: true
      responses:
        200:
          description: Search results
          content:
            application/json:
              schema:
                type: object
                properties:
                  results:
                    type: array
                    items:
                      type: object
                      properties:
                        node:
                          $ref: '#/components/schemas/Node'
                        similarity:
                          type: number
                        snippet:
                          type: string
                        context:
                          type: object
                          properties:
                            surrounding:
                              type: string
                            file:
                              type: string
                            line:
                              type: integer
                  query:
                    type: string
                  executionTime:
                    type: string
                  totalResults:
                    type: integer
```

### 5.3 Data Schemas

```yaml
components:
  schemas:
    Project:
      type: object
      required: [id, name, status, createdAt, updatedAt]
      properties:
        id:
          type: string
          pattern: '^proj-[a-f0-9]{32}$'
          example: "proj-a1b2c3d4e5f6789012345678901234567"
        name:
          type: string
          minLength: 1
          maxLength: 100
          example: "my-awesome-project"
        description:
          type: string
          maxLength: 500
        language:
          type: string
          enum: [python, javascript, typescript, rust, auto]
        rootPath:
          type: string
          example: "/path/to/project"
        status:
          type: string
          enum: [analyzing, ready, error]
        createdAt:
          type: string
          format: date-time
        updatedAt:
          type: string
          format: date-time
        metrics:
          $ref: '#/components/schemas/ProjectMetrics'

    ProjectMetrics:
      type: object
      properties:
        fileCount:
          type: integer
          minimum: 0
        nodeCount:
          type: integer
          minimum: 0
        edgeCount:
          type: integer
          minimum: 0
        linesOfCode:
          type: integer
          minimum: 0
        lastAnalyzed:
          type: string
          format: date-time
        analysisTimeMs:
          type: integer
          minimum: 0

    Node:
      type: object
      required: [id, type, name, file, location]
      properties:
        id:
          type: string
          pattern: '^node-[a-f0-9]{8}-[a-f0-9]{4}-4[a-f0-9]{3}-[89aAbB][a-f0-9]{3}-[a-f0-9]{12}$'
        type:
          type: string
          enum: [function, class, variable, module, import, interface, type, constant]
        name:
          type: string
        qualifiedName:
          type: string
        file:
          $ref: '#/components/schemas/FileReference'
        location:
          $ref: '#/components/schemas/SourceLocation'
        metadata:
          type: object
          properties:
            visibility:
              type: string
              enum: [public, private, protected]
            isAsync:
              type: boolean
            isStatic:
              type: boolean
            parameters:
              type: array
              items:
                $ref: '#/components/schemas/Parameter'
            returnType:
              type: string
            complexity:
              type: integer
              minimum: 1
            docstring:
              type: string
        embeddings:
          type: object
          properties:
            semantic:
              type: array
              items:
                type: number
            structural:
              type: array
              items:
                type: number

    Edge:
      type: object
      required: [id, type, source, target]
      properties:
        id:
          type: string
          pattern: '^edge-[a-f0-9]{32}$'
        type:
          type: string
          enum: [calls, imports, extends, implements, references, contains, declares, uses]
        source:
          type: string
          description: Source node ID
        target:
          type: string
          description: Target node ID
        weight:
          type: number
          minimum: 0
          maximum: 1
          default: 1.0
        properties:
          type: object
          properties:
            confidence:
              type: number
              minimum: 0
              maximum: 1
            context:
              type: string
            lineNumber:
              type: integer
              minimum: 1
        createdAt:
          type: string
          format: date-time

    Error:
      type: object
      required: [error, message]
      properties:
        error:
          type: string
        message:
          type: string
        details:
          type: object
        requestId:
          type: string
        timestamp:
          type: string
          format: date-time

  responses:
    BadRequest:
      description: Bad request
      content:
        application/json:
          schema:
            $ref: '#/components/schemas/Error'
    Unauthorized:
      description: Unauthorized
      content:
        application/json:
          schema:
            allOf:
              - $ref: '#/components/schemas/Error'
              - type: object
                properties:
                  error:
                    example: "unauthorized"
                  message:
                    example: "Invalid or missing authentication credentials"
    NotFound:
      description: Resource not found
      content:
        application/json:
          schema:
            $ref: '#/components/schemas/Error'
    RateLimited:
      description: Too many requests
      headers:
        X-RateLimit-Limit:
          schema:
            type: integer
        X-RateLimit-Remaining:
          schema:
            type: integer
        X-RateLimit-Reset:
          schema:
            type: integer
        Retry-After:
          schema:
            type: integer
      content:
        application/json:
          schema:
            $ref: '#/components/schemas/Error'
```

---

## 6. Performance Benchmarks & Testing

### 6.1 Performance Requirements

#### Latency Targets
```yaml
performance_targets:
  p50: 10ms    # 50th percentile
  p90: 25ms    # 90th percentile
  p95: 35ms    # 95th percentile
  p99: 50ms    # 99th percentile
  p99.9: 100ms # 99.9th percentile

endpoint_specific:
  "GET /projects": 5ms
  "POST /projects/{id}/analyze": 50ms
  "POST /projects/{id}/search/semantic": 30ms
  "POST /projects/{id}/graph/traverse": 40ms
  "GET /projects/{id}/stream": 2ms (connection establishment)
```

#### Throughput Targets
```yaml
throughput_targets:
  sustained_rps: 10000    # Sustained requests per second
  peak_rps: 25000         # Peak requests per second
  concurrent_users: 1000  # Concurrent active users
  concurrent_streams: 5000 # Concurrent SSE connections

resource_usage:
  cpu_utilization: <80%
  memory_usage: <500MB
  disk_io: <100MB/s
  network_io: <1GB/s
```

### 6.2 Load Testing Scenarios

#### Scenario 1: Normal Operations
```yaml
load_test_normal:
  duration: 10m
  users: 500
  ramp_up: 2m
  requests:
    - endpoint: "GET /projects"
      weight: 30%
      think_time: 1s
    - endpoint: "GET /projects/{id}"
      weight: 25%
      think_time: 0.5s
    - endpoint: "POST /projects/{id}/search/text"
      weight: 20%
      think_time: 2s
    - endpoint: "POST /projects/{id}/graph/traverse"
      weight: 15%
      think_time: 3s
    - endpoint: "POST /projects/{id}/search/semantic"
      weight: 10%
      think_time: 5s
```

#### Scenario 2: Spike Testing
```yaml
load_test_spike:
  duration: 5m
  spike_users: 2000
  baseline_users: 200
  spike_duration: 30s
  spike_interval: 60s
```

#### Scenario 3: Analysis Workload
```yaml
load_test_analysis:
  duration: 30m
  concurrent_analyses: 10
  project_sizes:
    small: 1000    # files
    medium: 10000  # files  
    large: 50000   # files
  analysis_types:
    - full: 20%
    - incremental: 80%
```

### 6.3 Benchmarking Framework

#### Performance Test Suite
```rust
// Example benchmark structure
#[cfg(test)]
mod benchmarks {
    use criterion::{black_box, criterion_group, criterion_main, Criterion};
    use codegraph_api::*;

    fn bench_project_search(c: &mut Criterion) {
        let client = setup_test_client();
        let project_id = create_test_project(&client);
        
        c.bench_function("semantic_search", |b| {
            b.iter(|| {
                client.search_semantic(black_box(&project_id), black_box("test query"))
            })
        });
    }

    fn bench_graph_traversal(c: &mut Criterion) {
        let client = setup_test_client();
        let project_id = create_test_project(&client);
        
        c.benchmark_group("graph_traversal")
            .throughput(Throughput::Elements(1000))
            .bench_function("depth_3", |b| {
                b.iter(|| {
                    client.graph_traverse(
                        black_box(&project_id),
                        black_box(&TraversalRequest {
                            start_nodes: vec!["node-123".to_string()],
                            direction: Direction::Outbound,
                            max_depth: 3,
                            ..Default::default()
                        })
                    )
                })
            });
    }

    criterion_group!(benches, bench_project_search, bench_graph_traversal);
    criterion_main!(benches);
}
```

#### Continuous Performance Monitoring
```yaml
performance_monitoring:
  metrics_collection:
    interval: 1s
    retention: 30d
    
  alerts:
    - name: "High Latency"
      condition: "p95 > 50ms for 2m"
      severity: warning
      
    - name: "Critical Latency"
      condition: "p99 > 100ms for 1m"
      severity: critical
      
    - name: "Low Throughput"
      condition: "rps < 5000 for 5m"
      severity: warning
      
    - name: "High Error Rate"
      condition: "error_rate > 1% for 2m"
      severity: critical

  performance_budgets:
    bundle_size: 50MB
    startup_time: 100ms
    memory_baseline: 100MB
    memory_per_project: 5MB
```

### 6.4 Optimization Strategies

#### Caching Strategy
```yaml
caching:
  levels:
    - L1: "In-memory (Redis)"
      ttl: 5m
      capacity: 1GB
      hit_ratio_target: 95%
      
    - L2: "Distributed cache (Redis Cluster)"
      ttl: 1h
      capacity: 10GB
      hit_ratio_target: 80%
      
    - L3: "CDN (CloudFront)"
      ttl: 24h
      capacity: unlimited
      hit_ratio_target: 60%

  cache_keys:
    project_metadata: "project:{id}:meta"
    search_results: "search:{project_id}:{query_hash}"
    graph_data: "graph:{project_id}:{version}"
    embeddings: "embed:{node_id}:{model_version}"

  invalidation_strategy:
    - trigger: "project_update"
      invalidate: ["project:{id}:*", "graph:{id}:*"]
    - trigger: "node_update" 
      invalidate: ["embed:{node_id}:*", "search:*"]
```

#### Database Optimization
```yaml
database:
  connection_pool:
    min_connections: 5
    max_connections: 100
    idle_timeout: 300s
    max_lifetime: 3600s
    
  read_replicas: 3
  write_strategy: "primary_only"
  read_strategy: "round_robin"
  
  indexing:
    - table: "nodes"
      columns: ["project_id", "type", "name"]
      type: "btree"
    - table: "edges" 
      columns: ["source", "target", "type"]
      type: "btree"
    - table: "embeddings"
      columns: ["node_id"]
      type: "hash"

  partitioning:
    - table: "nodes"
      strategy: "hash"
      key: "project_id"
      partitions: 16
    - table: "embeddings"
      strategy: "range" 
      key: "created_at"
      interval: "1 month"
```

---

This comprehensive REST API architecture specification provides a robust foundation for the CodeGraph high-performance code intelligence system, with detailed patterns for resource design, HTTP/2 streaming, versioning, security, and performance optimization.