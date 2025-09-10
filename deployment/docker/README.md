# CodeGraph Production Deployment (Docker)

This directory contains a production-grade containerization setup for CodeGraph with:

- Multi-stage Dockerfile for minimal runtime images
- Docker Compose stack: api, vector-maintainer, graph-backup, Prometheus, Grafana
- Security hardening, health checks, and resource limits
- Persistent volumes and automated backup strategy
- Orchestration scripts and a deployment test harness

## Quick Start

- Build and start all services:
  - `deployment/docker/scripts/start.sh`
- Stop services:
  - `deployment/docker/scripts/stop.sh`
- Scale API replicas (Compose):
  - `deployment/docker/scripts/scale.sh 3`

API runs on `http://localhost:8080`.

## Volumes

- `graph-data`: persists RocksDB at `/var/lib/codegraph/graph.db`
- `backups`: periodic compressed backups (`.tar.zst`) created by `graph-backup`

Backups are created every 4 hours by default and the latest 10 are retained. Adjust via env in `docker-compose.yml`.

## Health & Monitoring

- API health: `/health`, `/health/ready`, `/health/live`
- Prometheus (9090) scrapes API metrics at `/metrics`
- Grafana (3000) provides dashboards (anonymous access enabled by default)

## Security Hardening

- Containers drop all Linux capabilities and enforce `no-new-privileges`
- Non-root user (UID 10001) for runtime
- Read-only root FS with explicit writable volumes
- Minimal runtime image with only required shared libraries

## Configuration

- Defaults from `config/production.toml` are copied into the image
- Override via env (e.g., `CODEGRAPH__SERVER__PORT`) or by bind-mounting `config/`

## Tests

A deployment test harness is included:

- `deployment/docker/tests/run_tests.sh`
  - Static checks over Compose/Dockerfile
  - Optional image size test: `RUN_BUILD_TEST=1`
  - Optional integration health check: `RUN_INTEGRATION=1`

Example:

```
RUN_BUILD_TEST=1 RUN_INTEGRATION=1 bash deployment/docker/tests/run_tests.sh
```

## Backups

- Sidecar service `graph-backup` tars `/var/lib/codegraph` to `backups` volume
- `deployment/docker/scripts/restore.sh` restores a selected archive

## Notes

- Vector operations are in-process within the API. `vector-maintainer` sidecar periodically triggers `/vector/index/rebuild` to keep indexes fresh.
- Ensure Docker Desktop or engine has sufficient resources (CPU/RAM) for the stack.

