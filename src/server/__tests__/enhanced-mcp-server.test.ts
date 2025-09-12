import { describe, it, expect, beforeEach, afterEach, jest } from '@jest/globals';
import { EnhancedMCPServer } from '../enhanced-mcp-server';
import { CodeGraphIntegrationService } from '../codegraph-integration';
import { Client } from '@modelcontextprotocol/sdk/client/index.js';
import { StreamableHTTPClientTransport } from '@modelcontextprotocol/sdk/client/streamableHttp.js';
import { StdioClientTransport } from '@modelcontextprotocol/sdk/client/stdio.js';
import fetch from 'node-fetch';

// Mock fetch globally
global.fetch = fetch as any;

describe('EnhancedMCPServer', () => {
  let server: EnhancedMCPServer;
  let client: Client;
  
  beforeEach(() => {
    server = new EnhancedMCPServer({
      name: 'test-server',
      version: '1.0.0',
      transport: 'http',
      httpPort: 3333,
      enableCache: true,
      enableMetrics: true
    });
  });

  afterEach(async () => {
    if (client) {
      await client.close();
    }
    if (server) {
      await server.stop();
    }
  });

  describe('Server Initialization', () => {
    it('should initialize with correct configuration', () => {
      expect(server).toBeDefined();
      const metrics = server.getMetrics();
      expect(metrics.totalRequests).toBe(0);
      expect(metrics.activeSessions).toBe(0);
    });

    it('should start HTTP transport', async () => {
      await server.start();
      
      // Test health endpoint
      const response = await fetch('http://localhost:3333/health');
      const health = await response.json();
      
      expect(health.status).toBe('healthy');
      expect(health.version).toBe('1.0.0');
      expect(health.transport).toBe('http');
    });

    it('should support dual transport mode', async () => {
      const dualServer = new EnhancedMCPServer({
        name: 'dual-server',
        version: '1.0.0',
        transport: 'both',
        httpPort: 3334
      });

      await dualServer.start();
      
      // Verify HTTP is running
      const response = await fetch('http://localhost:3334/health');
      expect(response.status).toBe(200);
      
      await dualServer.stop();
    });
  });

  describe('CodeGraph Tools', () => {
    beforeEach(async () => {
      await server.start();
      
      client = new Client({
        name: 'test-client',
        version: '1.0.0'
      });
      
      const transport = new StreamableHTTPClientTransport(
        new URL('http://localhost:3333/mcp')
      );
      
      await client.connect(transport);
    });

    it('should list available tools', async () => {
      const tools = await client.listTools();
      
      expect(tools.tools).toBeDefined();
      expect(tools.tools.length).toBeGreaterThan(0);
      
      const toolNames = tools.tools.map(t => t.name);
      expect(toolNames).toContain('codegraph_find_dependencies');
      expect(toolNames).toContain('codegraph_reverse_dependencies');
      expect(toolNames).toContain('codegraph_architecture_analysis');
      expect(toolNames).toContain('discover_capabilities');
    });

    it('should discover capabilities', async () => {
      const result = await client.callTool({
        name: 'discover_capabilities',
        arguments: { category: 'all' }
      });

      expect(result.content).toBeDefined();
      expect(result.content[0].type).toBe('text');
      
      const capabilities = JSON.parse(result.content[0].text);
      expect(capabilities.analysis).toContain('codegraph_find_dependencies');
      expect(capabilities.architecture).toContain('codegraph_architecture_analysis');
      expect(capabilities.metrics).toContain('codegraph_complexity_metrics');
    });

    it('should handle batch analysis', async () => {
      const result = await client.callTool({
        name: 'batch_analysis',
        arguments: {
          analyses: [
            {
              type: 'dependencies',
              target: 'src/index.ts',
              options: { depth: 2 }
            },
            {
              type: 'complexity',
              target: 'src/utils.ts'
            }
          ]
        }
      });

      expect(result.content).toBeDefined();
      const batchResults = JSON.parse(result.content[0].text);
      expect(Array.isArray(batchResults)).toBe(true);
      expect(batchResults.length).toBe(2);
    });
  });

  describe('Performance Metrics', () => {
    beforeEach(async () => {
      await server.start();
      
      client = new Client({
        name: 'test-client',
        version: '1.0.0'
      });
      
      const transport = new StreamableHTTPClientTransport(
        new URL('http://localhost:3333/mcp')
      );
      
      await client.connect(transport);
    });

    it('should track performance metrics', async () => {
      // Make some tool calls
      await client.callTool({
        name: 'get_performance_stats',
        arguments: {}
      });

      const metrics = server.getMetrics();
      expect(metrics.totalRequests).toBeGreaterThan(0);
    });

    it('should expose metrics endpoint', async () => {
      const response = await fetch('http://localhost:3333/metrics');
      const metrics = await response.json();
      
      expect(metrics.server).toBeDefined();
      expect(metrics.server.uptime).toBeGreaterThan(0);
      expect(metrics.codegraph).toBeDefined();
    });

    it('should provide metrics resource', async () => {
      const resources = await client.listResources();
      const metricsResources = resources.resources.filter(r => 
        r.uri.startsWith('metrics://')
      );
      
      expect(metricsResources.length).toBeGreaterThan(0);
      
      const serverMetrics = await client.readResource({
        uri: 'metrics://server'
      });
      
      expect(serverMetrics.contents).toBeDefined();
      expect(serverMetrics.contents.length).toBe(1);
      
      const metrics = JSON.parse(serverMetrics.contents[0].text);
      expect(metrics.uptime).toBeDefined();
      expect(metrics.memoryUsage).toBeDefined();
    });
  });

  describe('Session Management', () => {
    it('should create and manage sessions', async () => {
      await server.start();
      
      const client1 = new Client({
        name: 'client-1',
        version: '1.0.0'
      });
      
      const transport1 = new StreamableHTTPClientTransport(
        new URL('http://localhost:3333/mcp')
      );
      
      await client1.connect(transport1);
      
      let metrics = server.getMetrics();
      expect(metrics.totalSessions).toBe(1);
      expect(metrics.activeSessions).toBe(1);
      
      // Create second client
      const client2 = new Client({
        name: 'client-2',
        version: '1.0.0'
      });
      
      const transport2 = new StreamableHTTPClientTransport(
        new URL('http://localhost:3333/mcp')
      );
      
      await client2.connect(transport2);
      
      metrics = server.getMetrics();
      expect(metrics.totalSessions).toBe(2);
      expect(metrics.activeSessions).toBe(2);
      
      // Close first client
      await client1.close();
      await new Promise(resolve => setTimeout(resolve, 100)); // Wait for cleanup
      
      metrics = server.getMetrics();
      expect(metrics.activeSessions).toBe(1);
      
      await client2.close();
    });
  });

  describe('Error Handling', () => {
    beforeEach(async () => {
      await server.start();
      
      client = new Client({
        name: 'test-client',
        version: '1.0.0'
      });
      
      const transport = new StreamableHTTPClientTransport(
        new URL('http://localhost:3333/mcp')
      );
      
      await client.connect(transport);
    });

    it('should handle invalid tool calls gracefully', async () => {
      try {
        await client.callTool({
          name: 'non_existent_tool',
          arguments: {}
        });
        fail('Should have thrown an error');
      } catch (error) {
        expect(error).toBeDefined();
      }
    });

    it('should handle malformed requests', async () => {
      const response = await fetch('http://localhost:3333/mcp', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ invalid: 'request' })
      });
      
      expect(response.status).toBe(400);
      const error = await response.json();
      expect(error.error).toBeDefined();
    });
  });
});

describe('CodeGraphIntegrationService', () => {
  let service: CodeGraphIntegrationService;
  
  beforeEach(() => {
    service = new CodeGraphIntegrationService('http://localhost:3030', {
      cacheEnabled: true,
      cacheTTL: 5000
    });
  });

  describe('Cache Management', () => {
    it('should cache query results', async () => {
      // Mock fetch to track calls
      const originalFetch = global.fetch;
      let fetchCallCount = 0;
      
      global.fetch = jest.fn(async () => {
        fetchCallCount++;
        return {
          ok: true,
          json: async () => ({
            directDependencies: [],
            transitiveDependencies: [],
            dependencyGraph: { nodes: [], edges: [] }
          })
        } as any;
      });

      // First call should hit the API
      await service.findDependencies('test.ts');
      expect(fetchCallCount).toBe(1);
      
      // Second call should use cache
      await service.findDependencies('test.ts');
      expect(fetchCallCount).toBe(1);
      
      // Different target should hit API again
      await service.findDependencies('other.ts');
      expect(fetchCallCount).toBe(2);
      
      global.fetch = originalFetch;
    });

    it('should clear cache', async () => {
      const originalFetch = global.fetch;
      let fetchCallCount = 0;
      
      global.fetch = jest.fn(async () => {
        fetchCallCount++;
        return {
          ok: true,
          json: async () => ({ result: 'test' })
        } as any;
      });

      await service.getComplexityMetrics('test.ts');
      expect(fetchCallCount).toBe(1);
      
      await service.getComplexityMetrics('test.ts');
      expect(fetchCallCount).toBe(1); // Cached
      
      service.clearCache();
      
      await service.getComplexityMetrics('test.ts');
      expect(fetchCallCount).toBe(2); // Cache cleared
      
      global.fetch = originalFetch;
    });
  });

  describe('Performance Metrics', () => {
    it('should track performance metrics', async () => {
      const originalFetch = global.fetch;
      
      global.fetch = jest.fn(async () => ({
        ok: true,
        json: async () => ({ cycles: [] })
      } as any));

      await service.findCyclicDependencies('src');
      
      const metrics = service.getPerformanceMetrics();
      expect(metrics.has('findCyclicDependencies')).toBe(true);
      
      const metric = metrics.get('findCyclicDependencies');
      expect(metric?.count).toBe(1);
      expect(metric?.avg).toBeGreaterThan(0);
      
      global.fetch = originalFetch;
    });
  });

  describe('Error Handling', () => {
    it('should handle API errors', async () => {
      const originalFetch = global.fetch;
      
      global.fetch = jest.fn(async () => ({
        ok: false,
        statusText: 'Internal Server Error'
      } as any));

      let errorEmitted = false;
      service.on('error', () => {
        errorEmitted = true;
      });

      try {
        await service.analyzeArchitecture('src');
        fail('Should have thrown an error');
      } catch (error) {
        expect(error).toBeDefined();
        expect(errorEmitted).toBe(true);
      }
      
      global.fetch = originalFetch;
    });
  });
});