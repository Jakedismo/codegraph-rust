# Monitoring Setup

This document describes how to run the monitoring stack for the CodeGraph application.

## Prerequisites

- Docker
- Docker Compose

## Running the Monitoring Stack

1.  Start the Docker daemon.
2.  Run the following command from the root of the project:

    ```bash
    docker-compose up -d --build
    ```

This will start the following services:

-   `codegraph-api`: The main application.
-   `prometheus`: The Prometheus server.
-   `grafana`: The Grafana server.
-   `alertmanager`: The Alertmanager server.

## Accessing the Services

-   **Prometheus**: http://localhost:9090
-   **Grafana**: http://localhost:3001
-   **Alertmanager**: http://localhost:9093

## Metrics

The application exposes the following metrics on the `/metrics` endpoint:

-   `sync_operations_total`: A counter for the total number of sync operations.
-   `sync_operation_duration_seconds`: A histogram for the duration of sync operations.

## Dashboards

A pre-configured Grafana dashboard is available. To import it:

1.  Go to the Grafana UI at http://localhost:3001.
2.  Log in with the default credentials (admin/admin).
3.  Go to Dashboards -> Import.
4.  Upload the `grafana-dashboard.json` file.

## Alerts

The following alerts are configured in `prometheus.rules.yml`:

-   `SyncJobFailed`: Fires when a sync job fails.
-   `SyncJobSlow`: Fires when the 95th percentile of sync job duration is over 1 hour.
