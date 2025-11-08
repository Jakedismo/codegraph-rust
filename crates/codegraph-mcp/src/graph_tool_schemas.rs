// ABOUTME: LLM tool schemas for SurrealDB graph analysis functions
// ABOUTME: JSON schemas for agentic tool calling - defines parameters and descriptions for LLM consumption

use serde::{Deserialize, Serialize};
use serde_json::{json, Value as JsonValue};

/// Tool schema for LLM function calling
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolSchema {
    pub name: String,
    pub description: String,
    pub parameters: JsonValue,
}

/// Collection of all graph analysis tool schemas
pub struct GraphToolSchemas;

impl GraphToolSchemas {
    /// Get all tool schemas for registration with LLM
    pub fn all() -> Vec<ToolSchema> {
        vec![
            Self::get_transitive_dependencies(),
            Self::detect_circular_dependencies(),
            Self::trace_call_chain(),
            Self::calculate_coupling_metrics(),
            Self::get_hub_nodes(),
            Self::get_reverse_dependencies(),
        ]
    }

    /// Schema for get_transitive_dependencies function
    pub fn get_transitive_dependencies() -> ToolSchema {
        ToolSchema {
            name: "get_transitive_dependencies".to_string(),
            description: "Get all transitive dependencies of a code node up to specified depth. \
                Follows dependency edges recursively to find all nodes this node depends on. \
                Useful for impact analysis and understanding dependency chains."
                .to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "node_id": {
                        "type": "string",
                        "description": "The ID of the code node to analyze (e.g., 'nodes:123' or ID extracted from search results)"
                    },
                    "edge_type": {
                        "type": "string",
                        "description": "Type of dependency relationship to follow",
                        "enum": ["Calls", "Imports", "Uses", "Extends", "Implements", "References", "Contains", "Defines"]
                    },
                    "depth": {
                        "type": "integer",
                        "description": "Maximum traversal depth (1-10, defaults to 3 if not specified)",
                        "minimum": 1,
                        "maximum": 10,
                        "default": 3
                    }
                },
                "required": ["node_id", "edge_type"]
            }),
        }
    }

    /// Schema for detect_circular_dependencies function
    pub fn detect_circular_dependencies() -> ToolSchema {
        ToolSchema {
            name: "detect_circular_dependencies".to_string(),
            description: "Detect circular dependencies in the codebase for a given edge type. \
                Returns pairs of nodes that have bidirectional relationships (A depends on B and B depends on A). \
                Critical for identifying architectural issues and potential cyclic import problems.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "edge_type": {
                        "type": "string",
                        "description": "Type of dependency relationship to analyze for cycles",
                        "enum": ["Calls", "Imports", "Uses", "Extends", "Implements", "References"]
                    }
                },
                "required": ["edge_type"]
            }),
        }
    }

    /// Schema for trace_call_chain function
    pub fn trace_call_chain() -> ToolSchema {
        ToolSchema {
            name: "trace_call_chain".to_string(),
            description:
                "Trace the execution call chain starting from a specific function or method. \
                Follows 'Calls' edges recursively to map which functions are invoked. \
                Essential for understanding control flow and execution paths through the codebase."
                    .to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "from_node": {
                        "type": "string",
                        "description": "The ID of the starting function/method node (extracted from search results)"
                    },
                    "max_depth": {
                        "type": "integer",
                        "description": "Maximum call chain depth to traverse (1-10, defaults to 5 for call chains)",
                        "minimum": 1,
                        "maximum": 10,
                        "default": 5
                    }
                },
                "required": ["from_node"]
            }),
        }
    }

    /// Schema for calculate_coupling_metrics function
    pub fn calculate_coupling_metrics() -> ToolSchema {
        ToolSchema {
            name: "calculate_coupling_metrics".to_string(),
            description: "Calculate architectural coupling metrics for a code node. \
                Returns afferent coupling (Ca = incoming dependencies), efferent coupling (Ce = outgoing dependencies), \
                and instability (I = Ce/(Ce+Ca), where 0=stable, 1=unstable). \
                Use this to assess architectural quality and identify problematic coupling patterns.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "node_id": {
                        "type": "string",
                        "description": "The ID of the code node to analyze (extracted from search results)"
                    }
                },
                "required": ["node_id"]
            }),
        }
    }

    /// Schema for get_hub_nodes function
    pub fn get_hub_nodes() -> ToolSchema {
        ToolSchema {
            name: "get_hub_nodes".to_string(),
            description: "Identify highly connected hub nodes in the code graph with degree >= min_degree. \
                Returns nodes sorted by total degree (incoming + outgoing connections) in descending order. \
                Useful for finding architectural hotspots, central components, and potential bottlenecks or god objects.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "min_degree": {
                        "type": "integer",
                        "description": "Minimum total degree (incoming + outgoing connections) to qualify as a hub (defaults to 5)",
                        "minimum": 1,
                        "default": 5
                    }
                },
                "required": []
            }),
        }
    }

    /// Schema for get_reverse_dependencies function
    pub fn get_reverse_dependencies() -> ToolSchema {
        ToolSchema {
            name: "get_reverse_dependencies".to_string(),
            description: "Get all reverse dependencies (dependents) of a code node - nodes that depend ON this node. \
                Follows incoming dependency edges recursively up to specified depth. \
                Critical for change impact analysis - shows what will be affected if you modify this node.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "node_id": {
                        "type": "string",
                        "description": "The ID of the code node to analyze (extracted from search results)"
                    },
                    "edge_type": {
                        "type": "string",
                        "description": "Type of dependency relationship to follow backwards",
                        "enum": ["Calls", "Imports", "Uses", "Extends", "Implements", "References"]
                    },
                    "depth": {
                        "type": "integer",
                        "description": "Maximum traversal depth for reverse dependencies (1-10, defaults to 3)",
                        "minimum": 1,
                        "maximum": 10,
                        "default": 3
                    }
                },
                "required": ["node_id", "edge_type"]
            }),
        }
    }

    /// Get schema by name
    pub fn get_by_name(name: &str) -> Option<ToolSchema> {
        Self::all().into_iter().find(|s| s.name == name)
    }

    /// Get all tool names
    pub fn tool_names() -> Vec<String> {
        Self::all().into_iter().map(|s| s.name).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_schemas_valid() {
        let schemas = GraphToolSchemas::all();
        assert_eq!(schemas.len(), 6, "Should have exactly 6 tool schemas");

        for schema in schemas {
            assert!(!schema.name.is_empty(), "Schema name should not be empty");
            assert!(
                !schema.description.is_empty(),
                "Schema description should not be empty"
            );
            assert!(
                schema.parameters.is_object(),
                "Schema parameters should be an object"
            );
        }
    }

    #[test]
    fn test_get_by_name() {
        let schema = GraphToolSchemas::get_by_name("get_transitive_dependencies");
        assert!(schema.is_some());
        assert_eq!(schema.unwrap().name, "get_transitive_dependencies");

        let missing = GraphToolSchemas::get_by_name("nonexistent");
        assert!(missing.is_none());
    }

    #[test]
    fn test_tool_names() {
        let names = GraphToolSchemas::tool_names();
        assert_eq!(names.len(), 6);
        assert!(names.contains(&"get_transitive_dependencies".to_string()));
        assert!(names.contains(&"detect_circular_dependencies".to_string()));
        assert!(names.contains(&"trace_call_chain".to_string()));
        assert!(names.contains(&"calculate_coupling_metrics".to_string()));
        assert!(names.contains(&"get_hub_nodes".to_string()));
        assert!(names.contains(&"get_reverse_dependencies".to_string()));
    }

    #[test]
    fn test_schema_serialization() {
        let schema = GraphToolSchemas::get_transitive_dependencies();
        let json = serde_json::to_string(&schema).unwrap();
        assert!(json.contains("get_transitive_dependencies"));
        assert!(json.contains("node_id"));
        assert!(json.contains("edge_type"));
    }
}
