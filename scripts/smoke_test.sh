#!/usr/bin/env bash
set -euo pipefail

# Usage: smoke_test.sh <namespace>
NAMESPACE=${1:-default}

echo "Executing in namespace: $NAMESPACE"

# Use ephemeral curl pod to test the service endpoint inside the cluster
kubectl -n "$NAMESPACE" run smoke-curl --image=curlimages/curl:8.7.1 --restart=Never \
  --rm -i -- \
  curl -fsS --max-time 10 http://codegraph-api:3000/health

echo "Smoke test passed"
