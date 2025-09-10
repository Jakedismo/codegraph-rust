CI/CD Pipeline Overview

This repository includes fully automated CI/CD using GitHub Actions with multi-environment deployments (dev, staging, prod), quality gates, container image publishing, semantic releases, and automated rollbacks for Kubernetes.

Workflows

- `ci.yml`: Cross-platform CI (lint, clippy, tests, coverage, build). Already present.
- `deploy.yml`: Continuous Deployment for dev, staging, prod with quality gates, image build/push, Trivy scan, and Kubernetes rollout + rollback.
- `release-please.yml`: Semantic version automation via Release Please, generating release PRs and tags.
- `release.yml`: Build cross-platform binaries and Docker images when a tag `v*` is published.
- `semantic-pr.yml`: Enforces Conventional Commit semantics on PR titles.

Environments & Triggers

- Dev: Auto-deploys on `push` to `develop` (environment: `dev`).
- Staging: Auto-deploys on `push` to `main` (environment: `staging`).
- Production: Deploys on `release: published` (environment: `production`).
- Manual: `workflow_dispatch` supports deploying to any environment on demand.

Quality Gates

- `cargo fmt --check`, `cargo clippy -D warnings`
- `cargo test --workspace --all-features`
- `cargo audit --deny warnings`
- Trivy image scan: fails on `CRITICAL`/`HIGH` vulnerabilities.

Container Image

- Registry: `ghcr.io/<owner>/<repo>`.
- Tags:
  - Dev/Staging: short commit SHA (`${{ github.sha }}` prefix).
  - Production: release tag (e.g., `v1.2.3`).

Kubernetes Deployment

- Manifests in `deploy/k8s/` with a Deployment and Service for `codegraph-api`.
- RollingUpdate configured for zero-downtime (maxUnavailable=0, maxSurge=25%).
- Readiness and liveness probes against `/health` on port 3000.
- Script `scripts/deploy_k8s.sh` performs apply, set-image, rollout wait, and smoke test.
- Script `scripts/smoke_test.sh` runs an in-cluster curl against the service to validate health.

Automated Rollback

- If smoke test or rollout fails, the deploy script performs `kubectl rollout undo` to revert to the previous ReplicaSet automatically.

Required Secrets

Define GitHub Environment secrets for each environment:

- Dev environment (`dev`):
  - `DEV_KUBECONFIG`: Base64 or raw kubeconfig content for the dev cluster.

- Staging environment (`staging`):
  - `STAGING_KUBECONFIG`: Base64 or raw kubeconfig content for the staging cluster.

- Production environment (`production`):
  - `PROD_KUBECONFIG`: Base64 or raw kubeconfig content for the prod cluster.

Note: GHCR uses the default `GITHUB_TOKEN` to push images; grant `packages: write` permission in the workflow.

GitHub Environments

- Protect `staging` and `production` with required reviewers for manual approvals.
- Optionally add environment URLs for tracking.

Local Testing

- Build & run via Docker Compose locally: `docker compose up --build`.
- Kubernetes apply (dev/test clusters):
  - `kubectl apply -n codegraph-dev -f deploy/k8s/`
  - `kubectl set image -n codegraph-dev deployment/codegraph-api codegraph-api=ghcr.io/<owner>/<repo>:<tag>`
  - `kubectl rollout status -n codegraph-dev deployment/codegraph-api`

Conventional Commits & Releases

- PR titles are validated for Conventional Commits.
- Release Please opens a release PR with a semver bump based on commit history.
- When merged, a tag `vX.Y.Z` is created and `release.yml` publishes artifacts and images.

