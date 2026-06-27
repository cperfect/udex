# Tasks - ST0026: Open Observability

## Work Packages

- [x] **WP01 - Observability runtime stack.** `projects/observability/`: compose
  for Collector, Tempo, Prometheus, Loki, Grafana, Vector; per-component config
  trees; cert/secret generation (gitignored, folded into `gen-keys-and-certs.sh`);
  up/down/rebuild scripts (compose profile); layer into main compose + devcontainer
  (symlinks); `dev-doctor.sh` updates. Verify stack health via each component API.
- [x] **WP02 - Telemetry foundation + server enablement.** `udex-telemetry` crate;
  `observability` config section (schema + validation + defaults); OTLP
  traces/metrics/hybrid-logs wired into server startup with graceful on/off +
  sampling. Verify traces in Tempo, metrics in Prometheus, OTLP logs in Loki.
- [x] **WP03 - Deep instrumentation (datastore + server).** sqlx client query
  spans; server handler spans + core metrics (per-method counts/latencies); gRPC
  server-side W3C context extraction; validate `postgresqlreceiver` against the
  live DB.
- [x] **WP04 - SDK + CLI client tracing.** SDK client spans + W3C `traceparent`
  injection interceptor, provider-free and host-span-integrable; CLI (reference
  client) enables telemetry via `udex-telemetry`.
- [x] **WP05 - K8s deployment with observability.** Collector in-cluster; Helm
  values/templates; app pods pointed at the OTLP endpoint with full trace +
  metric sampling for dev.
- [ ] **WP06 - Integration tests (idempotent).** Assert signals land by querying
  component APIs/data sources (Tempo / Prometheus / Loki + postgres metrics). New
  `test_obs_` / `test_obs_k8s_` prefixes.
- [ ] **WP07 - Docs.** ARCHITECTURE.md observability section + mermaid diagrams;
  README updates (compose / observability / k8s); CONTRIBUTING test-naming + new
  prefixes; SECRETS.md for any new credentials.

## Task Notes

- Each implementation WP includes its own smoke verification; WP06 is the
  comprehensive idempotent suite.
- `udex-telemetry` must be registered in `intent/llm/MODULES.md` before creation
  (Highlander).
- No protobuf change: the `observability` config lives in server YAML config, not
  the gRPC API.

## Dependencies

```text
WP01 -> WP02 -> { WP03, WP04 } -> WP05 -> WP06 -> WP07
```

- WP01 (stack) first - gives backends to point at.
- WP02 (foundation + server enablement) depends on WP01 for validation.
- WP03 and WP04 both depend on WP02 and can proceed in either order.
- WP05 (k8s) depends on WP02/WP03.
- WP06 (tests) depends on the instrumentation existing across WP02-WP05.
- WP07 (docs) last, capturing the as-built system.
