# SurrealDB Migration Scripts

Versioned migration tools for upgrading codegraph's SurrealDB storage.

## Latest: v1.0.0

Migrates codegraph data from **SurrealDB v2.3.7** to **SurrealDB v3.0.5**.

```
cd v1.0.0/
cp migrate.py /where/you/run/it/
$EDITOR migrate.py   # set SRC_URL, TGT_URL, passwords
python3 migrate.py
```

See [v1.0.0/README.md](v1.0.0/README.md) for full documentation.

## Why

SurrealDB v3.0.5 cannot open RocksDB data created by v2.3.7. The storage
format version is checked on boot and `surreal fix` is not implemented in v3.
This tool migrates data live via RPC with no filesystem export.

## Versions

| Version | Source DB | Target DB | Status |
|---------|-----------|-----------|--------|
| [v1.0.0](v1.0.0/) | v2.3.7 | v3.0.5 | ✅ Tested |
