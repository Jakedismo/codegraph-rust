.PHONY: build test lint fmt check bench clean install-deps doc

# Default target
all: check test

# Build all crates
build:
	cargo build --workspace

# Build in release mode
build-release:
	cargo build --workspace --release

# Build MCP server with AutoAgents experimental feature
build-mcp-autoagents:
	cargo build --release -p codegraph-mcp --bin codegraph --features "ai-enhanced,autoagents-experimental,faiss,ollama"

# Build MCP HTTP server with experimental HTTP transport
.PHONY: build-mcp-http
build-mcp-http:
	cargo build --release -p codegraph-mcp --bin codegraph --features "ai-enhanced,autoagents-experimental,faiss,embeddings-ollama,server-http"

# Run HTTP server (depends on build)
.PHONY: run-http-server
run-http-server: build-mcp-http
	./target/release/codegraph start http --port 3000

# Build API with size-optimized profile and strip/compress
build-api-size:
	cargo build --profile release-size --bin codegraph-api
	./scripts/strip_compress.sh target/release-size/codegraph-api

size:
	@echo "Sizes for codegraph-api binaries (if present):"
	@for p in target/release/codegraph-api target/release-size/codegraph-api; do \
	  if [ -f $$p ]; then echo "$$p: $$(du -h $$p | cut -f1)"; fi; \
	done

# Run all tests
test:
	cargo test --workspace

# Run tests with coverage (requires cargo-tarpaulin)
test-coverage:
	cargo tarpaulin --workspace --out Html

# Lint with clippy
lint:
	# Limit clippy to default member(s) to avoid compiling experimental crates
	cargo clippy -p codegraph-core -- -A clippy::all -A warnings

# Format code
fmt:
	cargo fmt --all

# Check formatting
fmt-check:
	cargo fmt --all -- --check

# Full check (format, lint, test)
check: fmt lint test

# Run benchmarks
bench:
	@echo "Running benchmarks and saving baseline: $${BASELINE_NAME:-baseline} (ENABLE_FAISS_BENCH=$${ENABLE_FAISS_BENCH:-0})"
	BASELINE_NAME=$${BASELINE_NAME:-baseline} ENABLE_FAISS_BENCH=$${ENABLE_FAISS_BENCH:-0} scripts/run_benchmarks.sh

bench-report:
	@echo "Generating benchmark report from latest results"
	python3 scripts/generate_bench_report.py --output benchmarks/reports/benchmark_report_latest.md

bench-compare:
	@echo "Comparing current benchmark results to baseline: $${BASELINE_NAME:-baseline}"
	python3 scripts/compare_bench.py --baseline $${BASELINE_NAME:-baseline} --threshold $${THRESHOLD:-0.10}

# Clean build artifacts
clean:
	cargo clean

# Install development dependencies
install-deps:
	cargo install cargo-tarpaulin
	cargo install cargo-audit
	cargo install cargo-outdated

# Generate documentation
doc:
	cargo doc --workspace --no-deps --open

# Security audit
audit:
	cargo audit

# Check for outdated dependencies
outdated:
	cargo outdated --workspace

# Run the API server
run-api:
	RUST_LOG=debug cargo run --bin codegraph-api

# Sync validation stress tests (standalone harness)
sync-validate:
	cargo run --manifest-path high_perf_test/Cargo.toml

# Development workflow
dev: fmt lint test

# CI workflow
ci: fmt-check lint test

# Quick check without tests
quick: fmt lint

# Run API E2E/integration tests (API crate)
e2e:
	cargo test -p codegraph-api -- --nocapture

# Load testing using k6 (requires k6 installed)
load-test:
	BASE_URL=$${BASE_URL:-http://localhost:3000} scripts/load/run.sh

# Deployment validation against a running endpoint
deploy-validate:
	BASE_URL=$${BASE_URL:-http://localhost:3000} scripts/deploy/validate.sh

# Performance regression check (Criterion benches vs baseline)
perf-regression:
	BASELINE_NAME=$${BASELINE_NAME:-baseline} THRESHOLD=$${THRESHOLD:-0.10} scripts/perf/regression.sh

# Cross-platform build (local)
build-cross:
	@echo "Building release binaries for Linux, macOS, Windows targets"
	bash scripts/build_cross.sh

# Build and run specific examples
example-parse:
	cargo run --bin codegraph-api &
	sleep 2
	curl -X POST http://localhost:3000/parse \
		-H "Content-Type: application/json" \
		-d '{"file_path": "src/main.rs"}'
	pkill -f codegraph-api

# ===================== Memory Safety / Leak Detection =====================

# Run API with leak detection enabled (feature-gated)
run-api-leaks:
	RUST_LOG=debug cargo run --bin codegraph-api --features leak-detect

# Export on-demand leak report to target/memory_reports
leak-report:
	curl -s http://localhost:3000/memory/leaks | jq .

# View runtime memory stats (requires leak-detect)
leak-stats:
	curl -s http://localhost:3000/memory/stats | jq .

# Run tests under Miri (requires nightly toolchain + miri component)
miri:
	rustup toolchain install nightly --component miri || true
	cargo +nightly miri test --workspace

# Run tests with Address/Leak Sanitizer (requires nightly)
asan-test:
	rustup toolchain install nightly || true
	RUSTFLAGS="-Zsanitizer=address" \
	RUSTDOCFLAGS="-Zsanitizer=address" \
	cargo +nightly test --workspace -Zbuild-std --target x86_64-apple-darwin
