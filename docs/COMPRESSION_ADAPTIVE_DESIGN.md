## Development Context

- Feature: Adaptive compression optimization (zstd, lz4), real-time response compression, and dictionary-based compression for repeated data
- Technical Stack: Rust (workspace crates: axum, tower-http, sysinfo, zstd, lz4_flex), async (tokio), tracing
- Constraints:
  - Keep interfaces stable in core crates
  - Avoid blocking in async paths
  - HTTP negotiation must respect `Accept-Encoding`
  - Balance compression ratio and CPU usage under load
- Success Criteria:
  - LZ4 and Zstd supported end-to-end for vector/cache compression
  - Adaptive selection favors CPU efficiency vs. ratio based on load and payload size
  - API responses compressed (gzip/br/zstd) when clients advertise support
  - Optional Zstd dictionary can be trained and used for repeated data patterns

## Design Overview

- In-memory/vector compression:
  - Extend `codegraph-cache` MemoryManager with lossless LZ4 and Zstd compression for `Vec<f32>` vectors
  - Add adaptive selection heuristic using `sysinfo` CPU load and payload size
  - Provide optional Zstd dictionary training from samples

- HTTP response compression:
  - Add `tower_http::compression::CompressionLayer` to API router to negotiate gzip/deflate/brotli/zstd
  - Keep middleware order: CORS → Compression → headers → backpressure → tracing

## Trade-off Strategy

- Small payloads (<4 KiB): use LZ4 to minimize CPU overhead
- High CPU (>75%): prefer LZ4 (faster, lower ratio)
- Large payloads (>64 KiB) + moderate CPU: use Zstd level 6 for better ratios
- Default moderate payloads: use Zstd level 3

## Acceptance Tests (conceptual)

- Compression API: round-trip (compress → decompress) equality for LZ4/Zstd
- Adaptive path: selection changes with synthetic CPU load and payload size thresholds
- HTTP responses: Content-Encoding negotiated (gzip/br/zstd) and payload size reduction observed
- Zstd dictionary: training succeeds and improves compression ratio on repeated samples

## Notes

- LZ4 dictionaries are not wired; Zstd dictionary support added where it matters most
- HTTP dictionary-based compression is not standardized; dictionaries are applied for internal data paths
