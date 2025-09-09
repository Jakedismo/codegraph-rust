// Real-time Subscription Architecture for CodeGraph
// High-performance event streaming with sub-50ms latency

import { PubSub } from 'graphql-subscriptions';
import { RedisPubSub } from 'graphql-redis-subscriptions';
import { withFilter } from 'graphql-subscriptions';
import { GraphQLResolveInfo } from 'graphql';

// Event types for the subscription system
export interface CodeChangeEvent {
  type: 'ADD' | 'MODIFY' | 'DELETE' | 'RENAME' | 'MOVE';
  nodeId: string;
  node: any;
  oldValue?: any;
  newValue?: any;
  timestamp: Date;
  source: string;
  batchId?: string;
}

export interface GraphUpdateEvent {
  type: 'NODES_ADDED' | 'NODES_REMOVED' | 'NODES_MODIFIED' | 'RELATIONS_ADDED' | 'RELATIONS_REMOVED' | 'RELATIONS_MODIFIED';
  affectedNodes: string[];
  affectedRelations: string[];
  changeCount: number;
  timestamp: Date;
  batchId?: string;
}

export interface PerformanceAlert {
  queryId: string;
  executionTime: number;
  threshold: number;
  query: string;
  variables: any;
  suggestions: string[];
  timestamp: Date;
}

// High-performance Redis-based pub/sub system
export class CodeGraphPubSub {
  private static instance: CodeGraphPubSub;
  private pubsub: RedisPubSub;
  private eventBuffer: Map<string, any[]> = new Map();
  private batchTimer: NodeJS.Timeout | null = null;
  private readonly BATCH_SIZE = 100;
  private readonly BATCH_TIMEOUT_MS = 10; // Very short batching for low latency
  
  private constructor() {
    this.pubsub = new RedisPubSub({
      publisher: {
        host: process.env.REDIS_HOST || 'localhost',
        port: parseInt(process.env.REDIS_PORT || '6379'),
        // Connection pooling for high throughput
        maxRetriesPerRequest: 3,
        connectTimeout: 1000,
        lazyConnect: true,
      },
      subscriber: {
        host: process.env.REDIS_HOST || 'localhost',
        port: parseInt(process.env.REDIS_PORT || '6379'),
        maxRetriesPerRequest: 3,
        connectTimeout: 1000,
        lazyConnect: true,
      },
      // Use binary protocol for better performance
      messageEventName: 'pmessage',
      patternEventName: 'psubscribe'
    });
  }

  static getInstance(): CodeGraphPubSub {
    if (!CodeGraphPubSub.instance) {
      CodeGraphPubSub.instance = new CodeGraphPubSub();
    }
    return CodeGraphPubSub.instance;
  }

  // Optimized event publishing with batching
  async publishCodeChange(event: CodeChangeEvent): Promise<void> {
    const channel = `code_changed`;
    
    // Add to batch buffer
    if (!this.eventBuffer.has(channel)) {
      this.eventBuffer.set(channel, []);
    }
    
    this.eventBuffer.get(channel)!.push(event);
    
    // Publish immediately for critical events or when batch is full
    if (this.shouldPublishImmediately(event) || this.eventBuffer.get(channel)!.length >= this.BATCH_SIZE) {
      await this.flushBatch(channel);
    } else {
      // Schedule batch publish if not already scheduled
      if (!this.batchTimer) {
        this.batchTimer = setTimeout(() => this.flushAllBatches(), this.BATCH_TIMEOUT_MS);
      }
    }
  }

  private shouldPublishImmediately(event: CodeChangeEvent): boolean {
    // Publish immediately for deletions or critical changes
    return event.type === 'DELETE' || event.source === 'user_action';
  }

  private async flushBatch(channel: string): Promise<void> {
    const events = this.eventBuffer.get(channel);
    if (!events || events.length === 0) return;

    // Create batch event
    const batchEvent = {
      events,
      batchId: `batch_${Date.now()}_${Math.random().toString(36).substr(2, 9)}`,
      timestamp: new Date(),
      count: events.length
    };

    await this.pubsub.publish(channel, batchEvent);
    this.eventBuffer.set(channel, []);
  }

  private async flushAllBatches(): Promise<void> {
    const flushPromises = Array.from(this.eventBuffer.keys()).map(channel => 
      this.flushBatch(channel)
    );
    
    await Promise.all(flushPromises);
    this.batchTimer = null;
  }

  async publishGraphUpdate(event: GraphUpdateEvent): Promise<void> {
    await this.pubsub.publish('graph_updated', event);
  }

  async publishPerformanceAlert(alert: PerformanceAlert): Promise<void> {
    await this.pubsub.publish('performance_alert', alert);
  }

  async publishCacheInvalidation(pattern: string, affectedQueries: string[], reason: string): Promise<void> {
    const event = {
      pattern,
      affectedQueries,
      reason,
      timestamp: new Date()
    };
    
    await this.pubsub.publish('cache_invalidated', event);
  }

  // High-performance subscription management
  subscribeToCodeChanges(filter?: any) {
    return withFilter(
      () => this.pubsub.asyncIterator('code_changed'),
      (payload, variables) => {
        if (!filter) return true;
        
        // Fast filtering logic
        if (filter.filePattern) {
          const regex = new RegExp(filter.filePattern);
          return payload.events.some((event: CodeChangeEvent) => 
            regex.test(event.node?.location?.file || '')
          );
        }
        
        if (filter.nodeTypes) {
          return payload.events.some((event: CodeChangeEvent) => 
            filter.nodeTypes.includes(event.node?.type)
          );
        }
        
        return true;
      }
    );
  }

  subscribeToGraphUpdates(filter?: any) {
    return withFilter(
      () => this.pubsub.asyncIterator('graph_updated'),
      (payload, variables) => {
        if (!filter) return true;
        
        if (filter.nodeIds) {
          return payload.affectedNodes.some((nodeId: string) => 
            filter.nodeIds.includes(nodeId)
          );
        }
        
        if (filter.relationTypes) {
          // Would need additional metadata in the event
          return true; // Simplified for now
        }
        
        return true;
      }
    );
  }

  subscribeToPerformanceAlerts(thresholdMs: number = 50) {
    return withFilter(
      () => this.pubsub.asyncIterator('performance_alert'),
      (payload: PerformanceAlert) => {
        return payload.executionTime >= thresholdMs;
      }
    );
  }

  async close(): Promise<void> {
    if (this.batchTimer) {
      clearTimeout(this.batchTimer);
      await this.flushAllBatches();
    }
    await this.pubsub.close();
  }
}

// Subscription resolvers with performance monitoring
export class SubscriptionResolvers {
  private pubsub: CodeGraphPubSub;
  private performanceMonitor: PerformanceMonitor;

  constructor() {
    this.pubsub = CodeGraphPubSub.getInstance();
    this.performanceMonitor = new PerformanceMonitor();
  }

  // Code change subscription with smart filtering
  codeChanged = {
    subscribe: withFilter(
      () => this.pubsub.subscribeToCodeChanges(),
      async (payload: any, args: any, context: any, info: GraphQLResolveInfo) => {
        const startTime = performance.now();
        
        try {
          // Apply filters efficiently
          let filtered = payload.events;
          
          if (args.filePattern) {
            const regex = new RegExp(args.filePattern);
            filtered = filtered.filter((event: CodeChangeEvent) => 
              regex.test(event.node?.location?.file || '')
            );
          }
          
          if (args.nodeTypes?.length > 0) {
            filtered = filtered.filter((event: CodeChangeEvent) => 
              args.nodeTypes.includes(event.node?.type)
            );
          }
          
          // Performance tracking
          const duration = performance.now() - startTime;
          if (duration > 5) { // 5ms threshold for filter processing
            console.warn(`Slow subscription filter: ${duration}ms`);
          }
          
          return filtered.length > 0;
        } catch (error) {
          console.error('Subscription filter error:', error);
          return false;
        }
      }
    ),
    
    resolve: async (payload: any) => {
      // Return the most recent event or a batch summary
      if (payload.events.length === 1) {
        return payload.events[0];
      }
      
      // For batches, return a summary event
      return {
        type: 'BATCH',
        batchId: payload.batchId,
        eventCount: payload.count,
        events: payload.events,
        timestamp: payload.timestamp
      };
    }
  };

  // Graph update subscription with intelligent debouncing
  graphUpdated = {
    subscribe: () => this.pubsub.subscribeToGraphUpdates(),
    resolve: async (payload: GraphUpdateEvent, args: any, context: any) => {
      // Enrich with additional context if needed
      return {
        ...payload,
        impactedSubgraphs: await this.calculateImpactedSubgraphs(payload.affectedNodes)
      };
    }
  };

  // Performance alert subscription with escalation
  performanceAlert = {
    subscribe: (parent: any, args: { thresholdMs?: number }) => 
      this.pubsub.subscribeToPerformanceAlerts(args.thresholdMs),
    
    resolve: async (payload: PerformanceAlert) => {
      // Add enriched information
      const queryComplexity = await this.analyzeQueryComplexity(payload.query, payload.variables);
      
      return {
        ...payload,
        complexity: queryComplexity,
        recommendedOptimizations: this.generateOptimizationSuggestions(payload, queryComplexity)
      };
    }
  };

  // Analysis progress subscription for long-running operations
  analysisProgress = {
    subscribe: withFilter(
      () => this.pubsub.pubsub.asyncIterator('analysis_progress'),
      (payload: any, args: { analysisId: string }) => {
        return payload.analysisId === args.analysisId;
      }
    )
  };

  private async calculateImpactedSubgraphs(nodeIds: string[]): Promise<string[]> {
    // Implementation would analyze which subgraphs are affected
    return []; // Placeholder
  }

  private async analyzeQueryComplexity(query: string, variables: any): Promise<number> {
    // Implementation would parse and analyze query complexity
    return 100; // Placeholder
  }

  private generateOptimizationSuggestions(alert: PerformanceAlert, complexity: number): string[] {
    const suggestions = [];
    
    if (alert.executionTime > 100) {
      suggestions.push('Consider adding query limits or pagination');
    }
    
    if (complexity > 200) {
      suggestions.push('Break down complex queries into smaller parts');
    }
    
    if (alert.query.includes('subgraph') && alert.executionTime > 50) {
      suggestions.push('Use depth limits for subgraph queries');
    }
    
    return suggestions;
  }
}

// Performance monitoring for subscription system
export class PerformanceMonitor {
  private metrics: Map<string, number[]> = new Map();
  private readonly METRIC_WINDOW_SIZE = 100;

  recordSubscriptionLatency(subscriptionType: string, latency: number): void {
    if (!this.metrics.has(subscriptionType)) {
      this.metrics.set(subscriptionType, []);
    }
    
    const latencies = this.metrics.get(subscriptionType)!;
    latencies.push(latency);
    
    // Keep only recent measurements
    if (latencies.length > this.METRIC_WINDOW_SIZE) {
      latencies.shift();
    }
  }

  getAverageLatency(subscriptionType: string): number {
    const latencies = this.metrics.get(subscriptionType);
    if (!latencies || latencies.length === 0) return 0;
    
    return latencies.reduce((sum, lat) => sum + lat, 0) / latencies.length;
  }

  getPercentileLatency(subscriptionType: string, percentile: number): number {
    const latencies = this.metrics.get(subscriptionType);
    if (!latencies || latencies.length === 0) return 0;
    
    const sorted = [...latencies].sort((a, b) => a - b);
    const index = Math.floor((percentile / 100) * sorted.length);
    return sorted[index] || 0;
  }

  async checkPerformanceThresholds(): Promise<void> {
    for (const [subscriptionType, latencies] of this.metrics.entries()) {
      const avgLatency = this.getAverageLatency(subscriptionType);
      const p95Latency = this.getPercentileLatency(subscriptionType, 95);
      
      if (avgLatency > 25 || p95Latency > 50) {
        const alert: PerformanceAlert = {
          queryId: `subscription_${subscriptionType}`,
          executionTime: Math.max(avgLatency, p95Latency),
          threshold: 25,
          query: `subscription { ${subscriptionType} }`,
          variables: {},
          suggestions: [
            'Review subscription filter complexity',
            'Consider batch processing for high-volume subscriptions',
            'Check Redis connection pool settings'
          ],
          timestamp: new Date()
        };
        
        await CodeGraphPubSub.getInstance().publishPerformanceAlert(alert);
      }
    }
  }
}

// Connection management for scalable subscriptions
export class SubscriptionConnectionManager {
  private connections: Map<string, WebSocket> = new Map();
  private subscriptions: Map<string, Set<string>> = new Map(); // connectionId -> subscriptionIds
  private readonly MAX_SUBSCRIPTIONS_PER_CONNECTION = 50;
  private readonly CONNECTION_TIMEOUT_MS = 30000;

  addConnection(connectionId: string, ws: WebSocket): void {
    this.connections.set(connectionId, ws);
    this.subscriptions.set(connectionId, new Set());
    
    // Set up connection timeout
    setTimeout(() => {
      if (this.connections.has(connectionId)) {
        this.removeConnection(connectionId);
      }
    }, this.CONNECTION_TIMEOUT_MS);
  }

  removeConnection(connectionId: string): void {
    this.connections.delete(connectionId);
    this.subscriptions.delete(connectionId);
  }

  addSubscription(connectionId: string, subscriptionId: string): boolean {
    const connectionSubs = this.subscriptions.get(connectionId);
    if (!connectionSubs) return false;
    
    if (connectionSubs.size >= this.MAX_SUBSCRIPTIONS_PER_CONNECTION) {
      return false; // Rate limiting
    }
    
    connectionSubs.add(subscriptionId);
    return true;
  }

  removeSubscription(connectionId: string, subscriptionId: string): void {
    this.subscriptions.get(connectionId)?.delete(subscriptionId);
  }

  getConnectionStats(): {
    totalConnections: number;
    totalSubscriptions: number;
    averageSubscriptionsPerConnection: number;
  } {
    const totalConnections = this.connections.size;
    const totalSubscriptions = Array.from(this.subscriptions.values())
      .reduce((sum, subs) => sum + subs.size, 0);
    
    return {
      totalConnections,
      totalSubscriptions,
      averageSubscriptionsPerConnection: totalConnections > 0 ? totalSubscriptions / totalConnections : 0
    };
  }
}

// Export configured subscription resolvers
export const subscriptionResolvers = new SubscriptionResolvers();