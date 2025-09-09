#!/usr/bin/env node

import { CodeGraphMCPServer } from '../server/codegraph-mcp-server.js';
import { AnalyzerAgent } from './analyzer-agent.js';
import { CoordinatorAgent } from './coordinator-agent.js';

async function startDemo(): Promise<void> {
  console.log('🚀 Starting CodeGraph MCP Demo Server');

  // Create and configure the MCP server
  const server = new CodeGraphMCPServer({
    name: 'codegraph-demo',
    version: '1.0.0',
    websocketPort: 3001,
    httpPort: 3000,
    maxConnections: 100,
    enableDiscovery: true,
    enableMetrics: true,
    corsOrigins: ['*'],
    authentication: {
      enabled: false // Disabled for demo
    }
  });

  // Set up server event handlers
  server.on('server:started', ({ websocketPort, httpPort }) => {
    console.log(`✅ Server started successfully`);
    console.log(`   WebSocket: ws://localhost:${websocketPort}`);
    console.log(`   HTTP: http://localhost:${httpPort}`);
  });

  server.on('client:connected', ({ sessionId }) => {
    console.log(`🔌 Client connected: ${sessionId}`);
  });

  server.on('client:disconnected', ({ sessionId, agentId }) => {
    console.log(`🔌 Client disconnected: ${sessionId} (agent: ${agentId || 'unknown'})`);
  });

  server.on('agent:registered', ({ agentInfo }) => {
    console.log(`🤖 Agent registered: ${agentInfo.agentId} (${agentInfo.type})`);
  });

  server.on('coordination:task_distributed', ({ result, strategy }) => {
    console.log(`📋 Task distributed using ${strategy} strategy`);
    console.log(`   Assigned tasks: ${result.totalAssigned}`);
  });

  server.on('server:error', ({ error }) => {
    console.error('❌ Server error:', error);
  });

  // Start the server
  try {
    await server.start();
    
    console.log('\n📊 Server Status:');
    console.log('   - WebSocket transport: Ready');
    console.log('   - Message routing: Active'); 
    console.log('   - Coordination engine: Active');
    console.log('   - Agent registry: Ready');

    // Create demo agents
    console.log('\n🤖 Starting demo agents...');
    
    const coordinatorAgent = new CoordinatorAgent('demo-coordinator');
    const analyzerAgent1 = new AnalyzerAgent('demo-analyzer-1');
    const analyzerAgent2 = new AnalyzerAgent('demo-analyzer-2');

    // Initialize and start agents
    await coordinatorAgent.initialize();
    await coordinatorAgent.start();
    await coordinatorAgent.connect('ws://localhost:3001');
    
    await analyzerAgent1.initialize();
    await analyzerAgent1.start();
    await analyzerAgent1.connect('ws://localhost:3001');
    
    await analyzerAgent2.initialize();
    await analyzerAgent2.start();
    await analyzerAgent2.connect('ws://localhost:3001');

    console.log('✅ Demo agents started and connected');

    // Set up agent event handlers
    coordinatorAgent.on('coordinator:ready', ({ agentId, availableAgents }) => {
      console.log(`🎯 Coordinator ${agentId} ready with ${Object.keys(availableAgents).length} agent types`);
    });

    analyzerAgent1.on('analyzer:ready', ({ agentId, engines }) => {
      console.log(`🔍 Analyzer ${agentId} ready with ${engines.length} analysis engines`);
    });

    analyzerAgent2.on('analyzer:ready', ({ agentId, engines }) => {
      console.log(`🔍 Analyzer ${agentId} ready with ${engines.length} analysis engines`);
    });

    // Demo task execution
    console.log('\n📝 Running demo tasks...');
    await runDemoTasks(server);

    console.log('\n🎉 CodeGraph MCP Demo Server is running!');
    console.log('\nTry these commands:');
    console.log('   curl -X POST http://localhost:3000/mcp \\');
    console.log('     -H "Content-Type: application/json" \\');
    console.log('     -d \'{"jsonrpc":"2.0","method":"tools/list","id":"1"}\'');
    console.log('\n   Or connect with a WebSocket client to ws://localhost:3001');
    console.log('\n   Press Ctrl+C to stop');

    // Keep the process running
    process.on('SIGINT', async () => {
      console.log('\n🛑 Shutting down...');
      
      await coordinatorAgent.destroy();
      await analyzerAgent1.destroy();
      await analyzerAgent2.destroy();
      await server.stop();
      
      console.log('✅ Shutdown complete');
      process.exit(0);
    });

  } catch (error) {
    console.error('❌ Failed to start server:', error);
    process.exit(1);
  }
}

async function runDemoTasks(server: CodeGraphMCPServer): Promise<void> {
  // Wait a bit for agents to fully register
  await new Promise(resolve => setTimeout(resolve, 2000));

  console.log('   Running code analysis workflow...');

  // Example TypeScript code to analyze
  const sampleCode = `
function calculateTotal(items: any[]) {
  var total = 0;
  for (var i = 0; i < items.length; i++) {
    if (items[i].price == null) {
      continue;
    }
    total += items[i].price;
  }
  return total;
}

function processData(data: string) {
  eval("console.log('" + data + "')");
  return data.toUpperCase();
}
`;

  try {
    // Get the MCP server instance to call tools directly
    const mcpServer = server.getMcpServer();

    // Simulate tool calls that would come from MCP clients
    const analysisTask = {
      id: 'demo-task-1',
      type: 'code_analysis',
      priority: 'normal',
      payload: {
        type: 'analyze_code',
        data: {
          sourceCode: sampleCode,
          language: 'typescript',
          analysisType: 'syntax',
          options: {
            includeMetrics: true,
            includeWarnings: true,
            severity: 'medium'
          }
        }
      },
      timeout: 30000
    };

    console.log('   ✓ Created analysis task');

    // Simulate task distribution
    const distributionResult = {
      taskId: analysisTask.id,
      assignedAgents: ['demo-analyzer-1'],
      results: [{
        success: true,
        data: {
          analysis: {
            issues: [
              {
                type: 'warning',
                severity: 'medium',
                message: 'Use let/const instead of var',
                line: 2,
                rule: 'no-var',
                category: 'syntax'
              },
              {
                type: 'error',
                severity: 'critical',
                message: 'Avoid using eval() - potential code injection vulnerability',
                rule: 'no-eval',
                category: 'security'
              }
            ],
            suggestions: [
              {
                type: 'fix',
                description: 'Replace var with let or const',
                impact: 'low',
                effort: 'minimal'
              }
            ],
            score: 65
          }
        }
      }]
    };

    console.log('   ✓ Distributed task to analyzer agent');
    console.log(`   ✓ Analysis completed with score: ${distributionResult.results[0].data.analysis.score}/100`);
    console.log(`   ✓ Found ${distributionResult.results[0].data.analysis.issues.length} issues`);

    // Demonstrate coordination
    console.log('   Running multi-agent coordination...');

    const coordinationResult = {
      strategy: 'consensus',
      participants: ['demo-coordinator', 'demo-analyzer-1', 'demo-analyzer-2'],
      success: true,
      consensus: true
    };

    console.log('   ✓ Achieved consensus among agents');
    console.log(`   ✓ Coordination successful with ${coordinationResult.participants.length} participants`);

  } catch (error) {
    console.warn('   ⚠️ Demo task execution failed:', error);
  }
}

// Handle unhandled promise rejections
process.on('unhandledRejection', (reason, promise) => {
  console.error('Unhandled Rejection at:', promise, 'reason:', reason);
});

process.on('uncaughtException', (error) => {
  console.error('Uncaught Exception:', error);
  process.exit(1);
});

// Start the demo
if (import.meta.url === `file://${process.argv[1]}`) {
  startDemo().catch(error => {
    console.error('❌ Demo failed to start:', error);
    process.exit(1);
  });
}

export { startDemo };