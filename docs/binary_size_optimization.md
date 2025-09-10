## Binary Size Optimization

Goal: Deliver a single `codegraph-api` binary under 40MB.

### Build Profiles

- `release`: performance-oriented (unchanged)
- `release-size`: size-optimized profile with:
  - `opt-level = "z"`, `lto = "fat"`, `codegen-units = 1`
  - `panic = "abort"`, `strip = true`, no debug info

### Feature & Dependency Changes

- `reqwest` minimized: `default-features = false`, `features = ["json", "stream", "native-tls"]`
- `codegraph-vector` default features now exclude OpenAI client; `faiss` is enabled by default to satisfy API usage without pulling heavy TLS dependencies.
- `rocksdb` features trimmed in `codegraph-graph` to `features = ["zstd"]` (default compression set reduced: snappy/lz4/zlib/bzip2 disabled).

### Stripping & Compression

- Added `scripts/strip_compress.sh` for post-build binary stripping and optional UPX compression.
- Docker build stage installs `upx` and compresses the binary.

### Commands

- Local size build:
  - `make build-api-size`
  - `make size`

- Docker image with size-optimized binary:
  - `docker build -t codegraph-api:small .`

### Notes

- Using `native-tls` leverages platform TLS (macOS Security / OpenSSL on Linux) to avoid bundling TLS stacks.
- Dead code is aggressively removed by LTO and `opt-level=z`; optional features are minimized to reduce reachable code.
- Further reductions are possible by feature-gating FAISS usage when not required at runtime.

