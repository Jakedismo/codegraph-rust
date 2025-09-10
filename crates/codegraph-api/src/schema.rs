use async_graphql::{EmptyMutation, Schema, dataloader::DataLoader};
use crate::queries::Query;
use crate::subscriptions::SubscriptionRoot;
use crate::state::AppState;
use crate::graphql::loaders::{LoaderFactory, NodeLoader, EdgesBySourceLoader, SemanticSearchLoader, GraphTraversalLoader};
use std::sync::Arc;

pub type CodeGraphSchema = Schema<Query, EmptyMutation, SubscriptionRoot>;

pub fn create_schema(state: AppState) -> CodeGraphSchema {
    let state_arc = Arc::new(state.clone());
    let loader_factory = LoaderFactory::new(state_arc);

    // Create DataLoaders for efficient batch loading
    let node_loader = loader_factory.create_node_loader();
    let edges_loader = loader_factory.create_edges_loader();
    let semantic_search_loader = loader_factory.create_semantic_search_loader();
    let traversal_loader = loader_factory.create_traversal_loader();

    Schema::build(Query, EmptyMutation, SubscriptionRoot::default())
        // Attach shared app state for resolvers/subscriptions
        .data(state)
        
        // Attach DataLoaders for efficient batch operations
        .data(node_loader)
        .data(edges_loader)
        .data(semantic_search_loader)
        .data(traversal_loader)
        
        // Basic safety limits for performance and DoS prevention
        .limit_depth(16)
        .limit_complexity(20_000)
        
        // Request timeout to prevent long-running queries
        .query_timeout(std::time::Duration::from_secs(30))
        
        // Enable query complexity analysis
        .enable_query_complexity_analysis()
        
        .finish()
}
