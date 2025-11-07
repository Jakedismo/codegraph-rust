#!/bin/bash

# ABOUTME: Verification script for CodeGraph MCP server setup
# ABOUTME: Tests SurrealDB connection, LLM provider configuration, and MCP server health

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
SURREAL_ENDPOINT="${SURREAL_ENDPOINT:-ws://localhost:3004}"
SURREAL_HTTP_ENDPOINT="http://localhost:3004"
SURREAL_NAMESPACE="${SURREAL_NAMESPACE:-ouroboros}"
SURREAL_DATABASE="${SURREAL_DATABASE:-codegraph}"
SURREAL_USER="${SURREAL_USER:-root}"
SURREAL_PASSWORD="${SURREAL_PASSWORD:-root}"

echo -e "${BLUE}═══════════════════════════════════════════════════${NC}"
echo -e "${BLUE}   CodeGraph MCP Server Verification${NC}"
echo -e "${BLUE}═══════════════════════════════════════════════════${NC}"
echo ""

# Test counter
PASSED=0
FAILED=0

test_passed() {
    echo -e "${GREEN}✓ $1${NC}"
    ((PASSED++))
}

test_failed() {
    echo -e "${RED}✗ $1${NC}"
    ((FAILED++))
}

test_warning() {
    echo -e "${YELLOW}⚠ $1${NC}"
}

# =============================================================================
# 1. Check if SurrealDB is running
# =============================================================================
echo -e "${YELLOW}[1/6] Checking SurrealDB availability...${NC}"

if curl -sf "$SURREAL_HTTP_ENDPOINT/health" > /dev/null 2>&1; then
    test_passed "SurrealDB is running on port 3004"
else
    test_failed "SurrealDB is not running on port 3004"
    echo "       Start SurrealDB with: surreal start --bind 0.0.0.0:3004 --user root --pass root file://data/surreal.db"
fi
echo ""

# =============================================================================
# 2. Test WebSocket connection
# =============================================================================
echo -e "${YELLOW}[2/6] Testing WebSocket connection...${NC}"

# Test if we can connect via WebSocket (using curl's WebSocket upgrade)
if command -v websocat &> /dev/null; then
    # Use websocat if available
    if timeout 2 websocat -n1 "$SURREAL_ENDPOINT" < /dev/null 2>/dev/null; then
        test_passed "WebSocket connection successful"
    else
        test_warning "WebSocket test inconclusive (websocat available but connection unclear)"
    fi
elif command -v wscat &> /dev/null; then
    # Use wscat if available
    if timeout 2 wscat -c "$SURREAL_ENDPOINT" -x 'ping' 2>/dev/null | grep -q 'pong\|connected'; then
        test_passed "WebSocket connection successful"
    else
        test_warning "WebSocket test inconclusive (wscat available but connection unclear)"
    fi
else
    test_warning "WebSocket testing tools not available (install websocat or wscat for full testing)"
    echo "       Assuming WebSocket works if HTTP health check passed"
fi
echo ""

# =============================================================================
# 3. Test SurrealDB authentication and namespace access
# =============================================================================
echo -e "${YELLOW}[3/6] Testing SurrealDB authentication and namespace...${NC}"

if command -v surreal &> /dev/null; then
    # Test if we can authenticate and query
    TEST_QUERY="INFO FOR DB;"
    if surreal sql --endpoint "$SURREAL_HTTP_ENDPOINT" \
        --namespace "$SURREAL_NAMESPACE" \
        --database "$SURREAL_DATABASE" \
        --auth-level root \
        --username "$SURREAL_USER" \
        --password "$SURREAL_PASSWORD" \
        --command "$TEST_QUERY" > /dev/null 2>&1; then
        test_passed "SurrealDB authentication successful"
        test_passed "Namespace '$SURREAL_NAMESPACE' accessible"
        test_passed "Database '$SURREAL_DATABASE' accessible"
    else
        test_failed "SurrealDB authentication or namespace access failed"
        echo "       Check credentials: username=$SURREAL_USER, namespace=$SURREAL_NAMESPACE"
    fi
else
    test_warning "SurrealDB CLI not installed - skipping authentication test"
    echo "       Install from: https://surrealdb.com/install"
fi
echo ""

# =============================================================================
# 4. Check schema tables
# =============================================================================
echo -e "${YELLOW}[4/6] Checking database schema...${NC}"

if command -v surreal &> /dev/null; then
    TABLES_QUERY="SELECT * FROM schema_versions LIMIT 1;"
    if surreal sql --endpoint "$SURREAL_HTTP_ENDPOINT" \
        --namespace "$SURREAL_NAMESPACE" \
        --database "$SURREAL_DATABASE" \
        --auth-level root \
        --username "$SURREAL_USER" \
        --password "$SURREAL_PASSWORD" \
        --command "$TABLES_QUERY" 2>&1 | grep -q "version"; then
        test_passed "Schema tables exist and are accessible"

        # Get schema version
        VERSION=$(surreal sql --endpoint "$SURREAL_HTTP_ENDPOINT" \
            --namespace "$SURREAL_NAMESPACE" \
            --database "$SURREAL_DATABASE" \
            --auth-level root \
            --username "$SURREAL_USER" \
            --password "$SURREAL_PASSWORD" \
            --command "SELECT version FROM schema_versions ORDER BY version DESC LIMIT 1;" 2>&1 | grep -oP 'version.*\K\d+' | head -1)

        if [ -n "$VERSION" ]; then
            echo "       Current schema version: $VERSION"
        fi
    else
        test_warning "Schema tables not found - run './schema/apply-schema.sh' to initialize"
    fi
else
    test_warning "Cannot check schema - SurrealDB CLI not installed"
fi
echo ""

# =============================================================================
# 5. Check LLM Provider Configuration
# =============================================================================
echo -e "${YELLOW}[5/6] Checking LLM provider configuration...${NC}"

# Check for .env file
if [ -f ".env" ]; then
    test_passed "Found .env configuration file"

    # Check for LLM provider settings
    if grep -q "CODEGRAPH_LLM_PROVIDER\|LLM_PROVIDER" .env 2>/dev/null; then
        PROVIDER=$(grep -E "^(CODEGRAPH_LLM_PROVIDER|LLM_PROVIDER)=" .env | cut -d'=' -f2 | tr -d '"' | tr -d "'" | head -1)
        echo "       LLM Provider: ${PROVIDER:-<not set>}"

        case "$PROVIDER" in
            openai)
                if grep -q "OPENAI_API_KEY" .env; then
                    test_passed "OpenAI API key configured"
                else
                    test_failed "OpenAI provider selected but OPENAI_API_KEY not found in .env"
                fi
                ;;
            anthropic)
                if grep -q "ANTHROPIC_API_KEY" .env; then
                    test_passed "Anthropic API key configured"
                else
                    test_failed "Anthropic provider selected but ANTHROPIC_API_KEY not found in .env"
                fi
                ;;
            ollama|qwen)
                test_passed "Local Ollama/Qwen provider configured"
                if grep -q "CODEGRAPH_OLLAMA_URL" .env; then
                    OLLAMA_URL=$(grep "CODEGRAPH_OLLAMA_URL" .env | cut -d'=' -f2 | tr -d '"' | tr -d "'")
                    echo "       Ollama URL: $OLLAMA_URL"
                fi
                ;;
            lmstudio)
                test_passed "LM Studio provider configured"
                if grep -q "CODEGRAPH_LMSTUDIO_URL" .env; then
                    LMSTUDIO_URL=$(grep "CODEGRAPH_LMSTUDIO_URL" .env | cut -d'=' -f2 | tr -d '"' | tr -d "'")
                    echo "       LM Studio URL: $LMSTUDIO_URL"
                fi
                ;;
            *)
                test_warning "LLM provider '$PROVIDER' may not be supported or configured correctly"
                ;;
        esac
    else
        test_warning "LLM_PROVIDER not set in .env file"
        echo "       Set CODEGRAPH_LLM_PROVIDER=openai (or anthropic, ollama, etc.)"
    fi

    # Check model configuration
    if grep -q "CODEGRAPH_MODEL" .env; then
        MODEL=$(grep "CODEGRAPH_MODEL" .env | cut -d'=' -f2 | tr -d '"' | tr -d "'")
        echo "       Model: $MODEL"
    else
        test_warning "CODEGRAPH_MODEL not set in .env"
    fi
else
    test_failed ".env file not found in current directory"
    echo "       Create .env with LLM provider configuration"
fi
echo ""

# =============================================================================
# 6. Test MCP Server Build
# =============================================================================
echo -e "${YELLOW}[6/6] Checking CodeGraph MCP server binary...${NC}"

if command -v codegraph &> /dev/null; then
    test_passed "CodeGraph MCP server binary is installed"

    # Try to get version
    VERSION_OUTPUT=$(codegraph --version 2>&1 || echo "unknown")
    echo "       Version: $VERSION_OUTPUT"

    # Check if it's in PATH
    BINARY_PATH=$(which codegraph)
    echo "       Location: $BINARY_PATH"
else
    test_warning "CodeGraph binary not found in PATH"
    echo "       Build and install with: cargo install --path crates/codegraph-mcp"
fi
echo ""

# =============================================================================
# Summary
# =============================================================================
echo -e "${BLUE}═══════════════════════════════════════════════════${NC}"
echo -e "${BLUE}   Verification Summary${NC}"
echo -e "${BLUE}═══════════════════════════════════════════════════${NC}"
echo ""
echo -e "${GREEN}Passed: $PASSED${NC}"
if [ $FAILED -gt 0 ]; then
    echo -e "${RED}Failed: $FAILED${NC}"
fi
echo ""

if [ $FAILED -eq 0 ]; then
    echo -e "${GREEN}✓ All critical checks passed!${NC}"
    echo ""
    echo "Next steps:"
    echo "1. Apply schema (if not done): cd schema && ./apply-schema.sh"
    echo "2. Index your codebase: codegraph index <path>"
    echo "3. Start MCP server: codegraph start stdio"
    echo ""
    exit 0
else
    echo -e "${RED}✗ Some checks failed. Please fix the issues above.${NC}"
    echo ""
    echo "Common fixes:"
    echo "• Start SurrealDB: surreal start --bind 0.0.0.0:3004 --user root --pass root file://data/surreal.db"
    echo "• Apply schema: cd schema && ./apply-schema.sh"
    echo "• Configure LLM: Add CODEGRAPH_LLM_PROVIDER and API keys to .env"
    echo "• Build MCP server: cargo build --release -p codegraph-mcp"
    echo ""
    exit 1
fi
