// Authentication and Authorization Middleware for CodeGraph
// High-performance security layer with sub-50ms overhead

import jwt from 'jsonwebtoken';
import { GraphQLResolveInfo, GraphQLError } from 'graphql';
import { RateLimiterRedis } from 'rate-limiter-flexible';
import { createHash } from 'crypto';

// Permission types for code intelligence operations
export enum Permission {
  READ_CODE = 'read:code',
  READ_GRAPH = 'read:graph',
  READ_METRICS = 'read:metrics',
  READ_ANALYSIS = 'read:analysis',
  WRITE_ANNOTATIONS = 'write:annotations',
  MANAGE_CACHE = 'manage:cache',
  ADMIN_SYSTEM = 'admin:system',
  SUBSCRIBE_UPDATES = 'subscribe:updates'
}

// Resource types that can be protected
export enum ResourceType {
  NODE = 'node',
  RELATION = 'relation',
  SUBGRAPH = 'subgraph',
  PROJECT = 'project',
  REPOSITORY = 'repository'
}

// User context with security information
export interface AuthContext {
  userId: string;
  username: string;
  permissions: Permission[];
  roles: string[];
  organizationId?: string;
  projectAccess: string[];
  rateLimit: {
    remaining: number;
    reset: Date;
  };
  sessionId: string;
  issuedAt: Date;
  expiresAt: Date;
}

// Resource access control
export interface ResourceAccess {
  type: ResourceType;
  id: string;
  requiredPermissions: Permission[];
  organizationRestricted?: boolean;
  projectRestricted?: boolean;
}

// High-performance JWT token manager
export class TokenManager {
  private static readonly JWT_SECRET = process.env.JWT_SECRET || 'your-secret-key';
  private static readonly JWT_ALGORITHM = 'HS256';
  private static readonly TOKEN_EXPIRY = '1h';
  private static readonly REFRESH_EXPIRY = '7d';
  
  // Token cache to avoid repeated JWT verification
  private static tokenCache = new Map<string, { context: AuthContext; expires: number }>();
  private static readonly CACHE_TTL_MS = 5 * 60 * 1000; // 5 minutes

  static async verifyToken(token: string): Promise<AuthContext | null> {
    const startTime = performance.now();
    
    try {
      // Check cache first for performance
      const cached = this.tokenCache.get(token);
      if (cached && cached.expires > Date.now()) {
        const duration = performance.now() - startTime;
        if (duration > 1) console.warn(`Token cache lookup took ${duration}ms`);
        return cached.context;
      }

      // Verify JWT
      const decoded = jwt.verify(token, this.JWT_SECRET, { algorithm: this.JWT_ALGORITHM }) as any;
      
      // Build auth context
      const context: AuthContext = {
        userId: decoded.sub,
        username: decoded.username,
        permissions: decoded.permissions || [],
        roles: decoded.roles || [],
        organizationId: decoded.org,
        projectAccess: decoded.projects || [],
        rateLimit: {
          remaining: decoded.rateLimit?.remaining || 1000,
          reset: new Date(decoded.rateLimit?.reset || Date.now() + 3600000)
        },
        sessionId: decoded.sessionId,
        issuedAt: new Date(decoded.iat * 1000),
        expiresAt: new Date(decoded.exp * 1000)
      };

      // Cache the result
      this.tokenCache.set(token, {
        context,
        expires: Date.now() + this.CACHE_TTL_MS
      });

      const duration = performance.now() - startTime;
      if (duration > 5) {
        console.warn(`JWT verification took ${duration}ms`);
      }

      return context;
    } catch (error) {
      console.error('Token verification failed:', error);
      return null;
    }
  }

  static generateToken(payload: any): string {
    return jwt.sign(payload, this.JWT_SECRET, {
      algorithm: this.JWT_ALGORITHM,
      expiresIn: this.TOKEN_EXPIRY,
      issuer: 'codegraph-api',
      audience: 'codegraph-clients'
    });
  }

  static generateRefreshToken(userId: string): string {
    return jwt.sign(
      { sub: userId, type: 'refresh' },
      this.JWT_SECRET,
      {
        algorithm: this.JWT_ALGORITHM,
        expiresIn: this.REFRESH_EXPIRY,
        issuer: 'codegraph-api'
      }
    );
  }

  static clearTokenCache(): void {
    this.tokenCache.clear();
  }

  // Periodic cache cleanup
  static startCacheCleanup(): void {
    setInterval(() => {
      const now = Date.now();
      for (const [token, data] of this.tokenCache.entries()) {
        if (data.expires <= now) {
          this.tokenCache.delete(token);
        }
      }
    }, 60000); // Clean every minute
  }
}

// Rate limiting system
export class RateLimitManager {
  private limiters: Map<string, RateLimiterRedis> = new Map();
  private redisClient: any;

  constructor(redisClient?: any) {
    this.redisClient = redisClient;
    this.initializeLimiters();
  }

  private initializeLimiters(): void {
    // Standard user limits
    this.limiters.set('user', new RateLimiterRedis({
      storeClient: this.redisClient,
      keyPrefix: 'rl:user',
      points: 1000, // requests
      duration: 3600, // per hour
      blockDuration: 60, // block for 1 minute
      execEvenly: true // spread requests evenly
    }));

    // Premium user limits
    this.limiters.set('premium', new RateLimiterRedis({
      storeClient: this.redisClient,
      keyPrefix: 'rl:premium',
      points: 5000,
      duration: 3600,
      blockDuration: 30,
      execEvenly: true
    }));

    // Complex query limits (separate from standard limits)
    this.limiters.set('complex_query', new RateLimiterRedis({
      storeClient: this.redisClient,
      keyPrefix: 'rl:complex',
      points: 100,
      duration: 3600,
      blockDuration: 120,
      execEvenly: true
    }));

    // Subscription limits
    this.limiters.set('subscription', new RateLimiterRedis({
      storeClient: this.redisClient,
      keyPrefix: 'rl:sub',
      points: 50, // concurrent subscriptions
      duration: 1,
      blockDuration: 10,
      execEvenly: false
    }));
  }

  async checkRateLimit(
    userId: string, 
    userTier: string, 
    operation: string,
    complexity: number = 1
  ): Promise<{ allowed: boolean; remaining: number; reset: Date }> {
    const startTime = performance.now();
    
    try {
      const limiterKey = this.selectLimiter(userTier, operation, complexity);
      const limiter = this.limiters.get(limiterKey);
      
      if (!limiter) {
        return { allowed: true, remaining: 1000, reset: new Date(Date.now() + 3600000) };
      }

      const key = `${userId}:${operation}`;
      const points = Math.max(1, Math.floor(complexity / 10)); // Scale points by complexity
      
      const resRateLimiter = await limiter.consume(key, points);
      
      const duration = performance.now() - startTime;
      if (duration > 5) {
        console.warn(`Rate limit check took ${duration}ms`);
      }
      
      return {
        allowed: true,
        remaining: resRateLimiter.remainingPoints || 0,
        reset: new Date(Date.now() + (resRateLimiter.msBeforeNext || 0))
      };
    } catch (rateLimiterRes: any) {
      const duration = performance.now() - startTime;
      if (duration > 5) {
        console.warn(`Rate limit check took ${duration}ms`);
      }
      
      return {
        allowed: false,
        remaining: 0,
        reset: new Date(Date.now() + (rateLimiterRes.msBeforeNext || 60000))
      };
    }
  }

  private selectLimiter(userTier: string, operation: string, complexity: number): string {
    if (complexity > 200) return 'complex_query';
    if (operation.startsWith('subscribe')) return 'subscription';
    if (userTier === 'premium') return 'premium';
    return 'user';
  }
}

// Permission-based authorization
export class AuthorizationEngine {
  private static permissionHierarchy: Map<Permission, Permission[]> = new Map([
    [Permission.ADMIN_SYSTEM, [
      Permission.READ_CODE,
      Permission.READ_GRAPH,
      Permission.READ_METRICS,
      Permission.READ_ANALYSIS,
      Permission.WRITE_ANNOTATIONS,
      Permission.MANAGE_CACHE,
      Permission.SUBSCRIBE_UPDATES
    ]],
    [Permission.READ_ANALYSIS, [
      Permission.READ_CODE,
      Permission.READ_GRAPH,
      Permission.READ_METRICS
    ]],
    [Permission.READ_GRAPH, [
      Permission.READ_CODE
    ]]
  ]);

  static hasPermission(context: AuthContext, required: Permission): boolean {
    // Direct permission check
    if (context.permissions.includes(required)) {
      return true;
    }

    // Check if user has a higher-level permission that includes the required one
    for (const userPermission of context.permissions) {
      const includedPermissions = this.permissionHierarchy.get(userPermission);
      if (includedPermissions?.includes(required)) {
        return true;
      }
    }

    return false;
  }

  static async checkResourceAccess(
    context: AuthContext,
    resource: ResourceAccess
  ): Promise<boolean> {
    const startTime = performance.now();
    
    try {
      // Check permissions
      const hasRequiredPermissions = resource.requiredPermissions.every(permission =>
        this.hasPermission(context, permission)
      );

      if (!hasRequiredPermissions) {
        return false;
      }

      // Check organization access
      if (resource.organizationRestricted && resource.id.includes(':')) {
        const [orgId] = resource.id.split(':');
        if (context.organizationId !== orgId) {
          return false;
        }
      }

      // Check project access
      if (resource.projectRestricted) {
        const projectId = this.extractProjectId(resource);
        if (!context.projectAccess.includes(projectId)) {
          return false;
        }
      }

      const duration = performance.now() - startTime;
      if (duration > 2) {
        console.warn(`Resource access check took ${duration}ms`);
      }

      return true;
    } catch (error) {
      console.error('Resource access check failed:', error);
      return false;
    }
  }

  private static extractProjectId(resource: ResourceAccess): string {
    // Extract project ID from resource ID
    // Format: project:projectId:resourceId
    if (resource.id.startsWith('project:')) {
      return resource.id.split(':')[1];
    }
    return resource.id;
  }

  static filterAccessibleNodes(
    context: AuthContext,
    nodes: Array<{ id: string; projectId?: string; organizationId?: string }>
  ): Array<{ id: string; projectId?: string; organizationId?: string }> {
    return nodes.filter(node => {
      // Organization filter
      if (node.organizationId && context.organizationId !== node.organizationId) {
        return false;
      }

      // Project filter
      if (node.projectId && !context.projectAccess.includes(node.projectId)) {
        return false;
      }

      return true;
    });
  }
}

// Main authentication middleware
export class AuthMiddleware {
  private tokenManager: TokenManager;
  private rateLimitManager: RateLimitManager;
  private authEngine: AuthorizationEngine;

  constructor(redisClient?: any) {
    this.tokenManager = new TokenManager();
    this.rateLimitManager = new RateLimitManager(redisClient);
    this.authEngine = new AuthorizationEngine();
    
    // Start background processes
    TokenManager.startCacheCleanup();
  }

  // GraphQL context function
  async createContext(req: any): Promise<{ auth: AuthContext | null }> {
    const startTime = performance.now();
    
    try {
      // Extract token from request
      const token = this.extractToken(req);
      if (!token) {
        return { auth: null };
      }

      // Verify token
      const auth = await TokenManager.verifyToken(token);
      if (!auth) {
        return { auth: null };
      }

      // Check if token is expired
      if (auth.expiresAt < new Date()) {
        return { auth: null };
      }

      const duration = performance.now() - startTime;
      if (duration > 10) {
        console.warn(`Auth context creation took ${duration}ms`);
      }

      return { auth };
    } catch (error) {
      console.error('Context creation failed:', error);
      return { auth: null };
    }
  }

  // Field-level authorization directive
  authDirective = (requiredPermissions: Permission[]) => {
    return async (resolve: any, root: any, args: any, context: any, info: GraphQLResolveInfo) => {
      const startTime = performance.now();
      
      try {
        // Check authentication
        if (!context.auth) {
          throw new GraphQLError('Authentication required', {
            extensions: { code: 'UNAUTHENTICATED' }
          });
        }

        // Check permissions
        const hasPermission = requiredPermissions.every(permission =>
          AuthorizationEngine.hasPermission(context.auth, permission)
        );

        if (!hasPermission) {
          throw new GraphQLError('Insufficient permissions', {
            extensions: { code: 'FORBIDDEN' }
          });
        }

        // Check rate limits
        const complexity = this.estimateFieldComplexity(info);
        const rateLimitResult = await this.rateLimitManager.checkRateLimit(
          context.auth.userId,
          this.getUserTier(context.auth),
          info.fieldName,
          complexity
        );

        if (!rateLimitResult.allowed) {
          throw new GraphQLError('Rate limit exceeded', {
            extensions: {
              code: 'RATE_LIMITED',
              retryAfter: rateLimitResult.reset
            }
          });
        }

        // Update rate limit info in context
        context.auth.rateLimit = {
          remaining: rateLimitResult.remaining,
          reset: rateLimitResult.reset
        };

        const authDuration = performance.now() - startTime;
        if (authDuration > 5) {
          console.warn(`Auth directive took ${authDuration}ms for ${info.fieldName}`);
        }

        // Execute the resolver
        return await resolve(root, args, context, info);
      } catch (error) {
        console.error(`Auth directive error for ${info.fieldName}:`, error);
        throw error;
      }
    };
  };

  // Resource-level authorization
  async authorizeResource(
    context: AuthContext,
    resourceType: ResourceType,
    resourceId: string,
    requiredPermissions: Permission[]
  ): Promise<boolean> {
    const resource: ResourceAccess = {
      type: resourceType,
      id: resourceId,
      requiredPermissions,
      organizationRestricted: true,
      projectRestricted: resourceType === ResourceType.PROJECT
    };

    return AuthorizationEngine.checkResourceAccess(context, resource);
  }

  private extractToken(req: any): string | null {
    // Check Authorization header
    const authHeader = req.headers.authorization;
    if (authHeader && authHeader.startsWith('Bearer ')) {
      return authHeader.substring(7);
    }

    // Check cookie
    if (req.cookies?.token) {
      return req.cookies.token;
    }

    // Check WebSocket connection (for subscriptions)
    if (req.connectionParams?.authorization) {
      return req.connectionParams.authorization.replace('Bearer ', '');
    }

    return null;
  }

  private estimateFieldComplexity(info: GraphQLResolveInfo): number {
    let complexity = 1;
    
    // Base complexity for different field types
    const fieldComplexity: Record<string, number> = {
      'subgraph': 50,
      'findPath': 30,
      'dependencyGraph': 40,
      'impactAnalysis': 60,
      'searchNodes': 20,
      'codeMetrics': 25
    };

    complexity = fieldComplexity[info.fieldName] || 1;

    // Add complexity based on query depth
    const depth = this.calculateSelectionDepth(info.fieldNodes[0].selectionSet);
    complexity += depth * 5;

    return complexity;
  }

  private calculateSelectionDepth(selectionSet: any, currentDepth: number = 0): number {
    if (!selectionSet || !selectionSet.selections) {
      return currentDepth;
    }

    let maxDepth = currentDepth;
    for (const selection of selectionSet.selections) {
      if (selection.selectionSet) {
        const depth = this.calculateSelectionDepth(selection.selectionSet, currentDepth + 1);
        maxDepth = Math.max(maxDepth, depth);
      }
    }

    return maxDepth;
  }

  private getUserTier(auth: AuthContext): string {
    if (auth.roles.includes('premium')) return 'premium';
    if (auth.roles.includes('enterprise')) return 'enterprise';
    return 'standard';
  }
}

// Utility functions for common auth patterns
export const authUtils = {
  // Create auth directive for GraphQL schema
  createAuthDirective: (permissions: Permission[]) => ({
    AUTH: permissions
  }),

  // Check multiple permissions with OR logic
  hasAnyPermission: (context: AuthContext, permissions: Permission[]): boolean => {
    return permissions.some(permission => 
      AuthorizationEngine.hasPermission(context, permission)
    );
  },

  // Generate secure session ID
  generateSessionId: (): string => {
    return createHash('sha256')
      .update(`${Date.now()}-${Math.random()}-${process.pid}`)
      .digest('hex')
      .substring(0, 32);
  },

  // Validate user context
  requireAuth: (context: any): AuthContext => {
    if (!context.auth) {
      throw new GraphQLError('Authentication required', {
        extensions: { code: 'UNAUTHENTICATED' }
      });
    }
    return context.auth;
  },

  // Create secure hash for sensitive operations
  createSecureHash: (input: string, salt?: string): string => {
    const hashSalt = salt || process.env.HASH_SALT || 'default-salt';
    return createHash('sha256').update(input + hashSalt).digest('hex');
  }
};

// Export configured middleware instance
export const authMiddleware = new AuthMiddleware();