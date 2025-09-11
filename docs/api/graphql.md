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

# GraphQL API Reference

**Type-safe, flexible queries for CodeGraph data**

## Endpoint

**POST** `/api/v1/graphql`

## Authentication

Include your API key in the Authorization header:

```http
POST /api/v1/graphql
Authorization: Bearer YOUR_API_KEY
Content-Type: application/json
```

## Interactive Playground

Explore the schema interactively at: `https://api.codegraph.dev/playground`

## Schema Overview

```graphql
type Query {
  # Project queries
  project(id: ID!): Project
  projects(filter: ProjectFilter, pagination: Pagination): ProjectConnection
  
  # Search queries  
  search(query: String!, filter: SearchFilter): SearchResult
  similarCode(code: String!, filter: SimilarityFilter): SimilarityResult
  
  # Entity queries
  entity(id: ID!): Entity
  entities(filter: EntityFilter, pagination: Pagination): EntityConnection
  
  # Graph traversal
  traverse(start: ID!, pattern: GraphPattern): TraversalResult
}

type Mutation {
  # Project mutations
  createProject(input: CreateProjectInput!): CreateProjectResult
  updateProject(id: ID!, input: UpdateProjectInput!): UpdateProjectResult
  deleteProject(id: ID!, force: Boolean = false): DeleteProjectResult
  
  # Indexing mutations
  reindexProject(id: ID!): IndexingJob
  cancelJob(id: ID!): CancelJobResult
}

type Subscription {
  # Project subscriptions
  projectUpdates(id: ID!): ProjectUpdate
  indexingProgress(jobId: ID!): IndexingProgress
  
  # Search subscriptions
  searchResults(query: String!): SearchResultUpdate
}
```

## Core Types

### Project
```graphql
type Project {
  id: ID!
  name: String!
  description: String
  path: String!
  languages: [String!]!
  status: ProjectStatus!
  createdAt: DateTime!
  updatedAt: DateTime!
  
  # Statistics
  stats: ProjectStats!
  
  # Configuration
  config: ProjectConfig!
  
  # Relationships
  entities(filter: EntityFilter, pagination: Pagination): EntityConnection
  indexingJobs(status: JobStatus): [IndexingJob!]!
}

enum ProjectStatus {
  INITIALIZING
  INDEXING  
  INDEXED
  ERROR
  PAUSED
}

type ProjectStats {
  files: Int!
  linesOfCode: Int!
  functions: Int!
  structs: Int!
  complexity: ComplexityStats!
  lastAnalyzed: DateTime
}

type ComplexityStats {
  averageCyclomatic: Float!
  maxCyclomatic: Int!
  averageCognitive: Float!
  maxCognitive: Int!
}
```

### Entity
```graphql
type Entity {
  id: ID!
  name: String!
  type: EntityType!
  language: String!
  
  # Location
  file: String!
  line: Int!
  column: Int!
  
  # Content
  signature: String
  documentation: String
  sourceCode: String
  
  # Metrics
  complexity: ComplexityMetrics
  
  # Relationships
  dependencies: [EntityDependency!]!
  dependents: [EntityDependency!]!
  references: [EntityReference!]!
  
  # Project context
  project: Project!
}

enum EntityType {
  FUNCTION
  METHOD
  STRUCT
  ENUM
  TRAIT
  MODULE
  CLASS
  INTERFACE
  VARIABLE
  CONSTANT
}

type EntityDependency {
  id: ID!
  entity: Entity!
  type: DependencyType!
  strength: Float!
}

enum DependencyType {
  CALLS
  IMPORTS
  EXTENDS
  IMPLEMENTS
  USES
  REFERENCES
}
```

## Queries

### Project Queries

#### Get Single Project
```graphql
query GetProject($id: ID!) {
  project(id: $id) {
    id
    name
    description
    status
    stats {
      files
      linesOfCode
      functions
      complexity {
        averageCyclomatic
        maxCyclomatic
      }
    }
    config {
      enableVectorSearch
      chunkSize
      embeddingModel
    }
  }
}
```

**Variables:**
```json
{
  "id": "proj_abc123"
}
```

#### List Projects with Filtering
```graphql
query ListProjects(
  $filter: ProjectFilter
  $pagination: Pagination
) {
  projects(filter: $filter, pagination: $pagination) {
    edges {
      node {
        id
        name
        status
        createdAt
        stats {
          files
          linesOfCode
        }
      }
    }
    pageInfo {
      hasNextPage
      hasPreviousPage
      startCursor
      endCursor
    }
    totalCount
  }
}
```

**Variables:**
```json
{
  "filter": {
    "status": ["INDEXED"],
    "languages": ["rust", "python"],
    "createdAfter": "2024-01-01T00:00:00Z"
  },
  "pagination": {
    "first": 20,
    "after": "cursor123"
  }
}
```

### Search Queries

#### Text Search
```graphql
query SearchCode(
  $query: String!
  $filter: SearchFilter
) {
  search(query: $query, filter: $filter) {
    results {
      id
      score
      entity {
        id
        name
        type
        file
        line
        signature
        project {
          id
          name
        }
      }
      context {
        before
        match
        after
      }
    }
    totalCount
    took
  }
}
```

**Variables:**
```json
{
  "query": "async function",
  "filter": {
    "projectIds": ["proj_abc123"],
    "languages": ["rust"],
    "entityTypes": ["FUNCTION", "METHOD"],
    "limit": 20
  }
}
```

#### Vector Similarity Search
```graphql
query FindSimilar(
  $code: String!
  $filter: SimilarityFilter
) {
  similarCode(code: $code, filter: $filter) {
    queryEmbedding
    results {
      id
      similarity
      entity {
        id
        name
        signature
        file
        line
        project {
          name
        }
      }
    }
    totalCount
    took
  }
}
```

**Variables:**
```json
{
  "code": "async fn process_data(input: Vec<String>) -> Result<()>",
  "filter": {
    "projectId": "proj_abc123",
    "threshold": 0.8,
    "limit": 10,
    "entityTypes": ["FUNCTION"]
  }
}
```

### Entity Queries

#### Get Entity with Dependencies
```graphql
query GetEntityDetails($id: ID!) {
  entity(id: $id) {
    id
    name
    type
    file
    line
    signature
    documentation
    complexity {
      cyclomatic
      cognitive
      maintainability
    }
    dependencies {
      id
      type
      strength
      entity {
        id
        name
        type
        file
      }
    }
    dependents {
      id
      type
      entity {
        id
        name
        file
      }
    }
    project {
      id
      name
    }
  }
}
```

#### Complex Entity Query
```graphql
query ComplexEntityAnalysis(
  $projectId: ID!
  $entityType: EntityType!
) {
  project(id: $projectId) {
    name
    entities(
      filter: { 
        types: [$entityType]
        complexityMin: 10
      }
      pagination: { first: 50 }
    ) {
      edges {
        node {
          id
          name
          complexity {
            cyclomatic
            cognitive
          }
          dependencies {
            id
            type
            entity {
              name
              complexity {
                cyclomatic
              }
            }
          }
        }
      }
    }
  }
}
```

### Graph Traversal

#### Find Call Chains
```graphql
query FindCallChains(
  $startEntity: ID!
  $maxDepth: Int = 5
) {
  traverse(
    start: $startEntity
    pattern: {
      match: "(start:Function)-[calls*1..5]->(end:Function)"
      where: "end.complexity.cyclomatic > 10"
      return: ["path", "start.name", "end.name", "end.complexity"]
    }
  ) {
    results {
      path {
        entities {
          id
          name
          type
        }
        relationships {
          type
          strength
        }
      }
      startName
      endName
      endComplexity {
        cyclomatic
        cognitive
      }
    }
    totalCount
    executionTime
  }
}
```

## Mutations

### Project Management

#### Create Project
```graphql
mutation CreateProject($input: CreateProjectInput!) {
  createProject(input: $input) {
    project {
      id
      name
      status
      createdAt
    }
    indexingJob {
      id
      status
      estimatedTime
    }
    errors {
      field
      message
    }
  }
}
```

**Variables:**
```json
{
  "input": {
    "name": "my-new-project",
    "description": "A new Rust project",
    "path": "/path/to/project",
    "languages": ["rust", "toml"],
    "ignorePatterns": ["target/", "*.tmp"],
    "config": {
      "enableVectorSearch": true,
      "chunkSize": 512,
      "embeddingModel": "sentence-transformers"
    }
  }
}
```

#### Update Project
```graphql
mutation UpdateProject(
  $id: ID!
  $input: UpdateProjectInput!
) {
  updateProject(id: $id, input: $input) {
    project {
      id
      updatedAt
      config {
        chunkSize
        embeddingModel
      }
    }
    reindexJob {
      id
      status
    }
    errors {
      field
      message
    }
  }
}
```

#### Delete Project
```graphql
mutation DeleteProject($id: ID!, $force: Boolean = false) {
  deleteProject(id: $id, force: $force) {
    success
    deletionJob {
      id
      status
      estimatedTime
    }
    errors {
      message
    }
  }
}
```

### Indexing Operations

#### Trigger Reindex
```graphql
mutation ReindexProject($id: ID!) {
  reindexProject(id: $id) {
    id
    type
    status
    createdAt
    estimatedTime
    config {
      fullIndex
      incrementalOnly
    }
  }
}
```

#### Cancel Job
```graphql
mutation CancelJob($id: ID!) {
  cancelJob(id: $id) {
    success
    job {
      id
      status
      canceledAt
    }
    message
  }
}
```

## Subscriptions

### Project Updates
```graphql
subscription ProjectUpdates($id: ID!) {
  projectUpdates(id: $id) {
    type
    timestamp
    project {
      id
      status
      updatedAt
    }
    data {
      ... on StatusChange {
        previousStatus
        newStatus
        reason
      }
      ... on StatsUpdate {
        stats {
          files
          linesOfCode
          functions
        }
      }
    }
  }
}
```

### Indexing Progress
```graphql
subscription IndexingProgress($jobId: ID!) {
  indexingProgress(jobId: $jobId) {
    jobId
    status
    progress {
      percent
      currentFile
      processedFiles
      totalFiles
      filesPerSecond
    }
    stats {
      functionsFound
      structsFound
      errors
      warnings
    }
    estimatedCompletion
  }
}
```

### Real-time Search
```graphql
subscription LiveSearch($query: String!) {
  searchResults(query: $query) {
    type
    timestamp
    results {
      id
      score
      entity {
        name
        file
        line
      }
      reason
    }
  }
}
```

## Error Handling

GraphQL errors follow the standard format with extensions for additional context:

```json
{
  "errors": [
    {
      "message": "Project not found",
      "locations": [{"line": 3, "column": 5}],
      "path": ["project"],
      "extensions": {
        "code": "NOT_FOUND",
        "type": "PROJECT_NOT_FOUND", 
        "requestId": "req_1234567890",
        "details": {
          "projectId": "proj_invalid"
        }
      }
    }
  ],
  "data": {
    "project": null
  }
}
```

### Error Codes

| Code | Description |
|------|-------------|
| `VALIDATION_ERROR` | Input validation failed |
| `NOT_FOUND` | Resource not found |
| `UNAUTHORIZED` | Authentication required |
| `FORBIDDEN` | Insufficient permissions |
| `RATE_LIMITED` | Rate limit exceeded |
| `INTERNAL_ERROR` | Server error |
| `TIMEOUT` | Operation timeout |

## Advanced Features

### Fragments

Define reusable fragments for complex queries:

```graphql
fragment EntityDetails on Entity {
  id
  name
  type
  file
  line
  signature
  complexity {
    cyclomatic
    cognitive
  }
  project {
    id
    name
  }
}

query SearchWithDetails($query: String!) {
  search(query: $query) {
    results {
      id
      score
      entity {
        ...EntityDetails
      }
    }
  }
}
```

### Directives

#### @include / @skip
```graphql
query ConditionalQuery(
  $includeComplexity: Boolean!
  $skipDependencies: Boolean!
) {
  entity(id: "entity_123") {
    id
    name
    complexity @include(if: $includeComplexity) {
      cyclomatic
      cognitive
    }
    dependencies @skip(if: $skipDependencies) {
      entity {
        name
      }
    }
  }
}
```

#### @cached (Custom)
```graphql
query CachedProjectStats($id: ID!) {
  project(id: $id) {
    stats @cached(ttl: 300) {
      files
      linesOfCode
      functions
    }
  }
}
```

### Batching

Execute multiple operations in a single request:

```json
[
  {
    "query": "query GetProject($id: ID!) { project(id: $id) { name status } }",
    "variables": {"id": "proj_123"}
  },
  {
    "query": "query GetProject($id: ID!) { project(id: $id) { name status } }",
    "variables": {"id": "proj_456"}
  }
]
```

## Client Examples

### JavaScript/TypeScript with Apollo Client
```typescript
import { ApolloClient, InMemoryCache, gql } from '@apollo/client';

const client = new ApolloClient({
  uri: 'https://api.codegraph.dev/api/v1/graphql',
  cache: new InMemoryCache(),
  headers: {
    authorization: 'Bearer YOUR_API_KEY'
  }
});

const GET_PROJECT = gql`
  query GetProject($id: ID!) {
    project(id: $id) {
      id
      name
      stats {
        files
        linesOfCode
        functions
      }
    }
  }
`;

const { data } = await client.query({
  query: GET_PROJECT,
  variables: { id: 'proj_abc123' }
});

console.log(data.project);
```

### Python with GQL
```python
from gql import gql, Client
from gql.transport.requests import RequestsHTTPTransport

transport = RequestsHTTPTransport(
    url="https://api.codegraph.dev/api/v1/graphql",
    headers={"Authorization": "Bearer YOUR_API_KEY"}
)

client = Client(transport=transport)

query = gql("""
    query GetProject($id: ID!) {
        project(id: $id) {
            id
            name
            stats {
                files
                linesOfCode
                functions
            }
        }
    }
""")

result = client.execute(query, variable_values={"id": "proj_abc123"})
print(result)
```

### Rust with graphql-client
```rust
use graphql_client::{GraphQLQuery, Response};
use reqwest::Client;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "schema.graphql",
    query_path = "get_project.graphql"
)]
struct GetProject;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = Client::new();
    
    let query = GetProject::build_query(get_project::Variables {
        id: "proj_abc123".to_string(),
    });
    
    let response = client
        .post("https://api.codegraph.dev/api/v1/graphql")
        .header("Authorization", "Bearer YOUR_API_KEY")
        .json(&query)
        .send()
        .await?;
    
    let response_body: Response<get_project::ResponseData> = response.json().await?;
    
    if let Some(data) = response_body.data {
        println!("Project: {:?}", data.project);
    }
    
    Ok(())
}
```

### cURL
```bash
curl -X POST "https://api.codegraph.dev/api/v1/graphql" \
  -H "Authorization: Bearer YOUR_API_KEY" \
  -H "Content-Type: application/json" \
  -d '{
    "query": "query GetProject($id: ID!) { project(id: $id) { id name stats { files linesOfCode } } }",
    "variables": {"id": "proj_abc123"}
  }'
```

## Schema Introspection

Query the schema itself:

```graphql
query SchemaIntrospection {
  __schema {
    types {
      name
      description
      fields {
        name
        type {
          name
        }
      }
    }
  }
}
```

Get available queries:
```graphql
query AvailableQueries {
  __schema {
    queryType {
      fields {
        name
        description
        args {
          name
          type {
            name
          }
        }
      }
    }
  }
}
```