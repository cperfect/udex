---
verblock: "24 Jun 2026:v0.1: vscode - Initial version"
wp_id: WP-05
title: "K8s deployment with observability"
scope: Small
status: Done
---

# WP-05: K8s deployment with observability

## Objective

Enable observability in the k8s dev deployment with full trace + metric sampling:
run an OpenTelemetry Collector in-cluster and configure the app pods to export
telemetry to it.

## Deliverables

- OTel Collector deployed in-cluster (Helm template(s) + values): OTLP receivers,
  `postgresqlreceiver` pointed at the Compose Postgres via `host.k3d.internal`
  (matching the existing datastore/authz pattern), exporters to the dev backends.
- Decision + implementation for where Tempo/Prometheus/Loki/Grafana live for the
  k8s path (reuse the local stack vs in-cluster) - recorded in `impl.md`.
- Helm `values.yaml` + `configmap`/`secret`/`deployment` updates so app pods get
  `observability.enabled=true`, the in-cluster OTLP endpoint, the OTLP CA, and
  `sample_ratio: 1.0` (full sampling).
- OTLP cert material wired into the chart (same `--set-file` pattern as the
  existing certs in `deploy.sh`).

## Acceptance Criteria

- [x] After `deploy.sh`, app pods start with telemetry enabled and export to the
  Collector (no crashloop; pods Running 1/1; logs show "OpenTelemetry initialised"
  endpoint `https://host.k3d.internal:4317`, full sampling).
- [x] Traces and metrics from cluster traffic reach the backends with full
  sampling; PostgreSQL receiver metrics present (WP01).
- [x] Works with the default 2-replica deployment (8/8 `test_sdk_k8s*` pass,
  including the multi-instance suite).
- [x] Helm chart renders cleanly (`helm template`).

## As-built notes

- **Decision: reuse the local stack, no in-cluster Collector.** Rather than
  running a second Collector inside k3d, the app pods export OTLP directly to the
  **local-stack Collector** (`projects/observability`) published on the host,
  reached from pods via `host.k3d.internal:4317` - the same bridge the chart
  already uses for postgres (`:5432`) and Hydra JWKS (`:4444`). The local
  Collector already scrapes PostgreSQL metrics and fans out to Tempo/Prometheus/
  Loki, so a second collector would be pure duplication. Tempo/Prometheus/Loki/
  Grafana therefore remain the local stack for the k8s path too.
- **Chart changes**: `values.yaml` gains an `observability` block (enabled,
  `otlpEndpoint=https://host.k3d.internal:4317`, `sampleRatio=1.0`,
  `deploymentEnvironment=k3d`) and `secrets.otlpCa`. `configmap.yaml` renders
  `server.observability` (with `otlp_ca: /etc/udex/otel/ca.crt` and a
  `deployment.environment` resource attribute) when enabled. `secret.yaml` adds
  `otlp-ca.crt` (required when enabled); `deployment.yaml` mounts it at
  `/etc/udex/otel/ca.crt`. `deploy.sh` passes `--set-file secrets.otlpCa`.
- **OTLP cert SAN**: added `host.k3d.internal` to
  `projects/observability/certs/regenerate_certs.sh` so the pods' TLS verification
  of the Collector cert succeeds.
- **Test harness**: the k8s integration `redeploy_k8s_server` helm invocation now
  also passes `--set-file secrets.otlpCa` (the chart requires it when enabled).
- **Verified live**: deployed (rev 9, rolling update, both replicas Running).
  Cluster traffic from the k8s suite produced 20+ `deployment.environment=k3d`
  traces in Tempo (e.g. `/udex.index.v1.IndexService/Describe -> db.get_index`,
  connected request->datastore) and 19 k3d-labelled `udex_rpc_requests_total`
  series in Prometheus. The k8s deployment is left observability-enabled (the dev
  default); bring up `projects/observability/scripts/up.sh` to view the data.

## Dependencies

- WP02 + WP03 (server must emit telemetry and deep instrumentation). Reuses the
  ST0024/ST0025 k8s cert + multi-replica patterns.
