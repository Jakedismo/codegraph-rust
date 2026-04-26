# SurrealDB Migration Scripts — v1.0.0

Migrates codegraph data from **SurrealDB v2.3.7** to **SurrealDB v3.0.5**.

## Problem

SurrealDB v3.0.5 cannot open RocksDB data files created by v2.3.7.
The storage format version is stored internally and checked on boot — `surreal fix`
is not implemented in v3. v2.3.7's `EXPORT` produces SurrealQL with v2-specific
syntax that v3.0.5's parser rejects.

## Solution

Dual-server live migration via Python `surrealdb` client (v1.0.8):

```
v2.3.7 (ws://host:8765/rpc)  →  Python reads all rows  →  v3.0.5 (ws://host:8771/rpc)
     [live RocksDB]                via WebSocket             [fresh RocksDB]
```

- `INSERT IGNORE` makes it idempotent — safe to resume if interrupted
- HNSW indexes are created AFTER bulk load (faster inserts)
- No filesystem export needed

## Usage

```bash
# 1. Start fresh v3.0.5 on a new directory
/tmp/surreal start \
  --user root --pass YOUR_PASSWORD \
  --bind 0.0.0.0:8771 \
  --log error \
  rocksdb:/tmp/surrealdb-v305-target

# 2. Run migration (use nohup for long runs)
/bin/bash -c 'nohup python3 migrate.py >> migrate-progress.log 2>&1 &'
tail -f migrate-progress.log

# 3. After migration: verify
python3 -c "
from surrealdb import Surreal
tgt = Surreal('ws://127.0.0.1:8771/rpc')
tgt.signin({'user': 'root', 'pass': 'YOUR_PASS'})
tgt.use('ouroboros', 'codegraph')
for t in ['nodes','edges','symbol_embeddings']:
    r = tgt.query(f'SELECT count() as c FROM {t} GROUP ALL')
    print(f'{t}: {r[0][\"c\"]}')
"
```

## What gets migrated

| Table | Rows | Embedding dim |
|-------|------|---------------|
| `nodes` | 14,178 | 512D (nomic-embed-text) |
| `edges` | 165,200 | — |
| `symbol_embeddings` | 45,217 | 768D (nomic-embed-text) |

HNSW indexes recreated post-load:
```sql
DEFINE INDEX idx_nodes_emb ON nodes FIELDS embedding HNSW DIMENSION 512 DIST COSINE M 16 EFC 150;
DEFINE INDEX idx_sym_emb ON symbol_embeddings FIELDS embedding_768 HNSW DIMENSION 768 DIST COSINE M 16 EFC 150;
```

## Version history

- **v1.0.0** — Initial version. Migrates nodes, edges, symbol_embeddings.
  Tested against SurrealDB v2.3.7 → v3.0.5.
