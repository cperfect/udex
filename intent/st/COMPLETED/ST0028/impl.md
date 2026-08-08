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

### WP-04 -- Retire the old backend; CI and dev-doctor (complete)

`clickhouse`, `hyperdx`, `mongo` and `hyperdx-init` are gone, along with the ClickHouse exporter, its `create_schema`/`ttl` config, the ~1.5KB inline `DEFAULT_SOURCES` blob and the curl-based registration one-shot. The fixture is three services.

**Measured, not assumed** (AC-04.5). Backend image footprint drops from ClickHouse 765MB + HyperDX 929MB + MongoDB 1.04GB = **~2.73GB** to OpenObserve **525MB** — about 81% less. Cold start of the three observability services, from `create` to OpenObserve healthy and the collector ready, is **4s**. There is no before-measurement of the old stack's cold start to compare against; it was never timed and the services are now deleted. What can be said from its configuration is that HyperDX allowed a 30s `start_period` before its healthcheck even began counting and `hyperdx-init` gated on that, so readiness was structurally slower — evidence from config, not a stopwatch, and flagged as such.

CI credentials are **generated per job** rather than stored as repository secrets. The fixture is ephemeral, loopback-only and destroyed with the runner, so a long-lived secret would add rotation burden without adding protection. It also means the pipeline works on the first push with no repository configuration.

`dev-doctor.sh` gained its first observability check (net-new — it had none). Version policy is major-only per the owner's decision. Worth being straight about what that buys on a 0.x product: every OpenObserve release so far is major 0, so it catches a jump to 1.x but not a 0.x minor bump that changes the search API. The image tag in compose remains the authoritative pin, and the check prints the running version so drift from it is at least visible.

`OPENOBSERVE_URL` now follows the existing `HYDRA_*_URL` pattern — defaulted to localhost in `gen-env.sh`, overridden to the service name by `post-create.sh`. Without it, dev-doctor failed inside the devcontainer, where `localhost` is not the host that publishes port 5080.

### A defect that would have broken CI on the first push

Recreating the fixture wiped OpenObserve (no volume, by design), and `obs_local_traces_metrics_logs_land` then failed:

```text
OpenObserve rejected the query (400 Bad Request): unknown field 'udex_test_run'
```

The log baseline used the **strict** helper. On a cold store the logs stream has no `udex_test_run` column — it cannot, until this run's own telemetry lands. WP03 generalised cold-start tolerance beyond metrics but applied it only to the floor test; the trace and log paths were left strict. Every CI run starts with an empty store, so this was not an edge case.

Fixed at the root rather than by sprinkling tolerance: the "capture a baseline, drive traffic, poll for an increase" shape existed as **four hand-rolled copies** across two test binaries that did not agree on cold-start handling — which is precisely how the inconsistency survived. They are now one `openobserve_await` (Highlander).

The tolerance no longer costs loudness. `PendingReason` remembers *why* a poll kept coming back empty and, if the budget expires with every attempt rejected as not-yet-ingested, panics naming the stream or column — while a single successful query clears the memory, so a genuine "ingested but did not increase" still reports as the caller intends. Cold start is tolerated during polling; a wrong name still fails loudly, just at the end instead of immediately. `IN-AG-NO-SILENT-001` holds.

Verified by destroying OpenObserve and running against zero streams: all four obs tests pass. Also tightened `obs_search_surfaces_query_errors`, which asserted the panic names the bad *column* — true only against a warm stream; a cold one reports the missing stream first, so the test was passing on the accident of sibling tests running earlier.

### Operational notes

`docker compose up -d` does **not** reload a changed inline `configs:` block on a running container — the collector ran the old ClickHouse-exporting config for two hours after the exporter was deleted, failing every export with `lookup clickhouse: no such host`. `--force-recreate` is required. Together with the WP03 finding that `docker compose start <service>` fails outright on this stack, the reliable incantation after editing compose is `docker compose up -d --force-recreate <service>`. Both belong in the WP05 docs.

The devcontainer restarting also discards `~/.kube/config` while the k3d containers survive, leaving `kubectl` pointed at nothing. `k3d kubeconfig merge udex --kubeconfig-merge-default` followed by the `0.0.0.0` → `host.docker.internal` patch from `cluster-create.sh` restores it without recreating the cluster.

### WP-05 -- Documentation (complete)

Nine documents updated: `projects/compose/README.md` (the largest edit), `docs/ARCHITECTURE.md`, `docs/DESIGN_DECISIONS.md`, `docs/SECRETS.md`, `.devcontainer/README.md`, `projects/k8s/README.md`, `projects/rust/CONTRIBUTING.md`, `projects/rust/telemetry/README.md` and the root `README.md`.

**The charting guidance was replaced, not deleted** (AC-05.2). The old HyperDX recipes existed because metrics charting is not self-evident — the udex and postgres counters are cumulative, so a raw value is a running total and a developer needs to know to take a rate. That is still true; only the mechanism changed. It changed for the better: OpenObserve answers **PromQL**, so the guidance is now `rate(udex_rpc_requests[5m])` instead of a described sequence of UI dropdowns.

Every documented query was **executed against the running fixture before being written down**, including the histogram quantile (`histogram_quantile(0.95, sum by (le) (rate(udex_rpc_duration_bucket[5m])))`, which needs `_bucket` because OpenObserve splits a histogram across five streams on ingest). The UI routes cited (`/web/logs`, `/web/traces`, `/web/metrics`, `/web/dashboards`, `/web/streams`) were each probed and return 200, rather than being guessed from memory of the product.

This also settles the open question `design.md` recorded — whether OpenObserve's charting story is good enough to replace HyperDX's. It is, and it is better in one specific respect the ST0027 record had listed as a loss: ClickHouse SQL needed explicit `argMax`-per-series handling for cumulative counters, which PromQL does natively.

**The ST0027 decision record was superseded, not rewritten** (AC-05.4). Its ClickHouse sections are intact under a supersession note, followed by a new "Why OpenObserve instead of ClickHouse + HyperDX?" section. The argument that led to ClickHouse is what makes this thread's move legible — deleting it would leave the current design looking arbitrary.

**`docs/SECRETS.md` states the posture change rather than just listing new keys.** The fixture moved from keyless to carrying a credential on a plaintext in-network hop. That is a real weakening, accepted for a dev/CI-only loopback fixture, and it is written down as such alongside the constraint that forces the password's restricted alphabet.

**The doc-reviewer pass earned its place** (AC-05.5): 0 critical, 4 major, 9 minor, 4 suggestions. All majors and minors were applied. Three findings were substantive rather than cosmetic:

- **The "durable log floor" claim was no longer true and I had carried it forward.** Under ST0027 Vector wrote to ClickHouse directly, so the container-log floor survived a collector outage. WP01 rerouted Vector through the collector, which silently gave that property up — and the docs still advertised it. Now corrected in `compose/README.md` and `ARCHITECTURE.md`, and added to the ST0028 costs list in `DESIGN_DECISIONS.md`, where it belonged from the start. This was a real regression recorded nowhere.
- **"Only the collector knows which backend exists" was overstated, in four places.** True of the *pipeline*; false of the repository, since the test helpers, `gen-env.sh`, `dev-doctor.sh` and CI all name OpenObserve directly. The accompanying claim that a backend swap is "a change to `docker-compose.yml` and nothing else" was simply wrong. Scoped in all four.
- **The service count was wrong throughout this thread.** The old fixture had **six** services (`clickhouse`, `otel-collector`, `mongo`, `hyperdx`, `hyperdx-init`, `vector`), not five — `hyperdx-init` was missed in the original count and the error propagated into planning, commit messages and the decision record. Corrected to six-to-three.

Also fixed: credential/login instructions had been copied into three documents, which is exactly how the old HyperDX credentials drifted — `compose/README.md` now owns them and the others link. The pinned-version table in `projects/rust/CONTRIBUTING.md` gained OpenObserve, the collector and Vector, with a pointer to `REQUIRED_OPENOBSERVE_MAJOR`. `.devcontainer/README.md` gained the `OPENOBSERVE_URL` row it was missing. Root `CONTRIBUTING.md` had stale `gen-env.sh` and compose descriptions and had not been in the change set at all. Two pre-existing defects were repaired in passing: a root-absolute broken link and a broken `#access` anchor in `ARCHITECTURE.md`, plus a bare code fence in root `CONTRIBUTING.md`.

I had also manually hard-wrapped ~30 lines of new prose in `SECRETS.md`, against an explicit project rule. Unwrapped.

`.claude/agent-memory/doc-reviewer/staleness_hotspots.md` gained the version pins and three new drift entries, including the durability claim — recorded specifically because it survived a full rewrite and was only caught in review.

Some ClickHouse and HyperDX mentions survive **deliberately** and should not be "fixed" by a later sweep: the retained decision record, the posture comparison in `SECRETS.md`, the "main practical difference from the previous HyperDX setup" line in the compose README, and ClickStack/HyperDX in `telemetry/README.md`, which documents a *third-party* backend a user might target via `otlp_headers` and has nothing to do with our fixture.

## Notes for later work packages

- The `.env` on this machine was appended to by hand rather than regenerated, because the rotation guard correctly refuses to rewrite it while a compose Postgres is live. A clean clone gets the same values from `gen-env.sh`; this only affects existing developer machines, and belongs in the WP05 upgrade note.
- Both backends are running, so the fixture is temporarily *heavier*, not lighter. The simplification only materialises at WP04. Anyone measuring CI footprint before then will get a misleading number.
