---
verblock: "20 May 2026:v0.2: Chris Perfect / Claude - WP04 as-built"
---

# ST0019 Implementation Notes

## WP04 — Documentation and FAQ

- `docs/FAQ.md`: new entry "Why does Udex use the gRPC Health Checking Protocol instead of a custom healthz endpoint?" — covers tooling support, per-service granularity, native k8s probe enablement, and that the old proto was bespoke. Future-features bullet added for non-TLS health port.
- `docs/ARCHITECTURE.md`: added unauthenticated endpoint note (three registered services) in the Authentication & Authorization section; links to FAQ.
- `projects/rust/server/README.md`: removed `healthz.rs` row; auth section updated to reference `grpc.health.v1.Health`.
- `projects/rust/api/README.md`: removed `healthz (udex.healthz.v1)` row.
- `projects/k8s/README.md`: new "Health probes" section — registered services table, why `tcpSocket` is used instead of native `grpc` probes (TLS constraint), and how to probe manually with `grpc-health-probe`.

## WP03 — Helm chart probe comment

Native k8s `grpc` probes do not support TLS (confirmed on 1.31.5). The server is TLS-only on port 443, so native probes would fail at the TLS handshake. `tcpSocket` probes are retained. The TODO comment was replaced with an explanation of the constraint and the future path (non-TLS health port enables native grpc probes without any extra binary). Helm lint passes.

## WP01+WP02 — tonic-health wire-up and test fixture migration

### As-built

**Server side (WP01)**

- `tonic-health = "0.13"` added to workspace `Cargo.toml` and `server/Cargo.toml`.
- `tonic_health::server::health_reporter()` called at startup in `server.rs`; returns `(HealthReporter, HealthServer<HealthService>)` — the service is added directly to the tonic router (no re-wrapping needed).
- The overall `""` service is set to `SERVING` in `server.rs` after both `IndexService::init()` and `EntryService::init()` complete successfully, so the overall status only becomes healthy once all services are ready.
- `HealthReporter` is cloned and passed to `IndexService::new()` and `EntryService::new()` as a second argument.
- Each service calls `reporter.set_service_status("udex.{entry,index}.v1.{Entry,Index}Service", ServingStatus::Serving)` at the end of its `init()` method.
- The `HealthCheck` trait and its `is_healthy()` impls were removed from `entry.rs`, `index.rs`, and `lib.rs`.
- `healthz.rs` deleted; `pub mod healthz` and `pub use healthz::HealthzService` removed from `lib.rs`.
- `udex.healthz.v1.proto` deleted; `api/build.rs` updated to remove it from the compile list and from the generated `mod.rs`.
- `api/src/lib.rs`: `pub use generated::udex::healthz::v1 as healthz` removed.
- `api/src/generated/udex.healthz.v1.rs` deleted.

**Test fixtures (WP02)**

Six test files updated (WP02 listed 5; `sdk/tests/integration_tests.rs` was also required):

| File | Change |
|------|--------|
| `server/tests/server_integration_tests.rs` | `test_server_healthz` renamed to `test_server_health_check`; assertions on `is_healthy`/`server_time`/`status_messages` replaced with `status == ServingStatus::Serving` |
| `server/tests/entry_service_integration_tests.rs` | Removed `HealthCheck` import; `test_entry_service_init` stripped of `is_healthy()` call (health verified end-to-end by `test_server_health_check`); service constructors given no-op reporters |
| `server/tests/index_service_integration_tests.rs` | Service constructor given no-op reporter |
| `cli/tests/serve_live_tests.rs` | `wait_for_ready` and `test_serve_healthz_over_tls` renamed to `test_serve_health_check_over_tls` |
| `cli/tests/entry_live_tests.rs` | `wait_for_server` updated |
| `cli/tests/index_oauth2_tests.rs` | `wait_for_server` updated |
| `cli/tests/token_oauth2_tests.rs` | `wait_for_server` updated |
| `sdk/tests/integration_tests.rs` | `wait_for_server` and k8s health poll updated |

`tonic-health` added as dev-dependency to `cli/Cargo.toml` and `sdk/Cargo.toml`.

### Design notes

- `HealthReporter::set_service_status` takes `&self` (Arc-backed) — no `mut` needed on the binding.
- `health_reporter()` returns the service already wrapped in `HealthServer<HealthService>`; `tonic_health::server::HealthServer` is private so never reference it directly — just pass the service value to `add_service()`.
- Reactive status updates (NOT_SERVING on datastore errors) are not implemented. At startup, all three registered services (`""`, `EntryService`, `IndexService`) are SERVING and stay that way. A dead process fails k8s liveness probes naturally. Proactive NOT_SERVING signals would require distinguishing transient from permanent errors — out of scope for this ST.
- The health service is unauthenticated, consistent with the old `HealthzService`.
- `HealthClient` (test-side) does not have a `connect()` convenience method; the test pattern is `endpoint.connect().await?` then `HealthClient::new(channel)`.
