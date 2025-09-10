# CodeGraph API - Health Monitoring

This document describes the comprehensive health monitoring system implemented for the CodeGraph API.

## Overview

The health monitoring system provides comprehensive observability for the CodeGraph API server, including:

- **Health Check Endpoints**: Multiple endpoints for different types of health checks
- **Metrics Collection**: Prometheus-compatible metrics for monitoring
- **Service Discovery**: Service registration and discovery with TTL support
- **Graceful Shutdown**: Clean shutdown handling with proper resource cleanup

## Health Check Endpoints

### Comprehensive Health Check
**Endpoint**: `GET /health`

Returns detailed health information about all system components.

**Response Structure**:
```json
{
  "status": "healthy|degraded|unhealthy",
  "version": "0.1.0",
  "timestamp": 1640995200,
  "uptime_seconds": 3600,
  "components": {
    "database": {
      "status": "healthy|unhealthy",
      "last_check": 1640995200,
      "details": "Connection pool: 5/10 active"
    },
    "vector_search": {
      "status": "healthy|unhealthy", 
      "last_check": 1640995200,
      "details": "FAISS index loaded: 50000 vectors"
    },
    "parser": {
      "status": "healthy|unhealthy",
      "last_check": 1640995200,
      "details": "Supported languages: 7"
    },
    "memory": {
      "status": "healthy|warning|critical",
      "last_check": 1640995200,
      "details": "Usage: 75% of 8GB"
    },
    "storage": {
      "status": "healthy|unhealthy",
      "last_check": 1640995200,
      "details": "Disk usage: 45% of 100GB"
    }
  },
  "metrics": {
    "cpu_usage_percent": 25.5,
    "memory_usage_bytes": 1073741824,
    "total_memory_bytes": 8589934592,
    "disk_usage_bytes": 48318382080,
    "total_disk_bytes": 107374182400,
    "active_connections": 5,
    "uptime_seconds": 3600
  }
}
```

**Status Codes**:
- `200`: All components healthy
- `503`: One or more components unhealthy

### Liveness Probe
**Endpoint**: `GET /health/live`

Kubernetes-compatible liveness probe to determine if the application is running.

**Response Structure**:
```json
{
  "status": "alive",
  "timestamp": 1640995200,
  "uptime_seconds": 3600
}
```

**Status Codes**:
- `200`: Application is alive
- `503`: Application should be restarted

### Readiness Probe  
**Endpoint**: `GET /health/ready`

Kubernetes-compatible readiness probe to determine if the application is ready to serve traffic.

**Response Structure**:
```json
{
  "status": "ready|not_ready", 
  "timestamp": 1640995200,
  "checks": {
    "database": "ready|not_ready",
    "dependencies": "ready|not_ready"
  }
}
```

**Status Codes**:
- `200`: Application is ready to serve traffic
- `503`: Application is not ready (remove from load balancer)

## Metrics Endpoint

### Prometheus Metrics
**Endpoint**: `GET /metrics`

Returns Prometheus-compatible metrics for monitoring and alerting.

**Included Metrics**:

#### HTTP Metrics
- `http_requests_total`: Total number of HTTP requests by method, endpoint, and status
- `http_request_duration_seconds`: Request duration histogram 
- `http_requests_in_flight`: Number of currently active requests

#### System Metrics  
- `system_cpu_usage_percent`: Current CPU usage percentage
- `system_memory_usage_bytes`: Current memory usage in bytes
- `system_memory_total_bytes`: Total available memory in bytes
- `system_disk_usage_bytes`: Current disk usage in bytes
- `system_disk_total_bytes`: Total available disk space in bytes

#### Application Metrics
- `application_uptime_seconds`: Application uptime in seconds
- `application_connections_active`: Number of active connections
- `build_info`: Build information with version and commit labels

#### Health Check Metrics
- `health_check_duration_seconds`: Duration of health checks by component
- `health_check_status`: Status of each health check component (0=unhealthy, 1=healthy)

## Service Discovery and Registration

### Register Service
**Endpoint**: `POST /services`

Register a new service in the service registry.

**Request Body**:
```json
{
  "service_name": "my-service",
  "version": "1.0.0", 
  "address": "127.0.0.1",
  "port": 8080,
  "tags": ["http", "api"],
  "metadata": {
    "region": "us-west-2",
    "environment": "production"
  },
  "health_check_url": "http://127.0.0.1:8080/health",
  "ttl_seconds": 60
}
```

**Response**:
```json
{
  "service_id": "my-service-127.0.0.1-8080",
  "message": "Service registered successfully",
  "expires_at": 1640995260
}
```

### Service Discovery
**Endpoint**: `GET /services/discover`

Discover services by name, tag, or health status.

**Query Parameters**:
- `service_name`: Filter by service name
- `tag`: Filter by tag
- `healthy`: Filter by health status (true/false)
- `limit`: Limit number of results

**Response**:
```json
{
  "services": [
    {
      "service_id": "my-service-127.0.0.1-8080",
      "service_name": "my-service",
      "version": "1.0.0",
      "address": "127.0.0.1", 
      "port": 8080,
      "tags": ["http", "api"],
      "metadata": {},
      "health_check_url": "http://127.0.0.1:8080/health",
      "ttl_seconds": 60,
      "registered_at": 1640995200,
      "last_heartbeat": 1640995200
    }
  ],
  "total": 1
}
```

### Service Heartbeat
**Endpoint**: `POST /services/heartbeat`

Send a heartbeat to keep a service registration alive.

**Request Body**:
```json
{
  "service_id": "my-service-127.0.0.1-8080"
}
```

**Response**:
```json
{
  "success": true,
  "message": "Heartbeat recorded",
  "next_heartbeat_in": 30
}
```

### Deregister Service
**Endpoint**: `DELETE /services/{service_id}`

Remove a service from the registry.

**Response**:
```json
{
  "message": "Service my-service-127.0.0.1-8080 deregistered successfully"
}
```

## Memory Leak Detection (Optional)

When compiled with the `leak-detect` feature, additional endpoints are available:

### Memory Statistics
**Endpoint**: `GET /memory/stats`

**Response**:
```json
{
  "total_allocations": 1000000,
  "active_allocations": 50000,
  "leaked_allocations": 10,
  "total_bytes_allocated": 1073741824,
  "active_bytes": 104857600,
  "leaked_bytes": 1024
}
```

### Export Leak Report
**Endpoint**: `GET /memory/leaks`

**Response**:
```json
{
  "exported": true,
  "path": "/tmp/leak_report_20231201_120000.json",
  "leaked_allocations": 10
}
```

## Configuration

### Environment Variables

- `HEALTH_CHECK_INTERVAL`: Interval for background health checks (default: 30s)
- `SERVICE_REGISTRY_CLEANUP_INTERVAL`: Interval for expired service cleanup (default: 30s)  
- `METRICS_ENABLED`: Enable/disable metrics collection (default: true)
- `LEAK_DETECTION_ENABLED`: Enable memory leak detection (default: false)

### Health Check Thresholds

The health monitoring system uses the following thresholds:

- **Memory Warning**: 80% of available memory
- **Memory Critical**: 95% of available memory  
- **Disk Warning**: 80% of available disk space
- **Disk Critical**: 95% of available disk space
- **Connection Pool Warning**: 80% of max connections

## Integration Examples

### Docker Health Check
```dockerfile
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
  CMD curl -f http://localhost:8080/health/live || exit 1
```

### Kubernetes Probes
```yaml
apiVersion: v1
kind: Pod
spec:
  containers:
  - name: codegraph-api
    image: codegraph-api:latest
    ports:
    - containerPort: 8080
    livenessProbe:
      httpGet:
        path: /health/live
        port: 8080
      initialDelaySeconds: 10
      periodSeconds: 30
    readinessProbe:
      httpGet:
        path: /health/ready
        port: 8080
      initialDelaySeconds: 5
      periodSeconds: 10
```

### Prometheus Configuration
```yaml
scrape_configs:
  - job_name: 'codegraph-api'
    static_configs:
      - targets: ['localhost:8080']
    metrics_path: /metrics
    scrape_interval: 15s
```

## Error Handling

All health endpoints return structured error responses:

```json
{
  "error": {
    "code": "HEALTH_CHECK_FAILED",
    "message": "Database connection failed",
    "details": {
      "component": "database",
      "error": "Connection timeout after 5s"
    }
  }
}
```

## Graceful Shutdown

The server implements graceful shutdown with:

1. Signal handling (SIGTERM, SIGINT)
2. Active request completion (30s timeout)
3. Resource cleanup (database connections, file handles)
4. Final health check status update

## Security Considerations

- Health endpoints do not expose sensitive information
- Service registry supports access control via metadata
- Memory leak detection data is sanitized
- All endpoints support rate limiting

## Performance Impact

The health monitoring system is designed for minimal performance impact:

- Background health checks run every 30 seconds
- Metrics collection uses efficient counters and histograms
- Service registry cleanup is batched
- Memory overhead is less than 1MB

## Troubleshooting

### Common Issues

1. **Health checks timeout**: Increase check intervals or timeouts
2. **High memory usage**: Enable leak detection feature
3. **Service discovery empty**: Check TTL settings and heartbeat frequency
4. **Metrics missing**: Verify Prometheus scraping configuration

### Debug Endpoints

Enable debug logging to see detailed health check execution:

```bash
RUST_LOG=debug ./codegraph-api
```