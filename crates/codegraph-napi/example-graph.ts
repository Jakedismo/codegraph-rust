/**
 * Example: Using CodeGraph Advanced Graph Analysis Functions
 *
 * Prerequisites:
 * 1. SurrealDB running: surreal start memory
 * 2. CodeGraph data indexed in SurrealDB
 * 3. SURREALDB_CONNECTION environment variable set
 *
 * Run: SURREALDB_CONNECTION=ws://localhost:8000 tsx example-graph.ts
 */

import {
  getTransitiveDependencies,
  detectCircularDependencies,
  traceCallChain,
  calculateCouplingMetrics,
  getHubNodes,
  getReverseDependencies,
} from './index';

// Color codes for terminal output
const colors = {
  reset: '\x1b[0m',
  bright: '\x1b[1m',
  red: '\x1b[31m',
  green: '\x1b[32m',
  yellow: '\x1b[33m',
  blue: '\x1b[34m',
  magenta: '\x1b[35m',
  cyan: '\x1b[36m',
};

function log(message: string, color: keyof typeof colors = 'reset') {
  console.log(`${colors[color]}${message}${colors.reset}`);
}

function section(title: string) {
  console.log('\n' + '='.repeat(60));
  log(title, 'bright');
  console.log('='.repeat(60) + '\n');
}

async function main() {
  log('🔍 CodeGraph Advanced Graph Analysis Examples\n', 'cyan');

  // Check environment
  if (!process.env.SURREALDB_CONNECTION) {
    log('❌ SURREALDB_CONNECTION not set', 'red');
    console.log('   Example: export SURREALDB_CONNECTION=ws://localhost:8000');
    console.log('   Or run: SURREALDB_CONNECTION=ws://localhost:8000 tsx example-graph.ts');
    process.exit(1);
  }

  log(`✓ Connected to: ${process.env.SURREALDB_CONNECTION}\n`, 'green');

  try {
    // ========================================
    // Test 1: Find Hub Nodes
    // ========================================
    section('Test 1: Finding Hub Nodes');
    log('Finding highly connected nodes (min degree: 5)...', 'cyan');

    const hubs = await getHubNodes(5);
    log(`✓ Found ${hubs.length} hub nodes\n`, 'green');

    if (hubs.length > 0) {
      log('Top 5 most connected nodes:', 'bright');
      hubs.slice(0, 5).forEach((hub, i) => {
        console.log(`\n${i + 1}. ${hub.node.name}`);
        console.log(`   Total degree: ${hub.totalDegree}`);
        console.log(`   Incoming: ${hub.afferentDegree}, Outgoing: ${hub.efferentDegree}`);

        if (hub.node.location) {
          console.log(`   Location: ${hub.node.location.filePath}`);
          if (hub.node.location.startLine) {
            console.log(`   Line: ${hub.node.location.startLine}`);
          }
        }

        if (hub.incomingByType.length > 0) {
          console.log('   Incoming by type:');
          hub.incomingByType.forEach(t =>
            console.log(`     ${t.edgeType}: ${t.count}`)
          );
        }

        if (hub.outgoingByType.length > 0) {
          console.log('   Outgoing by type:');
          hub.outgoingByType.forEach(t =>
            console.log(`     ${t.edgeType}: ${t.count}`)
          );
        }
      });

      // Save the first hub node ID for later tests
      const sampleNodeId = hubs[0].nodeId;

      // ========================================
      // Test 2: Calculate Coupling Metrics
      // ========================================
      section('Test 2: Coupling Metrics Analysis');
      log(`Analyzing coupling metrics for: ${hubs[0].node.name}`, 'cyan');

      const couplingResult = await calculateCouplingMetrics(sampleNodeId);
      log('✓ Metrics calculated\n', 'green');

      console.log('Coupling Metrics:');
      console.log(`  Afferent coupling (Ca): ${couplingResult.metrics.afferentCoupling}`);
      console.log(`  Efferent coupling (Ce): ${couplingResult.metrics.efferentCoupling}`);
      console.log(`  Total coupling: ${couplingResult.metrics.totalCoupling}`);
      console.log(`  Instability (I): ${couplingResult.metrics.instability.toFixed(3)}`);
      console.log(`  Stability: ${couplingResult.metrics.stability.toFixed(3)}`);
      console.log(`  Category: ${couplingResult.metrics.couplingCategory}`);

      if (couplingResult.metrics.isStable) {
        log('  Status: ✓ STABLE', 'green');
      } else if (couplingResult.metrics.isUnstable) {
        log('  Status: ⚠ UNSTABLE', 'yellow');
      }

      console.log(`\n${couplingResult.dependents.length} dependents:`);
      couplingResult.dependents.slice(0, 5).forEach(d => {
        console.log(`  - ${d.name}${d.kind ? ` (${d.kind})` : ''}`);
      });
      if (couplingResult.dependents.length > 5) {
        console.log(`  ... and ${couplingResult.dependents.length - 5} more`);
      }

      console.log(`\n${couplingResult.dependencies.length} dependencies:`);
      couplingResult.dependencies.slice(0, 5).forEach(d => {
        console.log(`  - ${d.name}${d.kind ? ` (${d.kind})` : ''}`);
      });
      if (couplingResult.dependencies.length > 5) {
        console.log(`  ... and ${couplingResult.dependencies.length - 5} more`);
      }

      // ========================================
      // Test 3: Transitive Dependencies
      // ========================================
      section('Test 3: Transitive Dependencies');
      log(`Finding transitive dependencies for: ${hubs[0].node.name}`, 'cyan');
      log('Edge type: Calls, Depth: 3', 'cyan');

      const dependencies = await getTransitiveDependencies(
        sampleNodeId,
        'Calls',
        3
      );
      log(`✓ Found ${dependencies.length} transitive dependencies\n`, 'green');

      if (dependencies.length > 0) {
        console.log('Dependency tree (first 10):');

        // Group by depth
        const byDepth = new Map<number, typeof dependencies>();
        dependencies.forEach(d => {
          const depth = d.dependencyDepth || 0;
          if (!byDepth.has(depth)) byDepth.set(depth, []);
          byDepth.get(depth)!.push(d);
        });

        console.log('\nDependencies by depth:');
        byDepth.forEach((nodes, depth) => {
          console.log(`  Depth ${depth}: ${nodes.length} nodes`);
        });

        console.log('\nSample dependencies:');
        dependencies.slice(0, 10).forEach(d => {
          const indent = '  '.repeat(d.dependencyDepth || 0);
          console.log(`${indent}[${d.dependencyDepth}] ${d.name}`);
          if (d.location) {
            console.log(`${indent}  ${d.location.filePath}:${d.location.startLine || '?'}`);
          }
        });
      }

      // ========================================
      // Test 4: Reverse Dependencies
      // ========================================
      section('Test 4: Reverse Dependencies (Dependents)');
      log(`Finding who depends on: ${hubs[0].node.name}`, 'cyan');
      log('Edge type: Calls, Depth: 2', 'cyan');

      const dependents = await getReverseDependencies(
        sampleNodeId,
        'Calls',
        2
      );
      log(`✓ Found ${dependents.length} dependents\n`, 'green');

      if (dependents.length > 0) {
        console.log('Impact radius:');

        // Group by depth
        const byDepth = new Map<number, typeof dependents>();
        dependents.forEach(d => {
          const depth = d.dependentDepth || 0;
          if (!byDepth.has(depth)) byDepth.set(depth, []);
          byDepth.get(depth)!.push(d);
        });

        byDepth.forEach((nodes, depth) => {
          console.log(`  ${depth} steps away: ${nodes.length} functions`);
        });

        console.log('\nSample dependents (first 10):');
        dependents.slice(0, 10).forEach(d => {
          console.log(`  ${d.name} (depth: ${d.dependentDepth})`);
          if (d.location) {
            console.log(`    ${d.location.filePath}:${d.location.startLine || '?'}`);
          }
        });

        // Impact analysis
        if (dependents.length > 20) {
          log('\n⚠ High impact node - many dependents!', 'yellow');
          log('  Changes here could affect many parts of the codebase.', 'yellow');
        }
      }

      // ========================================
      // Test 5: Trace Call Chain
      // ========================================
      section('Test 5: Call Chain Tracing');
      log(`Tracing call chain from: ${hubs[0].node.name}`, 'cyan');
      log('Max depth: 4', 'cyan');

      const callChain = await traceCallChain(sampleNodeId, 4);
      log(`✓ Found ${callChain.length} nodes in call chain\n`, 'green');

      if (callChain.length > 0) {
        console.log('Call hierarchy (first 15):');
        callChain.slice(0, 15).forEach(node => {
          const indent = '  '.repeat(node.callDepth || 0);
          console.log(`${indent}[${node.callDepth}] ${node.name}${node.kind ? ` (${node.kind})` : ''}`);

          if (node.calledBy && node.calledBy.length > 0) {
            const callers = node.calledBy.map(c => c.name).join(', ');
            console.log(`${indent}  ← Called by: ${callers}`);
          }

          if (node.location) {
            console.log(`${indent}  📍 ${node.location.filePath}:${node.location.startLine || '?'}`);
          }
        });

        // Analyze call patterns
        const maxDepth = Math.max(...callChain.map(n => n.callDepth || 0));
        console.log(`\nCall chain analysis:`);
        console.log(`  Maximum depth: ${maxDepth}`);
        console.log(`  Total functions: ${callChain.length}`);

        // Find functions called from multiple places
        const callCounts = new Map<string, number>();
        callChain.forEach(node => {
          const callerCount = (node.calledBy || []).length;
          if (callerCount > 0) {
            callCounts.set(node.name, callerCount);
          }
        });

        const multiCallers = Array.from(callCounts.entries())
          .filter(([_, count]) => count > 2)
          .sort((a, b) => b[1] - a[1]);

        if (multiCallers.length > 0) {
          console.log(`\nFunctions called from multiple places:`);
          multiCallers.slice(0, 5).forEach(([name, count]) => {
            console.log(`  ${name}: ${count} callers`);
          });
        }
      }

      // ========================================
      // Test 6: Detect Circular Dependencies
      // ========================================
      section('Test 6: Circular Dependency Detection');
      log('Checking for circular dependencies in "Calls" edges...', 'cyan');

      const callCycles = await detectCircularDependencies('Calls');
      log(`✓ Analysis complete\n`, 'green');

      if (callCycles.length === 0) {
        log('✓ No circular call dependencies found!', 'green');
      } else {
        log(`⚠ Found ${callCycles.length} circular call dependencies:\n`, 'yellow');

        callCycles.slice(0, 5).forEach((cycle, i) => {
          console.log(`${i + 1}. ${cycle.node1.name} ↔ ${cycle.node2.name}`);
          console.log(`   Dependency type: ${cycle.dependencyType}`);

          if (cycle.node1.location) {
            console.log(`   ${cycle.node1.name}: ${cycle.node1.location.filePath}:${cycle.node1.location.startLine || '?'}`);
          }
          if (cycle.node2.location) {
            console.log(`   ${cycle.node2.name}: ${cycle.node2.location.filePath}:${cycle.node2.location.startLine || '?'}`);
          }
          console.log();
        });

        if (callCycles.length > 5) {
          console.log(`   ... and ${callCycles.length - 5} more circular dependencies`);
        }
      }

      // Check for import cycles too
      log('\nChecking for circular dependencies in "Imports" edges...', 'cyan');
      const importCycles = await detectCircularDependencies('Imports');

      if (importCycles.length === 0) {
        log('✓ No circular import dependencies found!', 'green');
      } else {
        log(`⚠ Found ${importCycles.length} circular import dependencies`, 'yellow');
      }

      // ========================================
      // Summary
      // ========================================
      section('Summary');

      console.log('Graph Analysis Complete!\n');
      console.log('Results:');
      console.log(`  Hub nodes found: ${hubs.length}`);
      console.log(`  Transitive dependencies: ${dependencies.length}`);
      console.log(`  Reverse dependencies: ${dependents.length}`);
      console.log(`  Call chain length: ${callChain.length}`);
      console.log(`  Circular call dependencies: ${callCycles.length}`);
      console.log(`  Circular import dependencies: ${importCycles.length}`);

      console.log('\nCoupling Analysis:');
      console.log(`  Sample node: ${couplingResult.node.name}`);
      console.log(`  Instability: ${couplingResult.metrics.instability.toFixed(3)}`);
      console.log(`  Category: ${couplingResult.metrics.couplingCategory}`);

      // Overall health assessment
      console.log('\nCodebase Health Assessment:');

      const totalCycles = callCycles.length + importCycles.length;
      if (totalCycles === 0) {
        log('  ✓ No circular dependencies', 'green');
      } else {
        log(`  ⚠ ${totalCycles} circular dependencies detected`, 'yellow');
      }

      if (hubs.length > 0 && hubs[0].totalDegree > 50) {
        log('  ⚠ Very highly connected nodes detected', 'yellow');
        log('    Consider refactoring to reduce coupling', 'yellow');
      } else {
        log('  ✓ Node connectivity looks reasonable', 'green');
      }

      const avgInstability = couplingResult.metrics.instability;
      if (avgInstability < 0.3) {
        log('  ✓ Stable architecture (low instability)', 'green');
      } else if (avgInstability < 0.7) {
        log('  ℹ Moderate instability', 'cyan');
      } else {
        log('  ⚠ High instability', 'yellow');
      }

      log('\n✓ All tests completed successfully!', 'green');
    } else {
      log('\n⚠ No hub nodes found in database', 'yellow');
      console.log('   This could mean:');
      console.log('   1. No data has been indexed yet');
      console.log('   2. Database connection is working but no nodes exist');
      console.log('   3. Minimum degree threshold is too high');
      console.log('\n   Try indexing some code first or lowering the threshold.');
    }
  } catch (error) {
    section('Error');
    log('❌ An error occurred:', 'red');
    console.error(error instanceof Error ? error.message : String(error));

    if (error instanceof Error && error.message.includes('not initialized')) {
      console.log('\nTroubleshooting:');
      console.log('1. Ensure SurrealDB is running:');
      console.log('   surreal start memory');
      console.log('2. Verify SURREALDB_CONNECTION is set correctly');
      console.log('3. Check that data has been indexed in SurrealDB');
    }

    process.exit(1);
  }
}

main().catch(error => {
  console.error('Fatal error:', error);
  process.exit(1);
});
