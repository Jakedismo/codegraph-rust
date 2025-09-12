import { McpServer } from '@modelcontextprotocol/sdk/server/mcp.js';
import { StdioServerTransport } from '@modelcontextprotocol/sdk/server/stdio.js';
import { StreamableHTTPServerTransport } from '@modelcontextprotocol/sdk/server/streamableHttp.js';
import express, { Request, Response } from 'express';
import cors from 'cors';
import { randomUUID } from 'crypto';
import { z } from 'zod';
import { EventEmitter } from 'events';
import { 
  CodeGraphIntegrationService, 
  registerCodeGraphTools 
} from './codegraph-integration.js';
import { isInitializeRequest } from '@modelcontextprotocol/sdk/types.js';

export interface EnhancedMCPServerConfig {
  name: string;
  version: string;
  transport: 'stdio' | 'http' | 'both';
  httpPort?: number;
  corsOrigins?: string[];
  codeGraphApiUrl?: string;
  enableCache?: boolean;
  enableMetrics?: boolean;
  enableDnsRebindingProtection?: boolean;
  allowedHosts?: string[];
  sessionIdGenerator?: () => string;
}

export class EnhancedMCPServer extends EventEmitter {
  private mcpServer: McpServer;
  private codeGraphService: CodeGraphIntegrationService;
  private config: Required<EnhancedMCPServerConfig>;
  private transports: Map<string, StreamableHTTPServerTransport>;
  private app?: express.Application;
  private stdioTransport?: StdioServerTransport;
  
  // Metrics tracking
  private metrics = {
    totalRequests: 0,
    totalSessions: 0,
    activeSessions: 0,
    toolCalls: new Map<string, number>(),
    errors: 0,
    startTime: Date.now()
  };

  constructor(config: EnhancedMCPServerConfig) {
    super();
    
    this.config = {
      name: config.name,
      version: config.version,
      transport: config.transport,
      httpPort: config.httpPort ?? 3000,
      corsOrigins: config.corsOrigins ?? ['*'],
      codeGraphApiUrl: config.codeGraphApiUrl ?? 'http://localhost:3030',
      enableCache: config.enableCache ?? true,
      enableMetrics: config.enableMetrics ?? true,
      enableDnsRebindingProtection: config.enableDnsRebindingProtection ?? true,
      allowedHosts: config.allowedHosts ?? ['127.0.0.1', 'localhost'],
      sessionIdGenerator: config.sessionIdGenerator ?? (() => randomUUID())
    };
    
    this.transports = new Map();
    this.initializeComponents();
  }

  private initializeComponents(): void {
    // Initialize MCP server with debouncing for better performance
    this.mcpServer = new McpServer(
      {
        name: this.config.name,
        version: this.config.version
      },
      {
        debouncedNotificationMethods: [
          'notifications/tools/list_changed',
          'notifications/resources/list_changed',
          'notifications/prompts/list_changed'
        ]
      }
    );

    // Initialize CodeGraph integration service
    this.codeGraphService = new CodeGraphIntegrationService(
      this.config.codeGraphApiUrl,
      {
        cacheEnabled: this.config.enableCache,
        cacheTTL: 300000, // 5 minutes
        maxCacheSize: 100
      }
    );

    // Register CodeGraph tools
    registerCodeGraphTools(this.mcpServer, this.codeGraphService);
    
    // Register additional analytical tools
    this.registerAnalyticalTools();
    
    // Register performance monitoring resources
    if (this.config.enableMetrics) {
      this.registerMetricsResources();
    }

    // Setup transport error handling
    this.codeGraphService.on('error', (error) => {
      this.metrics.errors++;
      this.emit('codegraph:error', error);
    });
  }

  private registerAnalyticalTools(): void {
    // Tool capability discovery
    this.mcpServer.registerTool(
      'discover_capabilities',
      {
        title: 'Discover Tool Capabilities',
        description: 'List all available CodeGraph analysis capabilities',
        inputSchema: {
          category: z.enum(['all', 'analysis', 'architecture', 'metrics', 'search']).optional()
        }
      },
      async ({ category }) => {
        const capabilities = {
          analysis: [
            'codegraph_find_dependencies',
            'codegraph_reverse_dependencies',
            'codegraph_cyclic_dependencies'
          ],
          architecture: [
            'codegraph_architecture_analysis',
            'codegraph_function_relationships',
            'codegraph_file_mapping'
          ],
          metrics: [
            'codegraph_complexity_metrics',
            'get_performance_stats'
          ],
          search: [
            'codegraph_pattern_search'
          ]
        };

        const result = category === 'all' || !category 
          ? capabilities 
          : { [category]: capabilities[category] };

        return {
          content: [{
            type: 'text',
            text: JSON.stringify(result, null, 2)
          }]
        };
      }
    );

    // Batch analysis tool for performance
    this.mcpServer.registerTool(
      'batch_analysis',
      {
        title: 'Batch Code Analysis',
        description: 'Perform multiple analyses in parallel for better performance',
        inputSchema: {
          analyses: z.array(z.object({
            type: z.enum([
              'dependencies',
              'reverse_dependencies',
              'functions',
              'complexity',
              'architecture'
            ]),
            target: z.string(),
            options: z.record(z.any()).optional()
          }))
        }
      },
      async ({ analyses }) => {
        const results = await Promise.all(
          analyses.map(async (analysis) => {
            try {
              switch (analysis.type) {
                case 'dependencies':
                  return {
                    type: analysis.type,
                    result: await this.codeGraphService.findDependencies(
                      analysis.target,
                      analysis.options
                    )
                  };
                case 'reverse_dependencies':
                  return {
                    type: analysis.type,
                    result: await this.codeGraphService.findReverseDependencies(
                      analysis.target,
                      analysis.options
                    )
                  };
                case 'functions':
                  return {
                    type: analysis.type,
                    result: await this.codeGraphService.discoverFunctionRelationships(
                      analysis.target,
                      analysis.options
                    )
                  };
                case 'complexity':
                  return {
                    type: analysis.type,
                    result: await this.codeGraphService.getComplexityMetrics(
                      analysis.target,
                      analysis.options
                    )
                  };
                case 'architecture':
                  return {
                    type: analysis.type,
                    result: await this.codeGraphService.analyzeArchitecture(
                      analysis.target,
                      analysis.options
                    )
                  };
                default:
                  return {
                    type: analysis.type,
                    error: 'Unknown analysis type'
                  };
              }
            } catch (error) {
              return {
                type: analysis.type,
                error: error instanceof Error ? error.message : 'Unknown error'
              };
            }
          })
        );

        return {
          content: [{
            type: 'text',
            text: JSON.stringify(results, null, 2)
          }]
        };
      }
    );

    // Performance statistics tool
    this.mcpServer.registerTool(
      'get_performance_stats',
      {
        title: 'Get Performance Statistics',
        description: 'Get performance metrics for CodeGraph operations',
        inputSchema: {}
      },
      async () => {
        const metrics = this.codeGraphService.getPerformanceMetrics();
        const stats = Object.fromEntries(metrics);
        
        return {
          content: [{
            type: 'text',
            text: JSON.stringify({
              codeGraphMetrics: stats,
              serverMetrics: {
                uptime: Date.now() - this.metrics.startTime,
                totalRequests: this.metrics.totalRequests,
                activeSessions: this.metrics.activeSessions,
                errors: this.metrics.errors
              }
            }, null, 2)
          }]
        };
      }
    );

    // Clear cache tool
    this.mcpServer.registerTool(
      'clear_analysis_cache',
      {
        title: 'Clear Analysis Cache',
        description: 'Clear the CodeGraph analysis cache',
        inputSchema: {}
      },
      async () => {
        this.codeGraphService.clearCache();
        
        return {
          content: [{
            type: 'text',
            text: 'Cache cleared successfully'
          }]
        };
      }
    );
  }

  private registerMetricsResources(): void {
    // Server metrics resource
    this.mcpServer.registerResource(
      'server-metrics',
      'metrics://server',
      {
        title: 'Server Metrics',
        description: 'Real-time server performance metrics',
        mimeType: 'application/json'
      },
      async (uri) => ({
        contents: [{
          uri: uri.href,
          text: JSON.stringify({
            uptime: Date.now() - this.metrics.startTime,
            totalRequests: this.metrics.totalRequests,
            totalSessions: this.metrics.totalSessions,
            activeSessions: this.metrics.activeSessions,
            toolCalls: Object.fromEntries(this.metrics.toolCalls),
            errors: this.metrics.errors,
            memoryUsage: process.memoryUsage(),
            cpuUsage: process.cpuUsage()
          }, null, 2)
        }]
      })
    );

    // CodeGraph performance resource
    this.mcpServer.registerResource(
      'codegraph-performance',
      'metrics://codegraph',
      {
        title: 'CodeGraph Performance',
        description: 'CodeGraph operation performance metrics',
        mimeType: 'application/json'
      },
      async (uri) => {
        const metrics = this.codeGraphService.getPerformanceMetrics();
        return {
          contents: [{
            uri: uri.href,
            text: JSON.stringify(Object.fromEntries(metrics), null, 2)
          }]
        };
      }
    );
  }

  private setupStdioTransport(): void {
    this.stdioTransport = new StdioServerTransport();
    
    this.mcpServer.connect(this.stdioTransport)
      .then(() => {
        this.emit('transport:connected', { type: 'stdio' });
        console.error('MCP server running on stdio transport');
      })
      .catch((error) => {
        this.emit('transport:error', { type: 'stdio', error });
        console.error('Failed to start stdio transport:', error);
      });
  }

  private setupHttpTransport(): void {
    this.app = express();
    this.app.use(express.json());
    
    // Setup CORS
    this.app.use(cors({
      origin: this.config.corsOrigins,
      exposedHeaders: ['Mcp-Session-Id'],
      allowedHeaders: ['Content-Type', 'mcp-session-id']
    }));

    // Health check endpoint
    this.app.get('/health', (req, res) => {
      res.json({
        status: 'healthy',
        version: this.config.version,
        transport: 'http',
        sessions: this.transports.size
      });
    });

    // Main MCP endpoint
    this.app.post('/mcp', async (req: Request, res: Response) => {
      this.metrics.totalRequests++;
      
      const sessionId = req.headers['mcp-session-id'] as string | undefined;
      let transport: StreamableHTTPServerTransport;

      try {
        if (sessionId && this.transports.has(sessionId)) {
          transport = this.transports.get(sessionId)!;
        } else if (!sessionId && isInitializeRequest(req.body)) {
          // Create new session
          transport = new StreamableHTTPServerTransport({
            sessionIdGenerator: this.config.sessionIdGenerator,
            enableDnsRebindingProtection: this.config.enableDnsRebindingProtection,
            allowedHosts: this.config.allowedHosts,
            onsessioninitialized: (newSessionId) => {
              this.transports.set(newSessionId, transport);
              this.metrics.totalSessions++;
              this.metrics.activeSessions++;
              this.emit('session:created', { sessionId: newSessionId });
            }
          });

          transport.onclose = () => {
            if (transport.sessionId) {
              this.transports.delete(transport.sessionId);
              this.metrics.activeSessions--;
              this.emit('session:closed', { sessionId: transport.sessionId });
            }
          };

          await this.mcpServer.connect(transport);
        } else {
          res.status(400).json({
            jsonrpc: '2.0',
            error: {
              code: -32000,
              message: 'Bad Request: No valid session ID provided'
            },
            id: null
          });
          return;
        }

        await transport.handleRequest(req, res, req.body);
      } catch (error) {
        this.metrics.errors++;
        console.error('Error handling MCP request:', error);
        
        if (!res.headersSent) {
          res.status(500).json({
            jsonrpc: '2.0',
            error: {
              code: -32603,
              message: 'Internal server error'
            },
            id: null
          });
        }
      }
    });

    // SSE endpoint for notifications
    this.app.get('/mcp', async (req: Request, res: Response) => {
      const sessionId = req.headers['mcp-session-id'] as string | undefined;
      
      if (!sessionId || !this.transports.has(sessionId)) {
        res.status(400).send('Invalid or missing session ID');
        return;
      }
      
      const transport = this.transports.get(sessionId)!;
      await transport.handleRequest(req, res);
    });

    // Session termination endpoint
    this.app.delete('/mcp', async (req: Request, res: Response) => {
      const sessionId = req.headers['mcp-session-id'] as string | undefined;
      
      if (!sessionId || !this.transports.has(sessionId)) {
        res.status(400).send('Invalid or missing session ID');
        return;
      }
      
      const transport = this.transports.get(sessionId)!;
      await transport.handleRequest(req, res);
    });

    // Metrics endpoint
    if (this.config.enableMetrics) {
      this.app.get('/metrics', (req, res) => {
        res.json({
          server: {
            uptime: Date.now() - this.metrics.startTime,
            totalRequests: this.metrics.totalRequests,
            totalSessions: this.metrics.totalSessions,
            activeSessions: this.metrics.activeSessions,
            errors: this.metrics.errors
          },
          codegraph: Object.fromEntries(this.codeGraphService.getPerformanceMetrics())
        });
      });
    }

    // Start HTTP server
    this.app.listen(this.config.httpPort, () => {
      this.emit('transport:connected', { 
        type: 'http', 
        port: this.config.httpPort 
      });
      console.error(`MCP server running on HTTP port ${this.config.httpPort}`);
    });
  }

  public async start(): Promise<void> {
    try {
      this.emit('server:starting');
      
      switch (this.config.transport) {
        case 'stdio':
          this.setupStdioTransport();
          break;
        case 'http':
          this.setupHttpTransport();
          break;
        case 'both':
          this.setupStdioTransport();
          this.setupHttpTransport();
          break;
        default:
          throw new Error(`Unknown transport: ${this.config.transport}`);
      }
      
      this.emit('server:started', {
        transport: this.config.transport,
        httpPort: this.config.transport !== 'stdio' ? this.config.httpPort : undefined
      });
    } catch (error) {
      this.emit('server:error', error);
      throw error;
    }
  }

  public async stop(): Promise<void> {
    try {
      this.emit('server:stopping');
      
      // Close all HTTP transports
      for (const transport of this.transports.values()) {
        await transport.close();
      }
      
      // Close stdio transport if exists
      if (this.stdioTransport) {
        await this.stdioTransport.close();
      }
      
      this.emit('server:stopped');
    } catch (error) {
      this.emit('server:error', error);
      throw error;
    }
  }

  public getMetrics() {
    return {
      ...this.metrics,
      codeGraphMetrics: Object.fromEntries(this.codeGraphService.getPerformanceMetrics())
    };
  }
}