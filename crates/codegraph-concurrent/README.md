# codegraph-concurrent

Concurrent, lock-free and wait-free data structures for CodeGraph.

- WaitFreeSpscQueue: wait-free SPSC bounded ring buffer
- LockFreeMpmcQueue: lock-free bounded MPMC (wrapper over crossbeam ArrayQueue)
- LockFreeAdjacencyGraph: lock-free adjacency operations using ArcSwap + SkipMap

## Usage

```rust
use codegraph_concurrent::spsc::WaitFreeSpscQueue;

let (prod, cons) = WaitFreeSpscQueue::with_capacity(1024);
prod.try_push(123).unwrap();
assert_eq!(cons.try_pop().unwrap(), 123);
```

Enable loom tests:

```
cargo test -p codegraph-concurrent --features loom
```

See `docs/memory_ordering.md` for ordering rationale.
