import { exec } from 'child_process';
import { promisify } from 'util';
import { z } from 'zod';
import { McpServer } from '@modelcontextprotocol/sdk/server/mcp.js';
import { EventEmitter } from 'events';
import path from 'path';
import fs from 'fs/promises';
import { PerformanceOptimizer, QueryOptimizationStrategy } from './performance-optimizer.js';

const execAsync = promisify(exec);

export interface CodeGraphNode {
  id: string;
  name: string;
  type: 'function' | 'class' | 'module' | 'method' | 'variable' | 'type';
  file: string;
  line: number;
  column: number;
  language: string;
  content?: string;
  metadata?: Record<string, any>;
}

export interface DependencyEdge {
  from: string;
  to: string;
  type: 'imports' | 'calls' | 'extends' | 'implements' | 'uses' | 'references';
  weight?: number;
  metadata?: Record<string, any>;
}

export interface ArchitectureInfo {
  nodes: CodeGraphNode[];
  edges: DependencyEdge[];
  clusters?: Map<string, string[]>;
  metrics?: {
    totalNodes: number;
    totalEdges: number;
    avgDependencies: number;
    cyclomaticComplexity?: number;
    cohesion?: number;
    coupling?: number;
  };
}

export interface AnalysisOptions {
  depth?: number;
  includeTests?: boolean;
  includeVendor?: boolean;
  maxNodes?: number;
  timeout?: number;
  parallelism?: number;
}

export class CodeGraphIntegrationService extends EventEmitter {
  private apiUrl: string;
  private cacheEnabled: boolean;
  private cache: Map<string, any>;
  private performanceMetrics: Map<string, number[]>;
  private optimizer: PerformanceOptimizer;

  constructor(
    private apiHost: string = 'http://localhost:3030',
    private options: {
      cacheEnabled?: boolean;
      cacheTTL?: number;
      maxCacheSize?: number;
    } = {}
  ) {
    super();
    this.apiUrl = apiHost;
    this.cacheEnabled = options.cacheEnabled ?? true;
    this.cache = new Map();
    this.performanceMetrics = new Map();
    this.optimizer = new PerformanceOptimizer({
      maxConcurrency: 10,
      cacheSize: 1000,
      cacheTTL: options.cacheTTL,
      enableQueryOptimization: true
    });
  }

  /**
   * Find all dependencies of a given file or function
   */
  async findDependencies(
    target: string,
    options: AnalysisOptions = {}
  ): Promise<{
    directDependencies: CodeGraphNode[];
    transitiveDependencies: CodeGraphNode[];
    dependencyGraph: ArchitectureInfo;
  }> {
    // Optimize the query
    const optimized = this.optimizer.optimizeDependencyQuery({
      target,
      depth: options.depth,
      includeTests: options.includeTests
    });
    
    const finalOptions = { ...options, ...optimized.query };
    const cacheKey = this.optimizer.createCacheKey('findDependencies', { target, finalOptions });

    return this.optimizer.executeQuery(
      async () => {
        const response = await fetch(`${this.apiUrl}/api/dependencies`, {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ target, options: finalOptions })
        });

        if (!response.ok) {
          throw new Error(`CodeGraph API error: ${response.statusText}`);
        }

        const result = await response.json();
        return this.optimizer.compressResult(result);
      },
      cacheKey,
      { skipCache: options.depth && options.depth > 5 } // Skip cache for deep queries
    ).then(result => this.optimizer.decompressResult(result));
  }

  /**
   * Discover function relationships and call graphs
   */
  async discoverFunctionRelationships(
    filePath: string,
    options: AnalysisOptions = {}
  ): Promise<{
    functions: CodeGraphNode[];
    callGraph: DependencyEdge[];
    clusters: Map<string, string[]>;
  }> {
    const startTime = Date.now();
    const cacheKey = `func:${filePath}:${JSON.stringify(options)}`;

    if (this.cacheEnabled && this.cache.has(cacheKey)) {
      return this.cache.get(cacheKey);
    }

    try {
      const response = await fetch(`${this.apiUrl}/api/functions`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ filePath, options })
      });

      if (!response.ok) {
        throw new Error(`CodeGraph API error: ${response.statusText}`);
      }

      const result = await response.json();
      
      if (this.cacheEnabled) {
        this.cache.set(cacheKey, result);
      }

      this.recordMetric('discoverFunctionRelationships', Date.now() - startTime);
      return result;
    } catch (error) {
      this.emit('error', { operation: 'discoverFunctionRelationships', error });
      throw error;
    }
  }

  /**
   * Map files to their contained functions and classes
   */
  async mapFileToFunctions(
    pattern: string,
    options: AnalysisOptions = {}
  ): Promise<Map<string, CodeGraphNode[]>> {
    // Optimize file patterns for better performance
    const patterns = QueryOptimizationStrategy.optimizeFilePattern(pattern);
    
    if (patterns.length > 1) {
      // Execute optimized patterns in parallel
      const results = await Promise.all(
        patterns.map(p => this.mapSinglePattern(p, options))
      );
      
      // Merge results
      const merged = new Map<string, CodeGraphNode[]>();
      for (const result of results) {
        for (const [file, nodes] of result) {
          merged.set(file, nodes);
        }
      }
      return merged;
    }
    
    return this.mapSinglePattern(pattern, options);
  }

  private async mapSinglePattern(
    pattern: string,
    options: AnalysisOptions = {}
  ): Promise<Map<string, CodeGraphNode[]>> {
    const cacheKey = this.optimizer.createCacheKey('mapFileToFunctions', { pattern, options });

    return this.optimizer.executeQuery(
      async () => {
        const response = await fetch(`${this.apiUrl}/api/file-mapping`, {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ pattern, options })
        });

        if (!response.ok) {
          throw new Error(`CodeGraph API error: ${response.statusText}`);
        }

        const result = await response.json();
        return new Map(Object.entries(result));
      },
      cacheKey
    );
  }

  /**
   * Find reverse dependencies (what depends on this)
   */
  async findReverseDependencies(
    target: string,
    options: AnalysisOptions = {}
  ): Promise<{
    directDependents: CodeGraphNode[];
    transitiveDependents: CodeGraphNode[];
    impactAnalysis: {
      affectedFiles: string[];
      affectedFunctions: string[];
      riskLevel: 'low' | 'medium' | 'high';
    };
  }> {
    const startTime = Date.now();
    const cacheKey = `revdeps:${target}:${JSON.stringify(options)}`;

    if (this.cacheEnabled && this.cache.has(cacheKey)) {
      return this.cache.get(cacheKey);
    }

    try {
      const response = await fetch(`${this.apiUrl}/api/reverse-dependencies`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ target, options })
      });

      if (!response.ok) {
        throw new Error(`CodeGraph API error: ${response.statusText}`);
      }

      const result = await response.json();

      if (this.cacheEnabled) {
        this.cache.set(cacheKey, result);
      }

      this.recordMetric('findReverseDependencies', Date.now() - startTime);
      return result;
    } catch (error) {
      this.emit('error', { operation: 'findReverseDependencies', error });
      throw error;
    }
  }

  /**
   * Analyze architecture patterns and anti-patterns
   */
  async analyzeArchitecture(
    rootPath: string,
    options: AnalysisOptions = {}
  ): Promise<{
    architecture: ArchitectureInfo;
    patterns: Array<{
      type: string;
      instances: string[];
      severity?: 'info' | 'warning' | 'error';
    }>;
    recommendations: string[];
  }> {
    const startTime = Date.now();

    try {
      const response = await fetch(`${this.apiUrl}/api/architecture-analysis`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ rootPath, options })
      });

      if (!response.ok) {
        throw new Error(`CodeGraph API error: ${response.statusText}`);
      }

      const result = await response.json();
      this.recordMetric('analyzeArchitecture', Date.now() - startTime);
      return result;
    } catch (error) {
      this.emit('error', { operation: 'analyzeArchitecture', error });
      throw error;
    }
  }

  /**
   * Find cyclic dependencies in the codebase
   */
  async findCyclicDependencies(
    rootPath: string,
    options: AnalysisOptions = {}
  ): Promise<{
    cycles: Array<{
      nodes: string[];
      type: 'import' | 'call' | 'mixed';
      severity: 'low' | 'medium' | 'high';
    }>;
    suggestions: string[];
  }> {
    const startTime = Date.now();

    try {
      const response = await fetch(`${this.apiUrl}/api/cyclic-dependencies`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ rootPath, options })
      });

      if (!response.ok) {
        throw new Error(`CodeGraph API error: ${response.statusText}`);
      }

      const result = await response.json();
      this.recordMetric('findCyclicDependencies', Date.now() - startTime);
      return result;
    } catch (error) {
      this.emit('error', { operation: 'findCyclicDependencies', error });
      throw error;
    }
  }

  /**
   * Get complexity metrics for code analysis
   */
  async getComplexityMetrics(
    target: string,
    options: AnalysisOptions = {}
  ): Promise<{
    cyclomaticComplexity: number;
    cognitiveComplexity: number;
    halsteadMetrics: {
      volume: number;
      difficulty: number;
      effort: number;
    };
    maintainabilityIndex: number;
  }> {
    const startTime = Date.now();

    try {
      const response = await fetch(`${this.apiUrl}/api/complexity-metrics`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ target, options })
      });

      if (!response.ok) {
        throw new Error(`CodeGraph API error: ${response.statusText}`);
      }

      const result = await response.json();
      this.recordMetric('getComplexityMetrics', Date.now() - startTime);
      return result;
    } catch (error) {
      this.emit('error', { operation: 'getComplexityMetrics', error });
      throw error;
    }
  }

  /**
   * Search for code patterns using semantic analysis
   */
  async searchCodePatterns(
    pattern: string,
    options: AnalysisOptions & { semantic?: boolean } = {}
  ): Promise<{
    matches: Array<{
      file: string;
      line: number;
      confidence: number;
      snippet: string;
    }>;
    totalMatches: number;
  }> {
    const startTime = Date.now();

    try {
      const response = await fetch(`${this.apiUrl}/api/pattern-search`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ pattern, options })
      });

      if (!response.ok) {
        throw new Error(`CodeGraph API error: ${response.statusText}`);
      }

      const result = await response.json();
      this.recordMetric('searchCodePatterns', Date.now() - startTime);
      return result;
    } catch (error) {
      this.emit('error', { operation: 'searchCodePatterns', error });
      throw error;
    }
  }

  /**
   * Clear the cache
   */
  clearCache(): void {
    this.cache.clear();
    this.optimizer.reset();
    this.emit('cache:cleared');
  }

  /**
   * Get optimizer metrics
   */
  getOptimizerMetrics() {
    return {
      query: this.optimizer.getMetrics(),
      cache: this.optimizer.getCacheStats(),
      queue: this.optimizer.getQueueStats()
    };
  }

  /**
   * Get performance metrics
   */
  getPerformanceMetrics(): Map<string, { avg: number; min: number; max: number; count: number }> {
    const metrics = new Map();
    
    for (const [operation, times] of this.performanceMetrics) {
      if (times.length === 0) continue;
      
      metrics.set(operation, {
        avg: times.reduce((a, b) => a + b, 0) / times.length,
        min: Math.min(...times),
        max: Math.max(...times),
        count: times.length
      });
    }
    
    return metrics;
  }

  private recordMetric(operation: string, time: number): void {
    if (!this.performanceMetrics.has(operation)) {
      this.performanceMetrics.set(operation, []);
    }
    
    const metrics = this.performanceMetrics.get(operation)!;
    metrics.push(time);
    
    // Keep only last 100 measurements
    if (metrics.length > 100) {
      metrics.shift();
    }
  }
}

/**
 * Register CodeGraph tools with MCP server
 */
export function registerCodeGraphTools(
  mcpServer: McpServer,
  integrationService: CodeGraphIntegrationService
): void {
  // Find dependencies tool
  mcpServer.registerTool(
    'codegraph_find_dependencies',
    {
      title: 'Find Dependencies',
      description: 'Find all dependencies of a file, function, or module',
      inputSchema: {
        target: z.string().describe('File path, function name, or module to analyze'),
        depth: z.number().optional().describe('Maximum dependency depth to traverse'),
        includeTests: z.boolean().optional().describe('Include test files in analysis'),
        includeVendor: z.boolean().optional().describe('Include vendor/node_modules dependencies')
      }
    },
    async ({ target, depth, includeTests, includeVendor }) => {
      const result = await integrationService.findDependencies(target, {
        depth,
        includeTests,
        includeVendor
      });

      return {
        content: [{
          type: 'text',
          text: JSON.stringify(result, null, 2)
        }]
      };
    }
  );

  // Discover function relationships tool
  mcpServer.registerTool(
    'codegraph_function_relationships',
    {
      title: 'Discover Function Relationships',
      description: 'Analyze function call graphs and relationships within files',
      inputSchema: {
        filePath: z.string().describe('File path to analyze'),
        maxNodes: z.number().optional().describe('Maximum number of nodes to return'),
        includeTests: z.boolean().optional().describe('Include test functions')
      }
    },
    async ({ filePath, maxNodes, includeTests }) => {
      const result = await integrationService.discoverFunctionRelationships(filePath, {
        maxNodes,
        includeTests
      });

      return {
        content: [{
          type: 'text',
          text: JSON.stringify(result, null, 2)
        }]
      };
    }
  );

  // File to function mapping tool
  mcpServer.registerTool(
    'codegraph_file_mapping',
    {
      title: 'Map Files to Functions',
      description: 'Map files to their contained functions, classes, and methods',
      inputSchema: {
        pattern: z.string().describe('File pattern to analyze (e.g., "src/**/*.ts")'),
        includeTests: z.boolean().optional().describe('Include test files')
      }
    },
    async ({ pattern, includeTests }) => {
      const mapping = await integrationService.mapFileToFunctions(pattern, {
        includeTests
      });

      const result = Object.fromEntries(mapping);
      return {
        content: [{
          type: 'text',
          text: JSON.stringify(result, null, 2)
        }]
      };
    }
  );

  // Reverse dependency lookup tool
  mcpServer.registerTool(
    'codegraph_reverse_dependencies',
    {
      title: 'Find Reverse Dependencies',
      description: 'Find what depends on a given file, function, or module',
      inputSchema: {
        target: z.string().describe('File path, function name, or module to analyze'),
        depth: z.number().optional().describe('Maximum dependency depth to traverse')
      }
    },
    async ({ target, depth }) => {
      const result = await integrationService.findReverseDependencies(target, {
        depth
      });

      return {
        content: [{
          type: 'text',
          text: JSON.stringify(result, null, 2)
        }]
      };
    }
  );

  // Architecture analysis tool
  mcpServer.registerTool(
    'codegraph_architecture_analysis',
    {
      title: 'Analyze Architecture',
      description: 'Analyze codebase architecture, patterns, and anti-patterns',
      inputSchema: {
        rootPath: z.string().describe('Root directory to analyze'),
        maxNodes: z.number().optional().describe('Maximum nodes to analyze'),
        timeout: z.number().optional().describe('Analysis timeout in seconds')
      }
    },
    async ({ rootPath, maxNodes, timeout }) => {
      const result = await integrationService.analyzeArchitecture(rootPath, {
        maxNodes,
        timeout: timeout ? timeout * 1000 : undefined
      });

      return {
        content: [{
          type: 'text',
          text: JSON.stringify(result, null, 2)
        }]
      };
    }
  );

  // Cyclic dependencies detection tool
  mcpServer.registerTool(
    'codegraph_cyclic_dependencies',
    {
      title: 'Find Cyclic Dependencies',
      description: 'Detect circular dependencies in the codebase',
      inputSchema: {
        rootPath: z.string().describe('Root directory to analyze'),
        includeTests: z.boolean().optional().describe('Include test files in analysis')
      }
    },
    async ({ rootPath, includeTests }) => {
      const result = await integrationService.findCyclicDependencies(rootPath, {
        includeTests
      });

      return {
        content: [{
          type: 'text',
          text: JSON.stringify(result, null, 2)
        }]
      };
    }
  );

  // Complexity metrics tool
  mcpServer.registerTool(
    'codegraph_complexity_metrics',
    {
      title: 'Get Complexity Metrics',
      description: 'Calculate complexity metrics for code analysis',
      inputSchema: {
        target: z.string().describe('File or function to analyze')
      }
    },
    async ({ target }) => {
      const result = await integrationService.getComplexityMetrics(target);

      return {
        content: [{
          type: 'text',
          text: JSON.stringify(result, null, 2)
        }]
      };
    }
  );

  // Code pattern search tool
  mcpServer.registerTool(
    'codegraph_pattern_search',
    {
      title: 'Search Code Patterns',
      description: 'Search for code patterns using semantic analysis',
      inputSchema: {
        pattern: z.string().describe('Pattern to search for'),
        semantic: z.boolean().optional().describe('Use semantic search (AI-powered)'),
        maxNodes: z.number().optional().describe('Maximum results to return')
      }
    },
    async ({ pattern, semantic, maxNodes }) => {
      const result = await integrationService.searchCodePatterns(pattern, {
        semantic,
        maxNodes
      });

      return {
        content: [{
          type: 'text',
          text: JSON.stringify(result, null, 2)
        }]
      };
    }
  );

  // Performance metrics resource
  mcpServer.registerResource(
    'codegraph-metrics',
    'codegraph://metrics',
    {
      title: 'CodeGraph Performance Metrics',
      description: 'Performance metrics for CodeGraph operations',
      mimeType: 'application/json'
    },
    async (uri) => {
      const metrics = integrationService.getPerformanceMetrics();
      const metricsObj = Object.fromEntries(metrics);
      
      return {
        contents: [{
          uri: uri.href,
          text: JSON.stringify(metricsObj, null, 2)
        }]
      };
    }
  );
}