import { EventEmitter } from 'events';
import PQueue from 'p-queue';
import LRU from 'lru-cache';
import { createHash } from 'crypto';

export interface OptimizationConfig {
  maxConcurrency?: number;
  cacheSize?: number;
  cacheTTL?: number;
  batchSize?: number;
  batchDelay?: number;
  enableQueryOptimization?: boolean;
  enableResultCompression?: boolean;
}

export interface QueryMetrics {
  hits: number;
  misses: number;
  evictions: number;
  avgResponseTime: number;
  p95ResponseTime: number;
  p99ResponseTime: number;
}

export class PerformanceOptimizer extends EventEmitter {
  private cache: LRU<string, any>;
  private queue: PQueue;
  private batchQueue: Map<string, any[]>;
  private batchTimers: Map<string, NodeJS.Timeout>;
  private responseTimes: number[];
  private metrics: QueryMetrics;
  private config: Required<OptimizationConfig>;

  constructor(config: OptimizationConfig = {}) {
    super();
    
    this.config = {
      maxConcurrency: config.maxConcurrency ?? 10,
      cacheSize: config.cacheSize ?? 1000,
      cacheTTL: config.cacheTTL ?? 300000, // 5 minutes
      batchSize: config.batchSize ?? 10,
      batchDelay: config.batchDelay ?? 100, // 100ms
      enableQueryOptimization: config.enableQueryOptimization ?? true,
      enableResultCompression: config.enableResultCompression ?? true
    };

    this.cache = new LRU({
      max: this.config.cacheSize,
      ttl: this.config.cacheTTL,
      dispose: () => {
        this.metrics.evictions++;
      }
    });

    this.queue = new PQueue({ 
      concurrency: this.config.maxConcurrency 
    });

    this.batchQueue = new Map();
    this.batchTimers = new Map();
    this.responseTimes = [];
    
    this.metrics = {
      hits: 0,
      misses: 0,
      evictions: 0,
      avgResponseTime: 0,
      p95ResponseTime: 0,
      p99ResponseTime: 0
    };
  }

  /**
   * Execute a query with caching and optimization
   */
  async executeQuery<T>(
    queryFn: () => Promise<T>,
    cacheKey: string,
    options: {
      skipCache?: boolean;
      priority?: number;
    } = {}
  ): Promise<T> {
    const startTime = Date.now();

    // Check cache first
    if (!options.skipCache && this.cache.has(cacheKey)) {
      this.metrics.hits++;
      this.emit('cache:hit', { key: cacheKey });
      return this.cache.get(cacheKey)!;
    }

    this.metrics.misses++;
    this.emit('cache:miss', { key: cacheKey });

    // Execute query with concurrency control
    const result = await this.queue.add(
      async () => {
        try {
          const data = await queryFn();
          
          // Store in cache
          this.cache.set(cacheKey, data);
          
          return data;
        } catch (error) {
          this.emit('query:error', { error, cacheKey });
          throw error;
        }
      },
      { priority: options.priority ?? 0 }
    );

    // Record response time
    const responseTime = Date.now() - startTime;
    this.recordResponseTime(responseTime);

    return result as T;
  }

  /**
   * Batch multiple queries for better performance
   */
  async batchQuery<T>(
    batchKey: string,
    query: any,
    executeBatch: (queries: any[]) => Promise<T[]>
  ): Promise<T> {
    return new Promise((resolve, reject) => {
      // Add to batch queue
      if (!this.batchQueue.has(batchKey)) {
        this.batchQueue.set(batchKey, []);
      }
      
      const batch = this.batchQueue.get(batchKey)!;
      const queryIndex = batch.length;
      batch.push({ query, resolve, reject, index: queryIndex });

      // Clear existing timer
      if (this.batchTimers.has(batchKey)) {
        clearTimeout(this.batchTimers.get(batchKey)!);
      }

      // Execute batch if size limit reached
      if (batch.length >= this.config.batchSize) {
        this.executeBatchNow(batchKey, executeBatch);
      } else {
        // Set timer for delayed execution
        const timer = setTimeout(() => {
          this.executeBatchNow(batchKey, executeBatch);
        }, this.config.batchDelay);
        
        this.batchTimers.set(batchKey, timer);
      }
    });
  }

  private async executeBatchNow<T>(
    batchKey: string,
    executeBatch: (queries: any[]) => Promise<T[]>
  ): Promise<void> {
    const batch = this.batchQueue.get(batchKey);
    if (!batch || batch.length === 0) return;

    // Clear batch queue and timer
    this.batchQueue.delete(batchKey);
    if (this.batchTimers.has(batchKey)) {
      clearTimeout(this.batchTimers.get(batchKey)!);
      this.batchTimers.delete(batchKey);
    }

    try {
      const queries = batch.map(item => item.query);
      const results = await executeBatch(queries);
      
      // Resolve individual promises
      batch.forEach((item, index) => {
        if (results[index] !== undefined) {
          item.resolve(results[index]);
        } else {
          item.reject(new Error('No result for batch query'));
        }
      });
      
      this.emit('batch:executed', { 
        batchKey, 
        size: batch.length 
      });
    } catch (error) {
      // Reject all promises in batch
      batch.forEach(item => item.reject(error));
      this.emit('batch:error', { batchKey, error });
    }
  }

  /**
   * Optimize a dependency query for better performance
   */
  optimizeDependencyQuery(query: {
    target: string;
    depth?: number;
    includeTests?: boolean;
  }): {
    optimized: boolean;
    query: any;
    strategy: string;
  } {
    if (!this.config.enableQueryOptimization) {
      return { optimized: false, query, strategy: 'none' };
    }

    let strategy = 'standard';
    let optimizedQuery = { ...query };

    // Limit depth for performance
    if (!query.depth || query.depth > 3) {
      optimizedQuery.depth = 3;
      strategy = 'depth-limited';
    }

    // Skip tests by default for better performance
    if (query.includeTests === undefined) {
      optimizedQuery.includeTests = false;
      strategy = strategy === 'standard' ? 'skip-tests' : `${strategy}+skip-tests`;
    }

    return {
      optimized: strategy !== 'standard',
      query: optimizedQuery,
      strategy
    };
  }

  /**
   * Create a cache key from query parameters
   */
  createCacheKey(operation: string, params: any): string {
    const hash = createHash('sha256');
    hash.update(operation);
    hash.update(JSON.stringify(params));
    return hash.digest('hex');
  }

  /**
   * Prefetch data that is likely to be needed
   */
  async prefetch<T>(
    predictions: Array<{
      queryFn: () => Promise<T>;
      cacheKey: string;
      probability: number;
    }>
  ): Promise<void> {
    // Sort by probability and prefetch top candidates
    const toPrefetch = predictions
      .filter(p => p.probability > 0.5)
      .sort((a, b) => b.probability - a.probability)
      .slice(0, 5);

    await Promise.all(
      toPrefetch.map(prediction =>
        this.executeQuery(
          prediction.queryFn,
          prediction.cacheKey,
          { priority: -1 } // Low priority for prefetch
        ).catch(() => {
          // Ignore prefetch errors
        })
      )
    );

    this.emit('prefetch:complete', { 
      count: toPrefetch.length 
    });
  }

  /**
   * Compress large results for better memory usage
   */
  compressResult(data: any): any {
    if (!this.config.enableResultCompression) {
      return data;
    }

    const dataStr = JSON.stringify(data);
    if (dataStr.length < 10000) {
      return data; // Don't compress small data
    }

    // Simple compression: remove unnecessary whitespace and duplicates
    // In production, you might use a proper compression library
    return {
      compressed: true,
      data: JSON.parse(dataStr) // This would be actual compression
    };
  }

  /**
   * Decompress compressed results
   */
  decompressResult(data: any): any {
    if (!data.compressed) {
      return data;
    }

    return data.data; // This would be actual decompression
  }

  /**
   * Record response time for metrics
   */
  private recordResponseTime(time: number): void {
    this.responseTimes.push(time);
    
    // Keep only last 1000 measurements
    if (this.responseTimes.length > 1000) {
      this.responseTimes.shift();
    }

    // Update metrics
    this.updateMetrics();
  }

  /**
   * Update performance metrics
   */
  private updateMetrics(): void {
    if (this.responseTimes.length === 0) return;

    const sorted = [...this.responseTimes].sort((a, b) => a - b);
    
    this.metrics.avgResponseTime = 
      sorted.reduce((a, b) => a + b, 0) / sorted.length;
    
    this.metrics.p95ResponseTime = 
      sorted[Math.floor(sorted.length * 0.95)];
    
    this.metrics.p99ResponseTime = 
      sorted[Math.floor(sorted.length * 0.99)];
  }

  /**
   * Get current performance metrics
   */
  getMetrics(): QueryMetrics {
    return { ...this.metrics };
  }

  /**
   * Clear all caches and reset metrics
   */
  reset(): void {
    this.cache.clear();
    this.batchQueue.clear();
    this.batchTimers.forEach(timer => clearTimeout(timer));
    this.batchTimers.clear();
    this.responseTimes = [];
    
    this.metrics = {
      hits: 0,
      misses: 0,
      evictions: 0,
      avgResponseTime: 0,
      p95ResponseTime: 0,
      p99ResponseTime: 0
    };

    this.emit('optimizer:reset');
  }

  /**
   * Get cache statistics
   */
  getCacheStats(): {
    size: number;
    maxSize: number;
    hitRate: number;
    missRate: number;
  } {
    const total = this.metrics.hits + this.metrics.misses;
    return {
      size: this.cache.size,
      maxSize: this.config.cacheSize,
      hitRate: total > 0 ? this.metrics.hits / total : 0,
      missRate: total > 0 ? this.metrics.misses / total : 0
    };
  }

  /**
   * Get queue statistics
   */
  getQueueStats(): {
    size: number;
    pending: number;
    isPaused: boolean;
  } {
    return {
      size: this.queue.size,
      pending: this.queue.pending,
      isPaused: this.queue.isPaused
    };
  }

  /**
   * Pause query processing
   */
  pause(): void {
    this.queue.pause();
    this.emit('optimizer:paused');
  }

  /**
   * Resume query processing
   */
  resume(): void {
    this.queue.start();
    this.emit('optimizer:resumed');
  }
}

/**
 * Query optimization strategies
 */
export class QueryOptimizationStrategy {
  /**
   * Optimize file pattern queries
   */
  static optimizeFilePattern(pattern: string): string[] {
    // Split broad patterns into more specific ones
    if (pattern === '**/*') {
      return ['**/*.ts', '**/*.js', '**/*.tsx', '**/*.jsx'];
    }
    
    if (pattern === 'src/**/*') {
      return ['src/**/*.ts', 'src/**/*.js', 'src/**/*.tsx', 'src/**/*.jsx'];
    }
    
    return [pattern];
  }

  /**
   * Optimize dependency depth based on target type
   */
  static optimizeDependencyDepth(target: string, requestedDepth?: number): number {
    // File extensions
    const ext = target.split('.').pop();
    
    // Test files usually have fewer dependencies
    if (target.includes('.test.') || target.includes('.spec.')) {
      return Math.min(requestedDepth ?? 2, 2);
    }
    
    // Config files typically have shallow dependencies
    if (target.includes('config') || ext === 'json' || ext === 'yaml') {
      return Math.min(requestedDepth ?? 1, 1);
    }
    
    // Entry points might need deeper analysis
    if (target.includes('index') || target.includes('main') || target.includes('app')) {
      return requestedDepth ?? 4;
    }
    
    // Default depth
    return requestedDepth ?? 3;
  }

  /**
   * Determine if parallel execution would be beneficial
   */
  static shouldParallelize(queries: any[]): boolean {
    // Parallelize if we have multiple queries
    if (queries.length < 2) return false;
    
    // Don't parallelize if queries are interdependent
    const targets = queries.map(q => q.target).filter(Boolean);
    const uniqueTargets = new Set(targets);
    
    // If targets overlap significantly, sequential might be better
    if (uniqueTargets.size < targets.length * 0.7) {
      return false;
    }
    
    return true;
  }

  /**
   * Group queries for efficient batching
   */
  static groupQueries(queries: any[]): Map<string, any[]> {
    const groups = new Map<string, any[]>();
    
    for (const query of queries) {
      const key = `${query.type}-${query.depth ?? 'default'}`;
      
      if (!groups.has(key)) {
        groups.set(key, []);
      }
      
      groups.get(key)!.push(query);
    }
    
    return groups;
  }
}