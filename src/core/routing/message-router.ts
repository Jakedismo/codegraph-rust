import { EventEmitter } from 'events';
import { JsonRpcMessage } from '@modelcontextprotocol/sdk/types.js';
import { MessageValidator, ValidationResult } from '../validation/message-validator.js';

export interface RoutePattern {
  method: string;
  version?: string;
  priority?: number;
  conditions?: RouteCondition[];
}

export interface RouteCondition {
  field: string;
  operator: 'eq' | 'ne' | 'gt' | 'lt' | 'in' | 'matches' | 'exists';
  value: any;
}

export interface RouteHandler {
  handle(message: JsonRpcMessage, context: RouteContext): Promise<RouteResult>;
  canHandle?(message: JsonRpcMessage, context: RouteContext): boolean;
}

export interface RouteContext {
  sessionId: string;
  agentId?: string;
  metadata: Record<string, any>;
  timestamp: Date;
  correlationId?: string;
}

export interface RouteResult {
  success: boolean;
  response?: JsonRpcMessage;
  error?: string;
  forward?: ForwardInstruction;
  metadata?: Record<string, any>;
}

export interface ForwardInstruction {
  targetAgents?: string[];
  targetSessions?: string[];
  broadcast?: boolean;
  delay?: number;
  retries?: number;
}

export interface LoadBalancingStrategy {
  selectTarget(candidates: string[], context: RouteContext): string | null;
}

export interface FailoverStrategy {
  getAlternatives(primary: string, context: RouteContext): string[];
}

export class MessageRouter extends EventEmitter {
  private routes = new Map<string, RouteHandler[]>();
  private validator: MessageValidator;
  private loadBalancer?: LoadBalancingStrategy;
  private failoverStrategy?: FailoverStrategy;
  private metrics = {
    messagesRouted: 0,
    routingErrors: 0,
    averageLatency: 0,
    lastRouted: null as Date | null
  };

  constructor(validator?: MessageValidator) {
    super();
    this.validator = validator || new MessageValidator();
  }

  public addRoute(pattern: RoutePattern, handler: RouteHandler): void {
    const key = this.createRouteKey(pattern);
    
    if (!this.routes.has(key)) {
      this.routes.set(key, []);
    }
    
    const handlers = this.routes.get(key)!;
    
    // Insert handler based on priority (higher priority first)
    const priority = pattern.priority || 0;
    const insertIndex = handlers.findIndex(h => (h as any).priority < priority);
    
    if (insertIndex === -1) {
      handlers.push(handler);
    } else {
      handlers.splice(insertIndex, 0, handler);
    }
    
    (handler as any).priority = priority;
    (handler as any).pattern = pattern;
    
    this.emit('route:added', { pattern, handler });
  }

  public removeRoute(pattern: RoutePattern, handler?: RouteHandler): boolean {
    const key = this.createRouteKey(pattern);
    const handlers = this.routes.get(key);
    
    if (!handlers) return false;
    
    if (handler) {
      const index = handlers.indexOf(handler);
      if (index !== -1) {
        handlers.splice(index, 1);
        if (handlers.length === 0) {
          this.routes.delete(key);
        }
        this.emit('route:removed', { pattern, handler });
        return true;
      }
    } else {
      // Remove all handlers for this pattern
      const removed = handlers.length;
      this.routes.delete(key);
      this.emit('route:removed', { pattern, count: removed });
      return removed > 0;
    }
    
    return false;
  }

  public async route(message: JsonRpcMessage, context: RouteContext): Promise<RouteResult> {
    const startTime = Date.now();
    
    try {
      // Validate message
      const validation = this.validator.validateMessage(message);
      if (!validation.valid) {
        this.metrics.routingErrors++;
        return {
          success: false,
          error: `Message validation failed: ${validation.errors?.join(', ')}`,
          response: this.createErrorResponse(message.id, -32602, 'Invalid params')
        };
      }

      // Find matching handlers
      const handlers = this.findHandlers(message, context);
      
      if (handlers.length === 0) {
        this.emit('route:no_handler', { message, context });
        return {
          success: false,
          error: `No handler found for method: ${message.method}`,
          response: this.createErrorResponse(message.id, -32601, 'Method not found')
        };
      }

      // Try handlers in priority order
      for (const handler of handlers) {
        try {
          if (handler.canHandle && !handler.canHandle(message, context)) {
            continue;
          }

          const result = await handler.handle(message, context);
          
          if (result.success) {
            this.updateMetrics(startTime);
            this.emit('route:success', { message, context, result, handler });
            return result;
          } else if (result.error) {
            this.emit('route:handler_error', { message, context, error: result.error, handler });
          }
          
        } catch (error) {
          this.emit('route:handler_exception', { message, context, error, handler });
          continue; // Try next handler
        }
      }

      this.metrics.routingErrors++;
      return {
        success: false,
        error: 'All handlers failed',
        response: this.createErrorResponse(message.id, -32603, 'Internal error')
      };

    } catch (error) {
      this.metrics.routingErrors++;
      this.emit('route:error', { message, context, error });
      
      return {
        success: false,
        error: `Routing failed: ${error}`,
        response: this.createErrorResponse(message.id, -32603, 'Internal error')
      };
    }
  }

  public async routeWithLoadBalancing(
    message: JsonRpcMessage, 
    context: RouteContext,
    candidates: string[]
  ): Promise<RouteResult> {
    if (!this.loadBalancer || candidates.length === 0) {
      return this.route(message, context);
    }

    const target = this.loadBalancer.selectTarget(candidates, context);
    if (!target) {
      return {
        success: false,
        error: 'Load balancer could not select target'
      };
    }

    const targetContext = { ...context, targetAgent: target };
    return this.route(message, targetContext);
  }

  public async routeWithFailover(
    message: JsonRpcMessage,
    context: RouteContext,
    primary: string
  ): Promise<RouteResult> {
    // Try primary target first
    const primaryContext = { ...context, targetAgent: primary };
    const primaryResult = await this.route(message, primaryContext);
    
    if (primaryResult.success) {
      return primaryResult;
    }

    // Get alternatives from failover strategy
    if (!this.failoverStrategy) {
      return primaryResult;
    }

    const alternatives = this.failoverStrategy.getAlternatives(primary, context);
    
    for (const alternative of alternatives) {
      const altContext = { ...context, targetAgent: alternative };
      const altResult = await this.route(message, altContext);
      
      if (altResult.success) {
        this.emit('route:failover_success', { 
          message, 
          context, 
          primary, 
          alternative,
          result: altResult 
        });
        return altResult;
      }
    }

    this.emit('route:failover_exhausted', { message, context, primary, alternatives });
    return primaryResult; // Return original error
  }

  public async broadcastToGroup(
    group: string,
    message: JsonRpcMessage,
    context: RouteContext
  ): Promise<RouteResult[]> {
    const groupContext = { ...context, targetGroup: group };
    
    // This would typically involve looking up group members
    // For now, we'll emit an event that the transport layer can handle
    this.emit('route:broadcast', { group, message, context: groupContext });
    
    return [{
      success: true,
      metadata: { broadcast: true, group }
    }];
  }

  private findHandlers(message: JsonRpcMessage, context: RouteContext): RouteHandler[] {
    const handlers: RouteHandler[] = [];
    
    for (const [routeKey, routeHandlers] of this.routes) {
      if (this.matchesRoute(routeKey, message, context)) {
        handlers.push(...routeHandlers);
      }
    }
    
    // Sort by priority (higher first)
    return handlers.sort((a, b) => ((b as any).priority || 0) - ((a as any).priority || 0));
  }

  private matchesRoute(routeKey: string, message: JsonRpcMessage, context: RouteContext): boolean {
    const [method, version] = routeKey.split('@');
    
    if (method !== '*' && method !== message.method) {
      return false;
    }
    
    if (version && version !== '*') {
      // Version matching logic would go here
      // For now, we'll assume no version means any version
    }
    
    // Get the pattern conditions for additional matching
    const routeHandlers = this.routes.get(routeKey);
    if (!routeHandlers || routeHandlers.length === 0) {
      return false;
    }
    
    const pattern = (routeHandlers[0] as any).pattern as RoutePattern;
    if (pattern?.conditions) {
      return this.evaluateConditions(pattern.conditions, message, context);
    }
    
    return true;
  }

  private evaluateConditions(
    conditions: RouteCondition[],
    message: JsonRpcMessage,
    context: RouteContext
  ): boolean {
    for (const condition of conditions) {
      if (!this.evaluateCondition(condition, message, context)) {
        return false;
      }
    }
    return true;
  }

  private evaluateCondition(
    condition: RouteCondition,
    message: JsonRpcMessage,
    context: RouteContext
  ): boolean {
    const value = this.getFieldValue(condition.field, message, context);
    
    switch (condition.operator) {
      case 'eq':
        return value === condition.value;
      case 'ne':
        return value !== condition.value;
      case 'gt':
        return value > condition.value;
      case 'lt':
        return value < condition.value;
      case 'in':
        return Array.isArray(condition.value) && condition.value.includes(value);
      case 'matches':
        return typeof value === 'string' && new RegExp(condition.value).test(value);
      case 'exists':
        return value !== undefined && value !== null;
      default:
        return false;
    }
  }

  private getFieldValue(field: string, message: JsonRpcMessage, context: RouteContext): any {
    const parts = field.split('.');
    
    let current: any;
    if (parts[0] === 'message') {
      current = message;
      parts.shift();
    } else if (parts[0] === 'context') {
      current = context;
      parts.shift();
    } else {
      current = message;
    }
    
    for (const part of parts) {
      if (current && typeof current === 'object') {
        current = current[part];
      } else {
        return undefined;
      }
    }
    
    return current;
  }

  private createRouteKey(pattern: RoutePattern): string {
    return `${pattern.method}@${pattern.version || '*'}`;
  }

  private createErrorResponse(id: any, code: number, message: string): JsonRpcMessage {
    return {
      jsonrpc: '2.0',
      error: { code, message },
      id
    };
  }

  private updateMetrics(startTime: number): void {
    const latency = Date.now() - startTime;
    this.metrics.messagesRouted++;
    this.metrics.lastRouted = new Date();
    
    // Simple moving average for latency
    this.metrics.averageLatency = 
      (this.metrics.averageLatency * 0.9) + (latency * 0.1);
  }

  public setLoadBalancer(strategy: LoadBalancingStrategy): void {
    this.loadBalancer = strategy;
  }

  public setFailoverStrategy(strategy: FailoverStrategy): void {
    this.failoverStrategy = strategy;
  }

  public getRoutes(): Array<{ pattern: RoutePattern; handlers: number }> {
    const routes: Array<{ pattern: RoutePattern; handlers: number }> = [];
    
    for (const [key, handlers] of this.routes) {
      if (handlers.length > 0) {
        const pattern = (handlers[0] as any).pattern as RoutePattern;
        routes.push({
          pattern,
          handlers: handlers.length
        });
      }
    }
    
    return routes;
  }

  public getMetrics() {
    return { ...this.metrics };
  }

  public clearRoutes(): void {
    this.routes.clear();
    this.emit('routes:cleared');
  }

  public getHandlerCount(): number {
    let count = 0;
    for (const handlers of this.routes.values()) {
      count += handlers.length;
    }
    return count;
  }
}

// Built-in load balancing strategies
export class RoundRobinLoadBalancer implements LoadBalancingStrategy {
  private currentIndex = 0;

  selectTarget(candidates: string[], context: RouteContext): string | null {
    if (candidates.length === 0) return null;
    
    const target = candidates[this.currentIndex % candidates.length];
    this.currentIndex = (this.currentIndex + 1) % candidates.length;
    return target;
  }
}

export class RandomLoadBalancer implements LoadBalancingStrategy {
  selectTarget(candidates: string[], context: RouteContext): string | null {
    if (candidates.length === 0) return null;
    
    const index = Math.floor(Math.random() * candidates.length);
    return candidates[index];
  }
}

export class WeightedLoadBalancer implements LoadBalancingStrategy {
  constructor(private weights: Map<string, number> = new Map()) {}

  selectTarget(candidates: string[], context: RouteContext): string | null {
    if (candidates.length === 0) return null;
    
    // If no weights configured, use random selection
    if (this.weights.size === 0) {
      return candidates[Math.floor(Math.random() * candidates.length)];
    }
    
    const totalWeight = candidates.reduce((sum, candidate) => 
      sum + (this.weights.get(candidate) || 1), 0
    );
    
    let random = Math.random() * totalWeight;
    
    for (const candidate of candidates) {
      const weight = this.weights.get(candidate) || 1;
      random -= weight;
      if (random <= 0) {
        return candidate;
      }
    }
    
    return candidates[0]; // Fallback
  }

  setWeight(agent: string, weight: number): void {
    this.weights.set(agent, Math.max(0, weight));
  }
}