![CodeGraph](docs/assets/banner.png)

# CodeGraph

**Your codebase, understood.**

CodeGraph transforms your entire codebase into a semantically searchable knowledge graph that AI agents can actually *reason* about—not just grep through.

> **Ready to get started?** Jump to the [Installation Guide](docs/INSTALLATION_GUIDE.md) for step-by-step setup instructions.
>
> **Already set up?** See the [Usage Guide](docs/USAGE_GUIDE.md) for tips on getting the most out of CodeGraph with your AI assistant.

---

## The Problem

AI coding assistants are powerful, but they're flying blind. They see files one at a time, grep for patterns, and burn tokens trying to understand your architecture. Every conversation starts from zero.

**What if your AI assistant already knew your codebase?**

---

## What CodeGraph Does Differently

### 1. Graph + Embeddings = True Understanding

Most semantic search tools create embeddings and call it a day. CodeGraph builds a **real knowledge graph**:

```
Your Code → AST + FastML → Graph Construction → Vector Embeddings
                ↓                  ↓                    ↓
           Functions          Dependencies        Semantic Search
           Classes            Call chains         Similarity
           Modules            Data flow           Context
```

When you search, you don't just get "similar code"—you get code with its **relationships intact**. The function that matches your query, plus what calls it, what it depends on, and where it fits in the architecture.

### 2. Agentic Tools, Not Just Search

CodeGraph doesn't return a list of files and wish you luck. It ships **7 agentic tools** that do the thinking:

| Tool | What It Actually Does |
|------|----------------------|
| `agentic_code_search` | Multi-step semantic search with AI-synthesized answers |
| `agentic_dependency_analysis` | Maps impact before you touch anything |
| `agentic_call_chain_analysis` | Traces execution paths through your system |
| `agentic_architecture_analysis` | Gives you the 10,000-foot view |
| `agentic_api_surface_analysis` | Understands your public interfaces |
| `agentic_context_builder` | Gathers everything needed for a feature |
| `agentic_semantic_question` | Answers complex questions about your code |

Each tool runs a **reasoning agent** (ReAct or LATS) that plans, searches, analyzes graph relationships, and synthesizes an answer. Not a search result—an *answer*.

### 3. Tier-Aware Intelligence

Here's something clever: CodeGraph automatically adjusts its behavior based on your LLM's context window.

Running a small local model? Get focused, efficient queries.
Using GPT-5 or Claude with 200K context? Get comprehensive, exploratory analysis.

| Your Model | CodeGraph's Behavior |
|------------|---------------------|
| < 50K tokens | Terse prompts, 5 reasoning steps |
| 50K-150K | Balanced analysis, 10 steps |
| 150K-500K | Detailed exploration, 15 steps |
| > 500K (Grok, etc.) | Full monty, 20 steps |

**Same tool, automatically optimized for your setup.**

### 4. Hybrid Search That Actually Works

We don't pick sides in the "embeddings vs keywords" debate. CodeGraph combines:

- **70% vector similarity** (semantic understanding)
- **30% lexical search** (exact matches matter)
- **Graph traversal** (relationships and context)
- **Optional reranking** (cross-encoder precision)

The result? You find `handleUserAuth` when you search for "login logic"—but also when you search for "handleUserAuth".

---

## Why This Matters for AI Coding

When you connect CodeGraph to Claude Code, Cursor, or any MCP-compatible agent:

**Before:** Your AI reads files one by one, grepping around, burning tokens on context-gathering.

**After:** Your AI calls `agentic_dependency_analysis("UserService")` and instantly knows what breaks if you refactor it.

This isn't incremental improvement. It's the difference between an AI that *searches* your code and one that *understands* it.

---

## Quick Start

### 1. Install

```bash
# Clone and build with all features
git clone https://github.com/yourorg/codegraph-rust
cd codegraph-rust
./install-codegraph-full-features.sh
```

### 2. Start SurrealDB

```bash
# Local persistent storage
surreal start --bind 0.0.0.0:3004 --user root --pass root file://$HOME/.codegraph/surreal.db
```

### 3. Apply Schema

```bash
cd schema && ./apply-schema.sh
```

### 4. Index Your Code

```bash
codegraph index /path/to/project -r -l rust,typescript,python
```

### 5. Connect to Claude Code

Add to your MCP config:
```json
{
  "mcpServers": {
    "codegraph": {
      "command": "/full/path/to/codegraph",
      "args": ["start", "stdio", "--watch"]
    }
  }
}
```

**That's it.** Your AI now understands your codebase.

---

## The Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                         Claude Code / MCP Client                │
└─────────────────────────────────┬───────────────────────────────┘
                                  │ MCP Protocol
                                  ▼
┌─────────────────────────────────────────────────────────────────┐
│                        CodeGraph MCP Server                     │
│  ┌───────────────────────────────────────────────────────────┐  │
│  │                    Agentic Tools Layer                    │  │
│  │  ┌─────────┐ ┌─────────┐ ┌─────────┐ ┌─────────────────┐  │  │
│  │  │ ReAct   │ │  LATS   │ │  Tier   │ │ Tool Execution  │  │  │
│  │  │ Agent   │ │  Agent  │ │ Selector│ │    Pipeline     │  │  │
│  │  └────┬────┘ └────┬────┘ └────┬────┘ └────────┬────────┘  │  │
│  └───────┼───────────┼───────────┼───────────────┼───────────┘  │
│          └───────────┴───────────┴───────────────┘              │
│                              │                                  │
│  ┌───────────────────────────┼───────────────────────────────┐  │
│  │                  Inner Graph Tools                        │  │
│  │  ┌──────────────┐ ┌──────────────┐ ┌──────────────────┐   │  │
│  │  │ Transitive   │ │    Call      │ │     Coupling     │   │  │
│  │  │ Dependencies │ │   Chains     │ │     Metrics      │   │  │
│  │  └──────────────┘ └──────────────┘ └──────────────────┘   │  │
│  │  ┌──────────────┐ ┌──────────────┐ ┌──────────────────┐   │  │
│  │  │   Reverse    │ │    Cycle     │ │       Hub        │   │  │
│  │  │    Deps      │ │  Detection   │ │      Nodes       │   │  │
│  │  └──────────────┘ └──────────────┘ └──────────────────┘   │  │
│  └───────────────────────────┬───────────────────────────────┘  │
└──────────────────────────────┼──────────────────────────────────┘
                               │
┌──────────────────────────────┼──────────────────────────────────┐
│                         SurrealDB                               │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────────┐  │
│  │   Nodes     │  │    Edges    │  │   Chunks + Embeddings   │  │
│  │  (AST +     │  │  (calls,    │  │   (HNSW vector index)   │  │
│  │   FastML)   │  │   imports)  │  │                         │  │
│  └─────────────┘  └─────────────┘  └─────────────────────────┘  │
│                                                                 │
│  ┌────────────────────────────────────────────────────────────┐ │
│  │              SurrealQL Graph Functions                     │ │
│  │   fn::semantic_search_chunks_with_context                  │ │
│  │   fn::get_transitive_dependencies                          │ │
│  │   fn::trace_call_chain                                     │ │
│  │   fn::calculate_coupling_metrics                           │ │
│  └────────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────────┘
```

**Key insight:** The agentic tools don't just call one function. They *reason* about which graph operations to perform, chain them together, and synthesize results. A single `agentic_dependency_analysis` call might:

1. Search for the target component semantically
2. Get its direct dependencies
3. Trace transitive dependencies
4. Check for circular dependencies
5. Calculate coupling metrics
6. Identify hub nodes that might be affected
7. Synthesize all findings into an actionable answer

---

## Supported Languages

CodeGraph uses tree-sitter for parsing and supports:

Rust • Python • TypeScript • JavaScript • Go • Java • C++ • C • Swift • Kotlin • C# • Ruby • PHP • Dart

---

## Provider Flexibility

### Embeddings
Use any model with dimensions 384-4096:
- **Local:** Ollama, LM Studio, ONNX Runtime
- **Cloud:** OpenAI, Jina AI

### LLM (for agentic reasoning)
- **Local:** Ollama, LM Studio
- **Cloud:** Anthropic Claude, OpenAI, xAI Grok

### Database
- **SurrealDB** with HNSW vector index (2-5ms queries)
- Free cloud tier available at [surrealdb.com/cloud](https://surrealdb.com/cloud)

---

## Configuration

Global config in `~/.codegraph/config.toml`:

```toml
[embedding]
provider = "ollama"
model = "qwen3-embedding:0.6b"
dimension = 1024

[llm]
provider = "anthropic"
model = "claude-sonnet-4"

[database.surrealdb]
connection = "ws://localhost:3004"
namespace = "ouroboros"
database = "codegraph"
```

See [INSTALLATION_GUIDE.md](docs/INSTALLATION_GUIDE.md) for complete configuration options.

---

## Daemon Mode

Keep your index fresh automatically:

```bash
# With MCP server (recommended)
codegraph start stdio --watch

# Standalone daemon
codegraph daemon start /path/to/project --languages rust,typescript
```

Changes are detected, debounced, and re-indexed in the background.

---

## What's Next

- [ ] More language support
- [ ] Cross-repository analysis
- [ ] Custom graph schemas
- [ ] Plugin system for custom analyzers

---

## Philosophy

CodeGraph exists because we believe AI coding assistants should be *augmented*, not replaced. The best AI-human collaboration happens when the AI has deep context about what you're working with.

We're not trying to replace your IDE, your type checker, or your tests. We're giving your AI the context it needs to actually help.

**Your codebase is a graph. Let your AI see it that way.**

---

## License

MIT

---

## Links

- [Installation Guide](docs/INSTALLATION_GUIDE.md)
- [SurrealDB Cloud](https://surrealdb.com/cloud) (free tier)
- [Jina AI](https://jina.ai) (free API tokens)
- [Ollama](https://ollama.com) (local models)
