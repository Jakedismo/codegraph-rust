---
pdf-engine: lualatex
mainfont: "DejaVu Serif"
monofont: "DejaVu Sans Mono"
header-includes: |
  \usepackage{fontspec}
  \directlua{
    luaotfload.add_fallback("emojifallback", {"NotoColorEmoji:mode=harf;"})
  }
  \setmainfont[
    RawFeature={fallback=emojifallback}
  ]{DejaVu Serif}
---

# CodeGraph Deployment Guide

**Production-ready deployment strategies for CodeGraph API and services**

## Quick Navigation

- [Docker Deployment](#docker-deployment) - Container-based deployment
- [Kubernetes Orchestration](#kubernetes-deployment) - Scalable container orchestration  
- [Environment Configuration](#environment-configuration) - Configuration management
- [Database Setup](#database-setup) - RocksDB and data persistence
- [SSL/TLS Setup](#ssltls-configuration) - Security configuration
- [Monitoring & Alerting](#monitoring-and-alerting) - Observability setup
- [Backup & Recovery](#backup-and-recovery) - Data protection

## System Requirements

### Minimum Requirements
- **CPU**: 2 cores (4 threads)
- **RAM**: 4GB (8GB recommended)
- **Storage**: 20GB SSD (50GB+ for large projects)
- **Network**: 1Gbps connection

### Production Requirements  
- **CPU**: 8+ cores (16 threads recommended)
- **RAM**: 16GB (32GB+ for high throughput)
- **Storage**: 100GB+ NVMe SSD
- **Network**: 10Gbps connection with low latency

### Operating System Support
- **Linux**: Ubuntu 20.04+, CentOS 8+, Debian 11+, RHEL 8+
- **macOS**: 12.0+ (development only)
- **Windows**: Server 2019+ (limited support)

## Docker Deployment

### Quick Start with Docker

**1. Pull the Official Image**
```bash
# Latest stable release
docker pull codegraph/api:latest

# Specific version
docker pull codegraph/api:v0.1.0

# Development builds
docker pull codegraph/api:main
```

**2. Basic Container Run**
```bash
docker run -d \
  --name codegraph-api \
  -p 8000:8000 \
  -v codegraph-data:/app/data \
  -e CODEGRAPH_LOG_LEVEL=info \
  codegraph/api:latest
```

**3. Verify Deployment**
```bash
# Health check
curl http://localhost:8000/health

# Expected response
{
  "status": "healthy",
  "version": "0.1.0", 
  "uptime": "30s",
  "features": ["graph", "vector", "mcp"]
}
```

### Production Docker Configuration

**docker-compose.yml**
```yaml
version: '3.8'

services:
  codegraph-api:
    image: codegraph/api:latest
    container_name: codegraph-api
    restart: unless-stopped
    ports:
      - "8000:8000"
    volumes:
      # Data persistence
      - codegraph-data:/app/data
      - codegraph-config:/app/config
      - codegraph-logs:/app/logs
      # Optional: Mount source code for analysis
      - /host/projects:/app/projects:ro
    environment:
      # Server configuration
      - CODEGRAPH_HOST=0.0.0.0
      - CODEGRAPH_PORT=8000
      - CODEGRAPH_WORKERS=8
      
      # Database settings  
      - CODEGRAPH_DB_PATH=/app/data/rocks.db
      - CODEGRAPH_DB_CACHE_SIZE=2048
      - CODEGRAPH_BACKUP_ENABLED=true
      
      # Performance tuning
      - CODEGRAPH_MAX_MEMORY=4GB
      - CODEGRAPH_CONCURRENT_LIMIT=200
      - CODEGRAPH_QUERY_TIMEOUT=30s
      
      # Logging and monitoring
      - CODEGRAPH_LOG_LEVEL=info
      - CODEGRAPH_LOG_FORMAT=json
      - CODEGRAPH_METRICS_ENABLED=true
      
      # Security
      - CODEGRAPH_API_KEY=${CODEGRAPH_API_KEY}
      - CODEGRAPH_RATE_LIMIT=1000
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:8000/health"]
      interval: 30s
      timeout: 10s
      retries: 3
      start_period: 40s
    deploy:
      resources:
        limits:
          cpus: '4.0'
          memory: 4G
        reservations:
          cpus: '2.0'
          memory: 2G
    logging:
      driver: json-file
      options:
        max-size: "100m"
        max-file: "5"

  # Optional: Prometheus for metrics
  prometheus:
    image: prom/prometheus:latest
    container_name: codegraph-prometheus
    restart: unless-stopped
    ports:
      - "9090:9090"
    volumes:
      - ./prometheus.yml:/etc/prometheus/prometheus.yml:ro
      - prometheus-data:/prometheus
    command:
      - '--config.file=/etc/prometheus/prometheus.yml'
      - '--storage.tsdb.path=/prometheus'
      - '--web.console.libraries=/etc/prometheus/console_libraries'
      - '--web.console.templates=/etc/prometheus/consoles'
      - '--storage.tsdb.retention.time=30d'
      - '--web.enable-lifecycle'

  # Optional: Grafana for visualization
  grafana:
    image: grafana/grafana:latest
    container_name: codegraph-grafana
    restart: unless-stopped
    ports:
      - "3000:3000"
    volumes:
      - grafana-data:/var/lib/grafana
      - ./grafana/dashboards:/etc/grafana/provisioning/dashboards
      - ./grafana/datasources:/etc/grafana/provisioning/datasources
    environment:
      - GF_SECURITY_ADMIN_PASSWORD=${GRAFANA_ADMIN_PASSWORD:-admin}
      - GF_USERS_ALLOW_SIGN_UP=false

volumes:
  codegraph-data:
    driver: local
  codegraph-config:
    driver: local  
  codegraph-logs:
    driver: local
  prometheus-data:
    driver: local
  grafana-data:
    driver: local

networks:
  default:
    name: codegraph
```

**Environment File (.env)**
```bash
# Security
CODEGRAPH_API_KEY=your-secret-api-key-here
GRAFANA_ADMIN_PASSWORD=secure-grafana-password

# Optional: Custom configuration
CODEGRAPH_LOG_LEVEL=info
CODEGRAPH_WORKERS=8
CODEGRAPH_MAX_MEMORY=4GB
```

### Multi-Stage Docker Build

For custom builds, use our optimized Dockerfile:

```dockerfile
# Multi-stage build for minimal production image
FROM rust:1.75-slim as builder

WORKDIR /app

# Install system dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    cmake \
    clang \
    build-essential \
    && rm -rf /var/lib/apt/lists/*

# Copy dependency files
COPY Cargo.toml Cargo.lock ./
COPY crates/ crates/

# Build with optimizations
RUN cargo build --release --locked

# Runtime stage with minimal distroless image
FROM gcr.io/distroless/cc-debian12

WORKDIR /app

# Copy binary from builder
COPY --from=builder /app/target/release/codegraph-api /app/

# Create non-root user
USER nonroot:nonroot

# Create directories with proper permissions
COPY --chown=nonroot:nonroot --from=builder /app/target/release/codegraph-api /app/codegraph-api

# Expose port
EXPOSE 8000

# Health check
HEALTHCHECK --interval=30s --timeout=10s --start-period=5s --retries=3 \
  CMD ["/app/codegraph-api", "--version"]

# Entry point
ENTRYPOINT ["/app/codegraph-api"]
```

## Kubernetes Deployment

### Prerequisites

**Required Kubernetes Version**: 1.24+

**Required Resources**:
- Persistent Volume support
- LoadBalancer or Ingress Controller
- Metrics Server (optional)

### Namespace Setup

```yaml
# namespace.yaml
apiVersion: v1
kind: Namespace
metadata:
  name: codegraph
  labels:
    name: codegraph
    app.kubernetes.io/name: codegraph
```

### ConfigMap and Secrets

```yaml
# configmap.yaml  
apiVersion: v1
kind: ConfigMap
metadata:
  name: codegraph-config
  namespace: codegraph
data:
  config.toml: |
    [server]
    host = "0.0.0.0"
    port = 8000
    workers = 8
    max_connections = 1000
    timeout = "30s"
    
    [database]
    path = "/app/data/rocks.db"
    cache_size = 2048
    max_open_files = 2000
    enable_statistics = true
    backup_interval = "24h"
    
    [vector]
    index_type = "hnsw"
    dimension = 768
    metric = "cosine"
    m = 16
    ef_construction = 200
    ef_search = 64
    
    [performance]
    query_timeout = "30s"
    index_batch_size = 2000
    concurrent_limit = 200
    max_memory_usage = "4GB"
    
    [logging]
    level = "info"
    format = "json"
    output = "stdout"
    
    [metrics]
    enabled = true
    endpoint = "/metrics"
    include_system_metrics = true

---
# secrets.yaml
apiVersion: v1
kind: Secret
metadata:
  name: codegraph-secrets
  namespace: codegraph
type: Opaque
data:
  # Base64 encoded values
  api-key: eW91ci1zZWNyZXQtYXBpLWtleQ==
  # admin-password: YWRtaW4tcGFzc3dvcmQ=
```

### Persistent Storage

```yaml
# storage.yaml
apiVersion: v1
kind: PersistentVolumeClaim
metadata:
  name: codegraph-data-pvc
  namespace: codegraph
spec:
  accessModes:
    - ReadWriteOnce
  storageClassName: fast-ssd  # Use your storage class
  resources:
    requests:
      storage: 100Gi

---
apiVersion: v1  
kind: PersistentVolumeClaim
metadata:
  name: codegraph-config-pvc
  namespace: codegraph
spec:
  accessModes:
    - ReadWriteOnce
  storageClassName: standard
  resources:
    requests:
      storage: 1Gi
```

### Deployment

```yaml
# deployment.yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: codegraph-api
  namespace: codegraph
  labels:
    app: codegraph-api
    version: v0.1.0
spec:
  replicas: 3
  strategy:
    type: RollingUpdate
    rollingUpdate:
      maxSurge: 1
      maxUnavailable: 1
  selector:
    matchLabels:
      app: codegraph-api
  template:
    metadata:
      labels:
        app: codegraph-api
        version: v0.1.0
      annotations:
        prometheus.io/scrape: "true"
        prometheus.io/path: "/metrics"
        prometheus.io/port: "8000"
    spec:
      securityContext:
        runAsNonRoot: true
        runAsUser: 65534  # nonroot user
        fsGroup: 65534
      containers:
      - name: codegraph-api
        image: codegraph/api:latest
        imagePullPolicy: Always
        ports:
        - name: http
          containerPort: 8000
          protocol: TCP
        - name: metrics
          containerPort: 8000  # Same port, different path
          protocol: TCP
        env:
        - name: CODEGRAPH_CONFIG
          value: "/app/config/config.toml"
        - name: CODEGRAPH_API_KEY
          valueFrom:
            secretKeyRef:
              name: codegraph-secrets
              key: api-key
        - name: RUST_LOG
          value: "info"
        - name: RUST_BACKTRACE
          value: "1"
        resources:
          requests:
            memory: "2Gi"
            cpu: "1000m"
            ephemeral-storage: "2Gi"
          limits:
            memory: "4Gi"
            cpu: "2000m"  
            ephemeral-storage: "4Gi"
        volumeMounts:
        - name: data
          mountPath: /app/data
        - name: config
          mountPath: /app/config
        - name: tmp
          mountPath: /tmp
        livenessProbe:
          httpGet:
            path: /health
            port: http
          initialDelaySeconds: 30
          periodSeconds: 10
          timeoutSeconds: 5
          failureThreshold: 3
        readinessProbe:
          httpGet:
            path: /health  
            port: http
          initialDelaySeconds: 10
          periodSeconds: 5
          timeoutSeconds: 3
          failureThreshold: 2
        startupProbe:
          httpGet:
            path: /health
            port: http
          initialDelaySeconds: 10
          periodSeconds: 10
          timeoutSeconds: 5
          failureThreshold: 30
      volumes:
      - name: data
        persistentVolumeClaim:
          claimName: codegraph-data-pvc
      - name: config
        configMap:
          name: codegraph-config
      - name: tmp
        emptyDir: {}
      nodeSelector:
        node-type: compute  # Optional: Target specific nodes
      tolerations:
      - key: "codegraph"
        operator: "Equal"
        value: "dedicated"
        effect: "NoSchedule"
```

### Service and Ingress

```yaml
# service.yaml
apiVersion: v1
kind: Service  
metadata:
  name: codegraph-api-service
  namespace: codegraph
  labels:
    app: codegraph-api
spec:
  type: ClusterIP
  ports:
  - name: http
    port: 80
    targetPort: 8000
    protocol: TCP
  selector:
    app: codegraph-api

---
# ingress.yaml
apiVersion: networking.k8s.io/v1
kind: Ingress
metadata:
  name: codegraph-api-ingress
  namespace: codegraph
  annotations:
    # Nginx Ingress Controller
    kubernetes.io/ingress.class: nginx
    cert-manager.io/cluster-issuer: letsencrypt-prod
    nginx.ingress.kubernetes.io/ssl-redirect: "true"
    nginx.ingress.kubernetes.io/proxy-body-size: "100m"
    nginx.ingress.kubernetes.io/proxy-read-timeout: "300"
    nginx.ingress.kubernetes.io/proxy-send-timeout: "300"
    nginx.ingress.kubernetes.io/rate-limit: "1000"
    nginx.ingress.kubernetes.io/rate-limit-window: "1m"
spec:
  tls:
  - hosts:
    - api.codegraph.dev
    secretName: codegraph-api-tls
  rules:
  - host: api.codegraph.dev
    http:
      paths:
      - path: /
        pathType: Prefix
        backend:
          service:
            name: codegraph-api-service
            port:
              number: 80
```

### Horizontal Pod Autoscaler

```yaml
# hpa.yaml
apiVersion: autoscaling/v2
kind: HorizontalPodAutoscaler
metadata:
  name: codegraph-api-hpa
  namespace: codegraph
spec:
  scaleTargetRef:
    apiVersion: apps/v1
    kind: Deployment
    name: codegraph-api
  minReplicas: 3
  maxReplicas: 20
  metrics:
  - type: Resource
    resource:
      name: cpu
      target:
        type: Utilization
        averageUtilization: 70
  - type: Resource
    resource:
      name: memory
      target:
        type: Utilization
        averageUtilization: 80
  - type: Pods
    pods:
      metric:
        name: http_requests_per_second
      target:
        type: AverageValue
        averageValue: "1000"
  behavior:
    scaleDown:
      stabilizationWindowSeconds: 300
      policies:
      - type: Percent
        value: 10
        periodSeconds: 60
    scaleUp:
      stabilizationWindowSeconds: 60
      policies:
      - type: Percent
        value: 50
        periodSeconds: 60
      - type: Pods
        value: 2
        periodSeconds: 60
```

### Pod Disruption Budget

```yaml
# pdb.yaml
apiVersion: policy/v1
kind: PodDisruptionBudget
metadata:
  name: codegraph-api-pdb
  namespace: codegraph
spec:
  minAvailable: 2  # Always keep at least 2 pods running
  selector:
    matchLabels:
      app: codegraph-api
```

### Deployment Commands

```bash
# Create namespace
kubectl apply -f namespace.yaml

# Deploy storage
kubectl apply -f storage.yaml

# Deploy configuration
kubectl apply -f configmap.yaml
kubectl apply -f secrets.yaml

# Deploy application
kubectl apply -f deployment.yaml
kubectl apply -f service.yaml
kubectl apply -f ingress.yaml

# Deploy autoscaling
kubectl apply -f hpa.yaml
kubectl apply -f pdb.yaml

# Verify deployment
kubectl get all -n codegraph
kubectl describe pod -l app=codegraph-api -n codegraph

# Check logs
kubectl logs -f deployment/codegraph-api -n codegraph

# Port forward for testing
kubectl port-forward svc/codegraph-api-service 8000:80 -n codegraph
```

## Environment Configuration

### Configuration Hierarchy

CodeGraph uses a layered configuration system:

1. **Default values** (built into binary)
2. **Configuration files** (`config.toml`)  
3. **Environment variables** (override config files)
4. **Command line arguments** (override environment)

### Complete Configuration Reference

```toml
# config.toml - Production configuration template

[server]
# Network binding
host = "0.0.0.0"                    # Listen address
port = 8000                         # HTTP port
workers = 8                         # Worker threads (CPU cores)
max_connections = 1000              # Concurrent connections
timeout = "30s"                     # Request timeout
keep_alive = "75s"                  # Connection keep-alive
cors_origins = ["https://yourdomain.com"]  # CORS allowed origins

[database]  
# RocksDB configuration
path = "/app/data/rocks.db"         # Database directory
cache_size = 2048                   # Memory cache size (MB)
max_open_files = 2000               # OS file limit
write_buffer_size = 128             # Write buffer (MB)
max_write_buffer_number = 6         # Number of write buffers
enable_statistics = true            # Performance stats
paranoid_checks = false             # Extra validation

# Compression settings  
compression_type = "zstd"           # Compression algorithm
compression_level = 6               # Compression level (1-9)

# Backup configuration
backup_enabled = true               # Enable backups
backup_interval = "24h"             # Backup frequency  
backup_retention = "30d"            # Backup retention
backup_path = "/app/backups"        # Backup directory

[vector]
# FAISS vector search configuration
index_type = "hnsw"                 # Index type: hnsw, ivf, flat
dimension = 768                     # Embedding dimension
metric = "cosine"                   # Distance metric: cosine, l2, inner_product

# HNSW-specific settings
m = 16                              # Number of connections
ef_construction = 200               # Build-time search width
ef_search = 64                      # Query-time search width
max_elements = 10000000             # Maximum vectors

# Model configuration
embedding_model = "sentence-transformers"  # Model type
model_path = "/app/models"          # Model cache directory
batch_size = 32                     # Batch processing size

[parsing]
# Language support
languages = [
    "rust", "python", "javascript", "typescript",
    "go", "java", "cpp", "c", "csharp", "kotlin"
]
max_file_size = "50MB"              # Maximum file size
max_files_per_project = 100000      # File limit per project

# Global ignore patterns
ignore_patterns = [
    # Build artifacts
    "target/", "build/", "dist/", "out/", ".output/",
    # Dependencies  
    "node_modules/", "vendor/", ".cargo/",
    # Cache and temporary
    "__pycache__/", ".cache/", ".tmp/", "*.pyc", "*.pyo",
    # Version control
    ".git/", ".svn/", ".hg/", ".bzr/",
    # IDE and editors
    ".vscode/", ".idea/", "*.swp", "*.swo", "*~",
    # OS files
    ".DS_Store", "Thumbs.db", "desktop.ini"
]

# Language-specific settings
[parsing.rust]
parse_tests = true
parse_benchmarks = true
parse_examples = true

[parsing.python]
parse_notebooks = true
parse_stubs = true

[mcp]
# Model Context Protocol
enabled = true                      # Enable MCP server
endpoint = "/mcp"                   # MCP endpoint path
max_request_size = "10MB"           # Maximum request size
rate_limit = 100                    # Requests per minute per client
timeout = "60s"                     # Request timeout

[security]
# API security
api_key_required = true             # Require API keys
api_key_header = "Authorization"    # API key header name
jwt_secret = "your-jwt-secret"      # JWT signing secret
jwt_expiration = "24h"              # JWT token lifetime

# Rate limiting
rate_limiting = true                # Enable rate limiting
max_requests_per_minute = 1000      # Global rate limit
max_requests_per_hour = 10000       # Hourly limit
burst_size = 100                    # Burst allowance

# CORS settings
cors_enabled = true                 # Enable CORS
cors_max_age = 3600                 # Preflight cache time

[logging]
# Logging configuration
level = "info"                      # Log level: trace, debug, info, warn, error
format = "json"                     # Format: json, pretty
output = "stdout"                   # Output: stdout, stderr, file path

# Structured logging
include_request_id = true           # Include request IDs
include_user_agent = true           # Include user agents
include_response_time = true        # Include response times

# File rotation (when output is file)
max_file_size = "100MB"             # Max log file size  
max_files = 10                      # Number of files to keep

[metrics]
# Prometheus metrics
enabled = true                      # Enable metrics
endpoint = "/metrics"               # Metrics endpoint
include_system_metrics = true       # System metrics (CPU, memory)
include_custom_metrics = true       # Application metrics

# Metric collection intervals
collection_interval = "15s"         # Metric collection frequency
histogram_buckets = [               # Response time buckets
    0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0
]

[performance]
# Performance tuning
query_timeout = "30s"               # Query timeout
index_timeout = "5m"                # Indexing timeout
search_timeout = "10s"              # Search timeout

# Concurrency limits
max_concurrent_queries = 100        # Concurrent queries
max_concurrent_indexing = 5         # Concurrent indexing jobs
max_concurrent_backups = 2          # Concurrent backups

# Memory management
max_memory_usage = "4GB"            # Memory limit
gc_interval = "5m"                  # Garbage collection frequency
memory_pressure_threshold = 0.85    # Memory pressure trigger

# Caching
enable_query_cache = true           # Enable query caching
query_cache_size = 1000             # Cache entries
query_cache_ttl = "1h"              # Cache time-to-live
enable_result_cache = true          # Enable result caching
result_cache_ttl = "15m"            # Result cache TTL

# Batch processing
batch_size = 1000                   # Default batch size
max_batch_size = 10000              # Maximum batch size
batch_timeout = "30s"               # Batch processing timeout
```

### Environment Variables

All configuration options can be overridden with environment variables using the pattern `CODEGRAPH_<SECTION>_<KEY>`:

```bash
# Server configuration
export CODEGRAPH_SERVER_HOST=0.0.0.0
export CODEGRAPH_SERVER_PORT=8000
export CODEGRAPH_SERVER_WORKERS=8

# Database configuration
export CODEGRAPH_DATABASE_PATH=/app/data/rocks.db
export CODEGRAPH_DATABASE_CACHE_SIZE=2048

# Security
export CODEGRAPH_SECURITY_API_KEY_REQUIRED=true
export CODEGRAPH_SECURITY_JWT_SECRET=your-secure-jwt-secret

# Performance
export CODEGRAPH_PERFORMANCE_MAX_MEMORY_USAGE=4GB
export CODEGRAPH_PERFORMANCE_QUERY_TIMEOUT=30s

# Logging
export CODEGRAPH_LOGGING_LEVEL=info
export CODEGRAPH_LOGGING_FORMAT=json

# Special environment variables
export RUST_LOG=codegraph=info,tokio=warn,hyper=warn
export RUST_BACKTRACE=1  # Enable backtraces in debug builds
```

## Database Setup

### RocksDB Configuration

CodeGraph uses RocksDB for graph storage with optimized settings:

**Performance Tuning**:
```toml
[database]
# Memory allocation
cache_size = 2048                   # Shared cache (MB)
write_buffer_size = 256             # Memtable size (MB)  
max_write_buffer_number = 6         # Number of memtables

# File management
max_open_files = 2000               # OS file limit
max_background_jobs = 8             # Background threads

# Compaction settings
level0_file_num_compaction_trigger = 4
level0_slowdown_writes_trigger = 20
level0_stop_writes_trigger = 36
target_file_size_base = 67108864    # 64MB
max_bytes_for_level_base = 268435456 # 256MB

# Compression
compression_type = "zstd"           # Best compression
compression_level = 6               # Balanced compression
bottommost_compression_type = "zstd"
```

### Data Directory Structure

```
/app/data/
├── rocks.db/                      # Main RocksDB database
│   ├── CURRENT                    # Current manifest
│   ├── MANIFEST-*                 # Database manifest
│   ├── OPTIONS-*                  # Database options  
│   └── *.sst                      # SST files (data)
├── vector/                        # Vector indices
│   ├── hnsw.index                 # FAISS HNSW index
│   ├── metadata.json              # Index metadata
│   └── embeddings.bin             # Raw embeddings
├── cache/                         # Query caches
└── temp/                          # Temporary files
```

### Backup Strategy

**Automated Backups**:
```bash
#!/bin/bash
# backup.sh - Automated backup script

BACKUP_DIR="/app/backups"
TIMESTAMP=$(date +%Y%m%d_%H%M%S)
BACKUP_NAME="codegraph_backup_${TIMESTAMP}"

# Create backup directory
mkdir -p "${BACKUP_DIR}/${BACKUP_NAME}"

# RocksDB backup (using checkpoint)
curl -X POST "http://localhost:8000/api/v1/admin/backup" \
  -H "Authorization: Bearer ${ADMIN_API_KEY}" \
  -d "{\"path\": \"${BACKUP_DIR}/${BACKUP_NAME}\"}"

# Compress backup
tar -czf "${BACKUP_DIR}/${BACKUP_NAME}.tar.gz" \
  -C "${BACKUP_DIR}" "${BACKUP_NAME}"

# Remove uncompressed backup
rm -rf "${BACKUP_DIR}/${BACKUP_NAME}"

# Clean old backups (keep 30 days)
find "${BACKUP_DIR}" -name "*.tar.gz" -mtime +30 -delete

echo "Backup completed: ${BACKUP_NAME}.tar.gz"
```

**Cron Job Setup**:
```bash
# Add to crontab
0 2 * * * /app/scripts/backup.sh
```

## SSL/TLS Configuration

### Reverse Proxy with Nginx

```nginx
# /etc/nginx/sites-available/codegraph
server {
    listen 443 ssl http2;
    listen [::]:443 ssl http2;
    server_name api.codegraph.dev;

    # SSL configuration
    ssl_certificate /etc/letsencrypt/live/api.codegraph.dev/fullchain.pem;
    ssl_certificate_key /etc/letsencrypt/live/api.codegraph.dev/privkey.pem;
    
    # SSL security settings
    ssl_protocols TLSv1.2 TLSv1.3;
    ssl_ciphers ECDHE-RSA-AES256-GCM-SHA512:DHE-RSA-AES256-GCM-SHA512:ECDHE-RSA-AES256-GCM-SHA384:DHE-RSA-AES256-GCM-SHA384;
    ssl_prefer_server_ciphers off;
    ssl_session_cache shared:SSL:10m;
    ssl_session_timeout 10m;

    # Security headers
    add_header Strict-Transport-Security "max-age=31536000; includeSubDomains" always;
    add_header X-Content-Type-Options nosniff always;
    add_header X-Frame-Options DENY always;
    add_header X-XSS-Protection "1; mode=block" always;
    add_header Referrer-Policy "strict-origin-when-cross-origin" always;

    # Rate limiting
    limit_req_zone $binary_remote_addr zone=api:10m rate=100r/m;
    limit_req zone=api burst=20 nodelay;

    # Proxy configuration
    location / {
        proxy_pass http://localhost:8000;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
        
        # Timeouts
        proxy_connect_timeout 60s;
        proxy_send_timeout 60s;
        proxy_read_timeout 60s;
        
        # Buffer settings
        proxy_buffering on;
        proxy_buffer_size 4k;
        proxy_buffers 8 4k;
        proxy_busy_buffers_size 8k;
        
        # WebSocket support
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection "upgrade";
    }

    # Health check endpoint (bypass rate limiting)
    location /health {
        proxy_pass http://localhost:8000/health;
        proxy_set_header Host $host;
        limit_req off;
    }

    # Metrics endpoint (restrict access)
    location /metrics {
        allow 10.0.0.0/8;
        allow 172.16.0.0/12;
        allow 192.168.0.0/16;
        deny all;
        
        proxy_pass http://localhost:8000/metrics;
        proxy_set_header Host $host;
    }
}

# Redirect HTTP to HTTPS
server {
    listen 80;
    listen [::]:80;
    server_name api.codegraph.dev;
    return 301 https://$server_name$request_uri;
}
```

### TLS Certificate Management

**Let's Encrypt with Certbot**:
```bash
# Install certbot
sudo apt-get update
sudo apt-get install snapd
sudo snap install --classic certbot

# Generate certificate
sudo certbot --nginx -d api.codegraph.dev

# Auto-renewal (add to crontab)
0 12 * * * /usr/bin/certbot renew --quiet
```

## Monitoring and Alerting

### Prometheus Configuration

```yaml
# prometheus.yml
global:
  scrape_interval: 15s
  evaluation_interval: 15s

rule_files:
  - "codegraph_rules.yml"

alerting:
  alertmanagers:
    - static_configs:
        - targets:
          - alertmanager:9093

scrape_configs:
  - job_name: 'codegraph-api'
    static_configs:
      - targets: ['codegraph-api:8000']
    metrics_path: '/metrics'
    scrape_interval: 15s
    scrape_timeout: 10s
    
  - job_name: 'node-exporter'
    static_configs:
      - targets: ['node-exporter:9100']
    scrape_interval: 15s
```

### Alerting Rules

```yaml
# codegraph_rules.yml  
groups:
- name: codegraph.rules
  rules:
  # High error rate
  - alert: CodeGraphHighErrorRate
    expr: rate(codegraph_requests_total{status=~"5.."}[5m]) > 0.05
    for: 2m
    labels:
      severity: critical
    annotations:
      summary: "CodeGraph API high error rate"
      description: "Error rate is {{ $value | humanizePercentage }} for the last 5 minutes"

  # High latency
  - alert: CodeGraphHighLatency
    expr: histogram_quantile(0.95, rate(codegraph_request_duration_seconds_bucket[5m])) > 1.0
    for: 5m
    labels:
      severity: warning
    annotations:
      summary: "CodeGraph API high latency"
      description: "95th percentile latency is {{ $value }}s"

  # Memory usage
  - alert: CodeGraphHighMemoryUsage
    expr: process_resident_memory_bytes / (1024 * 1024 * 1024) > 3.5
    for: 5m
    labels:
      severity: warning
    annotations:
      summary: "CodeGraph API high memory usage"
      description: "Memory usage is {{ $value }}GB"

  # Database size
  - alert: CodeGraphDatabaseSizeGrowing
    expr: increase(codegraph_database_size_bytes[1h]) > (100 * 1024 * 1024)
    for: 0m
    labels:
      severity: info
    annotations:
      summary: "CodeGraph database growing rapidly"
      description: "Database grew by {{ $value | humanizeBytes }} in the last hour"
```

### Grafana Dashboards

```json
{
  "dashboard": {
    "id": null,
    "title": "CodeGraph API Dashboard",
    "tags": ["codegraph"],
    "timezone": "browser",
    "panels": [
      {
        "title": "Request Rate",
        "type": "graph",
        "targets": [
          {
            "expr": "rate(codegraph_requests_total[5m])",
            "legendFormat": "{{method}} {{endpoint}}"
          }
        ]
      },
      {
        "title": "Response Time",
        "type": "graph", 
        "targets": [
          {
            "expr": "histogram_quantile(0.50, rate(codegraph_request_duration_seconds_bucket[5m]))",
            "legendFormat": "50th percentile"
          },
          {
            "expr": "histogram_quantile(0.95, rate(codegraph_request_duration_seconds_bucket[5m]))",
            "legendFormat": "95th percentile"
          }
        ]
      },
      {
        "title": "Memory Usage",
        "type": "graph",
        "targets": [
          {
            "expr": "process_resident_memory_bytes",
            "legendFormat": "RSS"
          },
          {
            "expr": "process_virtual_memory_bytes", 
            "legendFormat": "Virtual"
          }
        ]
      }
    ]
  }
}
```

## Backup and Recovery

### Backup Scripts

**Complete Backup Script**:
```bash
#!/bin/bash
# comprehensive_backup.sh

set -euo pipefail

# Configuration
BACKUP_ROOT="/app/backups"
DATA_DIR="/app/data"
CONFIG_DIR="/app/config"
RETENTION_DAYS=30
TIMESTAMP=$(date +%Y%m%d_%H%M%S)
BACKUP_DIR="${BACKUP_ROOT}/${TIMESTAMP}"

# Logging
exec 1> >(logger -t codegraph-backup)
exec 2> >(logger -t codegraph-backup)

echo "Starting backup at $(date)"

# Create backup directory
mkdir -p "${BACKUP_DIR}"

# 1. Create RocksDB checkpoint (consistent snapshot)
echo "Creating database checkpoint..."
curl -sf -X POST "http://localhost:8000/api/v1/admin/checkpoint" \
  -H "Authorization: Bearer ${ADMIN_API_KEY}" \
  -d "{\"path\": \"${BACKUP_DIR}/rocksdb\"}" || {
  echo "Failed to create database checkpoint"
  exit 1
}

# 2. Backup vector indices
echo "Backing up vector indices..."
if [ -d "${DATA_DIR}/vector" ]; then
  cp -r "${DATA_DIR}/vector" "${BACKUP_DIR}/vector"
fi

# 3. Backup configuration
echo "Backing up configuration..."  
cp -r "${CONFIG_DIR}" "${BACKUP_DIR}/config"

# 4. Create metadata file
echo "Creating backup metadata..."
cat > "${BACKUP_DIR}/metadata.json" << EOF
{
  "timestamp": "${TIMESTAMP}",
  "version": "$(curl -s http://localhost:8000/health | jq -r .version)",
  "backup_type": "full",
  "size": "$(du -sh ${BACKUP_DIR} | cut -f1)",
  "files": $(find "${BACKUP_DIR}" -type f | wc -l)
}
EOF

# 5. Compress backup
echo "Compressing backup..."
tar -czf "${BACKUP_ROOT}/codegraph_${TIMESTAMP}.tar.gz" \
  -C "${BACKUP_ROOT}" "${TIMESTAMP}"

# Remove uncompressed backup
rm -rf "${BACKUP_DIR}"

# 6. Verify backup
echo "Verifying backup..."
if tar -tzf "${BACKUP_ROOT}/codegraph_${TIMESTAMP}.tar.gz" > /dev/null; then
  echo "Backup verification successful"
else
  echo "Backup verification failed"
  exit 1
fi

# 7. Clean old backups
echo "Cleaning old backups..."
find "${BACKUP_ROOT}" -name "codegraph_*.tar.gz" \
  -mtime +${RETENTION_DAYS} -delete

# 8. Upload to cloud storage (optional)
if [ "${CLOUD_BACKUP:-}" = "true" ]; then
  echo "Uploading to cloud storage..."
  aws s3 cp "${BACKUP_ROOT}/codegraph_${TIMESTAMP}.tar.gz" \
    "s3://${S3_BACKUP_BUCKET}/codegraph/backups/"
fi

echo "Backup completed successfully at $(date)"
echo "Backup file: codegraph_${TIMESTAMP}.tar.gz"
```

### Recovery Procedures

**Full Recovery Script**:
```bash
#!/bin/bash
# restore_backup.sh

set -euo pipefail

BACKUP_FILE="$1"
RESTORE_DIR="/app/restore"
DATA_DIR="/app/data"
CONFIG_DIR="/app/config"

if [ $# -ne 1 ]; then
  echo "Usage: $0 <backup_file.tar.gz>"
  exit 1
fi

echo "Starting recovery from ${BACKUP_FILE}"

# 1. Stop CodeGraph service
echo "Stopping CodeGraph service..."
systemctl stop codegraph-api || docker stop codegraph-api

# 2. Create restore directory
mkdir -p "${RESTORE_DIR}"

# 3. Extract backup
echo "Extracting backup..."
tar -xzf "${BACKUP_FILE}" -C "${RESTORE_DIR}"

# Find backup directory (timestamped)
BACKUP_TIMESTAMP=$(basename "${BACKUP_FILE}" .tar.gz | sed 's/codegraph_//')
BACKUP_EXTRACT_DIR="${RESTORE_DIR}/${BACKUP_TIMESTAMP}"

if [ ! -d "${BACKUP_EXTRACT_DIR}" ]; then
  echo "Backup directory not found in archive"
  exit 1
fi

# 4. Backup current data (safety)
if [ -d "${DATA_DIR}" ]; then
  echo "Backing up current data..."
  mv "${DATA_DIR}" "${DATA_DIR}.backup.$(date +%s)"
fi

# 5. Restore database
echo "Restoring database..."
mv "${BACKUP_EXTRACT_DIR}/rocksdb" "${DATA_DIR}"

# 6. Restore vector indices
if [ -d "${BACKUP_EXTRACT_DIR}/vector" ]; then
  echo "Restoring vector indices..."
  mv "${BACKUP_EXTRACT_DIR}/vector" "${DATA_DIR}/vector"
fi

# 7. Restore configuration
echo "Restoring configuration..."
if [ -d "${CONFIG_DIR}.backup" ]; then
  rm -rf "${CONFIG_DIR}.backup"
fi
mv "${CONFIG_DIR}" "${CONFIG_DIR}.backup" 2>/dev/null || true
mv "${BACKUP_EXTRACT_DIR}/config" "${CONFIG_DIR}"

# 8. Set correct ownership
chown -R codegraph:codegraph "${DATA_DIR}" "${CONFIG_DIR}"

# 9. Start service
echo "Starting CodeGraph service..."
systemctl start codegraph-api || docker start codegraph-api

# 10. Verify recovery
echo "Verifying recovery..."
sleep 10
if curl -f http://localhost:8000/health > /dev/null 2>&1; then
  echo "Recovery successful - service is healthy"
else
  echo "Recovery may have failed - service health check failed"
  exit 1
fi

# Cleanup
rm -rf "${RESTORE_DIR}"

echo "Recovery completed successfully"
```

## Production Checklist

### Pre-deployment Checklist

- [ ] **Infrastructure**
  - [ ] Hardware requirements verified
  - [ ] Network configuration tested
  - [ ] Storage provisioned and tested
  - [ ] Load balancer configured

- [ ] **Security**
  - [ ] API keys generated and stored securely
  - [ ] TLS certificates installed and tested
  - [ ] Firewall rules configured
  - [ ] Security scanning completed

- [ ] **Configuration**
  - [ ] Production configuration reviewed
  - [ ] Environment variables set
  - [ ] Resource limits configured
  - [ ] Backup strategy implemented

- [ ] **Monitoring**
  - [ ] Prometheus metrics configured
  - [ ] Alerting rules defined
  - [ ] Grafana dashboards imported
  - [ ] Log aggregation setup

### Post-deployment Verification

```bash
#!/bin/bash
# deployment_verification.sh

echo "=== CodeGraph Deployment Verification ==="

# 1. Health check
echo "1. Checking API health..."
curl -f http://localhost:8000/health || exit 1

# 2. Database connection
echo "2. Verifying database..."
curl -f http://localhost:8000/api/v1/health | jq '.components.database.status' || exit 1

# 3. Vector search
echo "3. Testing vector search..."
curl -f http://localhost:8000/api/v1/health | jq '.components.vector_search.status' || exit 1

# 4. Metrics endpoint
echo "4. Checking metrics..."
curl -f http://localhost:8000/metrics > /dev/null || exit 1

# 5. Performance test
echo "5. Running performance test..."
ab -n 100 -c 10 http://localhost:8000/health > /dev/null || exit 1

echo "All verification checks passed!"
```

### Troubleshooting Guide

**Common Issues**:

1. **Service won't start**
   - Check logs: `docker logs codegraph-api`
   - Verify configuration: `codegraph-api --check-config`
   - Check permissions: `ls -la /app/data`

2. **High memory usage**
   - Adjust cache size: `CODEGRAPH_DATABASE_CACHE_SIZE=1024`
   - Monitor with: `docker stats codegraph-api`

3. **Slow queries**
   - Check database stats: `curl localhost:8000/api/v1/admin/stats`
   - Optimize RocksDB settings
   - Consider adding read replicas

4. **Backup failures**
   - Verify disk space: `df -h /app/backups`
   - Check permissions: `ls -la /app/backups`
   - Monitor backup logs

For additional troubleshooting, see the [Operations Manual](../operations/).