---
pdf-engine: lualatex
mainfont: "DejaVu Serif"
monofont: "DejaVu Sans Mono"
header-includes: |
  \usepackage{fontspec}
  \directlua{
    luaotfload.add_fallback("emojifallback", {"NotoColorEmoji:mode=harf;"})
  }
  \setmainfont[
    RawFeature={fallback=emojifallback}
  ]{DejaVu Serif}
---

# CodeGraph Documentation Hub

Welcome to the comprehensive documentation for CodeGraph, a sophisticated code analysis and embedding system built in Rust.

## 📋 Documentation Overview

This documentation is organized into seven main categories to help you navigate the project effectively:

### 🚀 [Getting Started](guides/getting-started.md)
Your first steps with CodeGraph - installation, setup, and basic usage.

### 📖 [Tutorials](tutorials/)
Step-by-step learning path from beginner to advanced usage patterns.

### 📚 [Examples](examples/)
Practical examples and code samples for common use cases.

### 🏗️ [Architecture](architecture/)
Detailed architectural documentation and system design specifications.

### 🔧 [API Documentation](api/)
Complete API reference and integration guides.

### 📋 [Specifications](specifications/)
Technical specifications, implementation plans, and detailed feature documentation.

### 🔍 [Reference](reference/)
Complete reference documentation for APIs, configuration, and components.

### 🛠️ [Troubleshooting](troubleshooting/)
Common issues, solutions, and debugging techniques.

---

## 🚀 Quick Navigation

### New to CodeGraph?
- **[Getting Started Guide](guides/getting-started.md)** - Complete setup and first analysis
- **[Your First Analysis Tutorial](tutorials/)** - Step-by-step learning
- **[Basic Examples](examples/)** - Ready-to-run code samples

### Need Help?
- **[Troubleshooting Guide](troubleshooting/)** - Common issues and solutions
- **[API Reference](reference/)** - Complete API documentation
- **[Configuration Guide](reference/#configuration-reference)** - Setup and tuning

### Development
- **[CI/CD Setup](guides/CI_CD_README.md)** - Setting up continuous integration
- **[Architecture Overview](architecture/UNIFIED_ARCHITECTURE_SPECIFICATION.md)** - System design

### Architecture & Design
- **[CodeGraph RAG Architecture](architecture/CODEGRAPH_RAG_ARCHITECTURE.md)** - Retrieval Augmented Generation architecture
- **[Unified Architecture Specification](architecture/UNIFIED_ARCHITECTURE_SPECIFICATION.md)** - Complete system architecture
- **[REST API Architecture](architecture/REST_API_ARCHITECTURE.md)** - API design and structure

### Implementation Details
- **[Technical Implementation](specifications/CodeGraph-Technical-Implementation.md)** - Core implementation details
- **[Implementation Plan](specifications/IMPLEMENTATION_PLAN.md)** - Development roadmap
- **[Phase 1 Roadmap](specifications/PHASE_1_IMPLEMENTATION_ROADMAP.md)** - Initial phase development plan

### System Specifications
- **[RAG Integration](specifications/RAG_INTEGRATION_SPECIFICATIONS.md)** - RAG system integration specifications
- **[Feature Inventory](specifications/FEATURE_INVENTORY.md)** - Complete feature catalog
- **[Embedding System](specifications/CodeGraphEmbeddingSystem.md)** - Vector embedding implementation

### Storage & Performance
- **[Vector Storage](specifications/PERSISTENT_VECTOR_STORAGE_SUMMARY.md)** - Persistent vector storage design
- **[Performance Benchmarks](specifications/performance_benchmarks.md)** - System performance analysis
- **[RocksDB Optimization](specifications/rocksdb_graph_storage_optimization.md)** - Graph storage optimization

### Advanced Features
- **[FAISS Implementation](specifications/FAISS_IMPLEMENTATION.md)** - FAISS vector search integration
- **[Session Memory](specifications/SESSION-MEMORY.md)** - Session management system
- **[Versioning System](specifications/VERSIONING_SYSTEM_SUMMARY.md)** - Version control integration

---

## 📚 Documentation Categories

### 🚀 Getting Started & Learning
The best path for new users to learn CodeGraph:

| Section | Description | Best For |
|---------|-------------|----------|
| [Getting Started](guides/getting-started.md) | Installation, setup, first analysis | New users |
| [Tutorials](tutorials/) | Progressive learning path | All skill levels |
| [Examples](examples/) | Practical code samples | Implementation reference |

### 🏗️ Architecture & Design
Located in `architecture/`, these documents describe the high-level system design:

| Document | Description |
|----------|-------------|
| [CODEGRAPH_RAG_ARCHITECTURE.md](architecture/CODEGRAPH_RAG_ARCHITECTURE.md) | RAG architecture for code understanding |
| [UNIFIED_ARCHITECTURE_SPECIFICATION.md](architecture/UNIFIED_ARCHITECTURE_SPECIFICATION.md) | Complete system architecture |
| [REST_API_ARCHITECTURE.md](architecture/REST_API_ARCHITECTURE.md) | API design and endpoints |

### 🔧 API & Integration
Located in `api/`, covering integration and API usage:

| Document | Description |
|----------|-------------|
| [codegraph-mcp-spec.md](api/codegraph-mcp-spec.md) | Model Context Protocol specification |

### 📖 User Guides
Located in `guides/`, providing practical development guidance:

| Document | Description |
|----------|-------------|
| [getting-started.md](guides/getting-started.md) | Complete getting started guide |
| [CI_CD_README.md](guides/CI_CD_README.md) | Continuous integration setup |

### 🔍 Reference Documentation
Located in `reference/`, complete technical reference:

| Document | Description |
|----------|-------------|
| [API Reference](reference/#api-reference) | Complete API documentation |
| [Configuration Reference](reference/#configuration-reference) | All configuration options |
| [Command Line Reference](reference/#command-line-reference) | CLI usage and options |
| [Language Support](reference/#language-support-reference) | Supported languages and features |

### 🛠️ Support & Troubleshooting
Located in `troubleshooting/`, for when things go wrong:

| Section | Description |
|---------|-------------|
| [Common Issues](troubleshooting/#common-issues) | Frequently encountered problems |
| [Performance Issues](troubleshooting/#performance-issues) | Optimization and tuning |
| [Debugging Techniques](troubleshooting/#debugging-techniques) | Debug and profiling tools |

### Technical Specifications
Located in `specifications/`, containing detailed technical documentation:

| Document | Description |
|----------|-------------|
| [RAG_INTEGRATION_SPECIFICATIONS.md](specifications/RAG_INTEGRATION_SPECIFICATIONS.md) | RAG system integration specs |
| [IMPLEMENTATION_PLAN.md](specifications/IMPLEMENTATION_PLAN.md) | Development implementation plan |
| [PHASE_1_IMPLEMENTATION_ROADMAP.md](specifications/PHASE_1_IMPLEMENTATION_ROADMAP.md) | Phase 1 development roadmap |
| [FEATURE_INVENTORY.md](specifications/FEATURE_INVENTORY.md) | Complete feature catalog |
| [CodeGraph-Technical-Implementation.md](specifications/CodeGraph-Technical-Implementation.md) | Core technical implementation |
| [CodeGraphEmbeddingSystem.md](specifications/CodeGraphEmbeddingSystem.md) | Embedding system design |
| [FAISS_IMPLEMENTATION.md](specifications/FAISS_IMPLEMENTATION.md) | FAISS integration details |
| [PERSISTENT_VECTOR_STORAGE_SUMMARY.md](specifications/PERSISTENT_VECTOR_STORAGE_SUMMARY.md) | Vector storage implementation |
| [VERSIONING_SYSTEM_SUMMARY.md](specifications/VERSIONING_SYSTEM_SUMMARY.md) | Version control integration |
| [SESSION-MEMORY.md](specifications/SESSION-MEMORY.md) | Session management system |
| [performance_benchmarks.md](specifications/performance_benchmarks.md) | Performance analysis and benchmarks |
| [rocksdb_graph_storage_optimization.md](specifications/rocksdb_graph_storage_optimization.md) | RocksDB optimization strategies |

---

## 🔍 Finding What You Need

### By Experience Level

**👶 Complete Beginner**
1. [Getting Started Guide](guides/getting-started.md) - Start here!
2. [First Analysis Tutorial](tutorials/) - Your first CodeGraph project
3. [Basic Examples](examples/) - Simple, runnable examples
4. [Troubleshooting](troubleshooting/) - When things go wrong

**👨‍💻 Experienced Developer**  
1. [Architecture Overview](architecture/UNIFIED_ARCHITECTURE_SPECIFICATION.md) - System design
2. [API Reference](reference/) - Complete technical reference
3. [Advanced Examples](examples/) - Real-world integration patterns
4. [Performance Guide](troubleshooting/#performance-issues) - Optimization techniques

**🏗️ System Architect**
1. [Complete Architecture Spec](architecture/UNIFIED_ARCHITECTURE_SPECIFICATION.md) - Full system design
2. [RAG Architecture](architecture/CODEGRAPH_RAG_ARCHITECTURE.md) - AI integration patterns  
3. [Specifications](specifications/) - Detailed technical specifications
4. [Production Deployment](tutorials/) - Enterprise deployment patterns

### By Use Case

**🚀 Setting up the project:**
1. [Getting Started Guide](guides/getting-started.md)
2. [CI/CD Setup](guides/CI_CD_README.md)
3. [Implementation Plan](specifications/IMPLEMENTATION_PLAN.md)

**🏗️ Understanding the architecture:**
1. [Unified Architecture](architecture/UNIFIED_ARCHITECTURE_SPECIFICATION.md)
2. [RAG Architecture](architecture/CODEGRAPH_RAG_ARCHITECTURE.md)
3. [Technical Implementation](specifications/CodeGraph-Technical-Implementation.md)

**🔌 API integration:**
1. [REST API Architecture](architecture/REST_API_ARCHITECTURE.md)
2. [MCP Specification](api/codegraph-mcp-spec.md)
3. [RAG Integration](specifications/RAG_INTEGRATION_SPECIFICATIONS.md)
4. [Integration Examples](examples/) - Practical integration samples

**⚡ Performance optimization:**
1. [Performance Benchmarks](specifications/performance_benchmarks.md)
2. [RocksDB Optimization](specifications/rocksdb_graph_storage_optimization.md)
3. [Vector Storage](specifications/PERSISTENT_VECTOR_STORAGE_SUMMARY.md)
4. [Performance Troubleshooting](troubleshooting/#performance-issues)

**🔍 Code Analysis & Search:**
1. [Vector Embeddings Tutorial](tutorials/) - Semantic code search
2. [Multi-language Examples](examples/) - Cross-language analysis
3. [Custom Analysis Examples](examples/) - Domain-specific tools

**🛠️ Development & Integration:**
1. [Development Tutorials](tutorials/) - Build custom tools
2. [Integration Patterns](examples/) - Common integration scenarios
3. [API Examples](examples/) - REST API usage patterns

### By Component

**Core System:**
- [Technical Implementation](specifications/CodeGraph-Technical-Implementation.md)
- [Unified Architecture](architecture/UNIFIED_ARCHITECTURE_SPECIFICATION.md)

**Vector Search:**
- [Embedding System](specifications/CodeGraphEmbeddingSystem.md)
- [FAISS Implementation](specifications/FAISS_IMPLEMENTATION.md)
- [Vector Storage](specifications/PERSISTENT_VECTOR_STORAGE_SUMMARY.md)

**Storage & Performance:**
- [RocksDB Optimization](specifications/rocksdb_graph_storage_optimization.md)
- [Performance Benchmarks](specifications/performance_benchmarks.md)

**API & Integration:**
- [REST API Architecture](architecture/REST_API_ARCHITECTURE.md)
- [MCP Specification](api/codegraph-mcp-spec.md)
- [RAG Integration](specifications/RAG_INTEGRATION_SPECIFICATIONS.md)

---

## 🤝 Contributing to Documentation

When updating documentation:

1. **Follow the existing structure** and naming conventions
2. **Update this index** when adding new documents  
3. **Ensure proper cross-references** between related documents
4. **Use consistent markdown formatting** throughout
5. **Include the PDF generation header block** for markdown files
6. **Test all code examples** thoroughly before publishing
7. **Update navigation links** in related documents

### Documentation Categories
- **Tutorials**: Step-by-step learning content
- **Examples**: Practical, runnable code samples
- **Guides**: Task-oriented documentation
- **Reference**: Complete API and configuration docs
- **Architecture**: System design documentation
- **Specifications**: Detailed technical specs
- **Troubleshooting**: Problem-solving guides

---

## 📞 Support & Next Steps

### Getting Help
- **New to CodeGraph?** Start with the [Getting Started Guide](guides/getting-started.md)
- **Need examples?** Browse the [Examples](examples/) directory
- **Having issues?** Check the [Troubleshooting Guide](troubleshooting/)
- **Want to learn more?** Follow the [Tutorials](tutorials/) path
- **Need technical details?** See the [Reference](reference/) documentation

### Additional Resources
- **Main Project**: [README.md](../README.md) for project overview
- **Development**: [CLAUDE.md](../CLAUDE.md) for development guidelines
- **GitHub Issues**: Report bugs and request features
- **Community**: Join discussions and get support

### Documentation Overview
| Section | Purpose | Best For |
|---------|---------|----------|
| [Getting Started](guides/getting-started.md) | First-time setup | New users |
| [Tutorials](tutorials/) | Progressive learning | All skill levels |
| [Examples](examples/) | Practical samples | Implementation |
| [Reference](reference/) | Complete API docs | Developers |
| [Troubleshooting](troubleshooting/) | Problem solving | When stuck |

**Navigation:** [← Back to Main README](../README.md) | [Getting Started →](guides/getting-started.md)