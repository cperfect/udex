# Udex Design Decisions

This document captures the rationale behind Udex's core design choices — why the system is shaped the way it is. For operational how-tos, usage guidance, and troubleshooting, see the [FAQ](FAQ.md).

## Data model and invariants

### Why is this not just a KV-store?
Firstly because there are *two keys* per entry;the arbitrary unique key and the context hash and both are valid for lookup purposes. Secondly I wanted it to be highly opinionated about keys, contexts, hashes and the mappings between them to fit the use cases without it becoming a free-for-all. Thirdly because most KV-stores aren't transactional (which is really just a special case of the former).

### Why have a Context Hash?
The context hash firstly makes context duplication detection fast and easy and also makes entry query via context fast and easy (which is basically the same thing). This deliberately shifts the cost of generating the hash for read operations to the Indexer (or other client).

### Why is it transactional?
Transactions enforce data quality and make reasoning and implementation of state management much easier, especially in the kind of distributed systems that integration scenarios involve. Ultimately [state is *hard*](https://istheenemyofgood.medium.com/001-state-is-hard-18fa3b1812ae). Modern transactional datastores are highly-performant and I don't think the tradeoff with non-transactional implementations is worth it for Udex.

### Why are keys:contexts 1:1?
The driving use case is cross-party key resolution. If Party A sends the same entity twice — because of a network retry, a restart, or a race — Udex must return the same stable key both times. Returning a different key would break any downstream system that cached or stored the first one.

The 1:1 invariant provides that guarantee at the database level: one context fingerprint maps to exactly one entry key, always. No application-layer deduplication is needed, and `create_entry` is safe to call from retry-prone systems without coordination.

### Why are contexts immutable?
If a context could be updated, its fingerprint would change silently — any caller holding the old key could get back a different context, breaking the stable-key guarantee that is the point of Udex. Immutability makes the key permanent: whatever was true when the entry was created stays true, and callers that need a different mapping encode that intent in a new context (see [How do I handle key migrations?](FAQ.md#how-do-i-handle-key-migrations)).

## Technology and interface choices

### Why Rust?
As a high-performance, high-reliability back-end system I wanted it to be something that handled concurrency well and was memory safe. My preferred language for this would have been Go but I wanted an excuse to learn Rust.

### Why RPC and not REST?
I think the RPC model suits Udex better than REST, especially for the bulk scenarios. gRPC in particular is more network efficient than HTTP REST. I also think that, as a service involved in integration and data governance, it would be better for the API to avoid breaking changes and keep backwards compatibility as far as possible and the protobuf model suits this, which in turn leads to RPC.

### Why is this all driven by a CLI?
The CLI serves two purposes: operational control (starting and stopping the server, managing indices and entries) and a scriptable, auditable client surface for development and testing that does not require building a custom client.

### Why YAML for configuration (not TOML)?
Udex uses YAML for its configuration file. YAML is effectively a universal configuration standard and is the default format in Kubernetes, where Udex is deployed — the Helm chart and k8s manifests are already YAML. Using YAML for the application config as well means operators work in a single syntax end-to-end and removes the format mismatch between Udex's config and the manifests that surround it. This is a deliberate replacement of the original TOML format, not an additional option: there is one config format and one parser.

### Why does Udex use the gRPC Health Checking Protocol instead of a custom healthz endpoint?

Udex implements the [standard gRPC Health Checking Protocol](https://github.com/grpc/grpc/blob/master/doc/health-checking.md) (`grpc.health.v1.Health`) rather than a bespoke healthz service for four reasons:

- **Broad tooling support** — the standard is understood natively by Kubernetes, Envoy, Istio, `grpc-health-probe`, and any gRPC-aware load balancer. No Udex-specific client code or configuration is required.
- **Per-service granularity** — the protocol carries a service name (`""` for overall server, `"udex.entry.v1.EntryService"`, `"udex.index.v1.IndexService"`). This is built in; a custom proto would duplicate the concern.
- **Enables native Kubernetes `grpc` probes** — Kubernetes has supported native `grpc` probe type since 1.24 (beta, enabled by default; GA in 1.27). These probes actually exercise the gRPC stack, whereas `tcpSocket` probes only establish a TCP connection (no TLS handshake, no gRPC). The current deployment uses `tcpSocket` because the server is TLS-only on port 443 and native `grpc` probes do not support TLS; a future non-TLS health port would remove that constraint.
- **The old `udex.healthz.v1` proto was bespoke** — it had no tooling support outside this repository and imposed a custom client on every caller that needed to check server health.

## Observability

> **Superseded in part (ST0028).** The reasoning below records why the fixture moved from the Grafana stack to ClickHouse + HyperDX, and it still explains the shape the fixture has today — one always-on store, declaratively configured, with the application coupled only to OTLP. What changed is *which* store: ST0028 replaced ClickHouse, HyperDX and MongoDB with a single **OpenObserve** service. See [Why OpenObserve instead of ClickHouse + HyperDX?](#why-openobserve-instead-of-clickhouse--hyperdx) below. The ClickHouse sections are kept rather than rewritten, because the argument that led there is what makes the later move legible.

### Why ClickHouse instead of the original Grafana stack (Tempo/Prometheus/Loki)?

The first local observability stack (ST0026) was six services: an OTel Collector fanning out to **Tempo** (traces), **Prometheus** (metrics), and **Loki** (logs), with **Grafana** for the UI and **Vector** shipping container logs. That is four separate stores — each with its own query language, retention, and operational surface — plus a bespoke separate-compose-project that had to attach to the running base stack's network by hand.

ST0027 replaced it with a single **ClickHouse**-backed pipeline. ClickHouse stores traces, metrics, and logs in one SQL-queryable store with one retention story, so four backends and the collector's fan-out collapse to a store plus a collector. That is far fewer moving parts, it folds cleanly into the base `projects/compose` stack as an always-on fixture (present like PostgreSQL and Hydra, so the observability tests can be a hard dependency rather than skip), and it puts all three signals in one place for cross-signal correlation. Crucially the application was untouched: it only ever emits OTLP, so swapping the backend is exactly the "any OTel-compatible backend can be substituted, never the application" property the design is built around.

### Why the modular ClickStack and not the all-in-one image?

ClickStack ships an **all-in-one** image (the HyperDX UI + ClickHouse + MongoDB + a collector in one container). A spike evaluated it and found three things that make it unfit for a deterministic, git-tracked, always-on dev/CI fixture:

- **Its collector config is control-plane-driven, not declarative.** The bundled collector is OpAMP-supervised — its configuration is pushed at runtime by HyperDX and stored in MongoDB, edited through the UI. OTLP ingestion is not even wired until a first-run user/team is created. That conflicts with "configuration must not be mutated at runtime" and with keeping config in git, and it breaks deterministic CI bring-up (there is a manual setup step before anything works).
- **Ingestion is gated by a per-team key.** After setup, the collector requires an `Authorization` header carrying a key generated at first-run. Using the all-in-one would mean injecting that runtime-generated secret into the application's OTLP config — which would couple the app to a specific backend and break the solution-agnostic boundary that is the whole point.
- **(TLS entanglement.)** A minor third factor, later moot once we relaxed the *local* OTLP hop to plaintext.

So Udex runs the **modular** shape instead: our own stock OpenTelemetry Collector (the contrib `clickhouse` exporter) with static, version-controlled config, exporting to ClickHouse, with HyperDX riding on top purely as a reader-only UI. This dissolves all three problems — no OpAMP, no runtime config, no per-team key — and the application keeps emitting plain, keyless OTLP. The cost is a few more containers than a single image, but each is declaratively configured and the tests query ClickHouse directly (they never depend on the UI). To keep the door open for users who *do* want a header-authed backend (including the all-in-one), the telemetry config exposes an optional `otlp_headers` map — the agnostic escape hatch, added without coupling the application to any backend.

### What are the trade-offs versus the original Grafana stack, and why accept them?

The replaced stack had real strengths: Grafana, Tempo, Prometheus, and Loki are industry-standard, mature, purpose-built tools, with familiar query languages (PromQL/LogQL/TraceQL) and component isolation (one store failing does not take the others down). The ClickHouse approach gives some of that up: a single store is a single point of failure for all signals, HyperDX is less mature and less polished than Grafana (and has no no-auth mode, so a local user is auto-registered for it), and ClickHouse SQL is less ergonomic than purpose-built telemetry languages for some queries (cumulative-counter metrics, for instance, need explicit `argMax`-per-series handling that PromQL does natively). The local OTLP hop was also relaxed from TLS to plaintext (explicitly gated by `dangerous_allow_non_tls`) because the bind-mount-free fixture cannot mount cert files.

We accept those trade-offs because **the goal of this fixture is dev/test simplicity and replicability, not running a production observability platform** — which is precisely why ST0027 exists. For a thing that must come up identically on every developer's machine and in CI, with zero manual steps, and that the test suite can depend on as a hard requirement, "one store, always-on, no network dance, queryable in plain SQL" beats "four best-of-breed stores behind a bespoke side-project." The losses (UI polish, per-signal query ergonomics, store isolation) barely matter for an ephemeral, human-facing-only-in-dev aid, while the wins (fewer moving parts, deterministic bring-up, enforced test coverage) directly target what made the old stack painful. The trade-off would invert at production scale — but that was never this fixture's job, and because the application stays OTLP-agnostic, a real deployment can point at Grafana Cloud, Honeycomb, or anything else with no code change. The fixture optimises for *reproducible development and testing*; production backend choice is left open by design.

### Why OpenObserve instead of ClickHouse + HyperDX?

ST0027's argument was "collapse the stores to reduce moving parts." It worked, but it stopped one step short: the *store* collapsed to one while the *fixture* did not. ClickHouse held the data, HyperDX read it, MongoDB existed only to hold HyperDX's own state, and a `hyperdx-init` one-shot registered a local user so a developer never met a signup form. Six services, a ~1.5KB inline `DEFAULT_SOURCES` JSON blob to pre-provision datasources, and three components (HyperDX, Mongo, the init job) that existed solely to make a UI usable.

ST0028 replaced all four with **OpenObserve**, which is store, query API and UI in one binary. In local mode it keeps metadata in SQLite and segments on local disk, so it needs no companion database and no bootstrap. The fixture goes from six services to three: OpenObserve, the collector, and Vector. Backend image footprint drops from about 2.73GB to 525MB. The three observability services cold-start in about 4 seconds — an absolute measurement, not a comparison; the old stack's startup was never timed before it was removed.

Two properties made this worth doing rather than merely tidy:

- **The UI stops being a separate concern.** No datasource seeding, no state database, no user-registration job — the parts most likely to rot were the parts that only existed to serve the reader UI.
- **Metrics get a real query language.** ClickHouse SQL needed explicit `argMax`-per-series handling for cumulative counters (a trade-off named as a loss in the section above); OpenObserve answers PromQL, so `rate(udex_rpc_requests[5m])` says what it means. That recovers one of the specific things the Grafana stack was better at.

The application was again untouched. It emits anonymous OTLP to a configurable endpoint, and OpenObserve's requirement for credentials on ingest and query is terminated in the **collector's** exporter config — the same separation ST0027 used to keep ClickStack's per-team key away from the app. Vector's log floor was rerouted through the collector for the same reason, so the collector's exporter is the only part of the telemetry pipeline that names a backend. (The test helpers, `gen-env.sh`, `dev-doctor.sh` and CI all name it too — they query or provision it directly. The agnostic boundary is the pipeline, not the repository.)

**Storage is local disk, not object storage.** MinIO was the original proposal and was rejected: it reintroduces a stateful service and a bucket bootstrap to a fixture whose entire purpose is being light, and the object-storage path is not a Udex capability under test. The OTel capability that matters is the application's OTLP emission, which is identical either way.

The costs, stated rather than glossed:

- **A credential now crosses the compose network.** ClickHouse was keyless. OpenObserve requires authentication, so the fixture carries a generated password and a derived Basic header over a plaintext in-network hop. Dev-only and loopback-published, but a genuine weakening — see [docs/SECRETS.md](SECRETS.md#observability).
- **OpenObserve is a 0.x product**, less mature than ClickHouse, and its image is distroless — so it cannot host a compose healthcheck at all, and dependents gate on `service_started` instead.
- **Schema is derived from ingested data**, so a stream or column does not exist until something writes it. That is convenient (nothing to provision) but means queries against a cold fixture must tolerate not-yet-ingested responses without letting a genuinely wrong column name pass unnoticed.
- **The container-log floor lost its independence.** Under ST0027 Vector wrote to the store directly, so postgres/hydra logs survived a collector outage. Routing Vector through the collector — done so that only the collector names a backend — means an outage now takes both. The trade was accepted because the *application's* durable floor is its own JSON-on-stdout, which is unaffected, and because a dev fixture is not where log durability is load-bearing. It is still a property that was given up rather than one that carried over.

The reasoning is the same one that justified ST0027, applied once more: for a fixture that must come up identically everywhere with zero manual steps, fewer moving parts wins, and the application's OTLP-agnostic boundary is what makes the swap cheap enough to be worth making twice.

For the as-built detail and the work-package history, see steel threads ST0027 and ST0028.

## Scope and non-goals

### What won't Udex support?
* Non-transactional datastores
* Complex/aggregate cross-context queries

