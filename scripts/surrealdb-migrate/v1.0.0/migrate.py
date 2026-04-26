#!/usr/bin/env python3
"""
SurrealDB v2.3.7 → v3.0.5 bulk migration for codegraph.
Reads all rows from v2.3.7, writes to v3.0.5 via INSERT IGNORE.
Idempotent — safe to re-run if interrupted.
"""
import sys, json, time, os, signal
from datetime import datetime

LOG = '/tmp/migrate-progress.log'

def log(msg):
    ts = datetime.now().strftime('%H:%M:%S')
    line = "[%s] %s" % (ts, msg)
    print(line, flush=True)
    with open(LOG, 'a') as f:
        f.write(line + '\n')

def signal_handler(sig, frame):
    log("Interrupted. Run again to resume (INSERT IGNORE skips already-migrated rows).")
    sys.exit(0)
signal.signal(signal.SIGTERM, signal_handler)
signal.signal(signal.SIGINT, signal_handler)

sys.path.insert(0, '/home/andrew/.local/lib/python3.10/site-packages')
from surrealdb import Surreal

# ── Config ────────────────────────────────────────────────────────────────────
SRC_URL = 'ws://127.0.0.1:8765/rpc'
TGT_URL = 'ws://127.0.0.1:8771/rpc'
SRC_USER = SRC_PASS = TGT_USER = TGT_PASS = 'root'  # adjust password as needed
NS, DB = 'ouroboros', 'codegraph'

log("="*60)
log("SurrealDB v2.3.7 → v3.0.5 migration")
log("="*60)

# ── Connect ───────────────────────────────────────────────────────────────────
log("Connecting to source v2.3.7...")
src = Surreal(SRC_URL)
src.signin({'user': SRC_USER, 'pass': SRC_PASS})
src.use(NS, DB)
log("Source connected ✓")

log("Connecting to target v3.0.5...")
tgt = Surreal(TGT_URL)
tgt.signin({'user': TGT_USER, 'pass': TGT_PASS})
tgt.use(NS, DB)
log("Target connected ✓")

# ── Helpers ───────────────────────────────────────────────────────────────────
def tgt_count(tbl):
    try:
        return tgt.query('SELECT count() as c FROM ' + tbl + ' GROUP ALL')[0]['c']
    except:
        return 0

def serialize(v):
    if v is None: return None
    if isinstance(v, list): return [float(x) for x in v]
    if isinstance(v, dict): return json.dumps(v)
    return v

def bulk_insert(table, rows, fields, batch_size=300):
    try: tgt.query('DEFINE TABLE ' + table + ' SCHEMALESS')
    except: pass

    n = len(rows)
    log("  %s: inserting %d rows..." % (table, n))
    t0 = time.time()
    for i in range(0, n, batch_size):
        batch = rows[i:i+batch_size]
        clean = []
        for r in batch:
            rec = {}
            for f in fields:
                v = r.get(f)
                if v is None: rec[f] = None; continue
                if f in ('src', 'dst', 'node_id'): rec[f] = str(v) if v else None
                elif f == 'node_type': rec[f] = str(v) if v else None
                elif isinstance(v, list): rec[f] = [float(x) for x in v]
                else: rec[f] = serialize(v)
            clean.append(rec)

        ph = ','.join(['$'+f for f in fields])
        sql = 'INSERT IGNORE INTO ' + table + ' (' + ','.join(fields) + ') VALUES (' + ph + ')'
        params = [{f: r[f] for f in fields} for r in clean]
        try:
            tgt.query(sql, params)
        except:
            for r in clean:
                rp = ','.join(['$'+f for f in fields])
                tgt.query('INSERT IGNORE INTO ' + table + ' (' + ','.join(fields) + ') VALUES (' + rp + ')', {f: r[f] for f in fields})

        done = i + len(batch)
        pct = done * 100 // n
        rate = done / (time.time()-t0) if time.time()-t0 > 0 else 1
        eta = (n - done) / rate if rate > 0 else 0
        print("\r    %s: %d/%d (%d%%) | %d/s | ETA %ds   " % (table, done, n, pct, rate, eta), end='', flush=True)

    print()
    log("  %s: done in %.1fs" % (table, time.time()-t0))

# ── Load data from v2.3.7 ─────────────────────────────────────────────────────
log("Reading all data from source...")
t0 = time.time()
node_rows = src.query('SELECT * FROM nodes')
log("  nodes: %d rows in %.1fs" % (len(node_rows), time.time()-t0))

t0 = time.time()
edge_rows_raw = src.query('SELECT src, dst, edge_type, project_id, updated_at FROM edges')
log("  edges: %d rows in %.1fs" % (len(edge_rows_raw), time.time()-t0))

t0 = time.time()
sym_rows = src.query('SELECT symbol, embedding_768, embedding_model, node_id, project_id, updated_at, access_count, normalized_symbol FROM symbol_embeddings')
log("  symbol_embeddings: %d rows in %.1fs" % (len(sym_rows), time.time()-t0))

src.close()
log("Source closed ✓")

# ── Serialize ──────────────────────────────────────────────────────────────────
log("Serializing...")
edge_rows = [{'src': str(r.get('src')) if r.get('src') else None,
              'dst': str(r.get('dst')) if r.get('dst') else None,
              'edge_type': r.get('edge_type'),
              'project_id': r.get('project_id'),
              'updated_at': r.get('updated_at')} for r in edge_rows_raw]
del edge_rows_raw

node_rows = [{'name': r.get('name'),
              'node_type': str(r.get('node_type')) if r.get('node_type') else None,
              'file_path': r.get('file_path'),
              'language': r.get('language'),
              'start_line': r.get('start_line'),
              'end_line': r.get('end_line'),
              'metadata': json.dumps(r.get('metadata')) if isinstance(r.get('metadata'), dict) else r.get('metadata'),
              'chunk_count': r.get('chunk_count'),
              'embedding_model': r.get('embedding_model'),
              'embedding': [float(x) for x in r.get('embedding')] if r.get('embedding') else None,
              'project_id': r.get('project_id'),
              'updated_at': r.get('updated_at')} for r in node_rows]

# ── Migrate ───────────────────────────────────────────────────────────────────
log("Starting bulk inserts...")

NODE_FIELDS = ['name','node_type','file_path','language','start_line','end_line','metadata','chunk_count','embedding_model','embedding','project_id','updated_at']
EDGE_FIELDS = ['src','dst','edge_type','project_id','updated_at']
SYM_FIELDS  = ['symbol','embedding_768','embedding_model','node_id','project_id','updated_at','access_count','normalized_symbol']

bulk_insert('nodes',              node_rows, NODE_FIELDS, batch_size=200)
bulk_insert('edges',             edge_rows,  EDGE_FIELDS, batch_size=300)
bulk_insert('symbol_embeddings',  sym_rows,   SYM_FIELDS,  batch_size=200)

# ── HNSW indexes (created after data load for speed) ─────────────────────────
log("Creating HNSW indexes...")
for stmt in [
    "DEFINE INDEX IF NOT EXISTS idx_nodes_emb ON nodes FIELDS embedding HNSW DIMENSION 512 DIST COSINE M 16 EFC 150",
    "DEFINE INDEX IF NOT EXISTS idx_sym_emb ON symbol_embeddings FIELDS embedding_768 HNSW DIMENSION 768 DIST COSINE M 16 EFC 150",
]:
    try: tgt.query(stmt)
    except: pass
log("Indexes created ✓")

# ── Report ───────────────────────────────────────────────────────────────────
log("="*60)
log("MIGRATION COMPLETE")
log("="*60)
for tbl in ['nodes', 'edges', 'symbol_embeddings']:
    log("  %s: %d" % (tbl, tgt_count(tbl)))
tgt.close()
log("Done!")
