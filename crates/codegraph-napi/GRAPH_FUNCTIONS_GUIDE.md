# CodeGraph Advanced Graph Analysis Functions

Comprehensive guide to using CodeGraph's advanced graph analysis functions for architectural analysis, dependency tracking, and code quality metrics.

## Table of Contents

- [Overview](#overview)
- [Setup](#setup)
- [Function Reference](#function-reference)
- [Use Cases](#use-cases)
- [Integration Patterns](#integration-patterns)
- [Troubleshooting](#troubleshooting)

## Overview

CodeGraph provides six powerful graph analysis functions that enable deep insights into your codebase structure:

1. **getTransitiveDependencies** - Find all downstream dependencies
2. **getReverseDependencies** - Find all upstream dependents
3. **detectCircularDependencies** - Identify circular dependency cycles
4. **traceCallChain** - Map function call hierarchies
5. **calculateCouplingMetrics** - Measure component coupling
6. **getHubNodes** - Find highly connected architectural nodes

### When to Use These Functions

**Use these functions for:**
- Architectural analysis and refactoring planning
- Impact analysis before code changes
- Code quality metrics and health monitoring
- Dependency visualization and documentation
- CI/CD quality gates
- Code review automation

**Requirements:**
- SurrealDB instance running and accessible
- CodeGraph data indexed in SurrealDB
- `cloud-surrealdb` feature enabled (default in builds)

## Setup

### 1. Install SurrealDB

```bash
# macOS
brew install surrealdb/tap/surreal

# Linux
curl -sSf https://install.surrealdb.com | sh

# Windows
iwr https://install.surrealdb.com -useb | iex
```

### 2. Start SurrealDB

```bash
# In-memory (development)
surreal start memory

# File-based (persistent)
surreal start --log trace --user root --pass root file://data/codegraph.db

# Production with authentication
surreal start --bind 0.0.0.0:8000 --user admin --pass secure-password file://data/codegraph.db
```

### 3. Configure Connection

Set the environment variable before running your Node.js application:

```bash
export SURREALDB_CONNECTION="ws://localhost:8000"

# With authentication
export SURREALDB_CONNECTION="ws://admin:secure-password@localhost:8000"
```

### 4. Verify Setup

```typescript
import { getHubNodes } from 'codegraph-napi';

try {
  const hubs = await getHubNodes(1);
  console.log('Connection successful!');
} catch (error) {
  console.error('Connection failed:', error.message);
}
```

## Function Reference

### 1. getTransitiveDependencies

**Purpose:** Find all dependencies of a node up to a specified depth, answering "what does this depend on?"

**Signature:**
```typescript
async function getTransitiveDependencies(
  nodeId: string,
  edgeType: string,
  depth?: number
): Promise<DependencyNode[]>
```

**Parameters:**
- `nodeId` - The node ID to analyze (e.g., `"node:function-uuid"`)
- `edgeType` - Type of edge to traverse (e.g., `"Calls"`, `"Imports"`, `"Uses"`)
- `depth` - Maximum traversal depth (default: 3)

**Returns:** Array of dependency nodes with depth information

**Example:**
```typescript
import { getTransitiveDependencies } from 'codegraph-napi';

// Find all functions called by 'processPayment'
const deps = await getTransitiveDependencies(
  'node:process-payment-fn',
  'Calls',
  3
);

console.log(`processPayment depends on ${deps.length} functions:`);
deps.forEach(dep => {
  const indent = '  '.repeat(dep.dependencyDepth || 0);
  console.log(`${indent}[${dep.dependencyDepth}] ${dep.name}`);
  if (dep.location) {
    console.log(`${indent}  ${dep.location.filePath}:${dep.location.startLine}`);
  }
});
```

**Common Patterns:**

```typescript
// Analyze import dependencies
const imports = await getTransitiveDependencies(
  'node:module-id',
  'Imports',
  5
);

// Group by depth
const byDepth = new Map();
deps.forEach(d => {
  const depth = d.dependencyDepth || 0;
  if (!byDepth.has(depth)) byDepth.set(depth, []);
  byDepth.get(depth).push(d.name);
});

console.log('Dependencies by depth:');
byDepth.forEach((nodes, depth) => {
  console.log(`  Depth ${depth}: ${nodes.length} nodes`);
});
```

### 2. getReverseDependencies

**Purpose:** Find all dependents of a node - who depends on this? Critical for impact analysis.

**Signature:**
```typescript
async function getReverseDependencies(
  nodeId: string,
  edgeType: string,
  depth?: number
): Promise<DependencyNode[]>
```

**Parameters:**
- `nodeId` - The node ID to analyze
- `edgeType` - Type of edge to traverse backwards
- `depth` - Maximum traversal depth (default: 3)

**Returns:** Array of dependent nodes with depth information

**Example:**
```typescript
import { getReverseDependencies } from 'codegraph-napi';

// Find who calls the 'validateUser' function
const dependents = await getReverseDependencies(
  'node:validate-user-fn',
  'Calls',
  2
);

console.log(`validateUser is called by ${dependents.length} functions:`);
dependents.forEach(dep => {
  console.log(`  ${dep.name} (depth: ${dep.dependentDepth})`);
  if (dep.location) {
    console.log(`    ${dep.location.filePath}:${dep.location.startLine}`);
  }
});
```

**Use Case: Impact Analysis Before Refactoring**

```typescript
async function analyzeImpact(functionId: string) {
  const dependents = await getReverseDependencies(functionId, 'Calls', 5);

  console.log('\nImpact Analysis Report');
  console.log('======================');

  // Count affected files
  const affectedFiles = new Set(
    dependents
      .filter(d => d.location)
      .map(d => d.location!.filePath)
  );

  console.log(`\nAffected functions: ${dependents.length}`);
  console.log(`Affected files: ${affectedFiles.size}`);

  // Group by depth to show impact radius
  const byDepth = new Map();
  dependents.forEach(d => {
    const depth = d.dependentDepth || 0;
    if (!byDepth.has(depth)) byDepth.set(depth, []);
    byDepth.get(depth).push(d);
  });

  console.log('\nImpact radius:');
  byDepth.forEach((nodes, depth) => {
    console.log(`  ${depth} steps away: ${nodes.length} functions`);
  });

  // Identify critical paths (public APIs at depth 0)
  const directDependents = byDepth.get(0) || [];
  if (directDependents.length > 10) {
    console.warn(`\nWarning: ${directDependents.length} direct dependents!`);
    console.warn('Consider deprecation strategy before changes.');
  }

  return { dependents, affectedFiles: Array.from(affectedFiles) };
}
```

### 3. detectCircularDependencies

**Purpose:** Identify circular dependency cycles that can cause issues in modular systems.

**Signature:**
```typescript
async function detectCircularDependencies(
  edgeType: string
): Promise<CircularDependency[]>
```

**Parameters:**
- `edgeType` - Type of edge to check for cycles (e.g., `"Calls"`, `"Imports"`)

**Returns:** Array of circular dependency pairs

**Example:**
```typescript
import { detectCircularDependencies } from 'codegraph-napi';

// Check for circular imports
const cycles = await detectCircularDependencies('Imports');

if (cycles.length === 0) {
  console.log('No circular dependencies found!');
} else {
  console.warn(`Found ${cycles.length} circular dependencies:\n`);

  cycles.forEach((cycle, i) => {
    console.log(`${i + 1}. ${cycle.node1.name} <--> ${cycle.node2.name}`);
    console.log(`   Type: ${cycle.dependencyType}`);

    if (cycle.node1.location) {
      console.log(`   ${cycle.node1.location.filePath}:${cycle.node1.location.startLine}`);
    }
    if (cycle.node2.location) {
      console.log(`   ${cycle.node2.location.filePath}:${cycle.node2.location.startLine}`);
    }
    console.log();
  });
}
```

**Use Case: CI/CD Quality Gate**

```typescript
async function checkCircularDependencies() {
  const importCycles = await detectCircularDependencies('Imports');
  const callCycles = await detectCircularDependencies('Calls');

  const totalCycles = importCycles.length + callCycles.length;

  console.log('Circular Dependency Check');
  console.log('=========================');
  console.log(`Import cycles: ${importCycles.length}`);
  console.log(`Call cycles: ${callCycles.length}`);
  console.log(`Total: ${totalCycles}\n`);

  if (totalCycles > 0) {
    console.error('FAILED: Circular dependencies detected!');
    process.exit(1);
  } else {
    console.log('PASSED: No circular dependencies');
    process.exit(0);
  }
}
```

### 4. traceCallChain

**Purpose:** Map the full call hierarchy from a starting function, showing what it calls and the call tree.

**Signature:**
```typescript
async function traceCallChain(
  fromNode: string,
  maxDepth?: number
): Promise<CallChainNode[]>
```

**Parameters:**
- `fromNode` - Starting node ID
- `maxDepth` - Maximum depth to trace (default: 5)

**Returns:** Array of nodes in the call chain with caller information

**Example:**
```typescript
import { traceCallChain } from 'codegraph-napi';

// Trace calls from main entry point
const chain = await traceCallChain('node:main-function', 5);

console.log('Call Chain:');
chain.forEach(node => {
  const indent = '  '.repeat(node.callDepth || 0);
  console.log(`${indent}${node.name} (${node.kind})`);

  if (node.calledBy && node.calledBy.length > 0) {
    const callers = node.calledBy.map(c => c.name).join(', ');
    console.log(`${indent}  ← Called by: ${callers}`);
  }

  if (node.location) {
    console.log(`${indent}  📍 ${node.location.filePath}:${node.location.startLine}`);
  }
});
```

**Use Case: Performance Profiling Path**

```typescript
async function analyzeExecutionPath(entryPoint: string) {
  const chain = await traceCallChain(entryPoint, 10);

  // Build call tree
  const tree = new Map();
  chain.forEach(node => {
    const depth = node.callDepth || 0;
    if (!tree.has(depth)) tree.set(depth, []);
    tree.get(depth).push(node);
  });

  console.log('Execution Path Analysis');
  console.log('======================\n');

  console.log(`Total function calls: ${chain.length}`);
  console.log(`Maximum call depth: ${Math.max(...Array.from(tree.keys()))}`);

  // Identify hot paths (functions called from multiple places)
  const callCounts = new Map();
  chain.forEach(node => {
    const count = (node.calledBy || []).length;
    callCounts.set(node.name, count);
  });

  const hotFunctions = Array.from(callCounts.entries())
    .filter(([_, count]) => count > 3)
    .sort((a, b) => b[1] - a[1]);

  if (hotFunctions.length > 0) {
    console.log('\nHot Functions (called from multiple places):');
    hotFunctions.forEach(([name, count]) => {
      console.log(`  ${name}: ${count} callers`);
    });
  }

  return { chain, tree, hotFunctions };
}
```

### 5. calculateCouplingMetrics

**Purpose:** Calculate afferent and efferent coupling metrics (Ca, Ce) and instability for a component.

**Signature:**
```typescript
async function calculateCouplingMetrics(
  nodeId: string
): Promise<CouplingMetricsResult>
```

**Parameters:**
- `nodeId` - The node ID to analyze

**Returns:** Comprehensive coupling metrics with dependencies and dependents

**Metrics Explained:**
- **Afferent Coupling (Ca):** Number of classes/modules that depend on this one
- **Efferent Coupling (Ce):** Number of classes/modules this one depends on
- **Instability (I):** Ce / (Ca + Ce) - ranges from 0 (stable) to 1 (unstable)
- **Stability:** 1 - Instability

**Example:**
```typescript
import { calculateCouplingMetrics } from 'codegraph-napi';

const result = await calculateCouplingMetrics('node:user-service');

console.log(`\nCoupling Analysis: ${result.node.name}`);
console.log('='.repeat(50));

console.log('\nMetrics:');
console.log(`  Afferent coupling (Ca): ${result.metrics.afferentCoupling}`);
console.log(`  Efferent coupling (Ce): ${result.metrics.efferentCoupling}`);
console.log(`  Total coupling: ${result.metrics.totalCoupling}`);
console.log(`  Instability (I): ${result.metrics.instability.toFixed(3)}`);
console.log(`  Stability: ${result.metrics.stability.toFixed(3)}`);
console.log(`  Category: ${result.metrics.couplingCategory}`);

console.log(`\n${result.dependents.length} components depend on this:`);
result.dependents.slice(0, 5).forEach(d => {
  console.log(`  ← ${d.name} (${d.kind})`);
});
if (result.dependents.length > 5) {
  console.log(`  ... and ${result.dependents.length - 5} more`);
}

console.log(`\nThis depends on ${result.dependencies.length} components:`);
result.dependencies.slice(0, 5).forEach(d => {
  console.log(`  → ${d.name} (${d.kind})`);
});
if (result.dependencies.length > 5) {
  console.log(`  ... and ${result.dependencies.length - 5} more`);
}
```

**Use Case: Architecture Health Dashboard**

```typescript
async function generateArchitectureReport(moduleIds: string[]) {
  console.log('Architecture Health Report');
  console.log('=========================\n');

  const results = await Promise.all(
    moduleIds.map(id => calculateCouplingMetrics(id))
  );

  // Sort by instability (most unstable first)
  results.sort((a, b) => b.metrics.instability - a.metrics.instability);

  console.log('Modules by Instability:\n');
  results.forEach((result, i) => {
    const { node, metrics } = result;
    const status = metrics.isStable ? '✓ STABLE' : '⚠ UNSTABLE';

    console.log(`${i + 1}. ${node.name}`);
    console.log(`   Instability: ${metrics.instability.toFixed(3)} ${status}`);
    console.log(`   Ca: ${metrics.afferentCoupling}, Ce: ${metrics.efferentCoupling}`);
    console.log();
  });

  // Identify problematic modules
  const unstable = results.filter(r => r.metrics.isUnstable);
  const highCoupling = results.filter(r => r.metrics.totalCoupling > 20);

  if (unstable.length > 0) {
    console.warn(`\nWarning: ${unstable.length} unstable modules found`);
    console.warn('Consider refactoring to reduce coupling.');
  }

  if (highCoupling.length > 0) {
    console.warn(`\nWarning: ${highCoupling.length} highly coupled modules`);
    console.warn('These modules may be difficult to maintain.');
  }

  return { results, unstable, highCoupling };
}
```

### 6. getHubNodes

**Purpose:** Identify highly connected nodes that serve as architectural hubs or bottlenecks.

**Signature:**
```typescript
async function getHubNodes(
  minDegree?: number
): Promise<HubNode[]>
```

**Parameters:**
- `minDegree` - Minimum total degree to be considered a hub (default: 5)

**Returns:** Array of hub nodes with detailed degree information

**Example:**
```typescript
import { getHubNodes } from 'codegraph-napi';

const hubs = await getHubNodes(10);

console.log(`Found ${hubs.length} hub nodes:\n`);

hubs.forEach((hub, i) => {
  console.log(`${i + 1}. ${hub.node.name}`);
  console.log(`   Total degree: ${hub.totalDegree}`);
  console.log(`   Incoming: ${hub.afferentDegree}, Outgoing: ${hub.efferentDegree}`);

  if (hub.incomingByType.length > 0) {
    console.log('   Incoming connections:');
    hub.incomingByType.forEach(t =>
      console.log(`     ${t.edgeType}: ${t.count}`)
    );
  }

  if (hub.outgoingByType.length > 0) {
    console.log('   Outgoing connections:');
    hub.outgoingByType.forEach(t =>
      console.log(`     ${t.edgeType}: ${t.count}`)
    );
  }

  if (hub.node.location) {
    console.log(`   📍 ${hub.node.location.filePath}`);
  }
  console.log();
});
```

**Use Case: Identify Architectural Risk**

```typescript
async function identifyArchitecturalRisks() {
  const hubs = await getHubNodes(15);

  console.log('Architectural Risk Analysis');
  console.log('==========================\n');

  // Classify hubs by risk level
  const critical = hubs.filter(h => h.totalDegree >= 50);
  const high = hubs.filter(h => h.totalDegree >= 30 && h.totalDegree < 50);
  const moderate = hubs.filter(h => h.totalDegree >= 15 && h.totalDegree < 30);

  console.log(`Critical risk (≥50 connections): ${critical.length}`);
  console.log(`High risk (30-49 connections): ${high.length}`);
  console.log(`Moderate risk (15-29 connections): ${moderate.length}\n`);

  if (critical.length > 0) {
    console.error('CRITICAL HUBS - High risk of cascading failures:\n');
    critical.forEach(hub => {
      console.error(`  ${hub.node.name}`);
      console.error(`    ${hub.totalDegree} total connections`);
      console.error(`    ${hub.afferentDegree} dependents would break if this fails`);
      if (hub.node.location) {
        console.error(`    ${hub.node.location.filePath}`);
      }
      console.error();
    });

    console.error('Recommendation: Implement circuit breakers and fallbacks.');
  }

  // Identify bottlenecks (high afferent, low efferent)
  const bottlenecks = hubs.filter(h =>
    h.afferentDegree > h.efferentDegree * 3
  );

  if (bottlenecks.length > 0) {
    console.warn('\nBOTTLENECKS - Single points of failure:\n');
    bottlenecks.forEach(hub => {
      console.warn(`  ${hub.node.name}`);
      console.warn(`    ${hub.afferentDegree} dependents rely on this`);
      console.warn('    Consider horizontal scaling or redundancy\n');
    });
  }

  return { critical, high, moderate, bottlenecks };
}
```

## Use Cases

### 1. Architectural Analysis

**Scenario:** You're joining a new codebase and need to understand its architecture.

```typescript
async function analyzeArchitecture() {
  console.log('Codebase Architecture Analysis\n');

  // Step 1: Find key architectural nodes
  console.log('1. Identifying architectural hubs...');
  const hubs = await getHubNodes(10);
  console.log(`   Found ${hubs.length} highly connected components\n`);

  // Step 2: Analyze coupling for each hub
  console.log('2. Analyzing coupling metrics...');
  for (const hub of hubs.slice(0, 5)) {
    const metrics = await calculateCouplingMetrics(hub.nodeId);
    console.log(`   ${hub.node.name}:`);
    console.log(`     Instability: ${metrics.metrics.instability.toFixed(3)}`);
    console.log(`     ${metrics.dependents.length} dependents, ${metrics.dependencies.length} dependencies`);
  }
  console.log();

  // Step 3: Check for architectural issues
  console.log('3. Checking for circular dependencies...');
  const cycles = await detectCircularDependencies('Imports');
  console.log(`   Found ${cycles.length} circular dependencies\n`);

  // Step 4: Map key execution paths
  console.log('4. Tracing main execution paths...');
  const mainHub = hubs[0];
  const callChain = await traceCallChain(mainHub.nodeId, 3);
  console.log(`   Main hub calls ${callChain.length} functions`);
}
```

### 2. Refactoring Guidance

**Scenario:** Planning to refactor a module and need impact analysis.

```typescript
async function planRefactoring(moduleId: string) {
  console.log('Refactoring Impact Analysis\n');

  // Find who depends on this module
  const dependents = await getReverseDependencies(moduleId, 'Imports', 10);

  // Get coupling metrics
  const metrics = await calculateCouplingMetrics(moduleId);

  // Find what this module depends on
  const dependencies = await getTransitiveDependencies(moduleId, 'Imports', 10);

  console.log('Impact Summary:');
  console.log(`  ${dependents.length} modules would be affected by changes`);
  console.log(`  ${dependencies.length} dependencies to preserve`);
  console.log(`  Instability: ${metrics.metrics.instability.toFixed(3)}`);

  if (metrics.metrics.isUnstable) {
    console.log('\nRecommendation: High instability - safer to refactor');
  } else {
    console.warn('\nWarning: Stable module with many dependents');
    console.warn('Consider deprecation strategy or adapter pattern');
  }

  // Group dependents by distance
  const byDepth = new Map();
  dependents.forEach(d => {
    const depth = d.dependentDepth || 0;
    if (!byDepth.has(depth)) byDepth.set(depth, []);
    byDepth.get(depth).push(d);
  });

  console.log('\nImpact Radius:');
  byDepth.forEach((nodes, depth) => {
    console.log(`  ${depth} steps: ${nodes.length} modules`);
  });

  return { dependents, dependencies, metrics };
}
```

### 3. Code Quality Metrics

**Scenario:** Generate code quality metrics for dashboards and reporting.

```typescript
async function generateQualityMetrics() {
  console.log('Code Quality Metrics Report\n');

  // Get all major components
  const hubs = await getHubNodes(5);

  const metrics = {
    totalComponents: hubs.length,
    coupling: [],
    circularDependencies: 0,
    unstableComponents: 0,
    highRiskComponents: 0,
  };

  // Analyze each component
  for (const hub of hubs) {
    const coupling = await calculateCouplingMetrics(hub.nodeId);

    metrics.coupling.push({
      name: hub.node.name,
      afferent: coupling.metrics.afferentCoupling,
      efferent: coupling.metrics.efferentCoupling,
      instability: coupling.metrics.instability,
    });

    if (coupling.metrics.isUnstable) {
      metrics.unstableComponents++;
    }

    if (hub.totalDegree > 50) {
      metrics.highRiskComponents++;
    }
  }

  // Check for cycles
  const cycles = await detectCircularDependencies('Imports');
  metrics.circularDependencies = cycles.length;

  // Calculate average instability
  const avgInstability =
    metrics.coupling.reduce((sum, c) => sum + c.instability, 0) /
    metrics.coupling.length;

  console.log('Metrics Summary:');
  console.log(`  Total components analyzed: ${metrics.totalComponents}`);
  console.log(`  Average instability: ${avgInstability.toFixed(3)}`);
  console.log(`  Unstable components: ${metrics.unstableComponents}`);
  console.log(`  High-risk components: ${metrics.highRiskComponents}`);
  console.log(`  Circular dependencies: ${metrics.circularDependencies}`);

  // Quality score (0-100)
  const qualityScore = Math.max(
    0,
    100 -
      (avgInstability * 50) -
      (metrics.circularDependencies * 5) -
      (metrics.highRiskComponents * 10)
  );

  console.log(`\nOverall Quality Score: ${qualityScore.toFixed(1)}/100`);

  return metrics;
}
```

### 4. Dependency Impact Analysis

**Scenario:** Evaluate the impact of upgrading or removing a dependency.

```typescript
async function analyzeDependencyImpact(dependencyId: string) {
  console.log(`Dependency Impact Analysis: ${dependencyId}\n`);

  // Find all code that depends on this
  const directDependents = await getReverseDependencies(
    dependencyId,
    'Imports',
    1
  );

  const transitiveDependents = await getReverseDependencies(
    dependencyId,
    'Imports',
    10
  );

  console.log('Direct Impact:');
  console.log(`  ${directDependents.length} files import this dependency`);

  console.log('\nTransitive Impact:');
  console.log(`  ${transitiveDependents.length} files affected transitively`);

  // Group by file
  const affectedFiles = new Set(
    transitiveDependents
      .filter(d => d.location)
      .map(d => d.location!.filePath)
  );

  console.log(`  ${affectedFiles.size} files need review\n`);

  // Identify critical paths
  const criticalDependents = directDependents.filter(async d => {
    const metrics = await calculateCouplingMetrics(d.id);
    return metrics.metrics.afferentCoupling > 10;
  });

  if (criticalDependents.length > 0) {
    console.warn('Critical Dependents (high afferent coupling):');
    criticalDependents.forEach(d => {
      console.warn(`  - ${d.name}`);
    });
    console.warn('\nThese components have many dependents themselves.');
    console.warn('Changes here will cascade widely.\n');
  }

  return {
    directDependents,
    transitiveDependents,
    affectedFiles: Array.from(affectedFiles),
  };
}
```

## Integration Patterns

### CI/CD Pipeline Integration

```typescript
// scripts/quality-gate.ts
import {
  detectCircularDependencies,
  getHubNodes,
  calculateCouplingMetrics,
} from 'codegraph-napi';

async function qualityGate() {
  console.log('Running CodeGraph Quality Gate...\n');

  let exitCode = 0;

  // Check 1: No circular dependencies
  const cycles = await detectCircularDependencies('Imports');
  if (cycles.length > 0) {
    console.error(`❌ FAIL: ${cycles.length} circular dependencies found`);
    exitCode = 1;
  } else {
    console.log('✓ PASS: No circular dependencies');
  }

  // Check 2: No super-hubs (> 100 connections)
  const hubs = await getHubNodes(100);
  if (hubs.length > 0) {
    console.error(`❌ FAIL: ${hubs.length} super-hub nodes (>100 connections)`);
    exitCode = 1;
  } else {
    console.log('✓ PASS: No super-hub nodes');
  }

  // Check 3: Average instability in acceptable range
  const allHubs = await getHubNodes(5);
  const metrics = await Promise.all(
    allHubs.slice(0, 20).map(h => calculateCouplingMetrics(h.nodeId))
  );

  const avgInstability =
    metrics.reduce((sum, m) => sum + m.metrics.instability, 0) / metrics.length;

  if (avgInstability > 0.7) {
    console.error(`❌ FAIL: High average instability (${avgInstability.toFixed(3)})`);
    exitCode = 1;
  } else {
    console.log(`✓ PASS: Average instability acceptable (${avgInstability.toFixed(3)})`);
  }

  process.exit(exitCode);
}

qualityGate().catch(err => {
  console.error('Quality gate error:', err);
  process.exit(1);
});
```

### Code Review Automation

```typescript
// scripts/review-helper.ts
import { getReverseDependencies, calculateCouplingMetrics } from 'codegraph-napi';

async function analyzeChangedFiles(changedFiles: string[]) {
  console.log('Code Review Impact Analysis\n');

  for (const file of changedFiles) {
    console.log(`\nAnalyzing: ${file}`);

    // Find all functions/modules in this file
    // (Assumes you have a way to get node IDs from file paths)
    const nodeId = await getNodeIdFromFile(file);

    if (!nodeId) {
      console.log('  No indexed nodes found');
      continue;
    }

    // Check impact
    const dependents = await getReverseDependencies(nodeId, 'Imports', 3);
    const metrics = await calculateCouplingMetrics(nodeId);

    console.log(`  Impact: ${dependents.length} dependent modules`);
    console.log(`  Coupling: Ca=${metrics.metrics.afferentCoupling}, Ce=${metrics.metrics.efferentCoupling}`);
    console.log(`  Instability: ${metrics.metrics.instability.toFixed(3)}`);

    if (dependents.length > 20) {
      console.warn('  ⚠️  High impact change - request additional reviewers');
    }

    if (metrics.metrics.isUnstable && dependents.length > 10) {
      console.warn('  ⚠️  Unstable module with many dependents - extra caution');
    }
  }
}

// Helper function (implementation depends on your indexing strategy)
async function getNodeIdFromFile(filePath: string): Promise<string | null> {
  // Query your CodeGraph index for nodes in this file
  return null;
}
```

### Dashboard/Reporting Tools

```typescript
// scripts/generate-dashboard.ts
import {
  getHubNodes,
  calculateCouplingMetrics,
  detectCircularDependencies,
} from 'codegraph-napi';
import * as fs from 'fs/promises';

async function generateDashboard() {
  console.log('Generating architecture dashboard...\n');

  // Collect metrics
  const hubs = await getHubNodes(5);
  const cycles = await detectCircularDependencies('Imports');

  const metricsData = await Promise.all(
    hubs.slice(0, 50).map(async hub => {
      const metrics = await calculateCouplingMetrics(hub.nodeId);
      return {
        name: hub.node.name,
        totalDegree: hub.totalDegree,
        afferentCoupling: metrics.metrics.afferentCoupling,
        efferentCoupling: metrics.metrics.efferentCoupling,
        instability: metrics.metrics.instability,
        category: metrics.metrics.couplingCategory,
        location: hub.node.location?.filePath,
      };
    })
  );

  // Generate HTML report
  const html = `
<!DOCTYPE html>
<html>
<head>
  <title>CodeGraph Architecture Dashboard</title>
  <script src="https://cdn.jsdelivr.net/npm/chart.js"></script>
  <style>
    body { font-family: Arial, sans-serif; margin: 20px; }
    .metric { display: inline-block; margin: 20px; padding: 20px; border: 1px solid #ccc; }
    .metric h3 { margin: 0 0 10px 0; }
    .metric .value { font-size: 48px; font-weight: bold; }
    canvas { max-width: 600px; }
  </style>
</head>
<body>
  <h1>Architecture Health Dashboard</h1>

  <div class="metric">
    <h3>Total Components</h3>
    <div class="value">${hubs.length}</div>
  </div>

  <div class="metric">
    <h3>Circular Dependencies</h3>
    <div class="value" style="color: ${cycles.length > 0 ? 'red' : 'green'}">
      ${cycles.length}
    </div>
  </div>

  <div class="metric">
    <h3>Avg Instability</h3>
    <div class="value">
      ${(metricsData.reduce((s, m) => s + m.instability, 0) / metricsData.length).toFixed(2)}
    </div>
  </div>

  <h2>Coupling Distribution</h2>
  <canvas id="couplingChart"></canvas>

  <script>
    const data = ${JSON.stringify(metricsData)};

    new Chart(document.getElementById('couplingChart'), {
      type: 'scatter',
      data: {
        datasets: [{
          label: 'Components',
          data: data.map(d => ({ x: d.efferentCoupling, y: d.afferentCoupling })),
          backgroundColor: 'rgba(54, 162, 235, 0.5)',
        }]
      },
      options: {
        scales: {
          x: { title: { display: true, text: 'Efferent Coupling (Ce)' } },
          y: { title: { display: true, text: 'Afferent Coupling (Ca)' } }
        }
      }
    });
  </script>
</body>
</html>
  `;

  await fs.writeFile('architecture-dashboard.html', html);
  console.log('Dashboard generated: architecture-dashboard.html');
}

generateDashboard().catch(console.error);
```

### VS Code Extension Integration

```typescript
// extension.ts
import * as vscode from 'vscode';
import { getReverseDependencies, traceCallChain } from 'codegraph-napi';

export function activate(context: vscode.ExtensionContext) {
  // Command: Show function impact
  const showImpact = vscode.commands.registerCommand(
    'codegraph.showImpact',
    async () => {
      const editor = vscode.window.activeTextEditor;
      if (!editor) return;

      const nodeId = await getNodeIdAtCursor(editor);
      if (!nodeId) {
        vscode.window.showWarningMessage('No function found at cursor');
        return;
      }

      const dependents = await getReverseDependencies(nodeId, 'Calls', 3);

      vscode.window.showInformationMessage(
        `This function is called by ${dependents.length} others`
      );

      // Show in side panel
      const panel = vscode.window.createWebviewPanel(
        'functionImpact',
        'Function Impact',
        vscode.ViewColumn.Two,
        {}
      );

      panel.webview.html = generateImpactHtml(dependents);
    }
  );

  context.subscriptions.push(showImpact);
}

async function getNodeIdAtCursor(
  editor: vscode.TextEditor
): Promise<string | null> {
  // Implementation depends on your indexing strategy
  return null;
}

function generateImpactHtml(dependents: any[]): string {
  return `
    <html>
      <body>
        <h2>Function Impact Analysis</h2>
        <p>${dependents.length} dependent functions:</p>
        <ul>
          ${dependents.map(d => `<li>${d.name} (depth: ${d.dependentDepth})</li>`).join('')}
        </ul>
      </body>
    </html>
  `;
}
```

## Troubleshooting

### Connection Issues

**Problem:** `GraphFunctions not initialized`

**Solution:**
```bash
# Verify SurrealDB is running
curl http://localhost:8000/health

# Check environment variable
echo $SURREALDB_CONNECTION

# Set if missing
export SURREALDB_CONNECTION="ws://localhost:8000"
```

### Empty Results

**Problem:** Functions return empty arrays

**Possible causes:**
1. No data indexed in SurrealDB
2. Incorrect edge types
3. Node IDs don't exist

**Solution:**
```typescript
// Verify data exists
const hubs = await getHubNodes(1);  // Very low threshold
if (hubs.length === 0) {
  console.error('No nodes found in database');
  console.error('Ensure data is indexed in SurrealDB');
} else {
  console.log('Database has data');
  console.log('Sample node ID:', hubs[0].nodeId);
}
```

### Performance Issues

**Problem:** Queries are slow

**Solutions:**
1. Reduce depth parameters
2. Use more specific edge types
3. Ensure SurrealDB indices are created
4. Consider connection pooling

```typescript
// Instead of this (slow):
const deps = await getTransitiveDependencies(nodeId, 'Calls', 10);

// Do this (faster):
const deps = await getTransitiveDependencies(nodeId, 'Calls', 3);
```

### Authentication Errors

**Problem:** Connection denied

**Solution:**
```bash
# Include credentials in connection string
export SURREALDB_CONNECTION="ws://username:password@localhost:8000"
```

### Feature Not Enabled

**Problem:** `cloud-surrealdb feature not enabled`

**Solution:**
Rebuild with feature flag:
```bash
cd crates/codegraph-napi
cargo build --release --features cloud-surrealdb
npm run build
```

## Best Practices

1. **Start with low depth values** - Use depth 1-3 initially, increase only if needed
2. **Cache results** - Graph operations can be expensive, cache when appropriate
3. **Use specific edge types** - More specific = faster queries
4. **Handle errors gracefully** - Always wrap in try/catch
5. **Monitor query performance** - Log execution times in production
6. **Batch operations** - Use Promise.all() for parallel queries
7. **Index strategically** - Ensure SurrealDB has appropriate indices

## Additional Resources

- [SurrealDB Documentation](https://surrealdb.com/docs)
- [CodeGraph Core Documentation](../../README.md)
- [NAPI-RS Documentation](https://napi.rs)
- [Example Scripts](./example-graph.ts)
