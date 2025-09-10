.PHONY: build test lint fmt check bench clean install-deps doc

# Default target
all: check test

# Build all crates
build:
	cargo build --workspace

# Build in release mode
build-release:
	cargo build --workspace --release

# Run all tests
test:
	cargo test --workspace

# Run tests with coverage (requires cargo-tarpaulin)
test-coverage:
	cargo tarpaulin --workspace --out Html

# Lint with clippy
lint:
	cargo clippy --workspace --all-targets --all-features -- -D warnings

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

# Build and run specific examples
example-parse:
	cargo run --bin codegraph-api &
	sleep 2
	curl -X POST http://localhost:3000/parse \
		-H "Content-Type: application/json" \
		-d '{"file_path": "src/main.rs"}'
	pkill -f codegraph-api
