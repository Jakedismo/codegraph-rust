# CodeGraph MCP Server

A comprehensive Model Context Protocol (MCP) server implementation for multi-agent coordination, built with WebSocket transport and sophisticated agent orchestration capabilities.

## Features

### 🚀 Core Capabilities
- **Full MCP Protocol Compliance** - Implements MCP specification 2025-03-26
- **WebSocket Transport** - Real-time bidirectional communication with fallback support
- **Multi-Agent Coordination** - Sophisticated orchestration patterns for distributed agents
- **Advanced Message Routing** - Intelligent routing with load balancing and failover
- **Comprehensive Validation** - JSON-RPC 2.0 and custom schema validation
- **Production Ready** - Built for scalability, reliability, and performance

### 🤖 Agent System
- **Base Agent Framework** - Extensible foundation for custom agent implementations
- **Built-in Agent Types** - Coordinator, Analyzer, Transformer, Validator, Reporter
- **Dynamic Capabilities** - Runtime capability registration and discovery
- **Health Monitoring** - Automatic health checks and status tracking
- **Task Management** - Sophisticated task assignment and lifecycle management

### 🔀 Communication Patterns
- **Point-to-Point** - Direct agent communication
- **Publish-Subscribe** - Event-driven messaging
- **Broadcast** - One-to-many messaging
- **Pipeline** - Sequential processing chains
- **Scatter-Gather** - Parallel processing with result aggregation

### 🎯 Coordination Features
- **Workflow Orchestration** - Complex multi-step workflow execution
- **Consensus Algorithms** - Distributed decision making
- **Load Balancing** - Intelligent task distribution
- **Synchronization** - Barrier synchronization for coordinated execution
- **Conflict Resolution** - Automated conflict detection and resolution

## Quick Start

```bash
# Install dependencies
npm install

# Run development server
npm run dev

# Build and run production server
npm run build
npm start
```

## Architecture Overview

The CodeGraph MCP Server implements a sophisticated multi-agent coordination framework with:

1. **WebSocket Transport Layer** - Full-duplex real-time communication
2. **Message Router** - Intelligent routing with validation
3. **Coordination Engine** - Multi-agent orchestration patterns
4. **Agent SDK** - Extensible agent framework
5. **Protocol Compliance** - Full MCP specification support

## File Structure

```
src/
├── core/
│   ├── agents/                    # Agent framework and types
│   ├── coordination/              # Multi-agent coordination engine
│   ├── routing/                   # Message routing system
│   ├── transport/                 # WebSocket transport layer
│   └── validation/                # Message validation
├── server/                        # Main MCP server implementation
├── examples/                      # Example agents and demo server
└── README.md                      # This file
```

## Example Usage

The server provides MCP tools for agent coordination:

- `register_agent` - Register new agents with the coordination system
- `distribute_task` - Distribute tasks to specified agents
- `coordinate_agents` - Coordinate multiple agents for consensus or synchronization
- `get_agent_status` - Get the current status of agents

MCP resources:

- `agents` - List of all registered agents
- `tasks` - Active tasks and their status
- `metrics` - Server performance metrics

## GraphQL Schema Highlights

### Core Types
- `CodeNode` interface with implementations for Files, Functions, Classes, Variables
- `CodeRelation` for expressing relationships between nodes
- Rich metadata and location information
- Performance-optimized field resolvers

### Query Patterns
- **Graph Traversal**: `findPath`, `subgraph`, `neighbors`
- **Search & Discovery**: `searchNodes`, `batchAnalysis`  
- **Dependency Analysis**: `dependencyGraph`, `impactAnalysis`
- **Code Metrics**: `codeMetrics` with complexity analysis

### Real-time Subscriptions
- `codeChanged` - File and code structure changes
- `graphUpdated` - Relationship changes  
- `performanceAlert` - System performance monitoring
- `analysisProgress` - Long-running analysis updates

## Caching Strategy

### Multi-tier Architecture
1. **L1 Cache** - In-memory LRU (5 min TTL, 1000 items)
2. **L2 Cache** - Redis distributed cache (1 hour TTL, 10k items) 
3. **L3 Cache** - Persistent storage (24 hour TTL)

### Cache Policies
- `SHORT_TERM` - Frequently changing data
- `MEDIUM_TERM` - Stable code structures  
- `LONG_TERM` - Historical analysis results
- `PERSISTENT` - Immutable reference data

## Authentication & Authorization

### Permission System
- Granular permissions for different operation types
- Resource-level access control
- Organization and project-scoped data access
- Rate limiting by user tier and query complexity

### Security Features
- JWT token authentication with caching
- Permission hierarchy and inheritance
- Rate limiting with Redis backend
- Automatic token refresh and validation

## Performance Monitoring

### Real-time Metrics
- Query execution time breakdown
- Cache hit ratios and performance
- Resource utilization tracking  
- Error rate monitoring

### Alerting System
- Performance threshold violations
- Resource utilization alerts
- Error rate spike detection
- Automatic optimization suggestions

## Usage Examples

### Basic Node Query
```graphql
query GetFunction($id: ID!) {
  node(id: $id) {
    ... on FunctionNode {
      id
      name
      signature
      location {
        file
        startLine
        endLine
      }
    }
  }
}
```

### Graph Traversal
```graphql
query FindDependencies($rootId: ID!) {
  dependencyGraph(rootId: $rootId, maxDepth: 3) {
    graph {
      nodes {
        id
        name
        type
      }
      relations {
        type
        source { id }
        target { id }
      }
    }
    cycles {
      id
      name
    }
  }
}
```

### Real-time Subscription
```graphql
subscription WatchCodeChanges($filePattern: String) {
  codeChanged(filePattern: $filePattern) {
    type
    node {
      id
      name
      location {
        file
      }
    }
    timestamp
  }
}
```

### Batch Analysis
```graphql
query AnalyzeCodebase($requests: [AnalysisRequest!]!) {
  batchAnalysis(requests: $requests, cachePolicy: MEDIUM_TERM) {
    nodes {
      id
      type
    }
    metrics
    performance {
      queryTime
      nodesTraversed
      cacheHitRatio
    }
  }
}
```

## Implementation Guidelines

### Database Integration
The schema assumes integration with a graph database (Neo4j, Amazon Neptune) or optimized relational database with graph capabilities. Key considerations:

- Use connection pooling for database access
- Implement prepared statements for common queries
- Batch database operations when possible
- Index frequently queried relationships

### Deployment Considerations
- Use Redis cluster for distributed caching
- Implement horizontal scaling with load balancers
- Monitor memory usage for large graph operations  
- Configure connection limits and timeouts appropriately

### Development Setup
1. Install dependencies: `npm install`
2. Configure Redis connection
3. Set up database connections
4. Configure JWT secrets and authentication
5. Start GraphQL server with schema

## Contributing

When contributing to this API:

1. Maintain sub-50ms performance requirements
2. Add comprehensive performance monitoring
3. Include authentication checks for new operations
4. Update caching strategies as needed
5. Document new query patterns and optimizations

## Performance Testing

Run the included performance validation:

```typescript
import { performanceValidator } from './performance-specifications';

const results = await performanceValidator.runPerformanceTests();
console.log('Performance test results:', results);
```

The system includes automated performance testing that validates response times against targets and provides optimization recommendations.