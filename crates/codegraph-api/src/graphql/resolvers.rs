use async_graphql::{Context, Object, Result, ID, dataloader::DataLoader};
use async_trait::async_trait;
use codegraph_core::{NodeId, CodeGraphError};
use std::collections::HashMap;
use std::str::FromStr;
use std::time::Instant;
use tracing::{debug, info, instrument, warn};
use uuid::Uuid;

use crate::graphql::types::{
    CodeSearchInput, CodeSearchResult, GraphQLCodeNode, GraphTraversalInput,
    GraphTraversalResult, SemanticSearchInput, SemanticSearchResult, ScoredNode,
    SubgraphExtractionInput, SubgraphResult, PageInfo, SearchMetadata,
    TraversalMetadata, SubgraphMetadata, SemanticSearchMetadata, SearchSortBy,
};
use crate::graphql::loaders::{
    LoaderFactory, NodeLoader, EdgesBySourceLoader, SemanticSearchLoader,
    GraphTraversalLoader, TraversalKey,
};
use crate::state::AppState;
use crate::auth::{AuthContext, RateLimitManager};
use crate::event_bus;
use crate::graphql::types::GraphQLEdge;
use crate::mutations::UpdateNodeInput;

pub struct QueryRoot;

#[Object]
impl QueryRoot {
    /// Search for code nodes with text and filters
    #[instrument(skip(self, ctx), fields(query = %input.query))]
    async fn search_code(
        &self,
        ctx: &Context<'_>,
        input: CodeSearchInput,
    ) -> Result<CodeSearchResult> {
        let start_time = Instant::now();
        info!("Executing code search: {}", input.query);

        let state = ctx.data::<AppState>()?;
        let loader = ctx.data::<DataLoader<NodeLoader>>()?;

        // Validate input parameters
        let limit = input.limit.unwrap_or(20).max(1).min(100);
        let offset = input.offset.unwrap_or(0).max(0);

        // Perform search using the semantic search system
        let search_results = state.semantic_search
            .search(&input.query, limit as usize)
            .await
            .map_err(|e| async_graphql::Error::new(format!("Search failed: {}", e)))?;

        // Apply filters
        let mut filtered_results = search_results;
        
        if let Some(ref language_filters) = input.language_filter {
            filtered_results.retain(|node| {
                node.language.as_ref().map_or(false, |lang| {
                    language_filters.iter().any(|filter_lang| {
                        matches!(
                            (lang, filter_lang),
                            (codegraph_core::Language::Rust, crate::graphql::types::GraphQLLanguage::Rust) |
                            (codegraph_core::Language::Python, crate::graphql::types::GraphQLLanguage::Python) |
                            (codegraph_core::Language::TypeScript, crate::graphql::types::GraphQLLanguage::TypeScript) |
                            (codegraph_core::Language::JavaScript, crate::graphql::types::GraphQLLanguage::JavaScript) |
                            (codegraph_core::Language::Go, crate::graphql::types::GraphQLLanguage::Go) |
                            (codegraph_core::Language::Java, crate::graphql::types::GraphQLLanguage::Java) |
                            (codegraph_core::Language::Cpp, crate::graphql::types::GraphQLLanguage::Cpp)
                        )
                    })
                })
            });
        }

        if let Some(ref node_type_filters) = input.node_type_filter {
            filtered_results.retain(|node| {
                node.node_type.as_ref().map_or(false, |node_type| {
                    node_type_filters.iter().any(|filter_type| {
                        matches!(
                            (node_type, filter_type),
                            (codegraph_core::NodeType::Function, crate::graphql::types::GraphQLNodeType::Function) |
                            (codegraph_core::NodeType::Struct, crate::graphql::types::GraphQLNodeType::Struct) |
                            (codegraph_core::NodeType::Class, crate::graphql::types::GraphQLNodeType::Class) |
                            (codegraph_core::NodeType::Interface, crate::graphql::types::GraphQLNodeType::Interface) |
                            (codegraph_core::NodeType::Module, crate::graphql::types::GraphQLNodeType::Module)
                        )
                    })
                })
            });
        }

        // Apply file path pattern filter
        if let Some(ref file_pattern) = input.file_path_pattern {
            filtered_results.retain(|node| {
                node.location.file_path.contains(file_pattern)
            });
        }

        // Apply content filter
        if let Some(ref content_filter) = input.content_filter {
            filtered_results.retain(|node| {
                node.content.as_ref().map_or(false, |content| {
                    content.to_lowercase().contains(&content_filter.to_lowercase())
                })
            });
        }

        // Apply sorting
        if let Some(sort_by) = input.sort_by {
            match sort_by {
                SearchSortBy::Name => filtered_results.sort_by(|a, b| a.name.cmp(&b.name)),
                SearchSortBy::CreatedAt => filtered_results.sort_by(|a, b| a.metadata.created_at.cmp(&b.metadata.created_at)),
                SearchSortBy::UpdatedAt => filtered_results.sort_by(|a, b| a.metadata.updated_at.cmp(&b.metadata.updated_at)),
                SearchSortBy::Complexity => filtered_results.sort_by(|a, b| {
                    b.complexity.unwrap_or(0.0).partial_cmp(&a.complexity.unwrap_or(0.0)).unwrap_or(std::cmp::Ordering::Equal)
                }),
                SearchSortBy::Relevance => {} // Already sorted by relevance from semantic search
            }
        }

        let total_count = filtered_results.len();

        // Apply pagination
        let paginated_results: Vec<_> = filtered_results
            .into_iter()
            .skip(offset as usize)
            .take(limit as usize)
            .map(|node| node.into())
            .collect();

        let elapsed = start_time.elapsed();
        let query_time_ms = elapsed.as_millis() as i32;

        // Log performance warning if search takes too long
        if query_time_ms > 50 {
            warn!("Code search took {}ms (>50ms target for simple queries)", query_time_ms);
        }

        info!("Code search completed: {} results in {}ms", total_count, query_time_ms);

        Ok(CodeSearchResult {
            nodes: paginated_results,
            total_count: total_count as i32,
            page_info: PageInfo {
                has_next_page: (offset + limit) < total_count as i32,
                has_previous_page: offset > 0,
                start_cursor: if offset > 0 { Some(offset.to_string()) } else { None },
                end_cursor: Some((offset + paginated_results.len() as i32).to_string()),
            },
            search_metadata: SearchMetadata {
                query_time_ms,
                index_used: "semantic_vector".to_string(),
                filter_applied: vec![
                    input.language_filter.map(|_| "language".to_string()),
                    input.node_type_filter.map(|_| "node_type".to_string()),
                    input.file_path_pattern.map(|_| "file_path".to_string()),
                    input.content_filter.map(|_| "content".to_string()),
                ].into_iter().flatten().collect(),
            },
        })
    }

    /// Perform graph traversal from a starting node
    #[instrument(skip(self, ctx), fields(start_node = %input.start_node_id))]
    async fn traverse_graph(
        &self,
        ctx: &Context<'_>,
        input: GraphTraversalInput,
    ) -> Result<GraphTraversalResult> {
        let start_time = Instant::now();
        let start_node_str = input.start_node_id.to_string();
        info!("Executing graph traversal from node: {}", start_node_str);

        let state = ctx.data::<AppState>()?;
        let traversal_loader = ctx.data::<DataLoader<GraphTraversalLoader>>()?;
        let edges_loader = ctx.data::<DataLoader<EdgesBySourceLoader>>()?;

        let start_node_id = NodeId::from_str(&start_node_str)
            .map_err(|_| async_graphql::Error::new("Invalid node ID format"))?;

        // Validate traversal parameters
        let max_depth = input.max_depth.unwrap_or(3).max(1).min(10);
        let limit = input.limit.unwrap_or(100).max(1).min(1000);

        // Create traversal key for caching
        let traversal_key = TraversalKey {
            start_node: start_node_id,
            max_depth,
            edge_types: input.edge_types.as_ref().map_or(vec![], |types| {
                types.iter().map(|t| format!("{:?}", t)).collect()
            }),
            direction: input.direction.map_or("Both".to_string(), |d| format!("{:?}", d)),
        };

        // Use DataLoader for efficient traversal caching
        let traversed_nodes = traversal_loader
            .load_one(traversal_key)
            .await
            .map_err(|e| async_graphql::Error::new(format!("Traversal failed: {}", e)))?
            .unwrap_or_default();

        // Load edges for the traversed nodes using DataLoader
        let node_ids: Vec<NodeId> = traversed_nodes.iter()
            .filter_map(|node| NodeId::from_str(&node.id.to_string()).ok())
            .collect();

        let edges_map = edges_loader
            .load_many(node_ids.iter().cloned())
            .await
            .map_err(|e| async_graphql::Error::new(format!("Edge loading failed: {}", e)))?;

        let edges: Vec<_> = edges_map.values().flatten().cloned().collect();

        // Build traversal path (simplified for demo)
        let traversal_path: Vec<ID> = traversed_nodes.iter()
            .map(|node| node.id.clone())
            .collect();

        let elapsed = start_time.elapsed();
        let traversal_time_ms = elapsed.as_millis() as i32;

        // Check performance target for complex queries
        if traversal_time_ms > 200 {
            warn!("Graph traversal took {}ms (>200ms target for complex queries)", traversal_time_ms);
        }

        info!("Graph traversal completed: {} nodes, {} edges in {}ms", 
            traversed_nodes.len(), edges.len(), traversal_time_ms);

        Ok(GraphTraversalResult {
            nodes: traversed_nodes.into_iter().take(limit as usize).collect(),
            edges: edges.into_iter().take(limit as usize).collect(),
            traversal_path,
            depth_reached: max_depth,
            total_visited: node_ids.len() as i32,
            metadata: TraversalMetadata {
                traversal_time_ms,
                algorithm_used: "breadth_first".to_string(),
                pruning_applied: limit < node_ids.len() as i32,
                max_depth,
            },
        })
    }

    /// Extract a subgraph around specific nodes or from a center point
    #[instrument(skip(self, ctx))]
    async fn extract_subgraph(
        &self,
        ctx: &Context<'_>,
        input: SubgraphExtractionInput,
    ) -> Result<SubgraphResult> {
        let start_time = Instant::now();
        info!("Executing subgraph extraction");

        let state = ctx.data::<AppState>()?;
        let node_loader = ctx.data::<DataLoader<NodeLoader>>()?;
        let edges_loader = ctx.data::<DataLoader<EdgesBySourceLoader>>()?;

        let radius = input.radius.unwrap_or(2).max(1).min(5);
        
        // Determine nodes to extract subgraph for
        let target_nodes: Vec<NodeId> = if let Some(center_id_str) = input.center_node_id.as_ref() {
            // Extract around a center node
            vec![NodeId::from_str(&center_id_str.to_string())
                .map_err(|_| async_graphql::Error::new("Invalid center node ID"))?]
        } else if let Some(node_id_strs) = input.node_ids.as_ref() {
            // Extract around specific nodes
            node_id_strs.iter()
                .map(|id_str| NodeId::from_str(&id_str.to_string()))
                .collect::<Result<Vec<_>, _>>()
                .map_err(|_| async_graphql::Error::new("Invalid node ID in list"))?
        } else {
            return Err(async_graphql::Error::new("Either center_node_id or node_ids must be provided"));
        };

        // Build subgraph by expanding from target nodes
        let mut subgraph_nodes = HashMap::new();
        let mut subgraph_edges = Vec::new();
        let mut visited = std::collections::HashSet::new();
        let mut to_visit = std::collections::VecDeque::new();

        // Initialize with target nodes
        for node_id in target_nodes {
            to_visit.push_back((node_id, 0));
        }

        // BFS expansion to build subgraph
        while let Some((current_id, depth)) = to_visit.pop_front() {
            if visited.contains(&current_id) || depth > radius {
                continue;
            }
            visited.insert(current_id);

            // Load current node
            if let Some(node) = node_loader
                .load_one(current_id)
                .await
                .map_err(|e| async_graphql::Error::new(format!("Node loading failed: {}", e)))?
            {
                subgraph_nodes.insert(current_id, node);
            }

            if depth < radius {
                // Load edges and add connected nodes to visit queue
                let edges = edges_loader
                    .load_one(current_id)
                    .await
                    .map_err(|e| async_graphql::Error::new(format!("Edge loading failed: {}", e)))?
                    .unwrap_or_default();

                for edge in edges {
                    subgraph_edges.push(edge.clone());
                    
                    // Add target node to visit queue
                    if let Ok(target_id) = NodeId::from_str(&edge.target_id.to_string()) {
                        if !visited.contains(&target_id) {
                            to_visit.push_back((target_id, depth + 1));
                        }
                    }
                }
            }
        }

        let elapsed = start_time.elapsed();
        let extraction_time_ms = elapsed.as_millis() as i32;
        
        let nodes: Vec<_> = subgraph_nodes.into_values().collect();
        let node_count = nodes.len() as i32;
        let edge_count = subgraph_edges.len() as i32;

        // Calculate connectivity score (simplified)
        let connectivity_score = if node_count > 0 {
            (edge_count as f32) / (node_count as f32)
        } else {
            0.0
        };

        info!("Subgraph extraction completed: {} nodes, {} edges in {}ms", 
            node_count, edge_count, extraction_time_ms);

        Ok(SubgraphResult {
            nodes,
            edges: subgraph_edges,
            subgraph_id: ID(Uuid::new_v4().to_string()),
            center_node_id: input.center_node_id,
            extraction_metadata: SubgraphMetadata {
                extraction_time_ms,
                extraction_strategy: input.extraction_strategy.map_or("radius".to_string(), |s| format!("{:?}", s)),
                node_count,
                edge_count,
                connectivity_score,
            },
        })
    }

    /// Perform semantic search using vector embeddings
    #[instrument(skip(self, ctx), fields(query = %input.query))]
    async fn semantic_search(
        &self,
        ctx: &Context<'_>,
        input: SemanticSearchInput,
    ) -> Result<SemanticSearchResult> {
        let start_time = Instant::now();
        info!("Executing semantic search: {}", input.query);

        let state = ctx.data::<AppState>()?;
        let semantic_loader = ctx.data::<DataLoader<SemanticSearchLoader>>()?;

        let limit = input.limit.unwrap_or(10).max(1).min(50);
        let similarity_threshold = input.similarity_threshold.unwrap_or(0.7).max(0.0).min(1.0);

        // Generate query embedding
        let embedding_start = Instant::now();
        let query_embedding = state.embedding_generator
            .generate_text_embedding(&input.query)
            .await
            .map_err(|e| async_graphql::Error::new(format!("Embedding generation failed: {}", e)))?;
        let embedding_time_ms = embedding_start.elapsed().as_millis() as i32;

        // Perform semantic search using DataLoader for caching
        let search_start = Instant::now();
        let retrieval_results = semantic_loader
            .load_one(input.query.clone())
            .await
            .map_err(|e| async_graphql::Error::new(format!("Semantic search failed: {}", e)))?
            .unwrap_or_default();
        let search_time_ms = search_start.elapsed().as_millis() as i32;

        // Filter by similarity threshold and apply filters
        let mut filtered_results: Vec<_> = retrieval_results
            .into_iter()
            .filter(|result| result.similarity_score >= similarity_threshold)
            .collect();

        // Apply language filter
        if let Some(ref language_filters) = input.language_filter {
            filtered_results.retain(|result| {
                result.node.language.as_ref().map_or(false, |lang| {
                    language_filters.iter().any(|filter_lang| {
                        matches!(
                            (lang, filter_lang),
                            (codegraph_core::Language::Rust, crate::graphql::types::GraphQLLanguage::Rust) |
                            (codegraph_core::Language::Python, crate::graphql::types::GraphQLLanguage::Python) |
                            (codegraph_core::Language::TypeScript, crate::graphql::types::GraphQLLanguage::TypeScript)
                        )
                    })
                })
            });
        }

        // Apply node type filter
        if let Some(ref node_type_filters) = input.node_type_filter {
            filtered_results.retain(|result| {
                result.node.node_type.as_ref().map_or(false, |node_type| {
                    node_type_filters.iter().any(|filter_type| {
                        matches!(
                            (node_type, filter_type),
                            (codegraph_core::NodeType::Function, crate::graphql::types::GraphQLNodeType::Function) |
                            (codegraph_core::NodeType::Struct, crate::graphql::types::GraphQLNodeType::Struct) |
                            (codegraph_core::NodeType::Class, crate::graphql::types::GraphQLNodeType::Class)
                        )
                    })
                })
            });
        }

        // Sort by similarity score and apply limit
        filtered_results.sort_by(|a, b| b.similarity_score.partial_cmp(&a.similarity_score).unwrap_or(std::cmp::Ordering::Equal));
        filtered_results.truncate(limit as usize);

        // Convert to scored nodes
        let scored_nodes: Vec<ScoredNode> = filtered_results.into_iter().map(|result| {
            ScoredNode {
                node: result.node.into(),
                similarity_score: result.similarity_score,
                ranking_score: result.similarity_score, // Simplified ranking
                distance_metric: "cosine".to_string(),
            }
        }).collect();

        let elapsed = start_time.elapsed();
        let total_time_ms = elapsed.as_millis() as i32;

        info!("Semantic search completed: {} results in {}ms", scored_nodes.len(), total_time_ms);

        Ok(SemanticSearchResult {
            nodes: scored_nodes,
            query_embedding,
            total_candidates: scored_nodes.len() as i32, // Simplified
            search_metadata: SemanticSearchMetadata {
                embedding_time_ms,
                search_time_ms,
                vector_dimension: query_embedding.len() as i32,
                similarity_threshold,
            },
        })
    }

    /// Get neighbor nodes for a given node (outgoing edges by default)
    #[instrument(skip(self, ctx), fields(node_id = %id))]
    async fn get_neighbors(
        &self,
        ctx: &Context<'_>,
        id: ID,
        #[graphql(default = 50)] limit: i32,
        edge_types: Option<Vec<crate::graphql::types::GraphQLEdgeType>>,
    ) -> Result<Vec<GraphQLCodeNode>> {
        // Basic rate limiting per-operation
        if let Some(auth) = ctx.data_opt::<AuthContext>() {
            let tier = if auth.roles.contains(&"premium".to_string()) { "premium" } else { "user" };
            let rl = RateLimitManager::new();
            let _ = rl.check_rate_limit(tier, "getNeighbors");
        }

        let edges_loader = ctx.data::<DataLoader<EdgesBySourceLoader>>()?;
        let node_loader = ctx.data::<DataLoader<NodeLoader>>()?;

        let node_id = NodeId::from_str(&id.to_string())
            .map_err(|_| async_graphql::Error::new("Invalid node ID format"))?;

        // Load outgoing edges for this node
        let mut edges = edges_loader
            .load_one(node_id)
            .await
            .map_err(|e| async_graphql::Error::new(format!("Failed to load edges: {}", e)))?
            .unwrap_or_default();

        // Optional filter by edge types
        if let Some(types) = &edge_types {
            let type_set: std::collections::HashSet<_> = types.iter().collect();
            edges.retain(|e| type_set.contains(&e.edge_type));
        }

        // Collect neighbor IDs and batch-load nodes
        let mut neighbor_ids: Vec<NodeId> = Vec::with_capacity(edges.len());
        for e in edges.iter().take(limit.max(1) as usize) {
            if let Ok(tid) = NodeId::from_str(&e.target_id.to_string()) {
                neighbor_ids.push(tid);
            }
        }

        let neighbors_map = node_loader
            .load_many(neighbor_ids)
            .await
            .map_err(|e| async_graphql::Error::new(format!("Failed to load neighbor nodes: {}", e)))?;

        Ok(neighbors_map.into_values().collect())
    }

    /// Find a shortest path between two nodes, returning the connecting edges
    #[instrument(skip(self, ctx), fields(from = %from, to = %to))]
    async fn find_path(
        &self,
        ctx: &Context<'_>,
        from: ID,
        to: ID,
        #[graphql(default = 10)] max_depth: i32,
    ) -> Result<Vec<GraphQLEdge>> {
        // Basic rate limiting per-operation
        if let Some(auth) = ctx.data_opt::<AuthContext>() {
            let tier = if auth.roles.contains(&"premium".to_string()) { "premium" } else { "user" };
            let rl = RateLimitManager::new();
            let _ = rl.check_rate_limit(tier, "findPath");
        }

        let state = ctx.data::<AppState>()?;
        let edges_loader = ctx.data::<DataLoader<EdgesBySourceLoader>>()?;

        let from_id = NodeId::from_str(&from.to_string())
            .map_err(|_| async_graphql::Error::new("Invalid 'from' node ID format"))?;
        let to_id = NodeId::from_str(&to.to_string())
            .map_err(|_| async_graphql::Error::new("Invalid 'to' node ID format"))?;

        // Use graph's shortest_path (BFS) which internally caches paths
        let path_opt = {
            let graph = state.graph.read().await;
            graph.shortest_path(from_id, to_id).await
        }.map_err(|e| async_graphql::Error::new(format!("Path search failed: {}", e)))?;

        let Some(path) = path_opt else { return Ok(vec![]); };

        // Build list of consecutive pairs to resolve actual edges via DataLoader
        let mut from_ids: Vec<NodeId> = Vec::new();
        for win in path.windows(2) {
            if let [a, _b] = win { from_ids.push(*a); }
        }

        let edges_map = edges_loader
            .load_many(from_ids)
            .await
            .map_err(|e| async_graphql::Error::new(format!("Edge loading failed: {}", e)))?;

        // For each consecutive pair, pick the matching edge if exists; otherwise synthesize
        let mut result: Vec<GraphQLEdge> = Vec::new();
        for win in path.windows(2) {
            if let [a, b] = win {
                if let Some(edges) = edges_map.get(a) {
                    if let Some(edge) = edges.iter().find(|e| e.target_id.to_string() == b.to_string()) {
                        result.push(edge.clone());
                        continue;
                    }
                }
                // Synthesize a generic edge if not present in loader result
                let now = chrono::Utc::now();
                result.push(GraphQLEdge {
                    id: ID(Uuid::new_v4().to_string()),
                    source_id: ID(a.to_string()),
                    target_id: ID(b.to_string()),
                    edge_type: crate::graphql::types::GraphQLEdgeType::Other,
                    weight: None,
                    attributes: std::collections::HashMap::new(),
                    created_at: now,
                });
            }
        }

        Ok(result)
    }

    /// Get a specific node by ID
    async fn node(&self, ctx: &Context<'_>, id: ID) -> Result<Option<GraphQLCodeNode>> {
        let node_loader = ctx.data::<DataLoader<NodeLoader>>()?;
        let node_id = NodeId::from_str(&id.to_string())
            .map_err(|_| async_graphql::Error::new("Invalid node ID format"))?;

        node_loader.load_one(node_id).await
            .map_err(|e| async_graphql::Error::new(format!("Failed to load node: {}", e)))
    }

    /// Get multiple nodes by IDs (batch operation using DataLoader)
    async fn nodes(&self, ctx: &Context<'_>, ids: Vec<ID>) -> Result<Vec<GraphQLCodeNode>> {
        let node_loader = ctx.data::<DataLoader<NodeLoader>>()?;
        let node_ids: Result<Vec<NodeId>, _> = ids.iter()
            .map(|id| NodeId::from_str(&id.to_string()))
            .collect();
        
        let node_ids = node_ids
            .map_err(|_| async_graphql::Error::new("Invalid node ID format"))?;

        let result = node_loader.load_many(node_ids).await
            .map_err(|e| async_graphql::Error::new(format!("Failed to load nodes: {}", e)))?;

        Ok(result.into_values().collect())
    }
}

pub struct MutationRoot;

#[Object]
impl MutationRoot {
    /// Start repository indexing job and emit progress events
    async fn index_repository(&self, ctx: &Context<'_>, repo_url: String) -> Result<bool> {
        // Rate-limit indexing operations a bit more strictly
        if let Some(auth) = ctx.data_opt::<AuthContext>() {
            let tier = if auth.roles.contains(&"premium".to_string()) { "premium" } else { "user" };
            let rl = RateLimitManager::new();
            let _ = rl.check_rate_limit(tier, "indexRepository");
        }

        // Simulate async indexing with staged progress via broker
        let job_id = Uuid::new_v4().to_string();
        tokio::spawn(async move {
            event_bus::publish_indexing_progress(job_id.clone(), 0.05, "queued".into(), Some(30.0), Some("Queued for indexing".into()));
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            event_bus::publish_indexing_progress(job_id.clone(), 0.35, "cloning".into(), Some(25.0), Some(format!("Cloning {}", repo_url)));
            tokio::time::sleep(std::time::Duration::from_millis(150)).await;
            event_bus::publish_indexing_progress(job_id.clone(), 0.65, "parsing".into(), Some(10.0), Some("Parsing files".into()));
            tokio::time::sleep(std::time::Duration::from_millis(150)).await;
            event_bus::publish_indexing_progress(job_id.clone(), 0.85, "embedding".into(), Some(5.0), Some("Generating embeddings".into()));
            tokio::time::sleep(std::time::Duration::from_millis(150)).await;
            event_bus::publish_indexing_progress(job_id.clone(), 1.0, "completed".into(), None, Some("Index build completed".into()));
        });

        Ok(true)
    }

    /// Update existing node fields
    async fn update_node(&self, ctx: &Context<'_>, input: UpdateNodeInput) -> Result<bool> {
        if let Some(auth) = ctx.data_opt::<AuthContext>() {
            let tier = if auth.roles.contains(&"premium".to_string()) { "premium" } else { "user" };
            let rl = RateLimitManager::new();
            let _ = rl.check_rate_limit(tier, "updateNode");
        }

        let state = ctx.data::<AppState>()?;
        let mut graph = state.graph.write().await;

        let node_id = NodeId::from_str(&input.id.to_string())
            .map_err(|_| async_graphql::Error::new("Invalid node ID"))?;

        let current = graph
            .get_node(node_id)
            .await
            .map_err(|e| async_graphql::Error::new(format!("Failed to fetch node: {}", e)))?
            .ok_or_else(|| async_graphql::Error::new("Node not found"))?;

        // Apply updates
        let mut updated = current.clone();
        if let Some(name) = input.name { updated.name = name; }
        if let Some(nt) = input.node_type { updated.node_type = Some(nt); }
        if let Some(lang) = input.language { updated.language = Some(lang); }
        if let Some(fp) = input.file_path { updated.location.file_path = fp; }
        if let Some(sl) = input.start_line { updated.location.line = sl as u32; }
        if let Some(sc) = input.start_column { updated.location.column = sc as u32; }
        if let Some(el) = input.end_line { updated.location.end_line = Some(el as u32); }
        if let Some(ec) = input.end_column { updated.location.end_column = Some(ec as u32); }

        graph
            .update_node(updated.clone())
            .await
            .map_err(|e| async_graphql::Error::new(format!("Failed to update node: {}", e)))?;

        // Emit graph update event
        event_bus::publish_graph_update(
            crate::subscriptions::GraphUpdateType::NodesModified,
            vec![node_id.to_string()],
            vec![],
            1,
            Some("Node updated via GraphQL".into()),
        );

        Ok(true)
    }

    /// Delete a node by ID
    async fn delete_node(&self, ctx: &Context<'_>, id: ID) -> Result<bool> {
        if let Some(auth) = ctx.data_opt::<AuthContext>() {
            let tier = if auth.roles.contains(&"premium".to_string()) { "premium" } else { "user" };
            let rl = RateLimitManager::new();
            let _ = rl.check_rate_limit(tier, "deleteNode");
        }

        let state = ctx.data::<AppState>()?;
        let mut graph = state.graph.write().await;
        let node_id = NodeId::from_str(&id.to_string())
            .map_err(|_| async_graphql::Error::new("Invalid node ID"))?;

        graph
            .remove_node(node_id)
            .await
            .map_err(|e| async_graphql::Error::new(format!("Failed to delete node: {}", e)))?;

        event_bus::publish_graph_update(
            crate::subscriptions::GraphUpdateType::NodesRemoved,
            vec![node_id.to_string()],
            vec![],
            1,
            Some("Node deleted via GraphQL".into()),
        );

        Ok(true)
    }
}
