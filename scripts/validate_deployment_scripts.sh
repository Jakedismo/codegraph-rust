#!/bin/bash

set -euo pipefail

# Validation script for deployment scripts
# Usage: validate_deployment_scripts.sh

SCRIPT_DIR="$(dirname "$0")"
VALIDATION_FAILED=0

echo "🔍 Validating CodeGraph deployment scripts"
echo "📁 Script directory: $SCRIPT_DIR"
echo ""

# Function to validate script exists and is executable
validate_script() {
    local script_name=$1
    local script_path="${SCRIPT_DIR}/${script_name}"
    
    echo "🔎 Validating $script_name..."
    
    # Check if file exists
    if [[ ! -f "$script_path" ]]; then
        echo "❌ Script not found: $script_path"
        VALIDATION_FAILED=1
        return 1
    fi
    
    # Check if file is executable
    if [[ ! -x "$script_path" ]]; then
        echo "⚠️  Script is not executable: $script_path"
        echo "   Run: chmod +x $script_path"
        VALIDATION_FAILED=1
        return 1
    fi
    
    # Basic syntax validation
    if ! bash -n "$script_path"; then
        echo "❌ Syntax error in script: $script_path"
        VALIDATION_FAILED=1
        return 1
    fi
    
    echo "✅ $script_name is valid and executable"
    return 0
}

# Function to validate script usage/help output
validate_script_usage() {
    local script_name=$1
    local script_path="${SCRIPT_DIR}/${script_name}"
    
    echo "📋 Testing $script_name usage information..."
    
    # Test that script shows usage when run with no arguments
    if timeout 10s bash "$script_path" >/dev/null 2>&1; then
        echo "⚠️  Script $script_name does not show usage when run without arguments"
        # This is a warning, not a failure
    else
        echo "✅ $script_name properly handles missing arguments"
    fi
    
    return 0
}

# Function to validate required dependencies
validate_dependencies() {
    echo "🔧 Validating required dependencies..."
    
    local deps_ok=true
    
    # Check for kubectl
    if ! command -v kubectl >/dev/null 2>&1; then
        echo "❌ kubectl is not installed or not in PATH"
        deps_ok=false
    else
        echo "✅ kubectl is available"
    fi
    
    # Check for curl
    if ! command -v curl >/dev/null 2>&1; then
        echo "❌ curl is not installed or not in PATH"
        deps_ok=false
    else
        echo "✅ curl is available"
    fi
    
    # Check for jq (optional but recommended)
    if ! command -v jq >/dev/null 2>&1; then
        echo "⚠️  jq is not installed - some features will be limited"
    else
        echo "✅ jq is available"
    fi
    
    if [[ "$deps_ok" == "false" ]]; then
        echo "❌ Missing required dependencies"
        VALIDATION_FAILED=1
        return 1
    fi
    
    echo "✅ All required dependencies are available"
    return 0
}

# Function to validate Kubernetes manifests
validate_k8s_manifests() {
    echo "📋 Validating Kubernetes manifests..."
    
    local manifests_dir="${SCRIPT_DIR}/../deploy/k8s"
    
    if [[ ! -d "$manifests_dir" ]]; then
        echo "❌ Kubernetes manifests directory not found: $manifests_dir"
        VALIDATION_FAILED=1
        return 1
    fi
    
    # Check for required manifest files
    local required_manifests=("deployment.yaml" "service.yaml")
    
    for manifest in "${required_manifests[@]}"; do
        local manifest_path="${manifests_dir}/${manifest}"
        
        if [[ ! -f "$manifest_path" ]]; then
            echo "❌ Required manifest not found: $manifest_path"
            VALIDATION_FAILED=1
            continue
        fi
        
        # Validate basic YAML structure and required fields
        local has_api_version=$(grep -c "^apiVersion:" "$manifest_path" || echo "0")
        local has_kind=$(grep -c "^kind:" "$manifest_path" || echo "0")
        local has_metadata=$(grep -c "^metadata:" "$manifest_path" || echo "0")
        local has_spec=$(grep -c "^spec:" "$manifest_path" || echo "0")
        
        if [[ $has_api_version -eq 1 && $has_kind -eq 1 && $has_metadata -eq 1 && $has_spec -eq 1 ]]; then
            echo "✅ Valid manifest: $manifest"
        else
            echo "❌ Manifest missing required fields: $manifest"
            echo "   apiVersion: $has_api_version, kind: $has_kind, metadata: $has_metadata, spec: $has_spec"
            VALIDATION_FAILED=1
        fi
    done
    
    return 0
}

# Function to validate script integrations
validate_script_integrations() {
    echo "🔗 Validating script integrations..."
    
    # Check that deploy_k8s.sh references health_check.sh
    if grep -q "health_check.sh" "${SCRIPT_DIR}/deploy_k8s.sh"; then
        echo "✅ deploy_k8s.sh integrates with health_check.sh"
    else
        echo "⚠️  deploy_k8s.sh does not reference health_check.sh"
    fi
    
    # Check that blue_green_deploy.sh references health_check.sh
    if grep -q "health_check.sh" "${SCRIPT_DIR}/blue_green_deploy.sh"; then
        echo "✅ blue_green_deploy.sh integrates with health_check.sh"
    else
        echo "⚠️  blue_green_deploy.sh does not reference health_check.sh"
    fi
    
    # Check that switch_traffic.sh references health_check.sh
    if grep -q "health_check.sh" "${SCRIPT_DIR}/switch_traffic.sh"; then
        echo "✅ switch_traffic.sh integrates with health_check.sh"
    else
        echo "⚠️  switch_traffic.sh does not reference health_check.sh"
    fi
    
    return 0
}

# Function to validate environment variable handling
validate_environment_variables() {
    echo "🌍 Validating environment variable handling..."
    
    local env_vars_ok=true
    
    # Check that scripts handle KUBECONFIG_CONTENT
    for script in "deploy_k8s.sh" "blue_green_deploy.sh" "switch_traffic.sh"; do
        if grep -q "KUBECONFIG_CONTENT" "${SCRIPT_DIR}/${script}"; then
            echo "✅ $script handles KUBECONFIG_CONTENT"
        else
            echo "❌ $script does not handle KUBECONFIG_CONTENT"
            env_vars_ok=false
        fi
    done
    
    # Check timeout variable handling
    for script in "deploy_k8s.sh" "blue_green_deploy.sh" "health_check.sh"; do
        if grep -q "TIMEOUT" "${SCRIPT_DIR}/${script}"; then
            echo "✅ $script supports timeout configuration"
        else
            echo "⚠️  $script does not support timeout configuration"
        fi
    done
    
    if [[ "$env_vars_ok" == "false" ]]; then
        VALIDATION_FAILED=1
        return 1
    fi
    
    return 0
}

# Function to validate error handling
validate_error_handling() {
    echo "⚠️  Validating error handling patterns..."
    
    # Check for proper error handling (set -euo pipefail)
    for script in "deploy_k8s.sh" "blue_green_deploy.sh" "health_check.sh" "switch_traffic.sh"; do
        if grep -q "set -euo pipefail" "${SCRIPT_DIR}/${script}"; then
            echo "✅ $script uses proper error handling"
        else
            echo "⚠️  $script may not use proper error handling"
        fi
    done
    
    return 0
}

# Function to generate validation report
generate_report() {
    echo ""
    echo "📊 Validation Report Summary"
    echo "=========================="
    
    # Count scripts
    local script_count=0
    for script in "deploy_k8s.sh" "blue_green_deploy.sh" "health_check.sh" "switch_traffic.sh"; do
        if [[ -f "${SCRIPT_DIR}/${script}" ]]; then
            ((script_count++))
        fi
    done
    
    echo "📋 Scripts found: $script_count/4"
    echo "🔧 Dependencies: $(command -v kubectl >/dev/null && echo "kubectl ✅" || echo "kubectl ❌") $(command -v curl >/dev/null && echo "curl ✅" || echo "curl ❌")"
    echo "📁 Manifests: $(find "${SCRIPT_DIR}/../deploy/k8s" -name "*.yaml" 2>/dev/null | wc -l || echo "0") files"
    
    if [[ $VALIDATION_FAILED -eq 0 ]]; then
        echo ""
        echo "🎉 All validations passed!"
        echo "✅ Deployment scripts are ready for use"
        return 0
    else
        echo ""
        echo "❌ Some validations failed"
        echo "⚠️  Please fix the issues above before using the deployment scripts"
        return 1
    fi
}

# Main validation execution
main() {
    echo "🏁 Starting deployment script validation..."
    echo ""
    
    # Validate each script
    local scripts=("deploy_k8s.sh" "blue_green_deploy.sh" "health_check.sh" "switch_traffic.sh")
    
    for script in "${scripts[@]}"; do
        validate_script "$script"
        validate_script_usage "$script"
        echo ""
    done
    
    # Validate dependencies
    validate_dependencies
    echo ""
    
    # Validate Kubernetes manifests
    validate_k8s_manifests
    echo ""
    
    # Validate script integrations
    validate_script_integrations
    echo ""
    
    # Validate environment variables
    validate_environment_variables
    echo ""
    
    # Validate error handling
    validate_error_handling
    echo ""
    
    # Generate final report
    generate_report
}

# Run main function
main