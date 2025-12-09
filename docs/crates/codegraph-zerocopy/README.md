# CodeGraph Zerocopy (`codegraph-zerocopy`)

## Overview
Focuses on serialization performance using zero-copy techniques.

## Features
- **Rkyv Integration**: Wrappers and traits to facilitate the usage of `rkyv` for storing and retrieving complex graph structures without parsing overhead.
- **Performance**: Critical for large graphs where deserialization time can become a bottleneck.
