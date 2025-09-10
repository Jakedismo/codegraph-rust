# codegraph-parser

High-performance source parsing for CodeGraph using Tree-sitter.

## Performance I/O Enhancements

This crate includes optional, Linux-optimized async I/O paths and improved scheduling:

- io_uring (optional feature `io-uring`, Linux-only):
  - Uses `tokio-uring` for small file reads to reduce syscall overhead and improve batched throughput.
- Zero-copy mapping for large files:
  - Uses `memmap2` to map large files and convert to text, reducing explicit read syscalls.
- Async batching and scheduling:
  - Directory traversal via `ignore` crate (honors .gitignore) in a fast parallel walker.
  - Size-aware scheduling to parse larger files first and reduce tail latency.

### Enabling io_uring

```
cargo build -p codegraph-parser --features io-uring --target x86_64-unknown-linux-gnu
```

Note: io_uring requires a Linux kernel with io_uring support. When disabled or unavailable, the reader falls back to standard Tokio async file I/O.

### Notes

- The parser maintains compatibility with existing APIs and caches. No API surface changes are required to adopt the new I/O paths.
- The file read strategy is selected automatically based on platform and file size.

