#!/usr/bin/env bash
set -euo pipefail

# Enhanced Kubernetes deployment script for CodeGraph
# Usage: deploy_k8s.sh <env> <image> [namespace]
# Example: deploy_k8s.sh dev ghcr.io/acme/codegraph:abc123 codegraph-dev

ENVIRONMENT=${1:-}
IMAGE=${2:-}
NAMESPACE=${3:-}
DEPLOYMENT_TIMEOUT=${DEPLOYMENT_TIMEOUT:-300}
HEALTH_CHECK_TIMEOUT=${HEALTH_CHECK_TIMEOUT:-120}

if [[ -z "$ENVIRONMENT" || -z "$IMAGE" ]]; then
  echo "Usage: $0 <env> <image> [namespace]"
  echo "Environment variables:"
  echo "  DEPLOYMENT_TIMEOUT: Deployment timeout in seconds (default: 300)"
  echo "  HEALTH_CHECK_TIMEOUT: Health check timeout in seconds (default: 120)"
  exit 2
fi

if [[ -z "$NAMESPACE" ]]; then
  NAMESPACE="codegraph-${ENVIRONMENT}"
fi

# Environment-specific configuration
case $ENVIRONMENT in
  "dev")
    REPLICAS=1
    RESOURCE_REQUESTS_CPU="100m"
    RESOURCE_REQUESTS_MEMORY="256Mi"
    RESOURCE_LIMITS_CPU="500m"
    RESOURCE_LIMITS_MEMORY="512Mi"
    ;;
  "staging")
    REPLICAS=2
    RESOURCE_REQUESTS_CPU="200m"
    RESOURCE_REQUESTS_MEMORY="512Mi"
    RESOURCE_LIMITS_CPU="1000m"
    RESOURCE_LIMITS_MEMORY="1Gi"
    ;;
  "prod")
    REPLICAS=3
    RESOURCE_REQUESTS_CPU="500m"
    RESOURCE_REQUESTS_MEMORY="1Gi"
    RESOURCE_LIMITS_CPU="2000m"
    RESOURCE_LIMITS_MEMORY="2Gi"
    ;;
  *)
    echo "⚠️  Unknown environment: $ENVIRONMENT, using default configuration"
    REPLICAS=1
    RESOURCE_REQUESTS_CPU="100m"
    RESOURCE_REQUESTS_MEMORY="256Mi"
    RESOURCE_LIMITS_CPU="500m"
    RESOURCE_LIMITS_MEMORY="512Mi"
    ;;
esac

echo "🚀 Starting deployment to $ENVIRONMENT environment"
echo "📦 Image:       $IMAGE"
echo "🔧 Namespace:   $NAMESPACE"
echo "📊 Replicas:    $REPLICAS"
echo "⏱️  Timeout:     ${DEPLOYMENT_TIMEOUT}s"

# Expect KUBECONFIG content via env var KUBECONFIG_CONTENT (base64 or raw)
if [[ -z "${KUBECONFIG_CONTENT:-}" ]]; then
  echo "❌ KUBECONFIG_CONTENT is required in environment"
  exit 3
fi

echo "🔧 Setting up kubectl configuration..."
TMP_KUBECONFIG=$(mktemp)
if echo "$KUBECONFIG_CONTENT" | grep -q "apiVersion: v1"; then
  echo "$KUBECONFIG_CONTENT" > "$TMP_KUBECONFIG"
else
  # assume base64
  echo "$KUBECONFIG_CONTENT" | base64 -d > "$TMP_KUBECONFIG"
fi
export KUBECONFIG="$TMP_KUBECONFIG"

# Test cluster connectivity
echo "🔍 Testing cluster connectivity..."
if ! kubectl cluster-info >/dev/null 2>&1; then
  echo "❌ Failed to connect to Kubernetes cluster"
  exit 4
fi

echo "🏗️  Ensuring namespace exists..."
kubectl get ns "$NAMESPACE" >/dev/null 2>&1 || kubectl create ns "$NAMESPACE"

# Store current deployment for rollback if needed
echo "💾 Backing up current deployment state..."
BACKUP_FILE="/tmp/deployment-backup-${NAMESPACE}-$(date +%Y%m%d-%H%M%S).yaml"
if kubectl -n "$NAMESPACE" get deployment codegraph-api >/dev/null 2>&1; then
  kubectl -n "$NAMESPACE" get deployment codegraph-api -o yaml > "$BACKUP_FILE"
  CURRENT_IMAGE=$(kubectl -n "$NAMESPACE" get deployment codegraph-api -o jsonpath='{.spec.template.spec.containers[0].image}' 2>/dev/null || echo "none")
  echo "📄 Current image: $CURRENT_IMAGE"
  echo "💾 Backup saved to: $BACKUP_FILE"
else
  echo "ℹ️  No existing deployment found, this is a fresh deployment"
  CURRENT_IMAGE="none"
fi

# Create environment-specific ConfigMap
echo "⚙️  Creating configuration..."
kubectl -n "$NAMESPACE" create configmap codegraph-config \
  --from-literal=ENVIRONMENT="$ENVIRONMENT" \
  --from-literal=LOG_LEVEL="$([ "$ENVIRONMENT" = "prod" ] && echo "info" || echo "debug")" \
  --from-literal=RUST_LOG="$([ "$ENVIRONMENT" = "prod" ] && echo "info" || echo "debug")" \
  --from-literal=ROCKSDB_PATH="/data/rocksdb" \
  --from-literal=VECTOR_STORE_PATH="/data/vectors" \
  --from-literal=BIND_ADDRESS="0.0.0.0:8080" \
  --dry-run=client -o yaml | kubectl apply -f -

echo "📋 Applying base manifests..."
# Apply service first (stable resource)
kubectl -n "$NAMESPACE" apply -f deploy/k8s/service.yaml

# Apply deployment with resource updates
kubectl -n "$NAMESPACE" apply -f deploy/k8s/deployment.yaml

# Update deployment with environment-specific settings
echo "🔄 Updating deployment configuration..."
kubectl -n "$NAMESPACE" patch deployment codegraph-api --type=merge -p="{
  \"spec\": {
    \"replicas\": $REPLICAS,
    \"template\": {
      \"spec\": {
        \"containers\": [{
          \"name\": \"codegraph-api\",
          \"resources\": {
            \"requests\": {
              \"cpu\": \"$RESOURCE_REQUESTS_CPU\",
              \"memory\": \"$RESOURCE_REQUESTS_MEMORY\"
            },
            \"limits\": {
              \"cpu\": \"$RESOURCE_LIMITS_CPU\",
              \"memory\": \"$RESOURCE_LIMITS_MEMORY\"
            }
          }
        }]
      }
    }
  }
}"

echo "🎯 Setting image to $IMAGE..."
kubectl -n "$NAMESPACE" set image deployment/codegraph-api codegraph-api="$IMAGE" --record

echo "⏳ Waiting for rollout to complete..."
if ! kubectl -n "$NAMESPACE" rollout status deployment/codegraph-api --timeout="${DEPLOYMENT_TIMEOUT}s"; then
  echo "❌ Deployment failed or timed out"
  
  if [[ "$CURRENT_IMAGE" != "none" ]]; then
    echo "🔄 Rolling back to previous image: $CURRENT_IMAGE"
    kubectl -n "$NAMESPACE" set image deployment/codegraph-api codegraph-api="$CURRENT_IMAGE"
    kubectl -n "$NAMESPACE" rollout status deployment/codegraph-api --timeout=120s
    echo "✅ Rollback completed"
  fi
  
  echo "📊 Deployment status:"
  kubectl -n "$NAMESPACE" get pods -l app=codegraph-api
  kubectl -n "$NAMESPACE" describe deployment codegraph-api
  exit 5
fi

# Verify deployment health
echo "🏥 Verifying deployment health..."
if ! kubectl -n "$NAMESPACE" wait --for=condition=available --timeout=60s deployment/codegraph-api; then
  echo "❌ Deployment is not available"
  kubectl -n "$NAMESPACE" get pods -l app=codegraph-api
  exit 6
fi

# Run comprehensive health checks
echo "🔍 Running health checks..."
if [[ -f "$(dirname "$0")/health_check.sh" ]]; then
  if ! timeout "$HEALTH_CHECK_TIMEOUT" bash "$(dirname "$0")/health_check.sh" "$NAMESPACE"; then
    echo "❌ Health checks failed"
    exit 7
  fi
else
  echo "⚠️  health_check.sh not found, running basic smoke test..."
  if [[ -f "$(dirname "$0")/smoke_test.sh" ]]; then
    if ! timeout "$HEALTH_CHECK_TIMEOUT" bash "$(dirname "$0")/smoke_test.sh" "$NAMESPACE"; then
      echo "❌ Smoke test failed"
      exit 8
    fi
  else
    echo "⚠️  No health check scripts found, skipping detailed verification"
  fi
fi

# Display deployment summary
echo ""
echo "✅ Deployment to $ENVIRONMENT succeeded!"
echo "📋 Summary:"
echo "   Environment: $ENVIRONMENT"
echo "   Namespace: $NAMESPACE"
echo "   Image: $IMAGE"
echo "   Previous Image: $CURRENT_IMAGE"
echo "   Replicas: $REPLICAS"
echo "   Resources: ${RESOURCE_REQUESTS_CPU}/${RESOURCE_LIMITS_CPU} CPU, ${RESOURCE_REQUESTS_MEMORY}/${RESOURCE_LIMITS_MEMORY} memory"

echo ""
echo "🌐 Service Information:"
kubectl -n "$NAMESPACE" get svc codegraph-api

echo ""
echo "📊 Pod Status:"
kubectl -n "$NAMESPACE" get pods -l app=codegraph-api -o wide

# Cleanup
rm -f "$TMP_KUBECONFIG"
echo ""
echo "🎉 Deployment completed successfully!"

