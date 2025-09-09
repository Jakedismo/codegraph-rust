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

This documentation is organized into four main categories to help you navigate the project effectively:

### 🏗️ [Architecture](architecture/)
Detailed architectural documentation and system design specifications.

### 🔧 [API Documentation](api/)
Complete API reference and integration guides.

### 📖 [Guides](guides/)
Step-by-step tutorials and development guides.

### 📋 [Specifications](specifications/)
Technical specifications, implementation plans, and detailed feature documentation.

---

## 🚀 Quick Navigation

### Getting Started
- **[Startup Guide](guides/startup.md)** - Get up and running with CodeGraph
- **[CI/CD Setup](guides/CI_CD_README.md)** - Setting up continuous integration

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

### Architecture Documentation
Located in `architecture/`, these documents describe the high-level system design:

| Document | Description |
|----------|-------------|
| [CODEGRAPH_RAG_ARCHITECTURE.md](architecture/CODEGRAPH_RAG_ARCHITECTURE.md) | RAG architecture for code understanding |
| [UNIFIED_ARCHITECTURE_SPECIFICATION.md](architecture/UNIFIED_ARCHITECTURE_SPECIFICATION.md) | Complete system architecture |
| [REST_API_ARCHITECTURE.md](architecture/REST_API_ARCHITECTURE.md) | API design and endpoints |

### API Documentation
Located in `api/`, covering integration and API usage:

| Document | Description |
|----------|-------------|
| [codegraph-mcp-spec.md](api/codegraph-mcp-spec.md) | Model Context Protocol specification |

### User Guides
Located in `guides/`, providing practical development guidance:

| Document | Description |
|----------|-------------|
| [startup.md](guides/startup.md) | Getting started with CodeGraph |
| [CI_CD_README.md](guides/CI_CD_README.md) | Continuous integration setup |

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

### By Use Case

**Setting up the project:**
1. [Startup Guide](guides/startup.md)
2. [CI/CD Setup](guides/CI_CD_README.md)
3. [Implementation Plan](specifications/IMPLEMENTATION_PLAN.md)

**Understanding the architecture:**
1. [Unified Architecture](architecture/UNIFIED_ARCHITECTURE_SPECIFICATION.md)
2. [RAG Architecture](architecture/CODEGRAPH_RAG_ARCHITECTURE.md)
3. [Technical Implementation](specifications/CodeGraph-Technical-Implementation.md)

**API integration:**
1. [REST API Architecture](architecture/REST_API_ARCHITECTURE.md)
2. [MCP Specification](api/codegraph-mcp-spec.md)
3. [RAG Integration](specifications/RAG_INTEGRATION_SPECIFICATIONS.md)

**Performance optimization:**
1. [Performance Benchmarks](specifications/performance_benchmarks.md)
2. [RocksDB Optimization](specifications/rocksdb_graph_storage_optimization.md)
3. [Vector Storage](specifications/PERSISTENT_VECTOR_STORAGE_SUMMARY.md)

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

1. Follow the existing structure and naming conventions
2. Update this index when adding new documents
3. Ensure proper cross-references between related documents
4. Use consistent markdown formatting
5. Include the PDF generation header block for markdown files

---

## 📞 Support

For questions about the documentation:
- Review the relevant sections above
- Check cross-references in individual documents
- Refer to the main [README.md](../README.md) for project overview

**Navigation:** [← Back to Main README](../README.md)