use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::collections::HashMap;
use std::hash::Hash;
use std::sync::Arc;
use uuid::Uuid;

pub type NodeId = Uuid;
pub type EdgeId = Uuid;
pub type IndexId = String;
pub type Vector = Vec<f32>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeNode<'a> {
    pub id: NodeId,
    pub kind: NodeKind,
    pub content: Cow<'a, str>,
    pub metadata: NodeMetadata,
    pub location: SourceRange,
    pub embedding: Option<Arc<Vector>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeEdge {
    pub id: EdgeId,
    pub from: NodeId,
    pub to: NodeId,
    pub kind: EdgeKind,
    pub weight: f32,
    pub metadata: EdgeMetadata,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum NodeKind {
    Function,
    Class,
    Interface,
    Method,
    Field,
    Variable,
    Parameter,
    Import,
    Export,
    Module,
    File,
    Directory,
    Comment,
    Literal,
    Expression,
    Statement,
    Type,
    Namespace,
    Enum,
    Struct,
    Trait,
    Macro,
    Constant,
    Generic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EdgeKind {
    Contains,
    References,
    Calls,
    Inherits,
    Implements,
    Imports,
    Exports,
    Defines,
    Uses,
    DependsOn,
    Overrides,
    Annotates,
    HasType,
    ReturnType,
    ParameterType,
    ThrowsException,
    DocumentedBy,
    Similar,
    ControlFlow,
    DataFlow,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeMetadata {
    pub language: Language,
    pub visibility: Visibility,
    pub modifiers: Vec<String>,
    pub annotations: HashMap<String, String>,
    pub size_bytes: usize,
    pub complexity: Option<u32>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EdgeMetadata {
    pub confidence: f32,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub annotations: HashMap<String, String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Language {
    Rust,
    Python,
    JavaScript,
    TypeScript,
    Java,
    Go,
    CSharp,
    Cpp,
    C,
    Kotlin,
    Swift,
    Ruby,
    Php,
    Shell,
    Sql,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Visibility {
    Public,
    Private,
    Protected,
    Internal,
    Package,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceRange {
    pub start: SourcePosition,
    pub end: SourcePosition,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourcePosition {
    pub line: u32,
    pub column: u32,
    pub byte_offset: usize,
}

#[derive(Debug, Clone)]
pub struct ParsedCode<'a> {
    pub language: Language,
    pub source: Cow<'a, str>,
    pub file_path: Cow<'a, str>,
    pub nodes: Vec<CodeNode<'a>>,
    pub edges: Vec<CodeEdge>,
    pub parse_duration: std::time::Duration,
}

#[derive(Debug, Clone)]
pub struct GraphQuery {
    pub node_kinds: Option<Vec<NodeKind>>,
    pub edge_kinds: Option<Vec<EdgeKind>>,
    pub languages: Option<Vec<Language>>,
    pub depth_limit: Option<u32>,
    pub result_limit: Option<usize>,
    pub filters: HashMap<String, QueryFilter>,
}

#[derive(Debug, Clone)]
pub enum QueryFilter {
    Equals(String),
    Contains(String),
    Regex(String),
    Range(f64, f64),
    In(Vec<String>),
    Not(Box<QueryFilter>),
    And(Vec<QueryFilter>),
    Or(Vec<QueryFilter>),
}

#[derive(Debug, Clone)]
pub struct SearchQuery {
    pub vector: Vector,
    pub k: usize,
    pub threshold: Option<f32>,
    pub index_name: Option<String>,
    pub filters: HashMap<String, QueryFilter>,
    pub include_metadata: bool,
}

#[derive(Debug, Clone)]
pub struct SearchResult {
    pub node_id: NodeId,
    pub score: f32,
    pub node: Option<CodeNode<'static>>,
    pub metadata: Option<HashMap<String, String>>,
}

#[derive(Debug, Clone)]
pub struct BatchSearchQuery {
    pub queries: Vec<SearchQuery>,
    pub merge_strategy: MergeStrategy,
}

#[derive(Debug, Clone, Copy)]
pub enum MergeStrategy {
    Union,
    Intersection,
    WeightedAverage,
    Max,
    Min,
}

#[derive(Debug, Clone)]
pub struct IndexStats {
    pub name: String,
    pub dimensions: usize,
    pub total_vectors: usize,
    pub memory_usage_bytes: usize,
    pub index_type: String,
    pub build_time: std::time::Duration,
    pub last_updated: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug)]
pub struct TransactionContext {
    pub id: Uuid,
    pub isolation_level: IsolationLevel,
    pub read_only: bool,
    pub timeout: std::time::Duration,
    pub created_at: std::time::Instant,
}

#[derive(Debug, Clone, Copy)]
pub enum IsolationLevel {
    ReadUncommitted,
    ReadCommitted,
    RepeatableRead,
    Serializable,
}

impl Default for NodeMetadata {
    fn default() -> Self {
        let now = chrono::Utc::now();
        Self {
            language: Language::Unknown,
            visibility: Visibility::Public,
            modifiers: Vec::new(),
            annotations: HashMap::new(),
            size_bytes: 0,
            complexity: None,
            created_at: now,
            updated_at: now,
        }
    }
}

impl Default for EdgeMetadata {
    fn default() -> Self {
        Self {
            confidence: 1.0,
            created_at: chrono::Utc::now(),
            annotations: HashMap::new(),
        }
    }
}

impl Default for GraphQuery {
    fn default() -> Self {
        Self {
            node_kinds: None,
            edge_kinds: None,
            languages: None,
            depth_limit: Some(10),
            result_limit: Some(1000),
            filters: HashMap::new(),
        }
    }
}

impl<'a> CodeNode<'a> {
    pub fn new(
        kind: NodeKind,
        content: impl Into<Cow<'a, str>>,
        location: SourceRange,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            kind,
            content: content.into(),
            metadata: NodeMetadata::default(),
            location,
            embedding: None,
        }
    }

    pub fn with_metadata(mut self, metadata: NodeMetadata) -> Self {
        self.metadata = metadata;
        self
    }

    pub fn with_embedding(mut self, embedding: Vector) -> Self {
        self.embedding = Some(Arc::new(embedding));
        self
    }

    pub fn into_owned(self) -> CodeNode<'static> {
        CodeNode {
            id: self.id,
            kind: self.kind,
            content: Cow::Owned(self.content.into_owned()),
            metadata: self.metadata,
            location: self.location,
            embedding: self.embedding,
        }
    }
}

impl CodeEdge {
    pub fn new(from: NodeId, to: NodeId, kind: EdgeKind) -> Self {
        Self {
            id: Uuid::new_v4(),
            from,
            to,
            kind,
            weight: 1.0,
            metadata: EdgeMetadata::default(),
        }
    }

    pub fn with_weight(mut self, weight: f32) -> Self {
        self.weight = weight;
        self
    }

    pub fn with_metadata(mut self, metadata: EdgeMetadata) -> Self {
        self.metadata = metadata;
        self
    }
}

impl SourceRange {
    pub fn new(start_line: u32, start_column: u32, start_offset: usize, 
               end_line: u32, end_column: u32, end_offset: usize) -> Self {
        Self {
            start: SourcePosition {
                line: start_line,
                column: start_column,
                byte_offset: start_offset,
            },
            end: SourcePosition {
                line: end_line,
                column: end_column,
                byte_offset: end_offset,
            },
        }
    }

    pub fn contains(&self, position: &SourcePosition) -> bool {
        position.byte_offset >= self.start.byte_offset && 
        position.byte_offset <= self.end.byte_offset
    }
}

unsafe impl Send for TransactionContext {}
unsafe impl Sync for TransactionContext {}