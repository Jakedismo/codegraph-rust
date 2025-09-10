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

# CodeGraph Operations Runbook

## Table of Contents

1. [Daily Operations](#daily-operations)
2. [Monitoring and Alerting](#monitoring-and-alerting)
3. [Performance Management](#performance-management)
4. [Backup and Recovery](#backup-and-recovery)
5. [Incident Response](#incident-response)
6. [Maintenance Procedures](#maintenance-procedures)
7. [Capacity Planning](#capacity-planning)
8. [Security Operations](#security-operations)
9. [Troubleshooting Procedures](#troubleshooting-procedures)
10. [Emergency Procedures](#emergency-procedures)

## Daily Operations

### Morning Health Check

**Frequency**: Daily at 8:00 AM
**Owner**: SRE Team
**Duration**: 15 minutes

#### Checklist

```bash
#!/bin/bash
# Daily health check script

echo "=== CodeGraph Daily Health Check ==="
date

# 1. Service Status
echo "1. Checking service status..."
systemctl status codegraph
if [ $? -eq 0 ]; then
    echo "✓ Service is running"
else
    echo "✗ Service is not running - ALERT"
    exit 1
fi

# 2. API Health
echo "2. Checking API health..."
response=$(curl -s -o /dev/null -w "%{http_code}" https://localhost:8080/health)
if [ "$response" = "200" ]; then
    echo "✓ API is healthy"
else
    echo "✗ API health check failed (HTTP $response) - ALERT"
fi

# 3. Database Connectivity
echo "3. Checking database connectivity..."
curl -s -H "Authorization: Bearer $API_KEY" \
     https://localhost:8080/stats/rocksdb > /dev/null
if [ $? -eq 0 ]; then
    echo "✓ Database is accessible"
else
    echo "✗ Database connectivity issue - ALERT"
fi

# 4. Disk Space
echo "4. Checking disk space..."
df -h /opt/codegraph/data | awk 'NR==2 {
    if (substr($5,1,length($5)-1) > 80) {
        print "✗ Disk usage is " $5 " - WARNING";
    } else {
        print "✓ Disk usage is " $5;
    }
}'

# 5. Memory Usage
echo "5. Checking memory usage..."
free -h | awk 'NR==2 {
    used = substr($3,1,length($3)-1);
    total = substr($2,1,length($2)-1);
    percent = (used/total)*100;
    if (percent > 85) {
        print "✗ Memory usage is " percent "% - WARNING";
    } else {
        print "✓ Memory usage is " percent "%";
    }
}'

# 6. Log Errors
echo "6. Checking for errors in logs..."
error_count=$(journalctl -u codegraph --since "1 hour ago" | grep -i error | wc -l)
if [ "$error_count" -gt 10 ]; then
    echo "✗ Found $error_count errors in last hour - WARNING"
else
    echo "✓ Error count: $error_count"
fi

echo "=== Health check completed ==="
```

### Performance Metrics Review

**Frequency**: Daily at 10:00 AM and 6:00 PM
**Owner**: DevOps Team

#### Key Metrics to Review

1. **Response Times**
   ```bash
   # Check average response times
   curl -s https://localhost:9090/metrics | grep http_request_duration_seconds
   ```

2. **Throughput**
   ```bash
   # Check request rate
   curl -s https://localhost:9090/metrics | grep http_requests_total
   ```

3. **Error Rates**
   ```bash
   # Check error rates
   curl -s https://localhost:9090/metrics | grep http_requests_total | grep -v '="200"'
   ```

4. **Resource Utilization**
   ```bash
   # CPU and Memory
   top -bn1 | grep codegraph
   
   # Disk I/O
   iostat -x 1 3
   ```

### Log Rotation and Cleanup

**Frequency**: Daily at 2:00 AM
**Owner**: System Administrator

```bash
#!/bin/bash
# Log cleanup script

# Compress logs older than 1 day
find /opt/codegraph/logs -name "*.log" -mtime +1 -exec gzip {} \;

# Delete compressed logs older than 30 days
find /opt/codegraph/logs -name "*.log.gz" -mtime +30 -delete

# Clean up temporary files
find /tmp -name "codegraph-*" -mtime +1 -delete

# Rotate RocksDB LOG files if they're too large
rocksdb_log="/opt/codegraph/data/rocksdb/LOG"
if [ -f "$rocksdb_log" ] && [ $(stat -c%s "$rocksdb_log") -gt 104857600 ]; then
    mv "$rocksdb_log" "${rocksdb_log}.$(date +%Y%m%d)"
    systemctl reload codegraph
fi
```

## Monitoring and Alerting

### Prometheus Metrics Collection

#### Key Metrics to Monitor

**Application Metrics:**
```
# Request metrics
http_requests_total
http_request_duration_seconds
http_requests_concurrent

# Business metrics
codegraph_nodes_total
codegraph_search_queries_total
codegraph_parse_operations_total

# Performance metrics
codegraph_vector_search_duration_seconds
codegraph_rocksdb_compaction_duration_seconds
codegraph_memory_usage_bytes
```

**System Metrics:**
```
# CPU and Memory
node_cpu_seconds_total
node_memory_MemAvailable_bytes
node_memory_MemTotal_bytes

# Disk I/O
node_disk_io_time_seconds_total
node_disk_reads_completed_total
node_disk_writes_completed_total

# Network
node_network_receive_bytes_total
node_network_transmit_bytes_total
```

### Alert Rules

#### Critical Alerts

**Service Down Alert:**
```yaml
- alert: CodeGraphServiceDown
  expr: up{job="codegraph"} == 0
  for: 1m
  labels:
    severity: critical
  annotations:
    summary: "CodeGraph service is down"
    description: "CodeGraph service has been down for more than 1 minute"
```

**High Error Rate Alert:**
```yaml
- alert: CodeGraphHighErrorRate
  expr: rate(http_requests_total{job="codegraph",code!~"2.."}[5m]) / rate(http_requests_total{job="codegraph"}[5m]) > 0.1
  for: 5m
  labels:
    severity: critical
  annotations:
    summary: "High error rate detected"
    description: "Error rate is {{ $value | humanizePercentage }}"
```

#### Warning Alerts

**High Response Time Alert:**
```yaml
- alert: CodeGraphHighResponseTime
  expr: histogram_quantile(0.95, rate(http_request_duration_seconds_bucket{job="codegraph"}[5m])) > 2
  for: 10m
  labels:
    severity: warning
  annotations:
    summary: "High response time"
    description: "95th percentile response time is {{ $value }}s"
```

**High Memory Usage Alert:**
```yaml
- alert: CodeGraphHighMemoryUsage
  expr: (node_memory_MemTotal_bytes - node_memory_MemAvailable_bytes) / node_memory_MemTotal_bytes > 0.85
  for: 15m
  labels:
    severity: warning
  annotations:
    summary: "High memory usage"
    description: "Memory usage is {{ $value | humanizePercentage }}"
```

### Grafana Dashboards

#### Main Dashboard Panels

1. **Service Overview**
   - Service status (up/down)
   - Request rate
   - Error rate
   - Response time percentiles

2. **Performance Metrics**
   - CPU usage
   - Memory usage
   - Disk I/O
   - Network I/O

3. **Business Metrics**
   - Nodes created/updated
   - Search queries
   - Parse operations
   - Vector searches

4. **Database Metrics**
   - RocksDB compaction stats
   - Cache hit rates
   - Write amplification
   - Read amplification

## Performance Management

### Performance Monitoring

#### Real-time Performance Check

```bash
#!/bin/bash
# Performance monitoring script

echo "=== CodeGraph Performance Check ==="

# API Response Times
echo "API Response Times:"
for endpoint in /health /search /nodes; do
    time=$(curl -o /dev/null -s -w "%{time_total}" https://localhost:8080$endpoint)
    echo "  $endpoint: ${time}s"
done

# Database Performance
echo "Database Performance:"
curl -s -H "Authorization: Bearer $API_KEY" \
     https://localhost:8080/stats/rocksdb | jq '.compaction_stats'

# Vector Search Performance
echo "Vector Search Performance:"
curl -s -H "Authorization: Bearer $API_KEY" \
     https://localhost:8080/vector/performance

# System Resources
echo "System Resources:"
echo "  CPU: $(top -bn1 | grep "Cpu(s)" | awk '{print $2}' | sed 's/%us,//')"
echo "  Memory: $(free | grep Mem | awk '{printf "%.1f%%", $3/$2 * 100.0}')"
echo "  Disk I/O: $(iostat -x 1 1 | awk '/Device/ { getline; print $4 " reads/s, " $5 " writes/s" }')"
```

### Performance Tuning

#### Database Optimization

**RocksDB Tuning for Read-Heavy Workloads:**
```toml
[database]
# Increase block cache
block_cache_size = 4294967296  # 4GB

# Optimize for reads
cache_index_and_filter_blocks = true
pin_l0_filter_and_index_blocks_in_cache = true
pin_top_level_index_and_filter = true

# Bloom filter optimization
optimize_filters_for_memory = true
bloom_locality = 1

# Compaction tuning
level0_file_num_compaction_trigger = 4
max_bytes_for_level_base = 1073741824  # 1GB
target_file_size_base = 134217728  # 128MB
```

**RocksDB Tuning for Write-Heavy Workloads:**
```toml
[database]
# Increase write buffers
write_buffer_size = 268435456  # 256MB
max_write_buffer_number = 6
min_write_buffer_number_to_merge = 2

# Optimize compaction
max_background_jobs = 8
level0_file_num_compaction_trigger = 8
level0_slowdown_writes_trigger = 16
level0_stop_writes_trigger = 24

# Compression for lower levels
compression_per_level = ["none", "none", "lz4", "lz4", "zstd", "zstd"]
```

#### Vector Search Optimization

```bash
# Check current index performance
curl -H "Authorization: Bearer $API_KEY" \
     https://localhost:8080/vector/index/stats

# Rebuild index with optimal parameters
curl -X POST \
     -H "Authorization: Bearer $API_KEY" \
     -H "Content-Type: application/json" \
     -d '{
       "index_type": "HNSW",
       "parameters": {
         "M": 16,
         "efConstruction": 200,
         "efSearch": 100
       }
     }' \
     https://localhost:8080/vector/index/rebuild
```

### Capacity Management

#### Resource Usage Tracking

```bash
#!/bin/bash
# Resource usage tracking script

# Database size growth
db_size=$(du -sh /opt/codegraph/data/rocksdb | cut -f1)
echo "Database size: $db_size"

# Node count growth
node_count=$(curl -s -H "Authorization: Bearer $API_KEY" \
             https://localhost:8080/stats | jq '.total_nodes')
echo "Total nodes: $node_count"

# Daily growth rate
echo "Growth tracking:" > /var/log/codegraph-growth.log
echo "$(date): DB=$db_size, Nodes=$node_count" >> /var/log/codegraph-growth.log
```

## Backup and Recovery

### Backup Procedures

#### Daily Backup Script

```bash
#!/bin/bash
# Daily backup script

BACKUP_DIR="/opt/codegraph/backups"
DATE=$(date +%Y%m%d)
BACKUP_PATH="$BACKUP_DIR/codegraph-backup-$DATE"

echo "Starting backup at $(date)"

# Create backup directory
mkdir -p "$BACKUP_PATH"

# 1. Create application backup via API
echo "Creating application backup..."
backup_id=$(curl -s -X POST \
            -H "Authorization: Bearer $API_KEY" \
            https://localhost:8080/backup | jq -r '.backup_id')

if [ "$backup_id" != "null" ]; then
    echo "Application backup created: $backup_id"
    echo "$backup_id" > "$BACKUP_PATH/backup_id.txt"
else
    echo "Failed to create application backup"
    exit 1
fi

# 2. Backup configuration files
echo "Backing up configuration..."
cp -r /opt/codegraph/config "$BACKUP_PATH/"

# 3. Export database metadata
echo "Exporting database metadata..."
curl -s -H "Authorization: Bearer $API_KEY" \
     https://localhost:8080/stats/rocksdb > "$BACKUP_PATH/db_stats.json"

# 4. Backup logs
echo "Backing up recent logs..."
journalctl -u codegraph --since "24 hours ago" > "$BACKUP_PATH/service.log"
cp /opt/codegraph/logs/codegraph.log "$BACKUP_PATH/" 2>/dev/null || true

# 5. Create checksum
echo "Creating checksums..."
find "$BACKUP_PATH" -type f -exec sha256sum {} \; > "$BACKUP_PATH/checksums.sha256"

# 6. Compress backup
echo "Compressing backup..."
tar -czf "$BACKUP_DIR/codegraph-backup-$DATE.tar.gz" -C "$BACKUP_DIR" "codegraph-backup-$DATE"
rm -rf "$BACKUP_PATH"

# 7. Clean old backups (keep 30 days)
find "$BACKUP_DIR" -name "codegraph-backup-*.tar.gz" -mtime +30 -delete

echo "Backup completed at $(date)"
```

### Recovery Procedures

#### Full System Recovery

```bash
#!/bin/bash
# Full system recovery procedure

if [ $# -ne 1 ]; then
    echo "Usage: $0 <backup_id>"
    exit 1
fi

BACKUP_ID=$1
BACKUP_DIR="/opt/codegraph/backups"

echo "Starting recovery from backup: $BACKUP_ID"

# 1. Stop the service
echo "Stopping CodeGraph service..."
systemctl stop codegraph

# 2. Backup current state
echo "Backing up current state..."
mv /opt/codegraph/data /opt/codegraph/data.pre-recovery.$(date +%Y%m%d_%H%M%S)

# 3. Extract backup
echo "Extracting backup..."
cd "$BACKUP_DIR"
tar -xzf "codegraph-backup-$BACKUP_ID.tar.gz"

# 4. Restore configuration
echo "Restoring configuration..."
cp -r "codegraph-backup-$BACKUP_ID/config/"* /opt/codegraph/config/

# 5. Restore from application backup
echo "Restoring from application backup..."
backup_id=$(cat "codegraph-backup-$BACKUP_ID/backup_id.txt")
curl -X POST \
     -H "Authorization: Bearer $API_KEY" \
     -H "Content-Type: application/json" \
     -d "{\"backup_id\": \"$backup_id\"}" \
     https://localhost:8080/backup/restore

# 6. Set permissions
echo "Setting permissions..."
chown -R codegraph:codegraph /opt/codegraph/

# 7. Start the service
echo "Starting CodeGraph service..."
systemctl start codegraph

# 8. Verify recovery
sleep 10
if systemctl is-active --quiet codegraph; then
    echo "Recovery successful - service is running"
else
    echo "Recovery failed - service is not running"
    exit 1
fi

# 9. Run integrity check
echo "Running integrity check..."
curl -X POST -H "Authorization: Bearer $API_KEY" \
     https://localhost:8080/integrity/check

echo "Recovery completed successfully"
```

#### Point-in-Time Recovery

```bash
#!/bin/bash
# Point-in-time recovery procedure

RECOVERY_TIME=$1
if [ -z "$RECOVERY_TIME" ]; then
    echo "Usage: $0 <YYYY-MM-DD HH:MM:SS>"
    exit 1
fi

echo "Starting point-in-time recovery to: $RECOVERY_TIME"

# 1. Find appropriate backup
latest_backup=$(find /opt/codegraph/backups -name "*.tar.gz" -newer /tmp/recovery_time -exec basename {} \; | sort -r | head -1)

if [ -z "$latest_backup" ]; then
    echo "No suitable backup found for recovery time: $RECOVERY_TIME"
    exit 1
fi

echo "Using backup: $latest_backup"

# 2. Perform base recovery
./full_recovery.sh "${latest_backup%%.tar.gz}"

# 3. Apply transaction logs up to recovery point
echo "Applying transaction logs..."
curl -X POST \
     -H "Authorization: Bearer $API_KEY" \
     -H "Content-Type: application/json" \
     -d "{\"recovery_time\": \"$RECOVERY_TIME\"}" \
     https://localhost:8080/recovery/point-in-time

echo "Point-in-time recovery completed"
```

## Incident Response

### Incident Classification

#### Severity Levels

**P0 - Critical**
- Service completely down
- Data loss detected
- Security breach

**P1 - High**
- Major functionality unavailable
- Significant performance degradation
- Elevated error rates (>5%)

**P2 - Medium**
- Minor functionality issues
- Moderate performance impact
- Error rates 1-5%

**P3 - Low**
- Cosmetic issues
- Minimal performance impact
- Documentation updates needed

### Incident Response Procedures

#### P0 Incident Response

```bash
#!/bin/bash
# P0 Incident Response Checklist

echo "=== P0 INCIDENT RESPONSE ==="
echo "Incident started at: $(date)"

# 1. Immediate Assessment
echo "1. IMMEDIATE ASSESSMENT"
echo "   - Service status: $(systemctl is-active codegraph)"
echo "   - API status: $(curl -s -o /dev/null -w "%{http_code}" https://localhost:8080/health)"
echo "   - Last successful backup: $(ls -lt /opt/codegraph/backups/*.tar.gz | head -1)"

# 2. Escalation
echo "2. ESCALATION"
echo "   - Notify on-call engineer"
echo "   - Create incident ticket"
echo "   - Notify stakeholders"

# 3. Mitigation Steps
echo "3. MITIGATION STEPS"
echo "   a. Check recent changes"
git -C /opt/codegraph/src log --oneline -5

echo "   b. Check system resources"
df -h /opt/codegraph/data
free -h
top -bn1 | head -20

echo "   c. Check logs for errors"
journalctl -u codegraph --since "1 hour ago" | grep -i error | tail -10

# 4. Recovery Actions
echo "4. RECOVERY ACTIONS (if needed)"
echo "   - Restart service: systemctl restart codegraph"
echo "   - Rollback deployment: ./rollback.sh"
echo "   - Restore from backup: ./restore_backup.sh [backup_id]"

echo "=== END P0 INCIDENT RESPONSE ==="
```

#### Communication Templates

**Initial Alert:**
```
🚨 P0 INCIDENT: CodeGraph Service Down

Status: Investigating
Impact: All API endpoints unavailable
Started: [timestamp]
Duration: [duration]

Actions Taken:
- Investigating root cause
- Checking system resources
- Preparing rollback if needed

Next Update: In 15 minutes
```

**Resolution Notice:**
```
✅ RESOLVED: CodeGraph Service Restored

Status: Resolved
Root Cause: [brief description]
Duration: [total duration]

Resolution:
- [action taken]
- [verification steps]

Post-Incident: Full post-mortem will be conducted
```

## Maintenance Procedures

### Planned Maintenance

#### Monthly Maintenance Window

**Schedule**: First Sunday of each month, 2:00 AM - 6:00 AM
**Owner**: SRE Team

```bash
#!/bin/bash
# Monthly maintenance procedure

echo "=== MONTHLY MAINTENANCE START ==="
echo "Started at: $(date)"

# 1. Pre-maintenance backup
echo "1. Creating pre-maintenance backup..."
./backup.sh

# 2. Update system packages
echo "2. Updating system packages..."
apt update && apt upgrade -y

# 3. Database maintenance
echo "3. Running database maintenance..."
systemctl stop codegraph

# Compact database
curl -X POST -H "Authorization: Bearer $API_KEY" \
     https://localhost:8080/maintenance/compact

# Update statistics
curl -X POST -H "Authorization: Bearer $API_KEY" \
     https://localhost:8080/maintenance/analyze

systemctl start codegraph

# 4. Log rotation
echo "4. Rotating logs..."
logrotate -f /etc/logrotate.d/codegraph

# 5. Clean temporary files
echo "5. Cleaning temporary files..."
find /tmp -name "codegraph-*" -mtime +7 -delete

# 6. Update certificates (if needed)
echo "6. Checking SSL certificates..."
cert_expiry=$(openssl x509 -in /opt/codegraph/config/server.crt -noout -enddate | cut -d= -f2)
echo "Certificate expires: $cert_expiry"

# 7. Performance optimization
echo "7. Running performance optimization..."
curl -X POST -H "Authorization: Bearer $API_KEY" \
     https://localhost:8080/optimization/auto-tune

# 8. Verify system health
echo "8. Verifying system health..."
./health_check.sh

echo "=== MONTHLY MAINTENANCE COMPLETE ==="
echo "Completed at: $(date)"
```

### Emergency Maintenance

#### Hotfix Deployment

```bash
#!/bin/bash
# Emergency hotfix deployment

if [ $# -ne 1 ]; then
    echo "Usage: $0 <hotfix_version>"
    exit 1
fi

HOTFIX_VERSION=$1

echo "=== EMERGENCY HOTFIX DEPLOYMENT ==="
echo "Deploying hotfix: $HOTFIX_VERSION"

# 1. Create emergency backup
echo "1. Creating emergency backup..."
./backup.sh emergency-$HOTFIX_VERSION

# 2. Download hotfix
echo "2. Downloading hotfix..."
wget -O /tmp/codegraph-$HOTFIX_VERSION.tar.gz \
     "https://releases.codegraph.com/$HOTFIX_VERSION/codegraph-linux-x86_64.tar.gz"

# 3. Verify checksum
echo "3. Verifying checksum..."
cd /tmp
sha256sum -c codegraph-$HOTFIX_VERSION.sha256 || exit 1

# 4. Stop service
echo "4. Stopping service..."
systemctl stop codegraph

# 5. Backup current binary
echo "5. Backing up current binary..."
cp /opt/codegraph/bin/codegraph-api /opt/codegraph/bin/codegraph-api.backup

# 6. Install hotfix
echo "6. Installing hotfix..."
tar -xzf codegraph-$HOTFIX_VERSION.tar.gz
cp codegraph-api /opt/codegraph/bin/
chown codegraph:codegraph /opt/codegraph/bin/codegraph-api
chmod +x /opt/codegraph/bin/codegraph-api

# 7. Start service
echo "7. Starting service..."
systemctl start codegraph

# 8. Verify deployment
echo "8. Verifying deployment..."
sleep 10
if systemctl is-active --quiet codegraph; then
    echo "Hotfix deployment successful"
    # Test API
    curl -f https://localhost:8080/health > /dev/null
    if [ $? -eq 0 ]; then
        echo "API is responding correctly"
    else
        echo "API health check failed - rolling back"
        systemctl stop codegraph
        cp /opt/codegraph/bin/codegraph-api.backup /opt/codegraph/bin/codegraph-api
        systemctl start codegraph
        exit 1
    fi
else
    echo "Hotfix deployment failed - rolling back"
    cp /opt/codegraph/bin/codegraph-api.backup /opt/codegraph/bin/codegraph-api
    systemctl start codegraph
    exit 1
fi

echo "=== HOTFIX DEPLOYMENT COMPLETE ==="
```

## Security Operations

### Security Monitoring

#### Daily Security Check

```bash
#!/bin/bash
# Daily security check

echo "=== DAILY SECURITY CHECK ==="

# 1. Check for failed authentication attempts
echo "1. Failed authentication attempts (last 24h):"
journalctl -u codegraph --since "24 hours ago" | grep -i "authentication failed" | wc -l

# 2. Check for suspicious API calls
echo "2. Suspicious API calls:"
journalctl -u codegraph --since "24 hours ago" | grep -E "(401|403)" | tail -5

# 3. Check SSL certificate validity
echo "3. SSL certificate status:"
openssl x509 -in /opt/codegraph/config/server.crt -noout -dates

# 4. Check for security updates
echo "4. Available security updates:"
apt list --upgradable 2>/dev/null | grep -i security

# 5. Check file permissions
echo "5. Checking file permissions:"
ls -la /opt/codegraph/config/
ls -la /opt/codegraph/bin/

# 6. Check for unusual network connections
echo "6. Active network connections:"
netstat -tulpn | grep :8080
```

### Access Management

#### API Key Rotation

```bash
#!/bin/bash
# API key rotation procedure

echo "=== API KEY ROTATION ==="

# 1. Generate new API key
NEW_API_KEY=$(openssl rand -base64 32)
echo "Generated new API key: $NEW_API_KEY"

# 2. Update configuration
echo "2. Updating configuration..."
sed -i "s/CODEGRAPH_API_KEY=.*/CODEGRAPH_API_KEY=$NEW_API_KEY/" /opt/codegraph/config/.env

# 3. Reload service configuration
echo "3. Reloading service..."
systemctl reload codegraph

# 4. Test new key
echo "4. Testing new API key..."
response=$(curl -s -H "Authorization: Bearer $NEW_API_KEY" \
           https://localhost:8080/health)
if echo "$response" | grep -q "healthy"; then
    echo "New API key is working"
else
    echo "New API key test failed"
    exit 1
fi

# 5. Notify stakeholders
echo "5. Update complete. Notify all API consumers of new key."
echo "New API key: $NEW_API_KEY"
```

For additional troubleshooting information, see the [Troubleshooting Guide](TROUBLESHOOTING_GUIDE.md).

For architectural details, see the [Architecture Documentation](ARCHITECTURE_DOCUMENTATION.md).