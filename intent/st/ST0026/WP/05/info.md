---
verblock: "24 Jun 2026:v0.1: vscode - Initial version"
wp_id: WP-05
title: "K8s deployment with observability"
scope: Small
status: Not Started
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

- [ ] After `deploy.sh`, app pods start with telemetry enabled and export to the
  in-cluster Collector (no crashloop; health stays SERVING).
- [ ] Traces and metrics from cluster traffic reach the backends with full
  sampling; PostgreSQL receiver metrics present.
- [ ] Works with the default 2-replica deployment (spans from either pod land).
- [ ] Helm chart renders cleanly (`helm template`) and passes any existing
  misconfig scans.

## Dependencies

- WP02 + WP03 (server must emit telemetry and deep instrumentation). Reuses the
  ST0024/ST0025 k8s cert + multi-replica patterns.
