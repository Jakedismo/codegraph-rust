# Multi-stage build for CodeGraph API
FROM rust:1.75-slim as builder

# Install system dependencies
RUN apt-get update && apt-get install -y \
    build-essential \
    cmake \
    pkg-config \
    libssl-dev \
    libclang-dev \
    && rm -rf /var/lib/apt/lists/*

# Set working directory
WORKDIR /app

# Copy manifests
COPY Cargo.toml Cargo.lock ./
COPY crates/ crates/

# Build dependencies first (for layer caching)
RUN cargo fetch

# Build the application
RUN cargo build --release --bin codegraph-api

# Runtime stage
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

# Create app user
RUN useradd -r -s /bin/false codegraph

# Copy binary from builder
COPY --from=builder /app/target/release/codegraph-api /usr/local/bin/codegraph-api

# Set permissions
RUN chown codegraph:codegraph /usr/local/bin/codegraph-api

# Create data directory
RUN mkdir -p /app/data && chown codegraph:codegraph /app/data

# Switch to non-root user
USER codegraph

# Set working directory
WORKDIR /app

# Expose port
EXPOSE 3000

# Health check
HEALTHCHECK --interval=30s --timeout=30s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:3000/health || exit 1

# Run the binary
CMD ["codegraph-api"]