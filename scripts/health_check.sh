#!/bin/bash

set -euo pipefail

# Health check script for CodeGraph deployment
# Usage: health_check.sh <namespace> [timeout_seconds]

NAMESPACE=${1:-"codegraph-dev"}
TIMEOUT=${2:-60}
SERVICE_NAME="codegraph-api"
HEALTH_ENDPOINT="/health"
READY_ENDPOINT="/ready"
METRICS_ENDPOINT="/metrics"

echo "🏥 Starting health check for $SERVICE_NAME in namespace $NAMESPACE"
echo "⏱️  Timeout: ${TIMEOUT}s"

# Function to check if pods are ready
check_pod_readiness() {
    local ready_pods
    local total_pods
    
    ready_pods=$(kubectl -n "$NAMESPACE" get pods -l app="$SERVICE_NAME" -o jsonpath='{.items[?(@.status.containerStatuses[0].ready==true)].metadata.name}' | wc -w)
    total_pods=$(kubectl -n "$NAMESPACE" get pods -l app="$SERVICE_NAME" -o jsonpath='{.items[*].metadata.name}' | wc -w)
    
    echo "📊 Pod readiness: $ready_pods/$total_pods pods ready"
    
    if [[ $ready_pods -eq 0 ]]; then
        echo "❌ No pods are ready"
        kubectl -n "$NAMESPACE" get pods -l app="$SERVICE_NAME"
        return 1
    fi
    
    if [[ $ready_pods -lt $total_pods ]]; then
        echo "⚠️  Some pods are not ready yet"
        kubectl -n "$NAMESPACE" get pods -l app="$SERVICE_NAME"
    fi
    
    return 0
}

# Function to get service endpoint for testing
get_service_endpoint() {
    # Try to get LoadBalancer external IP first
    local external_ip
    external_ip=$(kubectl -n "$NAMESPACE" get svc "$SERVICE_NAME" -o jsonpath='{.status.loadBalancer.ingress[0].ip}' 2>/dev/null || echo "")
    
    if [[ -n "$external_ip" && "$external_ip" != "null" ]]; then
        echo "http://$external_ip"
        return 0
    fi
    
    # Try external hostname
    local external_host
    external_host=$(kubectl -n "$NAMESPACE" get svc "$SERVICE_NAME" -o jsonpath='{.status.loadBalancer.ingress[0].hostname}' 2>/dev/null || echo "")
    
    if [[ -n "$external_host" && "$external_host" != "null" ]]; then
        echo "http://$external_host"
        return 0
    fi
    
    # Use port-forward as fallback
    echo "port-forward"
    return 0
}

# Function to test HTTP endpoint
test_http_endpoint() {
    local base_url=$1
    local endpoint=$2
    local expected_status=${3:-200}
    local description=$4
    
    echo "🔍 Testing $description: $base_url$endpoint"
    
    local response_code
    if [[ "$base_url" == "port-forward" ]]; then
        # Use kubectl port-forward for internal testing
        local port_forward_pid
        kubectl -n "$NAMESPACE" port-forward svc/"$SERVICE_NAME" 8080:80 >/dev/null 2>&1 &
        port_forward_pid=$!
        sleep 2
        
        response_code=$(curl -s -o /dev/null -w "%{http_code}" "http://localhost:8080$endpoint" || echo "000")
        kill $port_forward_pid 2>/dev/null || true
        wait $port_forward_pid 2>/dev/null || true
    else
        response_code=$(curl -s -o /dev/null -w "%{http_code}" "$base_url$endpoint" || echo "000")
    fi
    
    if [[ "$response_code" == "$expected_status" ]]; then
        echo "✅ $description: HTTP $response_code (expected $expected_status)"
        return 0
    else
        echo "❌ $description: HTTP $response_code (expected $expected_status)"
        return 1
    fi
}

# Function to test endpoint with retry logic
test_endpoint_with_retry() {
    local base_url=$1
    local endpoint=$2
    local expected_status=${3:-200}
    local description=$4
    local max_attempts=5
    local attempt=1
    
    while [[ $attempt -le $max_attempts ]]; do
        if test_http_endpoint "$base_url" "$endpoint" "$expected_status" "$description"; then
            return 0
        fi
        
        if [[ $attempt -lt $max_attempts ]]; then
            echo "⏳ Retry $attempt/$max_attempts for $description in 3 seconds..."
            sleep 3
        fi
        
        ((attempt++))
    done
    
    echo "❌ $description failed after $max_attempts attempts"
    return 1
}

# Function to check deployment status
check_deployment_status() {
    echo "📋 Checking deployment status..."
    
    # Check if deployment exists
    if ! kubectl -n "$NAMESPACE" get deployment "$SERVICE_NAME" >/dev/null 2>&1; then
        echo "❌ Deployment $SERVICE_NAME not found in namespace $NAMESPACE"
        return 1
    fi
    
    # Check deployment conditions
    local available
    available=$(kubectl -n "$NAMESPACE" get deployment "$SERVICE_NAME" -o jsonpath='{.status.conditions[?(@.type=="Available")].status}' 2>/dev/null || echo "False")
    
    if [[ "$available" != "True" ]]; then
        echo "❌ Deployment is not available"
        kubectl -n "$NAMESPACE" describe deployment "$SERVICE_NAME"
        return 1
    fi
    
    # Check replicas
    local ready_replicas
    local desired_replicas
    ready_replicas=$(kubectl -n "$NAMESPACE" get deployment "$SERVICE_NAME" -o jsonpath='{.status.readyReplicas}' 2>/dev/null || echo "0")
    desired_replicas=$(kubectl -n "$NAMESPACE" get deployment "$SERVICE_NAME" -o jsonpath='{.spec.replicas}' 2>/dev/null || echo "1")
    
    if [[ "$ready_replicas" -ne "$desired_replicas" ]]; then
        echo "❌ Not all replicas are ready: $ready_replicas/$desired_replicas"
        return 1
    fi
    
    echo "✅ Deployment status: $ready_replicas/$desired_replicas replicas ready"
    return 0
}

# Function to check service status
check_service_status() {
    echo "🌐 Checking service status..."
    
    if ! kubectl -n "$NAMESPACE" get svc "$SERVICE_NAME" >/dev/null 2>&1; then
        echo "❌ Service $SERVICE_NAME not found in namespace $NAMESPACE"
        return 1
    fi
    
    # Get service details
    local service_type
    local cluster_ip
    local external_ip
    
    service_type=$(kubectl -n "$NAMESPACE" get svc "$SERVICE_NAME" -o jsonpath='{.spec.type}')
    cluster_ip=$(kubectl -n "$NAMESPACE" get svc "$SERVICE_NAME" -o jsonpath='{.spec.clusterIP}')
    external_ip=$(kubectl -n "$NAMESPACE" get svc "$SERVICE_NAME" -o jsonpath='{.status.loadBalancer.ingress[0].ip}' 2>/dev/null || echo "none")
    
    echo "✅ Service type: $service_type"
    echo "✅ Cluster IP: $cluster_ip"
    if [[ "$external_ip" != "none" && "$external_ip" != "null" ]]; then
        echo "✅ External IP: $external_ip"
    fi
    
    return 0
}

# Function to check resource usage
check_resource_usage() {
    echo "📊 Checking resource usage..."
    
    # Get pod resource usage if metrics-server is available
    if kubectl top nodes >/dev/null 2>&1; then
        echo "📈 Pod resource usage:"
        kubectl -n "$NAMESPACE" top pods -l app="$SERVICE_NAME" 2>/dev/null || echo "⚠️  Pod metrics not available"
    else
        echo "⚠️  Metrics server not available, skipping resource usage check"
    fi
    
    # Check resource requests and limits
    echo "📋 Resource configuration:"
    kubectl -n "$NAMESPACE" get pods -l app="$SERVICE_NAME" -o jsonpath='{range .items[*]}{.metadata.name}{"\n"}{range .spec.containers[*]}  CPU Request: {.resources.requests.cpu}{"\n"}  CPU Limit: {.resources.limits.cpu}{"\n"}  Memory Request: {.resources.requests.memory}{"\n"}  Memory Limit: {.resources.limits.memory}{"\n"}{end}{"\n"}{end}' 2>/dev/null || echo "⚠️  Resource information not available"
    
    return 0
}

# Function to check logs for errors
check_logs() {
    echo "📝 Checking recent logs for errors..."
    
    # Get logs from all pods and check for errors
    local error_count=0
    local warning_count=0
    
    for pod in $(kubectl -n "$NAMESPACE" get pods -l app="$SERVICE_NAME" -o jsonpath='{.items[*].metadata.name}'); do
        echo "📋 Checking logs for pod: $pod"
        
        # Check for ERROR level logs in last 100 lines
        local errors
        errors=$(kubectl -n "$NAMESPACE" logs "$pod" --tail=100 2>/dev/null | grep -i "error" | wc -l || echo "0")
        error_count=$((error_count + errors))
        
        # Check for WARN level logs in last 100 lines
        local warnings
        warnings=$(kubectl -n "$NAMESPACE" logs "$pod" --tail=100 2>/dev/null | grep -i "warn" | wc -l || echo "0")
        warning_count=$((warning_count + warnings))
        
        # Show recent critical errors
        echo "📄 Recent errors in $pod:"
        kubectl -n "$NAMESPACE" logs "$pod" --tail=50 2>/dev/null | grep -i "error" | tail -3 || echo "   No recent errors found"
    done
    
    if [[ $error_count -gt 10 ]]; then
        echo "⚠️  High error count detected: $error_count errors in recent logs"
    elif [[ $error_count -gt 0 ]]; then
        echo "ℹ️  Found $error_count errors in recent logs"
    else
        echo "✅ No errors found in recent logs"
    fi
    
    if [[ $warning_count -gt 0 ]]; then
        echo "ℹ️  Found $warning_count warnings in recent logs"
    fi
    
    return 0
}

# Main health check execution
main() {
    local start_time
    start_time=$(date +%s)
    local health_check_passed=true
    
    echo "🏁 Starting comprehensive health check..."
    echo ""
    
    # Step 1: Check deployment status
    if ! check_deployment_status; then
        health_check_passed=false
    fi
    echo ""
    
    # Step 2: Check service status
    if ! check_service_status; then
        health_check_passed=false
    fi
    echo ""
    
    # Step 3: Check pod readiness
    if ! check_pod_readiness; then
        health_check_passed=false
    fi
    echo ""
    
    # Step 4: Get service endpoint
    local service_endpoint
    service_endpoint=$(get_service_endpoint)
    echo "🔗 Service endpoint method: $service_endpoint"
    echo ""
    
    # Step 5: Test application endpoints
    echo "🌐 Testing application endpoints..."
    local endpoint_tests_passed=true
    
    # Test health endpoint
    if ! test_endpoint_with_retry "$service_endpoint" "$HEALTH_ENDPOINT" "200" "Health endpoint"; then
        endpoint_tests_passed=false
    fi
    
    # Test readiness endpoint
    if ! test_endpoint_with_retry "$service_endpoint" "$READY_ENDPOINT" "200" "Readiness endpoint"; then
        endpoint_tests_passed=false
    fi
    
    # Test metrics endpoint (optional, don't fail if not available)
    if ! test_endpoint_with_retry "$service_endpoint" "$METRICS_ENDPOINT" "200" "Metrics endpoint"; then
        echo "ℹ️  Metrics endpoint test failed (this is optional)"
    fi
    
    if [[ "$endpoint_tests_passed" == "false" ]]; then
        health_check_passed=false
    fi
    echo ""
    
    # Step 6: Check resource usage
    check_resource_usage
    echo ""
    
    # Step 7: Check logs
    check_logs
    echo ""
    
    # Calculate execution time
    local end_time
    end_time=$(date +%s)
    local execution_time=$((end_time - start_time))
    
    # Final result
    if [[ "$health_check_passed" == "true" ]]; then
        echo "🎉 Health check PASSED in ${execution_time}s"
        echo "✅ All critical checks completed successfully"
        exit 0
    else
        echo "💥 Health check FAILED in ${execution_time}s"
        echo "❌ One or more critical checks failed"
        
        # Show current pod status for debugging
        echo ""
        echo "🔍 Current pod status for debugging:"
        kubectl -n "$NAMESPACE" get pods -l app="$SERVICE_NAME" -o wide
        
        echo ""
        echo "📋 Recent events:"
        kubectl -n "$NAMESPACE" get events --sort-by='.lastTimestamp' | tail -10
        
        exit 1
    fi
}

# Validate kubectl is available
if ! command -v kubectl >/dev/null 2>&1; then
    echo "❌ kubectl is not installed or not in PATH"
    exit 1
fi

# Run main function with timeout
if ! timeout "$TIMEOUT" bash -c 'main "$@"' _ "$@"; then
    echo "⏰ Health check timed out after ${TIMEOUT}s"
    exit 1
fi