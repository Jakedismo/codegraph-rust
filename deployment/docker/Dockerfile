# CodeGraph production container (multi-stage, minimal runtime)

# --- Builder stage -----------------------------------------------------------
FROM rust:1.75-slim AS builder

ARG DEBIAN_FRONTEND=noninteractive
RUN apt-get update && apt-get install -y --no-install-recommends \
    build-essential \
    cmake \
    pkg-config \
    clang \
    libssl-dev \
    libclang-dev \
    zlib1g-dev \
    libbz2-dev \
    liblz4-dev \
    libzstd-dev \
    libsnappy-dev \
    librocksdb-dev \
    libfaiss-dev \
    ca-certificates \
    curl \
  && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy workspace manifests first to leverage Docker layer caching
COPY Cargo.toml Cargo.lock ./
COPY crates/ ./crates/

# Build dependencies (cache-friendly)
RUN cargo fetch

# Build release binary for codegraph-api
ENV RUSTFLAGS="-C target-cpu=native -C debuginfo=0"
RUN cargo build --release -p codegraph-api && \
    strip target/release/codegraph-api || true

# Collect all dynamic library runtime deps for the binary
RUN set -euo pipefail; \
    mkdir -p /opt/libs; \
    ldd target/release/codegraph-api | awk '{print $3}' | \
      grep -E '^/' | sort -u | \
      xargs -I '{}' cp -v '{}' /opt/libs/ || true; \
    # include libgcc_s & libstdc++ explicitly when present
    for f in /usr/lib/x86_64-linux-gnu/libstdc++.so.* /lib/x86_64-linux-gnu/libgcc_s.so.*; do \
      [ -e "$f" ] && cp -v "$f" /opt/libs/ || true; \
    done

# Copy production config (can be overridden by bind mount)
COPY config/ /opt/config/


# --- Runtime stage -----------------------------------------------------------
FROM debian:bookworm-slim AS runtime

ARG DEBIAN_FRONTEND=noninteractive
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    tini \
    wget \
  && rm -rf /var/lib/apt/lists/*

# Non-root user
RUN useradd -r -u 10001 -s /usr/sbin/nologin codegraph

WORKDIR /app

# Minimal runtime deps copied from builder to keep image small
COPY --from=builder /opt/libs/ /usr/local/lib/
ENV LD_LIBRARY_PATH=/usr/local/lib

# Copy binary and default config
COPY --from=builder /app/target/release/codegraph-api /usr/local/bin/codegraph-api
COPY --from=builder /opt/config/ /app/config/

# Data dir for RocksDB
RUN mkdir -p /var/lib/codegraph && chown -R codegraph:codegraph /var/lib/codegraph

# Drop privileges and run as non-root
USER codegraph:codegraph

# Expose production port (matches config/production.toml)
EXPOSE 8080

# Security: read-only rootfs except for volumes
VOLUME ["/var/lib/codegraph", "/app/config"]

ENV APP_ENV=production \
    RUST_LOG=info \
    RUST_BACKTRACE=1

HEALTHCHECK --interval=10s --timeout=3s --start-period=10s --retries=3 \
  CMD wget -qO- http://127.0.0.1:8080/health/ready >/dev/null 2>&1 || exit 1

# Ensure tini is PID 1 for proper signal handling
ENTRYPOINT ["/usr/bin/tini", "--"]
CMD ["/usr/local/bin/codegraph-api"]
