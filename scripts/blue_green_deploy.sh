#!/bin/bash

set -euo pipefail

# Blue-Green deployment script for CodeGraph
# Usage: blue_green_deploy.sh <environment> <image> [namespace]

ENVIRONMENT=${1:-}
IMAGE=${2:-}
NAMESPACE=${3:-}
DEPLOYMENT_TIMEOUT=${DEPLOYMENT_TIMEOUT:-300}
HEALTH_CHECK_TIMEOUT=${HEALTH_CHECK_TIMEOUT:-120}
TRAFFIC_SWITCH_TIMEOUT=${TRAFFIC_SWITCH_TIMEOUT:-60}

if [[ -z "$ENVIRONMENT" || -z "$IMAGE" ]]; then
    echo "Usage: $0 <environment> <image> [namespace]"
    echo "Environment variables:"
    echo "  DEPLOYMENT_TIMEOUT: Deployment timeout in seconds (default: 300)"
    echo "  HEALTH_CHECK_TIMEOUT: Health check timeout in seconds (default: 120)"
    echo "  TRAFFIC_SWITCH_TIMEOUT: Traffic switch timeout in seconds (default: 60)"
    exit 1
fi

if [[ -z "$NAMESPACE" ]]; then
    NAMESPACE="codegraph-${ENVIRONMENT}"
fi

# Only use blue-green for production and staging
if [[ "$ENVIRONMENT" != "prod" && "$ENVIRONMENT" != "staging" ]]; then
    echo "ℹ️  Blue-green deployment is only used for prod and staging environments"
    echo "🔄 Using standard rolling deployment for $ENVIRONMENT"
    exec "$(dirname "$0")/deploy_k8s.sh" "$ENVIRONMENT" "$IMAGE" "$NAMESPACE"
fi

APP_NAME="codegraph-api"
SERVICE_NAME="codegraph-api"
BLUE_DEPLOYMENT="${APP_NAME}-blue"
GREEN_DEPLOYMENT="${APP_NAME}-green"

echo "🔵🟢 Starting Blue-Green deployment to $ENVIRONMENT"
echo "📦 Image: $IMAGE"
echo "🔧 Namespace: $NAMESPACE"
echo "⏱️  Deployment timeout: ${DEPLOYMENT_TIMEOUT}s"
echo "🏥 Health check timeout: ${HEALTH_CHECK_TIMEOUT}s"

# Setup kubeconfig
if [[ -z "${KUBECONFIG_CONTENT:-}" ]]; then
    echo "❌ KUBECONFIG_CONTENT is required in environment"
    exit 2
fi

echo "🔧 Setting up kubectl configuration..."
TMP_KUBECONFIG=$(mktemp)
if echo "$KUBECONFIG_CONTENT" | grep -q "apiVersion: v1"; then
    echo "$KUBECONFIG_CONTENT" > "$TMP_KUBECONFIG"
else
    echo "$KUBECONFIG_CONTENT" | base64 -d > "$TMP_KUBECONFIG"
fi
export KUBECONFIG="$TMP_KUBECONFIG"

# Test cluster connectivity
echo "🔍 Testing cluster connectivity..."
if ! kubectl cluster-info >/dev/null 2>&1; then
    echo "❌ Failed to connect to Kubernetes cluster"
    exit 3
fi

# Ensure namespace exists
echo "🏗️  Ensuring namespace exists..."
kubectl create namespace "$NAMESPACE" --dry-run=client -o yaml | kubectl apply -f -

# Environment-specific configuration
case $ENVIRONMENT in
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
esac

# Function to get current active deployment (blue or green)
get_active_deployment() {
    local current_selector
    current_selector=$(kubectl -n "$NAMESPACE" get service "$SERVICE_NAME" -o jsonpath='{.spec.selector.version}' 2>/dev/null || echo "none")
    
    case "$current_selector" in
        "blue")
            echo "blue"
            ;;
        "green")
            echo "green"
            ;;
        *)
            # If no specific version selector, check which deployment exists and is ready
            if kubectl -n "$NAMESPACE" get deployment "$BLUE_DEPLOYMENT" >/dev/null 2>&1; then
                local blue_ready
                blue_ready=$(kubectl -n "$NAMESPACE" get deployment "$BLUE_DEPLOYMENT" -o jsonpath='{.status.readyReplicas}' 2>/dev/null || echo "0")
                if [[ "$blue_ready" -gt 0 ]]; then
                    echo "blue"
                    return
                fi
            fi
            
            if kubectl -n "$NAMESPACE" get deployment "$GREEN_DEPLOYMENT" >/dev/null 2>&1; then
                local green_ready
                green_ready=$(kubectl -n "$NAMESPACE" get deployment "$GREEN_DEPLOYMENT" -o jsonpath='{.status.readyReplicas}' 2>/dev/null || echo "0")
                if [[ "$green_ready" -gt 0 ]]; then
                    echo "green"
                    return
                fi
            fi
            
            echo "none"
            ;;
    esac
}

# Function to get inactive deployment
get_inactive_deployment() {
    local active=$1
    case "$active" in
        "blue")
            echo "green"
            ;;
        "green")
            echo "blue"
            ;;
        *)
            echo "blue"  # Default to blue for first deployment
            ;;
    esac
}

# Function to create deployment manifest
create_deployment_manifest() {
    local deployment_name=$1
    local version_label=$2
    
    cat <<EOF
apiVersion: apps/v1
kind: Deployment
metadata:
  name: ${deployment_name}
  namespace: ${NAMESPACE}
  labels:
    app: ${APP_NAME}
    environment: ${ENVIRONMENT}
    version: ${version_label}
spec:
  replicas: ${REPLICAS}
  strategy:
    type: RollingUpdate
    rollingUpdate:
      maxUnavailable: 0
      maxSurge: 2
  selector:
    matchLabels:
      app: ${APP_NAME}
      version: ${version_label}
  template:
    metadata:
      labels:
        app: ${APP_NAME}
        environment: ${ENVIRONMENT}
        version: ${version_label}
      annotations:
        prometheus.io/scrape: "true"
        prometheus.io/port: "8080"
        prometheus.io/path: "/metrics"
        deployment.kubernetes.io/revision: "$(date +%s)"
    spec:
      securityContext:
        runAsNonRoot: true
        runAsUser: 1000
        fsGroup: 1000
      containers:
      - name: codegraph-api
        image: ${IMAGE}
        imagePullPolicy: Always
        ports:
        - containerPort: 8080
          name: http
          protocol: TCP
        - containerPort: 9090
          name: metrics
          protocol: TCP
        env:
        - name: ENVIRONMENT
          value: "${ENVIRONMENT}"
        - name: LOG_LEVEL
          value: "$([ "$ENVIRONMENT" = "prod" ] && echo "info" || echo "debug")"
        - name: RUST_LOG
          value: "$([ "$ENVIRONMENT" = "prod" ] && echo "info" || echo "debug")"
        - name: ROCKSDB_PATH
          value: "/data/rocksdb"
        - name: VECTOR_STORE_PATH
          value: "/data/vectors"
        - name: BIND_ADDRESS
          value: "0.0.0.0:8080"
        - name: DEPLOYMENT_VERSION
          value: "${version_label}"
        resources:
          requests:
            cpu: ${RESOURCE_REQUESTS_CPU}
            memory: ${RESOURCE_REQUESTS_MEMORY}
          limits:
            cpu: ${RESOURCE_LIMITS_CPU}
            memory: ${RESOURCE_LIMITS_MEMORY}
        livenessProbe:
          httpGet:
            path: /health
            port: 8080
          initialDelaySeconds: 30
          periodSeconds: 10
          timeoutSeconds: 5
          failureThreshold: 3
        readinessProbe:
          httpGet:
            path: /ready
            port: 8080
          initialDelaySeconds: 10
          periodSeconds: 5
          timeoutSeconds: 3
          failureThreshold: 3
        volumeMounts:
        - name: data
          mountPath: /data
      volumes:
      - name: data
        persistentVolumeClaim:
          claimName: ${APP_NAME}-data-${version_label}
      nodeSelector:
        kubernetes.io/arch: amd64
      tolerations:
      - key: "node.kubernetes.io/not-ready"
        operator: "Exists"
        effect: "NoExecute"
        tolerationSeconds: 300
      - key: "node.kubernetes.io/unreachable"
        operator: "Exists"
        effect: "NoExecute"
        tolerationSeconds: 300
EOF
}

# Function to create PVC for deployment
create_pvc() {
    local version_label=$1
    local storage_size
    storage_size=$([[ $ENVIRONMENT == "prod" ]] && echo "20Gi" || echo "10Gi")
    
    cat <<EOF | kubectl apply -f -
apiVersion: v1
kind: PersistentVolumeClaim
metadata:
  name: ${APP_NAME}-data-${version_label}
  namespace: ${NAMESPACE}
spec:
  accessModes:
    - ReadWriteOnce
  resources:
    requests:
      storage: ${storage_size}
  storageClassName: gp2
EOF
}

# Function to ensure service exists
ensure_service_exists() {
    if ! kubectl -n "$NAMESPACE" get service "$SERVICE_NAME" >/dev/null 2>&1; then
        echo "🌐 Creating main service..."
        cat <<EOF | kubectl apply -f -
apiVersion: v1
kind: Service
metadata:
  name: ${SERVICE_NAME}
  namespace: ${NAMESPACE}
  labels:
    app: ${APP_NAME}
    environment: ${ENVIRONMENT}
spec:
  selector:
    app: ${APP_NAME}
    version: blue  # Default to blue initially
  ports:
  - name: http
    port: 80
    targetPort: 8080
    protocol: TCP
  - name: metrics
    port: 9090
    targetPort: 9090
    protocol: TCP
  type: ClusterIP
EOF
    fi
}

# Function to switch traffic
switch_traffic() {
    local target_version=$1
    echo "🔄 Switching traffic to $target_version deployment..."
    
    kubectl -n "$NAMESPACE" patch service "$SERVICE_NAME" -p "{\"spec\":{\"selector\":{\"app\":\"$APP_NAME\",\"version\":\"$target_version\"}}}"
    
    # Wait for service to update
    sleep 5
    
    # Verify the switch
    local current_selector
    current_selector=$(kubectl -n "$NAMESPACE" get service "$SERVICE_NAME" -o jsonpath='{.spec.selector.version}')
    
    if [[ "$current_selector" == "$target_version" ]]; then
        echo "✅ Traffic successfully switched to $target_version"
        return 0
    else
        echo "❌ Failed to switch traffic to $target_version (current: $current_selector)"
        return 1
    fi
}

# Function to run health checks on specific deployment
health_check_deployment() {
    local version_label=$1
    local deployment_name="${APP_NAME}-${version_label}"
    
    echo "🏥 Running health checks on $version_label deployment..."
    
    # Check if deployment is ready
    if ! kubectl -n "$NAMESPACE" wait --for=condition=available --timeout=60s deployment/"$deployment_name"; then
        echo "❌ $deployment_name deployment is not available"
        return 1
    fi
    
    # Port-forward to test the deployment directly
    echo "🔍 Testing $version_label deployment health endpoints..."
    local port_forward_pid
    kubectl -n "$NAMESPACE" port-forward "deployment/$deployment_name" 8080:8080 >/dev/null 2>&1 &
    port_forward_pid=$!
    sleep 3
    
    local health_ok=true
    
    # Test health endpoint
    local health_response
    health_response=$(curl -s -o /dev/null -w "%{http_code}" "http://localhost:8080/health" || echo "000")
    if [[ "$health_response" != "200" ]]; then
        echo "❌ Health endpoint failed: HTTP $health_response"
        health_ok=false
    else
        echo "✅ Health endpoint: HTTP $health_response"
    fi
    
    # Test readiness endpoint
    local ready_response
    ready_response=$(curl -s -o /dev/null -w "%{http_code}" "http://localhost:8080/ready" || echo "000")
    if [[ "$ready_response" != "200" ]]; then
        echo "❌ Readiness endpoint failed: HTTP $ready_response"
        health_ok=false
    else
        echo "✅ Readiness endpoint: HTTP $ready_response"
    fi
    
    # Cleanup port-forward
    kill $port_forward_pid 2>/dev/null || true
    wait $port_forward_pid 2>/dev/null || true
    
    if [[ "$health_ok" == "true" ]]; then
        echo "✅ $version_label deployment health checks passed"
        return 0
    else
        echo "❌ $version_label deployment health checks failed"
        return 1
    fi
}

# Function to cleanup old deployment
cleanup_old_deployment() {
    local old_version=$1
    local old_deployment="${APP_NAME}-${old_version}"
    
    echo "🧹 Cleaning up old $old_version deployment..."
    
    # Scale down old deployment
    kubectl -n "$NAMESPACE" scale deployment "$old_deployment" --replicas=0 || true
    
    # Wait a bit for pods to terminate gracefully
    sleep 10
    
    # Delete old deployment
    kubectl -n "$NAMESPACE" delete deployment "$old_deployment" || true
    
    echo "✅ Old $old_version deployment cleaned up"
}

# Main blue-green deployment logic
main() {
    local start_time
    start_time=$(date +%s)
    
    echo "🔍 Determining current deployment state..."
    
    # Get current active deployment
    local active_deployment
    active_deployment=$(get_active_deployment)
    echo "📊 Current active deployment: $active_deployment"
    
    # Determine target deployment
    local target_deployment
    target_deployment=$(get_inactive_deployment "$active_deployment")
    echo "🎯 Target deployment: $target_deployment"
    
    local target_deployment_name="${APP_NAME}-${target_deployment}"
    
    # Ensure service exists
    ensure_service_exists
    
    # Create PVC for target deployment
    echo "💾 Creating persistent volume for $target_deployment deployment..."
    create_pvc "$target_deployment"
    
    # Deploy to inactive environment
    echo "🚀 Deploying $IMAGE to $target_deployment environment..."
    create_deployment_manifest "$target_deployment_name" "$target_deployment" | kubectl apply -f -
    
    # Wait for deployment to be ready
    echo "⏳ Waiting for $target_deployment deployment to be ready..."
    if ! kubectl -n "$NAMESPACE" rollout status deployment/"$target_deployment_name" --timeout="${DEPLOYMENT_TIMEOUT}s"; then
        echo "❌ $target_deployment deployment failed"
        
        # Show debugging information
        echo "📊 Deployment status:"
        kubectl -n "$NAMESPACE" get pods -l version="$target_deployment"
        kubectl -n "$NAMESPACE" describe deployment "$target_deployment_name"
        
        exit 4
    fi
    
    # Run health checks on new deployment
    if ! timeout "$HEALTH_CHECK_TIMEOUT" health_check_deployment "$target_deployment"; then
        echo "❌ Health checks failed for $target_deployment deployment"
        exit 5
    fi
    
    # Switch traffic to new deployment
    echo "🔄 Switching traffic from $active_deployment to $target_deployment..."
    if ! timeout "$TRAFFIC_SWITCH_TIMEOUT" switch_traffic "$target_deployment"; then
        echo "❌ Failed to switch traffic"
        exit 6
    fi
    
    # Run health checks on the service endpoint after traffic switch
    echo "🏥 Verifying service health after traffic switch..."
    sleep 5  # Allow some time for traffic to stabilize
    
    if [[ -f "$(dirname "$0")/health_check.sh" ]]; then
        if ! timeout "$HEALTH_CHECK_TIMEOUT" bash "$(dirname "$0")/health_check.sh" "$NAMESPACE"; then
            echo "❌ Service health checks failed after traffic switch"
            echo "🔄 Rolling back traffic to $active_deployment..."
            
            if [[ "$active_deployment" != "none" ]]; then
                switch_traffic "$active_deployment"
                echo "✅ Traffic rolled back to $active_deployment"
            fi
            
            exit 7
        fi
    else
        echo "⚠️  health_check.sh not found, skipping post-switch verification"
    fi
    
    # Cleanup old deployment if it exists
    if [[ "$active_deployment" != "none" ]]; then
        echo "🧹 Cleaning up old deployment after successful switch..."
        # Wait a bit to ensure traffic switch is stable
        sleep 30
        cleanup_old_deployment "$active_deployment"
    fi
    
    # Calculate execution time
    local end_time
    end_time=$(date +%s)
    local execution_time=$((end_time - start_time))
    
    # Show final status
    echo ""
    echo "🎉 Blue-Green deployment completed successfully in ${execution_time}s!"
    echo "📋 Summary:"
    echo "   Environment: $ENVIRONMENT"
    echo "   Namespace: $NAMESPACE"
    echo "   Image: $IMAGE"
    echo "   Previous active: $active_deployment"
    echo "   Current active: $target_deployment"
    echo "   Replicas: $REPLICAS"
    
    echo ""
    echo "🌐 Service Information:"
    kubectl -n "$NAMESPACE" get svc "$SERVICE_NAME"
    
    echo ""
    echo "📊 Pod Status:"
    kubectl -n "$NAMESPACE" get pods -l app="$APP_NAME" -o wide
    
    # Cleanup
    rm -f "$TMP_KUBECONFIG"
}

# Validate dependencies
if ! command -v kubectl >/dev/null 2>&1; then
    echo "❌ kubectl is not installed or not in PATH"
    exit 1
fi

if ! command -v curl >/dev/null 2>&1; then
    echo "❌ curl is not installed or not in PATH"
    exit 1
fi

# Run main function
main