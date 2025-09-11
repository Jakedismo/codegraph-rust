use crate::graphql::loaders::{
    EdgesBySourceLoader, GraphTraversalLoader, LoaderFactory, NodeLoader, SemanticSearchLoader,
};
use crate::queries::Query;
use crate::state::AppState;
use async_graphql::{dataloader::DataLoader, Schema, EmptyMutation, EmptySubscription};
#[cfg(not(feature = "minimal"))]
use crate::graphql::resolvers::MutationRoot;
#[cfg(not(feature = "minimal"))]
use crate::subscriptions::SubscriptionRoot;
use std::sync::Arc;

#[cfg(not(feature = "minimal"))]
pub type CodeGraphSchema = Schema<Query, MutationRoot, SubscriptionRoot>;
#[cfg(feature = "minimal")]
pub type CodeGraphSchema = Schema<Query, EmptyMutation, EmptySubscription>;

#[cfg(not(feature = "minimal"))]
pub fn create_schema(state: AppState) -> CodeGraphSchema {
    let state_arc = Arc::new(state.clone());
    let loader_factory = LoaderFactory::new(state_arc);

    // Create DataLoaders for efficient batch loading
    let node_loader = loader_factory.create_node_loader();
    let edges_loader = loader_factory.create_edges_loader();
    let semantic_search_loader = loader_factory.create_semantic_search_loader();
    let traversal_loader = loader_factory.create_traversal_loader();

    Schema::build(Query, MutationRoot, SubscriptionRoot::default())
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
        .finish()
}

#[cfg(feature = "minimal")]
pub fn create_schema(state: AppState) -> CodeGraphSchema {
    let state_arc = std::sync::Arc::new(state.clone());
    let loader_factory = crate::graphql::loaders::LoaderFactory::new(state_arc);

    // Create DataLoaders for efficient batch loading
    let node_loader = loader_factory.create_node_loader();
    let edges_loader = loader_factory.create_edges_loader();
    let semantic_search_loader = loader_factory.create_semantic_search_loader();
    let traversal_loader = loader_factory.create_traversal_loader();

    Schema::build(Query, EmptyMutation, EmptySubscription)
        .data(state)
        .data(node_loader)
        .data(edges_loader)
        .data(semantic_search_loader)
        .data(traversal_loader)
        .limit_depth(16)
        .limit_complexity(20_000)
        .finish()
}
#![cfg(feature = "graphql")]
