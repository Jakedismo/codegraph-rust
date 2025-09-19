/// MCP Tools Schema Definitions for CodeGraph with Qwen2.5-Coder Integration
use serde_json::{json, Value};

/// Get list of available CodeGraph MCP tools
pub fn get_tools_list() -> Value {
    json!([
        {
            "name": "codegraph.enhanced_search",
            "description": "Enhanced semantic search with Qwen2.5-Coder intelligence analysis",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "Natural language description of functionality to search for"
                    },
                    "include_analysis": {
                        "type": "boolean",
                        "default": true,
                        "description": "Include Qwen2.5-Coder AI analysis of results"
                    },
                    "max_results": {
                        "type": "integer",
                        "default": 10,
                        "minimum": 1,
                        "maximum": 50,
                        "description": "Maximum number of search results to return"
                    }
                },
                "required": ["query"]
            }
        },
        {
            "name": "codegraph.semantic_intelligence",
            "description": "Comprehensive codebase analysis using Qwen2.5-Coder's 128K context window",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "Analysis focus area or specific question about the codebase"
                    },
                    "task_type": {
                        "type": "string",
                        "enum": ["semantic_search", "comprehensive_analysis", "pattern_detection", "architecture_mapping"],
                        "default": "comprehensive_analysis",
                        "description": "Type of analysis to perform"
                    },
                    "max_context_tokens": {
                        "type": "integer",
                        "default": 80000,
                        "minimum": 10000,
                        "maximum": 120000,
                        "description": "Maximum context tokens to use (out of 128K available)"
                    }
                },
                "required": ["query"]
            }
        },
        {
            "name": "codegraph.impact_analysis",
            "description": "Analyze impact of proposed code changes using dependency mapping and Qwen intelligence",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "target_function": {
                        "type": "string",
                        "description": "Name of the function to analyze for impact"
                    },
                    "file_path": {
                        "type": "string",
                        "description": "Path to the file containing the target function"
                    },
                    "change_type": {
                        "type": "string",
                        "enum": ["modify", "delete", "rename", "move", "extract"],
                        "default": "modify",
                        "description": "Type of change being proposed"
                    }
                },
                "required": ["target_function", "file_path"]
            }
        },
        {
            "name": "codegraph.performance_metrics",
            "description": "Get real-time performance metrics for Qwen2.5-Coder operations",
            "inputSchema": {
                "type": "object",
                "properties": {},
                "additionalProperties": false
            }
        },
        {
            "name": "codegraph.cache_stats",
            "description": "Get intelligent cache statistics and performance analysis",
            "inputSchema": {
                "type": "object",
                "properties": {},
                "additionalProperties": false
            }
        },
        {
            "name": "vector.search",
            "description": "Basic vector similarity search without AI analysis",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "Search query text"
                    },
                    "paths": {
                        "type": "array",
                        "items": {"type": "string"},
                        "description": "Optional file paths to restrict search"
                    },
                    "langs": {
                        "type": "array",
                        "items": {"type": "string"},
                        "description": "Optional programming languages to filter"
                    },
                    "limit": {
                        "type": "integer",
                        "default": 10,
                        "description": "Maximum results to return"
                    }
                },
                "required": ["query"]
            }
        },
        {
            "name": "graph.neighbors",
            "description": "Get neighboring nodes in the code graph",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "node": {
                        "type": "string",
                        "description": "UUID of the target node"
                    },
                    "limit": {
                        "type": "integer",
                        "default": 20,
                        "description": "Maximum neighbors to return"
                    }
                },
                "required": ["node"]
            }
        },
        {
            "name": "graph.traverse",
            "description": "Traverse the code graph from a starting node",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "start": {
                        "type": "string",
                        "description": "UUID of the starting node"
                    },
                    "depth": {
                        "type": "integer",
                        "default": 2,
                        "description": "Maximum traversal depth"
                    },
                    "limit": {
                        "type": "integer",
                        "default": 100,
                        "description": "Maximum nodes to return"
                    }
                },
                "required": ["start"]
            }
        }
    ])
}

/// Get tools capability information for MCP initialization
pub fn get_tools_capability() -> Value {
    json!({
        "tools": {
            "listChanged": true
        }
    })
}

/// Generate example usage for each tool
pub fn get_tool_examples() -> Value {
    json!({
        "examples": {
            "codegraph.enhanced_search": {
                "description": "Find authentication-related code with AI analysis",
                "request": {
                    "query": "user authentication and login flow",
                    "include_analysis": true,
                    "max_results": 8
                },
                "expected_response": {
                    "search_results": "Array of semantic matches",
                    "ai_analysis": "Qwen2.5-Coder comprehensive analysis",
                    "generation_guidance": "How to generate similar code",
                    "quality_assessment": "Code quality evaluation"
                }
            },
            "codegraph.semantic_intelligence": {
                "description": "Comprehensive analysis of system architecture",
                "request": {
                    "query": "explain the overall system architecture and component relationships",
                    "task_type": "comprehensive_analysis",
                    "max_context_tokens": 100000
                },
                "expected_response": {
                    "comprehensive_analysis": "Detailed architectural analysis",
                    "structured_insights": "6-section structured analysis",
                    "generation_guidance": "Architectural guidance for new code"
                }
            },
            "codegraph.impact_analysis": {
                "description": "Analyze impact before modifying critical function",
                "request": {
                    "target_function": "validateCredentials",
                    "file_path": "src/auth/AuthService.ts",
                    "change_type": "modify"
                },
                "expected_response": {
                    "risk_assessment": "Risk level with detailed reasoning",
                    "affected_components": "List of impacted functions/tests/APIs",
                    "implementation_plan": "Step-by-step safe change process",
                    "safety_recommendations": "Rollback and validation strategies"
                }
            }
        }
    })
}

/// Get performance benchmarks for documentation
pub fn get_performance_benchmarks() -> Value {
    json!({
        "qwen_2_5_coder_14b_128k": {
            "model_specs": {
                "parameters": "14B",
                "context_window": 128000,
                "quantization": "Q4_K_M",
                "memory_requirement": "24GB VRAM",
                "fits_in": "32GB MacBook Pro"
            },
            "performance_targets": {
                "enhanced_search": "2-3 seconds",
                "semantic_intelligence": "4-6 seconds",
                "impact_analysis": "3-5 seconds",
                "pattern_detection": "3-4 seconds"
            },
            "quality_targets": {
                "confidence_threshold": 0.8,
                "context_utilization": 0.8,
                "analysis_accuracy": 0.9
            }
        }
    })
}