#!/bin/bash
# ==============================================================================
# CodeGraph Docker Build Script
# Automated building of optimized Docker images
# ==============================================================================

set -euo pipefail

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
REGISTRY="${DOCKER_REGISTRY:-codegraph}"
VERSION="${VERSION:-latest}"
BUILD_DATE=$(date -u +'%Y-%m-%dT%H:%M:%SZ')
GIT_COMMIT=$(git rev-parse --short HEAD 2>/dev/null || echo "unknown")

# Build targets
TARGETS=("minimal" "optimized" "development")

# Functions
log() {
    echo -e "${BLUE}[$(date +'%Y-%m-%d %H:%M:%S')]${NC} $1"
}

success() {
    echo -e "${GREEN}[$(date +'%Y-%m-%d %H:%M:%S')] ✅${NC} $1"
}

warn() {
    echo -e "${YELLOW}[$(date +'%Y-%m-%d %H:%M:%S')] ⚠️${NC} $1"
}

error() {
    echo -e "${RED}[$(date +'%Y-%m-%d %H:%M:%S')] ❌${NC} $1"
    exit 1
}

# Parse command line arguments
show_help() {
    cat << EOF
CodeGraph Docker Build Script

Usage: $0 [OPTIONS] [TARGET]

OPTIONS:
    -h, --help              Show this help message
    -r, --registry REGISTRY Set Docker registry (default: codegraph)
    -v, --version VERSION   Set image version (default: latest)
    -p, --push              Push images to registry after build
    -c, --cache             Use build cache
    --no-cache              Disable build cache
    --scan                  Run security scan after build
    --sbom                  Generate SBOM (Software Bill of Materials)

TARGETS:
    minimal                 Ultra-minimal image using scratch (~5MB)
    optimized               Security-hardened distroless image (~50MB)
    development             Development image with debugging tools (~500MB)
    all                     Build all targets (default)

EXAMPLES:
    $0                      Build all targets
    $0 minimal              Build only minimal image
    $0 -p optimized         Build and push optimized image
    $0 --scan --sbom all    Build all with security scan and SBOM

EOF
}

# Build function
build_image() {
    local target=$1
    local dockerfile="Dockerfile.${target}"
    local image_name="${REGISTRY}/codegraph-api:${target}-${VERSION}"
    local cache_flag=""
    local buildx_args=""
    
    # Set Dockerfile path
    case $target in
        "minimal")
            dockerfile="Dockerfile.minimal"
            ;;
        "optimized")
            dockerfile="Dockerfile.optimized"
            ;;
        "development")
            dockerfile="Dockerfile.optimized"
            buildx_args="--target development"
            ;;
    esac
    
    # Check if Dockerfile exists
    if [[ ! -f "$dockerfile" ]]; then
        error "Dockerfile $dockerfile not found!"
    fi
    
    # Set cache options
    if [[ "$USE_CACHE" == "true" ]]; then
        cache_flag="--cache-from type=registry,ref=${REGISTRY}/codegraph-api:cache-${target}"
        cache_flag+=" --cache-to type=registry,ref=${REGISTRY}/codegraph-api:cache-${target},mode=max"
    elif [[ "$USE_CACHE" == "false" ]]; then
        cache_flag="--no-cache"
    fi
    
    log "Building $target image: $image_name"
    log "Using Dockerfile: $dockerfile"
    
    # Build command
    local build_cmd="docker buildx build"
    build_cmd+=" --platform linux/amd64,linux/arm64"
    build_cmd+=" --file $dockerfile"
    build_cmd+=" --tag $image_name"
    build_cmd+=" --tag ${REGISTRY}/codegraph-api:${target}-latest"
    build_cmd+=" --label org.opencontainers.image.created=$BUILD_DATE"
    build_cmd+=" --label org.opencontainers.image.version=$VERSION"
    build_cmd+=" --label org.opencontainers.image.revision=$GIT_COMMIT"
    build_cmd+=" --label org.opencontainers.image.title=CodeGraph-${target}"
    build_cmd+=" --label org.opencontainers.image.description=CodeGraph API - ${target} build"
    build_cmd+=" $cache_flag"
    build_cmd+=" $buildx_args"
    
    # Add SBOM generation if requested
    if [[ "$GENERATE_SBOM" == "true" ]]; then
        build_cmd+=" --sbom=true"
    fi
    
    # Add attestations
    build_cmd+=" --provenance=true"
    
    # Add load or push flag
    if [[ "$PUSH_IMAGES" == "true" ]]; then
        build_cmd+=" --push"
    else
        build_cmd+=" --load"
    fi
    
    build_cmd+=" ."
    
    # Execute build
    log "Executing: $build_cmd"
    if eval "$build_cmd"; then
        success "Successfully built $target image"
        
        # Show image size if built locally
        if [[ "$PUSH_IMAGES" != "true" ]]; then
            local size=$(docker images --format "table {{.Size}}" "${REGISTRY}/codegraph-api:${target}-latest" | tail -n 1)
            log "Image size: $size"
        fi
    else
        error "Failed to build $target image"
    fi
}

# Security scan function
run_security_scan() {
    local target=$1
    local image_name="${REGISTRY}/codegraph-api:${target}-${VERSION}"
    
    log "Running security scan on $image_name"
    
    # Run Trivy scan
    if command -v trivy &> /dev/null; then
        log "Running Trivy security scan..."
        trivy image --severity HIGH,CRITICAL "$image_name" || warn "Trivy scan found vulnerabilities"
    fi
    
    # Run Docker Scout scan
    if command -v docker &> /dev/null && docker scout --help &> /dev/null; then
        log "Running Docker Scout scan..."
        docker scout cves "$image_name" || warn "Docker Scout found vulnerabilities"
    fi
    
    # Run Grype scan
    if command -v grype &> /dev/null; then
        log "Running Grype security scan..."
        grype "$image_name" || warn "Grype scan found vulnerabilities"
    fi
}

# Main script
main() {
    # Default values
    USE_CACHE=""
    PUSH_IMAGES="false"
    RUN_SCAN="false"
    GENERATE_SBOM="false"
    BUILD_TARGET="all"
    
    # Parse arguments
    while [[ $# -gt 0 ]]; do
        case $1 in
            -h|--help)
                show_help
                exit 0
                ;;
            -r|--registry)
                REGISTRY="$2"
                shift 2
                ;;
            -v|--version)
                VERSION="$2"
                shift 2
                ;;
            -p|--push)
                PUSH_IMAGES="true"
                shift
                ;;
            -c|--cache)
                USE_CACHE="true"
                shift
                ;;
            --no-cache)
                USE_CACHE="false"
                shift
                ;;
            --scan)
                RUN_SCAN="true"
                shift
                ;;
            --sbom)
                GENERATE_SBOM="true"
                shift
                ;;
            minimal|optimized|development|all)
                BUILD_TARGET="$1"
                shift
                ;;
            *)
                error "Unknown option: $1"
                ;;
        esac
    done
    
    # Validate Docker Buildx
    if ! command -v docker &> /dev/null; then
        error "Docker is not installed or not in PATH"
    fi
    
    if ! docker buildx version &> /dev/null; then
        error "Docker Buildx is not available"
    fi
    
    # Create buildx builder if it doesn't exist
    if ! docker buildx inspect codegraph-builder &> /dev/null; then
        log "Creating Docker Buildx builder..."
        docker buildx create --name codegraph-builder --use
    fi
    
    # Set default cache behavior
    if [[ -z "$USE_CACHE" ]]; then
        USE_CACHE="true"
    fi
    
    log "Starting CodeGraph Docker build process"
    log "Registry: $REGISTRY"
    log "Version: $VERSION"
    log "Build Date: $BUILD_DATE"
    log "Git Commit: $GIT_COMMIT"
    log "Target: $BUILD_TARGET"
    log "Push Images: $PUSH_IMAGES"
    log "Use Cache: $USE_CACHE"
    log "Run Security Scan: $RUN_SCAN"
    log "Generate SBOM: $GENERATE_SBOM"
    
    # Build images
    if [[ "$BUILD_TARGET" == "all" ]]; then
        for target in "${TARGETS[@]}"; do
            build_image "$target"
            
            if [[ "$RUN_SCAN" == "true" ]]; then
                run_security_scan "$target"
            fi
        done
    else
        if [[ " ${TARGETS[@]} " =~ " ${BUILD_TARGET} " ]]; then
            build_image "$BUILD_TARGET"
            
            if [[ "$RUN_SCAN" == "true" ]]; then
                run_security_scan "$BUILD_TARGET"
            fi
        else
            error "Invalid target: $BUILD_TARGET. Valid targets: ${TARGETS[*]}"
        fi
    fi
    
    success "Docker build process completed!"
    
    # Show summary
    if [[ "$PUSH_IMAGES" != "true" ]]; then
        log "Built images:"
        docker images --filter "reference=${REGISTRY}/codegraph-api" --format "table {{.Repository}}:{{.Tag}}\t{{.Size}}\t{{.CreatedAt}}"
    fi
}

# Run main function
main "$@"