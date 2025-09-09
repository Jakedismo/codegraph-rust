import { EventEmitter } from 'events';
import { randomUUID } from 'crypto';
import {
  AgentType,
  AgentStatus,
  AgentConfig,
  AgentCapability,
  AgentMetadata,
  Task,
  TaskResult,
  AgentMessage,
  MessageType,
  Priority,
  TaskStatus,
  AgentError,
  TaskError,
  PerformanceMetrics,
  createTaskUpdate,
  createTaskResult,
  createHeartbeat
} from './agent-types.js';
import { WebSocketClientTransport } from '../transport/websocket-client-transport.js';
import { JsonRpcMessage } from '@modelcontextprotocol/sdk/types.js';

export interface AgentEventMap {
  'status:changed': { oldStatus: AgentStatus; newStatus: AgentStatus; reason?: string };
  'task:assigned': { task: Task };
  'task:completed': { task: Task; result: TaskResult };
  'task:failed': { task: Task; error: string };
  'message:received': { message: AgentMessage };
  'message:sent': { message: AgentMessage };
  'error': { error: Error };
  'capability:registered': { capability: AgentCapability };
  'capability:removed': { capability: AgentCapability };
  'heartbeat:sent': { timestamp: Date };
  'connected': void;
  'disconnected': void;
}

export abstract class BaseAgent extends EventEmitter {
  protected config: AgentConfig;
  protected transport: WebSocketClientTransport | null = null;
  protected status: AgentStatus = AgentStatus.INITIALIZING;
  protected activeTasks = new Map<string, Task>();
  protected capabilities = new Map<string, AgentCapability>();
  protected metrics: PerformanceMetrics = {
    averageLatency: 0,
    throughput: 0,
    errorRate: 0,
    lastUpdated: new Date()
  };
  protected heartbeatInterval?: NodeJS.Timer;
  protected taskTimeout = new Map<string, NodeJS.Timeout>();

  constructor(config: AgentConfig) {
    super();
    this.config = config;
    this.initializeCapabilities();
  }

  // Abstract methods that concrete agents must implement
  protected abstract handleTask(task: Task): Promise<TaskResult>;
  protected abstract onInitialize(): Promise<void>;
  protected abstract onStart(): Promise<void>;
  protected abstract onStop(): Promise<void>;

  public async initialize(): Promise<void> {
    try {
      this.setStatus(AgentStatus.INITIALIZING);
      await this.onInitialize();
      this.emit('initialized');
    } catch (error) {
      this.setStatus(AgentStatus.ERROR);
      throw new AgentError(
        `Failed to initialize agent: ${error}`,
        'INIT_FAILED',
        this.config.agentId,
        { error }
      );
    }
  }

  public async start(): Promise<void> {
    try {
      if (this.status !== AgentStatus.INITIALIZING) {
        throw new AgentError(
          'Agent must be initialized before starting',
          'INVALID_STATE',
          this.config.agentId
        );
      }

      await this.onStart();
      this.setStatus(AgentStatus.IDLE);
      this.startHeartbeat();
      this.emit('started');
    } catch (error) {
      this.setStatus(AgentStatus.ERROR);
      throw new AgentError(
        `Failed to start agent: ${error}`,
        'START_FAILED',
        this.config.agentId,
        { error }
      );
    }
  }

  public async stop(): Promise<void> {
    try {
      await this.onStop();
      this.stopHeartbeat();
      this.clearTaskTimeouts();
      
      if (this.transport) {
        await this.transport.close();
        this.transport = null;
      }

      this.setStatus(AgentStatus.OFFLINE);
      this.emit('stopped');
    } catch (error) {
      this.setStatus(AgentStatus.ERROR);
      throw new AgentError(
        `Failed to stop agent: ${error}`,
        'STOP_FAILED',
        this.config.agentId,
        { error }
      );
    }
  }

  public async destroy(): Promise<void> {
    await this.stop();
    this.setStatus(AgentStatus.TERMINATED);
    this.removeAllListeners();
  }

  public async connect(transportUrl: string): Promise<void> {
    try {
      this.transport = new WebSocketClientTransport({ url: transportUrl });
      
      this.transport.on('connected', () => {
        this.emit('connected');
      });

      this.transport.on('disconnected', () => {
        this.emit('disconnected');
      });

      this.transport.on('message', (message: JsonRpcMessage) => {
        this.handleIncomingMessage(message);
      });

      this.transport.on('error', (error: Error) => {
        this.emit('error', { error });
      });

      await this.transport.connect();
      
      // Register agent with server
      const registered = await this.transport.registerAgent(
        this.config.agentId,
        this.getRegistrationMetadata()
      );

      if (!registered) {
        throw new Error('Failed to register with server');
      }

    } catch (error) {
      throw new AgentError(
        `Failed to connect: ${error}`,
        'CONNECTION_FAILED',
        this.config.agentId,
        { error, transportUrl }
      );
    }
  }

  public async assignTask(task: Task): Promise<void> {
    if (!this.canAcceptTask(task)) {
      throw new TaskError(
        'Cannot accept task - agent at capacity or wrong type',
        task.id,
        'TASK_REJECTED'
      );
    }

    this.activeTasks.set(task.id, { ...task, status: TaskStatus.ASSIGNED, assignedTo: this.config.agentId });
    this.setStatus(AgentStatus.BUSY);
    
    // Set timeout for task if specified
    if (task.timeout) {
      const timeout = setTimeout(() => {
        this.handleTaskTimeout(task.id);
      }, task.timeout);
      this.taskTimeout.set(task.id, timeout);
    }

    this.emit('task:assigned', { task });

    // Send task update
    await this.sendTaskUpdate(task.id, TaskStatus.IN_PROGRESS);

    // Process task asynchronously
    this.processTask(task).catch(error => {
      this.handleTaskError(task.id, error);
    });
  }

  private async processTask(task: Task): Promise<void> {
    const startTime = Date.now();
    
    try {
      const result = await this.handleTask(task);
      const processingTime = Date.now() - startTime;

      result.completedAt = new Date();
      result.processingTime = processingTime;

      // Update task status
      const updatedTask = this.activeTasks.get(task.id);
      if (updatedTask) {
        updatedTask.status = TaskStatus.COMPLETED;
        updatedTask.result = result;
        updatedTask.updatedAt = new Date();
      }

      // Clear timeout
      this.clearTaskTimeout(task.id);

      // Send result
      await this.sendTaskResult(task.id, result);

      // Remove from active tasks
      this.activeTasks.delete(task.id);

      // Update status
      this.updateStatusAfterTask();
      this.updateMetrics(processingTime, true);

      this.emit('task:completed', { task, result });

    } catch (error) {
      await this.handleTaskError(task.id, error);
    }
  }

  private async handleTaskError(taskId: string, error: any): Promise<void> {
    const task = this.activeTasks.get(taskId);
    if (!task) return;

    const processingTime = Date.now() - task.createdAt.getTime();

    task.status = TaskStatus.FAILED;
    task.error = error.message || error.toString();
    task.updatedAt = new Date();

    this.clearTaskTimeout(taskId);
    this.activeTasks.delete(taskId);
    this.updateStatusAfterTask();
    this.updateMetrics(processingTime, false);

    await this.sendTaskUpdate(taskId, TaskStatus.FAILED);
    this.emit('task:failed', { task, error: task.error });
  }

  private async handleTaskTimeout(taskId: string): Promise<void> {
    const task = this.activeTasks.get(taskId);
    if (!task) return;

    task.status = TaskStatus.TIMEOUT;
    task.error = 'Task timed out';
    task.updatedAt = new Date();

    this.activeTasks.delete(taskId);
    this.updateStatusAfterTask();

    await this.sendTaskUpdate(taskId, TaskStatus.TIMEOUT);
    this.emit('task:failed', { task, error: 'Task timed out' });
  }

  private async handleIncomingMessage(message: JsonRpcMessage): Promise<void> {
    try {
      if (message.method === 'codegraph/agent/coordinate') {
        const params = message.params as any;
        const agentMessage: AgentMessage = {
          type: params.payload.type,
          from: params.agentId,
          to: this.config.agentId,
          sessionId: params.sessionId,
          priority: params.priority || Priority.NORMAL,
          payload: params.payload.data,
          timestamp: new Date()
        };

        this.emit('message:received', { message: agentMessage });
        await this.processAgentMessage(agentMessage);
      }
    } catch (error) {
      this.emit('error', { error });
    }
  }

  private async processAgentMessage(message: AgentMessage): Promise<void> {
    switch (message.type) {
      case MessageType.TASK_ASSIGNMENT:
        const task = message.payload.task as Task;
        await this.assignTask(task);
        break;
        
      case MessageType.HEARTBEAT:
        // Respond to heartbeat
        await this.sendHeartbeat();
        break;
        
      default:
        // Let concrete agents handle other message types
        await this.onMessageReceived(message);
    }
  }

  protected async onMessageReceived(message: AgentMessage): Promise<void> {
    // Override in concrete agents for custom message handling
  }

  private async sendMessage(message: AgentMessage): Promise<void> {
    if (!this.transport) {
      throw new Error('Transport not connected');
    }

    const mcpMessage: JsonRpcMessage = {
      jsonrpc: '2.0',
      method: 'codegraph/agent/coordinate',
      params: {
        agentId: this.config.agentId,
        sessionId: message.sessionId,
        priority: message.priority,
        payload: {
          type: message.type,
          data: message.payload
        }
      },
      id: randomUUID()
    };

    await this.transport.write(mcpMessage);
    this.emit('message:sent', { message });
  }

  private async sendTaskUpdate(taskId: string, status: TaskStatus): Promise<void> {
    const message = createTaskUpdate(taskId, status, undefined, 'default');
    await this.sendMessage(message);
  }

  private async sendTaskResult(taskId: string, result: TaskResult): Promise<void> {
    const message = createTaskResult(taskId, result, 'default');
    await this.sendMessage(message);
  }

  private async sendHeartbeat(): Promise<void> {
    const metadata = {
      status: this.status,
      activeTasks: this.activeTasks.size,
      capabilities: Array.from(this.capabilities.keys()),
      metrics: this.metrics
    };

    const message = createHeartbeat(this.config.agentId, 'default', metadata);
    await this.sendMessage(message);
    this.emit('heartbeat:sent', { timestamp: new Date() });
  }

  private startHeartbeat(): void {
    const interval = this.config.healthCheck?.interval || 30000;
    this.heartbeatInterval = setInterval(() => {
      this.sendHeartbeat().catch(error => {
        this.emit('error', { error });
      });
    }, interval);
  }

  private stopHeartbeat(): void {
    if (this.heartbeatInterval) {
      clearInterval(this.heartbeatInterval);
      this.heartbeatInterval = undefined;
    }
  }

  private clearTaskTimeout(taskId: string): void {
    const timeout = this.taskTimeout.get(taskId);
    if (timeout) {
      clearTimeout(timeout);
      this.taskTimeout.delete(taskId);
    }
  }

  private clearTaskTimeouts(): void {
    for (const timeout of this.taskTimeout.values()) {
      clearTimeout(timeout);
    }
    this.taskTimeout.clear();
  }

  private setStatus(newStatus: AgentStatus, reason?: string): void {
    const oldStatus = this.status;
    this.status = newStatus;
    this.emit('status:changed', { oldStatus, newStatus, reason });
  }

  private updateStatusAfterTask(): void {
    if (this.activeTasks.size === 0) {
      this.setStatus(AgentStatus.IDLE);
    }
  }

  private updateMetrics(processingTime: number, success: boolean): void {
    const now = Date.now();
    const timeDiff = now - (this.metrics.lastUpdated?.getTime() || now);
    
    // Update throughput (tasks per second)
    this.metrics.throughput = (this.metrics.throughput || 0) * 0.9 + (1000 / Math.max(timeDiff, 1)) * 0.1;
    
    // Update average latency
    this.metrics.averageLatency = (this.metrics.averageLatency || 0) * 0.9 + processingTime * 0.1;
    
    // Update error rate
    const errorContribution = success ? 0 : 1;
    this.metrics.errorRate = (this.metrics.errorRate || 0) * 0.9 + errorContribution * 0.1;
    
    this.metrics.lastUpdated = new Date();
  }

  private canAcceptTask(task: Task): boolean {
    const maxConcurrency = this.config.maxConcurrency || 10;
    return this.activeTasks.size < maxConcurrency && this.status !== AgentStatus.ERROR;
  }

  private initializeCapabilities(): void {
    for (const capability of this.config.capabilities) {
      this.capabilities.set(capability.name, capability);
    }
  }

  private getRegistrationMetadata(): Record<string, any> {
    return {
      type: this.config.type,
      capabilities: Array.from(this.capabilities.values()),
      metadata: this.config.metadata,
      maxConcurrency: this.config.maxConcurrency,
      status: this.status
    };
  }

  // Public getters
  public getId(): string {
    return this.config.agentId;
  }

  public getType(): AgentType {
    return this.config.type;
  }

  public getStatus(): AgentStatus {
    return this.status;
  }

  public getActiveTasks(): Task[] {
    return Array.from(this.activeTasks.values());
  }

  public getCapabilities(): AgentCapability[] {
    return Array.from(this.capabilities.values());
  }

  public getMetrics(): PerformanceMetrics {
    return { ...this.metrics };
  }

  public hasCapability(name: string): boolean {
    return this.capabilities.has(name);
  }

  // Capability management
  public registerCapability(capability: AgentCapability): void {
    this.capabilities.set(capability.name, capability);
    this.emit('capability:registered', { capability });
  }

  public removeCapability(name: string): AgentCapability | undefined {
    const capability = this.capabilities.get(name);
    if (capability) {
      this.capabilities.delete(name);
      this.emit('capability:removed', { capability });
      return capability;
    }
    return undefined;
  }

  public updateCapability(name: string, updates: Partial<AgentCapability>): boolean {
    const capability = this.capabilities.get(name);
    if (capability) {
      Object.assign(capability, updates);
      return true;
    }
    return false;
  }
}