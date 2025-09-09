// Query Optimization and Caching Strategies for CodeGraph
// Multi-tier caching system designed for sub-50ms response times

import { LRUCache } from 'lru-cache';
import { createHash } from 'crypto';
import { GraphQLResolveInfo, parse, validate, execute } from 'graphql';

// Cache configuration constants
const CACHE_CONFIG = {
  L1_MAX_SIZE: 1000,           // In-memory cache size
  L1_TTL_MS: 5 * 60 * 1000,    // 5 minutes
  L2_MAX_SIZE: 10000,          // Redis cache size
  L2_TTL_MS: 60 * 60 * 1000,   // 1 hour
  L3_TTL_MS: 24 * 60 * 60 * 1000, // 24 hours (persistent)
  QUERY_COMPLEXITY_THRESHOLD: 100,
  PRELOAD_BATCH_SIZE: 50
};

// Cache key generation utilities
export class CacheKeyGenerator {
  static generateQueryKey(query: string, variables: any, context: any): string {
    const normalized = this.normalizeQuery(query);
    const varsHash = this.hashObject(variables || {});
    const contextHash = this.hashObject({
      userId: context.userId,
      permissions: context.permissions,
      timestamp: Math.floor(Date.now() / (5 * 60 * 1000)) // 5-minute buckets
    });
    
    return `query:${this.hashString(normalized)}:${varsHash}:${contextHash}`;
  }

  static generateNodeKey(nodeId: string, fields: string[]): string {
    const fieldsHash = this.hashObject(fields.sort());
    return `node:${nodeId}:${fieldsHash}`;
  }

  static generateRelationKey(sourceId: string, targetId: string, relationType: string): string {
    return `relation:${sourceId}:${targetId}:${relationType}`;
  }

  static generateTraversalKey(
    startNodes: string[], 
    depth: number, 
    relationTypes: string[], 
    filter: any
  ): string {
    const startHash = this.hashObject(startNodes.sort());
    const filterHash = this.hashObject(filter || {});
    return `traversal:${startHash}:${depth}:${relationTypes.join(',')}:${filterHash}`;
  }

  private static normalizeQuery(query: string): string {
    // Remove comments and normalize whitespace
    return query
      .replace(/\s+/g, ' ')
      .replace(/#.*$/gm, '')
      .trim();
  }

  private static hashString(str: string): string {
    return createHash('sha256').update(str).digest('hex').substring(0, 16);
  }

  private static hashObject(obj: any): string {
    return this.hashString(JSON.stringify(obj));
  }
}

// Multi-tier cache implementation
export class MultiTierCache {
  private l1Cache: LRUCache<string, any>; // In-memory
  private l2Cache: any; // Redis client
  private l3Cache: any; // Persistent storage
  private hitStats: Map<string, { hits: number; misses: number }> = new Map();

  constructor(redisClient?: any, persistentStorage?: any) {
    this.l1Cache = new LRUCache({
      max: CACHE_CONFIG.L1_MAX_SIZE,
      ttl: CACHE_CONFIG.L1_TTL_MS,
      updateAgeOnGet: true,
      updateAgeOnHas: true
    });

    this.l2Cache = redisClient;
    this.l3Cache = persistentStorage;
  }

  async get(key: string, tier: 'L1' | 'L2' | 'L3' | 'ALL' = 'ALL'): Promise<any> {
    const startTime = performance.now();
    
    try {
      // L1 Cache (In-memory)
      if (tier === 'L1' || tier === 'ALL') {
        const l1Value = this.l1Cache.get(key);
        if (l1Value !== undefined) {
          this.recordHit(key, 'L1', performance.now() - startTime);
          return l1Value;
        }
      }

      // L2 Cache (Redis)
      if ((tier === 'L2' || tier === 'ALL') && this.l2Cache) {
        const l2Value = await this.l2Cache.get(key);
        if (l2Value !== null) {
          const parsed = JSON.parse(l2Value);
          
          // Promote to L1
          this.l1Cache.set(key, parsed);
          
          this.recordHit(key, 'L2', performance.now() - startTime);
          return parsed;
        }
      }

      // L3 Cache (Persistent)
      if ((tier === 'L3' || tier === 'ALL') && this.l3Cache) {
        const l3Value = await this.l3Cache.get(key);
        if (l3Value !== null) {
          // Promote to higher tiers
          this.l1Cache.set(key, l3Value);
          if (this.l2Cache) {
            await this.l2Cache.setex(key, CACHE_CONFIG.L2_TTL_MS / 1000, JSON.stringify(l3Value));
          }
          
          this.recordHit(key, 'L3', performance.now() - startTime);
          return l3Value;
        }
      }

      this.recordMiss(key, performance.now() - startTime);
      return null;
    } catch (error) {
      console.error(`Cache get error for key ${key}:`, error);
      return null;
    }
  }

  async set(key: string, value: any, policy: 'SHORT_TERM' | 'MEDIUM_TERM' | 'LONG_TERM' | 'PERSISTENT' = 'MEDIUM_TERM'): Promise<void> {
    const startTime = performance.now();
    
    try {
      // Always set in L1
      this.l1Cache.set(key, value);
      
      // Set in appropriate tiers based on policy
      switch (policy) {
        case 'SHORT_TERM':
          // L1 only (already set)
          break;
          
        case 'MEDIUM_TERM':
          if (this.l2Cache) {
            await this.l2Cache.setex(key, CACHE_CONFIG.L2_TTL_MS / 1000, JSON.stringify(value));
          }
          break;
          
        case 'LONG_TERM':
          if (this.l2Cache) {
            await this.l2Cache.setex(key, CACHE_CONFIG.L2_TTL_MS / 1000, JSON.stringify(value));
          }
          if (this.l3Cache) {
            await this.l3Cache.set(key, value, CACHE_CONFIG.L3_TTL_MS);
          }
          break;
          
        case 'PERSISTENT':
          if (this.l2Cache) {
            await this.l2Cache.set(key, JSON.stringify(value));
          }
          if (this.l3Cache) {
            await this.l3Cache.setPersistent(key, value);
          }
          break;
      }
      
      const duration = performance.now() - startTime;
      if (duration > 10) {
        console.warn(`Slow cache set: ${duration}ms for key ${key}`);
      }
    } catch (error) {
      console.error(`Cache set error for key ${key}:`, error);
    }
  }

  async invalidate(pattern: string): Promise<number> {
    let invalidatedCount = 0;
    
    try {
      // L1 Cache invalidation
      const l1Keys = Array.from(this.l1Cache.keys());
      const l1Matches = l1Keys.filter(key => this.matchesPattern(key, pattern));
      l1Matches.forEach(key => {
        this.l1Cache.delete(key);
        invalidatedCount++;
      });
      
      // L2 Cache invalidation
      if (this.l2Cache) {
        const l2Keys = await this.l2Cache.keys(pattern);
        if (l2Keys.length > 0) {
          await this.l2Cache.del(...l2Keys);
          invalidatedCount += l2Keys.length;
        }
      }
      
      // L3 Cache invalidation
      if (this.l3Cache) {
        const l3Count = await this.l3Cache.invalidatePattern(pattern);
        invalidatedCount += l3Count;
      }
      
      return invalidatedCount;
    } catch (error) {
      console.error(`Cache invalidation error for pattern ${pattern}:`, error);
      return 0;
    }
  }

  private matchesPattern(key: string, pattern: string): boolean {
    const regex = new RegExp(pattern.replace(/\*/g, '.*'));
    return regex.test(key);
  }

  private recordHit(key: string, tier: string, duration: number): void {
    const cacheType = key.split(':')[0];
    if (!this.hitStats.has(cacheType)) {
      this.hitStats.set(cacheType, { hits: 0, misses: 0 });
    }
    this.hitStats.get(cacheType)!.hits++;
  }

  private recordMiss(key: string, duration: number): void {
    const cacheType = key.split(':')[0];
    if (!this.hitStats.has(cacheType)) {
      this.hitStats.set(cacheType, { hits: 0, misses: 0 });
    }
    this.hitStats.get(cacheType)!.misses++;
  }

  getCacheStats() {
    const stats: any = {
      l1: {
        size: this.l1Cache.size,
        maxSize: this.l1Cache.max,
        hitRatio: this.l1Cache.calculatedSize
      },
      hitRatios: {}
    };

    for (const [type, data] of this.hitStats.entries()) {
      const total = data.hits + data.misses;
      stats.hitRatios[type] = total > 0 ? data.hits / total : 0;
    }

    return stats;
  }
}

// Query optimization engine
export class QueryOptimizer {
  private cache: MultiTierCache;
  private queryComplexityCache: Map<string, number> = new Map();
  private optimizationRules: OptimizationRule[] = [];

  constructor(cache: MultiTierCache) {
    this.cache = cache;
    this.initializeOptimizationRules();
  }

  async optimizeQuery(
    query: string, 
    variables: any, 
    context: any
  ): Promise<{ query: string; variables: any; cacheKey: string; complexity: number }> {
    const startTime = performance.now();
    
    try {
      // Parse query for analysis
      const ast = parse(query);
      
      // Calculate or retrieve complexity score
      const originalComplexity = await this.calculateComplexity(ast, variables);
      
      // Apply optimization rules
      let optimizedQuery = query;
      let optimizedVariables = { ...variables };
      
      for (const rule of this.optimizationRules) {
        if (rule.condition(ast, variables, context)) {
          const result = rule.optimize(optimizedQuery, optimizedVariables, context);
          optimizedQuery = result.query;
          optimizedVariables = result.variables;
        }
      }
      
      // Generate cache key
      const cacheKey = CacheKeyGenerator.generateQueryKey(optimizedQuery, optimizedVariables, context);
      
      // Calculate optimized complexity
      const optimizedComplexity = optimizedQuery !== query 
        ? await this.calculateComplexity(parse(optimizedQuery), optimizedVariables)
        : originalComplexity;
      
      const duration = performance.now() - startTime;
      if (duration > 5) {
        console.warn(`Query optimization took ${duration}ms`);
      }
      
      return {
        query: optimizedQuery,
        variables: optimizedVariables,
        cacheKey,
        complexity: optimizedComplexity
      };
    } catch (error) {
      console.error('Query optimization failed:', error);
      return {
        query,
        variables,
        cacheKey: CacheKeyGenerator.generateQueryKey(query, variables, context),
        complexity: 999 // High complexity for failed optimization
      };
    }
  }

  private async calculateComplexity(ast: any, variables: any): Promise<number> {
    const queryHash = createHash('sha256').update(JSON.stringify(ast)).digest('hex');
    
    if (this.queryComplexityCache.has(queryHash)) {
      return this.queryComplexityCache.get(queryHash)!;
    }
    
    let complexity = 0;
    
    // Basic complexity calculation based on query structure
    const visitor = {
      Field: {
        enter: (node: any) => {
          complexity += 1;
          
          // Higher complexity for nested fields
          if (node.selectionSet) {
            complexity += node.selectionSet.selections.length;
          }
          
          // Higher complexity for certain field types
          if (['subgraph', 'findPath', 'dependencyGraph'].includes(node.name.value)) {
            complexity += 50;
          }
          
          if (['impactAnalysis', 'codeMetrics'].includes(node.name.value)) {
            complexity += 100;
          }
        }
      },
      
      Argument: {
        enter: (node: any) => {
          if (node.name.value === 'depth' && node.value.value > 3) {
            complexity += (node.value.value - 3) * 20;
          }
        }
      }
    };
    
    // Would implement actual AST visitor here
    // For now, using simplified calculation
    complexity = Math.min(complexity, 1000); // Cap at 1000
    
    this.queryComplexityCache.set(queryHash, complexity);
    return complexity;
  }

  private initializeOptimizationRules(): void {
    // Rule: Add pagination to large result sets
    this.optimizationRules.push({
      name: 'auto_pagination',
      condition: (ast, vars, ctx) => {
        // Check if query lacks pagination but requests large datasets
        return this.queryLacksLimits(ast) && this.estimateResultSize(ast, vars) > 1000;
      },
      optimize: (query, vars, ctx) => {
        return {
          query: this.addPaginationToQuery(query),
          variables: { ...vars, limit: 100, offset: 0 }
        };
      }
    });

    // Rule: Optimize depth for graph traversal
    this.optimizationRules.push({
      name: 'depth_optimization',
      condition: (ast, vars, ctx) => {
        return this.hasExcessiveDepth(ast, vars);
      },
      optimize: (query, vars, ctx) => {
        const optimizedVars = { ...vars };
        if (optimizedVars.maxDepth > 5) {
          optimizedVars.maxDepth = 5;
        }
        return { query, variables: optimizedVars };
      }
    });

    // Rule: Preload related data
    this.optimizationRules.push({
      name: 'preload_optimization',
      condition: (ast, vars, ctx) => {
        return this.canBenefitFromPreloading(ast, vars, ctx);
      },
      optimize: (query, vars, ctx) => {
        // Add preload hints to query
        const optimizedQuery = this.addPreloadHints(query);
        return { query: optimizedQuery, variables: vars };
      }
    });
  }

  private queryLacksLimits(ast: any): boolean {
    // Implementation would check if query has limit/pagination
    return false; // Simplified
  }

  private estimateResultSize(ast: any, vars: any): number {
    // Implementation would estimate result set size
    return 100; // Simplified
  }

  private hasExcessiveDepth(ast: any, vars: any): boolean {
    return vars.maxDepth > 5 || vars.depth > 5;
  }

  private canBenefitFromPreloading(ast: any, vars: any, ctx: any): boolean {
    // Check if query pattern suggests preloading would help
    return false; // Simplified
  }

  private addPaginationToQuery(query: string): string {
    // Implementation would modify query to add pagination
    return query;
  }

  private addPreloadHints(query: string): string {
    // Implementation would add preload directives
    return query;
  }
}

interface OptimizationRule {
  name: string;
  condition: (ast: any, variables: any, context: any) => boolean;
  optimize: (query: string, variables: any, context: any) => { query: string; variables: any };
}

// Cache warming and preloading system
export class CacheWarmupManager {
  private cache: MultiTierCache;
  private warmupQueue: WarmupTask[] = [];
  private isWarming = false;

  constructor(cache: MultiTierCache) {
    this.cache = cache;
    this.schedulePeriodicWarmup();
  }

  async warmupCache(patterns: CacheWarmupPattern[]): Promise<CacheWarmupResult> {
    const startTime = performance.now();
    let patternsWarmed = 0;
    let itemsCached = 0;

    for (const pattern of patterns) {
      try {
        const items = await this.generateWarmupItems(pattern);
        
        const batchPromises = [];
        for (let i = 0; i < items.length; i += CACHE_CONFIG.PRELOAD_BATCH_SIZE) {
          const batch = items.slice(i, i + CACHE_CONFIG.PRELOAD_BATCH_SIZE);
          batchPromises.push(this.warmupBatch(batch));
        }
        
        const batchResults = await Promise.all(batchPromises);
        itemsCached += batchResults.reduce((sum, count) => sum + count, 0);
        patternsWarmed++;
      } catch (error) {
        console.error(`Warmup failed for pattern:`, pattern, error);
      }
    }

    const timeSpent = performance.now() - startTime;
    
    return {
      success: patternsWarmed > 0,
      patternsWarmed,
      itemsCached,
      timeSpent
    };
  }

  private async generateWarmupItems(pattern: CacheWarmupPattern): Promise<WarmupItem[]> {
    const items: WarmupItem[] = [];
    
    // Generate cache keys based on pattern
    // This is simplified - real implementation would query the graph
    for (const nodeType of pattern.nodeTypes) {
      for (const relationType of pattern.relationTypes || []) {
        items.push({
          key: `warmup:${nodeType}:${relationType}:${pattern.depth}`,
          query: this.generateWarmupQuery(nodeType, relationType, pattern),
          priority: pattern.priority || 1
        });
      }
    }
    
    return items.sort((a, b) => b.priority - a.priority);
  }

  private generateWarmupQuery(nodeType: string, relationType: string, pattern: CacheWarmupPattern): string {
    return `
      query WarmupQuery($nodeType: NodeType!, $depth: Int!) {
        searchNodes(types: [$nodeType], limit: 50) {
          nodes {
            id
            neighbors(depth: $depth, types: [${relationType}]) {
              target {
                id
                type
              }
            }
          }
        }
      }
    `;
  }

  private async warmupBatch(items: WarmupItem[]): Promise<number> {
    let cached = 0;
    
    const promises = items.map(async (item) => {
      try {
        // Execute query and cache result
        const result = await this.executeWarmupQuery(item.query);
        await this.cache.set(item.key, result, 'MEDIUM_TERM');
        return 1;
      } catch (error) {
        console.error(`Warmup item failed:`, item.key, error);
        return 0;
      }
    });
    
    const results = await Promise.all(promises);
    return results.reduce((sum, count) => sum + count, 0);
  }

  private async executeWarmupQuery(query: string): Promise<any> {
    // This would execute against your GraphQL engine
    return {}; // Placeholder
  }

  private schedulePeriodicWarmup(): void {
    setInterval(async () => {
      if (!this.isWarming && this.warmupQueue.length > 0) {
        this.isWarming = true;
        try {
          const task = this.warmupQueue.shift()!;
          await this.executeWarmupTask(task);
        } finally {
          this.isWarming = false;
        }
      }
    }, 30000); // Every 30 seconds
  }

  private async executeWarmupTask(task: WarmupTask): Promise<void> {
    // Implementation for scheduled warmup tasks
  }
}

interface CacheWarmupPattern {
  nodeTypes: string[];
  relationTypes?: string[];
  depth: number;
  priority?: number;
}

interface CacheWarmupResult {
  success: boolean;
  patternsWarmed: number;
  itemsCached: number;
  timeSpent: number;
}

interface WarmupItem {
  key: string;
  query: string;
  priority: number;
}

interface WarmupTask {
  patterns: CacheWarmupPattern[];
  scheduledAt: Date;
  priority: number;
}

// Export the complete caching system
export class CodeGraphCacheSystem {
  private cache: MultiTierCache;
  private optimizer: QueryOptimizer;
  private warmupManager: CacheWarmupManager;

  constructor(redisClient?: any, persistentStorage?: any) {
    this.cache = new MultiTierCache(redisClient, persistentStorage);
    this.optimizer = new QueryOptimizer(this.cache);
    this.warmupManager = new CacheWarmupManager(this.cache);
  }

  async executeOptimizedQuery(
    query: string, 
    variables: any, 
    context: any, 
    info: GraphQLResolveInfo
  ): Promise<any> {
    const startTime = performance.now();
    
    try {
      // Optimize the query
      const optimized = await this.optimizer.optimizeQuery(query, variables, context);
      
      // Check cache first
      const cached = await this.cache.get(optimized.cacheKey);
      if (cached !== null) {
        const duration = performance.now() - startTime;
        console.log(`Cache hit: ${duration}ms`);
        return cached;
      }
      
      // Execute query (this would integrate with your GraphQL executor)
      const result = await this.executeQuery(optimized.query, optimized.variables, context);
      
      // Determine cache policy based on query complexity
      const cachePolicy = this.determineCachePolicy(optimized.complexity);
      
      // Cache the result
      await this.cache.set(optimized.cacheKey, result, cachePolicy);
      
      const duration = performance.now() - startTime;
      console.log(`Query executed: ${duration}ms`);
      
      return result;
    } catch (error) {
      console.error('Optimized query execution failed:', error);
      throw error;
    }
  }

  private determineCachePolicy(complexity: number): 'SHORT_TERM' | 'MEDIUM_TERM' | 'LONG_TERM' | 'PERSISTENT' {
    if (complexity > 500) return 'LONG_TERM';
    if (complexity > 200) return 'MEDIUM_TERM';
    return 'SHORT_TERM';
  }

  private async executeQuery(query: string, variables: any, context: any): Promise<any> {
    // This would integrate with your actual GraphQL execution engine
    return {}; // Placeholder
  }

  getCache(): MultiTierCache {
    return this.cache;
  }

  getOptimizer(): QueryOptimizer {
    return this.optimizer;
  }

  getWarmupManager(): CacheWarmupManager {
    return this.warmupManager;
  }
}