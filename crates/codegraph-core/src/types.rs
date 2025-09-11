use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::str::FromStr;
use uuid::Uuid;

pub type NodeId = Uuid;
pub type EdgeId = Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Language {
    Rust,
    TypeScript,
    JavaScript,
    Python,
    Go,
    Java,
    Cpp,
    Other(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum NodeType {
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
    Other(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EdgeType {
    Calls,
    Defines,
    Uses,
    Imports,
    Extends,
    Implements,
    Contains,
    References,
    Other(String),
}

impl Default for EdgeType {
    fn default() -> Self {
        EdgeType::References
    }
}

impl fmt::Display for EdgeType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            EdgeType::Calls => "calls",
            EdgeType::Defines => "defines",
            EdgeType::Uses => "uses",
            EdgeType::Imports => "imports",
            EdgeType::Extends => "extends",
            EdgeType::Implements => "implements",
            EdgeType::Contains => "contains",
            EdgeType::References => "references",
            EdgeType::Other(s) => s.as_str(),
        };
        write!(f, "{}", s)
    }
}

impl FromStr for EdgeType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "calls" => Ok(EdgeType::Calls),
            "defines" => Ok(EdgeType::Defines),
            "uses" => Ok(EdgeType::Uses),
            "imports" => Ok(EdgeType::Imports),
            "extends" => Ok(EdgeType::Extends),
            "implements" => Ok(EdgeType::Implements),
            "contains" => Ok(EdgeType::Contains),
            "references" => Ok(EdgeType::References),
            other => Ok(EdgeType::Other(other.to_string())),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Location {
    pub file_path: String,
    pub line: u32,
    pub column: u32,
    pub end_line: Option<u32>,
    pub end_column: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Metadata {
    pub attributes: HashMap<String, String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ChangeEvent {
    Created(String),  // file path
    Modified(String), // file path
    Deleted(String),  // file path
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdatePayload {
    pub event: ChangeEvent,
    pub content: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Delta {
    pub file_path: String,
    pub changes: Vec<String>,
}
