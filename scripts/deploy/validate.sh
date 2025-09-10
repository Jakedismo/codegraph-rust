#!/usr/bin/env bash
# =============================================================================
# Production Deployment Validation Script
# Comprehensive validation framework for zero-downtime deployments
# =============================================================================
set -euo pipefail

# Configuration
BASE_URL="${BASE_URL:-http://localhost:3000}"
TIMEOUT_SECS="${TIMEOUT_SECS:-60}"
API_KEY="${API_KEY:-test-api-key}"
VERBOSE="${VERBOSE:-false}"
CONCURRENT_REQUESTS="${CONCURRENT_REQUESTS:-10}"
MAX_RESPONSE_TIME_MS="${MAX_RESPONSE_TIME_MS:-500}"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Logging functions
log_info() {
    echo -e "${BLUE}[INFO]${NC} $*"
}

log_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $*"
}

log_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $*"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $*"
}

# Enhanced health check with retry logic
validate_health_endpoint() {
    log_info "🏥 Validating health endpoint with authentication"
    
    local deadline=$(( $(date +%s) + TIMEOUT_SECS ))
    local attempt=1
    
    until curl -sSf -H "X-API-KEY: ${API_KEY}" "${BASE_URL}/health" >/dev/null; do
        if (( $(date +%s) > deadline )); then
            log_error "Health check did not become ready in ${TIMEOUT_SECS}s after ${attempt} attempts"
            return 1
        fi
        log_info "Health check attempt $attempt (waiting...)"
        ((attempt++))
        sleep 2
    done
    
    log_success "Health endpoint is responding (attempt $attempt)"
    
    # Validate health response structure
    local health_response
    health_response=$(curl -sS -H "X-API-KEY: ${API_KEY}" "${BASE_URL}/health")
    
    if echo "$health_response" | jq -e '.status' >/dev/null 2>&1; then
        local status
        status=$(echo "$health_response" | jq -r '.status')
        log_success "Health status: $status"
    else
        log_warning "Health response does not contain expected JSON structure"
    fi
    
    return 0
}

# Test HTTP/2 optimization features
validate_http2_endpoints() {
    log_info "🚀 Validating HTTP/2 optimization endpoints"
    
    if curl -sS -H "X-API-KEY: ${API_KEY}" "${BASE_URL}/http2/health" >/dev/null; then
        log_success "HTTP/2 health endpoint responding"
    else
        log_error "HTTP/2 health endpoint not responding"
        return 1
    fi
    
    if curl -sS -H "X-API-KEY: ${API_KEY}" "${BASE_URL}/http2/config" >/dev/null; then
        log_success "HTTP/2 configuration endpoint responding"
    else
        log_warning "HTTP/2 configuration endpoint not responding"
    fi
    
    return 0
}

# Test GraphQL endpoint
validate_graphql_endpoint() {
    log_info "📊 Validating GraphQL endpoint"
    
    local gql='{"query":"query { version }"}'
    local response
    
    if response=$(curl -sS -H "Content-Type: application/json" -H "X-API-KEY: ${API_KEY}" -d "${gql}" "${BASE_URL}/graphql" 2>/dev/null); then
        if echo "$response" | jq -e '.data.version' >/dev/null 2>&1; then
            local version
            version=$(echo "$response" | jq -r '.data.version')
            log_success "GraphQL endpoint responding with version: $version"
        else
            log_success "GraphQL endpoint responding"
        fi
    else
        log_error "GraphQL endpoint not responding"
        return 1
    fi
    
    return 0
}

# Validate Prometheus metrics
validate_metrics_endpoint() {
    log_info "📈 Validating metrics endpoint"
    
    local metrics_response
    if metrics_response=$(curl -sS "${BASE_URL}/metrics" 2>/dev/null); then
        # Check for key process metrics
        if echo "$metrics_response" | grep -q "process_cpu_seconds_total"; then
            log_success "Process CPU metrics available"
        else
            log_warning "Process CPU metrics missing"
        fi
        
        if echo "$metrics_response" | grep -q "process_resident_memory_bytes"; then
            log_success "Process memory metrics available"
            
            # Extract memory usage
            local memory_bytes
            memory_bytes=$(echo "$metrics_response" | grep "process_resident_memory_bytes" | grep -v "#" | awk '{print $2}' | head -1)
            if [[ -n "$memory_bytes" ]]; then
                local memory_mb=$((memory_bytes / 1024 / 1024))
                log_info "Current memory usage: ${memory_mb}MB"
            fi
        else
            log_warning "Process memory metrics missing"
        fi
        
        # Check for application-specific metrics
        if echo "$metrics_response" | grep -q "http_requests_total"; then
            log_success "HTTP request metrics available"
        else
            log_info "HTTP request metrics not found (may not be implemented yet)"
        fi
        
    else
        log_error "Metrics endpoint not responding"
        return 1
    fi
    
    return 0
}

# Performance validation with response time checks
validate_performance() {
    log_info "⚡ Validating response time performance"
    
    local endpoint="/health"
    local sample_count=5
    local total_time=0
    local max_time=0
    local min_time=999999
    
    for i in $(seq 1 $sample_count); do
        local start_time
        start_time=$(date +%s.%N)
        
        if curl -sSf -H "X-API-KEY: ${API_KEY}" "${BASE_URL}${endpoint}" >/dev/null 2>&1; then
            local end_time
            end_time=$(date +%s.%N)
            local response_time
            response_time=$(echo "$end_time - $start_time" | bc -l)
            local response_time_ms
            response_time_ms=$(echo "$response_time * 1000" | bc -l | cut -d. -f1)
            
            total_time=$(echo "$total_time + $response_time" | bc -l)
            
            if (( response_time_ms > max_time )); then
                max_time=$response_time_ms
            fi
            
            if (( response_time_ms < min_time )); then
                min_time=$response_time_ms
            fi
            
            if [[ "$VERBOSE" == "true" ]]; then
                log_info "Sample $i: ${response_time_ms}ms"
            fi
        else
            log_error "Performance test request $i failed"
            return 1
        fi
    done
    
    local avg_time
    avg_time=$(echo "scale=0; ($total_time * 1000) / $sample_count" | bc -l)
    
    log_info "Response times - Min: ${min_time}ms, Avg: ${avg_time}ms, Max: ${max_time}ms"
    
    if (( avg_time <= MAX_RESPONSE_TIME_MS )); then
        log_success "Average response time within threshold (${avg_time}ms <= ${MAX_RESPONSE_TIME_MS}ms)"
    else
        log_error "Average response time exceeds threshold (${avg_time}ms > ${MAX_RESPONSE_TIME_MS}ms)"
        return 1
    fi
    
    return 0
}

# Concurrent request handling validation
validate_concurrent_requests() {
    log_info "🚀 Validating concurrent request handling ($CONCURRENT_REQUESTS requests)"
    
    local temp_dir
    temp_dir=$(mktemp -d)
    local pids=()
    
    # Launch concurrent requests
    for i in $(seq 1 $CONCURRENT_REQUESTS); do
        (
            if curl -sSf -H "X-API-KEY: ${API_KEY}" --max-time 10 "${BASE_URL}/health" >/dev/null 2>&1; then
                echo "success" > "$temp_dir/result_$i"
            else
                echo "failed" > "$temp_dir/result_$i"
            fi
        ) &
        pids+=($!)
    done
    
    # Wait for all requests with timeout
    local wait_timeout=30
    local start_wait
    start_wait=$(date +%s)
    
    for pid in "${pids[@]}"; do
        if (( $(date +%s) - start_wait > wait_timeout )); then
            log_error "Concurrent request test timed out"
            kill "${pids[@]}" 2>/dev/null || true
            rm -rf "$temp_dir"
            return 1
        fi
        wait "$pid" || true
    done
    
    # Count successful requests
    local success_count=0
    for i in $(seq 1 $CONCURRENT_REQUESTS); do
        if [[ -f "$temp_dir/result_$i" && "$(cat "$temp_dir/result_$i")" == "success" ]]; then
            ((success_count++))
        fi
    done
    
    # Cleanup
    rm -rf "$temp_dir"
    
    if (( success_count == CONCURRENT_REQUESTS )); then
        log_success "All $CONCURRENT_REQUESTS concurrent requests succeeded"
    elif (( success_count >= CONCURRENT_REQUESTS * 8 / 10 )); then
        log_warning "$success_count/$CONCURRENT_REQUESTS concurrent requests succeeded (80%+ threshold met)"
    else
        log_error "Only $success_count/$CONCURRENT_REQUESTS concurrent requests succeeded"
        return 1
    fi
    
    return 0
}

# Security header validation
validate_security_headers() {
    log_info "🔒 Validating security headers"
    
    local headers
    headers=$(curl -sI -H "X-API-KEY: ${API_KEY}" --max-time 10 "${BASE_URL}/health" 2>/dev/null)
    
    local security_headers=(
        "X-Frame-Options"
        "X-Content-Type-Options"
        "X-XSS-Protection"
    )
    
    local found_headers=0
    for header in "${security_headers[@]}"; do
        if echo "$headers" | grep -qi "$header"; then
            log_success "$header header present"
            ((found_headers++))
        else
            log_warning "$header header missing"
        fi
    done
    
    if (( found_headers >= 2 )); then
        log_success "Sufficient security headers present ($found_headers/3)"
    else
        log_warning "Insufficient security headers ($found_headers/3)"
    fi
    
    return 0
}

# Main validation orchestration
main() {
    log_info "🎯 Starting comprehensive deployment validation"
    log_info "Target URL: $BASE_URL"
    log_info "Timeout: ${TIMEOUT_SECS}s, Concurrent requests: $CONCURRENT_REQUESTS"
    log_info "Max response time: ${MAX_RESPONSE_TIME_MS}ms"
    
    local start_time
    start_time=$(date +%s)
    
    local validations=(
        "validate_health_endpoint"
        "validate_http2_endpoints"
        "validate_graphql_endpoint"
        "validate_metrics_endpoint"
        "validate_performance"
        "validate_concurrent_requests"
        "validate_security_headers"
    )
    
    local passed=0
    local failed=0
    
    for validation in "${validations[@]}"; do
        log_info "Running: $validation"
        if $validation; then
            ((passed++))
        else
            ((failed++))
        fi
        echo ""
    done
    
    local end_time
    end_time=$(date +%s)
    local duration=$((end_time - start_time))
    
    log_info "🏁 Validation completed in ${duration}s"
    log_info "Results: $passed passed, $failed failed"
    
    if (( failed == 0 )); then
        log_success "🎉 All validations passed! Deployment is ready for production traffic."
        return 0
    else
        log_error "❌ $failed validations failed. Review issues before proceeding."
        return 1
    fi
}

# Check dependencies
check_dependencies() {
    local deps=("curl" "jq" "bc")
    for dep in "${deps[@]}"; do
        if ! command -v "$dep" >/dev/null 2>&1; then
            log_error "Required dependency not found: $dep"
            log_error "Please install: $dep"
            exit 1
        fi
    done
}

# Parse command line arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        --base-url)
            BASE_URL="$2"
            shift 2
            ;;
        --timeout)
            TIMEOUT_SECS="$2"
            shift 2
            ;;
        --verbose)
            VERBOSE="true"
            shift
            ;;
        --concurrent)
            CONCURRENT_REQUESTS="$2"
            shift 2
            ;;
        --max-response-time)
            MAX_RESPONSE_TIME_MS="$2"
            shift 2
            ;;
        --help)
            echo "Usage: $0 [OPTIONS]"
            echo "Options:"
            echo "  --base-url URL            Base URL to validate (default: http://localhost:3000)"
            echo "  --timeout SECS           Request timeout in seconds (default: 60)"
            echo "  --concurrent NUM         Number of concurrent requests (default: 10)"
            echo "  --max-response-time MS   Max acceptable response time (default: 500)"
            echo "  --verbose                Enable verbose output"
            echo "  --help                   Show this help message"
            echo ""
            echo "Environment variables:"
            echo "  BASE_URL                 Base URL to validate"
            echo "  API_KEY                  API key for authentication"
            echo "  TIMEOUT_SECS            Request timeout"
            echo "  VERBOSE                 Enable verbose output (true/false)"
            exit 0
            ;;
        *)
            log_error "Unknown option: $1"
            exit 1
            ;;
    esac
done

# Run validation
check_dependencies
main

