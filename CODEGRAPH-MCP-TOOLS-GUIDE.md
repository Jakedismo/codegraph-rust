# CodeGraph MCP Tools Usage Guidelines

## 🌍 **Universal Programming Language Support (11 Languages)**

CodeGraph provides **revolutionary AI intelligence** across the most popular programming languages:

### 🚀 **Tier 1: Advanced Semantic Analysis (8 Languages)**
- **Rust** - Complete ownership/borrowing analysis, trait relationships, async patterns
- **Python** - Type hints, docstrings, dynamic analysis patterns
- **JavaScript** - Modern ES6+, async/await, functional patterns
- **TypeScript** - Type system analysis, generics, interface relationships
- **Swift** - iOS/macOS development, SwiftUI patterns, protocol-oriented programming
- **C#** - .NET patterns, LINQ analysis, async/await, dependency injection
- **Ruby** - Rails patterns, metaprogramming, dynamic typing intelligence
- **PHP** - Laravel/Symfony patterns, namespace analysis, modern PHP features

### 🚀 **Tier 2: Basic Semantic Analysis (3 Languages)**
- **Go** - Goroutines, interfaces, package management
- **Java** - OOP patterns, annotations, Spring framework detection
- **C++** - Modern C++, templates, memory management patterns

**Revolutionary Total: 11 languages with AI-powered semantic analysis**

## 📋 **MCP Tools - Streamlined Essential Suite (8 Tools)**

### 🧠 AI Intelligence & Analysis Tools

#### 1. `enhanced_search` - AI-Powered Semantic Search
```
Description: Search your codebase with AI analysis. Finds code patterns, architectural insights, and team conventions. Use when you need intelligent analysis of search results.
Required: query (what to search for)
Optional: limit (max results, default 10)
Example: "Find all authentication-related code and explain the patterns"
```

#### 2. `semantic_intelligence` - Deep Architectural Analysis
```
Description: Perform deep architectural analysis of your entire codebase using AI. Explains system design, component relationships, and overall architecture. Use for understanding large codebases or documenting architecture.
Required: query (analysis focus)
Optional: task_type (analysis type, default 'semantic_search'), max_context_tokens (AI context limit, default 80000)
Example: "Explain the overall system architecture and component relationships"
```

#### 3. `impact_analysis` - Breaking Change Prediction
```
Description: Predict the impact of modifying a specific function or class. Shows what code depends on it and might break. Use before refactoring to avoid breaking changes.
Required: target_function (function/class name), file_path (path to file containing it)
Optional: change_type (type of change, default 'modify')
Example: "What would happen if I modify the authentication middleware?"
```

#### 4. `pattern_detection` - Team Convention Analysis
```
Description: Analyze your team's coding patterns and conventions. Detects naming conventions, code organization patterns, error handling styles, and quality metrics. Use to understand team standards or onboard new developers.
Required: No parameters required
Example: "Analyze the coding patterns and conventions in this codebase"
```

### 🔍 Advanced Search & Graph Navigation Tools

#### 5. `vector_search` - Fast Similarity Search
```
Description: Fast vector similarity search to find code similar to your query. Returns raw search results without AI analysis (faster than enhanced_search). Use for quick code discovery.
Required: query (what to find)
Optional: paths (filter by directories), langs (filter by languages), limit (max results, default 10)
Example: "Search for error handling patterns in the service layer"
```

#### 6. `graph_neighbors` - Code Dependency Analysis
```
Description: Find all code that depends on or is used by a specific code element. Shows dependencies, imports, and relationships. Use to understand code impact before refactoring.
Required: node (UUID from search results)
Optional: limit (max results, default 20)
Note: Get node UUIDs from vector_search or enhanced_search results
Example: "Find all code that depends on the UserService class"
```

#### 7. `graph_traverse` - Architectural Flow Exploration
```
Description: Follow dependency chains through your codebase to understand architectural flow and code relationships. Use to trace execution paths or understand system architecture.
Required: start (UUID from search results)
Optional: depth (how far to traverse, default 2), limit (max results, default 100)
Note: Get start UUIDs from vector_search or enhanced_search results
Example: "Trace the complete flow from API endpoint to database"
```

### 📊 Performance & System Analytics Tools

#### 8. `performance_metrics` - System Health Monitoring
```
Description: Get CodeGraph system performance metrics including cache hit rates, search performance, and AI model usage stats. Use to monitor system health or troubleshoot performance issues.
Required: No parameters required
Example: "Show me the performance metrics and optimization suggestions"
```

## 🚀 Usage Best Practices

### **Setup & Indexing**
- **Always index your codebase** before starting AI-assisted development with `codegraph init .` and `codegraph index . --recursive`
- **Automatic language detection** - Just run `codegraph index .` to process all 11 supported languages
- **Reindex periodically** after adding features, version upgrades, API changes etc.

### **Workflow Recommendations**
- **Use `pattern_detection`** first to understand team conventions and coding standards
- **Use `impact_analysis`** before making significant changes to predict breaking changes
- **Use `enhanced_search`** for AI-powered architectural understanding and code discovery
- **Use `semantic_intelligence`** for comprehensive codebase analysis and documentation
- **Use `vector_search`** for fast similarity-based code search across large codebases
- **Use `graph_neighbors` and `graph_traverse`** for dependency analysis and architectural exploration (requires UUIDs from search results)
- **Monitor system health** with `performance_metrics` for optimization insights

### 💡 **Tool Selection Guide**

**For Code Discovery**: `vector_search` (fast) → `enhanced_search` (AI analysis)
**For Architecture Understanding**: `semantic_intelligence` → `graph_traverse`
**For Refactoring Safety**: `impact_analysis` → `graph_neighbors`
**For Team Onboarding**: `pattern_detection` → `semantic_intelligence`

## 🛠️ **Quick Setup for New Projects**

```bash
# 1. Navigate to your project directory
cd /path/to/your/project

# 2. Initialize CodeGraph (one-time setup)
codegraph init .

# 3. Index your codebase (supports all 11 languages automatically)
codegraph index . --recursive

# 4. Start using CodeGraph tools in Claude Code!
# No configuration needed - works globally with any project
```

## 🎯 **Integration Notes**

- **Global Operation**: Works from any project directory without manual configuration
- **Zero Tool Overlap**: Designed to complement Claude Code's built-in tools
- **Universal Compatibility**: Supports 90%+ of popular programming languages
- **Framework Intelligence**: Automatically detects and analyzes framework-specific patterns

---

*This guide can be copied to any project's CLAUDE.md file for universal CodeGraph MCP tool access.*