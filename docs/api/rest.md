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

# REST API Reference

**Complete HTTP endpoint documentation for CodeGraph API**

## Base URL

- **Production**: `https://api.codegraph.dev`
- **Development**: `http://localhost:8000`

## Authentication

Include your API key in requests:

```bash
curl -H "Authorization: Bearer YOUR_API_KEY" \
     -H "Content-Type: application/json" \
     https://api.codegraph.dev/api/v1/health
```

## Health & Status

### Health Check

**GET** `/health`

Basic health check endpoint.

**Response:**
```json
{
  "status": "healthy",
  "version": "0.1.0",
  "uptime": "24h 30m 15s",
  "features": ["graph", "vector", "mcp"]
}
```

### Detailed Health Check

**GET** `/api/v1/health`

Comprehensive system health with component status.

**Response:**
```json
{
  "status": "healthy",
  "timestamp": "2024-09-10T12:00:00Z",
  "version": "0.1.0",
  "uptime": "24h 30m 15s",
  "components": {
    "database": {
      "status": "healthy",
      "response_time": "2ms",
      "connections": {
        "active": 15,
        "max": 100
      }
    },
    "vector_search": {
      "status": "healthy", 
      "index_size": "1.2GB",
      "documents": 125000
    },
    "memory": {
      "used": "512MB",
      "available": "1.5GB",
      "usage_percent": 25.6
    }
  }
}
```

## Project Management

### List Projects

**GET** `/api/v1/projects`

List all projects with pagination.

**Query Parameters:**
| Parameter | Type | Description | Default |
|-----------|------|-------------|---------|
| `page` | integer | Page number (1-based) | 1 |
| `limit` | integer | Items per page (1-100) | 20 |
| `sort` | string | Sort field (`name`, `created_at`, `updated_at`) | `created_at` |
| `order` | string | Sort order (`asc`, `desc`) | `desc` |
| `search` | string | Search projects by name | - |

**Example:**
```bash
curl "https://api.codegraph.dev/api/v1/projects?page=1&limit=10&sort=name&order=asc"
```

**Response:**
```json
{
  "data": [
    {
      "id": "proj_abc123",
      "name": "my-rust-project",
      "description": "Production Rust API",
      "path": "/projects/my-rust-project",
      "languages": ["rust", "toml"],
      "status": "indexed",
      "created_at": "2024-09-01T10:00:00Z",
      "updated_at": "2024-09-10T12:00:00Z",
      "stats": {
        "files": 150,
        "lines_of_code": 25000,
        "functions": 500,
        "structs": 120
      }
    }
  ],
  "meta": {
    "page": 1,
    "limit": 10,
    "total": 25,
    "total_pages": 3,
    "has_next": true,
    "has_previous": false
  }
}
```

### Create Project

**POST** `/api/v1/projects`

Create a new project for analysis.

**Request Body:**
```json
{
  "name": "my-project",
  "description": "Optional project description",
  "path": "/path/to/codebase",
  "languages": ["rust", "python", "javascript"],
  "ignore_patterns": [
    "target/",
    "node_modules/", 
    "*.pyc",
    "__pycache__/"
  ],
  "config": {
    "enable_vector_search": true,
    "embedding_model": "sentence-transformers",
    "chunk_size": 512,
    "overlap_size": 50
  }
}
```

**Response (201 Created):**
```json
{
  "data": {
    "id": "proj_abc123",
    "name": "my-project",
    "status": "initializing",
    "created_at": "2024-09-10T12:00:00Z",
    "indexing_job": {
      "id": "job_xyz789",
      "status": "queued",
      "estimated_time": "5m 30s"
    }
  }
}
```

### Get Project

**GET** `/api/v1/projects/{id}`

Retrieve project details by ID.

**Path Parameters:**
| Parameter | Type | Description |
|-----------|------|-------------|
| `id` | string | Project ID |

**Response:**
```json
{
  "data": {
    "id": "proj_abc123",
    "name": "my-rust-project",
    "description": "Production Rust API",
    "path": "/projects/my-rust-project", 
    "languages": ["rust", "toml"],
    "status": "indexed",
    "created_at": "2024-09-01T10:00:00Z",
    "updated_at": "2024-09-10T12:00:00Z",
    "config": {
      "enable_vector_search": true,
      "embedding_model": "sentence-transformers",
      "chunk_size": 512
    },
    "stats": {
      "files": 150,
      "lines_of_code": 25000,
      "functions": 500,
      "structs": 120,
      "last_analysis": "2024-09-10T10:30:00Z"
    },
    "health": {
      "index_status": "healthy",
      "last_error": null,
      "warning_count": 2
    }
  }
}
```

### Update Project

**PUT** `/api/v1/projects/{id}`

Update project configuration.

**Request Body (partial updates supported):**
```json
{
  "description": "Updated description",
  "ignore_patterns": ["target/", "*.tmp"],
  "config": {
    "chunk_size": 256
  }
}
```

**Response (200 OK):**
```json
{
  "data": {
    "id": "proj_abc123",
    "updated_at": "2024-09-10T12:15:00Z",
    "reindex_job": {
      "id": "job_reindex_456",
      "status": "queued"
    }
  }
}
```

### Delete Project

**DELETE** `/api/v1/projects/{id}`

Delete a project and all associated data.

**Query Parameters:**
| Parameter | Type | Description | Default |
|-----------|------|-------------|---------|
| `force` | boolean | Skip confirmation for non-empty projects | false |

**Response (202 Accepted):**
```json
{
  "data": {
    "deletion_job": {
      "id": "job_delete_789",
      "status": "scheduled",
      "estimated_time": "2m 0s"
    }
  }
}
```

## Code Search

### Text Search

**GET** `/api/v1/search`

Search code using text queries with advanced filtering.

**Query Parameters:**
| Parameter | Type | Description | Default |
|-----------|------|-------------|---------|
| `q` | string | Search query (required) | - |
| `project_id` | string | Limit to specific project | - |
| `language` | string | Filter by programming language | - |
| `file_type` | string | Filter by file extension | - |
| `entity_type` | string | Filter by code entity type | - |
| `limit` | integer | Maximum results (1-100) | 20 |
| `offset` | integer | Result offset for pagination | 0 |

**Example:**
```bash
curl "https://api.codegraph.dev/api/v1/search?q=async+function&project_id=proj_abc123&language=rust&limit=10"
```

**Response:**
```json
{
  "data": {
    "results": [
      {
        "id": "match_001",
        "score": 0.95,
        "file": "src/handlers.rs",
        "line": 42,
        "column": 5,
        "entity_type": "function",
        "entity_name": "process_request",
        "context": {
          "before": "// Handle incoming requests",
          "match": "pub async fn process_request(req: Request) -> Result<Response> {",
          "after": "    let user_id = extract_user_id(&req)?;"
        },
        "project": {
          "id": "proj_abc123",
          "name": "my-rust-project"
        }
      }
    ],
    "total": 45,
    "took": "15ms"
  }
}
```

### Vector Similarity Search

**POST** `/api/v1/similar`

Find similar code using semantic vector search.

**Request Body:**
```json
{
  "code": "async fn process_data(input: Vec<String>) -> Result<()>",
  "project_id": "proj_abc123",
  "threshold": 0.8,
  "limit": 10,
  "filters": {
    "language": "rust",
    "entity_types": ["function", "method"]
  }
}
```

**Response:**
```json
{
  "data": {
    "query_embedding": [0.123, -0.456, 0.789],
    "results": [
      {
        "id": "sim_001",
        "similarity": 0.92,
        "file": "src/processor.rs",
        "line": 85,
        "entity_type": "function",
        "entity_name": "handle_batch",
        "signature": "async fn handle_batch(items: Vec<Item>) -> Result<ProcessedBatch>",
        "context": "Process multiple items in batch",
        "project": {
          "id": "proj_abc123", 
          "name": "my-rust-project"
        }
      }
    ],
    "total": 8,
    "took": "25ms"
  }
}
```

## Graph Traversal

### Get Entity

**GET** `/api/v1/entities/{id}`

Retrieve a specific code entity with its relationships.

**Path Parameters:**
| Parameter | Type | Description |
|-----------|------|-------------|
| `id` | string | Entity ID |

**Query Parameters:**
| Parameter | Type | Description | Default |
|-----------|------|-------------|---------|
| `include_deps` | boolean | Include dependencies | false |
| `include_refs` | boolean | Include references | false |
| `depth` | integer | Relationship traversal depth | 1 |

**Response:**
```json
{
  "data": {
    "id": "entity_func_001",
    "name": "process_request",
    "type": "function",
    "language": "rust",
    "file": "src/handlers.rs",
    "line": 42,
    "column": 5,
    "signature": "pub async fn process_request(req: Request) -> Result<Response>",
    "documentation": "Processes incoming HTTP requests with validation",
    "complexity": {
      "cyclomatic": 5,
      "cognitive": 8
    },
    "dependencies": [
      {
        "id": "entity_func_002",
        "name": "extract_user_id",
        "type": "function",
        "relationship": "calls"
      }
    ],
    "references": [
      {
        "id": "entity_test_001",
        "name": "test_process_request",
        "type": "test_function",
        "relationship": "tested_by"
      }
    ]
  }
}
```

### Graph Query

**POST** `/api/v1/graph/query`

Execute complex graph traversals with custom patterns.

**Request Body:**
```json
{
  "start_entities": ["entity_func_001"],
  "pattern": {
    "match": "(start:Function)-[calls]->(dep:Function)",
    "where": "dep.complexity.cyclomatic > 10",
    "return": ["start.name", "dep.name", "dep.complexity"]
  },
  "limit": 50,
  "timeout": "30s"
}
```

**Response:**
```json
{
  "data": {
    "results": [
      {
        "start_name": "process_request",
        "dep_name": "complex_validation",
        "dep_complexity": {
          "cyclomatic": 15,
          "cognitive": 22
        }
      }
    ],
    "total": 12,
    "execution_time": "45ms"
  }
}
```

## Analysis Jobs

### List Jobs

**GET** `/api/v1/jobs`

List analysis and indexing jobs.

**Query Parameters:**
| Parameter | Type | Description | Default |
|-----------|------|-------------|---------|
| `status` | string | Filter by status | - |
| `type` | string | Filter by job type | - |
| `project_id` | string | Filter by project | - |
| `limit` | integer | Results per page | 20 |

**Response:**
```json
{
  "data": [
    {
      "id": "job_xyz789",
      "type": "project_index",
      "status": "running",
      "project_id": "proj_abc123",
      "created_at": "2024-09-10T12:00:00Z",
      "started_at": "2024-09-10T12:01:00Z",
      "progress": {
        "percent": 65,
        "current_file": "src/complex_module.rs",
        "processed_files": 98,
        "total_files": 150
      },
      "estimated_completion": "2024-09-10T12:08:00Z"
    }
  ]
}
```

### Get Job Status

**GET** `/api/v1/jobs/{id}`

Get detailed job status and progress.

**Response:**
```json
{
  "data": {
    "id": "job_xyz789",
    "type": "project_index",
    "status": "running",
    "project_id": "proj_abc123",
    "created_at": "2024-09-10T12:00:00Z",
    "started_at": "2024-09-10T12:01:00Z",
    "progress": {
      "percent": 65,
      "current_file": "src/complex_module.rs",
      "processed_files": 98,
      "total_files": 150,
      "files_per_second": 12.5
    },
    "stats": {
      "functions_found": 250,
      "structs_found": 45,
      "errors": 2,
      "warnings": 8
    },
    "logs": [
      {
        "timestamp": "2024-09-10T12:05:00Z",
        "level": "info",
        "message": "Processing module: complex_module.rs"
      }
    ]
  }
}
```

### Cancel Job

**DELETE** `/api/v1/jobs/{id}`

Cancel a running job.

**Response (202 Accepted):**
```json
{
  "data": {
    "id": "job_xyz789",
    "status": "canceling",
    "message": "Job cancellation requested"
  }
}
```

## Metrics & Statistics

### Project Statistics

**GET** `/api/v1/projects/{id}/stats`

Get comprehensive project metrics.

**Response:**
```json
{
  "data": {
    "project_id": "proj_abc123",
    "generated_at": "2024-09-10T12:00:00Z",
    "files": {
      "total": 150,
      "by_language": {
        "rust": 120,
        "toml": 15,
        "markdown": 10,
        "yaml": 5
      }
    },
    "code": {
      "total_lines": 25000,
      "code_lines": 18500,
      "comment_lines": 4500,
      "blank_lines": 2000
    },
    "entities": {
      "functions": 500,
      "structs": 120,
      "enums": 35,
      "traits": 25,
      "modules": 45
    },
    "complexity": {
      "average_cyclomatic": 3.2,
      "max_cyclomatic": 15,
      "average_cognitive": 5.8,
      "max_cognitive": 22
    },
    "dependencies": {
      "internal": 89,
      "external": 156,
      "depth": {
        "max": 8,
        "average": 3.5
      }
    },
    "quality": {
      "test_coverage": 85.2,
      "documentation_coverage": 78.9,
      "maintainability_index": 82.1
    }
  }
}
```

### System Metrics

**GET** `/api/v1/metrics`

Get system-wide metrics (Prometheus format).

**Response (text/plain):**
```
# HELP codegraph_requests_total Total number of requests
# TYPE codegraph_requests_total counter
codegraph_requests_total{method="GET",endpoint="/api/v1/projects"} 1250

# HELP codegraph_request_duration_seconds Request duration
# TYPE codegraph_request_duration_seconds histogram
codegraph_request_duration_seconds_bucket{le="0.1"} 950
codegraph_request_duration_seconds_bucket{le="0.5"} 1200
codegraph_request_duration_seconds_bucket{le="1.0"} 1245
codegraph_request_duration_seconds_bucket{le="+Inf"} 1250

# HELP codegraph_active_connections Active connections
# TYPE codegraph_active_connections gauge
codegraph_active_connections 45

# HELP codegraph_database_connections Database connections
# TYPE codegraph_database_connections gauge
codegraph_database_connections{pool="main",state="active"} 15
codegraph_database_connections{pool="main",state="idle"} 35
```

## Error Responses

All endpoints return consistent error responses following RFC 7807:

### 400 Bad Request
```json
{
  "type": "https://api.codegraph.dev/errors/validation",
  "title": "Validation Error",
  "status": 400,
  "detail": "Missing required field 'name'",
  "instance": "/api/v1/projects",
  "request_id": "req_1234567890",
  "errors": [
    {
      "field": "name",
      "message": "Field is required"
    }
  ]
}
```

### 401 Unauthorized
```json
{
  "type": "https://api.codegraph.dev/errors/authentication",
  "title": "Authentication Required", 
  "status": 401,
  "detail": "Invalid or missing API key",
  "instance": "/api/v1/projects",
  "request_id": "req_1234567890"
}
```

### 404 Not Found
```json
{
  "type": "https://api.codegraph.dev/errors/not-found",
  "title": "Resource Not Found",
  "status": 404,
  "detail": "Project with ID 'proj_invalid' not found",
  "instance": "/api/v1/projects/proj_invalid", 
  "request_id": "req_1234567890"
}
```

### 429 Too Many Requests
```json
{
  "type": "https://api.codegraph.dev/errors/rate-limit",
  "title": "Rate Limit Exceeded",
  "status": 429,
  "detail": "API rate limit exceeded. Try again in 60 seconds.",
  "instance": "/api/v1/search",
  "request_id": "req_1234567890",
  "retry_after": 60
}
```

### 500 Internal Server Error
```json
{
  "type": "https://api.codegraph.dev/errors/internal",
  "title": "Internal Server Error",
  "status": 500,
  "detail": "An unexpected error occurred. Please try again later.",
  "instance": "/api/v1/projects",
  "request_id": "req_1234567890"
}
```

## Client Examples

### Rust
```rust
use codegraph_client::{Client, ProjectConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = Client::new("https://api.codegraph.dev")
        .with_api_key("your-api-key");
    
    // Create project
    let project = client
        .create_project(ProjectConfig {
            name: "my-project".to_string(),
            path: "/path/to/code".into(),
            languages: vec!["rust".to_string()],
        })
        .await?;
    
    // Search code
    let results = client
        .search("async fn", &project.id)
        .limit(10)
        .await?;
    
    println!("Found {} matches", results.len());
    Ok(())
}
```

### Python
```python
import asyncio
from codegraph import CodeGraphClient

async def main():
    client = CodeGraphClient(
        base_url="https://api.codegraph.dev",
        api_key="your-api-key"
    )
    
    # Create project
    project = await client.create_project(
        name="my-python-project",
        path="/path/to/project",
        languages=["python"]
    )
    
    # Vector similarity search
    similar = await client.find_similar(
        code="def process_data(items):",
        project_id=project.id,
        threshold=0.8
    )
    
    print(f"Found {len(similar)} similar functions")

asyncio.run(main())
```

### JavaScript/TypeScript
```typescript
import { CodeGraphClient } from '@codegraph/client';

const client = new CodeGraphClient({
  baseUrl: 'https://api.codegraph.dev',
  apiKey: 'your-api-key'
});

async function main() {
  // Create project  
  const project = await client.projects.create({
    name: 'my-js-project',
    path: '/path/to/project',
    languages: ['javascript', 'typescript']
  });
  
  // Search code
  const results = await client.search({
    query: 'async function',
    projectId: project.id,
    limit: 10
  });
  
  console.log(`Found ${results.data.length} matches`);
}

main().catch(console.error);
```

### cURL Examples
```bash
# Create project
curl -X POST "https://api.codegraph.dev/api/v1/projects" \
  -H "Authorization: Bearer YOUR_API_KEY" \
  -H "Content-Type: application/json" \
  -d '{
    "name": "my-project",
    "path": "/path/to/code",
    "languages": ["rust", "python"]
  }'

# Search code
curl "https://api.codegraph.dev/api/v1/search?q=async+function&limit=5" \
  -H "Authorization: Bearer YOUR_API_KEY"

# Vector similarity search
curl -X POST "https://api.codegraph.dev/api/v1/similar" \
  -H "Authorization: Bearer YOUR_API_KEY" \
  -H "Content-Type: application/json" \
  -d '{
    "code": "async fn process_data(input: Vec<String>) -> Result<()>",
    "threshold": 0.8,
    "limit": 10
  }'
```