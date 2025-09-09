// Graph Traversal Query Patterns for CodeGraph
// Optimized patterns for sub-50ms performance requirements

import { GraphQLResolveInfo } from 'graphql';

// Performance-optimized graph traversal utilities
export class GraphTraversalOptimizer {
  private static readonly MAX_DEPTH = 10;
  private static readonly BATCH_SIZE = 100;
  private static readonly TIMEOUT_MS = 45; // Leave 5ms buffer for response formatting

  // Bidirectional BFS for shortest path finding
  static async findShortestPath(
    from: string,
    to: string,
    relationTypes: string[],
    maxDepth: number = this.MAX_DEPTH
  ): Promise<string[]> {
    const startTime = performance.now();
    
    if (from === to) return [from];
    
    const forwardQueue = [{ nodeId: from, path: [from], depth: 0 }];
    const backwardQueue = [{ nodeId: to, path: [to], depth: 0 }];
    const forwardVisited = new Map<string, string[]>();
    const backwardVisited = new Map<string, string[]>();
    
    forwardVisited.set(from, [from]);
    backwardVisited.set(to, [to]);
    
    while (forwardQueue.length > 0 && backwardQueue.length > 0) {
      // Check timeout
      if (performance.now() - startTime > this.TIMEOUT_MS) {
        throw new Error('Path finding timed out');
      }
      
      // Alternate between forward and backward search
      const result = await this.expandSearchFront(
        forwardQueue, forwardVisited, backwardVisited, 
        relationTypes, maxDepth, true
      );
      
      if (result) return result;
      
      const backResult = await this.expandSearchFront(
        backwardQueue, backwardVisited, forwardVisited,
        relationTypes, maxDepth, false
      );
      
      if (backResult) return backResult;
    }
    
    return []; // No path found
  }

  private static async expandSearchFront(
    queue: Array<{nodeId: string, path: string[], depth: number}>,
    visited: Map<string, string[]>,
    otherVisited: Map<string, string[]>,
    relationTypes: string[],
    maxDepth: number,
    isForward: boolean
  ): Promise<string[] | null> {
    if (queue.length === 0) return null;
    
    const current = queue.shift()!;
    if (current.depth >= maxDepth) return null;
    
    // Check if we've met the other search
    if (otherVisited.has(current.nodeId)) {
      const otherPath = otherVisited.get(current.nodeId)!;
      return isForward 
        ? [...current.path, ...otherPath.slice(1).reverse()]
        : [...otherPath.reverse(), ...current.path.slice(1)];
    }
    
    // Get neighbors using batch query for performance
    const neighbors = await this.getBatchNeighbors([current.nodeId], relationTypes, isForward);
    
    for (const neighborId of neighbors.get(current.nodeId) || []) {
      if (!visited.has(neighborId)) {
        const newPath = [...current.path, neighborId];
        visited.set(neighborId, newPath);
        queue.push({
          nodeId: neighborId,
          path: newPath,
          depth: current.depth + 1
        });
      }
    }
    
    return null;
  }

  // Optimized batch neighbor fetching
  private static async getBatchNeighbors(
    nodeIds: string[],
    relationTypes: string[],
    outbound: boolean = true
  ): Promise<Map<string, string[]>> {
    // This would integrate with your data layer
    // Implementation would use database batch queries, connection pooling, etc.
    const result = new Map<string, string[]>();
    
    // Batch query to minimize database roundtrips
    const batchQueries = [];
    for (let i = 0; i < nodeIds.length; i += this.BATCH_SIZE) {
      const batch = nodeIds.slice(i, i + this.BATCH_SIZE);
      batchQueries.push(this.fetchNeighborsBatch(batch, relationTypes, outbound));
    }
    
    const batchResults = await Promise.all(batchQueries);
    batchResults.forEach(batchResult => {
      batchResult.forEach((neighbors, nodeId) => {
        result.set(nodeId, neighbors);
      });
    });
    
    return result;
  }

  private static async fetchNeighborsBatch(
    nodeIds: string[],
    relationTypes: string[],
    outbound: boolean
  ): Promise<Map<string, string[]>> {
    // Database implementation would go here
    // This is a placeholder for the actual database query
    return new Map();
  }
}

// Graph query pattern implementations
export const graphTraversalPatterns = {
  
  // Pattern: Deep dependency analysis with cycle detection
  async findDependencyCycles(rootId: string, maxDepth: number = 5): Promise<string[][]> {
    const visited = new Set<string>();
    const recursionStack = new Set<string>();
    const cycles: string[][] = [];
    const currentPath: string[] = [];
    
    await this.dfsDetectCycles(rootId, visited, recursionStack, currentPath, cycles, maxDepth);
    return cycles;
  },

  async dfsDetectCycles(
    nodeId: string,
    visited: Set<string>,
    recursionStack: Set<string>,
    currentPath: string[],
    cycles: string[][],
    maxDepth: number
  ): Promise<void> {
    if (currentPath.length >= maxDepth) return;
    
    visited.add(nodeId);
    recursionStack.add(nodeId);
    currentPath.push(nodeId);
    
    const dependencies = await GraphTraversalOptimizer.getBatchNeighbors(
      [nodeId], ['DEPENDS_ON'], true
    );
    
    for (const depId of dependencies.get(nodeId) || []) {
      if (recursionStack.has(depId)) {
        // Found cycle
        const cycleStart = currentPath.indexOf(depId);
        cycles.push([...currentPath.slice(cycleStart), depId]);
      } else if (!visited.has(depId)) {
        await this.dfsDetectCycles(depId, visited, recursionStack, currentPath, cycles, maxDepth);
      }
    }
    
    recursionStack.delete(nodeId);
    currentPath.pop();
  },

  // Pattern: Multi-hop relationship queries with pruning
  async findMultiHopRelationships(
    startNodes: string[],
    targetTypes: string[],
    relationChain: string[],
    maxResults: number = 100
  ): Promise<Array<{path: string[], confidence: number}>> {
    const results: Array<{path: string[], confidence: number}> = [];
    const startTime = performance.now();
    
    for (const startNode of startNodes) {
      if (performance.now() - startTime > GraphTraversalOptimizer['TIMEOUT_MS']) break;
      
      const paths = await this.exploreRelationChain(
        startNode, targetTypes, relationChain, [], 0.0
      );
      
      results.push(...paths);
      if (results.length >= maxResults) break;
    }
    
    return results
      .sort((a, b) => b.confidence - a.confidence)
      .slice(0, maxResults);
  },

  async exploreRelationChain(
    currentNode: string,
    targetTypes: string[],
    relationChain: string[],
    currentPath: string[],
    baseConfidence: number
  ): Promise<Array<{path: string[], confidence: number}>> {
    if (relationChain.length === 0) {
      // Check if current node matches target types
      const nodeType = await this.getNodeType(currentNode);
      if (targetTypes.includes(nodeType)) {
        return [{
          path: [...currentPath, currentNode],
          confidence: baseConfidence + 1.0
        }];
      }
      return [];
    }
    
    const results: Array<{path: string[], confidence: number}> = [];
    const [nextRelation, ...remainingChain] = relationChain;
    
    const neighbors = await GraphTraversalOptimizer.getBatchNeighbors(
      [currentNode], [nextRelation], true
    );
    
    for (const neighbor of neighbors.get(currentNode) || []) {
      if (!currentPath.includes(neighbor)) { // Avoid cycles
        const subResults = await this.exploreRelationChain(
          neighbor,
          targetTypes,
          remainingChain,
          [...currentPath, currentNode],
          baseConfidence + 0.8 // Decay confidence with distance
        );
        results.push(...subResults);
      }
    }
    
    return results;
  },

  async getNodeType(nodeId: string): Promise<string> {
    // Implementation would fetch node type from database
    return 'FUNCTION'; // Placeholder
  },

  // Pattern: Subgraph extraction with relevance scoring
  async extractRelevantSubgraph(
    seedNodes: string[],
    targetSize: number,
    relevanceWeights: Record<string, number>
  ): Promise<{nodes: string[], relations: Array<{from: string, to: string, type: string}>}> {
    const subgraph = new Set<string>(seedNodes);
    const relations: Array<{from: string, to: string, type: string}> = [];
    const candidateNodes = new Map<string, number>(); // nodeId -> relevance score
    
    // Start with seed nodes
    for (const seed of seedNodes) {
      const neighbors = await GraphTraversalOptimizer.getBatchNeighbors([seed], [], true);
      for (const neighbor of neighbors.get(seed) || []) {
        candidateNodes.set(neighbor, this.calculateRelevanceScore(neighbor, relevanceWeights));
      }
    }
    
    // Greedily add most relevant nodes
    while (subgraph.size < targetSize && candidateNodes.size > 0) {
      const bestCandidate = [...candidateNodes.entries()]
        .reduce((best, current) => current[1] > best[1] ? current : best);
      
      const nodeId = bestCandidate[0];
      subgraph.add(nodeId);
      candidateNodes.delete(nodeId);
      
      // Add new candidates from this node's neighbors
      const newNeighbors = await GraphTraversalOptimizer.getBatchNeighbors([nodeId], [], true);
      for (const neighbor of newNeighbors.get(nodeId) || []) {
        if (!subgraph.has(neighbor) && !candidateNodes.has(neighbor)) {
          candidateNodes.set(neighbor, this.calculateRelevanceScore(neighbor, relevanceWeights));
        }
      }
    }
    
    // Extract relations between selected nodes
    for (const node of subgraph) {
      const neighbors = await GraphTraversalOptimizer.getBatchNeighbors([node], [], true);
      for (const neighbor of neighbors.get(node) || []) {
        if (subgraph.has(neighbor)) {
          relations.push({from: node, to: neighbor, type: 'UNKNOWN'}); // Would get actual type
        }
      }
    }
    
    return {
      nodes: Array.from(subgraph),
      relations
    };
  },

  calculateRelevanceScore(nodeId: string, weights: Record<string, number>): number {
    // Implementation would calculate based on node properties and relationships
    return Math.random(); // Placeholder
  },

  // Pattern: Centrality-based node ranking
  async calculateCentralityScores(
    nodeIds: string[],
    algorithm: 'betweenness' | 'closeness' | 'pagerank' = 'pagerank'
  ): Promise<Record<string, number>> {
    switch (algorithm) {
      case 'pagerank':
        return this.calculatePageRank(nodeIds);
      case 'betweenness':
        return this.calculateBetweennessCentrality(nodeIds);
      case 'closeness':
        return this.calculateClosenessCentrality(nodeIds);
      default:
        throw new Error(`Unknown centrality algorithm: ${algorithm}`);
    }
  },

  async calculatePageRank(
    nodeIds: string[],
    dampingFactor: number = 0.85,
    iterations: number = 100,
    tolerance: number = 1e-6
  ): Promise<Record<string, number>> {
    const scores: Record<string, number> = {};
    const newScores: Record<string, number> = {};
    
    // Initialize scores
    const initialScore = 1.0 / nodeIds.length;
    nodeIds.forEach(id => scores[id] = initialScore);
    
    for (let i = 0; i < iterations; i++) {
      // Reset new scores
      nodeIds.forEach(id => newScores[id] = (1 - dampingFactor) / nodeIds.length);
      
      // Calculate new scores
      for (const nodeId of nodeIds) {
        const outgoingLinks = await GraphTraversalOptimizer.getBatchNeighbors([nodeId], [], true);
        const linkCount = outgoingLinks.get(nodeId)?.length || 1;
        const contribution = dampingFactor * scores[nodeId] / linkCount;
        
        for (const linkedNode of outgoingLinks.get(nodeId) || []) {
          if (newScores[linkedNode] !== undefined) {
            newScores[linkedNode] += contribution;
          }
        }
      }
      
      // Check convergence
      const maxDiff = Math.max(...nodeIds.map(id => Math.abs(newScores[id] - scores[id])));
      if (maxDiff < tolerance) break;
      
      // Update scores
      Object.assign(scores, newScores);
    }
    
    return scores;
  },

  async calculateBetweennessCentrality(nodeIds: string[]): Promise<Record<string, number>> {
    const centrality: Record<string, number> = {};
    nodeIds.forEach(id => centrality[id] = 0);
    
    // For each pair of nodes, find shortest paths and accumulate betweenness
    for (let i = 0; i < nodeIds.length; i++) {
      for (let j = i + 1; j < nodeIds.length; j++) {
        const paths = await this.findAllShortestPaths(nodeIds[i], nodeIds[j]);
        
        if (paths.length > 0) {
          const pathWeight = 1.0 / paths.length;
          
          paths.forEach(path => {
            // Skip source and target nodes
            for (let k = 1; k < path.length - 1; k++) {
              centrality[path[k]] += pathWeight;
            }
          });
        }
      }
    }
    
    // Normalize by the number of pairs
    const normalizationFactor = (nodeIds.length - 1) * (nodeIds.length - 2) / 2;
    Object.keys(centrality).forEach(id => {
      centrality[id] /= normalizationFactor;
    });
    
    return centrality;
  },

  async calculateClosenessCentrality(nodeIds: string[]): Promise<Record<string, number>> {
    const centrality: Record<string, number> = {};
    
    for (const nodeId of nodeIds) {
      let totalDistance = 0;
      let reachableNodes = 0;
      
      for (const otherId of nodeIds) {
        if (nodeId !== otherId) {
          const shortestPath = await GraphTraversalOptimizer.findShortestPath(
            nodeId, otherId, [], 10
          );
          
          if (shortestPath.length > 0) {
            totalDistance += shortestPath.length - 1;
            reachableNodes++;
          }
        }
      }
      
      centrality[nodeId] = reachableNodes > 0 ? reachableNodes / totalDistance : 0;
    }
    
    return centrality;
  },

  async findAllShortestPaths(from: string, to: string): Promise<string[][]> {
    // Implementation would find all shortest paths, not just one
    const singlePath = await GraphTraversalOptimizer.findShortestPath(from, to, []);
    return singlePath.length > 0 ? [singlePath] : [];
  }
};

// Query optimization helpers
export class QueryOptimizer {
  static estimateComplexity(
    operation: string,
    depth: number,
    nodeCount: number,
    relationCount: number
  ): number {
    const baseComplexity = {
      'findPath': 10,
      'subgraph': 50,
      'dependencyAnalysis': 100,
      'impactAnalysis': 150
    };
    
    const complexity = (baseComplexity[operation] || 20) * Math.pow(depth, 2) * Math.log(nodeCount + 1);
    return Math.round(complexity);
  }
  
  static selectOptimalAlgorithm(
    queryType: string,
    graphSize: number,
    maxDepth: number
  ): string {
    if (queryType === 'shortestPath') {
      if (graphSize < 1000) return 'BFS';
      if (maxDepth <= 3) return 'BIDIRECTIONAL_BFS';
      return 'A_STAR';
    }
    
    if (queryType === 'subgraph') {
      return graphSize < 10000 ? 'DFS' : 'BFS';
    }
    
    return 'BFS'; // Default
  }
}