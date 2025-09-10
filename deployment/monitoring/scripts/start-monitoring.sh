#!/bin/bash
# Start the complete CodeGraph monitoring stack
# This script starts all monitoring components in the correct order

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Logging functions
log_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

log_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

log_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Check if Docker and Docker Compose are available
check_dependencies() {
    log_info "Checking dependencies..."
    
    if ! command -v docker &> /dev/null; then
        log_error "Docker is not installed or not in PATH"
        exit 1
    fi
    
    if ! command -v docker-compose &> /dev/null && ! command -v docker compose &> /dev/null; then
        log_error "Docker Compose is not installed or not in PATH"
        exit 1
    fi
    
    # Test Docker daemon
    if ! docker info &> /dev/null; then
        log_error "Docker daemon is not running"
        exit 1
    fi
    
    log_success "Dependencies check passed"
}

# Create required directories and set permissions
setup_directories() {
    log_info "Setting up directories..."
    
    # Create data directories
    mkdir -p data/{prometheus,grafana,elasticsearch,kibana,alertmanager}
    mkdir -p logs/{application,system,nginx}
    mkdir -p config/{prometheus,grafana,elasticsearch,kibana,logstash,filebeat,alertmanager}
    
    # Set permissions for Elasticsearch (requires UID 1000)
    sudo chown -R 1000:1000 data/elasticsearch || log_warning "Could not set Elasticsearch permissions - may need manual setup"
    
    # Set permissions for Grafana (requires UID 472)
    sudo chown -R 472:472 data/grafana || log_warning "Could not set Grafana permissions - may need manual setup"
    
    log_success "Directory setup completed"
}

# Start monitoring stack
start_stack() {
    log_info "Starting monitoring stack..."
    
    cd "$(dirname "$0")/.."
    
    # Start the monitoring stack
    if command -v docker compose &> /dev/null; then
        COMPOSE_CMD="docker compose"
    else
        COMPOSE_CMD="docker-compose"
    fi
    
    log_info "Using compose command: $COMPOSE_CMD"
    
    # Pull latest images first
    log_info "Pulling latest Docker images..."
    $COMPOSE_CMD -f docker-compose.monitoring.yml pull
    
    # Start services in order
    log_info "Starting Elasticsearch..."
    $COMPOSE_CMD -f docker-compose.monitoring.yml up -d elasticsearch
    
    # Wait for Elasticsearch to be ready
    log_info "Waiting for Elasticsearch to be ready..."
    wait_for_service "http://localhost:9200/_cluster/health" "Elasticsearch" 60
    
    log_info "Starting Prometheus..."
    $COMPOSE_CMD -f docker-compose.monitoring.yml up -d prometheus
    wait_for_service "http://localhost:9090/-/healthy" "Prometheus" 30
    
    log_info "Starting Grafana..."
    $COMPOSE_CMD -f docker-compose.monitoring.yml up -d grafana
    wait_for_service "http://localhost:3000/api/health" "Grafana" 30
    
    log_info "Starting remaining services..."
    $COMPOSE_CMD -f docker-compose.monitoring.yml up -d
    
    log_success "Monitoring stack started successfully!"
}

# Wait for a service to be ready
wait_for_service() {
    local url=$1
    local service_name=$2
    local max_attempts=${3:-30}
    local attempt=0
    
    while [ $attempt -lt $max_attempts ]; do
        if curl -s "$url" > /dev/null 2>&1; then
            log_success "$service_name is ready!"
            return 0
        fi
        
        attempt=$((attempt + 1))
        echo -n "."
        sleep 2
    done
    
    log_error "$service_name did not become ready in time"
    return 1
}

# Check service health
check_services() {
    log_info "Checking service health..."
    
    local services=(
        "http://localhost:9090/-/healthy:Prometheus"
        "http://localhost:3000/api/health:Grafana"
        "http://localhost:9200/_cluster/health:Elasticsearch"
        "http://localhost:5601/api/status:Kibana"
        "http://localhost:9093/-/healthy:Alertmanager"
    )
    
    for service in "${services[@]}"; do
        local url="${service%%:*}"
        local name="${service##*:}"
        
        if curl -s "$url" > /dev/null 2>&1; then
            log_success "$name is healthy"
        else
            log_error "$name is not responding"
        fi
    done
}

# Setup Grafana dashboards and data sources
setup_grafana() {
    log_info "Setting up Grafana dashboards..."
    
    # Wait a bit more for Grafana to fully initialize
    sleep 10
    
    # Check if we can reach Grafana API
    if ! curl -s "http://admin:admin@localhost:3000/api/datasources" > /dev/null; then
        log_warning "Could not connect to Grafana API - dashboards may need manual setup"
        return 1
    fi
    
    log_success "Grafana setup completed"
}

# Display access information
show_access_info() {
    log_success "Monitoring stack is ready!"
    echo
    echo "==============================================="
    echo "Service Access URLs:"
    echo "==============================================="
    echo "🔍 Grafana Dashboard:     http://localhost:3000 (admin/admin)"
    echo "📊 Prometheus:            http://localhost:9090"
    echo "🔍 Kibana:                http://localhost:5601"
    echo "🚨 Alertmanager:          http://localhost:9093"
    echo "📈 Node Exporter Metrics: http://localhost:9100/metrics"
    echo "🐳 cAdvisor:              http://localhost:8080"
    echo
    echo "API Health Endpoints:"
    echo "==============================================="
    echo "🏥 Health Check:          http://localhost:3000/health"
    echo "🏥 Enhanced Health:       http://localhost:3000/health/enhanced"
    echo "❤️  Liveness:             http://localhost:3000/health/live"
    echo "✅ Readiness:             http://localhost:3000/health/ready"
    echo "📊 Metrics:               http://localhost:3000/metrics"
    echo
    echo "Log Analysis:"
    echo "==============================================="
    echo "📝 Application logs will appear in Kibana once the app is running"
    echo "📊 Prometheus metrics are scraped every 15 seconds"
    echo "🚨 Alerts are evaluated every 30 seconds"
    echo
    echo "To stop the monitoring stack:"
    echo "  $0 stop"
    echo
}

# Stop monitoring stack
stop_stack() {
    log_info "Stopping monitoring stack..."
    
    cd "$(dirname "$0")/.."
    
    if command -v docker compose &> /dev/null; then
        docker compose -f docker-compose.monitoring.yml down
    else
        docker-compose -f docker-compose.monitoring.yml down
    fi
    
    log_success "Monitoring stack stopped"
}

# Clean up (stop and remove volumes)
cleanup() {
    log_info "Cleaning up monitoring stack..."
    
    cd "$(dirname "$0")/.."
    
    if command -v docker compose &> /dev/null; then
        docker compose -f docker-compose.monitoring.yml down -v
    else
        docker-compose -f docker-compose.monitoring.yml down -v
    fi
    
    # Remove data directories
    if [ -d "data" ]; then
        log_warning "Removing data directories..."
        sudo rm -rf data/
    fi
    
    log_success "Cleanup completed"
}

# Test monitoring stack
test_stack() {
    log_info "Testing monitoring stack..."
    
    # Test Prometheus targets
    log_info "Testing Prometheus targets..."
    if targets=$(curl -s "http://localhost:9090/api/v1/targets" | jq -r '.data.activeTargets[] | select(.health != "up") | .scrapeUrl'); then
        if [ -n "$targets" ]; then
            log_warning "Some Prometheus targets are down:"
            echo "$targets"
        else
            log_success "All Prometheus targets are up"
        fi
    else
        log_error "Could not check Prometheus targets"
    fi
    
    # Test alerting rules
    log_info "Testing alerting rules..."
    if curl -s "http://localhost:9090/api/v1/rules" > /dev/null; then
        log_success "Alerting rules loaded successfully"
    else
        log_error "Could not load alerting rules"
    fi
    
    # Test Grafana data sources
    log_info "Testing Grafana data sources..."
    if curl -s "http://admin:admin@localhost:3000/api/datasources" > /dev/null; then
        log_success "Grafana data sources accessible"
    else
        log_error "Could not access Grafana data sources"
    fi
    
    log_success "Testing completed"
}

# Main script logic
case "${1:-start}" in
    start)
        check_dependencies
        setup_directories
        start_stack
        setup_grafana
        show_access_info
        ;;
    stop)
        stop_stack
        ;;
    cleanup)
        cleanup
        ;;
    test)
        test_stack
        ;;
    restart)
        stop_stack
        sleep 5
        start_stack
        show_access_info
        ;;
    health)
        check_services
        ;;
    *)
        echo "Usage: $0 {start|stop|restart|cleanup|test|health}"
        echo
        echo "Commands:"
        echo "  start    - Start the monitoring stack (default)"
        echo "  stop     - Stop the monitoring stack"
        echo "  restart  - Restart the monitoring stack"
        echo "  cleanup  - Stop and remove all data"
        echo "  test     - Test monitoring stack functionality"
        echo "  health   - Check service health"
        exit 1
        ;;
esac