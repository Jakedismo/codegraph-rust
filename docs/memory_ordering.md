## Memory Ordering Validation

### SPSC Wait-Free Queue

- Producer (enqueue):
  - Read `head` with `Acquire` to check full condition
  - Write element into slot
  - Publish `tail` with `Release`

- Consumer (dequeue):
  - Read `tail` with `Acquire` to check empty condition
  - Read element from slot
  - Publish `head` with `Release`

This ensures the consumer observes slot writes after seeing the advanced `tail`, and the producer observes `head` updates when checking for space.

Loom tests (feature `loom`) explore interleavings for enqueue/dequeue to catch reordering issues.

### Lock-free Adjacency Graph

- Readers load adjacency pointers through `ArcSwap::load()` (Acquire)
- Writers update adjacency via `rcu` (copy-on-write) which uses CAS under the hood to safely publish a new `Arc<Vec<NodeId>>` with Release semantics

This provides lock-free reads and atomic pointer publication, preventing torn reads or missed edges under concurrency.

