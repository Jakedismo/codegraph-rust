// ABOUTME: JSON schemas for structured agentic tool outputs enforcing file paths
// ABOUTME: Combines freeform analysis with structured component/dependency data

use schemars::{schema_for, JsonSchema};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::llm_provider::{JsonSchema as LLMJsonSchema, ResponseFormat};

/// Common file location reference with line number
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct FileLocation {
    /// Component/symbol name
    pub name: String,
    /// Absolute or relative file path
    pub file_path: String,
    /// Line number where the component is defined
    pub line_number: Option<usize>,
    /// Optional brief description of the component's role
    pub description: Option<String>,
}

/// Dependency relationship between two components
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DependencyLink {
    /// Source component name
    pub from_name: String,
    /// Source file location
    pub from_file: String,
    /// Source line number
    pub from_line: Option<usize>,
    /// Target component name
    pub to_name: String,
    /// Target file location
    pub to_file: String,
    /// Target line number
    pub to_line: Option<usize>,
    /// Dependency type (e.g., "import", "call", "extends")
    pub dependency_type: String,
}

/// Structured output for agentic_code_search
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CodeSearchOutput {
    /// Natural language analysis of search results
    pub analysis: String,
    /// Relevant code components found
    pub components: Vec<FileLocation>,
    /// Key patterns or insights discovered
    pub patterns: Vec<String>,
}

/// Structured output for agentic_dependency_analysis
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DependencyAnalysisOutput {
    /// Natural language dependency analysis
    pub analysis: String,
    /// Components involved in the dependency graph
    pub components: Vec<FileLocation>,
    /// Dependency relationships
    pub dependencies: Vec<DependencyLink>,
    /// Circular dependencies detected (if any)
    pub circular_dependencies: Vec<Vec<String>>,
    /// Depth of dependency tree analyzed
    pub max_depth_analyzed: usize,
}

/// Call chain step in execution flow
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CallChainStep {
    /// Step number in the call chain
    pub step: usize,
    /// Function/method name
    pub function_name: String,
    /// File location
    pub file_path: String,
    /// Line number
    pub line_number: Option<usize>,
    /// What this step does
    pub action: String,
}

/// Structured output for agentic_call_chain_analysis
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CallChainOutput {
    /// Natural language analysis of execution flow
    pub analysis: String,
    /// Entry point of the call chain
    pub entry_point: FileLocation,
    /// Ordered call chain steps
    pub call_chain: Vec<CallChainStep>,
    /// Key decision points or branches
    pub decision_points: Vec<FileLocation>,
}

/// Architecture layer in the system
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ArchitectureLayer {
    /// Layer name (e.g., "Presentation", "Business Logic", "Data Access")
    pub name: String,
    /// Components in this layer
    pub components: Vec<FileLocation>,
    /// Responsibilities of this layer
    pub responsibilities: Vec<String>,
}

/// Coupling metric for a component
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CouplingMetric {
    /// Component being measured
    pub component: FileLocation,
    /// Afferent coupling (incoming dependencies)
    pub afferent_coupling: usize,
    /// Efferent coupling (outgoing dependencies)
    pub efferent_coupling: usize,
    /// Instability metric (efferent / (afferent + efferent))
    pub instability: f64,
}

/// Structured output for agentic_architecture_analysis
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ArchitectureAnalysisOutput {
    /// Natural language architecture analysis
    pub analysis: String,
    /// Architectural layers identified
    pub layers: Vec<ArchitectureLayer>,
    /// Hub nodes (highly connected components)
    pub hub_nodes: Vec<FileLocation>,
    /// Coupling metrics for key components
    pub coupling_metrics: Vec<CouplingMetric>,
    /// Architectural patterns detected
    pub patterns: Vec<String>,
    /// Architectural issues or smells
    pub issues: Vec<String>,
}

/// Public API endpoint or interface
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct APIEndpoint {
    /// Function/class/interface name
    pub name: String,
    /// File location
    pub file_path: String,
    /// Line number
    pub line_number: Option<usize>,
    /// API type (e.g., "HTTP endpoint", "public function", "exported class")
    pub api_type: String,
    /// Brief description of what it does
    pub description: String,
    /// Dependencies this endpoint relies on
    pub dependencies: Vec<String>,
}

/// Structured output for agentic_api_surface_analysis
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct APISurfaceOutput {
    /// Natural language API surface analysis
    pub analysis: String,
    /// Public API endpoints/interfaces
    pub endpoints: Vec<APIEndpoint>,
    /// API usage patterns
    pub usage_patterns: Vec<String>,
    /// Integration points with external systems
    pub integration_points: Vec<FileLocation>,
}

/// Structured output for agentic_context_builder
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ContextBuilderOutput {
    /// Natural language context summary
    pub analysis: String,
    /// Core components in this context
    pub core_components: Vec<FileLocation>,
    /// Dependency tree structure
    pub dependency_tree: DependencyAnalysisOutput,
    /// Execution flows
    pub execution_flows: Vec<CallChainOutput>,
    /// Architectural context
    pub architecture: ArchitectureAnalysisOutput,
    /// Related documentation or comments
    pub documentation_references: Vec<String>,
}

/// Structured output for agentic_semantic_question
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SemanticQuestionOutput {
    /// Direct answer to the question
    pub answer: String,
    /// Supporting evidence with file locations
    pub evidence: Vec<FileLocation>,
    /// Related components that provide context
    pub related_components: Vec<FileLocation>,
    /// Confidence level (0.0 to 1.0)
    pub confidence: f64,
}

/// Unified output type for all agentic tools
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AgenticOutput {
    CodeSearch(CodeSearchOutput),
    DependencyAnalysis(DependencyAnalysisOutput),
    CallChain(CallChainOutput),
    ArchitectureAnalysis(ArchitectureAnalysisOutput),
    APISurface(APISurfaceOutput),
    ContextBuilder(ContextBuilderOutput),
    SemanticQuestion(SemanticQuestionOutput),
}

impl AgenticOutput {
    /// Get the natural language analysis from any output type
    pub fn analysis(&self) -> &str {
        match self {
            Self::CodeSearch(o) => &o.analysis,
            Self::DependencyAnalysis(o) => &o.analysis,
            Self::CallChain(o) => &o.analysis,
            Self::ArchitectureAnalysis(o) => &o.analysis,
            Self::APISurface(o) => &o.analysis,
            Self::ContextBuilder(o) => &o.analysis,
            Self::SemanticQuestion(o) => &o.answer,
        }
    }

    /// Extract all file locations from any output type
    pub fn all_file_locations(&self) -> Vec<&FileLocation> {
        match self {
            Self::CodeSearch(o) => o.components.iter().collect(),
            Self::DependencyAnalysis(o) => o.components.iter().collect(),
            Self::CallChain(o) => {
                let mut locs = vec![&o.entry_point];
                locs.extend(o.decision_points.iter());
                locs
            }
            Self::ArchitectureAnalysis(o) => {
                let mut locs: Vec<&FileLocation> = o.hub_nodes.iter().collect();
                locs.extend(o.layers.iter().flat_map(|l| l.components.iter()));
                locs.extend(o.coupling_metrics.iter().map(|m| &m.component));
                locs
            }
            Self::APISurface(o) => o.integration_points.iter().collect(),
            Self::ContextBuilder(o) => {
                let mut locs = o.core_components.iter().collect::<Vec<_>>();
                locs.extend(o.dependency_tree.components.iter());
                locs
            }
            Self::SemanticQuestion(o) => {
                let mut locs = o.evidence.iter().collect::<Vec<_>>();
                locs.extend(o.related_components.iter());
                locs
            }
        }
    }
}

/// Helper to convert schemars schema to JSON value
fn schema_to_json_value<T: JsonSchema>() -> Value {
    let schema = schema_for!(T);
    serde_json::to_value(schema).expect("Failed to serialize schema")
}

/// Generate ResponseFormat for code search
pub fn code_search_response_format() -> ResponseFormat {
    ResponseFormat::JsonSchema {
        json_schema: LLMJsonSchema {
            name: "code_search_output".to_string(),
            schema: schema_to_json_value::<CodeSearchOutput>(),
            strict: true,
        },
    }
}

/// Generate ResponseFormat for dependency analysis
pub fn dependency_analysis_response_format() -> ResponseFormat {
    ResponseFormat::JsonSchema {
        json_schema: LLMJsonSchema {
            name: "dependency_analysis_output".to_string(),
            schema: schema_to_json_value::<DependencyAnalysisOutput>(),
            strict: true,
        },
    }
}

/// Generate ResponseFormat for call chain analysis
pub fn call_chain_response_format() -> ResponseFormat {
    ResponseFormat::JsonSchema {
        json_schema: LLMJsonSchema {
            name: "call_chain_output".to_string(),
            schema: schema_to_json_value::<CallChainOutput>(),
            strict: true,
        },
    }
}

/// Generate ResponseFormat for architecture analysis
pub fn architecture_analysis_response_format() -> ResponseFormat {
    ResponseFormat::JsonSchema {
        json_schema: LLMJsonSchema {
            name: "architecture_analysis_output".to_string(),
            schema: schema_to_json_value::<ArchitectureAnalysisOutput>(),
            strict: true,
        },
    }
}

/// Generate ResponseFormat for API surface analysis
pub fn api_surface_response_format() -> ResponseFormat {
    ResponseFormat::JsonSchema {
        json_schema: LLMJsonSchema {
            name: "api_surface_output".to_string(),
            schema: schema_to_json_value::<APISurfaceOutput>(),
            strict: true,
        },
    }
}

/// Generate ResponseFormat for context builder
pub fn context_builder_response_format() -> ResponseFormat {
    ResponseFormat::JsonSchema {
        json_schema: LLMJsonSchema {
            name: "context_builder_output".to_string(),
            schema: schema_to_json_value::<ContextBuilderOutput>(),
            strict: true,
        },
    }
}

/// Generate ResponseFormat for semantic question
pub fn semantic_question_response_format() -> ResponseFormat {
    ResponseFormat::JsonSchema {
        json_schema: LLMJsonSchema {
            name: "semantic_question_output".to_string(),
            schema: schema_to_json_value::<SemanticQuestionOutput>(),
            strict: true,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_code_search_schema() {
        let schema = schemars::schema_for!(CodeSearchOutput);
        assert!(schema.schema.object.is_some());

        // Verify required fields
        let obj = schema.schema.object.unwrap();
        assert!(obj.required.contains(&"analysis".to_string()));
        assert!(obj.required.contains(&"components".to_string()));
    }

    #[test]
    fn test_file_location_required_fields() {
        let schema = schemars::schema_for!(FileLocation);
        let obj = schema.schema.object.unwrap();

        // file_path and name must be required
        assert!(obj.required.contains(&"name".to_string()));
        assert!(obj.required.contains(&"file_path".to_string()));
    }

    #[test]
    fn test_analysis_extraction() {
        let output = AgenticOutput::CodeSearch(CodeSearchOutput {
            analysis: "Test analysis".to_string(),
            components: vec![],
            patterns: vec![],
        });

        assert_eq!(output.analysis(), "Test analysis");
    }
}
