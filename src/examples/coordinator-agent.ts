import { BaseAgent } from '../core/agents/base-agent.js';
import {
  AgentType,
  AgentConfig,
  Task,
  TaskResult,
  AgentMessage,
  MessageType,
  Priority,
  TaskStatus,
  createTaskAssignment,
  CoordinationRequest,
  CoordinationResponse
} from '../core/agents/agent-types.js';
import { randomUUID } from 'crypto';

export interface WorkflowDefinition {
  id: string;
  name: string;
  description: string;
  steps: WorkflowStep[];
  parallelism?: 'sequential' | 'parallel' | 'hybrid';
  timeout?: number;
}

export interface WorkflowStep {
  id: string;
  name: string;
  agentType: AgentType;
  taskType: string;
  dependencies?: string[];
  parameters?: Record<string, any>;
  timeout?: number;
  retries?: number;
}

export interface WorkflowExecution {
  id: string;
  workflowId: string;
  status: 'pending' | 'running' | 'completed' | 'failed' | 'cancelled';
  steps: StepExecution[];
  startTime: Date;
  endTime?: Date;
  result?: any;
  error?: string;
}

export interface StepExecution {
  stepId: string;
  taskId?: string;
  assignedAgent?: string;
  status: TaskStatus;
  startTime?: Date;
  endTime?: Date;
  result?: any;
  error?: string;
  retryCount: number;
}

export class CoordinatorAgent extends BaseAgent {
  private workflows = new Map<string, WorkflowDefinition>();
  private activeExecutions = new Map<string, WorkflowExecution>();
  private availableAgents = new Map<AgentType, string[]>();
  private agentCapabilities = new Map<string, Set<string>>();

  constructor(agentId: string = 'coordinator-001') {
    const config: AgentConfig = {
      agentId,
      type: AgentType.COORDINATOR,
      capabilities: [
        {
          name: 'workflow_orchestration',
          version: '1.0.0',
          description: 'Orchestrate complex multi-agent workflows',
          inputSchema: {
            type: 'object',
            properties: {
              workflow: { type: 'object' },
              parameters: { type: 'object' }
            },
            required: ['workflow']
          },
          outputSchema: {
            type: 'object',
            properties: {
              executionId: { type: 'string' },
              status: { type: 'string' },
              result: { type: 'object' }
            }
          }
        },
        {
          name: 'task_coordination',
          version: '1.0.0',
          description: 'Coordinate task distribution and dependency management',
          inputSchema: {
            type: 'object',
            properties: {
              tasks: { type: 'array' },
              agents: { type: 'array' },
              strategy: { type: 'string' }
            }
          }
        },
        {
          name: 'agent_discovery',
          version: '1.0.0',
          description: 'Discover and manage available agents',
          inputSchema: {
            type: 'object',
            properties: {
              criteria: { type: 'object' }
            }
          }
        },
        {
          name: 'resource_allocation',
          version: '1.0.0',
          description: 'Allocate resources and manage agent workloads',
          inputSchema: {
            type: 'object',
            properties: {
              requirements: { type: 'object' },
              constraints: { type: 'object' }
            }
          }
        },
        {
          name: 'conflict_resolution',
          version: '1.0.0',
          description: 'Resolve conflicts between agents and tasks',
          inputSchema: {
            type: 'object',
            properties: {
              conflict: { type: 'object' },
              strategy: { type: 'string' }
            }
          }
        }
      ],
      maxConcurrency: 20,
      timeout: 60000,
      metadata: {
        hostname: 'coordinator-node-1',
        platform: process.platform,
        version: '1.0.0',
        startTime: new Date(),
        tags: ['coordination', 'orchestration', 'workflow']
      }
    };

    super(config);
    this.initializeDefaultWorkflows();
  }

  protected async onInitialize(): Promise<void> {
    console.log(`Initializing CoordinatorAgent ${this.getId()}`);
    await this.discoverAvailableAgents();
    await this.loadWorkflowTemplates();
  }

  protected async onStart(): Promise<void> {
    console.log(`Starting CoordinatorAgent ${this.getId()}`);
    this.startAgentMonitoring();
    this.emit('coordinator:ready', {
      agentId: this.getId(),
      availableAgents: Object.fromEntries(this.availableAgents),
      workflowCount: this.workflows.size
    });
  }

  protected async onStop(): Promise<void> {
    console.log(`Stopping CoordinatorAgent ${this.getId()}`);
    await this.cancelActiveExecutions();
    this.stopAgentMonitoring();
  }

  protected async handleTask(task: Task): Promise<TaskResult> {
    const startTime = Date.now();

    try {
      const taskData = task.payload.data;

      switch (task.type) {
        case 'execute_workflow':
          return await this.handleWorkflowExecution(taskData, task.id);
        case 'coordinate_tasks':
          return await this.handleTaskCoordination(taskData, task.id);
        case 'discover_agents':
          return await this.handleAgentDiscovery(taskData);
        case 'allocate_resources':
          return await this.handleResourceAllocation(taskData);
        case 'resolve_conflict':
          return await this.handleConflictResolution(taskData);
        default:
          throw new Error(`Unknown task type: ${task.type}`);
      }
    } catch (error) {
      const processingTime = Date.now() - startTime;
      return {
        success: false,
        error: error instanceof Error ? error.message : String(error),
        completedAt: new Date(),
        processingTime
      };
    }
  }

  private async handleWorkflowExecution(data: any, taskId: string): Promise<TaskResult> {
    const { workflowId, parameters = {} } = data;
    const workflow = this.workflows.get(workflowId);

    if (!workflow) {
      throw new Error(`Workflow not found: ${workflowId}`);
    }

    const executionId = randomUUID();
    const execution: WorkflowExecution = {
      id: executionId,
      workflowId,
      status: 'pending',
      steps: workflow.steps.map(step => ({
        stepId: step.id,
        status: TaskStatus.PENDING,
        retryCount: 0
      })),
      startTime: new Date()
    };

    this.activeExecutions.set(executionId, execution);

    try {
      const result = await this.executeWorkflow(execution, parameters);
      
      return {
        success: true,
        data: {
          executionId,
          workflowId,
          result,
          duration: Date.now() - execution.startTime.getTime()
        },
        completedAt: new Date(),
        processingTime: Date.now() - execution.startTime.getTime()
      };
    } catch (error) {
      execution.status = 'failed';
      execution.error = error instanceof Error ? error.message : String(error);
      execution.endTime = new Date();

      throw error;
    } finally {
      this.activeExecutions.delete(executionId);
    }
  }

  private async executeWorkflow(execution: WorkflowExecution, parameters: Record<string, any>): Promise<any> {
    const workflow = this.workflows.get(execution.workflowId)!;
    execution.status = 'running';

    const results = new Map<string, any>();

    if (workflow.parallelism === 'sequential') {
      await this.executeSequential(execution, workflow, parameters, results);
    } else if (workflow.parallelism === 'parallel') {
      await this.executeParallel(execution, workflow, parameters, results);
    } else {
      await this.executeHybrid(execution, workflow, parameters, results);
    }

    execution.status = 'completed';
    execution.endTime = new Date();
    execution.result = Object.fromEntries(results);

    return execution.result;
  }

  private async executeSequential(
    execution: WorkflowExecution,
    workflow: WorkflowDefinition,
    parameters: Record<string, any>,
    results: Map<string, any>
  ): Promise<void> {
    for (const step of workflow.steps) {
      await this.executeStep(execution, step, parameters, results);
    }
  }

  private async executeParallel(
    execution: WorkflowExecution,
    workflow: WorkflowDefinition,
    parameters: Record<string, any>,
    results: Map<string, any>
  ): Promise<void> {
    const promises = workflow.steps.map(step => 
      this.executeStep(execution, step, parameters, results)
    );
    
    await Promise.all(promises);
  }

  private async executeHybrid(
    execution: WorkflowExecution,
    workflow: WorkflowDefinition,
    parameters: Record<string, any>,
    results: Map<string, any>
  ): Promise<void> {
    // Build dependency graph
    const dependencyMap = new Map<string, string[]>();
    const inDegree = new Map<string, number>();

    for (const step of workflow.steps) {
      dependencyMap.set(step.id, step.dependencies || []);
      inDegree.set(step.id, (step.dependencies || []).length);
    }

    const queue: WorkflowStep[] = [];
    const running = new Set<string>();

    // Find steps with no dependencies
    for (const step of workflow.steps) {
      if (inDegree.get(step.id) === 0) {
        queue.push(step);
      }
    }

    while (queue.length > 0 || running.size > 0) {
      // Start all available steps
      const toStart = queue.splice(0);
      const promises: Promise<void>[] = [];

      for (const step of toStart) {
        running.add(step.id);
        const promise = this.executeStep(execution, step, parameters, results)
          .then(() => {
            running.delete(step.id);
            
            // Check if any new steps can be started
            for (const nextStep of workflow.steps) {
              if (!running.has(nextStep.id) && !results.has(nextStep.id)) {
                const deps = dependencyMap.get(nextStep.id) || [];
                if (deps.every(dep => results.has(dep))) {
                  queue.push(nextStep);
                }
              }
            }
          })
          .catch(error => {
            running.delete(step.id);
            throw error;
          });

        promises.push(promise);
      }

      if (promises.length > 0) {
        await Promise.race(promises);
      }
    }
  }

  private async executeStep(
    execution: WorkflowExecution,
    step: WorkflowStep,
    workflowParameters: Record<string, any>,
    results: Map<string, any>
  ): Promise<void> {
    const stepExecution = execution.steps.find(s => s.stepId === step.id)!;
    stepExecution.status = TaskStatus.IN_PROGRESS;
    stepExecution.startTime = new Date();

    try {
      // Select agent for this step
      const agent = await this.selectAgentForStep(step);
      stepExecution.assignedAgent = agent;

      // Prepare task
      const taskId = randomUUID();
      stepExecution.taskId = taskId;

      const task: Task = {
        id: taskId,
        type: step.taskType,
        priority: Priority.NORMAL,
        status: TaskStatus.PENDING,
        payload: {
          type: step.taskType,
          data: {
            ...step.parameters,
            ...workflowParameters,
            stepId: step.id,
            dependencies: step.dependencies?.map(dep => results.get(dep))
          }
        },
        createdAt: new Date(),
        updatedAt: new Date(),
        timeout: step.timeout
      };

      // Execute task with retries
      let lastError: Error | null = null;
      const maxRetries = step.retries || 1;

      for (let retry = 0; retry < maxRetries; retry++) {
        try {
          stepExecution.retryCount = retry;
          const result = await this.executeTaskOnAgent(task, agent);
          
          stepExecution.status = TaskStatus.COMPLETED;
          stepExecution.endTime = new Date();
          stepExecution.result = result;
          results.set(step.id, result);
          
          return;
        } catch (error) {
          lastError = error instanceof Error ? error : new Error(String(error));
          
          if (retry < maxRetries - 1) {
            // Wait before retry
            await new Promise(resolve => setTimeout(resolve, 1000 * (retry + 1)));
          }
        }
      }

      throw lastError;

    } catch (error) {
      stepExecution.status = TaskStatus.FAILED;
      stepExecution.endTime = new Date();
      stepExecution.error = error instanceof Error ? error.message : String(error);
      
      throw error;
    }
  }

  private async selectAgentForStep(step: WorkflowStep): Promise<string> {
    const candidates = this.availableAgents.get(step.agentType) || [];
    
    if (candidates.length === 0) {
      throw new Error(`No available agents of type ${step.agentType}`);
    }

    // Simple round-robin selection for now
    // In a real implementation, this would consider load, capabilities, etc.
    return candidates[Math.floor(Math.random() * candidates.length)];
  }

  private async executeTaskOnAgent(task: Task, agentId: string): Promise<any> {
    // Simulate task execution by sending message to agent
    const message = createTaskAssignment(task.id, agentId, task, 'default');
    
    // In a real implementation, this would send the message and wait for response
    return new Promise((resolve) => {
      setTimeout(() => {
        resolve({ success: true, data: `Task ${task.id} completed by ${agentId}` });
      }, Math.random() * 2000 + 500); // Simulate 0.5-2.5s execution time
    });
  }

  private async handleTaskCoordination(data: any, taskId: string): Promise<TaskResult> {
    const { tasks, strategy = 'load_balance' } = data;
    
    // Coordinate task distribution
    const assignments = new Map<string, Task[]>();
    
    for (const task of tasks) {
      const agent = await this.selectOptimalAgent(task, strategy);
      if (!assignments.has(agent)) {
        assignments.set(agent, []);
      }
      assignments.get(agent)!.push(task);
    }

    return {
      success: true,
      data: {
        assignments: Object.fromEntries(assignments),
        strategy,
        totalTasks: tasks.length
      },
      completedAt: new Date(),
      processingTime: 100
    };
  }

  private async selectOptimalAgent(task: Task, strategy: string): Promise<string> {
    // Simplified agent selection logic
    const allAgents = Array.from(this.availableAgents.values()).flat();
    return allAgents[Math.floor(Math.random() * allAgents.length)];
  }

  private async handleAgentDiscovery(data: any): Promise<TaskResult> {
    const { criteria = {} } = data;
    
    await this.discoverAvailableAgents();
    
    let discoveredAgents = Array.from(this.availableAgents.entries());
    
    // Apply criteria filters
    if (criteria.type) {
      discoveredAgents = discoveredAgents.filter(([type]) => type === criteria.type);
    }
    
    if (criteria.capabilities) {
      discoveredAgents = discoveredAgents.filter(([type, agents]) => 
        agents.some(agent => 
          criteria.capabilities.every((cap: string) => 
            this.agentCapabilities.get(agent)?.has(cap)
          )
        )
      );
    }

    return {
      success: true,
      data: {
        agents: Object.fromEntries(discoveredAgents),
        totalCount: discoveredAgents.reduce((sum, [, agents]) => sum + agents.length, 0)
      },
      completedAt: new Date(),
      processingTime: 50
    };
  }

  private async handleResourceAllocation(data: any): Promise<TaskResult> {
    const { requirements, constraints = {} } = data;
    
    // Simplified resource allocation
    const allocation = {
      cpu: Math.min(requirements.cpu || 1, constraints.maxCpu || 8),
      memory: Math.min(requirements.memory || 512, constraints.maxMemory || 4096),
      agents: Math.min(requirements.agents || 1, constraints.maxAgents || 10)
    };

    return {
      success: true,
      data: { allocation },
      completedAt: new Date(),
      processingTime: 25
    };
  }

  private async handleConflictResolution(data: any): Promise<TaskResult> {
    const { conflict, strategy = 'priority' } = data;
    
    // Simplified conflict resolution
    const resolution = {
      strategy,
      action: 'reschedule',
      affectedTasks: conflict.tasks || [],
      recommendation: 'Reschedule conflicting tasks with staggered execution times'
    };

    return {
      success: true,
      data: { resolution },
      completedAt: new Date(),
      processingTime: 75
    };
  }

  private async discoverAvailableAgents(): Promise<void> {
    // Simulate agent discovery
    this.availableAgents.set(AgentType.ANALYZER, ['analyzer-001', 'analyzer-002']);
    this.availableAgents.set(AgentType.TRANSFORMER, ['transformer-001']);
    this.availableAgents.set(AgentType.VALIDATOR, ['validator-001']);
    this.availableAgents.set(AgentType.REPORTER, ['reporter-001']);
    
    // Simulate capabilities
    this.agentCapabilities.set('analyzer-001', new Set(['syntax_analysis', 'quality_analysis']));
    this.agentCapabilities.set('analyzer-002', new Set(['security_analysis', 'performance_analysis']));
    this.agentCapabilities.set('transformer-001', new Set(['code_generation', 'refactoring']));
    this.agentCapabilities.set('validator-001', new Set(['test_execution', 'validation']));
    this.agentCapabilities.set('reporter-001', new Set(['report_generation', 'metrics']));
  }

  private async loadWorkflowTemplates(): Promise<void> {
    // Load default workflow templates
    // This would typically load from a database or configuration
  }

  private initializeDefaultWorkflows(): void {
    // Code analysis workflow
    const codeAnalysisWorkflow: WorkflowDefinition = {
      id: 'code-analysis-full',
      name: 'Complete Code Analysis',
      description: 'Perform comprehensive code analysis including syntax, quality, and security checks',
      steps: [
        {
          id: 'syntax-check',
          name: 'Syntax Analysis',
          agentType: AgentType.ANALYZER,
          taskType: 'analyze_code',
          parameters: { analysisType: 'syntax' },
          timeout: 30000
        },
        {
          id: 'quality-check',
          name: 'Quality Analysis',
          agentType: AgentType.ANALYZER,
          taskType: 'analyze_code',
          parameters: { analysisType: 'quality' },
          dependencies: ['syntax-check'],
          timeout: 45000
        },
        {
          id: 'security-check',
          name: 'Security Analysis',
          agentType: AgentType.ANALYZER,
          taskType: 'analyze_code',
          parameters: { analysisType: 'security' },
          dependencies: ['syntax-check'],
          timeout: 60000
        },
        {
          id: 'generate-report',
          name: 'Generate Analysis Report',
          agentType: AgentType.REPORTER,
          taskType: 'generate_report',
          parameters: { reportType: 'analysis_summary' },
          dependencies: ['quality-check', 'security-check'],
          timeout: 30000
        }
      ],
      parallelism: 'hybrid',
      timeout: 300000
    };

    this.workflows.set(codeAnalysisWorkflow.id, codeAnalysisWorkflow);

    // Code transformation workflow
    const transformationWorkflow: WorkflowDefinition = {
      id: 'code-transformation',
      name: 'Code Transformation Pipeline',
      description: 'Transform and validate code changes',
      steps: [
        {
          id: 'analyze-source',
          name: 'Analyze Source Code',
          agentType: AgentType.ANALYZER,
          taskType: 'analyze_code',
          timeout: 30000
        },
        {
          id: 'transform-code',
          name: 'Transform Code',
          agentType: AgentType.TRANSFORMER,
          taskType: 'transform_code',
          dependencies: ['analyze-source'],
          timeout: 60000
        },
        {
          id: 'validate-changes',
          name: 'Validate Transformation',
          agentType: AgentType.VALIDATOR,
          taskType: 'validate_code',
          dependencies: ['transform-code'],
          timeout: 45000
        }
      ],
      parallelism: 'sequential',
      timeout: 180000
    };

    this.workflows.set(transformationWorkflow.id, transformationWorkflow);
  }

  private startAgentMonitoring(): void {
    // Start monitoring agent health and availability
    setInterval(() => {
      this.checkAgentHealth();
    }, 30000);
  }

  private stopAgentMonitoring(): void {
    // Stop monitoring
  }

  private async checkAgentHealth(): Promise<void> {
    // Check health of all registered agents
    for (const [agentType, agents] of this.availableAgents) {
      for (const agentId of agents) {
        // Simulate health check
        const isHealthy = Math.random() > 0.1; // 90% healthy
        if (!isHealthy) {
          this.emit('agent:unhealthy', { agentId, agentType });
        }
      }
    }
  }

  private async cancelActiveExecutions(): Promise<void> {
    for (const execution of this.activeExecutions.values()) {
      execution.status = 'cancelled';
      execution.endTime = new Date();
    }
    this.activeExecutions.clear();
  }

  // Public methods for workflow management
  public registerWorkflow(workflow: WorkflowDefinition): void {
    this.workflows.set(workflow.id, workflow);
    this.emit('workflow:registered', { workflowId: workflow.id });
  }

  public getWorkflows(): WorkflowDefinition[] {
    return Array.from(this.workflows.values());
  }

  public getActiveExecutions(): WorkflowExecution[] {
    return Array.from(this.activeExecutions.values());
  }

  public getAvailableAgents(): Map<AgentType, string[]> {
    return new Map(this.availableAgents);
  }
}