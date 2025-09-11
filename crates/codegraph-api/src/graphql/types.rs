use async_graphql::{Enum, Object, SimpleObject, ID};
use chrono::{DateTime, Utc};
use codegraph_core::{CodeNode, EdgeType, Language, Location, NodeId, NodeType};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

#[derive(SimpleObject, Clone, Debug, Serialize, Deserialize)]
pub struct GraphQLCodeNode {
    pub id: ID,
    pub name: String,
    pub node_type: Option<GraphQLNodeType>,
    pub language: Option<GraphQLLanguage>,
    pub location: GraphQLLocation,
    pub content: Option<String>,
    pub complexity: Option<f32>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub attributes: HashMap<String, String>,
}

#[derive(SimpleObject, Clone, Debug, Serialize, Deserialize)]
pub struct GraphQLLocation {
    pub file_path: String,
    pub line: u32,
    pub column: u32,
    pub end_line: Option<u32>,
    pub end_column: Option<u32>,
}

#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug, Serialize, Deserialize)]
pub enum GraphQLNodeType {
    Function,
    Struct,
    Enum,
    Trait,
    Module,
    Variable,
    Import,
    Class,
    Interface,
    Type,
    Other,
}

#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug, Serialize, Deserialize)]
pub enum GraphQLLanguage {
    Rust,
    TypeScript,
    JavaScript,
    Python,
    Go,
    Java,
    Cpp,
    Other,
}

#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug, Serialize, Deserialize, Hash)]
pub enum GraphQLEdgeType {
    Calls,
    Defines,
    Uses,
    Imports,
    Extends,
    Implements,
    Contains,
    References,
    Other,
}

#[derive(SimpleObject, Clone, Debug, Serialize, Deserialize)]
pub struct GraphQLEdge {
    pub id: ID,
    pub source_id: ID,
    pub target_id: ID,
    pub edge_type: GraphQLEdgeType,
    pub weight: Option<f32>,
    pub attributes: HashMap<String, String>,
    pub created_at: DateTime<Utc>,
}

// GraphQL input for updating a node
#[derive(async_graphql::InputObject, Clone, Debug, Serialize, Deserialize)]
pub struct UpdateNodeInput {
    pub id: ID,
    pub name: Option<String>,
    pub node_type: Option<GraphQLNodeType>,
    pub language: Option<GraphQLLanguage>,
    pub file_path: Option<String>,
    pub start_line: Option<i32>,
    pub start_column: Option<i32>,
    pub end_line: Option<i32>,
    pub end_column: Option<i32>,
}

#[derive(SimpleObject, Clone, Debug, Serialize, Deserialize)]
pub struct CodeSearchResult {
    pub nodes: Vec<GraphQLCodeNode>,
    pub total_count: i32,
    pub page_info: PageInfo,
    pub search_metadata: SearchMetadata,
}

#[derive(SimpleObject, Clone, Debug, Serialize, Deserialize)]
pub struct PageInfo {
    pub has_next_page: bool,
    pub has_previous_page: bool,
    pub start_cursor: Option<String>,
    pub end_cursor: Option<String>,
}

#[derive(SimpleObject, Clone, Debug, Serialize, Deserialize)]
pub struct SearchMetadata {
    pub query_time_ms: i32,
    pub index_used: String,
    pub filter_applied: Vec<String>,
}

#[derive(SimpleObject, Clone, Debug, Serialize, Deserialize)]
pub struct GraphTraversalResult {
    pub nodes: Vec<GraphQLCodeNode>,
    pub edges: Vec<GraphQLEdge>,
    pub traversal_path: Vec<ID>,
    pub depth_reached: i32,
    pub total_visited: i32,
    pub metadata: TraversalMetadata,
}

#[derive(SimpleObject, Clone, Debug, Serialize, Deserialize)]
pub struct TraversalMetadata {
    pub traversal_time_ms: i32,
    pub algorithm_used: String,
    pub pruning_applied: bool,
    pub max_depth: i32,
}

#[derive(SimpleObject, Clone, Debug, Serialize, Deserialize)]
pub struct SubgraphResult {
    pub nodes: Vec<GraphQLCodeNode>,
    pub edges: Vec<GraphQLEdge>,
    pub subgraph_id: ID,
    pub center_node_id: Option<ID>,
    pub extraction_metadata: SubgraphMetadata,
}

#[derive(SimpleObject, Clone, Debug, Serialize, Deserialize)]
pub struct SubgraphMetadata {
    pub extraction_time_ms: i32,
    pub extraction_strategy: String,
    pub node_count: i32,
    pub edge_count: i32,
    pub connectivity_score: f32,
}

#[derive(SimpleObject, Clone, Debug, Serialize, Deserialize)]
pub struct SemanticSearchResult {
    pub nodes: Vec<ScoredNode>,
    pub query_embedding: Vec<f32>,
    pub total_candidates: i32,
    pub search_metadata: SemanticSearchMetadata,
}

#[derive(SimpleObject, Clone, Debug, Serialize, Deserialize)]
pub struct ScoredNode {
    pub node: GraphQLCodeNode,
    pub similarity_score: f32,
    pub ranking_score: f32,
    pub distance_metric: String,
}

#[derive(SimpleObject, Clone, Debug, Serialize, Deserialize)]
pub struct SemanticSearchMetadata {
    pub embedding_time_ms: i32,
    pub search_time_ms: i32,
    pub vector_dimension: i32,
    pub similarity_threshold: f32,
}

// Input types for mutations and complex queries
#[derive(async_graphql::InputObject, Clone, Debug, Serialize, Deserialize)]
pub struct CodeSearchInput {
    pub query: String,
    pub language_filter: Option<Vec<GraphQLLanguage>>,
    pub node_type_filter: Option<Vec<GraphQLNodeType>>,
    pub file_path_pattern: Option<String>,
    pub content_filter: Option<String>,
    pub limit: Option<i32>,
    pub offset: Option<i32>,
    pub sort_by: Option<SearchSortBy>,
}

#[derive(async_graphql::InputObject, Clone, Debug, Serialize, Deserialize)]
pub struct GraphTraversalInput {
    pub start_node_id: ID,
    pub max_depth: Option<i32>,
    pub edge_types: Option<Vec<GraphQLEdgeType>>,
    pub direction: Option<TraversalDirection>,
    pub limit: Option<i32>,
    pub include_cycles: Option<bool>,
}

#[derive(async_graphql::InputObject, Clone, Debug, Serialize, Deserialize)]
pub struct SubgraphExtractionInput {
    pub center_node_id: Option<ID>,
    pub node_ids: Option<Vec<ID>>,
    pub radius: Option<i32>,
    pub include_metadata: Option<bool>,
    pub extraction_strategy: Option<ExtractionStrategy>,
}

#[derive(async_graphql::InputObject, Clone, Debug, Serialize, Deserialize)]
pub struct SemanticSearchInput {
    pub query: String,
    pub similarity_threshold: Option<f32>,
    pub limit: Option<i32>,
    pub language_filter: Option<Vec<GraphQLLanguage>>,
    pub node_type_filter: Option<Vec<GraphQLNodeType>>,
}

#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug, Serialize, Deserialize)]
pub enum SearchSortBy {
    Relevance,
    Name,
    CreatedAt,
    UpdatedAt,
    Complexity,
}

#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug, Serialize, Deserialize)]
pub enum TraversalDirection {
    Outgoing,
    Incoming,
    Both,
}

#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug, Serialize, Deserialize)]
pub enum ExtractionStrategy {
    Radius,
    Connected,
    Semantic,
    Dependency,
}

// Conversion implementations
impl From<CodeNode> for GraphQLCodeNode {
    fn from(node: CodeNode) -> Self {
        Self {
            id: ID(node.id.to_string()),
            name: node.name.to_string(),
            node_type: node.node_type.map(Into::into),
            language: node.language.map(Into::into),
            location: node.location.into(),
            content: node.content.map(|s| s.to_string()),
            complexity: node.complexity,
            created_at: node.metadata.created_at,
            updated_at: node.metadata.updated_at,
            attributes: node.metadata.attributes,
        }
    }
}

impl From<Location> for GraphQLLocation {
    fn from(location: Location) -> Self {
        Self {
            file_path: location.file_path,
            line: location.line,
            column: location.column,
            end_line: location.end_line,
            end_column: location.end_column,
        }
    }
}

impl From<NodeType> for GraphQLNodeType {
    fn from(node_type: NodeType) -> Self {
        match node_type {
            NodeType::Function => GraphQLNodeType::Function,
            NodeType::Struct => GraphQLNodeType::Struct,
            NodeType::Enum => GraphQLNodeType::Enum,
            NodeType::Trait => GraphQLNodeType::Trait,
            NodeType::Module => GraphQLNodeType::Module,
            NodeType::Variable => GraphQLNodeType::Variable,
            NodeType::Import => GraphQLNodeType::Import,
            NodeType::Class => GraphQLNodeType::Class,
            NodeType::Interface => GraphQLNodeType::Interface,
            NodeType::Type => GraphQLNodeType::Type,
            NodeType::Other(_) => GraphQLNodeType::Other,
        }
    }
}

impl From<Language> for GraphQLLanguage {
    fn from(language: Language) -> Self {
        match language {
            Language::Rust => GraphQLLanguage::Rust,
            Language::TypeScript => GraphQLLanguage::TypeScript,
            Language::JavaScript => GraphQLLanguage::JavaScript,
            Language::Python => GraphQLLanguage::Python,
            Language::Go => GraphQLLanguage::Go,
            Language::Java => GraphQLLanguage::Java,
            Language::Cpp => GraphQLLanguage::Cpp,
            Language::Other(_) => GraphQLLanguage::Other,
        }
    }
}

impl From<EdgeType> for GraphQLEdgeType {
    fn from(edge_type: EdgeType) -> Self {
        match edge_type {
            EdgeType::Calls => GraphQLEdgeType::Calls,
            EdgeType::Defines => GraphQLEdgeType::Defines,
            EdgeType::Uses => GraphQLEdgeType::Uses,
            EdgeType::Imports => GraphQLEdgeType::Imports,
            EdgeType::Extends => GraphQLEdgeType::Extends,
            EdgeType::Implements => GraphQLEdgeType::Implements,
            EdgeType::Contains => GraphQLEdgeType::Contains,
            EdgeType::References => GraphQLEdgeType::References,
            EdgeType::Other(_) => GraphQLEdgeType::Other,
        }
    }
}

impl From<GraphQLNodeType> for NodeType {
    fn from(graphql_type: GraphQLNodeType) -> Self {
        match graphql_type {
            GraphQLNodeType::Function => NodeType::Function,
            GraphQLNodeType::Struct => NodeType::Struct,
            GraphQLNodeType::Enum => NodeType::Enum,
            GraphQLNodeType::Trait => NodeType::Trait,
            GraphQLNodeType::Module => NodeType::Module,
            GraphQLNodeType::Variable => NodeType::Variable,
            GraphQLNodeType::Import => NodeType::Import,
            GraphQLNodeType::Class => NodeType::Class,
            GraphQLNodeType::Interface => NodeType::Interface,
            GraphQLNodeType::Type => NodeType::Type,
            GraphQLNodeType::Other => NodeType::Other("other".to_string()),
        }
    }
}

impl From<GraphQLLanguage> for Language {
    fn from(graphql_language: GraphQLLanguage) -> Self {
        match graphql_language {
            GraphQLLanguage::Rust => Language::Rust,
            GraphQLLanguage::TypeScript => Language::TypeScript,
            GraphQLLanguage::JavaScript => Language::JavaScript,
            GraphQLLanguage::Python => Language::Python,
            GraphQLLanguage::Go => Language::Go,
            GraphQLLanguage::Java => Language::Java,
            GraphQLLanguage::Cpp => Language::Cpp,
            GraphQLLanguage::Other => Language::Other("other".to_string()),
        }
    }
}

impl From<GraphQLEdgeType> for EdgeType {
    fn from(graphql_edge_type: GraphQLEdgeType) -> Self {
        match graphql_edge_type {
            GraphQLEdgeType::Calls => EdgeType::Calls,
            GraphQLEdgeType::Defines => EdgeType::Defines,
            GraphQLEdgeType::Uses => EdgeType::Uses,
            GraphQLEdgeType::Imports => EdgeType::Imports,
            GraphQLEdgeType::Extends => EdgeType::Extends,
            GraphQLEdgeType::Implements => EdgeType::Implements,
            GraphQLEdgeType::Contains => EdgeType::Contains,
            GraphQLEdgeType::References => EdgeType::References,
            GraphQLEdgeType::Other => EdgeType::Other("other".to_string()),
        }
    }
}
