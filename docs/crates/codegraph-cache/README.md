# CodeGraph Cache (`codegraph-cache`)

## Overview
Implements caching strategies to speed up incremental indexing and queries.

## Features
- **Query Cache**: Caches results of expensive graph traversals or vector searches.
- **Invalidation**: Logic to invalidate cache entries when files change (hooked into file watchers in the daemon).
- **Read-ahead**: Experimental features for pre-fetching related graph nodes.
