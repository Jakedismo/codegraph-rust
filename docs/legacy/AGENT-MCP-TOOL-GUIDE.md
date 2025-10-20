# CodeGraph MCP Tools: Complete Agent Usage Guide

## 🎯 **Purpose**
This guide provides comprehensive instructions for AI agents on how to efficiently use CodeGraph MCP tools for maximum development productivity and codebase understanding.

## 📋 **Tool Selection Matrix**

### **🔍 When to Use Each Tool**

| **Task Category** | **Recommended Tool** | **Alternative** | **Performance** |
|-------------------|---------------------|-----------------|-----------------|
| **Code Discovery** | `vector_search` → `enhanced_search` | `pattern_detection` | Fast → Comprehensive |
| **Architecture Understanding** | `semantic_intelligence` → `graph_traverse` | `graph_neighbors` | Deep → Targeted |
| **Change Impact Analysis** | `impact_analysis` → `graph_neighbors` | `enhanced_search` | Precise → Broad |
| **Team Convention Analysis** | `pattern_detection` → `semantic_intelligence` | N/A | Instant → Detailed |
| **Natural Language Q&A** | `codebase_qa` → `enhanced_search` | `semantic_intelligence` | Conversational → Technical |
| **Documentation Generation** | `code_documentation` → `semantic_intelligence` | `enhanced_search` | AI-Generated → Analysis-Based |

---

## 🚀 **Tool-by-Tool Usage Guide**

### **1. 🔍 `vector_search` - Lightning-Fast Code Discovery**

**Purpose**: Find similar code across the entire codebase using semantic similarity

**When to Use**:
- Quick code discovery and exploration
- Finding examples of specific patterns
- Locating similar implementations
- Getting UUIDs for graph traversal tools

**Usage Examples**:
```json
// Find authentication-related code
{
  "query": "authentication login security",
  "limit": 5
}

// Find error handling patterns
{
  "query": "error handling try catch",
  "limit": 3,
  "langs": ["rust", "typescript"]
}

// Find specific function patterns
{
  "query": "async function database connection",
  "limit": 10,
  "paths": ["src/"]
}
```

**Expected Response**:
```json
{
  "results": [
    {
      "id": "uuid-12345",
      "name": "authenticate_user",
      "node_type": "Function",
      "path": "./src/auth.rs",
      "score": 0.85,
      "summary": "pub async fn authenticate_user(token: &str) -> Result<User>"
    }
  ]
}
```

**Best Practices**:
- Use descriptive keywords related to functionality
- Start broad, then narrow with filters if needed
- Use returned UUIDs for `graph_neighbors` and `graph_traverse`
- Performance: Sub-second response for 14K+ embedded entities

---

### **2. 🧠 `enhanced_search` - AI-Powered Semantic Analysis**

**Purpose**: Get comprehensive AI analysis with architectural insights and pattern detection

**When to Use**:
- Deep codebase understanding needed
- Complex architectural questions
- Pattern analysis with AI insights
- Quality assessment and recommendations

**Usage Examples**:
```json
// Comprehensive error handling analysis
{
  "query": "Analyze error handling patterns and recommend improvements",
  "limit": 8
}

// Architecture pattern discovery
{
  "query": "Find all authentication and authorization patterns in the system",
  "limit": 10
}

// Code quality assessment
{
  "query": "Identify potential security vulnerabilities and code quality issues",
  "limit": 15
}
```

**Expected Response**:
```json
{
  "ai_analysis": "The codebase demonstrates consistent error handling patterns using Result<T, E> types...",
  "generation_guidance": {
    "error_handling": "Error handling patterns identified",
    "patterns_to_follow": "Consistent Result type usage",
    "required_imports": "anyhow::Result, thiserror",
    "testing_approach": "Unit tests for error conditions"
  },
  "intelligence_metadata": {
    "model_used": "hf.co/unsloth/Qwen2.5-Coder-14B-Instruct-128K-GGUF:Q4_K_M",
    "processing_time_ms": 12721,
    "context_tokens": 302,
    "confidence_score": 0.9
  },
  "search_results": [/* specific code matches */]
}
```

**Best Practices**:
- Use specific, detailed queries for better AI analysis
- Expect 2-15 seconds processing time for comprehensive results
- Review `ai_analysis` for insights and `generation_guidance` for actionable advice
- High-value tool for architectural understanding

---

### **3. 🏗️ `semantic_intelligence` - Deep Architectural Analysis**

**Purpose**: Comprehensive codebase analysis with 128K context window for complete architectural understanding

**When to Use**:
- Understanding overall system architecture
- Analyzing component relationships and data flow
- Getting holistic codebase insights
- Preparing for major refactoring or feature development

**Usage Examples**:
```json
// Complete architectural analysis
{
  "query": "Explain the overall architecture and how different components interact",
  "task_type": "architecture_analysis",
  "max_context_tokens": 50000
}

// Data flow analysis
{
  "query": "Trace the data flow from API endpoints to database storage",
  "task_type": "data_flow_analysis",
  "max_context_tokens": 40000
}

// Dependency analysis
{
  "query": "Analyze the dependency structure and identify potential circular dependencies",
  "task_type": "dependency_analysis",
  "max_context_tokens": 60000
}
```

**Expected Response**:
```json
{
  "comprehensive_analysis": "The codebase follows a modular architecture with clear separation of concerns...",
  "structured_insights": {
    "architectural_context": "Layered architecture with API, service, and data layers",
    "patterns_detected": "Repository pattern, dependency injection, async/await",
    "quality_assessment": "Well-structured with good separation of concerns"
  },
  "model_performance": {
    "processing_time_ms": 4794,
    "context_tokens_used": 946,
    "confidence_score": 0.9
  }
}
```

**Best Practices**:
- Use for high-level architectural understanding
- Adjust `max_context_tokens` based on codebase size
- Expect 4-8 seconds for comprehensive analysis
- Best tool for onboarding to new codebases

---

### **4. ⚠️ `impact_analysis` - Breaking Change Prediction**

**Purpose**: Predict what will break before making changes to functions or classes

**When to Use**:
- Before refactoring functions or classes
- Planning breaking changes
- Understanding dependency cascades
- Risk assessment for modifications

**Usage Examples**:
```json
// Analyze function modification impact
{
  "target_function": "authenticate_user",
  "file_path": "src/auth.rs",
  "change_type": "modify"
}

// Assess class removal impact
{
  "target_function": "UserRepository",
  "file_path": "src/data/repository.rs",
  "change_type": "remove"
}

// Signature change impact
{
  "target_function": "process_payment",
  "file_path": "src/payments.rs",
  "change_type": "signature_change"
}
```

**Expected Response**:
```json
{
  "comprehensive_impact_analysis": "Modifying authenticate_user would affect 15 functions across 8 files...",
  "risk_assessment": {
    "level": "MEDIUM",
    "confidence": 0.85,
    "reasoning": "Function is widely used but has stable interface"
  },
  "affected_components": {
    "has_specific_components": true,
    "analysis": "API endpoints, middleware, and user service components affected"
  },
  "safety_recommendations": {
    "recommendations": "Create deprecation wrapper, update tests, staged rollout",
    "has_rollback_plan": true
  }
}
```

**Best Practices**:
- Always use before significant refactoring
- Provide exact function/class names and file paths
- Review risk assessment and safety recommendations
- Use for change planning and risk mitigation

---

### **5. 🔗 `graph_neighbors` - Dependency Relationship Exploration**

**Purpose**: Find all code that depends on or is used by a specific entity

**When to Use**:
- Understanding function/class dependencies
- Finding usage patterns
- Preparing for targeted refactoring
- Exploring code relationships

**Usage Examples**:
```json
// Find what depends on a function
{
  "node": "ba55fbbb-7570-49c9-b7eb-7d2799fc9072",
  "limit": 10
}

// Explore class relationships
{
  "node": "4bfdc0c7-1a30-42ac-8fa0-69c9c208ea69",
  "limit": 5
}
```

**Expected Response**:
```json
{
  "neighbors": [
    {
      "id": "related-uuid-1",
      "name": "calling_function",
      "node_type": "Function",
      "path": "./src/caller.rs",
      "relationship_type": "calls"
    }
  ]
}
```

**Best Practices**:
- Get UUIDs from `vector_search` or `enhanced_search` results
- Use for targeted dependency analysis
- Essential for understanding change impact
- Start with limit=5-10 to avoid overwhelming results

---

### **6. 🌊 `graph_traverse` - Architectural Flow Exploration**

**Purpose**: Follow dependency chains through the codebase to understand execution paths

**When to Use**:
- Tracing execution flows
- Understanding call chains
- Analyzing data flow paths
- Architectural exploration

**Usage Examples**:
```json
// Trace execution flow
{
  "start": "entry-point-uuid",
  "depth": 3,
  "limit": 20
}

// Analyze call chain
{
  "start": "function-uuid",
  "depth": 2,
  "limit": 15
}
```

**Expected Response**:
```json
{
  "nodes": [
    {
      "depth": 0,
      "id": "start-uuid",
      "name": "entry_function",
      "path": "./src/main.rs"
    },
    {
      "depth": 1,
      "id": "child-uuid",
      "name": "called_function",
      "path": "./src/service.rs"
    }
  ]
}
```

**Best Practices**:
- Start with depth=2-3 to avoid excessive results
- Use for understanding execution flows
- Combine with `graph_neighbors` for complete picture
- Excellent for architectural documentation

---

### **7. 📊 `pattern_detection` - Team Intelligence Analysis**

**Purpose**: Analyze coding patterns, conventions, and team standards

**When to Use**:
- Understanding team coding standards
- Onboarding to new codebases
- Consistency analysis
- Quality assessment

**Usage Examples**:
```json
// Analyze all patterns
{}

// No parameters needed - analyzes entire codebase
```

**Expected Response**:
```json
{
  "pattern_summary": {
    "consistency_score": 0.95,
    "overall_quality_score": 0.76,
    "total_patterns_detected": 5
  },
  "team_intelligence": {
    "patterns_detected": [
      {
        "name": "Error Handling Pattern",
        "confidence": 0.8,
        "frequency": 45,
        "quality_score": 0.8
      }
    ],
    "quality_metrics": {
      "consistency_score": 0.95,
      "best_practices_score": 0.72
    }
  },
  "actionable_insights": {
    "improvement_opportunities": [
      "Standardize error handling patterns",
      "Define consistent naming conventions"
    ]
  }
}
```

**Best Practices**:
- Use for codebase onboarding and quality assessment
- No parameters required - analyzes everything
- Review consistency scores and improvement opportunities
- Instant results with comprehensive insights

---

### **8. 🗣️ `codebase_qa` - Conversational AI (REVOLUTIONARY)**

**Purpose**: Ask natural language questions about the codebase and get intelligent responses

**When to Use**:
- Complex architectural questions
- Understanding system behavior
- Getting explanations in plain English
- Exploring unfamiliar code areas

**Usage Examples**:
```json
// Architectural understanding
{
  "question": "How does authentication work in this system?",
  "max_results": 10
}

// Impact exploration
{
  "question": "What would break if I change the database connection logic?",
  "max_results": 15
}

// Implementation guidance
{
  "question": "How should I implement caching based on existing patterns?",
  "max_results": 8
}

// Data flow analysis
{
  "question": "Explain the data flow from API request to database response",
  "max_results": 12
}
```

**Expected Response**:
```json
{
  "query_id": "uuid-response-id",
  "question": "How does authentication work in this system?",
  "answer": "The authentication system in this codebase follows a JWT-based approach with the following components: 1) Login endpoint validates credentials... 2) JWT middleware verifies tokens... 3) User context is maintained throughout requests...",
  "confidence": 0.87,
  "citations": [
    {
      "node_id": "auth-function-uuid",
      "name": "authenticate_user",
      "file_path": "./src/auth.rs",
      "line": 45,
      "relevance": 0.92
    }
  ],
  "processing_time_ms": 8500,
  "intelligence_level": "conversational_ai"
}
```

**Best Practices**:
- Ask specific, detailed questions for better responses
- Use for complex understanding that requires explanation
- Review citations for source verification
- Expect 5-15 seconds for comprehensive analysis
- Most powerful tool for architectural understanding

---

### **9. 📝 `code_documentation` - AI-Powered Documentation (REVOLUTIONARY)**

**Purpose**: Generate comprehensive documentation with graph context and dependency analysis

**When to Use**:
- Creating documentation for functions/classes/modules
- Understanding complex implementations
- Onboarding documentation
- API documentation generation

**Usage Examples**:
```json
// Comprehensive function documentation
{
  "target_name": "authenticate_user",
  "style": "comprehensive"
}

// Concise API documentation
{
  "target_name": "PaymentProcessor",
  "file_path": "src/payments.rs",
  "style": "concise"
}

// Tutorial-style documentation
{
  "target_name": "DatabaseConnection",
  "style": "tutorial"
}
```

**Expected Response**:
```json
{
  "target_name": "authenticate_user",
  "documentation": "## authenticate_user Function\n\n### Purpose\nValidates user credentials and returns authentication status...\n\n### Parameters\n- `token: &str` - JWT token to validate...\n\n### Returns\n- `Result<User, AuthError>` - Authenticated user or error...",
  "confidence": 0.89,
  "sources": [
    {
      "file": "./src/auth.rs",
      "line": 45,
      "relevance": 0.95,
      "context": "Main implementation"
    }
  ],
  "generation_method": "ai_powered_rag_documentation",
  "graph_context_used": true
}
```

**Best Practices**:
- Specify target name exactly as it appears in code
- Choose appropriate style: comprehensive, concise, tutorial
- Review sources for accuracy verification
- Use for generating high-quality documentation

---

## 🎯 **Situational Usage Patterns**

### **🔍 Code Exploration Workflow**
1. **Start**: `vector_search` to find relevant code
2. **Understand**: `enhanced_search` for AI analysis
3. **Explore**: `graph_neighbors` to see relationships
4. **Trace**: `graph_traverse` to follow execution paths

### **🏗️ Architecture Understanding Workflow**
1. **Overview**: `semantic_intelligence` for system understanding
2. **Patterns**: `pattern_detection` for team conventions
3. **Q&A**: `codebase_qa` for specific questions
4. **Document**: `code_documentation` for recording insights

### **⚠️ Change Planning Workflow**
1. **Impact**: `impact_analysis` for change prediction
2. **Dependencies**: `graph_neighbors` for affected components
3. **Flow**: `graph_traverse` for execution path analysis
4. **Verification**: `enhanced_search` for similar patterns

### **📚 Documentation Workflow**
1. **Generate**: `code_documentation` for initial draft
2. **Enhance**: `codebase_qa` for additional context
3. **Verify**: `semantic_intelligence` for completeness
4. **Validate**: `vector_search` for related examples

---

## ⚡ **Performance Expectations**

### **🔥 Fast Tools (< 1 second)**
- `vector_search`: Sub-second similarity matching
- `pattern_detection`: Instant team intelligence

### **🧠 AI Tools (2-15 seconds)**
- `enhanced_search`: 2-8 seconds for comprehensive analysis
- `semantic_intelligence`: 4-12 seconds for deep insights
- `impact_analysis`: 3-8 seconds for impact prediction
- `codebase_qa`: 5-15 seconds for conversational responses
- `code_documentation`: 6-20 seconds for generated docs

### **🔗 Graph Tools (< 2 seconds)**
- `graph_neighbors`: < 1 second for relationship exploration
- `graph_traverse`: 1-2 seconds for multi-hop traversal

---

## 🎯 **Agent Best Practices**

### **🚀 Optimal Tool Selection**
1. **Start Fast**: Use `vector_search` for quick discovery
2. **Go Deep**: Use AI tools for comprehensive understanding
3. **Be Specific**: Provide detailed queries for better results
4. **Use UUIDs**: Chain tools using returned node identifiers
5. **Verify Results**: Cross-reference with multiple tools when needed

### **📊 Efficiency Guidelines**
- **Large Codebases**: Start with `vector_search`, narrow with filters
- **New Codebases**: Begin with `semantic_intelligence` for overview
- **Specific Questions**: Use `codebase_qa` for natural language interaction
- **Change Planning**: Always use `impact_analysis` before modifications

### **🔄 Tool Chaining Patterns**
```
Discovery: vector_search → enhanced_search → graph_neighbors
Planning: impact_analysis → graph_traverse → codebase_qa
Documentation: semantic_intelligence → code_documentation → pattern_detection
```

---

## 🚨 **Common Pitfalls to Avoid**

### **❌ Don't Do This**:
- Using `enhanced_search` for simple lookups (use `vector_search`)
- Asking vague questions to `codebase_qa` ("tell me about the code")
- Using `graph_traverse` without limiting depth (performance impact)
- Ignoring confidence scores in AI responses
- Not using UUIDs for graph traversal tools

### **✅ Do This Instead**:
- Match tool capability to task complexity
- Ask specific, actionable questions
- Use appropriate depth/limit parameters
- Review confidence scores and citations
- Chain tools for comprehensive understanding

---

## 🎊 **Revolutionary Capabilities Summary**

### **🧠 What Makes CodeGraph Unique**:
1. **Conversational AI**: Natural language codebase interaction
2. **Single-Pass Processing**: 50% faster edge extraction
3. **AI Symbol Resolution**: 85-90% dependency linking success
4. **Comprehensive Analysis**: 128K context window understanding
5. **Real Relationships**: 25K+ actual dependency edges
6. **Instant Intelligence**: Sub-second similarity search

### **🚀 Maximum Value Approach**:
- Combine multiple tools for comprehensive understanding
- Use AI tools for complex questions, fast tools for discovery
- Leverage conversational AI for natural interaction
- Trust the revolutionary edge processing for accurate relationships
- Utilize the complete graph database for architectural insights

**Result**: Agents equipped with this guide can efficiently leverage CodeGraph's revolutionary AI capabilities for maximum development productivity and codebase mastery! 🚀