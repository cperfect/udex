# Tasks - ST0027: ClickHouse observability as an always-on compose fixture

## Tasks

Tracked as work packages (`intent/st/COMPLETED/ST0027/WP/`); all delivered.

- [x] **WP01 — ClickHouse + collector fixture in compose.** Add `clickhouse` + a stock `otel-collector` (OTLP receiver → `clickhouse` exporter) to `projects/compose` as always-on services; prove the app → collector → ClickHouse data path. De-risk: confirm the contrib exporter's schema matches what HyperDX reads.
- [x] **WP02 — HyperDX + Mongo dev UI.** Reader-only UI over ClickHouse with `DEFAULT_CONNECTIONS`/`DEFAULT_SOURCES` pre-provisioning + an auto-register init; never gates readiness. (Drove the bind-mount-free / plaintext-OTLP pivot.)
- [x] **WP03 — Postgres metrics + container-log floor.** `postgresqlreceiver` → ClickHouse metrics; a slim Vector ships postgres/hydra container logs to `otel_logs` (filelog can't name docker logs).
- [x] **WP04 — Test migration to ClickHouse, always-run obs.** ClickHouse SQL query helpers (fail-not-skip); migrate the 3 `test_obs_k8s_*` tests; add the always-run non-k8s `obs.rs` binary; flip the k8s deployment to plaintext.
- [x] **WP05 — Agnostic `otlp_headers` config.** Optional OTLP header support in `udex-telemetry` (+ CLI env) for header-authed backends; values redacted in Debug.
- [x] **WP06 — Decommission + docs + CI.** Delete `projects/observability/`; CI brings obs up and runs the obs tests; refresh `dev-doctor.sh`, `gen-env.sh`, post-create, and the docs.

## Task Notes

- The app stays solution-agnostic throughout: it only emits OTLP to a configurable endpoint; all ClickStack/ClickHouse specifics live in the `projects/compose` fixture.
- Two plans were revised mid-thread and recorded in `design.md`/`impl.md`: TLS → plaintext (bind-mount-free fixture), and "retire Vector" → "retain a slim Vector" (filelog can't attribute docker logs).
- Post-completion follow-ups: OTLP now requires TLS unless `dangerous_allow_non_tls` (and the authz flag was renamed to match); the `danger`/`dangerous` naming was unified to `dangerous_allow_non_tls`.

## Dependencies

- WP01 is the foundation (data path + schema) and gates WP02/WP04.
- WP03 depends on WP01 (collector + ClickHouse exist).
- WP04 depends on WP01 (data path) and WP03 (postgres metrics + logs present for the assertions).
- WP05 is independent and could land any time.
- WP06 depends on WP01–WP04 being in place before the old stack is removed and CI flips.
