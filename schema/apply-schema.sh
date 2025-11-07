#!/bin/bash

# ABOUTME: Script to apply CodeGraph SurrealDB schema to a running SurrealDB instance.
# ABOUTME: Supports both local and remote databases with authentication.

set -e

# Default values
ENDPOINT="${SURREAL_ENDPOINT:-http://localhost:3004}"
NAMESPACE="${SURREAL_NAMESPACE:-ouroboros}"
DATABASE="${SURREAL_DATABASE:-codegraph}"
USERNAME="${SURREAL_USER:-root}"
PASSWORD="${SURREAL_PASSWORD:-root}"
SCHEMA_FILE="codegraph.surql"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Help message
show_help() {
    cat << EOF
Usage: $(basename "$0") [OPTIONS]

Apply CodeGraph SurrealDB schema to a running database instance.

OPTIONS:
    -e, --endpoint URL      SurrealDB endpoint (default: http://localhost:3004)
    -n, --namespace NAME    Namespace to use (default: ouroboros)
    -d, --database NAME     Database name (default: codegraph)
    -u, --username USER     Username for authentication (default: root)
    -p, --password PASS     Password for authentication (default: root)
    -s, --schema FILE       Schema file to apply (default: codegraph.surql)
    -m, --migrate           Apply migrations from migrations/ directory
    -h, --help              Show this help message

ENVIRONMENT VARIABLES:
    SURREAL_ENDPOINT        Same as --endpoint
    SURREAL_NAMESPACE       Same as --namespace
    SURREAL_DATABASE        Same as --database
    SURREAL_USER            Same as --username
    SURREAL_PASSWORD        Same as --password

EXAMPLES:
    # Apply schema to local database
    ./apply-schema.sh

    # Apply schema to remote database
    ./apply-schema.sh -e https://db.example.com:8000 -u admin -p secret

    # Apply schema with custom namespace and database
    ./apply-schema.sh -n my_namespace -d my_database

    # Apply schema and run migrations
    ./apply-schema.sh --migrate

EOF
}

# Parse arguments
APPLY_MIGRATIONS=false
while [[ $# -gt 0 ]]; do
    case $1 in
        -e|--endpoint)
            ENDPOINT="$2"
            shift 2
            ;;
        -n|--namespace)
            NAMESPACE="$2"
            shift 2
            ;;
        -d|--database)
            DATABASE="$2"
            shift 2
            ;;
        -u|--username)
            USERNAME="$2"
            shift 2
            ;;
        -p|--password)
            PASSWORD="$2"
            shift 2
            ;;
        -s|--schema)
            SCHEMA_FILE="$2"
            shift 2
            ;;
        -m|--migrate)
            APPLY_MIGRATIONS=true
            shift
            ;;
        -h|--help)
            show_help
            exit 0
            ;;
        *)
            echo -e "${RED}Error: Unknown option $1${NC}"
            show_help
            exit 1
            ;;
    esac
done

# Check if surreal CLI is installed
if ! command -v surreal &> /dev/null; then
    echo -e "${RED}Error: SurrealDB CLI (surreal) is not installed${NC}"
    echo "Install it from: https://surrealdb.com/install"
    exit 1
fi

# Check if schema file exists
if [ ! -f "$SCHEMA_FILE" ]; then
    echo -e "${RED}Error: Schema file '$SCHEMA_FILE' not found${NC}"
    exit 1
fi

# Print configuration
echo -e "${GREEN}=== CodeGraph Schema Application ===${NC}"
echo "Endpoint:  $ENDPOINT"
echo "Namespace: $NAMESPACE"
echo "Database:  $DATABASE"
echo "Schema:    $SCHEMA_FILE"
echo ""

# Test connection
echo -e "${YELLOW}Testing connection...${NC}"
if ! surreal is-ready --endpoint "$ENDPOINT" 2>/dev/null; then
    echo -e "${RED}Error: Cannot connect to SurrealDB at $ENDPOINT${NC}"
    echo "Make sure SurrealDB is running and accessible"
    exit 1
fi
echo -e "${GREEN}✓ Connection successful${NC}"
echo ""

# Apply schema
echo -e "${YELLOW}Applying schema...${NC}"
if surreal sql --endpoint "$ENDPOINT" \
    --namespace "$NAMESPACE" \
    --database "$DATABASE" \
    --auth-level root \
    --username "$USERNAME" \
    --password "$PASSWORD" \
    < "$SCHEMA_FILE"; then
    echo -e "${GREEN}✓ Schema applied successfully${NC}"
else
    echo -e "${RED}Error: Failed to apply schema${NC}"
    exit 1
fi
echo ""

# Apply migrations if requested
if [ "$APPLY_MIGRATIONS" = true ]; then
    if [ -d "migrations" ] && [ "$(ls -A migrations/*.surql 2>/dev/null)" ]; then
        echo -e "${YELLOW}Applying migrations...${NC}"
        for migration in migrations/*.surql; do
            if [ "$migration" = "migrations/template.surql" ]; then
                continue
            fi
            echo "Applying: $(basename "$migration")"
            if surreal sql --endpoint "$ENDPOINT" \
                --namespace "$NAMESPACE" \
                --database "$DATABASE" \
                --auth-level root \
                --username "$USERNAME" \
                --password "$PASSWORD" \
                < "$migration"; then
                echo -e "${GREEN}✓ Migration applied: $(basename "$migration")${NC}"
            else
                echo -e "${RED}Error: Failed to apply migration: $(basename "$migration")${NC}"
                exit 1
            fi
        done
        echo ""
    else
        echo -e "${YELLOW}No migrations found in migrations/ directory${NC}"
        echo ""
    fi
fi

# Verify schema
echo -e "${YELLOW}Verifying schema...${NC}"
VERIFY_QUERY="INFO FOR DB;"
if surreal sql --endpoint "$ENDPOINT" \
    --namespace "$NAMESPACE" \
    --database "$DATABASE" \
    --auth-level root \
    --username "$USERNAME" \
    --password "$PASSWORD" \
    --command "$VERIFY_QUERY" > /dev/null; then
    echo -e "${GREEN}✓ Schema verification successful${NC}"
else
    echo -e "${RED}Warning: Schema verification failed${NC}"
fi

echo ""
echo -e "${GREEN}=== Schema Application Complete ===${NC}"
echo ""
echo "Next steps:"
echo "1. Verify tables: surreal sql --endpoint $ENDPOINT -ns $NAMESPACE -db $DATABASE --command 'INFO FOR DB;'"
echo "2. Run CodeGraph indexer to populate the database"
echo "3. Query your code graph!"
