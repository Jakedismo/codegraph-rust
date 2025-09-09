import { JsonRpcMessage } from '@modelcontextprotocol/sdk/types.js';

export enum AgentType {
  COORDINATOR = 'coordinator',
  ANALYZER = 'analyzer',
  TRANSFORMER = 'transformer',
  VALIDATOR = 'validator',
  REPORTER = 'reporter',
  ORCHESTRATOR = 'orchestrator',
  MONITOR = 'monitor'
}

export enum AgentStatus {
  INITIALIZING = 'initializing',
  ACTIVE = 'active',
  IDLE = 'idle',
  BUSY = 'busy',
  ERROR = 'error',
  OFFLINE = 'offline',
  TERMINATED = 'terminated'
}

export enum TaskStatus {
  PENDING = 'pending',
  ASSIGNED = 'assigned',
  IN_PROGRESS = 'in_progress',
  COMPLETED = 'completed',
  FAILED = 'failed',
  CANCELLED = 'cancelled',
  TIMEOUT = 'timeout'
}

export enum MessageType {
  TASK_ASSIGNMENT = 'task_assignment',
  TASK_UPDATE = 'task_update',
  TASK_RESULT = 'task_result',
  SYNC_REQUEST = 'sync_request',
  SYNC_RESPONSE = 'sync_response',
  HEARTBEAT = 'heartbeat',
  COORDINATION = 'coordination',
  DISCOVERY = 'discovery',
  REGISTRATION = 'registration',
  ERROR = 'error'
}

export enum Priority {
  LOW = 'low',
  NORMAL = 'normal',
  HIGH = 'high',
  URGENT = 'urgent'
}

export interface AgentCapability {
  name: string;
  version: string;
  description?: string;
  inputSchema?: Record<string, any>;
  outputSchema?: Record<string, any>;
  tags?: string[];
  dependencies?: string[];
  performance?: PerformanceMetrics;
}

export interface PerformanceMetrics {
  averageLatency?: number;
  throughput?: number;
  errorRate?: number;
  lastUpdated?: Date;
}

export interface AgentMetadata {
  hostname?: string;
  platform?: string;
  version?: string;
  startTime?: Date;
  tags?: string[];
  environment?: Record<string, any>;
  resources?: ResourceUsage;
}

export interface ResourceUsage {
  cpu?: number;
  memory?: number;
  disk?: number;
  network?: number;
}

export interface AgentConfig {
  agentId: string;
  type: AgentType;
  capabilities: AgentCapability[];
  metadata?: AgentMetadata;
  maxConcurrency?: number;
  timeout?: number;
  retryPolicy?: RetryPolicy;
  healthCheck?: HealthCheckConfig;
}

export interface RetryPolicy {
  maxAttempts: number;
  backoffStrategy: 'fixed' | 'exponential' | 'linear';
  baseDelay: number;
  maxDelay?: number;
  jitter?: boolean;
}

export interface HealthCheckConfig {
  interval: number;
  timeout: number;
  endpoint?: string;
  method?: string;
  expectedStatus?: number;
}

export interface Task {
  id: string;
  type: string;
  priority: Priority;
  status: TaskStatus;
  payload: TaskPayload;
  assignedTo?: string;
  createdAt: Date;
  updatedAt: Date;
  timeout?: number;
  dependencies?: string[];
  metadata?: Record<string, any>;
  result?: TaskResult;
  error?: string;
}

export interface TaskPayload {
  type: string;
  data: Record<string, any>;
  metadata?: Record<string, any>;
}

export interface TaskResult {
  success: boolean;
  data?: Record<string, any>;
  error?: string;
  metadata?: Record<string, any>;
  completedAt: Date;
  processingTime: number;
}

export interface AgentMessage {
  type: MessageType;
  from: string;
  to?: string;
  sessionId: string;
  correlationId?: string;
  priority: Priority;
  payload: Record<string, any>;
  timestamp: Date;
  timeout?: number;
}

export interface CoordinationRequest {
  type: 'consensus' | 'synchronize' | 'distribute' | 'collect';
  participants: string[];
  payload: Record<string, any>;
  timeout?: number;
  metadata?: Record<string, any>;
}

export interface CoordinationResponse {
  success: boolean;
  data?: Record<string, any>;
  error?: string;
  participantResponses?: Map<string, any>;
}

// Communication patterns
export enum CommunicationPattern {
  POINT_TO_POINT = 'point_to_point',
  PUBLISH_SUBSCRIBE = 'publish_subscribe',
  REQUEST_RESPONSE = 'request_response',
  BROADCAST = 'broadcast',
  MULTICAST = 'multicast',
  PIPELINE = 'pipeline',
  SCATTER_GATHER = 'scatter_gather'
}

export interface CommunicationTopology {
  pattern: CommunicationPattern;
  participants: string[];
  configuration?: Record<string, any>;
}

// Agent discovery and registration
export interface AgentDiscoveryInfo {
  agentId: string;
  type: AgentType;
  status: AgentStatus;
  capabilities: AgentCapability[];
  endpoint: string;
  metadata: AgentMetadata;
  lastSeen: Date;
  ttl: number;
}

export interface RegistrationRequest {
  agentId: string;
  type: AgentType;
  capabilities: AgentCapability[];
  endpoint: string;
  metadata?: AgentMetadata;
  ttl?: number;
}

export interface RegistrationResponse {
  success: boolean;
  agentId: string;
  registrationId?: string;
  error?: string;
  validUntil?: Date;
}

// Event types for agent communication
export interface AgentEvent {
  type: string;
  source: string;
  timestamp: Date;
  payload: Record<string, any>;
  correlationId?: string;
  tags?: string[];
}

export interface TaskAssignmentEvent extends AgentEvent {
  type: 'task:assigned';
  payload: {
    taskId: string;
    assignedTo: string;
    task: Task;
  };
}

export interface TaskCompletedEvent extends AgentEvent {
  type: 'task:completed';
  payload: {
    taskId: string;
    result: TaskResult;
  };
}

export interface AgentStatusChangedEvent extends AgentEvent {
  type: 'agent:status_changed';
  payload: {
    agentId: string;
    oldStatus: AgentStatus;
    newStatus: AgentStatus;
    reason?: string;
  };
}

export interface CoordinationEvent extends AgentEvent {
  type: 'coordination:request' | 'coordination:response';
  payload: {
    coordinationId: string;
    participants: string[];
    data: Record<string, any>;
  };
}

// Error types
export class AgentError extends Error {
  constructor(
    message: string,
    public code: string,
    public agentId?: string,
    public details?: Record<string, any>
  ) {
    super(message);
    this.name = 'AgentError';
  }
}

export class TaskError extends Error {
  constructor(
    message: string,
    public taskId: string,
    public code: string,
    public details?: Record<string, any>
  ) {
    super(message);
    this.name = 'TaskError';
  }
}

export class CoordinationError extends Error {
  constructor(
    message: string,
    public coordinationId: string,
    public code: string,
    public failedParticipants?: string[]
  ) {
    super(message);
    this.name = 'CoordinationError';
  }
}

// Utility types for type safety
export type AgentEventHandler<T extends AgentEvent = AgentEvent> = (event: T) => void | Promise<void>;

export type TaskHandler = (task: Task) => Promise<TaskResult>;

export type CapabilityHandler = (
  request: Record<string, any>,
  capability: AgentCapability
) => Promise<Record<string, any>>;

// Message factory functions
export function createTaskAssignment(
  taskId: string,
  targetAgent: string,
  task: Task,
  sessionId: string
): AgentMessage {
  return {
    type: MessageType.TASK_ASSIGNMENT,
    from: 'coordinator',
    to: targetAgent,
    sessionId,
    priority: task.priority,
    payload: {
      taskId,
      task
    },
    timestamp: new Date()
  };
}

export function createTaskUpdate(
  taskId: string,
  status: TaskStatus,
  progress?: number,
  sessionId?: string
): AgentMessage {
  return {
    type: MessageType.TASK_UPDATE,
    from: 'agent',
    sessionId: sessionId || 'unknown',
    priority: Priority.NORMAL,
    payload: {
      taskId,
      status,
      progress,
      timestamp: new Date()
    },
    timestamp: new Date()
  };
}

export function createTaskResult(
  taskId: string,
  result: TaskResult,
  sessionId: string
): AgentMessage {
  return {
    type: MessageType.TASK_RESULT,
    from: 'agent',
    sessionId,
    priority: Priority.NORMAL,
    payload: {
      taskId,
      result
    },
    timestamp: new Date()
  };
}

export function createHeartbeat(agentId: string, sessionId: string, metadata?: Record<string, any>): AgentMessage {
  return {
    type: MessageType.HEARTBEAT,
    from: agentId,
    sessionId,
    priority: Priority.LOW,
    payload: {
      timestamp: new Date(),
      metadata
    },
    timestamp: new Date()
  };
}