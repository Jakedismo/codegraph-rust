import { EventEmitter } from 'events';
import { randomUUID } from 'crypto';
import {
  Task,
  TaskResult,
  TaskStatus,
  AgentMessage,
  MessageType,
  Priority,
  CoordinationRequest,
  CoordinationResponse,
  CommunicationPattern,
  CommunicationTopology,
  AgentStatus,
  CoordinationError
} from '../agents/agent-types.js';

export interface CoordinationStrategy {
  name: string;
  execute(request: CoordinationRequest, participants: string[]): Promise<CoordinationResponse>;
}

export interface ConsensusOptions {
  threshold?: number; // Percentage of participants that must agree (0-1)
  timeout?: number;
  retries?: number;
  strategy?: 'majority' | 'unanimous' | 'quorum';
}

export interface LoadBalanceOptions {
  strategy?: 'round_robin' | 'least_loaded' | 'random' | 'weighted';
  weights?: Map<string, number>;
  healthCheck?: boolean;
}

export interface SynchronizationBarrier {
  id: string;
  participants: string[];
  arrivals: Set<string>;
  timeout: number;
  createdAt: Date;
}

export interface TaskDistributionResult {
  assignments: Map<string, Task[]>;
  unassigned: Task[];
  totalAssigned: number;
}

export class CoordinationEngine extends EventEmitter {
  private activeCoordinations = new Map<string, CoordinationRequest>();
  private barriers = new Map<string, SynchronizationBarrier>();
  private agentLoads = new Map<string, number>();
  private agentStatuses = new Map<string, AgentStatus>();
  private strategies = new Map<string, CoordinationStrategy>();

  constructor() {
    super();
    this.initializeBuiltinStrategies();
  }

  // Task Distribution
  public async distributeTask(task: Task, agents: string[]): Promise<TaskResult[]> {
    if (agents.length === 0) {
      throw new Error('No agents available for task distribution');
    }

    const coordinationId = randomUUID();
    const request: CoordinationRequest = {
      type: 'distribute',
      participants: agents,
      payload: { task },
      timeout: task.timeout || 30000
    };

    this.activeCoordinations.set(coordinationId, request);

    try {
      const results: TaskResult[] = [];
      
      // Distribute task to all agents (broadcast pattern)
      for (const agentId of agents) {
        try {
          const agentTask = { ...task, assignedTo: agentId };
          const message: AgentMessage = {
            type: MessageType.TASK_ASSIGNMENT,
            from: 'coordinator',
            to: agentId,
            sessionId: coordinationId,
            priority: task.priority,
            payload: { task: agentTask },
            timestamp: new Date()
          };

          this.emit('task:distribute', { agentId, task: agentTask, message });
          
          // Wait for result (this would be handled by message responses in practice)
          const result = await this.waitForTaskResult(coordinationId, agentId, request.timeout);
          if (result) {
            results.push(result);
          }
        } catch (error) {
          this.emit('task:distribution_error', { agentId, task, error });
        }
      }

      return results;
    } finally {
      this.activeCoordinations.delete(coordinationId);
    }
  }

  public async distributeTasks(tasks: Task[], agents: string[], options: LoadBalanceOptions = {}): Promise<TaskDistributionResult> {
    const result: TaskDistributionResult = {
      assignments: new Map(),
      unassigned: [...tasks],
      totalAssigned: 0
    };

    if (agents.length === 0 || tasks.length === 0) {
      return result;
    }

    // Initialize agent assignments
    for (const agentId of agents) {
      result.assignments.set(agentId, []);
    }

    // Filter available agents based on health check
    const availableAgents = options.healthCheck 
      ? agents.filter(agent => this.isAgentHealthy(agent))
      : agents;

    if (availableAgents.length === 0) {
      return result;
    }

    // Distribute tasks based on strategy
    const strategy = options.strategy || 'round_robin';
    let currentAgentIndex = 0;

    for (let i = 0; i < tasks.length; i++) {
      const task = tasks[i];
      let selectedAgent: string;

      switch (strategy) {
        case 'round_robin':
          selectedAgent = availableAgents[currentAgentIndex % availableAgents.length];
          currentAgentIndex++;
          break;

        case 'least_loaded':
          selectedAgent = this.getLeastLoadedAgent(availableAgents);
          break;

        case 'random':
          selectedAgent = availableAgents[Math.floor(Math.random() * availableAgents.length)];
          break;

        case 'weighted':
          selectedAgent = this.getWeightedRandomAgent(availableAgents, options.weights);
          break;

        default:
          selectedAgent = availableAgents[0];
      }

      // Assign task to selected agent
      const agentTasks = result.assignments.get(selectedAgent)!;
      agentTasks.push({ ...task, assignedTo: selectedAgent });
      result.totalAssigned++;
      result.unassigned.splice(result.unassigned.indexOf(task), 1);

      // Update agent load
      this.updateAgentLoad(selectedAgent, 1);
    }

    this.emit('tasks:distributed', { result, strategy, agents: availableAgents });
    return result;
  }

  // Synchronization
  public async synchronize(agents: string[], barrierId: string, timeout = 30000): Promise<void> {
    if (this.barriers.has(barrierId)) {
      throw new Error(`Synchronization barrier ${barrierId} already exists`);
    }

    const barrier: SynchronizationBarrier = {
      id: barrierId,
      participants: [...agents],
      arrivals: new Set(),
      timeout,
      createdAt: new Date()
    };

    this.barriers.set(barrierId, barrier);

    // Set timeout for the barrier
    const timeoutHandle = setTimeout(() => {
      this.barriers.delete(barrierId);
      this.emit('synchronization:timeout', { barrierId, participants: agents, arrivals: Array.from(barrier.arrivals) });
    }, timeout);

    try {
      // Wait for all agents to arrive
      await this.waitForSynchronization(barrierId);
      clearTimeout(timeoutHandle);
      
      this.emit('synchronization:complete', { barrierId, participants: agents });
    } catch (error) {
      clearTimeout(timeoutHandle);
      this.barriers.delete(barrierId);
      throw error;
    }
  }

  public agentArriveAtBarrier(agentId: string, barrierId: string): boolean {
    const barrier = this.barriers.get(barrierId);
    if (!barrier) {
      return false;
    }

    if (!barrier.participants.includes(agentId)) {
      return false;
    }

    barrier.arrivals.add(agentId);
    this.emit('synchronization:arrival', { agentId, barrierId, totalArrivals: barrier.arrivals.size, totalParticipants: barrier.participants.length });

    // Check if all agents have arrived
    if (barrier.arrivals.size === barrier.participants.length) {
      this.barriers.delete(barrierId);
      this.emit('synchronization:ready', { barrierId });
    }

    return true;
  }

  // Consensus
  public async achieveConsensus(agents: string[], proposal: any, options: ConsensusOptions = {}): Promise<CoordinationResponse> {
    const coordinationId = randomUUID();
    const threshold = options.threshold || 0.5;
    const timeout = options.timeout || 30000;
    const strategy = options.strategy || 'majority';

    const request: CoordinationRequest = {
      type: 'consensus',
      participants: agents,
      payload: { proposal, threshold, strategy },
      timeout
    };

    this.activeCoordinations.set(coordinationId, request);

    try {
      const responses = new Map<string, boolean>();
      
      // Send consensus request to all agents
      for (const agentId of agents) {
        this.emit('consensus:request', { agentId, proposal, coordinationId });
      }

      // Wait for responses
      await this.waitForConsensusResponses(coordinationId, agents, timeout);

      // Collect responses (this would be populated by actual agent responses)
      const votes = Array.from(responses.values());
      const positiveVotes = votes.filter(vote => vote).length;
      const totalVotes = votes.length;

      let consensusReached = false;
      
      switch (strategy) {
        case 'majority':
          consensusReached = positiveVotes > totalVotes / 2;
          break;
        case 'unanimous':
          consensusReached = positiveVotes === totalVotes;
          break;
        case 'quorum':
          consensusReached = (positiveVotes / totalVotes) >= threshold;
          break;
      }

      const response: CoordinationResponse = {
        success: consensusReached,
        data: {
          proposal,
          positiveVotes,
          totalVotes,
          threshold,
          strategy
        },
        participantResponses: responses
      };

      this.emit('consensus:complete', { coordinationId, response });
      return response;

    } catch (error) {
      throw new CoordinationError(
        `Consensus failed: ${error}`,
        coordinationId,
        'CONSENSUS_FAILED',
        agents
      );
    } finally {
      this.activeCoordinations.delete(coordinationId);
    }
  }

  // Load Balancing
  public balanceLoad(tasks: Task[], agents: string[]): Promise<TaskDistributionResult> {
    return this.distributeTasks(tasks, agents, { strategy: 'least_loaded', healthCheck: true });
  }

  // Communication Pattern Management
  public async setupCommunicationTopology(topology: CommunicationTopology): Promise<void> {
    const { pattern, participants } = topology;

    switch (pattern) {
      case CommunicationPattern.POINT_TO_POINT:
        await this.setupPointToPoint(participants);
        break;
      case CommunicationPattern.PUBLISH_SUBSCRIBE:
        await this.setupPubSub(participants);
        break;
      case CommunicationPattern.BROADCAST:
        await this.setupBroadcast(participants);
        break;
      case CommunicationPattern.PIPELINE:
        await this.setupPipeline(participants);
        break;
      case CommunicationPattern.SCATTER_GATHER:
        await this.setupScatterGather(participants);
        break;
      default:
        throw new Error(`Unsupported communication pattern: ${pattern}`);
    }

    this.emit('topology:setup', { pattern, participants });
  }

  // Strategy Management
  public registerStrategy(strategy: CoordinationStrategy): void {
    this.strategies.set(strategy.name, strategy);
    this.emit('strategy:registered', { name: strategy.name });
  }

  public executeCustomStrategy(strategyName: string, request: CoordinationRequest): Promise<CoordinationResponse> {
    const strategy = this.strategies.get(strategyName);
    if (!strategy) {
      throw new Error(`Strategy not found: ${strategyName}`);
    }

    return strategy.execute(request, request.participants);
  }

  // Agent Management
  public updateAgentStatus(agentId: string, status: AgentStatus): void {
    this.agentStatuses.set(agentId, status);
    this.emit('agent:status_updated', { agentId, status });
  }

  public updateAgentLoad(agentId: string, loadDelta: number): void {
    const currentLoad = this.agentLoads.get(agentId) || 0;
    const newLoad = Math.max(0, currentLoad + loadDelta);
    this.agentLoads.set(agentId, newLoad);
    this.emit('agent:load_updated', { agentId, load: newLoad });
  }

  public getAgentLoad(agentId: string): number {
    return this.agentLoads.get(agentId) || 0;
  }

  public getAgentStatus(agentId: string): AgentStatus {
    return this.agentStatuses.get(agentId) || AgentStatus.OFFLINE;
  }

  public isAgentHealthy(agentId: string): boolean {
    const status = this.getAgentStatus(agentId);
    return status === AgentStatus.ACTIVE || status === AgentStatus.IDLE;
  }

  // Private helper methods
  private getLeastLoadedAgent(agents: string[]): string {
    let leastLoaded = agents[0];
    let minLoad = this.getAgentLoad(leastLoaded);

    for (const agentId of agents) {
      const load = this.getAgentLoad(agentId);
      if (load < minLoad) {
        minLoad = load;
        leastLoaded = agentId;
      }
    }

    return leastLoaded;
  }

  private getWeightedRandomAgent(agents: string[], weights?: Map<string, number>): string {
    if (!weights || weights.size === 0) {
      return agents[Math.floor(Math.random() * agents.length)];
    }

    const totalWeight = agents.reduce((sum, agent) => sum + (weights.get(agent) || 1), 0);
    let random = Math.random() * totalWeight;

    for (const agent of agents) {
      const weight = weights.get(agent) || 1;
      random -= weight;
      if (random <= 0) {
        return agent;
      }
    }

    return agents[0]; // Fallback
  }

  private async waitForTaskResult(coordinationId: string, agentId: string, timeout?: number): Promise<TaskResult | null> {
    return new Promise((resolve) => {
      const timeoutHandle = timeout ? setTimeout(() => resolve(null), timeout) : null;
      
      const handler = (data: any) => {
        if (data.coordinationId === coordinationId && data.agentId === agentId) {
          if (timeoutHandle) clearTimeout(timeoutHandle);
          this.removeListener('task:result', handler);
          resolve(data.result);
        }
      };
      
      this.on('task:result', handler);
    });
  }

  private async waitForSynchronization(barrierId: string): Promise<void> {
    return new Promise((resolve, reject) => {
      const handler = (data: any) => {
        if (data.barrierId === barrierId) {
          this.removeListener('synchronization:ready', handler);
          this.removeListener('synchronization:timeout', timeoutHandler);
          resolve();
        }
      };

      const timeoutHandler = (data: any) => {
        if (data.barrierId === barrierId) {
          this.removeListener('synchronization:ready', handler);
          this.removeListener('synchronization:timeout', timeoutHandler);
          reject(new Error(`Synchronization timeout for barrier ${barrierId}`));
        }
      };

      this.on('synchronization:ready', handler);
      this.on('synchronization:timeout', timeoutHandler);
    });
  }

  private async waitForConsensusResponses(coordinationId: string, agents: string[], timeout: number): Promise<void> {
    return new Promise((resolve) => {
      setTimeout(resolve, timeout); // Simplified - in practice, wait for actual responses
    });
  }

  // Communication topology setup methods
  private async setupPointToPoint(participants: string[]): Promise<void> {
    if (participants.length !== 2) {
      throw new Error('Point-to-point communication requires exactly 2 participants');
    }
    // Setup logic would go here
  }

  private async setupPubSub(participants: string[]): Promise<void> {
    // Setup pub/sub topics and subscriptions
  }

  private async setupBroadcast(participants: string[]): Promise<void> {
    // Setup broadcast channels
  }

  private async setupPipeline(participants: string[]): Promise<void> {
    // Setup sequential processing pipeline
  }

  private async setupScatterGather(participants: string[]): Promise<void> {
    // Setup scatter-gather pattern
  }

  private initializeBuiltinStrategies(): void {
    // Register built-in coordination strategies
    this.registerStrategy(new ConsensusStrategy());
    this.registerStrategy(new LoadBalancingStrategy());
  }

  // Get coordination status
  public getCoordinationStatus() {
    return {
      activeCoordinations: this.activeCoordinations.size,
      activeSynchronizationBarriers: this.barriers.size,
      agentLoads: Object.fromEntries(this.agentLoads),
      agentStatuses: Object.fromEntries(this.agentStatuses),
      registeredStrategies: Array.from(this.strategies.keys())
    };
  }
}

// Built-in coordination strategies
class ConsensusStrategy implements CoordinationStrategy {
  name = 'consensus';

  async execute(request: CoordinationRequest, participants: string[]): Promise<CoordinationResponse> {
    // Simplified consensus implementation
    return {
      success: true,
      data: { consensus: true },
      participantResponses: new Map()
    };
  }
}

class LoadBalancingStrategy implements CoordinationStrategy {
  name = 'load_balancing';

  async execute(request: CoordinationRequest, participants: string[]): Promise<CoordinationResponse> {
    // Simplified load balancing implementation
    return {
      success: true,
      data: { balanced: true },
      participantResponses: new Map()
    };
  }
}