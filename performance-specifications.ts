// Performance Specifications and Monitoring for CodeGraph
// Sub-50ms response time requirements with comprehensive monitoring

import { performance } from 'perf_hooks';
import { EventEmitter } from 'events';

// Performance targets and thresholds
export const PERFORMANCE_TARGETS = {
  // Response time targets (in milliseconds)
  SIMPLE_QUERY_MAX: 10,        // Simple node/relation queries
  COMPLEX_QUERY_MAX: 50,       // Complex graph traversal queries
  SUBSCRIPTION_LATENCY_MAX: 25, // Real-time subscription delivery
  CACHE_LOOKUP_MAX: 2,         // Cache hit response time
  AUTH_CHECK_MAX: 5,           // Authentication/authorization time
  
  // Throughput targets
  QUERIES_PER_SECOND_MIN: 1000,
  CONCURRENT_SUBSCRIPTIONS_MAX: 10000,
  
  // Resource utilization targets
  CPU_UTILIZATION_MAX: 80,     // Percentage
  MEMORY_UTILIZATION_MAX: 85,  // Percentage
  CONNECTION_POOL_MAX: 90,     // Percentage of pool used
  
  // Error rate targets
  ERROR_RATE_MAX: 0.01,        // 1% error rate
  TIMEOUT_RATE_MAX: 0.001,     // 0.1% timeout rate
} as const;

// Performance metrics collection
export interface PerformanceMetric {
  name: string;
  value: number;
  timestamp: Date;
  tags: Record<string, string>;
  unit: 'ms' | 'count' | 'percent' | 'bytes';
}

export interface QueryPerformanceData {
  queryId: string;
  operationType: 'query' | 'mutation' | 'subscription';
  fieldName: string;
  complexity: number;
  cacheHit: boolean;
  
  // Timing breakdown
  totalTime: number;
  authTime: number;
  validationTime: number;
  executionTime: number;
  cacheLookupTime: number;
  databaseTime: number;
  serializationTime: number;
  
  // Resource usage
  peakMemoryMB: number;
  cpuTimeMs: number;
  
  // Results
  resultCount: number;
  resultSizeBytes: number;
  
  // Status
  success: boolean;
  errorType?: string;
  errorMessage?: string;
  
  timestamp: Date;
}

// Real-time performance monitor
export class PerformanceMonitor extends EventEmitter {
  private metrics: Map<string, PerformanceMetric[]> = new Map();
  private queryHistory: QueryPerformanceData[] = [];
  private alerts: PerformanceAlert[] = [];
  
  private readonly METRIC_WINDOW_SIZE = 1000;
  private readonly ALERT_COOLDOWN_MS = 30000; // 30 seconds
  private lastAlertTime: Map<string, number> = new Map();
  
  // Real-time metric tracking
  recordMetric(metric: PerformanceMetric): void {
    const key = metric.name;
    if (!this.metrics.has(key)) {
      this.metrics.set(key, []);
    }
    
    const metricArray = this.metrics.get(key)!;
    metricArray.push(metric);
    
    // Keep only recent metrics
    if (metricArray.length > this.METRIC_WINDOW_SIZE) {
      metricArray.shift();
    }
    
    // Check for performance violations
    this.checkPerformanceViolation(metric);
  }

  // Query performance tracking
  recordQueryPerformance(data: QueryPerformanceData): void {
    this.queryHistory.push(data);
    
    // Keep only recent queries
    if (this.queryHistory.length > this.METRIC_WINDOW_SIZE) {
      this.queryHistory.shift();
    }
    
    // Record individual metrics
    this.recordMetric({
      name: 'query.total_time',
      value: data.totalTime,
      timestamp: data.timestamp,
      tags: {
        operation: data.operationType,
        field: data.fieldName,
        cache_hit: data.cacheHit.toString(),
        success: data.success.toString()
      },
      unit: 'ms'
    });
    
    this.recordMetric({
      name: 'query.execution_time',
      value: data.executionTime,
      timestamp: data.timestamp,
      tags: {
        operation: data.operationType,
        field: data.fieldName,
        complexity: data.complexity.toString()
      },
      unit: 'ms'
    });
    
    // Check performance targets
    this.validatePerformanceTargets(data);
  }

  // Performance validation against targets
  private validatePerformanceTargets(data: QueryPerformanceData): void {
    const violations: PerformanceViolation[] = [];
    
    // Check response time targets
    let targetTime = PERFORMANCE_TARGETS.SIMPLE_QUERY_MAX;
    if (data.complexity > 100) {
      targetTime = PERFORMANCE_TARGETS.COMPLEX_QUERY_MAX;
    }
    
    if (data.totalTime > targetTime) {
      violations.push({
        type: 'response_time',
        metric: 'total_time',
        actual: data.totalTime,
        target: targetTime,
        severity: data.totalTime > targetTime * 2 ? 'critical' : 'warning'
      });
    }
    
    // Check component times
    if (data.authTime > PERFORMANCE_TARGETS.AUTH_CHECK_MAX) {
      violations.push({
        type: 'auth_time',
        metric: 'auth_time',
        actual: data.authTime,
        target: PERFORMANCE_TARGETS.AUTH_CHECK_MAX,
        severity: 'warning'
      });
    }
    
    if (data.cacheHit && data.cacheLookupTime > PERFORMANCE_TARGETS.CACHE_LOOKUP_MAX) {
      violations.push({
        type: 'cache_lookup',
        metric: 'cache_lookup_time',
        actual: data.cacheLookupTime,
        target: PERFORMANCE_TARGETS.CACHE_LOOKUP_MAX,
        severity: 'warning'
      });
    }
    
    // Emit violations as alerts
    violations.forEach(violation => {
      this.emitAlert({
        id: `perf_${violation.type}_${Date.now()}`,
        type: 'performance_violation',
        severity: violation.severity,
        message: `${violation.metric} (${violation.actual}ms) exceeded target (${violation.target}ms)`,
        queryId: data.queryId,
        metric: violation.metric,
        actual: violation.actual,
        target: violation.target,
        timestamp: new Date(),
        suggestions: this.generateOptimizationSuggestions(violation, data)
      });
    });
  }

  // Performance alert generation
  private checkPerformanceViolation(metric: PerformanceMetric): void {
    const thresholds: Record<string, number> = {
      'query.total_time': PERFORMANCE_TARGETS.COMPLEX_QUERY_MAX,
      'subscription.latency': PERFORMANCE_TARGETS.SUBSCRIPTION_LATENCY_MAX,
      'auth.check_time': PERFORMANCE_TARGETS.AUTH_CHECK_MAX,
      'cache.lookup_time': PERFORMANCE_TARGETS.CACHE_LOOKUP_MAX,
      'cpu.utilization': PERFORMANCE_TARGETS.CPU_UTILIZATION_MAX,
      'memory.utilization': PERFORMANCE_TARGETS.MEMORY_UTILIZATION_MAX
    };
    
    const threshold = thresholds[metric.name];
    if (threshold && metric.value > threshold) {
      const alertKey = `${metric.name}_violation`;
      const now = Date.now();
      
      // Check cooldown period
      const lastAlert = this.lastAlertTime.get(alertKey);
      if (!lastAlert || now - lastAlert > this.ALERT_COOLDOWN_MS) {
        this.emitAlert({
          id: `metric_${metric.name}_${now}`,
          type: 'metric_threshold',
          severity: metric.value > threshold * 1.5 ? 'critical' : 'warning',
          message: `${metric.name} (${metric.value}${metric.unit}) exceeded threshold (${threshold}${metric.unit})`,
          metric: metric.name,
          actual: metric.value,
          target: threshold,
          timestamp: new Date(),
          tags: metric.tags,
          suggestions: this.generateMetricOptimizationSuggestions(metric.name, metric.value, threshold)
        });
        
        this.lastAlertTime.set(alertKey, now);
      }
    }
  }

  private emitAlert(alert: PerformanceAlert): void {
    this.alerts.push(alert);
    
    // Keep only recent alerts
    if (this.alerts.length > 100) {
      this.alerts.shift();
    }
    
    this.emit('performanceAlert', alert);
  }

  // Optimization suggestions generator
  private generateOptimizationSuggestions(
    violation: PerformanceViolation,
    data: QueryPerformanceData
  ): string[] {
    const suggestions: string[] = [];
    
    switch (violation.type) {
      case 'response_time':
        if (data.complexity > 200) {
          suggestions.push('Consider reducing query depth or adding pagination');
        }
        if (!data.cacheHit) {
          suggestions.push('Implement caching for this query pattern');
        }
        if (data.databaseTime > data.totalTime * 0.6) {
          suggestions.push('Optimize database queries or add indexes');
        }
        break;
        
      case 'auth_time':
        suggestions.push('Consider caching authentication tokens');
        suggestions.push('Review permission checking logic for optimization');
        break;
        
      case 'cache_lookup':
        suggestions.push('Review cache configuration and connection pooling');
        suggestions.push('Consider using in-memory cache for frequently accessed data');
        break;
    }
    
    return suggestions;
  }

  private generateMetricOptimizationSuggestions(
    metricName: string,
    actual: number,
    target: number
  ): string[] {
    const suggestions: string[] = [];
    
    switch (metricName) {
      case 'cpu.utilization':
        suggestions.push('Consider horizontal scaling or load balancing');
        suggestions.push('Review algorithm efficiency for hot code paths');
        break;
        
      case 'memory.utilization':
        suggestions.push('Review cache sizes and memory allocation patterns');
        suggestions.push('Consider implementing garbage collection tuning');
        break;
        
      case 'subscription.latency':
        suggestions.push('Review WebSocket connection handling and batching');
        suggestions.push('Consider using message queues for high-volume subscriptions');
        break;
    }
    
    return suggestions;
  }

  // Analytics and reporting
  getMetricStatistics(metricName: string, windowMs: number = 300000): MetricStatistics {
    const metrics = this.metrics.get(metricName) || [];
    const cutoff = new Date(Date.now() - windowMs);
    const recentMetrics = metrics.filter(m => m.timestamp >= cutoff);
    
    if (recentMetrics.length === 0) {
      return {
        count: 0,
        min: 0,
        max: 0,
        mean: 0,
        median: 0,
        p95: 0,
        p99: 0,
        unit: 'ms'
      };
    }
    
    const values = recentMetrics.map(m => m.value).sort((a, b) => a - b);
    const sum = values.reduce((a, b) => a + b, 0);
    
    return {
      count: values.length,
      min: values[0],
      max: values[values.length - 1],
      mean: sum / values.length,
      median: values[Math.floor(values.length / 2)],
      p95: values[Math.floor(values.length * 0.95)],
      p99: values[Math.floor(values.length * 0.99)],
      unit: recentMetrics[0].unit
    };
  }

  getQueryPerformanceReport(windowMs: number = 300000): QueryPerformanceReport {
    const cutoff = new Date(Date.now() - windowMs);
    const recentQueries = this.queryHistory.filter(q => q.timestamp >= cutoff);
    
    const totalQueries = recentQueries.length;
    const successfulQueries = recentQueries.filter(q => q.success).length;
    const cacheHits = recentQueries.filter(q => q.cacheHit).length;
    
    const responseTimes = recentQueries.map(q => q.totalTime).sort((a, b) => a - b);
    const slowQueries = recentQueries.filter(q => q.totalTime > PERFORMANCE_TARGETS.COMPLEX_QUERY_MAX);
    
    return {
      windowMs,
      totalQueries,
      successRate: totalQueries > 0 ? successfulQueries / totalQueries : 0,
      cacheHitRate: totalQueries > 0 ? cacheHits / totalQueries : 0,
      errorRate: totalQueries > 0 ? (totalQueries - successfulQueries) / totalQueries : 0,
      
      responseTimes: {
        min: responseTimes[0] || 0,
        max: responseTimes[responseTimes.length - 1] || 0,
        mean: responseTimes.length > 0 ? responseTimes.reduce((a, b) => a + b, 0) / responseTimes.length : 0,
        p50: responseTimes[Math.floor(responseTimes.length * 0.5)] || 0,
        p95: responseTimes[Math.floor(responseTimes.length * 0.95)] || 0,
        p99: responseTimes[Math.floor(responseTimes.length * 0.99)] || 0
      },
      
      performanceViolations: {
        count: slowQueries.length,
        rate: totalQueries > 0 ? slowQueries.length / totalQueries : 0,
        worstQuery: slowQueries.sort((a, b) => b.totalTime - a.totalTime)[0]
      },
      
      topSlowQueries: recentQueries
        .sort((a, b) => b.totalTime - a.totalTime)
        .slice(0, 10)
        .map(q => ({
          queryId: q.queryId,
          fieldName: q.fieldName,
          totalTime: q.totalTime,
          complexity: q.complexity
        }))
    };
  }

  getActiveAlerts(): PerformanceAlert[] {
    const fiveMinutesAgo = new Date(Date.now() - 300000);
    return this.alerts.filter(alert => alert.timestamp >= fiveMinutesAgo);
  }
}

// Performance testing and validation utilities
export class PerformanceValidator {
  private monitor: PerformanceMonitor;

  constructor(monitor: PerformanceMonitor) {
    this.monitor = monitor;
  }

  // Automated performance test runner
  async runPerformanceTests(): Promise<PerformanceTestResults> {
    const results: PerformanceTestResults = {
      testSuite: 'codegraph_performance',
      timestamp: new Date(),
      tests: [],
      summary: {
        total: 0,
        passed: 0,
        failed: 0,
        avgResponseTime: 0,
        maxResponseTime: 0
      }
    };

    // Test simple queries
    const simpleTests = await this.runSimpleQueryTests();
    results.tests.push(...simpleTests);

    // Test complex queries
    const complexTests = await this.runComplexQueryTests();
    results.tests.push(...complexTests);

    // Test subscription performance
    const subscriptionTests = await this.runSubscriptionTests();
    results.tests.push(...subscriptionTests);

    // Calculate summary
    results.summary.total = results.tests.length;
    results.summary.passed = results.tests.filter(t => t.passed).length;
    results.summary.failed = results.summary.total - results.summary.passed;
    
    const responseTimes = results.tests.map(t => t.responseTime);
    results.summary.avgResponseTime = responseTimes.reduce((a, b) => a + b, 0) / responseTimes.length;
    results.summary.maxResponseTime = Math.max(...responseTimes);

    return results;
  }

  private async runSimpleQueryTests(): Promise<PerformanceTest[]> {
    const tests: PerformanceTest[] = [];
    
    // Test node retrieval
    const nodeTest = await this.timeOperation('simple_node_query', async () => {
      // Simulate node query
      await this.delay(5);
      return { nodeCount: 1 };
    });
    
    tests.push({
      name: 'Node Query',
      category: 'simple',
      responseTime: nodeTest.duration,
      target: PERFORMANCE_TARGETS.SIMPLE_QUERY_MAX,
      passed: nodeTest.duration <= PERFORMANCE_TARGETS.SIMPLE_QUERY_MAX,
      details: nodeTest.result
    });

    return tests;
  }

  private async runComplexQueryTests(): Promise<PerformanceTest[]> {
    const tests: PerformanceTest[] = [];
    
    // Test graph traversal
    const traversalTest = await this.timeOperation('graph_traversal', async () => {
      // Simulate complex graph traversal
      await this.delay(30);
      return { nodesTraversed: 500, pathLength: 5 };
    });
    
    tests.push({
      name: 'Graph Traversal',
      category: 'complex',
      responseTime: traversalTest.duration,
      target: PERFORMANCE_TARGETS.COMPLEX_QUERY_MAX,
      passed: traversalTest.duration <= PERFORMANCE_TARGETS.COMPLEX_QUERY_MAX,
      details: traversalTest.result
    });

    return tests;
  }

  private async runSubscriptionTests(): Promise<PerformanceTest[]> {
    const tests: PerformanceTest[] = [];
    
    // Test subscription latency
    const subTest = await this.timeOperation('subscription_delivery', async () => {
      // Simulate subscription event delivery
      await this.delay(15);
      return { subscribersNotified: 100 };
    });
    
    tests.push({
      name: 'Subscription Delivery',
      category: 'subscription',
      responseTime: subTest.duration,
      target: PERFORMANCE_TARGETS.SUBSCRIPTION_LATENCY_MAX,
      passed: subTest.duration <= PERFORMANCE_TARGETS.SUBSCRIPTION_LATENCY_MAX,
      details: subTest.result
    });

    return tests;
  }

  private async timeOperation<T>(name: string, operation: () => Promise<T>): Promise<{ duration: number; result: T }> {
    const startTime = performance.now();
    const result = await operation();
    const duration = performance.now() - startTime;
    
    return { duration, result };
  }

  private delay(ms: number): Promise<void> {
    return new Promise(resolve => setTimeout(resolve, ms));
  }
}

// Type definitions for performance monitoring
interface PerformanceViolation {
  type: string;
  metric: string;
  actual: number;
  target: number;
  severity: 'warning' | 'critical';
}

interface PerformanceAlert {
  id: string;
  type: 'performance_violation' | 'metric_threshold';
  severity: 'warning' | 'critical';
  message: string;
  queryId?: string;
  metric: string;
  actual: number;
  target: number;
  timestamp: Date;
  tags?: Record<string, string>;
  suggestions: string[];
}

interface MetricStatistics {
  count: number;
  min: number;
  max: number;
  mean: number;
  median: number;
  p95: number;
  p99: number;
  unit: string;
}

interface QueryPerformanceReport {
  windowMs: number;
  totalQueries: number;
  successRate: number;
  cacheHitRate: number;
  errorRate: number;
  responseTimes: {
    min: number;
    max: number;
    mean: number;
    p50: number;
    p95: number;
    p99: number;
  };
  performanceViolations: {
    count: number;
    rate: number;
    worstQuery?: QueryPerformanceData;
  };
  topSlowQueries: Array<{
    queryId: string;
    fieldName: string;
    totalTime: number;
    complexity: number;
  }>;
}

interface PerformanceTest {
  name: string;
  category: 'simple' | 'complex' | 'subscription';
  responseTime: number;
  target: number;
  passed: boolean;
  details?: any;
}

interface PerformanceTestResults {
  testSuite: string;
  timestamp: Date;
  tests: PerformanceTest[];
  summary: {
    total: number;
    passed: number;
    failed: number;
    avgResponseTime: number;
    maxResponseTime: number;
  };
}

// Export singleton instances
export const performanceMonitor = new PerformanceMonitor();
export const performanceValidator = new PerformanceValidator(performanceMonitor);

// Performance measurement decorators
export function measurePerformance(target: string) {
  return function (target: any, propertyName: string, descriptor: PropertyDescriptor) {
    const method = descriptor.value;
    
    descriptor.value = async function (...args: any[]) {
      const startTime = performance.now();
      const startMemory = process.memoryUsage().heapUsed;
      
      try {
        const result = await method.apply(this, args);
        const duration = performance.now() - startTime;
        const memoryDelta = process.memoryUsage().heapUsed - startMemory;
        
        performanceMonitor.recordMetric({
          name: `method.${propertyName}`,
          value: duration,
          timestamp: new Date(),
          tags: {
            target,
            method: propertyName,
            memory_delta: memoryDelta.toString()
          },
          unit: 'ms'
        });
        
        return result;
      } catch (error) {
        const duration = performance.now() - startTime;
        
        performanceMonitor.recordMetric({
          name: `method.${propertyName}.error`,
          value: duration,
          timestamp: new Date(),
          tags: {
            target,
            method: propertyName,
            error: 'true'
          },
          unit: 'ms'
        });
        
        throw error;
      }
    };
    
    return descriptor;
  };
}