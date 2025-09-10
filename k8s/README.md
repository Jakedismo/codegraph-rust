Kubernetes deployment for CodeGraph API

Contents
- `namespace.yaml`: Dedicated `codegraph` namespace
- `serviceaccount.yaml`: ServiceAccount for pods
- `configmap.yaml`: Non-secret configuration
- `secret.yaml`: Optional secrets (e.g., `OPENAI_API_KEY`)
- `deployment.yaml`: RollingUpdate, probes, resources, anti-affinity
- `service.yaml`: ClusterIP service on port 80 to container 3000
- `hpa.yaml`: CPU+Memory based autoscaling (2-10 replicas)
- `pdb.yaml`: Ensures at least 1 pod remains during disruptions
- `kustomization.yaml`: Apply all with Kustomize

Prerequisites
- Build and push the container image, then set `image` in `deployment.yaml`.
- Ensure `metrics-server` is installed in the cluster for HPA.

Apply
```
kubectl apply -k k8s/
```

Zero-downtime notes
- RollingUpdate with `maxUnavailable=0` and readiness probe protects availability.
- `preStop` delay and `minReadySeconds` help drain connections before termination.
- `PodDisruptionBudget` avoids voluntary disruptions evicting all pods.

Configuration
- Logging via `RUST_LOG` in `configmap.yaml`.
- HTTP client pool tuning via env in `configmap.yaml`.
- Optional `OPENAI_API_KEY` via `secret.yaml` (leave empty or set via your secrets mechanism).

