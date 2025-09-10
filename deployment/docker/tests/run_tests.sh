#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")"/../../.. && pwd)"
COMPOSE_FILE="$ROOT_DIR/deployment/docker/docker-compose.yml"
DOCKERFILE="$ROOT_DIR/deployment/docker/Dockerfile"

failures=0
passes=0

pass() { echo "[PASS] $1"; passes=$((passes+1)); }
fail() { echo "[FAIL] $1"; failures=$((failures+1)); }

test_exists() {
  [[ -f "$COMPOSE_FILE" ]] && pass "compose file exists" || fail "compose file missing"
  [[ -f "$DOCKERFILE" ]] && pass "Dockerfile exists" || fail "Dockerfile missing"
}

test_compose_valid() {
  if docker compose -f "$COMPOSE_FILE" config >/dev/null 2>&1; then
    pass "compose config validates"
  else
    fail "compose config invalid"
  fi
}

test_services_present() {
  grep -qE "^\s*api:\s*$" "$COMPOSE_FILE" && pass "api service present" || fail "api service missing"
  grep -qE "^\s*vector-maintainer:\s*$" "$COMPOSE_FILE" && pass "vector-maintainer service present" || fail "vector-maintainer service missing"
  grep -qE "^\s*graph-backup:\s*$" "$COMPOSE_FILE" && pass "graph-backup service present" || fail "graph-backup service missing"
  grep -qE "^\s*prometheus:\s*$" "$COMPOSE_FILE" && pass "prometheus service present" || fail "prometheus service missing"
  grep -qE "^\s*grafana:\s*$" "$COMPOSE_FILE" && pass "grafana service present" || fail "grafana service missing"
}

test_healthchecks() {
  grep -q "healthcheck:" "$COMPOSE_FILE" && pass "healthchecks configured" || fail "missing healthchecks"
}

test_resource_limits() {
  grep -qE "mem_limit:|deploy:\n\s*resources:" "$COMPOSE_FILE" && pass "resource limits present" || fail "resource limits missing"
}

test_security_opts() {
  grep -q "no-new-privileges" "$COMPOSE_FILE" && pass "no-new-privileges set" || fail "no-new-privileges missing"
  grep -q "read_only: true" "$COMPOSE_FILE" && pass "read-only FS set" || fail "read-only FS missing"
  grep -q "cap_drop:" "$COMPOSE_FILE" && pass "capabilities dropped" || fail "capabilities not dropped"
}

test_volumes() {
  grep -q "graph-data:" "$COMPOSE_FILE" && pass "graph-data volume defined" || fail "graph-data volume missing"
  grep -q "backups:" "$COMPOSE_FILE" && pass "backups volume defined" || fail "backups volume missing"
}

test_scripts_exist() {
  for f in start.sh stop.sh scale.sh backup.sh restore.sh vector-maintainer.sh; do
    if [[ -f "$ROOT_DIR/deployment/docker/scripts/$f" ]]; then pass "script $f exists"; else fail "script $f missing"; fi
  done
}

test_dockerfile_multistage() {
  grep -q "FROM rust:1.75-slim AS builder" "$DOCKERFILE" && pass "builder stage present" || fail "builder stage missing"
  grep -q "FROM debian:bookworm-slim AS runtime" "$DOCKERFILE" && pass "runtime stage present" || fail "runtime stage missing"
}

test_dockerfile_nonroot() {
  grep -q "USER codegraph:codegraph" "$DOCKERFILE" && pass "non-root user set" || fail "non-root user missing"
}

test_dockerfile_expose() {
  grep -q "EXPOSE 8080" "$DOCKERFILE" && pass "port 8080 exposed" || fail "port 8080 not exposed"
}

test_build_image_size() {
  if [[ "${RUN_BUILD_TEST:-0}" == "1" ]]; then
    echo "[info] Building image for size test (this may take a while)..."
    IMG_ID=$(docker build -q -f "$DOCKERFILE" "$ROOT_DIR")
    SIZE_BYTES=$(docker image inspect "$IMG_ID" --format '{{.Size}}')
    SIZE_MB=$(( (SIZE_BYTES + 1024*1024 - 1) / (1024*1024) ))
    echo "[info] Image size: ${SIZE_MB} MB"
    if (( SIZE_MB < 200 )); then pass "image size < 200MB"; else fail "image size >= 200MB (${SIZE_MB}MB)"; fi
    docker image rm "$IMG_ID" >/dev/null 2>&1 || true
  else
    pass "image size test skipped (set RUN_BUILD_TEST=1 to enable)"
  fi
}

test_integration_health() {
  if [[ "${RUN_INTEGRATION:-0}" == "1" ]]; then
    docker compose -f "$COMPOSE_FILE" up -d --build api >/dev/null
    echo "[info] Waiting up to 30s for API health..."
    set +e
    for i in {1..30}; do
      if curl -fsS http://127.0.0.1:8080/health/ready >/dev/null 2>&1; then
        set -e
        pass "API health endpoint reachable"
        docker compose -f "$COMPOSE_FILE" down >/dev/null
        return
      fi
      sleep 1
    done
    set -e
    fail "API health not reachable within 30s"
    docker compose -f "$COMPOSE_FILE" down >/dev/null || true
  else
    pass "integration health test skipped (set RUN_INTEGRATION=1 to enable)"
  fi
}

main() {
  test_exists
  test_compose_valid
  test_services_present
  test_healthchecks
  test_resource_limits
  test_security_opts
  test_volumes
  test_scripts_exist
  test_dockerfile_multistage
  test_dockerfile_nonroot
  test_dockerfile_expose
  # Extra coverage checks
  if grep -q "8080:8080" "$COMPOSE_FILE"; then pass "port mapping 8080:8080 present"; else fail "port mapping missing"; fi
  if grep -q "user: \"10001:10001\"" "$COMPOSE_FILE"; then pass "non-root UID set in compose"; else fail "non-root UID missing in compose"; fi
  if grep -q "tmpfs:" "$COMPOSE_FILE"; then pass "tmpfs configured"; else fail "tmpfs not configured"; fi
  if grep -q "ulimits:" "$COMPOSE_FILE"; then pass "ulimits configured"; else fail "ulimits not configured"; fi
  if grep -q "networks:" "$COMPOSE_FILE"; then pass "networks section present"; else fail "networks section missing"; fi
  if grep -q "build:" "$COMPOSE_FILE"; then pass "build section present"; else fail "build section missing"; fi
  if grep -q "dockerfile: deployment/docker/Dockerfile" "$COMPOSE_FILE"; then pass "build uses deployment/docker/Dockerfile"; else fail "build dockerfile path missing"; fi
  if grep -q "HEALTHCHECK" "$DOCKERFILE"; then pass "HEALTHCHECK in Dockerfile"; else fail "HEALTHCHECK missing in Dockerfile"; fi
  if grep -q "VOLUME \[\"/var/lib/codegraph\", \"/app/config\"\]" "$DOCKERFILE"; then pass "volumes declared in Dockerfile"; else fail "volumes not declared in Dockerfile"; fi
  test_build_image_size
  test_integration_health

  echo "[result] passes=$passes failures=$failures"
  [[ $failures -eq 0 ]] || exit 1
}

main "$@"
