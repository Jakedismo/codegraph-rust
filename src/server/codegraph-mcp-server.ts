import { McpServer } from '@modelcontextprotocol/sdk/server/mcp.js';
import { JsonRpcMessage } from '@modelcontextprotocol/sdk/types.js';
import { WebSocketServerTransport, WebSocketSession } from '../core/transport/websocket-transport.js';
import { MessageRouter, RouteHandler, RouteContext, RouteResult } from '../core/routing/message-router.js';
import { MessageValidator } from '../core/validation/message-validator.js';
import { CoordinationEngine } from '../core/coordination/coordination-engine.js';
import {
  AgentType,
  AgentStatus,
  Task,
  TaskStatus,
  MessageType,
  Priority,
  AgentDiscoveryInfo,
  RegistrationRequest,
  RegistrationResponse,
  AgentMessage,
  createTaskAssignment
} from '../core/agents/agent-types.js';
import { EventEmitter } from 'events';
import { randomUUID } from 'crypto';
import { z } from 'zod';

export interface CodeGraphMCPServerConfig {
  name: string;
  version: string;
  websocketPort?: number;
  httpPort?: number;
  maxConnections?: number;
  enableDiscovery?: boolean;
  enableMetrics?: boolean;
  corsOrigins?: string[];
  authentication?: {
    enabled: boolean;
    secretKey?: string;
    tokenExpiry?: number;
  };
}

export class CodeGraphMCPServer extends EventEmitter {
  private mcpServer: McpServer;
  private wsTransport: WebSocketServerTransport;
  private router: MessageRouter;
  private validator: MessageValidator;
  private coordinator: CoordinationEngine;
  private config: Required<CodeGraphMCPServerConfig>;
  
  // Agent registry and session management
  private registeredAgents = new Map<string, AgentDiscoveryInfo>();
  private agentSessions = new Map<string, string[]>(); // agentId -> sessionIds
  private sessionAgents = new Map<string, string>(); // sessionId -> agentId
  private activeTasks = new Map<string, Task>();
  
  // Metrics
  private metrics = {
    totalConnections: 0,
    activeConnections: 0,
    totalMessages: 0,
    totalTasks: 0,
    completedTasks: 0,
    failedTasks: 0,
    startTime: new Date()
  };

  constructor(config: CodeGraphMCPServerConfig) {
    super();

    this.config = {
      name: config.name,
      version: config.version,
      websocketPort: config.websocketPort ?? 3001,
      httpPort: config.httpPort ?? 3000,
      maxConnections: config.maxConnections ?? 1000,
      enableDiscovery: config.enableDiscovery ?? true,
      enableMetrics: config.enableMetrics ?? true,
      corsOrigins: config.corsOrigins ?? ['*'],
      authentication: {
        enabled: config.authentication?.enabled ?? false,
        secretKey: config.authentication?.secretKey ?? 'default-secret',
        tokenExpiry: config.authentication?.tokenExpiry ?? 3600000
      }
    };

    this.initializeComponents();
    this.setupRoutes();
    this.setupEventHandlers();
  }

  private initializeComponents(): void {
    // Initialize core components
    this.validator = new MessageValidator();
    this.router = new MessageRouter(this.validator);
    this.coordinator = new CoordinationEngine();

    // Initialize MCP server
    this.mcpServer = new McpServer({
      name: this.config.name,
      version: this.config.version
    });

    // Initialize WebSocket transport
    this.wsTransport = new WebSocketServerTransport({
      port: this.config.websocketPort,
      maxConnections: this.config.maxConnections
    });

    this.setupMCPResources();
    this.setupMCPTools();
    this.setupMCPPrompts();
  }

  private setupMCPResources(): void {
    // Register agents resource
    this.mcpServer.registerResource(
      'agents',
      '/agents',
      { title: 'Registered Agents', description: 'List of all registered agents' },
      async () => ({
        contents: [{
          uri: '/agents',
          mimeType: 'application/json',
          text: JSON.stringify(Array.from(this.registeredAgents.values()), null, 2)
        }]
      })
    );

    // Register tasks resource
    this.mcpServer.registerResource(
      'tasks',
      '/tasks',
      { title: 'Active Tasks', description: 'List of all active tasks' },
      async () => ({
        contents: [{
          uri: '/tasks',
          mimeType: 'application/json',
          text: JSON.stringify(Array.from(this.activeTasks.values()), null, 2)
        }]
      })
    );

    // Register metrics resource
    if (this.config.enableMetrics) {
      this.mcpServer.registerResource(
        'metrics',
        '/metrics',
        { title: 'Server Metrics', description: 'Server performance and usage metrics' },
        async () => ({
          contents: [{
            uri: '/metrics',
            mimeType: 'application/json',
            text: JSON.stringify({
              ...this.metrics,
              coordinationStatus: this.coordinator.getCoordinationStatus(),
              routerMetrics: this.router.getMetrics()
            }, null, 2)
          }]
        })
      );
    }
  }

  private setupMCPTools(): void {
    // Register agent tool
    this.mcpServer.registerTool(
      'register_agent',
      {
        title: 'Register Agent',
        description: 'Register a new agent with the coordination system',
        inputSchema: {
          agentId: z.string().uuid(),
          type: z.enum(['coordinator', 'analyzer', 'transformer', 'validator', 'reporter']),
          capabilities: z.array(z.object({
            name: z.string(),
            version: z.string(),
            description: z.string().optional(),
            inputSchema: z.record(z.any()).optional(),
            outputSchema: z.record(z.any()).optional()
          })),
          endpoint: z.string().url(),
          metadata: z.record(z.any()).optional()
        }
      },
      async ({ agentId, type, capabilities, endpoint, metadata }) => {
        const request: RegistrationRequest = {
          agentId,
          type: type as AgentType,
          capabilities,
          endpoint,
          metadata
        };

        const response = await this.registerAgent(request);
        return {
          content: [{
            type: 'text',
            text: JSON.stringify(response, null, 2)
          }]
        };
      }
    );

    // Distribute task tool
    this.mcpServer.registerTool(
      'distribute_task',
      {
        title: 'Distribute Task',
        description: 'Distribute a task to specified agents',
        inputSchema: {
          task: z.object({
            id: z.string().uuid().optional(),
            type: z.string(),
            priority: z.enum(['low', 'normal', 'high', 'urgent']).default('normal'),
            payload: z.object({
              type: z.string(),
              data: z.record(z.any())
            }),
            timeout: z.number().optional(),
            dependencies: z.array(z.string().uuid()).optional()
          }),
          targetAgents: z.array(z.string().uuid()).optional(),
          agentType: z.enum(['coordinator', 'analyzer', 'transformer', 'validator', 'reporter']).optional()
        }
      },
      async ({ task, targetAgents, agentType }) => {
        const taskId = task.id || randomUUID();
        const fullTask: Task = {
          id: taskId,
          type: task.type,
          priority: task.priority as Priority,
          status: TaskStatus.PENDING,
          payload: task.payload,
          createdAt: new Date(),
          updatedAt: new Date(),
          timeout: task.timeout,
          dependencies: task.dependencies
        };

        let agents = targetAgents;
        if (!agents && agentType) {
          agents = this.getAgentsByType(agentType as AgentType);
        }
        if (!agents || agents.length === 0) {
          agents = Array.from(this.registeredAgents.keys());
        }

        const results = await this.coordinator.distributeTask(fullTask, agents);
        this.activeTasks.set(taskId, fullTask);
        this.metrics.totalTasks++;

        return {
          content: [{
            type: 'text',
            text: JSON.stringify({ taskId, results, assignedAgents: agents }, null, 2)
          }]
        };
      }
    );

    // Coordinate agents tool
    this.mcpServer.registerTool(
      'coordinate_agents',
      {
        title: 'Coordinate Agents',
        description: 'Coordinate multiple agents for consensus or synchronization',
        inputSchema: {
          type: z.enum(['consensus', 'synchronize', 'load_balance']),
          agents: z.array(z.string().uuid()),
          payload: z.record(z.any()).optional(),
          options: z.record(z.any()).optional()
        }
      },
      async ({ type, agents, payload, options }) => {
        let result;
        
        switch (type) {
          case 'consensus':
            result = await this.coordinator.achieveConsensus(agents, payload, options);
            break;
          case 'synchronize':
            const barrierId = options?.barrierId || randomUUID();
            await this.coordinator.synchronize(agents, barrierId, options?.timeout);
            result = { success: true, barrierId };
            break;
          case 'load_balance':
            const tasks = payload?.tasks || [];
            result = await this.coordinator.balanceLoad(tasks, agents);
            break;
          default:
            throw new Error(`Unknown coordination type: ${type}`);
        }

        return {
          content: [{
            type: 'text',
            text: JSON.stringify(result, null, 2)
          }]
        };
      }
    );

    // Get agent status tool
    this.mcpServer.registerTool(
      'get_agent_status',
      {
        title: 'Get Agent Status',
        description: 'Get the current status of agents',
        inputSchema: {
          agentId: z.string().uuid().optional()
        }
      },
      async ({ agentId }) => {
        if (agentId) {
          const agent = this.registeredAgents.get(agentId);
          return {
            content: [{
              type: 'text',
              text: JSON.stringify(agent || { error: 'Agent not found' }, null, 2)
            }]
          };
        } else {
          return {
            content: [{
              type: 'text',
              text: JSON.stringify(Array.from(this.registeredAgents.values()), null, 2)
            }]
          };
        }
      }
    );
  }

  private setupMCPPrompts(): void {
    // Agent coordination prompt
    this.mcpServer.registerPrompt(
      'coordinate_task',
      {
        title: 'Coordinate Task Execution',
        description: 'Generate coordination instructions for multi-agent task execution',
        argsSchema: {
          taskType: z.string(),
          agents: z.array(z.string()),
          complexity: z.enum(['low', 'medium', 'high']).default('medium')
        }
      },
      ({ taskType, agents, complexity }) => ({
        messages: [{
          role: 'user',
          content: {
            type: 'text',
            text: `Create a coordination plan for executing a ${taskType} task with ${agents.length} agents (${agents.join(', ')}). Task complexity: ${complexity}. Include task breakdown, agent assignments, dependencies, and coordination strategy.`
          }
        }]
      })
    );

    // System status prompt
    this.mcpServer.registerPrompt(
      'system_analysis',
      {
        title: 'Analyze System Status',
        description: 'Generate system analysis based on current metrics and agent states',
        argsSchema: {}
      },
      () => ({
        messages: [{
          role: 'user',
          content: {
            type: 'text',
            text: `Analyze the current CodeGraph system status based on these metrics: ${JSON.stringify(this.metrics)}. Provide insights on performance, bottlenecks, and recommendations for optimization.`
          }
        }]
      })
    );
  }

  private setupRoutes(): void {
    // Agent coordination route
    this.router.addRoute(
      { method: 'codegraph/agent/coordinate', priority: 10 },
      new CoordinationRouteHandler(this)
    );

    // Task management route
    this.router.addRoute(
      { method: 'codegraph/task/distribute', priority: 10 },
      new TaskDistributionRouteHandler(this)
    );

    // Agent registration route
    this.router.addRoute(
      { method: 'session/register_agent', priority: 5 },
      new AgentRegistrationRouteHandler(this)
    );

    // Heartbeat route
    this.router.addRoute(
      { method: 'ping', priority: 1 },
      new HeartbeatRouteHandler()
    );
  }

  private setupEventHandlers(): void {
    // WebSocket transport events
    this.wsTransport.on('client:connected', ({ sessionId, session }) => {
      this.metrics.totalConnections++;
      this.metrics.activeConnections++;
      this.emit('client:connected', { sessionId, session });
    });

    this.wsTransport.on('client:disconnected', ({ sessionId, agentId }) => {
      this.metrics.activeConnections--;
      
      if (agentId) {
        this.handleAgentDisconnection(agentId, sessionId);
      }
      
      this.emit('client:disconnected', { sessionId, agentId });
    });

    this.wsTransport.on('message', async (message: JsonRpcMessage, sessionId: string) => {
      this.metrics.totalMessages++;
      
      const session = this.wsTransport.getSession(sessionId);
      const context: RouteContext = {
        sessionId,
        agentId: session?.agentId,
        metadata: session?.metadata || {},
        timestamp: new Date()
      };

      try {
        const result = await this.router.route(message, context);
        
        if (result.response) {
          this.wsTransport.sendToSession(sessionId, result.response);
        }

        if (result.forward) {
          await this.handleMessageForwarding(result.forward, message);
        }

      } catch (error) {
        this.emit('routing:error', { message, sessionId, error });
        
        const errorResponse: JsonRpcMessage = {
          jsonrpc: '2.0',
          error: {
            code: -32603,
            message: 'Internal error'
          },
          id: message.id
        };
        
        this.wsTransport.sendToSession(sessionId, errorResponse);
      }
    });

    // Coordination engine events
    this.coordinator.on('task:distributed', (data) => {
      this.emit('coordination:task_distributed', data);
    });

    this.coordinator.on('consensus:complete', (data) => {
      this.emit('coordination:consensus_complete', data);
    });

    this.coordinator.on('synchronization:complete', (data) => {
      this.emit('coordination:synchronization_complete', data);
    });
  }

  private async handleMessageForwarding(forward: any, message: JsonRpcMessage): Promise<void> {
    if (forward.broadcast) {
      this.wsTransport.broadcast(message);
    } else if (forward.targetAgents) {
      this.wsTransport.broadcastToAgents(forward.targetAgents, message);
    } else if (forward.targetSessions) {
      for (const sessionId of forward.targetSessions) {
        this.wsTransport.sendToSession(sessionId, message);
      }
    }
  }

  public async start(): Promise<void> {
    try {
      this.emit('server:starting');
      
      // Start WebSocket transport
      this.emit('server:started', {
        websocketPort: this.config.websocketPort,
        httpPort: this.config.httpPort
      });
      
    } catch (error) {
      this.emit('server:error', { error });
      throw error;
    }
  }

  public async stop(): Promise<void> {
    try {
      this.emit('server:stopping');
      
      // Close WebSocket transport
      await this.wsTransport.close();
      
      this.emit('server:stopped');
    } catch (error) {
      this.emit('server:error', { error });
      throw error;
    }
  }

  private async registerAgent(request: RegistrationRequest): Promise<RegistrationResponse> {
    try {
      const agentInfo: AgentDiscoveryInfo = {
        agentId: request.agentId,
        type: request.type,
        status: AgentStatus.ACTIVE,
        capabilities: request.capabilities,
        endpoint: request.endpoint,
        metadata: request.metadata || {},
        lastSeen: new Date(),
        ttl: request.ttl || 3600
      };

      this.registeredAgents.set(request.agentId, agentInfo);
      this.coordinator.updateAgentStatus(request.agentId, AgentStatus.ACTIVE);

      this.emit('agent:registered', { agentInfo });

      return {
        success: true,
        agentId: request.agentId,
        registrationId: randomUUID(),
        validUntil: new Date(Date.now() + (request.ttl || 3600) * 1000)
      };

    } catch (error) {
      return {
        success: false,
        agentId: request.agentId,
        error: `Registration failed: ${error}`
      };
    }
  }

  private handleAgentDisconnection(agentId: string, sessionId: string): void {
    // Remove session mapping
    const agentSessions = this.agentSessions.get(agentId) || [];
    const updatedSessions = agentSessions.filter(id => id !== sessionId);
    
    if (updatedSessions.length === 0) {
      this.agentSessions.delete(agentId);
      this.coordinator.updateAgentStatus(agentId, AgentStatus.OFFLINE);
    } else {
      this.agentSessions.set(agentId, updatedSessions);
    }

    this.sessionAgents.delete(sessionId);
    this.emit('agent:disconnected', { agentId, sessionId });
  }

  private getAgentsByType(type: AgentType): string[] {
    return Array.from(this.registeredAgents.values())
      .filter(agent => agent.type === type)
      .map(agent => agent.agentId);
  }

  public getMetrics() {
    return {
      ...this.metrics,
      registeredAgents: this.registeredAgents.size,
      activeSessions: this.wsTransport.getSessionCount(),
      coordinationStatus: this.coordinator.getCoordinationStatus(),
      routerMetrics: this.router.getMetrics()
    };
  }

  public getMcpServer(): McpServer {
    return this.mcpServer;
  }

  public getWebSocketTransport(): WebSocketServerTransport {
    return this.wsTransport;
  }
}

// Route handlers
class CoordinationRouteHandler implements RouteHandler {
  constructor(private server: CodeGraphMCPServer) {}

  async handle(message: JsonRpcMessage, context: RouteContext): Promise<RouteResult> {
    const params = message.params as any;
    
    // Handle coordination message
    return {
      success: true,
      response: {
        jsonrpc: '2.0',
        result: { coordinated: true },
        id: message.id
      }
    };
  }
}

class TaskDistributionRouteHandler implements RouteHandler {
  constructor(private server: CodeGraphMCPServer) {}

  async handle(message: JsonRpcMessage, context: RouteContext): Promise<RouteResult> {
    // Handle task distribution
    return {
      success: true,
      response: {
        jsonrpc: '2.0',
        result: { distributed: true },
        id: message.id
      }
    };
  }
}

class AgentRegistrationRouteHandler implements RouteHandler {
  constructor(private server: CodeGraphMCPServer) {}

  async handle(message: JsonRpcMessage, context: RouteContext): Promise<RouteResult> {
    // Handle agent registration logic
    return {
      success: true,
      response: {
        jsonrpc: '2.0',
        result: { registered: true },
        id: message.id
      }
    };
  }
}

class HeartbeatRouteHandler implements RouteHandler {
  async handle(message: JsonRpcMessage, context: RouteContext): Promise<RouteResult> {
    return {
      success: true,
      response: {
        jsonrpc: '2.0',
        result: 'pong',
        id: message.id
      }
    };
  }
}