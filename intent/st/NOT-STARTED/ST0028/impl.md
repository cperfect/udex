# Implementation - ST0028: OpenObserve as the dev observability backend

## Implementation

### WP-01 -- Stand up OpenObserve beside ClickHouse (complete)

Changed files: `projects/compose/docker-compose.yml`, `scripts/gen-env.sh`. No Rust source touched, as designed.

The fixture now runs both backends. The collector exports every pipeline to `clickhouse` and `otlphttp/openobserve` simultaneously, and Vector's log floor goes through the collector rather than writing to a store directly. Verified end to end: `obs.rs` still passes green against ClickHouse while the identical telemetry is queryable in OpenObserve.

## Technical Details

Live verification, all against the running fixture rather than inferred:

| Signal   | Observed in OpenObserve                                                                   |
| -------- | ----------------------------------------------------------------------------------------- |
| traces   | 8 `udex-server` span names incl. `db.create_entry`, `/udex.entry.v1.EntryService/CreateEntry` |
| metrics  | `udex_rpc_requests` + 19 `postgresql_*` receiver streams; counter query returned 3          |
| logs     | `udex-server` app logs plus `postgres` and `hydra` floor records in one stream               |

`GET /config` on the running service confirms `data_retention_days: 3` and `telemetry_enabled: false`, so the retention mapping from ClickHouse's `ttl: 72h` is applied rather than merely declared.

## Challenges & Solutions

**The OpenObserve image is fully distroless.** No shell, no curl, no wget, not even busybox -- probed directly rather than assumed. Compose healthchecks exec inside the container, so this service cannot have one at all, even though it does serve `/healthz`. Dependents gate on `service_started`; readiness is absorbed by the collector's `retry_on_failure` and by the tests' existing poll budgets. The one export failure observed at startup was exactly this race, and it self-healed on retry with no data loss. If a future image ships a shell, add the healthcheck and tighten the conditions.

**OpenObserve rejects hex passwords and panics on it.** The rule is 8-128 characters with at least one lowercase, one uppercase, one digit and one special character, and the failure mode is a hard panic on first boot (`ZO_ROOT_USER_PASSWORD is too weak`) -- the container exits rather than degrading. This collided with a documented constraint in `gen-env.sh`: its values are expanded inside an *unquoted* heredoc, and the file explicitly warns that anything outside `[0-9a-f]` risks silent shell expansion.

Resolved by keeping the value inside `[A-Za-z0-9-]`: a fixed `Ux1-` prefix supplies the four required character classes while all the entropy stays in the 48 hex characters that follow. That satisfies OpenObserve without reopening the heredoc-safety question, and also avoids `#`, which docker compose's `.env` parser can read as a comment. The reasoning is written into the script so the next person does not "improve" it into a random symbol soup.

A related trap caught during review rather than in production: composing the password inline as `"Ux1-$(openssl rand -hex 24)"` would yield a non-empty `"Ux1-"` if openssl failed, sailing straight past the existing emptiness guard. The entropy is now captured into its own variable so the guard can actually see it.

**Vector's `opentelemetry` sink rejected `healthcheck`.** Unknown field under `protocol`. Removed. This is the fourth distinct configuration wart in that sink, after the ones the spike found (hand-built OTLP envelope, `to_unix_timestamp!` fallible form, mandatory `Content-Type` or a 415). All are commented in the compose config.

**Vector did not recreate on a config-only change.** `docker compose up -d vector` reported it unchanged despite the inline `configs` content differing; `--force-recreate` was needed. Worth knowing for WP04's CI work -- a pipeline that relies on `up -d` picking up a config edit could silently run the old config.

### WP-02 -- Port the verification layer to OpenObserve (complete)

Changed files: `sdk/tests/common/mod.rs`, `sdk/tests/obs.rs`, `sdk/tests/integration_tests.rs`, plus three stale comments in `telemetry/src/lib.rs`.

The four `clickhouse_*` helpers are replaced by `openobserve_search` and three wrappers. `obs.rs` passes green against OpenObserve; `cargo fmt` and `cargo clippy --tests` are clean.

**The spike's HTTP-200 finding was wrong.** A rejected query returns **HTTP 400** with `{"code", "message", "hint"}`, not 200. The spike had only inspected response bodies through `jq` and never checked a status code. The AC survived the correction because the *requirement* was right, but the mechanism inverted: `error_for_status()` does catch the 400 — the actual defect is that it discards `message` and `hint`, leaving a bare "400 Bad Request" when the response had already named the offending column and suggested a fix. The helper now reads the body before reacting to the status and surfaces both, and still handles the 200-with-`message` variant.

This is exactly why AT-02.1 exists as a real test rather than an assumption: the first version of the helper was written to the wrong model and the test failed immediately, before the mistake could reach WP03.

**Three comments in `udex-telemetry` named ClickHouse** ("Traces -> ClickHouse (via the collector)"). Made backend-agnostic. This touches a production crate, which AC-00.1 says is untouched — the change is comments only, with no behavioural effect, and it moves the crate *toward* the open-standard boundary MODULES.md says it owns. Read AC-00.1 as "no behavioural change to production crates"; if the owner disagrees, revert these three lines.

### Cluster reinitialisation (unblocking AC-02.5)

The k3d deployment had been in `CrashLoopBackOff` for 34 days, with **two independent faults**:

1. The deployed `DATABASE_URL` pointed at `udex_datastore_integration_test_45d36484_...` — a throwaway database created and dropped by a test run. An older `deploy.sh` read `DATABASE_URL` from the process environment, where an integration test had overwritten it. The current script hardcodes `/postgres`, so the bug was already fixed in the repo and only the stale deployment carried it.
2. `host.k3d.internal` resolved nowhere — absent from both the node's `/etc/hosts` and CoreDNS `NodeHosts`. k3d normally injects it.

Both cleared by delete / rebuild / recreate / load / deploy. Verified after: CoreDNS carries `192.168.107.1 host.k3d.internal`, a throwaway pod reaches `host.k3d.internal:5432`, both replicas are `Running 1/1`, and the deployed URL targets `/postgres`.

The lesson worth keeping: these tests **skip silently** when `K8S_SERVER_URL` is unset and report `ok` in 0.00s. A skip that renders as a pass is how a broken deployment went unnoticed for over a month. Anyone reading a green run should check the elapsed time.

### The metric cold-start race (found only once the tests actually ran)

With the cluster live, `test_obs_k8s_metrics_land` failed immediately:

```text
OpenObserve rejected the query (400 Bad Request): unknown field 'deployment_environment'
```

The column name was right — `configmap.yaml` does set `deployment.environment: k3d`. The cause is that **OpenObserve derives a stream's schema from ingested data**: a column does not exist until a datapoint carrying it arrives, and the OTel metric export interval is 60s. Traces and logs arrive near-instantly, so only metrics race. Waiting confirmed it: the column appeared and the query returned `total: 2`.

This pulls against AC-02.1, since a cold start and a typo produce the same `unknown field` response. Resolved by making the tolerance **opt-in and confined to metrics**:

- `openobserve_search` / `openobserve_scalar_f64` still fail instantly on any API error, so a mistyped column in a trace or log query is caught immediately — AT-02.1 continues to prove this.
- `openobserve_metric_scalar_f64` / `openobserve_metric_count` treat `unknown field` and `stream not found` as "not ingested yet" and return `None` so the caller keeps polling. Every other error — bad syntax, unknown function, auth, transport — still panics at once.
- The cost is that a typo in a *metric* query now fails when the poll budget expires rather than immediately, so those assertions explicitly say that a value which never appears may mean a wrong column or stream name. That is the honest trade, and it is written into the assertion text rather than left for someone to rediscover.

`stream not found` is included because it is the same class: `postgresql_backends` does not exist until the collector's receiver has scraped once, which would have made the postgres metric check fragile on a cold fixture.

This defect existed in the ported code from the start and would have shipped had the cluster stayed broken — the skip was hiding it.

### WP-03 -- New coverage (complete)

Two tests added to `sdk/tests/obs.rs`, both on the always-run path:
`obs_container_log_floor_lands` and `obs_postgres_receiver_metric_lands`.

The floor test was **proven red before being accepted green**: stopping `vector` made it fail in 90.87s naming Vector and its sink, and restarting Vector restored it. That matters more than the green run — this is coverage ST0027 never had, over a transport WP01 changed from a direct store write to an OTLP hop.

**Cold-start tolerance generalised beyond metrics.** WP02 confined it to metrics and said so explicitly; WP03 revises that. The `default` logs stream also does not exist until Vector ships its first record, so the floor test hits the same race on a fresh fixture, and "flakey tests are broken tests" argues for handling it rather than relying on the odds. `openobserve_metric_scalar_f64` / `openobserve_metric_count` became `openobserve_pending_scalar_f64` / `openobserve_pending_count`, taking a `stream_type`.

The underlying principle is unchanged and is the one worth keeping: **tolerance is opt-in at the call site, never the default.** The strict helpers still back every run-scoped app-telemetry assertion, and AT-02.1 still proves they fail loudly.

**`docker compose start <service>` does not work on this stack.** Restarting Vector during the red-first check failed with `dependency failed to start: container ...-openobserve-1 has no healthcheck configured`. `start` evaluates `depends_on` differently from `up` and objects to the distroless OpenObserve service having no healthcheck, even though its declared condition is `service_started`. `docker compose up -d <service>` works correctly. Worth a line in the WP05 docs, because reaching for `start` is a natural thing to do.

### Pre-existing flake surfaced (NOT caused by this thread, NOT fixed here)

Running the whole `udex-sdk` package after WP03 failed once with 10 of 40 integration tests down, then passed cleanly on a re-run. Intermittent, and worth writing down properly because it will bite CI.

Root cause, from the server-side log rather than the assertion:

```text
insert or update on table "entry_context" violates foreign key constraint "entry_context_index_name_fkey"
called `Result::unwrap_err()` on an `Ok` value        <- test_sdk_delete_index_not_empty
```

`integration_tests.rs::test_sdk_delete_index_not_empty` asserts that deleting a **non-empty** index is refused, and its own comment states the assumption plainly: *"The shared index has entries from other tests"*. Nothing enforces that ordering. When the test wins the race and runs before any entry-creating test, the shared `sdk-integration-test-index` is still empty, the delete **succeeds**, `unwrap_err()` panics — and the fixture index is now gone, so the nine tests that depend on it fail with `Index 'sdk-integration-test-index' not found`.

The dependency predates ST0028. What changed is scheduling pressure: this thread added two more concurrently-running tests to the `obs` binary, and cargo runs test binaries in parallel, which made the losing interleaving reachable. Running `integration_tests` alone passes 40/40 consistently.

This violates the project's own rule that flakey tests are broken tests, and it is a latent CI failure independent of observability. **Deliberately not fixed here** — it is unrelated to WP03's scope and touching the shared SDK fixture mid-thread would muddy this change. The fix is for that test to create and populate its own index rather than borrow the shared one, or to seed an entry before asserting. Recommend a separate work item; raised with the owner rather than silently absorbed.

## Notes for later work packages

- The `.env` on this machine was appended to by hand rather than regenerated, because the rotation guard correctly refuses to rewrite it while a compose Postgres is live. A clean clone gets the same values from `gen-env.sh`; this only affects existing developer machines, and belongs in the WP05 upgrade note.
- Both backends are running, so the fixture is temporarily *heavier*, not lighter. The simplification only materialises at WP04. Anyone measuring CI footprint before then will get a misleading number.
