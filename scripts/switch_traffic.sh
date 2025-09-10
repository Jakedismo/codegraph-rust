#!/bin/bash

set -euo pipefail

# Traffic switching script for Blue-Green deployments
# Usage: switch_traffic.sh <environment> <target_version> [namespace] [rollback_on_failure]

ENVIRONMENT=${1:-}
TARGET_VERSION=${2:-}
NAMESPACE=${3:-}
ROLLBACK_ON_FAILURE=${4:-"true"}
HEALTH_CHECK_TIMEOUT=${HEALTH_CHECK_TIMEOUT:-60}
TRAFFIC_STABILIZATION_TIME=${TRAFFIC_STABILIZATION_TIME:-10}

if [[ -z "$ENVIRONMENT" || -z "$TARGET_VERSION" ]]; then
    echo "Usage: $0 <environment> <target_version> [namespace] [rollback_on_failure]"
    echo ""
    echo "Arguments:"
    echo "  environment:           Target environment (dev|staging|prod)"
    echo "  target_version:        Target deployment version (blue|green)"
    echo "  namespace:             Kubernetes namespace (optional)"
    echo "  rollback_on_failure:   Auto-rollback on health check failure (default: true)"
    echo ""
    echo "Environment variables:"
    echo "  HEALTH_CHECK_TIMEOUT:         Health check timeout in seconds (default: 60)"
    echo "  TRAFFIC_STABILIZATION_TIME:   Time to wait after switch in seconds (default: 10)"
    echo "  KUBECONFIG_CONTENT:           Kubernetes config content (required)"
    exit 1
fi

if [[ -z "$NAMESPACE" ]]; then
    NAMESPACE="codegraph-${ENVIRONMENT}"
fi

# Validate target version
if [[ "$TARGET_VERSION" != "blue" && "$TARGET_VERSION" != "green" ]]; then
    echo "❌ Invalid target version: $TARGET_VERSION (must be 'blue' or 'green')"
    exit 1
fi

APP_NAME="codegraph-api"
SERVICE_NAME="codegraph-api"

echo "🔄 Traffic switching for CodeGraph"
echo "🎯 Target version: $TARGET_VERSION"
echo "🔧 Environment: $ENVIRONMENT"
echo "🏷️  Namespace: $NAMESPACE"
echo "🔄 Auto-rollback: $ROLLBACK_ON_FAILURE"

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
if ! kubectl cluster-info >/dev/null 2>&1; then
    echo "❌ Failed to connect to Kubernetes cluster"
    exit 3
fi

# Function to get current active version
get_current_version() {
    kubectl -n "$NAMESPACE" get service "$SERVICE_NAME" -o jsonpath='{.spec.selector.version}' 2>/dev/null || echo "unknown"
}

# Function to check if target deployment exists and is ready
check_target_deployment() {
    local target_deployment="${APP_NAME}-${TARGET_VERSION}"
    
    echo "🔍 Checking if $target_deployment exists and is ready..."
    
    # Check if deployment exists
    if ! kubectl -n "$NAMESPACE" get deployment "$target_deployment" >/dev/null 2>&1; then
        echo "❌ Deployment $target_deployment does not exist"
        return 1
    fi
    
    # Check if deployment is available
    local available
    available=$(kubectl -n "$NAMESPACE" get deployment "$target_deployment" -o jsonpath='{.status.conditions[?(@.type=="Available")].status}' 2>/dev/null || echo "False")
    
    if [[ "$available" != "True" ]]; then
        echo "❌ Deployment $target_deployment is not available"
        kubectl -n "$NAMESPACE" get deployment "$target_deployment"
        return 1
    fi
    
    # Check if all replicas are ready
    local ready_replicas
    local desired_replicas
    ready_replicas=$(kubectl -n "$NAMESPACE" get deployment "$target_deployment" -o jsonpath='{.status.readyReplicas}' 2>/dev/null || echo "0")
    desired_replicas=$(kubectl -n "$NAMESPACE" get deployment "$target_deployment" -o jsonpath='{.spec.replicas}' 2>/dev/null || echo "1")
    
    if [[ "$ready_replicas" -ne "$desired_replicas" ]]; then
        echo "❌ Not all replicas are ready: $ready_replicas/$desired_replicas"
        return 1
    fi
    
    echo "✅ Target deployment $target_deployment is ready ($ready_replicas/$desired_replicas replicas)"
    return 0
}

# Function to perform the traffic switch
perform_traffic_switch() {
    local target_version=$1
    
    echo "🔄 Switching traffic to $target_version..."
    
    # Record the switch time for metrics
    local switch_start_time
    switch_start_time=$(date +%s)
    
    # Update service selector to point to target version
    kubectl -n "$NAMESPACE" patch service "$SERVICE_NAME" --type=merge -p "{
        \"spec\": {
            \"selector\": {
                \"app\": \"$APP_NAME\",
                \"version\": \"$target_version\"
            }
        },
        \"metadata\": {
            \"annotations\": {
                \"traffic.switch/timestamp\": \"$(date -Iseconds)\",
                \"traffic.switch/target-version\": \"$target_version\",
                \"traffic.switch/switched-by\": \"switch_traffic.sh\"
            }
        }
    }"
    
    # Wait for traffic stabilization
    echo "⏳ Waiting ${TRAFFIC_STABILIZATION_TIME}s for traffic to stabilize..."
    sleep "$TRAFFIC_STABILIZATION_TIME"
    
    # Verify the switch was successful
    local current_version
    current_version=$(get_current_version)
    
    if [[ "$current_version" == "$target_version" ]]; then
        local switch_end_time
        switch_end_time=$(date +%s)
        local switch_duration=$((switch_end_time - switch_start_time))
        
        echo "✅ Traffic successfully switched to $target_version in ${switch_duration}s"
        return 0
    else
        echo "❌ Traffic switch verification failed"
        echo "   Expected: $target_version"
        echo "   Current:  $current_version"
        return 1
    fi
}

# Function to run health checks after traffic switch
verify_health_after_switch() {
    echo "🏥 Verifying service health after traffic switch..."
    
    # Wait a bit for metrics to stabilize
    sleep 5
    
    # Use dedicated health check script if available
    if [[ -f "$(dirname "$0")/health_check.sh" ]]; then
        if timeout "$HEALTH_CHECK_TIMEOUT" bash "$(dirname "$0")/health_check.sh" "$NAMESPACE"; then
            echo "✅ Health verification passed"
            return 0
        else
            echo "❌ Health verification failed"
            return 1
        fi
    else
        echo "⚠️  Dedicated health check script not found, performing basic checks..."
        
        # Basic health check using port-forward
        local port_forward_pid
        kubectl -n "$NAMESPACE" port-forward svc/"$SERVICE_NAME" 8080:80 >/dev/null 2>&1 &
        port_forward_pid=$!
        sleep 3
        
        local health_response
        health_response=$(curl -s -o /dev/null -w "%{http_code}" "http://localhost:8080/health" || echo "000")
        
        # Cleanup port-forward
        kill $port_forward_pid 2>/dev/null || true
        wait $port_forward_pid 2>/dev/null || true
        
        if [[ "$health_response" == "200" ]]; then
            echo "✅ Basic health check passed"
            return 0
        else
            echo "❌ Basic health check failed (HTTP $health_response)"
            return 1
        fi
    fi
}

# Function to rollback traffic
rollback_traffic() {
    local original_version=$1
    
    if [[ "$original_version" == "unknown" || "$original_version" == "$TARGET_VERSION" ]]; then
        echo "⚠️  Cannot rollback: original version is unknown or same as target"
        return 1
    fi
    
    echo "🔄 Rolling back traffic to $original_version..."
    
    kubectl -n "$NAMESPACE" patch service "$SERVICE_NAME" --type=merge -p "{
        \"spec\": {
            \"selector\": {
                \"app\": \"$APP_NAME\",
                \"version\": \"$original_version\"
            }
        },
        \"metadata\": {
            \"annotations\": {
                \"traffic.rollback/timestamp\": \"$(date -Iseconds)\",
                \"traffic.rollback/from-version\": \"$TARGET_VERSION\",
                \"traffic.rollback/to-version\": \"$original_version\",
                \"traffic.rollback/reason\": \"health-check-failure\"
            }
        }
    }"
    
    sleep 5
    
    # Verify rollback
    local current_version
    current_version=$(get_current_version)
    
    if [[ "$current_version" == "$original_version" ]]; then
        echo "✅ Traffic successfully rolled back to $original_version"
        return 0
    else
        echo "❌ Rollback verification failed"
        return 1
    fi
}

# Function to display traffic distribution (for monitoring)
show_traffic_status() {
    echo ""
    echo "📊 Current traffic status:"
    echo "🌐 Service configuration:"
    kubectl -n "$NAMESPACE" get svc "$SERVICE_NAME" -o custom-columns="NAME:.metadata.name,TYPE:.spec.type,CLUSTER-IP:.spec.clusterIP,EXTERNAL-IP:.status.loadBalancer.ingress[0].ip,SELECTOR:.spec.selector"
    
    echo ""
    echo "📋 Active deployments:"
    kubectl -n "$NAMESPACE" get deployments -l app="$APP_NAME" -o custom-columns="NAME:.metadata.name,READY:.status.readyReplicas,UP-TO-DATE:.status.updatedReplicas,AVAILABLE:.status.availableReplicas,VERSION:.metadata.labels.version"
    
    echo ""
    echo "🏷️  Pod distribution:"
    kubectl -n "$NAMESPACE" get pods -l app="$APP_NAME" -o custom-columns="NAME:.metadata.name,READY:.status.conditions[?(@.type==\"Ready\")].status,STATUS:.status.phase,VERSION:.metadata.labels.version,NODE:.spec.nodeName"
    
    # Show recent traffic switch annotations
    echo ""
    echo "📝 Recent traffic operations:"
    kubectl -n "$NAMESPACE" get svc "$SERVICE_NAME" -o jsonpath='{.metadata.annotations}' | jq -r 'to_entries[] | select(.key | startswith("traffic.")) | "\(.key): \(.value)"' 2>/dev/null || echo "No traffic operation annotations found"
}

# Main execution
main() {
    local start_time
    start_time=$(date +%s)
    
    # Get current version before switching
    local original_version
    original_version=$(get_current_version)
    echo "📊 Current active version: $original_version"
    
    # Check if we're already pointing to the target version
    if [[ "$original_version" == "$TARGET_VERSION" ]]; then
        echo "ℹ️  Traffic is already pointing to $TARGET_VERSION"
        show_traffic_status
        exit 0
    fi
    
    # Verify target deployment is ready
    if ! check_target_deployment; then
        echo "❌ Target deployment is not ready for traffic switch"
        exit 4
    fi
    
    # Perform the traffic switch
    if ! perform_traffic_switch "$TARGET_VERSION"; then
        echo "❌ Traffic switch failed"
        exit 5
    fi
    
    # Verify health after switch
    local health_check_passed=true
    if ! verify_health_after_switch; then
        health_check_passed=false
        
        if [[ "$ROLLBACK_ON_FAILURE" == "true" ]]; then
            echo "🔄 Health checks failed, attempting rollback..."
            
            if rollback_traffic "$original_version"; then
                echo "✅ Successfully rolled back to $original_version"
                show_traffic_status
                exit 6
            else
                echo "❌ Rollback failed - manual intervention required!"
                show_traffic_status
                exit 7
            fi
        else
            echo "⚠️  Health checks failed but auto-rollback is disabled"
            echo "⚠️  Manual intervention may be required"
        fi
    fi
    
    # Calculate execution time
    local end_time
    end_time=$(date +%s)
    local execution_time=$((end_time - start_time))
    
    if [[ "$health_check_passed" == "true" ]]; then
        echo ""
        echo "🎉 Traffic switch completed successfully in ${execution_time}s!"
        echo "📋 Summary:"
        echo "   Environment: $ENVIRONMENT"
        echo "   Namespace: $NAMESPACE"
        echo "   From: $original_version"
        echo "   To: $TARGET_VERSION"
        echo "   Switch time: ${execution_time}s"
        echo "   Health checks: ✅ PASSED"
        
        show_traffic_status
        
        # Cleanup
        rm -f "$TMP_KUBECONFIG"
        exit 0
    else
        echo ""
        echo "⚠️  Traffic switch completed but health checks failed in ${execution_time}s"
        show_traffic_status
        
        # Cleanup
        rm -f "$TMP_KUBECONFIG"
        exit 8
    fi
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

# Validate jq is available for JSON processing (optional but recommended)
if ! command -v jq >/dev/null 2>&1; then
    echo "⚠️  jq not found - some output formatting will be limited"
fi

# Run main function
main