use async_graphql::{Context, Object, Result, ID, dataloader::DataLoader};
use std::str::FromStr;
use uuid::Uuid;

use crate::graphql::{
    CodeSearchInput, CodeSearchResult, GraphTraversalInput, GraphTraversalResult,
    SemanticSearchInput, SemanticSearchResult, SubgraphExtractionInput, SubgraphResult,
    GraphQLCodeNode, QueryRoot,
};
use crate::graphql::loaders::{NodeLoader, EdgesBySourceLoader, SemanticSearchLoader, GraphTraversalLoader};

pub struct Query;

#[Object]
impl Query {
    /// Search for code nodes with text and filters
    async fn search_code(
        &self,
        ctx: &Context<'_>,
        input: CodeSearchInput,
    ) -> Result<CodeSearchResult> {
        let query_root = QueryRoot;
        query_root.search_code(ctx, input).await
    }

    /// Perform graph traversal from a starting node
    async fn traverse_graph(
        &self,
        ctx: &Context<'_>,
        input: GraphTraversalInput,
    ) -> Result<GraphTraversalResult> {
        let query_root = QueryRoot;
        query_root.traverse_graph(ctx, input).await
    }

    /// Extract a subgraph around specific nodes or from a center point
    async fn extract_subgraph(
        &self,
        ctx: &Context<'_>,
        input: SubgraphExtractionInput,
    ) -> Result<SubgraphResult> {
        let query_root = QueryRoot;
        query_root.extract_subgraph(ctx, input).await
    }

    /// Perform semantic search using vector embeddings
    async fn semantic_search(
        &self,
        ctx: &Context<'_>,
        input: SemanticSearchInput,
    ) -> Result<SemanticSearchResult> {
        let query_root = QueryRoot;
        query_root.semantic_search(ctx, input).await
    }

    /// Get a specific node by ID
    async fn node(&self, ctx: &Context<'_>, id: ID) -> Result<Option<GraphQLCodeNode>> {
        let query_root = QueryRoot;
        query_root.node(ctx, id).await
    }

    /// Get multiple nodes by IDs (batch operation using DataLoader)
    async fn nodes(&self, ctx: &Context<'_>, ids: Vec<ID>) -> Result<Vec<GraphQLCodeNode>> {
        let query_root = QueryRoot;
        query_root.nodes(ctx, ids).await
    }

    /// Health check endpoint
    async fn health(&self) -> Result<String> {
        Ok("GraphQL API is running".to_string())
    }

    /// Get API version
    async fn version(&self) -> Result<String> {
        Ok("1.0.0".to_string())
    }
}
