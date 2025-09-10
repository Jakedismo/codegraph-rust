#!/usr/bin/env bash
set -euo pipefail

# Usage: deploy_k8s.sh <env> <image> [namespace]
# Example: deploy_k8s.sh dev ghcr.io/acme/codegraph:abc123 codegraph-dev

ENVIRONMENT=${1:-}
IMAGE=${2:-}
NAMESPACE=${3:-}

if [[ -z "$ENVIRONMENT" || -z "$IMAGE" ]]; then
  echo "Usage: $0 <env> <image> [namespace]"
  exit 2
fi

if [[ -z "$NAMESPACE" ]]; then
  NAMESPACE="codegraph-${ENVIRONMENT}"
fi

echo "Environment: $ENVIRONMENT"
echo "Image:       $IMAGE"
echo "Namespace:   $NAMESPACE"

# Expect KUBECONFIG content via env var KUBECONFIG_CONTENT (base64 or raw)
if [[ -z "${KUBECONFIG_CONTENT:-}" ]]; then
  echo "KUBECONFIG_CONTENT is required in environment"
  exit 3
fi

TMP_KUBECONFIG=$(mktemp)
if echo "$KUBECONFIG_CONTENT" | grep -q "apiVersion: v1"; then
  echo "$KUBECONFIG_CONTENT" > "$TMP_KUBECONFIG"
else
  # assume base64
  echo "$KUBECONFIG_CONTENT" | base64 -d > "$TMP_KUBECONFIG"
fi
export KUBECONFIG="$TMP_KUBECONFIG"

echo "Ensuring namespace exists..."
kubectl get ns "$NAMESPACE" >/dev/null 2>&1 || kubectl create ns "$NAMESPACE"

echo "Applying base manifests (deployment + service)..."
kubectl -n "$NAMESPACE" apply -f deploy/k8s/service.yaml
kubectl -n "$NAMESPACE" apply -f deploy/k8s/deployment.yaml

echo "Setting image to $IMAGE ..."
kubectl -n "$NAMESPACE" set image deployment/codegraph-api codegraph-api="$IMAGE" --record

echo "Waiting for rollout..."
kubectl -n "$NAMESPACE" rollout status deployment/codegraph-api --timeout=180s

echo "Running smoke test within cluster..."
bash "$(dirname "$0")/smoke_test.sh" "$NAMESPACE"

echo "Deployment to $ENVIRONMENT succeeded"

