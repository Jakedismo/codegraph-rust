# CodeGraph

CodeGraph turns your local codebase into a searchable knowledge graph so any Model Context Protocol (MCP) client can answer questions with project-aware context. The CLI indexes source code, pushes semantic embeddings into FAISS, and exposes a fast MCP server that tools such as LM Studio or Claude Desktop can call.

This README focuses on the streamlined developer workflow we ship today. For a deeper dive into the agent architecture or legacy experiments, see the material moved under `docs/legacy/`.

---

## Quick Start

1. **Install prerequisites**
   ```bash
   # macOS / Linux prerequisites
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   brew install faiss            # or your distro's libfaiss-dev package
   ```

2. **Install CodeGraph**
   ```bash
   cd codegraph-rust
   cargo install --path crates/codegraph-mcp --features "embeddings,faiss"
   ```

3. **Prepare LM Studio** (recommended runtime)
   - Download LM Studio from [lmstudio.ai](https://lmstudio.ai/)
   - Download the models:
     - Embeddings: `jinaai/jina-code-embeddings-1.5b`
     - LLM (optional): `lmstudio-community/DeepSeek-Coder-V2-Lite-Instruct-Q4_K_M`
   - Start the LM Studio local server on `http://localhost:1234`

4. **Configure CodeGraph**
   Create `.env` in your repository (or export the variables):
   ```bash
   CODEGRAPH_EMBEDDING_PROVIDER=lmstudio
   CODEGRAPH_EMBEDDING_MODEL=jinaai/jina-code-embeddings-1.5b
   CODEGRAPH_LMSTUDIO_URL=http://localhost:1234
   CODEGRAPH_EMBEDDING_DIMENSION=1536
   RUST_LOG=warn
   ```

5. **Index and serve**
   ```bash
   codegraph index .          # builds .codegraph/ with embeddings + graph
   codegraph start stdio      # exposes MCP interface for LM Studio / Claude
   ```

That is the entire end-to-end workflow. LM Studio now has a project-specific vector store it can query through MCP.

---

## Cloud Provider Support ☁️

CodeGraph now supports both **local** and **cloud-based** LLM and embedding providers, giving you flexibility in deployment:

### Supported Cloud Providers

- **Anthropic Claude** - State-of-the-art code understanding with 200K context
- **OpenAI GPT** - GPT-4o and other OpenAI models
- **OpenAI-Compatible** - Any custom OpenAI-compatible endpoint

### Quick Setup with Wizard

The easiest way to configure cloud providers:

```bash
# Build and run the setup wizard
cargo build --release --bin codegraph-setup --features all-cloud-providers
./target/release/codegraph-setup
```

The interactive wizard will guide you through:
1. Choosing your embedding provider (ONNX, Ollama, LM Studio, or OpenAI)
2. Selecting your LLM provider (Ollama, LM Studio, Anthropic, OpenAI, or custom)
3. Configuring API keys and models
4. Setting advanced options

### Manual Configuration

Or configure manually in `.codegraph.toml`:

```toml
[llm]
enabled = true
provider = "anthropic"  # or "openai", "ollama", "lmstudio", "openai-compatible"
model = "claude-3-5-sonnet-20241022"
anthropic_api_key = "sk-ant-..."  # Or set ANTHROPIC_API_KEY env var
context_window = 200000
temperature = 0.1
max_tokens = 4096
```

**For detailed provider setup, pricing, and comparisons, see [docs/CLOUD_PROVIDERS.md](docs/CLOUD_PROVIDERS.md).**

---

## Everyday CLI

| Command | Description |
|---------|-------------|
| `codegraph index <path>` | Incrementally index or reindex a project. |
| `codegraph start stdio`  | Launch the MCP server on STDIO (works with LM Studio, Claude Desktop, etc.). |
| `codegraph start http`   | Optional HTTP transport for custom tooling. |
| `codegraph tools list`   | Inspect available MCP tools exposed by the server. |

Tips:
- Run `codegraph index` from each repository root you want to serve.
- Keep `RUST_LOG=warn` in your environment for a clean TUI while indexing.
- To script updates, rerun `codegraph index` after significant code changes; the cache handles incremental updates.

---

## Speed Metrics (Apple Silicon, LM Studio Backend)

| Scenario | Embedding Throughput | Notes |
|----------|----------------------|-------|
| LM Studio + Jina 1.5B embeddings | **120 embeddings/sec** | Uses MLX + Flash Attention 2. |
| Ollama + nomic-embed-code        | ~60 embeddings/sec     | Roughly half the speed of LM Studio. |
| FAISS vector search              | 2–5 ms latency         | With index caching enabled. |

### Optimization Cheatsheet

| Optimization | Typical Speedup | Extra Memory | Enabled by Default |
|--------------|-----------------|--------------|--------------------|
| FAISS index cache | 10–50× faster warm searches | 300–600 MB | ✅ |
| Embedding generator cache | 10–100× faster first query | ~90 MB | ✅ |
| Query result cache | 100× on repeated prompts | ~10 MB | ✅ |
| Parallel shard search | 2–3× | negligible | ✅ |

These numbers come from the same benchmarking harness described in `docs/performance/`. They are preserved here so you can quickly sanity-check your own runs.

---

## LM Studio Reference

The full LM Studio checklist (model recommendations, port configuration, batch-size tuning, and troubleshooting) lives in [LMSTUDIO_SETUP.md](LMSTUDIO_SETUP.md). It expands on:
- Switching between embedding and completion models within LM Studio
- Running multiple projects side by side
- Recommended `.env` / `.codegraph.toml` settings

For Ollama or ONNX-only setups, see `docs/INSTALLATION.md` and the legacy guides under `docs/legacy/`.

---

## Advanced Features

- **Graph analytics**: The indexer produces ownership graphs, dependency edges, and metadata that RAG tools consume automatically.
- **MCP tools**: `vector.search`, `graph.traverse`, `code.read`, and more are exposed through the MCP server.
- **FAISS tuning**: Set `CODEGRAPH_FAISS_TRAINING_THRESHOLD` or provide a pre-built index in `.codegraph/faiss.index` for massive monorepos.

Explore the `docs/` tree for deeper architecture notes, RAG pipeline details, deployment manifests, and operator playbooks.

---

## Contributing

We welcome pull requests! Running the full test suite:
```bash
cargo fmt --all
cargo clippy --workspace --all-targets
cargo test --workspace
```
Please open an issue if you want to discuss large changes or integrations.

---

## License

CodeGraph is dual-licensed under MIT and Apache 2.0. See `LICENSE-MIT` and `LICENSE-APACHE` for details.

